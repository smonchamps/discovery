//! Le moteur de synchronisation « enveloppes d'abord ».
//!
//! Protocole (décisions gelées, PHASE0.md §2) :
//! - synchro initiale du **plus récent au plus ancien**, par lots — la liste
//!   devient utilisable dès le premier lot ;
//! - synchro incrémentale : CONDSTORE quand le serveur l'expose (nouveaux
//!   messages + changements de flags), sinon différentiel d'UIDs pour les
//!   nouveaux ; les suppressions passent toujours par le différentiel ;
//! - changement d'UIDVALIDITY → resynchronisation complète.

use std::collections::HashSet;

use crate::action::Action;
use crate::envelope::Uid;
use crate::error::Error;
use crate::remote::MailServer;
use crate::store::{Store, SyncState};

const DEFAULT_BATCH_SIZE: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncMode {
    Initial,
    Incremental,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncReport {
    pub mode: SyncMode,
    /// Enveloppes récupérées ou mises à jour (nouveaux messages + flags).
    pub fetched: usize,
    /// Enveloppes locales supprimées car disparues du serveur.
    pub deleted: usize,
    /// Actions locales rejouées vers le serveur en tête de synchro.
    pub replayed: usize,
}

pub struct SyncEngine {
    batch_size: usize,
}

impl Default for SyncEngine {
    fn default() -> Self {
        Self {
            batch_size: DEFAULT_BATCH_SIZE,
        }
    }
}

impl SyncEngine {
    pub fn new(batch_size: usize) -> Self {
        Self {
            batch_size: batch_size.max(1),
        }
    }

    pub fn sync(
        &self,
        server: &mut dyn MailServer,
        store: &mut Store,
        account_id: i64,
        mailbox: &str,
    ) -> Result<SyncReport, Error> {
        let snapshot = server.select(mailbox)?;

        let state = match store.sync_state(account_id, mailbox)? {
            Some(state) if state.uid_validity == snapshot.uid_validity => state,
            Some(stale) => {
                store.reset_mailbox(stale.mailbox_id, snapshot.uid_validity)?;
                SyncState {
                    uid_validity: snapshot.uid_validity,
                    last_uid: 0,
                    highest_modseq: None,
                    ..stale
                }
            }
            None => {
                let mailbox_id =
                    store.create_mailbox(account_id, mailbox, snapshot.uid_validity)?;
                SyncState {
                    mailbox_id,
                    uid_validity: snapshot.uid_validity,
                    last_uid: 0,
                    highest_modseq: None,
                }
            }
        };

        // Ce que le serveur annonce, relevé à CHAQUE passage : c'est le
        // dénominateur de l'avancement (ADR 0010 §5). Relevé ici et non à
        // la création de la boîte, sinon il figerait la valeur du premier
        // jour et l'avancement dériverait à mesure que le courrier arrive.
        store.record_remote_total(state.mailbox_id, snapshot.exists)?;

        // Les intentions locales d'abord : la synchro qui suit reflète
        // ainsi leur effet (le rejeu bump le modseq côté serveur).
        let replayed = replay_actions(server, store, mailbox, state.mailbox_id)?;

        let mut report = if state.last_uid == 0 {
            self.initial_sync(server, store, mailbox, state.mailbox_id)?
        } else {
            self.incremental_sync(server, store, mailbox, &state, snapshot.exists)?
        };
        report.replayed = replayed;

        let last_uid = store.max_uid(state.mailbox_id)?;
        store.update_state(state.mailbox_id, last_uid, snapshot.highest_modseq)?;

        // La liste des dossiers n'est PLUS rafraîchie ici : chaque relève
        // payait un LIST identique — ~51 par compte et par cycle sur le
        // terrain du 2026-08-13 (ADR 0017). L'orchestrateur la rafraîchit
        // UNE fois par cycle, à l'inventaire, avec la liste qu'il a déjà
        // en main — déplacer hors ligne reste servi.
        Ok(report)
    }

    fn initial_sync(
        &self,
        server: &mut dyn MailServer,
        store: &mut Store,
        mailbox: &str,
        mailbox_id: i64,
    ) -> Result<SyncReport, Error> {
        let mut uids = server.list_uids(mailbox)?;
        uids.sort_unstable_by(|a, b| b.cmp(a));

        let mut fetched = 0;
        for chunk in uids.chunks(self.batch_size) {
            let envelopes = server.fetch_envelopes(mailbox, chunk)?;
            fetched += envelopes.len();
            store.upsert_envelopes(mailbox_id, &envelopes)?;
        }
        Ok(SyncReport {
            mode: SyncMode::Initial,
            fetched,
            deleted: 0,
            replayed: 0,
        })
    }

    fn incremental_sync(
        &self,
        server: &mut dyn MailServer,
        store: &mut Store,
        mailbox: &str,
        state: &SyncState,
        exists: u32,
    ) -> Result<SyncReport, Error> {
        let mut fetched = 0;
        let mut deleted = 0;

        let condstore_changes = match state.highest_modseq {
            Some(modseq) => server.changes_since(mailbox, modseq)?,
            None => None,
        };
        match condstore_changes {
            Some(changed) => {
                fetched += changed.len();
                store.upsert_envelopes(state.mailbox_id, &changed)?;
                // CONDSTORE ne signale pas les suppressions (il faudrait
                // QRESYNC, absent chez Gmail) : le différentiel d'UIDs
                // reste leur seule détection — mais il ne se paye QUE si
                // le décompte l'exige (E2b). Delta appliqué, base et
                // annonce d'accord : rien n'a disparu, et l'inventaire
                // complet (`UID SEARCH ALL`, 34 s sur l'INBOX du terrain)
                // n'aurait rien à dire.
                let local = store.envelope_count(state.mailbox_id)?;
                if local != u64::from(exists) {
                    let present: HashSet<Uid> = server.list_uids(mailbox)?.into_iter().collect();
                    deleted = store.remove_absent(state.mailbox_id, &present)?;
                }
            }
            None => {
                // Sans CONDSTORE : seuls les nouveaux messages sont détectés ;
                // les changements de flags attendront une resynchro complète.
                let server_uids = server.list_uids(mailbox)?;
                let mut new_uids: Vec<Uid> = server_uids
                    .iter()
                    .copied()
                    .filter(|uid| *uid > state.last_uid)
                    .collect();
                new_uids.sort_unstable_by(|a, b| b.cmp(a));
                for chunk in new_uids.chunks(self.batch_size) {
                    let envelopes = server.fetch_envelopes(mailbox, chunk)?;
                    fetched += envelopes.len();
                    store.upsert_envelopes(state.mailbox_id, &envelopes)?;
                }
                // Ici l'inventaire est déjà payé (il a servi aux nouveaux) :
                // le différentiel des suppressions est gratuit.
                let present: HashSet<Uid> = server_uids.into_iter().collect();
                deleted = store.remove_absent(state.mailbox_id, &present)?;
            }
        }

        Ok(SyncReport {
            mode: SyncMode::Incremental,
            fetched,
            deleted,
            replayed: 0,
        })
    }
}

/// Rejoue la file d'actions vers le serveur, dans l'ordre d'émission.
/// Au premier échec, le rejeu s'arrête et le reste de la file survit :
/// il sera retenté à la synchro suivante — aucune intention n'est perdue.
fn replay_actions(
    server: &mut dyn MailServer,
    store: &mut Store,
    mailbox: &str,
    mailbox_id: i64,
) -> Result<usize, Error> {
    let mut replayed = 0;
    for pending in store.pending_actions(mailbox_id)? {
        let outcome = match &pending.action {
            Action::MarkSeen => server.set_seen(mailbox, pending.uid, true),
            Action::MarkUnseen => server.set_seen(mailbox, pending.uid, false),
            Action::MarkFlagged => server.set_flagged(mailbox, pending.uid, true),
            Action::MarkUnflagged => server.set_flagged(mailbox, pending.uid, false),
            Action::Archive => server.archive(mailbox, pending.uid),
            Action::Delete => server.delete(mailbox, pending.uid),
            Action::MoveTo(target) => server.move_to(mailbox, pending.uid, target),
        };
        match outcome {
            Ok(()) => {
                store.remove_action(pending.id)?;
                replayed += 1;
            }
            Err(_) => break,
        }
    }
    Ok(replayed)
}

/// Dans quel ORDRE synchroniser les boîtes d'un compte — décision pure,
/// sans I/O, testable contre les bizarreries des serveurs réels.
///
/// Depuis l'[ADR 0010] on synchronise **tout**, sans exception : archive,
/// corbeille et spam compris. L'ordre n'est donc plus un détail, c'est ce
/// qui décide de ce que l'utilisateur voit en premier.
///
/// 1. **INBOX d'abord, toujours.** C'est la seule boîte que la liste
///    affiche : la faire passer après un dossier d'archive de 80 000
///    messages laisserait un écran vide pendant toute la première
///    synchronisation.
/// 2. **« Envoyés » ensuite**, parce que c'est lui qui complète les fils
///    ([ADR 0009]) — le reste n'est jamais regroupé.
/// 3. Le reste dans l'ordre du serveur.
///
/// Les dossiers non sélectionnables sont écartés : ce sont des conteneurs
/// sans courrier (`\Noselect`), et les SELECT échoueraient un par un.
///
/// [ADR 0009]: ../../../docs/adr/0009-portee-des-fils-au-compte.md
/// [ADR 0010]: ../../../docs/adr/0010-synchronisation-integrale.md
pub fn sync_order(folders: &[crate::remote::Folder], sent: Option<&str>) -> Vec<String> {
    let mut order: Vec<String> = Vec::with_capacity(folders.len() + 1);
    // INBOX même si le serveur ne la liste pas : elle existe toujours, et
    // une boîte de réception absente de la liste est une bizarrerie connue
    // des serveurs qui la traitent à part.
    order.push(crate::thread::RECEIVED_MAILBOX.to_string());
    if let Some(sent) = sent.filter(|sent| *sent != crate::thread::RECEIVED_MAILBOX) {
        order.push(sent.to_string());
    }
    for folder in folders {
        if folder.selectable && !order.iter().any(|deja| deja == &folder.wire) {
            order.push(folder.wire.clone());
        }
    }
    order
}

/// Coût disque estimé d'UN message, tout compris — enveloppe, index,
/// corps, pièces jointes.
///
/// Deux mesures du projet, pas un chiffre inventé ([ADR 0010] §4) :
/// ~49 ko par corps (137 Mo pour 2 801 messages, rattrapage complet de la
/// boîte réelle) + ~1,2 ko d'enveloppe et d'index (déduit de
/// `gate3-corps.db` : 778,9 Mo pour 200 000 enveloppes + 16 002 corps).
///
/// **Délibérément haute** : annoncer trop et tenir vaut mieux que
/// commencer et échouer à mi-chemin.
///
/// [ADR 0010]: ../../../docs/adr/0010-synchronisation-integrale.md
pub const SYNC_BYTES_PER_MESSAGE: u64 = 50 * 1024;

/// L'espace qui MANQUERAIT pour rapatrier `pending` messages — décision
/// pure, la garde de l'[ADR 0010] §4.
///
/// `None` : ça tient, on y va. `Some(octets)` : on REFUSE avant de
/// commencer, et le chiffre sert au message — « il manque 1,2 Go » se
/// comprend, « espace insuffisant » tout court laisse l'utilisateur
/// deviner s'il doit libérer 100 Mo ou 100 Go.
///
/// Pas de marge cachée en plus : l'estimation par message est déjà haute,
/// et deux marges empilées finissent par refuser des synchronisations qui
/// tiendraient.
///
/// [ADR 0010]: ../../../docs/adr/0010-synchronisation-integrale.md
pub fn disk_shortfall(pending: u64, available_bytes: u64) -> Option<u64> {
    let needed = pending.saturating_mul(SYNC_BYTES_PER_MESSAGE);
    if needed <= available_bytes {
        None
    } else {
        Some(needed - available_bytes)
    }
}

#[cfg(test)]
mod disk_shortfall_tests {
    use super::{SYNC_BYTES_PER_MESSAGE, disk_shortfall};

    /// Rien à rapatrier = rien à refuser, même sur un disque plein. Le
    /// cas COURANT : toutes les synchronisations incrémentales d'une boîte
    /// à jour passent par ici, et une garde qui les bloquerait sur un
    /// disque bien rempli interdirait de relever son courrier.
    #[test]
    fn rien_a_rapatrier_passe_meme_disque_plein() {
        assert_eq!(disk_shortfall(0, 0), None);
        assert_eq!(disk_shortfall(0, u64::MAX), None);
    }

    #[test]
    fn ca_tient_tout_juste() {
        assert_eq!(disk_shortfall(100, 100 * SYNC_BYTES_PER_MESSAGE), None);
    }

    /// Le manque est CHIFFRÉ : c'est lui qui rend le refus actionnable.
    #[test]
    fn le_manque_est_chiffre() {
        assert_eq!(
            disk_shortfall(100, 99 * SYNC_BYTES_PER_MESSAGE),
            Some(SYNC_BYTES_PER_MESSAGE)
        );
    }

    /// 200 000 messages × 50 ko = ~9,8 Go : le produit déborderait un
    /// u32, et une boîte encore plus grande ne doit pas faire paniquer la
    /// garde par un débordement — en debug, une multiplication nue sur
    /// u64 panique au lieu de boucler.
    #[test]
    fn une_boite_immense_ne_deborde_pas() {
        assert_eq!(disk_shortfall(u64::MAX, 0), Some(u64::MAX));
    }
}

/// L'avancement de la synchronisation, en pourcentage — décision pure.
///
/// `None` signifie **« je ne sais pas »**, et c'est un résultat à part
/// entière : tant qu'aucune boîte n'a été sélectionnée, on n'a pas de
/// dénominateur. Afficher « 0 % » dirait « je n'ai rien fait », et
/// « 100 % » dirait « j'ai fini » — deux mensonges. L'appelant n'affiche
/// alors rien.
///
/// Le résultat est plafonné à 100 : le local peut légitimement dépasser
/// l'annonce du serveur — des messages supprimés côté serveur entre deux
/// passages vivent encore en base jusqu'au différentiel suivant. Un
/// « 103 % » ferait douter de tout le reste de l'écran.
///
/// Et il ne rend jamais 100 tant qu'il reste quelque chose : l'arrondi
/// naturel afficherait « 100 % » à 19 999 messages sur 20 000, et
/// l'utilisateur verrait une barre pleine qui ne se termine pas.
/// Ce que le stockage sait d'un dossier au moment de décider (ADR 0017).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepereLocal {
    pub uid_validity: u32,
    /// Le UIDNEXT vu au relevé qui a précédé la DERNIÈRE relève soldée —
    /// pas `last_uid` : un serveur ne redescend jamais son UIDNEXT, alors
    /// que `last_uid` retombe quand le message le plus récent est
    /// supprimé, ce qui condamnerait le dossier à ne plus jamais être
    /// sauté.
    pub uidnext_vu: Option<u32>,
    /// Messages en base pour ce dossier.
    pub messages_locaux: u64,
    /// Des actions locales attendent leur rejeu : sauter les abandonnerait.
    pub actions_en_attente: bool,
    /// Le HIGHESTMODSEQ vu au SELECT de la dernière relève soldée
    /// (`sync_state`) — `None` sans CONDSTORE, ou tant qu'aucune relève
    /// n'a eu lieu depuis E2b.
    pub modseq_vu: Option<u64>,
}

