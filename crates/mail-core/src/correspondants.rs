//! L'annuaire des correspondants (PLAN-RETOURS-5, décision D4) : les
//! adresses que le courrier a montrées — expéditeurs vus (avec leur nom
//! d'affichage), destinataires de NOS envois — au service de
//! l'autocomplétion des champs À/Cc/Cci. Jamais un carnet édité.
//!
//! Trois règles :
//! - **une table PETITE, interrogée à la frappe** : un correspondant =
//!   une ligne — jamais un parcours d'`envelopes` par frappe dans la
//!   file sérialisée (la leçon de PLAN-DEFILEMENT-PROFOND) ;
//! - **rien depuis indésirables ni corbeille** (D4) : un spammeur ne
//!   devient pas une suggestion ;
//! - **le nom le plus récent gagne**, une adresse n'apparaît qu'une
//!   fois (dédoublonnage par adresse en minuscules).
//!
//! L'annuaire s'alimente au fil de l'eau (synchro : messages NEUFS
//! seulement — une re-synchronisation ne gonfle pas la fréquence ;
//! envoi : l'adresse qu'on écrit est une adresse connue) et se rattrape
//! UNE fois sur l'existant à l'ouverture (marque en `prefs`, passe
//! set-based).

use rusqlite::{Connection, OptionalExtension, params};

use crate::error::Error;
use crate::store::Store;

/// Une suggestion d'adresse : l'adresse (minuscules) et le dernier nom
/// d'affichage connu, s'il existe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Correspondant {
    pub address: String,
    pub name: Option<String>,
}

const MOIS: i64 = 30 * 24 * 3600;
const AN: i64 = 365 * 24 * 3600;

/// Le classement (décision pure, testable) : récence ET fréquence — un
/// correspondant récent pèse plus lourd à fréquence égale, un
/// correspondant fréquent plus lourd à récence égale. Paliers plutôt
/// qu'une décroissance continue : dérivable de tête, stable au test.
pub(crate) fn score(hits: i64, last_epoch: i64, now: i64) -> i64 {
    let age = now.saturating_sub(last_epoch);
    let poids = if age <= MOIS {
        4
    } else if age <= AN {
        2
    } else {
        1
    };
    hits.max(1) * poids
}

/// Échappe `%`, `_` et `\` d'un préfixe utilisateur pour un motif LIKE
/// (clause `ESCAPE '\'`).
fn echapper_like(brut: &str) -> String {
    let mut sortie = String::with_capacity(brut.len());
    for c in brut.chars() {
        if matches!(c, '%' | '_' | '\\') {
            sortie.push('\\');
        }
        sortie.push(c);
    }
    sortie
}

/// Note une adresse à l'annuaire : création ou mise à jour (fréquence
/// +1, récence au plus tard, le nom le plus récent gagne — un nom vide
/// ne remplace jamais un nom connu).
pub(crate) fn noter(
    conn: &Connection,
    address: &str,
    name: Option<&str>,
    epoch: i64,
) -> Result<(), Error> {
    let address = address.trim().to_lowercase();
    if address.is_empty() {
        return Ok(());
    }
    let name = name.map(str::trim).filter(|nom| !nom.is_empty());
    conn.prepare_cached(
        "INSERT INTO correspondants (address, name, last_epoch, hits)
         VALUES (?1, ?2, ?3, 1)
         ON CONFLICT(address) DO UPDATE SET
             name = CASE WHEN excluded.name IS NOT NULL
                          AND excluded.last_epoch >= last_epoch
                         THEN excluded.name
                         ELSE COALESCE(name, excluded.name) END,
             last_epoch = MAX(last_epoch, excluded.last_epoch),
             hits = hits + 1",
    )?
    .execute(params![address, name, epoch])?;
    Ok(())
}

