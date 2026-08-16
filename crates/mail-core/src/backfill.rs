//! Rattrapage des corps de messages — la pompe de fond de l'[ADR 0007].
//!
//! La synchro « enveloppes d'abord » (PLAN.md §3) rend la liste utilisable
//! instantanément, mais ne télécharge un corps qu'au clic. Mesuré sur le
//! terrain : 18 corps sur 537, 1 sur 2193. La recherche « plein texte » ne
//! portait donc, en pratique, que sur les sujets et les expéditeurs.
//!
//! Cette pompe complète la synchro sans la contredire : elle passe APRÈS,
//! en tâche de fond, et rapatrie les corps des messages récents.
//!
//! Trois propriétés la définissent :
//!
//! - **bornée** : un horizon de récence et un budget par passage, pour que
//!   le coût reste prévisible (< 1 Go, PLAN.md §1) ;
//! - **reprenable** : elle ne tient aucun curseur — l'état, c'est la base.
//!   Un corps déjà écrit sort de la liste des manquants, donc une coupure
//!   ne coûte que le lot en cours ;
//! - **groupée** : un aller-retour par message coûte ~192 ms sur un
//!   serveur réel (`spikes/body-backfill`). On demande les corps par lots.
//!
//! [ADR 0007]: ../../../docs/adr/0007-rattrapage-des-corps.md

use std::collections::HashSet;

use crate::envelope::Uid;
use crate::error::Error;
use crate::remote::MailServer;
use crate::store::Store;

/// Corps demandés en une commande. 50 est le compromis retenu : assez pour
/// amortir l'aller-retour, assez peu pour qu'une coupure ne perde qu'un
/// petit lot et que l'avancement reste vivant à l'écran.
pub const BACKFILL_BATCH: usize = 50;

/// « Pas d'horizon » : la valeur de `since_epoch` qui ne borne rien.
///
/// L'horizon de 12 mois de l'[ADR 0007] existait pour tenir le budget
/// disque (< 1 Go). L'[ADR 0010] lève ce budget : la production passe
/// désormais cette constante, et la borne ne survit qu'en paramètre —
/// les tests s'en servent pour rejouer des scénarios bornés, et un futur
/// réglage utilisateur la retrouverait telle quelle.
///
/// `i64::MIN` et non `0` : une date antérieure à 1970 — horloge fausse,
/// en-tête corrompu — produit un epoch négatif, et « tout » doit le
/// couvrir aussi.
///
/// [ADR 0010]: ../../../docs/adr/0010-synchronisation-integrale.md
pub const NO_HORIZON: i64 = i64::MIN;

/// Ce qu'un passage a fait, et ce qu'il reste à faire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackfillReport {
    /// Corps rapatriés et indexés pendant ce passage.
    pub fetched: usize,
    /// Messages de l'horizon qui attendent encore leur corps.
    pub remaining: u64,
}

/// Rapatrie jusqu'à `budget` corps manquants, du plus récent au plus
/// ancien, et les indexe au passage (c'est [`Store::save_body`] qui s'en
/// charge, dans sa transaction).
///
/// `since_epoch` est l'horizon : au-delà, on ne rapatrie rien.
pub fn backfill_bodies(
    server: &mut dyn MailServer,
    store: &mut Store,
    account_id: i64,
    mailbox: &str,
    since_epoch: i64,
    budget: usize,
) -> Result<BackfillReport, Error> {
    let Some(state) = store.sync_state(account_id, mailbox)? else {
        return Ok(BackfillReport {
            fetched: 0,
            remaining: 0,
        });
    };

    let mut fetched = 0usize;
    // Les UIDs déjà tentés dans CE passage. Sans cette mémoire, un message
    // que le serveur ne sert plus reviendrait à chaque tour dans la liste
    // des manquants — et la pompe tournerait sans fin.
    let mut attempted: HashSet<Uid> = HashSet::new();

    while fetched < budget {
        let window = (budget - fetched + attempted.len()).min(BACKFILL_BATCH + attempted.len());
        let candidates = store.bodies_to_backfill(account_id, mailbox, since_epoch, window)?;
        let batch: Vec<Uid> = candidates
            .into_iter()
            .filter(|uid| !attempted.contains(uid))
            .take((budget - fetched).min(BACKFILL_BATCH))
            .collect();
        if batch.is_empty() {
            break;
        }
        attempted.extend(batch.iter().copied());

        for (uid, body) in server.fetch_bodies_html(mailbox, &batch)? {
            store.save_body(state.mailbox_id, uid, &body.html, &body.attachments)?;
            fetched += 1;
        }
    }

    Ok(BackfillReport {
        fetched,
        remaining: store.bodies_pending_count(account_id, mailbox, since_epoch)?,
    })
}