/// Faut-il relever ce dossier, ou rien n'a bougé (ADR 0017) ?
///
/// La décision pure du « cycle sobre » — terrain du 2026-08-13 : le
/// cycle récurrent coûtait ~38 min sur une boîte réelle, chaque dossier
/// payant SELECT + UID SEARCH ALL même quand rien n'avait changé. Un
/// STATUS par dossier (déjà payé par la garde d'espace) suffit à
/// trancher. Toute incertitude — jamais relevé, valeurs tues par le
/// serveur, UIDVALIDITY changée, actions en attente — relève : la
/// sobriété n'a pas le droit de coûter un message.
pub fn faut_relever(distant: &crate::remote::FolderStatus, local: Option<&RepereLocal>) -> bool {
    let Some(local) = local else {
        return true;
    };
    if local.actions_en_attente {
        return true;
    }
    let (Some(uid_validity), Some(uid_next)) = (distant.uid_validity, distant.uid_next) else {
        return true;
    };
    if uid_validity != local.uid_validity {
        return true;
    }
    let Some(uidnext_vu) = local.uidnext_vu else {
        return true;
    };
    if uid_next != uidnext_vu {
        return true;
    }
    if u64::from(distant.messages) != local.messages_locaux {
        return true;
    }
    // E2b : un changement de drapeaux SEUL ne bouge ni UIDNEXT ni
    // MESSAGES — seul HIGHESTMODSEQ le trahit. Signal exigé des deux
    // côtés : un serveur muet (pas de CONDSTORE) garde le comportement
    // d'avant (ADR 0017 : rien n'est perdu qui était servi) ; un repère
    // local jamais posé (base d'avant E2b) relève UNE fois — le SELECT
    // de cette relève pose le modseq, et le dossier redevient sobre.
    match (distant.highest_modseq, local.modseq_vu) {
        (Some(distant), Some(vu)) => distant != vu,
        (Some(_), None) => true,
        (None, _) => false,
    }
}