impl Store {
    /// Ce que l'annuaire apprend d'une boîte : `(expéditeurs,
    /// destinataires)`. Indésirables et corbeille n'apprennent RIEN
    /// (D4) ; le dossier d'envois apprend AUSSI les destinataires.
    pub(crate) fn role_annuaire(&self, mailbox_id: i64) -> Result<(bool, bool), Error> {
        let (account_id, nom): (i64, String) = self.conn().query_row(
            "SELECT account_id, name FROM mailboxes WHERE id = ?1",
            [mailbox_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let dossiers = self.canonical_folders(account_id)?;
        if dossiers.boite("indesirables").as_deref() == Some(nom.as_str())
            || dossiers.boite("corbeille").as_deref() == Some(nom.as_str())
        {
            return Ok((false, false));
        }
        let envois = dossiers.boite("envoyes").as_deref() == Some(nom.as_str());
        Ok((true, envois))
    }

    /// Les suggestions pour un préfixe : appariement sur le DÉBUT de
    /// l'adresse, du nom, ou d'un mot du nom (LIKE — insensible à la
    /// casse ASCII, limite assumée sur les initiales accentuées) ;
    /// classement récence + fréquence ([`score`]), départage à la
    /// récence. Le filet `LIMIT 512` borne le tri Rust — au-delà, le
    /// préfixe est trop court pour qu'un rang parfait importe.
    pub fn completer_adresses(
        &self,
        prefixe: &str,
        limite: usize,
    ) -> Result<Vec<Correspondant>, Error> {
        let prefixe = prefixe.trim().to_lowercase();
        if prefixe.is_empty() || limite == 0 {
            return Ok(Vec::new());
        }
        let motif = format!("{}%", echapper_like(&prefixe));
        let mot = format!("% {motif}");
        let mut rows: Vec<(String, Option<String>, i64, i64)> = self
            .conn()
            .prepare_cached(
                "SELECT address, name, hits, last_epoch FROM correspondants
                 WHERE address LIKE ?1 ESCAPE '\\'
                    OR name LIKE ?1 ESCAPE '\\'
                    OR name LIKE ?2 ESCAPE '\\'
                 ORDER BY last_epoch DESC
                 LIMIT 512",
            )?
            .query_map(params![motif, mot], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?
            .collect::<Result<_, _>>()?;
        let now = chrono::Utc::now().timestamp();
        rows.sort_by_key(|(_, _, hits, last)| (-score(*hits, *last, now), -*last));
        Ok(rows
            .into_iter()
            .take(limite)
            .map(|(address, name, _, _)| Correspondant { address, name })
            .collect())
    }

    /// Le rattrapage UNIQUE de l'existant : peuple l'annuaire depuis les
    /// enveloppes déjà en base (set-based, le nom du message le plus
    /// récent gagne — bare column du MAX, comportement documenté de
    /// SQLite), puis pose la marque `prefs`. Idempotent : la marque
    /// re-vérifiée sous le verrou d'écriture — deux connexions
    /// concurrentes ne comptent jamais double.
    pub(crate) fn rattraper_correspondants(&self) -> Result<(), Error> {
        const MARQUE: &str = "annuaire_correspondants_v1";
        let fait: Option<String> = self
            .conn()
            .query_row(
                "SELECT value FROM prefs WHERE key = ?1",
                params![MARQUE],
                |row| row.get(0),
            )
            .optional()?;
        if fait.is_some() {
            return Ok(());
        }
        self.conn().execute_batch("BEGIN IMMEDIATE")?;
        let passe = (|| -> Result<(), Error> {
            let refait: Option<String> = self
                .conn()
                .query_row(
                    "SELECT value FROM prefs WHERE key = ?1",
                    params![MARQUE],
                    |row| row.get(0),
                )
                .optional()?;
            if refait.is_some() {
                return Ok(());
            }
            let comptes: Vec<i64> = self
                .conn()
                .prepare("SELECT id FROM accounts")?
                .query_map([], |row| row.get(0))?
                .collect::<Result<_, _>>()?;
            for account in comptes {
                let dossiers = self.canonical_folders(account)?;
                let mut exclus: Vec<i64> = Vec::new();
                for categorie in ["indesirables", "corbeille"] {
                    if let Some(nom) = dossiers.boite(categorie)
                        && let Some(state) = self.sync_state(account, &nom)?
                    {
                        exclus.push(state.mailbox_id);
                    }
                }
                let clause = if exclus.is_empty() {
                    String::new()
                } else {
                    let liste = exclus
                        .iter()
                        .map(i64::to_string)
                        .collect::<Vec<_>>()
                        .join(",");
                    format!(" AND e.mailbox_id NOT IN ({liste})")
                };
                self.conn().execute(
                    &format!(
                        "INSERT INTO correspondants (address, name, last_epoch, hits)
                         SELECT lower(e.sender_address), e.sender,
                                MAX(COALESCE(e.date_epoch, 0)), COUNT(*)
                         FROM envelopes e
                         JOIN mailboxes m ON m.id = e.mailbox_id
                         WHERE m.account_id = ?1
                           AND e.sender_address IS NOT NULL
                           AND e.sender_address <> ''{clause}
                         GROUP BY lower(e.sender_address)
                         ON CONFLICT(address) DO UPDATE SET
                             name = CASE WHEN excluded.last_epoch >= last_epoch
                                          AND excluded.name IS NOT NULL
                                         THEN excluded.name
                                         ELSE COALESCE(name, excluded.name) END,
                             last_epoch = MAX(last_epoch, excluded.last_epoch),
                             hits = hits + excluded.hits"
                    ),
                    params![account],
                )?;
                // Les destinataires de NOS envois : les listes sont
                // jointes par '\n' — découpe en Rust, le dossier
                // d'envois est petit devant le corpus.
                if let Some(nom) = dossiers.boite("envoyes")
                    && let Some(state) = self.sync_state(account, &nom)?
                {
                    let envois: Vec<(Option<String>, Option<String>, Option<i64>)> = self
                        .conn()
                        .prepare(
                            "SELECT to_addrs, cc_addrs, date_epoch
                             FROM envelopes WHERE mailbox_id = ?1",
                        )?
                        .query_map([state.mailbox_id], |row| {
                            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                        })?
                        .collect::<Result<_, _>>()?;
                    for (to, cc, date) in envois {
                        for liste in [to, cc].into_iter().flatten() {
                            for adresse in liste.split('\n').filter(|a| !a.is_empty()) {
                                noter(self.conn(), adresse, None, date.unwrap_or(0))?;
                            }
                        }
                    }
                }
            }
            self.conn().execute(
                "INSERT INTO prefs (key, value) VALUES (?1, '1')
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![MARQUE],
            )?;
            Ok(())
        })();
        match passe {
            Ok(()) => {
                self.conn().execute_batch("COMMIT")?;
                Ok(())
            }
            Err(err) => {
                let _ = self.conn().execute_batch("ROLLBACK");
                Err(err)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;
    use crate::envelope::Envelope;

    fn envelope(uid: u32, sujet: &str, nom: &str, adresse: &str, epoch: i64) -> Envelope {
        Envelope {
            uid,
            subject: Some(sujet.to_string()),
            sender: Some(nom.to_string()),
            sender_address: Some(adresse.to_string()),
            message_id: Some(format!("<m{uid}@exemple.fr>")),
            in_reply_to: None,
            date: Some(Utc.timestamp_opt(epoch, 0).unwrap()),
            seen: true,
            flagged: false,
            to_addrs: Vec::new(),
            cc_addrs: Vec::new(),
        }
    }

    fn decor() -> (Store, i64, i64, i64, i64) {
        let store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("moi@exemple.fr", "gmail")
            .unwrap();
        store.set_thread_scope(account, Some("Envoyés")).unwrap();
        store
            .replace_folders(
                account,
                &["INBOX", "Envoyés", "Spam", "Corbeille"]
                    .iter()
                    .map(|nom| crate::Folder {
                        wire: nom.to_string(),
                        display: nom.to_string(),
                        selectable: true,
                    })
                    .collect::<Vec<_>>(),
            )
            .unwrap();
        let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        let envoyes = store.create_mailbox(account, "Envoyés", 1).unwrap();
        let spam = store.create_mailbox(account, "Spam", 1).unwrap();
        (store, account, inbox, envoyes, spam)
    }

    /// Récence + fréquence : le récent pèse plus lourd à fréquence
    /// égale, le fréquent plus lourd à récence égale — et un
    /// correspondant récent bat un fréquent d'il y a des années.
    #[test]
    fn le_score_pese_recence_et_frequence() {
        let now = 10 * AN;
        // Fréquence égale : le récent gagne.
        assert!(score(10, now - 3600, now) > score(10, now - 2 * AN, now));
        // Récence égale : le fréquent gagne.
        assert!(score(20, now - 3600, now) > score(3, now - 3600, now));
        // 2 messages ce mois-ci battent 7 d'il y a des années.
        assert!(score(2, now - 3600, now) > score(7, now - 2 * AN, now));
    }

    /// La synchro apprend les expéditeurs — messages NEUFS seulement :
    /// une re-synchronisation du même lot ne gonfle pas la fréquence.
    #[test]
    fn la_synchro_apprend_les_expediteurs_sans_double_compte() {
        let (mut store, _account, inbox, _, _) = decor();
        let lot = [envelope(1, "un", "Alice Martin", "Alice@Exemple.fr", 100)];
        store.upsert_envelopes(inbox, &lot).unwrap();
        store.upsert_envelopes(inbox, &lot).unwrap();

        let (hits, name): (i64, Option<String>) = store
            .conn()
            .query_row(
                "SELECT hits, name FROM correspondants WHERE address = 'alice@exemple.fr'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(hits, 1, "re-synchronisé, pas re-compté");
        assert_eq!(name.as_deref(), Some("Alice Martin"));
    }

    /// D4 : indésirables et corbeille n'apprennent rien.
    #[test]
    fn rien_depuis_les_indesirables() {
        let (mut store, _account, _, _, spam) = decor();
        store
            .upsert_envelopes(
                spam,
                &[envelope(1, "pub", "Spammeur", "spam@nuisible.fr", 100)],
            )
            .unwrap();
        let total: i64 = store
            .conn()
            .query_row("SELECT COUNT(*) FROM correspondants", [], |row| row.get(0))
            .unwrap();
        assert_eq!(total, 0);
    }

    /// D4 : le dossier d'envois apprend AUSSI les destinataires (adresse
    /// nue — leur nom viendra d'un message reçu, s'il vient).
    #[test]
    fn les_envois_apprennent_les_destinataires() {
        let (mut store, _account, _, envoyes, _) = decor();
        let mut env = envelope(1, "notre envoi", "Moi", "moi@exemple.fr", 100);
        env.to_addrs = vec!["Camille.Rousseau@atelier.fr".to_string()];
        env.cc_addrs = vec!["s.nardi@atelier.fr".to_string()];
        store.upsert_envelopes(envoyes, &[env]).unwrap();

        let suggeres = store.completer_adresses("camille", 8).unwrap();
        assert_eq!(suggeres.len(), 1);
        assert_eq!(suggeres[0].address, "camille.rousseau@atelier.fr");
        assert!(store.completer_adresses("s.nardi", 8).unwrap().len() == 1);
    }

    /// Revue : les destinataires RATTRAPÉS d'un vieil envoi
    /// (`set_recipients`, pompe PLAN-RETOURS-MAIL) entrent aussi dans
    /// l'annuaire — le rattrapage d'ouverture est passé avant eux.
    #[test]
    fn les_destinataires_rattrapes_entrent_a_l_annuaire() {
        let (mut store, _account, _, envoyes, _) = decor();
        // Un envoi d'avant le stockage des À/Cc : pas de destinataires.
        store
            .upsert_envelopes(
                envoyes,
                &[envelope(1, "vieil envoi", "Moi", "moi@exemple.fr", 100)],
            )
            .unwrap();
        assert!(store.completer_adresses("vieux", 8).unwrap().is_empty());

        store
            .set_recipients(envoyes, 1, &["vieux@dest.fr".to_string()], &[])
            .unwrap();

        assert_eq!(store.completer_adresses("vieux", 8).unwrap().len(), 1);
    }

    /// L'adresse qu'on écrit est une adresse connue : l'envoi note ses
    /// destinataires dès la mise en file.
    #[test]
    fn l_envoi_note_ses_destinataires() {
        let store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("moi@exemple.fr", "gmail")
            .unwrap();
        let draft = crate::compose(
            "moi@exemple.fr",
            "nouveau@contact.fr",
            "",
            "copie@contact.fr",
            "objet",
            "corps",
            None,
        )
        .unwrap();
        store.enqueue_outbox(account, &draft).unwrap();

        assert_eq!(store.completer_adresses("nouveau", 8).unwrap().len(), 1);
        assert_eq!(store.completer_adresses("copie", 8).unwrap().len(), 1);
    }

    /// L'appariement : début d'adresse, début de nom, début d'un MOT du
    /// nom — et le préfixe est insensible à la casse.
    #[test]
    fn l_appariement_couvre_adresse_nom_et_mots_du_nom() {
        let (mut store, _account, inbox, _, _) = decor();
        store
            .upsert_envelopes(
                inbox,
                &[envelope(
                    1,
                    "x",
                    "Camille Rousseau",
                    "c.rousseau@atelier.fr",
                    100,
                )],
            )
            .unwrap();

        for prefixe in ["c.rous", "camille", "rousseau", "ROUSSEAU"] {
            let trouves = store.completer_adresses(prefixe, 8).unwrap();
            assert_eq!(trouves.len(), 1, "préfixe {prefixe:?}");
            assert_eq!(trouves[0].name.as_deref(), Some("Camille Rousseau"));
        }
        assert!(store.completer_adresses("ousseau", 8).unwrap().is_empty());
        // Les métacaractères LIKE sont des LITTÉRAUX de préfixe.
        assert!(store.completer_adresses("%", 8).unwrap().is_empty());
    }

    /// Une adresse = une ligne, le nom le plus récent gagne, et le
    /// classement sert le récent-fréquent d'abord.
    #[test]
    fn dedoublonnage_nom_recent_et_classement() {
        let (mut store, _account, inbox, _, _) = decor();
        let now = Utc::now().timestamp();
        store
            .upsert_envelopes(
                inbox,
                &[
                    envelope(
                        1,
                        "vieux",
                        "C. Rousseau",
                        "c.rousseau@atelier.fr",
                        now - 3 * AN,
                    ),
                    envelope(
                        2,
                        "recent",
                        "Camille Rousseau",
                        "c.rousseau@atelier.fr",
                        now - 60,
                    ),
                    envelope(
                        3,
                        "autre",
                        "Casimir Vieux",
                        "casimir@archives.fr",
                        now - 2 * AN,
                    ),
                ],
            )
            .unwrap();

        let trouves = store.completer_adresses("c", 8).unwrap();
        assert_eq!(trouves.len(), 2, "{trouves:?}");
        assert_eq!(trouves[0].address, "c.rousseau@atelier.fr");
        assert_eq!(
            trouves[0].name.as_deref(),
            Some("Camille Rousseau"),
            "le nom le plus récent gagne"
        );
        assert_eq!(trouves[1].address, "casimir@archives.fr");
        // La limite est respectée.
        assert_eq!(store.completer_adresses("c", 1).unwrap().len(), 1);
    }

    /// Le banc du budget de frappe (PLAN-RETOURS-5, gate E4) : 50 000
    /// correspondants — plus que d'expéditeurs uniques sur la vraie base
    /// de 256 k messages —, préfixe d'UNE lettre (le pire cas : le plus
    /// de correspondances). `cargo test --release -- --ignored
    /// banc_completer --nocapture` ; budget < 50 ms.
    #[test]
    #[ignore]
    fn banc_completer_50k() {
        let store = Store::open_in_memory().unwrap();
        {
            let conn = store.conn();
            conn.execute_batch("BEGIN").unwrap();
            let mut stmt = conn
                .prepare(
                    "INSERT INTO correspondants (address, name, last_epoch, hits)
                     VALUES (?1, ?2, ?3, ?4)",
                )
                .unwrap();
            for n in 0..50_000i64 {
                stmt.execute(params![
                    format!("contact{n}@domaine{}.fr", n % 977),
                    format!("Contact Num{n}"),
                    n * 60,
                    (n % 40) + 1
                ])
                .unwrap();
            }
            conn.execute_batch("COMMIT").unwrap();
        }
        let debut = std::time::Instant::now();
        let trouves = store.completer_adresses("c", 8).unwrap();
        let duree = debut.elapsed();
        println!("completer_adresses('c') sur 50 000 : {duree:?}");
        assert_eq!(trouves.len(), 8);
    }

    /// Le banc de la passe de rattrapage (une fois, au premier lancement
    /// sur une base existante) : 200 000 enveloppes, ~20 000 expéditeurs
    /// uniques. `cargo test --release banc_rattrapage -- --ignored
    /// --nocapture`.
    #[test]
    #[ignore]
    fn banc_rattrapage_200k() {
        let (store, _account, inbox, _, _) = decor();
        {
            let conn = store.conn();
            conn.execute_batch("BEGIN").unwrap();
            let mut stmt = conn
                .prepare(
                    "INSERT INTO envelopes (mailbox_id, uid, subject, sender,
                        sender_address, message_id, date_epoch, seen, flagged)
                     VALUES (?1, ?2, 'sujet', ?3, ?4, ?5, ?6, 1, 0)",
                )
                .unwrap();
            for n in 0..200_000i64 {
                stmt.execute(params![
                    inbox,
                    n + 1,
                    format!("Contact Num{}", n % 20_000),
                    format!("contact{}@domaine.fr", n % 20_000),
                    format!("<banc-{n}@exemple.fr>"),
                    n * 60
                ])
                .unwrap();
            }
            conn.execute_batch(
                "COMMIT;
                 DELETE FROM correspondants;
                 DELETE FROM prefs WHERE key = 'annuaire_correspondants_v1';",
            )
            .unwrap();
        }
        let debut = std::time::Instant::now();
        store.rattraper_correspondants().unwrap();
        let duree = debut.elapsed();
        let total: i64 = store
            .conn()
            .query_row("SELECT COUNT(*) FROM correspondants", [], |row| row.get(0))
            .unwrap();
        println!("rattrapage 200 000 enveloppes -> {total} correspondants : {duree:?}");
        assert_eq!(total, 20_000);
    }

    /// Le rattrapage : une base existante peuple l'annuaire UNE fois —
    /// la marque tient, rejouer ne double rien ; indésirables exclus.
    #[test]
    fn le_rattrapage_peuple_une_fois_sans_les_indesirables() {
        let (mut store, _account, inbox, envoyes, spam) = decor();
        store
            .upsert_envelopes(inbox, &[envelope(1, "x", "Alice", "alice@exemple.fr", 100)])
            .unwrap();
        let mut envoi = envelope(1, "notre envoi", "Moi", "moi@exemple.fr", 200);
        envoi.to_addrs = vec!["dest@exemple.fr".to_string()];
        store.upsert_envelopes(envoyes, &[envoi]).unwrap();
        store
            .upsert_envelopes(
                spam,
                &[envelope(1, "pub", "Spammeur", "spam@nuisible.fr", 300)],
            )
            .unwrap();
        // L'« existant » : l'annuaire vidé, la marque retirée — l'état
        // exact d'une base d'avant PLAN-RETOURS-5.
        store
            .conn()
            .execute_batch(
                "DELETE FROM correspondants;
                 DELETE FROM prefs WHERE key = 'annuaire_correspondants_v1';",
            )
            .unwrap();

        store.rattraper_correspondants().unwrap();
        store.rattraper_correspondants().unwrap();

        let hits: i64 = store
            .conn()
            .query_row(
                "SELECT hits FROM correspondants WHERE address = 'alice@exemple.fr'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(hits, 1, "la marque tient : jamais deux passes");
        assert_eq!(store.completer_adresses("dest", 8).unwrap().len(), 1);
        assert!(store.completer_adresses("spam", 8).unwrap().is_empty());
    }
}