/// En-têtes demandés en une commande. Bien plus que les corps : un bloc
/// d'en-têtes pèse ~3 ko contre ~50 ko pour un message entier, et la
/// dépense qui compte ici est l'aller-retour, pas les octets.
pub const THREAD_HEADER_BATCH: usize = 200;

/// Rapatrie les destinataires (À/Cc) manquants d'un dossier d'envois, du
/// plus récent au plus ancien (R4, rattrapage des envois — D2,
/// PLAN-RETOURS-MAIL).
///
/// Même forme bornée/reprenable/groupée que [`backfill_bodies`], mais elle
/// relit l'ENVELOPE — où À/Cc voyagent gratuitement avec l'expéditeur, sans
/// un octet de corps — et n'écrit QUE ces deux colonnes (jamais le fil ni
/// `refs`). Dans un dossier d'envois l'expéditeur est SOI : sans le
/// destinataire, ni la liste ni le volet ne peuvent dire à qui le message
/// est parti. La passe d'en-têtes ayant convergé, les envois déjà
/// synchronisés n'ont aucun destinataire en base — c'est cette pompe qui
/// les rattrape.
pub fn backfill_recipients(
    server: &mut dyn MailServer,
    store: &mut Store,
    account_id: i64,
    mailbox: &str,
    budget: usize,
) -> Result<BackfillReport, Error> {
    let Some(state) = store.sync_state(account_id, mailbox)? else {
        return Ok(BackfillReport {
            fetched: 0,
            remaining: 0,
        });
    };

    let mut fetched = 0usize;
    // Les UIDs déjà tentés dans CE passage — un message que le serveur ne
    // sert plus ne doit pas faire tourner la pompe sans fin (même garde
    // que [`backfill_bodies`]).
    let mut attempted: HashSet<Uid> = HashSet::new();

    while fetched < budget {
        let window =
            (budget - fetched + attempted.len()).min(THREAD_HEADER_BATCH + attempted.len());
        let candidates = store.recipients_to_backfill(account_id, mailbox, window)?;
        let batch: Vec<Uid> = candidates
            .into_iter()
            .filter(|uid| !attempted.contains(uid))
            .take((budget - fetched).min(THREAD_HEADER_BATCH))
            .collect();
        if batch.is_empty() {
            break;
        }
        attempted.extend(batch.iter().copied());

        for envelope in server.fetch_envelopes(mailbox, &batch)? {
            store.set_recipients(
                state.mailbox_id,
                envelope.uid,
                &envelope.to_addrs,
                &envelope.cc_addrs,
            )?;
            fetched += 1;
        }
    }

    Ok(BackfillReport {
        fetched,
        remaining: store.recipients_pending_count(account_id, mailbox)?,
    })
}