#[cfg(test)]
mod faut_relever_tests {
    use super::{RepereLocal, faut_relever};
    use crate::remote::FolderStatus;

    fn distant() -> FolderStatus {
        FolderStatus {
            messages: 40,
            uid_next: Some(101),
            uid_validity: Some(7),
            highest_modseq: Some(900),
        }
    }
    fn local() -> RepereLocal {
        RepereLocal {
            uid_validity: 7,
            uidnext_vu: Some(101),
            messages_locaux: 40,
            actions_en_attente: false,
            modseq_vu: Some(900),
        }
    }

    /// LE cas qui rend le cycle sobre : rien n'a bougé, on saute.
    #[test]
    fn rien_n_a_bouge_on_saute() {
        assert!(!faut_relever(&distant(), Some(&local())));
    }

    /// Jamais relevé : aucune base de comparaison, on relève.
    #[test]
    fn un_dossier_jamais_releve_se_releve() {
        assert!(faut_relever(&distant(), None));
    }

    /// Une arrivée bouge UIDNEXT — même si un départ simultané laisse le
    /// décompte identique (le glissement qu'un seul des deux tests ne
    /// verrait pas).
    #[test]
    fn une_arrivee_bouge_uidnext_meme_a_decompte_egal() {
        let bouge = FolderStatus {
            uid_next: Some(102),
            ..distant()
        };
        assert!(faut_relever(&bouge, Some(&local())));
    }

    /// Une suppression baisse MESSAGES sans toucher UIDNEXT.
    #[test]
    fn une_suppression_baisse_le_decompte() {
        let ampute = FolderStatus {
            messages: 39,
            ..distant()
        };
        assert!(faut_relever(&ampute, Some(&local())));
    }

    /// UIDVALIDITY changée : les UID locaux ne veulent plus rien dire —
    /// la relève (et son reset) est obligatoire, invariant §6.6.
    #[test]
    fn uidvalidity_changee_force_la_releve() {
        let regenere = FolderStatus {
            uid_validity: Some(8),
            ..distant()
        };
        assert!(faut_relever(&regenere, Some(&local())));
    }

    /// Des actions locales attendent leur rejeu : sauter les abandonnerait
    /// jusqu'à un hypothétique changement distant.
    #[test]
    fn des_actions_en_attente_forcent_la_releve() {
        let charge = RepereLocal {
            actions_en_attente: true,
            ..local()
        };
        assert!(faut_relever(&distant(), Some(&charge)));
    }

    /// LE cas d'E2b : un mail lu au téléphone ne bouge ni UIDNEXT ni
    /// MESSAGES — seul HIGHESTMODSEQ glisse, et le dossier DOIT se
    /// relever pour refléter le drapeau.
    #[test]
    fn un_changement_de_drapeaux_seul_reveille_le_dossier() {
        let drapeaux = FolderStatus {
            highest_modseq: Some(901),
            ..distant()
        };
        assert!(faut_relever(&drapeaux, Some(&local())));
    }