/// Rapatrie les en-têtes de fil manquants, du plus récent au plus ancien,
/// et recolle les conversations au passage.
///
/// Même forme que [`backfill_bodies`] — bornée, reprenable, groupée — mais
/// une raison d'être différente : celle-ci ne complète pas la recherche,
/// elle répare le REGROUPEMENT. Un message dont on n'a jamais lu les
/// `References` reste seul dans son fil alors qu'il appartient à un
/// échange.
pub fn backfill_thread_headers(
    server: &mut dyn MailServer,
    store: &mut Store,
    account_id: i64,
    mailbox: &str,
    since_epoch: i64,
    budget: usize,
) -> Result<BackfillReport, Error> {
    let Some(state) = store.sync_state(account_id, mailbox)? else {
        return Ok(BackfillReport {
            fetched: 0,
            remaining: 0,
        });
    };

    let mut fetched = 0usize;
    // Même garde que pour les corps : un message que le serveur ne sert
    // plus reviendrait sinon à chaque tour, et la pompe tournerait sans fin.
    let mut attempted: HashSet<Uid> = HashSet::new();

    while fetched < budget {
        let window =
            (budget - fetched + attempted.len()).min(THREAD_HEADER_BATCH + attempted.len());
        let candidates =
            store.thread_headers_to_backfill(account_id, mailbox, since_epoch, window)?;
        let batch: Vec<Uid> = candidates
            .into_iter()
            .filter(|uid| !attempted.contains(uid))
            .take((budget - fetched).min(THREAD_HEADER_BATCH))
            .collect();
        if batch.is_empty() {
            break;
        }
        attempted.extend(batch.iter().copied());

        for (uid, headers) in server.fetch_thread_headers(mailbox, &batch)? {
            store.set_thread_headers(
                state.mailbox_id,
                uid,
                headers.in_reply_to.as_deref(),
                headers.references.as_deref().unwrap_or_default(),
            )?;
            fetched += 1;
        }
    }

    Ok(BackfillReport {
        fetched,
        remaining: store.thread_headers_pending_count(account_id, mailbox, since_epoch)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::FakeServer;

    /// Décor : `n` messages avec corps sur le serveur, synchronisés (donc
    /// enveloppes en base) mais aucun corps téléchargé.
    fn synced(n: u32) -> (FakeServer, Store, i64) {
        let mut server = FakeServer::new(false);
        for uid in 1..=n {
            server.add_with_body(uid, &format!("sujet {uid}"), &format!("<p>corps {uid}</p>"));
        }
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("test@exemple.fr", "gmail")
            .unwrap();
        crate::SyncEngine::default()
            .sync(&mut server, &mut store, account, "INBOX")
            .unwrap();
        (server, store, account)
    }

    /// La raison d'être de la pompe : après son passage, un mot du CORPS
    /// est trouvable — ce qui était impossible avant.
    #[test]
    fn backfilled_bodies_become_searchable() {
        let (mut server, mut store, account) = synced(3);
        server
            .bodies
            .insert(2, "<p>le contrat de licence</p>".to_string());
        assert_eq!(store.search("contrat", 10).unwrap().len(), 0);

        let report = backfill_bodies(&mut server, &mut store, account, "INBOX", 0, 100).unwrap();

        assert_eq!(report.fetched, 3);
        assert_eq!(report.remaining, 0);
        assert_eq!(
            store.search("contrat", 10).unwrap().len(),
            1,
            "le corps rapatrié doit être indexé"
        );
    }

    /// R4, rattrapage des envois (D2) : un envoi déjà synchronisé sans
    /// destinataire en base (l'ancien schéma) reçoit, après la passe, l'À
    /// et le Cc que l'ENVELOPE du serveur portait. La pompe CONVERGE : un
    /// second passage ne redemande rien.
    #[test]
    fn backfill_recipients_remplit_les_envois_sans_destinataire() {
        let (mut server, mut store, account) = synced(3);
        // L'ENVELOPE complète vit sur le serveur ; la base, elle, n'a rien
        // (état hérité — `synced` n'écrit aucun destinataire).
        server.set_envelope_recipients(2, &["sebastien.monchamps@gmail.com"], &["copie@x.fr"]);
        assert_eq!(
            store.recipients_pending_count(account, "INBOX").unwrap(),
            3,
            "les trois envois attendent leurs destinataires"
        );

        let report = backfill_recipients(&mut server, &mut store, account, "INBOX", 100).unwrap();
        assert_eq!(report.fetched, 3);
        assert_eq!(report.remaining, 0);

        let relu = store.recent(account, "INBOX", 0, 10).unwrap();
        let m2 = relu.iter().find(|e| e.uid == 2).unwrap();
        assert_eq!(m2.to_addrs, vec!["sebastien.monchamps@gmail.com"]);
        assert_eq!(m2.cc_addrs, vec!["copie@x.fr"]);

        // Convergence : le second passage ne redemande rien (le message
        // sans destinataire porte désormais la marque vide, pas NULL).
        let second = backfill_recipients(&mut server, &mut store, account, "INBOX", 100).unwrap();
        assert_eq!(second.fetched, 0);
        assert_eq!(second.remaining, 0);
    }

    /// Le cœur du gain mesuré : une commande pour tout le lot, pas un
    /// aller-retour par message.
    #[test]
    fn backfill_groups_its_fetches() {
        let (mut server, mut store, account) = synced(5);

        backfill_bodies(&mut server, &mut store, account, "INBOX", 0, 100).unwrap();

        assert_eq!(
            server.body_batches.len(),
            1,
            "5 corps doivent tenir en UNE commande, pas 5"
        );
        assert_eq!(server.body_batches[0].len(), 5);
        assert_eq!(
            server.body_fetches, 0,
            "le chemin unitaire ne doit pas être emprunté"
        );
    }

    /// Le budget borne le passage : c'est lui qui empêche un rattrapage de
    /// monopoliser le réseau.
    #[test]
    fn backfill_stops_at_its_budget() {
        let (mut server, mut store, account) = synced(10);

        let report = backfill_bodies(&mut server, &mut store, account, "INBOX", 0, 4).unwrap();

        assert_eq!(report.fetched, 4);
        assert_eq!(report.remaining, 6);
    }

    /// Reprise après coupure : aucun curseur à restaurer, l'état c'est la
    /// base. Le second passage continue sans refaire le travail du premier.
    #[test]
    fn backfill_resumes_where_it_stopped_without_redoing_work() {
        let (mut server, mut store, account) = synced(6);

        let first = backfill_bodies(&mut server, &mut store, account, "INBOX", 0, 2).unwrap();
        let second = backfill_bodies(&mut server, &mut store, account, "INBOX", 0, 2).unwrap();

        assert_eq!(first.fetched, 2);
        assert_eq!(second.fetched, 2);
        assert_eq!(second.remaining, 2);
        // Les deux passages ont demandé des UIDs DIFFÉRENTS.
        let demandés: Vec<Uid> = server.body_batches.concat();
        let uniques: HashSet<Uid> = demandés.iter().copied().collect();
        assert_eq!(
            demandés.len(),
            uniques.len(),
            "aucun corps ne doit être redemandé"
        );
    }

    /// Les plus récents d'abord : c'est là que la recherche a le plus de
    /// valeur, et ça rend un rattrapage interrompu utile malgré tout.
    #[test]
    fn backfill_starts_with_the_newest() {
        let (mut server, mut store, account) = synced(5);

        backfill_bodies(&mut server, &mut store, account, "INBOX", 0, 2).unwrap();

        assert_eq!(server.body_batches[0], vec![5, 4]);
    }

    /// LE piège : un message que le serveur ne sert plus reste éternellement
    /// dans la liste des manquants. Sans mémoire des tentatives, la pompe
    /// tournerait sans fin.
    #[test]
    fn backfill_does_not_loop_on_a_body_the_server_never_returns() {
        let (mut server, mut store, account) = synced(3);
        server.bodies.remove(&2); // l'enveloppe existe, le corps non

        let report = backfill_bodies(&mut server, &mut store, account, "INBOX", 0, 100).unwrap();

        assert_eq!(report.fetched, 2, "les deux corps servis");
        assert_eq!(report.remaining, 1, "le muet reste compté comme manquant");
    }

    /// L'horizon borne le coût : au-delà, rien n'est rapatrié.
    #[test]
    fn backfill_ignores_what_lies_beyond_the_horizon() {
        let (mut server, mut store, account) = synced(4);
        // FakeServer date les messages à 1_700_000_000 + uid.
        let horizon = 1_700_000_000 + 3;

        let report =
            backfill_bodies(&mut server, &mut store, account, "INBOX", horizon, 100).unwrap();

        assert_eq!(
            report.fetched, 2,
            "seuls les UID 3 et 4 sont dans l'horizon"
        );
        assert_eq!(report.remaining, 0);
    }

    #[test]
    fn backfill_on_a_never_synced_mailbox_does_nothing() {
        let mut server = FakeServer::new(false);
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("test@exemple.fr", "gmail")
            .unwrap();

        let report = backfill_bodies(&mut server, &mut store, account, "INBOX", 0, 100).unwrap();

        assert_eq!(report.fetched, 0);
        assert!(server.body_batches.is_empty());
    }

    /// Décor du rattrapage d'en-têtes : deux messages du même échange, mais
    /// le message intermédiaire — celui qu'on aurait envoyé — n'est pas
    /// dans la boîte. Rien ne les relie tant que `References` n'est pas lu.
    fn echange_coupe() -> (FakeServer, Store, i64) {
        let mut server = FakeServer::new(false);
        server.add(1, "Devis");
        server.add(3, "Re: Devis");
        server.set_references(3, "<fake-1@example.com> <fake-2@example.com>");

        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("test@exemple.fr", "gmail")
            .unwrap();
        crate::SyncEngine::default()
            .sync(&mut server, &mut store, account, "INBOX")
            .unwrap();
        (server, store, account)
    }

    fn conversations(store: &Store) -> usize {
        store.unified_recent(0, 50).unwrap().len()
    }

    /// La raison d'être de la passe, en une assertion : deux lignes avant,
    /// une seule après.
    #[test]
    fn les_entetes_rapatries_recollent_un_fil_coupe() {
        let (mut server, mut store, account) = echange_coupe();
        assert_eq!(conversations(&store), 2, "faute du chaînon manquant");

        let report =
            backfill_thread_headers(&mut server, &mut store, account, "INBOX", 0, 100).unwrap();

        assert_eq!(report.fetched, 2);
        assert_eq!(report.remaining, 0);
        assert_eq!(conversations(&store), 1, "l'échange est reconstitué");
    }

    /// Un message SANS `References` doit sortir définitivement de la liste
    /// des manquants. Sinon la passe le redemanderait à chaque
    /// synchronisation, pour toujours.
    #[test]
    fn un_message_sans_references_n_est_pas_redemande() {
        let (mut server, mut store, account) = echange_coupe();
        backfill_thread_headers(&mut server, &mut store, account, "INBOX", 0, 100).unwrap();

        let encore =
            backfill_thread_headers(&mut server, &mut store, account, "INBOX", 0, 100).unwrap();

        assert_eq!(encore.fetched, 0, "plus rien à lire");
        assert_eq!(server.header_batches.len(), 1, "aucun second aller-retour");
    }

    /// Groupée, comme les corps : un aller-retour par message rendrait la
    /// passe intenable sur une boîte remplie.
    #[test]
    fn la_passe_demande_les_entetes_par_lots() {
        let mut server = FakeServer::new(false);
        for uid in 1..=5 {
            server.add(uid, &format!("sujet {uid}"));
        }
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("test@exemple.fr", "gmail")
            .unwrap();
        crate::SyncEngine::default()
            .sync(&mut server, &mut store, account, "INBOX")
            .unwrap();

        backfill_thread_headers(&mut server, &mut store, account, "INBOX", 0, 100).unwrap();

        assert_eq!(server.header_batches, vec![vec![5, 4, 3, 2, 1]]);
    }

    /// Bornée : un budget épuisé laisse le reste pour le passage suivant,
    /// et le rapport le dit.
    #[test]
    fn le_budget_borne_un_passage_et_le_reste_est_annonce() {
        let mut server = FakeServer::new(false);
        for uid in 1..=5 {
            server.add(uid, &format!("sujet {uid}"));
        }
        let mut store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("test@exemple.fr", "gmail")
            .unwrap();
        crate::SyncEngine::default()
            .sync(&mut server, &mut store, account, "INBOX")
            .unwrap();

        let report =
            backfill_thread_headers(&mut server, &mut store, account, "INBOX", 0, 2).unwrap();

        assert_eq!(report.fetched, 2);
        assert_eq!(report.remaining, 3);
    }
}