    /// Un serveur SANS CONDSTORE tait HIGHESTMODSEQ : le comportement
    /// d'avant E2b est conservé — les drapeaux n'étaient déjà pas
    /// resynchronisés (ADR 0017 : rien n'est perdu qui était servi), et
    /// forcer la relève ruinerait la sobriété d'E2a pour rien.
    #[test]
    fn un_serveur_sans_condstore_garde_la_sobriete() {
        let muet = FolderStatus {
            highest_modseq: None,
            ..distant()
        };
        assert!(!faut_relever(&muet, Some(&local())));
    }

    /// Base d'avant E2b : le modseq local n'a jamais été posé alors que
    /// le serveur en annonce un — UNE relève de convergence, qui pose le
    /// repère, puis le dossier redevient sobre.
    #[test]
    fn un_modseq_jamais_vu_releve_une_fois_pour_converger() {
        let herite = RepereLocal {
            modseq_vu: None,
            ..local()
        };
        assert!(faut_relever(&distant(), Some(&herite)));
    }

    /// Un serveur qui tait UIDNEXT ou UIDVALIDITY rend la décision
    /// conservatrice — on relève, on ne devine pas.
    #[test]
    fn un_serveur_muet_impose_la_releve() {
        let muet = FolderStatus {
            uid_next: None,
            ..distant()
        };
        assert!(faut_relever(&muet, Some(&local())));
        let sans_validity = FolderStatus {
            uid_validity: None,
            ..distant()
        };
        assert!(faut_relever(&sans_validity, Some(&local())));
        let jamais_vu = RepereLocal {
            uidnext_vu: None,
            ..local()
        };
        assert!(faut_relever(&distant(), Some(&jamais_vu)));
    }
}

pub fn sync_percent(local: u64, remote: u64) -> Option<u8> {
    if remote == 0 {
        return None;
    }
    if local >= remote {
        return Some(100);
    }
    let percent = (local * 100 / remote) as u8;
    Some(percent.min(99))
}

#[cfg(test)]
mod sync_percent_tests {
    use super::sync_percent;

    /// Sans dénominateur, on ne raconte rien — surtout pas « 0 % », qui
    /// serait indiscernable d'une synchronisation qui n'avance pas.
    #[test]
    fn sans_denominateur_on_ne_dit_rien() {
        assert_eq!(sync_percent(0, 0), None);
        assert_eq!(sync_percent(42, 0), None);
    }

    #[test]
    fn le_cas_courant() {
        assert_eq!(sync_percent(0, 200), Some(0));
        assert_eq!(sync_percent(50, 200), Some(25));
        assert_eq!(sync_percent(200, 200), Some(100));
    }

    /// Le local dépasse l'annonce du serveur dès qu'un message y est
    /// supprimé entre deux passages : il vit encore en base jusqu'au
    /// différentiel suivant. « 103 % » ferait douter du reste de l'écran.
    #[test]
    fn le_local_qui_depasse_est_plafonne() {
        assert_eq!(sync_percent(210, 200), Some(100));
    }

    /// LE défaut d'affichage classique : une barre pleine qui continue de
    /// tourner. 19 999 sur 20 000 arrondit à 100 % — et l'utilisateur
    /// conclut que l'application est bloquée.
    #[test]
    fn presque_fini_n_est_pas_fini() {
        assert_eq!(sync_percent(19_999, 20_000), Some(99));
    }
}

#[cfg(test)]
mod sync_order_tests {
    use super::sync_order;
    use crate::remote::Folder;

    fn folder(wire: &str, selectable: bool) -> Folder {
        Folder {
            wire: wire.to_string(),
            display: wire.to_string(),
            selectable,
        }
    }

    /// Le cas qui compte : la liste ne montre qu'INBOX. Si un serveur
    /// annonce ses dossiers dans l'ordre alphabétique, « Archive » passe
    /// avant — et l'utilisateur regarde un écran vide pendant que 80 000
    /// messages d'archive descendent.
    #[test]
    fn inbox_passe_toujours_en_premier() {
        let dossiers = [
            folder("Archive", true),
            folder("INBOX", true),
            folder("Spam", true),
        ];
        assert_eq!(sync_order(&dossiers, None)[0], "INBOX");
    }

    /// « Envoyés » complète les fils (ADR 0009) : il passe avant les
    /// dossiers qui, eux, ne seront jamais regroupés.
    #[test]
    fn les_envoyes_passent_avant_le_reste() {
        let dossiers = [
            folder("Archive", true),
            folder("INBOX", true),
            folder("Sent", true),
        ];
        let ordre = sync_order(&dossiers, Some("Sent"));
        assert_eq!(ordre, vec!["INBOX", "Sent", "Archive"]);
    }

    /// Une boîte synchronisée deux fois n'est pas une erreur bénigne :
    /// c'est un aller-retour réseau complet payé pour rien, sur le chemin
    /// le plus long du produit.
    #[test]
    fn aucune_boite_n_est_synchronisee_deux_fois() {
        let dossiers = [
            folder("INBOX", true),
            folder("Sent", true),
            folder("Sent", true),
        ];
        let ordre = sync_order(&dossiers, Some("Sent"));
        assert_eq!(ordre.len(), 2, "ordre obtenu : {ordre:?}");
    }

    /// `\Noselect` : un conteneur qui ne porte pas de courrier. Le
    /// sélectionner échoue — autant ne pas le tenter.
    #[test]
    fn les_conteneurs_sans_courrier_sont_ecartes() {
        let dossiers = [folder("INBOX", true), folder("[Gmail]", false)];
        assert_eq!(sync_order(&dossiers, None), vec!["INBOX"]);
    }

    /// Gmail expose « [Gmail]/Messages envoyés » ET INBOX. Certains
    /// serveurs génériques, eux, ne listent RIEN — la boîte de réception
    /// doit rester synchronisée quand même.
    #[test]
    fn un_serveur_qui_ne_liste_rien_synchronise_quand_meme_la_reception() {
        assert_eq!(sync_order(&[], None), vec!["INBOX"]);
    }

    /// Un serveur qui désigne INBOX comme dossier d'envois (vu sur des
    /// configurations exotiques) ne doit pas la faire synchroniser deux
    /// fois.
    #[test]
    fn un_dossier_d_envois_confondu_avec_la_reception_ne_double_pas() {
        assert_eq!(sync_order(&[], Some("INBOX")), vec!["INBOX"]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::FakeServer;

    fn test_account(store: &Store) -> i64 {
        store
            .adopt_or_create_account("test@exemple.fr", "gmail")
            .unwrap()
    }

    fn synced(server: &mut FakeServer, store: &mut Store, engine: &SyncEngine) -> SyncReport {
        let account = test_account(store);
        engine.sync(server, store, account, "INBOX").unwrap()
    }

    fn recent(store: &Store, offset: usize, limit: usize) -> Vec<crate::Envelope> {
        let account = test_account(store);
        store.recent(account, "INBOX", offset, limit).unwrap()
    }

    #[test]
    fn initial_sync_fetches_newest_first_in_batches() {
        let mut server = FakeServer::new(false);
        for uid in 1..=5 {
            server.add(uid, "sujet");
        }
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::new(2);

        let report = synced(&mut server, &mut store, &engine);

        assert_eq!(report.mode, SyncMode::Initial);
        assert_eq!(report.fetched, 5);
        assert_eq!(
            server.fetch_batches,
            vec![vec![5, 4], vec![3, 2], vec![1]],
            "la synchro initiale doit servir le plus récent d'abord"
        );
    }

    #[test]
    fn initial_sync_of_empty_mailbox_fetches_nothing() {
        let mut server = FakeServer::new(false);
        let mut store = Store::open_in_memory().unwrap();

        let report = synced(&mut server, &mut store, &SyncEngine::default());

        assert_eq!(report.fetched, 0);
        assert!(server.fetch_batches.is_empty());
    }

    #[test]
    fn resync_without_changes_is_incremental_and_idempotent() {
        for condstore in [false, true] {
            let mut server = FakeServer::new(condstore);
            server.add(1, "a");
            let mut store = Store::open_in_memory().unwrap();
            let engine = SyncEngine::default();

            synced(&mut server, &mut store, &engine);
            let second = synced(&mut server, &mut store, &engine);

            assert_eq!(second.mode, SyncMode::Incremental);
            assert_eq!(second.fetched, 0, "condstore={condstore}");
            assert_eq!(second.deleted, 0);
        }
    }

    #[test]
    fn incremental_fetches_only_new_messages() {
        let mut server = FakeServer::new(false);
        server.add(1, "ancien");
        server.add(2, "ancien");
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::default();
        synced(&mut server, &mut store, &engine);

        server.add(3, "nouveau");
        server.add(4, "nouveau");
        let report = synced(&mut server, &mut store, &engine);

        assert_eq!(report.fetched, 2);
        assert_eq!(server.fetch_batches.last(), Some(&vec![4, 3]));
        assert_eq!(recent(&store, 0, 10).len(), 4);
    }

    #[test]
    fn incremental_removes_expunged_messages() {
        let mut server = FakeServer::new(false);
        for uid in 1..=3 {
            server.add(uid, "sujet");
        }
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::default();
        synced(&mut server, &mut store, &engine);

        server.expunge(2);
        let report = synced(&mut server, &mut store, &engine);

        assert_eq!(report.deleted, 1);
        let uids: Vec<Uid> = recent(&store, 0, 10).iter().map(|e| e.uid).collect();
        assert_eq!(uids, vec![3, 1]);
    }

    #[test]
    fn condstore_propagates_flag_changes() {
        let mut server = FakeServer::new(true);
        server.add(1, "à lire");
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::default();
        synced(&mut server, &mut store, &engine);

        server.mark_seen(1);
        let report = synced(&mut server, &mut store, &engine);

        assert_eq!(report.fetched, 1);
        assert!(recent(&store, 0, 1)[0].seen);
    }

    #[test]
    fn condstore_picks_up_new_messages_too() {
        let mut server = FakeServer::new(true);
        server.add(1, "ancien");
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::default();
        synced(&mut server, &mut store, &engine);

        server.add(2, "nouveau");
        let report = synced(&mut server, &mut store, &engine);

        assert_eq!(report.fetched, 1);
        assert_eq!(recent(&store, 0, 10).len(), 2);
    }

    /// La sobriété d'E2b : quand CONDSTORE porte le delta et que les
    /// décomptes concordent, l'inventaire complet des UIDs — le
    /// `UID SEARCH ALL` à 34 s de l'INBOX du terrain — ne se paye PAS.
    /// Il ne se paye que quand une suppression le rend nécessaire.
    #[test]
    fn condstore_ne_paye_l_inventaire_que_si_le_decompte_l_exige() {
        let mut server = FakeServer::new(true);
        server.add(1, "a");
        server.add(2, "b");
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::default();
        synced(&mut server, &mut store, &engine);
        let apres_initiale = server.uid_list_calls;

        // Drapeau seul : delta CONDSTORE, décomptes égaux — zéro inventaire.
        server.mark_seen(1);
        let report = synced(&mut server, &mut store, &engine);
        assert_eq!(report.fetched, 1);
        assert!(recent(&store, 0, 1)[0].seen || recent(&store, 0, 2)[1].seen);
        assert_eq!(
            server.uid_list_calls, apres_initiale,
            "un drapeau ne justifie pas d'inventaire complet"
        );

        // Suppression : le décompte diverge, l'inventaire redevient dû.
        server.expunge(2);
        let report = synced(&mut server, &mut store, &engine);
        assert_eq!(report.deleted, 1);
        assert_eq!(
            server.uid_list_calls,
            apres_initiale + 1,
            "une suppression exige le différentiel d'UIDs"
        );
    }

    /// Limite connue et assumée : sans CONDSTORE, un flag changé côté serveur
    /// n'est pas rafraîchi par la synchro incrémentale. Ce test documente le
    /// comportement pour qu'une future correction soit un choix, pas un hasard.
    #[test]
    fn without_condstore_flag_changes_are_not_detected() {
        let mut server = FakeServer::new(false);
        server.add(1, "à lire");
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::default();
        synced(&mut server, &mut store, &engine);

        server.mark_seen(1);
        synced(&mut server, &mut store, &engine);

        assert!(!recent(&store, 0, 1)[0].seen);
    }

    fn mailbox_id(store: &Store) -> i64 {
        store
            .sync_state(test_account(store), "INBOX")
            .unwrap()
            .unwrap()
            .mailbox_id
    }

    #[test]
    fn replay_pushes_queued_actions_to_server_in_order() {
        let mut server = FakeServer::new(false);
        server.add(1, "a");
        server.add(2, "b");
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::default();
        synced(&mut server, &mut store, &engine);

        let id = mailbox_id(&store);
        store.enqueue_action(id, 1, Action::MarkSeen).unwrap();
        store.enqueue_action(id, 2, Action::MarkSeen).unwrap();
        store.enqueue_action(id, 1, Action::MarkUnseen).unwrap();

        let report = synced(&mut server, &mut store, &engine);

        assert_eq!(report.replayed, 3);
        assert_eq!(
            server.action_calls,
            vec!["seen:1:true", "seen:2:true", "seen:1:false"],
            "le rejeu doit préserver l'ordre d'émission"
        );
        assert!(store.pending_actions(id).unwrap().is_empty());
    }

    /// Depuis l'ADR 0017, la relève d'un dossier ne rafraîchit PLUS la
    /// liste des dossiers : ce LIST était payé à CHAQUE dossier (~51 par
    /// cycle au terrain du 2026-08-13). C'est l'orchestrateur qui la met
    /// en cache, UNE fois par cycle, à l'inventaire — l'offline-first du
    /// déplacement est tenu là. Ce test tient le nouveau contrat : si le
    /// moteur se remet à lister, la facture réseau revient en silence.
    #[test]
    fn syncing_does_not_refetch_the_folder_list() {
        let mut server = FakeServer::new(false);
        server.add(1, "a");
        server.folders = vec![crate::remote::Folder {
            wire: "Archiv&AOk-s".to_string(),
            display: "Archivés".to_string(),
            selectable: true,
        }];
        let mut store = Store::open_in_memory().unwrap();
        synced(&mut server, &mut store, &SyncEngine::default());

        // La relève n'a rien mis en cache : la liste appartient à
        // l'inventaire du cycle, pas au moteur.
        let cached = store.folders(test_account(&store)).unwrap();
        assert!(cached.is_empty());
    }

    /// Le déplacement suit la même boucle hors-ligne que le reste :
    /// journalisé au clic, rejoué à la synchro suivante. Le nom RÉSEAU du
    /// dossier doit ressortir intact — une action peut être rejouée des
    /// jours plus tard, sur un dossier accentué.
    #[test]
    fn replay_moves_the_message_to_its_journaled_folder() {
        let mut server = FakeServer::new(false);
        server.add(1, "a");
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::default();
        synced(&mut server, &mut store, &engine);

        let id = mailbox_id(&store);
        store
            .enqueue_action(id, 1, Action::MoveTo("Archiv&AOk-s".to_string()))
            .unwrap();

        let report = synced(&mut server, &mut store, &engine);

        assert_eq!(report.replayed, 1);
        assert_eq!(
            server.moved,
            vec![(1, "Archiv&AOk-s".to_string())],
            "le nom réseau doit arriver intact au serveur"
        );
        assert!(store.pending_actions(id).unwrap().is_empty());
    }

    /// Une coupure pendant le rejeu ne doit rien perdre : l'action
    /// reste en file pour la synchro suivante. Même garantie que pour
    /// les autres actions — le déplacement n'y fait pas exception.
    #[test]
    fn a_failed_move_stays_queued() {
        let mut server = FakeServer::new(false);
        server.add(1, "a");
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::default();
        synced(&mut server, &mut store, &engine);

        let id = mailbox_id(&store);
        store
            .enqueue_action(id, 1, Action::MoveTo("Factures".to_string()))
            .unwrap();
        server.actions_fail = true;

        let report = synced(&mut server, &mut store, &engine);

        assert_eq!(report.replayed, 0);
        assert!(server.moved.is_empty());
        assert_eq!(
            store.pending_actions(id).unwrap().len(),
            1,
            "l'intention doit survivre à la coupure"
        );
    }

    #[test]
    fn replay_stars_and_unstars_on_server() {
        let mut server = FakeServer::new(false);
        server.add(1, "à étoiler");
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::default();
        synced(&mut server, &mut store, &engine);

        let id = mailbox_id(&store);
        store.enqueue_action(id, 1, Action::MarkFlagged).unwrap();
        store.enqueue_action(id, 1, Action::MarkUnflagged).unwrap();

        let report = synced(&mut server, &mut store, &engine);

        assert_eq!(report.replayed, 2);
        assert_eq!(server.action_calls, vec!["flag:1:true", "flag:1:false"]);
        assert!(!server.messages[&1].0.flagged);
    }

    #[test]
    fn condstore_propagates_star_changes() {
        let mut server = FakeServer::new(true);
        server.add(1, "étoilé ailleurs");
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::default();
        synced(&mut server, &mut store, &engine);

        server.mark_flagged(1);
        let report = synced(&mut server, &mut store, &engine);

        assert_eq!(report.fetched, 1);
        assert!(recent(&store, 0, 1)[0].flagged);
    }

    #[test]
    fn replay_archives_and_deletes_on_server() {
        let mut server = FakeServer::new(false);
        server.add(1, "à archiver");
        server.add(2, "à supprimer");
        server.add(3, "à garder");
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::default();
        synced(&mut server, &mut store, &engine);

        let id = mailbox_id(&store);
        store.remove_local(id, 1).unwrap();
        store.remove_local(id, 2).unwrap();
        store.enqueue_action(id, 1, Action::Archive).unwrap();
        store.enqueue_action(id, 2, Action::Delete).unwrap();

        let report = synced(&mut server, &mut store, &engine);

        assert_eq!(report.replayed, 2);
        assert_eq!(server.action_calls, vec!["archive:1", "delete:2"]);
        assert!(!server.messages.contains_key(&1));
        assert!(!server.messages.contains_key(&2));
        let uids: Vec<Uid> = recent(&store, 0, 10).iter().map(|e| e.uid).collect();
        assert_eq!(uids, vec![3], "seul le message gardé reste localement");
    }

    /// Le gate de la Phase 2 : une coupure pendant le rejeu ne perd rien —
    /// la file survit et repart à la synchro suivante.
    #[test]
    fn failed_replay_keeps_actions_queued_for_next_sync() {
        let mut server = FakeServer::new(false);
        server.add(1, "a");
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::default();
        synced(&mut server, &mut store, &engine);

        let id = mailbox_id(&store);
        store.enqueue_action(id, 1, Action::MarkSeen).unwrap();

        server.actions_fail = true;
        let cut = synced(&mut server, &mut store, &engine);
        assert_eq!(cut.replayed, 0);
        assert_eq!(store.pending_actions(id).unwrap().len(), 1);

        server.actions_fail = false;
        let recovered = synced(&mut server, &mut store, &engine);
        assert_eq!(recovered.replayed, 1);
        assert!(store.pending_actions(id).unwrap().is_empty());
        assert!(server.messages[&1].0.seen);
    }

    #[test]
    fn uid_validity_reset_drops_now_meaningless_actions() {
        let mut server = FakeServer::new(false);
        server.add(1, "a");
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::default();
        synced(&mut server, &mut store, &engine);

        let id = mailbox_id(&store);
        store.enqueue_action(id, 1, Action::MarkSeen).unwrap();
        server.bump_uid_validity();

        let report = synced(&mut server, &mut store, &engine);

        assert_eq!(report.replayed, 0);
        assert!(server.action_calls.is_empty());
        assert!(store.pending_actions(id).unwrap().is_empty());
    }

    #[test]
    fn uid_validity_change_triggers_full_resync() {
        let mut server = FakeServer::new(false);
        server.add(1, "avant");
        server.add(2, "avant");
        let mut store = Store::open_in_memory().unwrap();
        let engine = SyncEngine::default();
        synced(&mut server, &mut store, &engine);

        server.bump_uid_validity();
        server.messages.clear();
        server.add(10, "après");
        let report = synced(&mut server, &mut store, &engine);

        assert_eq!(report.mode, SyncMode::Initial);
        let rows = recent(&store, 0, 10);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].uid, 10);
        assert_eq!(rows[0].subject.as_deref(), Some("après"));
    }
}
