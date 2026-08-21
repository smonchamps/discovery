//! L'écho local (PLAN-REACTIVITE E3, verdict R-D1 « < 1 s ») : la
//! destination d'un geste se montre depuis la base locale, sans attendre
//! le serveur — hors ligne compris.
//!
//! Trois garde-fous, non négociables :
//! - **jamais de clé forgée** : l'écho vit dans SA table, servi en liste
//!   par une UNION (`nav.rs`) — jamais un UID inventé dans `envelopes` ;
//! - **jamais sans intention** : un écho reflète une action journalisée
//!   (suppression, archivage) ou un envoi passé à `sent` — un écho
//!   d'envoi ne naît JAMAIS avant l'acceptation SMTP (« jamais d'envoi
//!   fantôme ») ;
//! - **jamais contre le serveur** : l'écho meurt à la réconciliation
//!   (la vraie ligne entre — même `message_id` dans la destination) ou
//!   au balayage (intention soldée, destination relevée sans copie : on
//!   n'affiche pas ce que le serveur dément).

use rusqlite::{OptionalExtension, params};

use crate::action::Action;
use crate::envelope::Uid;
use crate::error::Error;
use crate::store::Store;

/// Les catégories qui portent des échos — les destinations des trois
/// gestes couverts. Un déplacement vers un dossier libre n'a pas de
/// liste où se montrer (la nav ne sert que les canoniques) : pas d'écho.
pub const DESTINATIONS_ECHO: &[&str] = &["envoyes", "archives", "corbeille"];

/// Le texte d'un envoi rendu en HTML minimal : échappé, retours à la
/// ligne préservés. C'est NOTRE texte (le journal d'envoi) — l'échappement
/// est la seule exigence ; l'assainissement de lecture repasse derrière
/// comme pour tout corps (S1).
pub fn texte_en_html(texte: &str) -> String {
    let echappe = texte
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    format!("<div>{}</div>", echappe.replace('\n', "<br>"))
}

impl Store {
    /// Le message porte-t-il déjà une enveloppe dans la destination ?
    /// (Gmail : archiver laisse la copie de « Tous les messages » — la
    /// clause d'exclusion la démasque au retrait d'INBOX, l'écho serait
    /// un doublon.) Une destination irrésolue ou jamais synchronisée
    /// répond « non » : l'écho est alors la seule vérité disponible.
    fn present_en_destination(
        &self,
        account_id: i64,
        destination: &str,
        message_id: &str,
    ) -> Result<bool, Error> {
        let dossiers = self.canonical_folders(account_id)?;
        let Some(nom) = dossiers.boite(destination) else {
            return Ok(false);
        };
        let Some(state) = self.sync_state(account_id, &nom)? else {
            return Ok(false);
        };
        let present: bool = self.conn().query_row(
            "SELECT EXISTS(SELECT 1 FROM envelopes
              WHERE mailbox_id = ?1 AND message_id = ?2)",
            params![state.mailbox_id, message_id],
            |row| row.get(0),
        )?;
        Ok(present)
    }

    /// Le geste qui déplace (suppression, archivage, déplacement) — en
    /// UNE transaction : l'action est journalisée, la matière du message
    /// (enveloppe, corps, aperçu, compte de pièces) est VERSÉE à l'écho
    /// de destination, puis la source se vide. Un crash entre deux ne
    /// perd rien et ne fabrique rien : tout ou rien.
    ///
    /// `destination = None` (déplacement vers un dossier libre) ou
    /// message sans `message_id` (l'écho serait irréconciliable) :
    /// l'action et la disparition locale se font, sans écho — le
    /// comportement d'avant E3, intact.
    pub fn geste_avec_echo(
        &self,
        mailbox_id: i64,
        uid: Uid,
        action: Action,
        destination: Option<&str>,
    ) -> Result<(), Error> {
        let account_id: i64 = self.conn().query_row(
            "SELECT account_id FROM mailboxes WHERE id = ?1",
            [mailbox_id],
            |row| row.get(0),
        )?;
        let tx = self.conn().unchecked_transaction()?;
        tx.execute(
            "INSERT INTO pending_actions (mailbox_id, uid, kind) VALUES (?1, ?2, ?3)",
            params![mailbox_id, uid, action.to_kind()],
        )?;
        let action_id = tx.last_insert_rowid();
        if let Some(destination) = destination {
            // La matière de l'écho se lit AVANT que la source se vide.
            type Matiere = (
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<i64>,
                Option<String>,
            );
            let enveloppe: Option<Matiere> = tx
                .query_row(
                    "SELECT subject, sender, sender_address, message_id, date_epoch, to_addrs
                     FROM envelopes WHERE mailbox_id = ?1 AND uid = ?2",
                    params![mailbox_id, uid],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                            row.get(5)?,
                        ))
                    },
                )
                .optional()?;
            if let Some((subject, sender, sender_address, Some(message_id), date_epoch, to_addrs)) =
                enveloppe
                && !self.present_en_destination(account_id, destination, &message_id)?
            {
                let corps: Option<(Option<String>, Option<String>)> = tx
                    .query_row(
                        "SELECT html, preview FROM bodies
                         WHERE mailbox_id = ?1 AND uid = ?2",
                        params![mailbox_id, uid],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .optional()?;
                let (html, preview) = corps.unwrap_or((None, None));
                let pieces: i64 = tx.query_row(
                    "SELECT COUNT(*) FROM attachments WHERE mailbox_id = ?1 AND uid = ?2",
                    params![mailbox_id, uid],
                    |row| row.get(0),
                )?;
                tx.execute(
                    "INSERT INTO echos (account_id, destination, message_id, sender,
                        sender_address, subject, date_epoch, preview, html,
                        attachment_count, to_addrs, origin_action_id, created_epoch)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, unixepoch())",
                    params![
                        account_id,
                        destination,
                        message_id,
                        sender,
                        sender_address,
                        subject,
                        date_epoch,
                        preview,
                        html,
                        pieces,
                        to_addrs,
                        action_id
                    ],
                )?;
            }
        }
        // La disparition de la source — le même travail que
        // `remove_local`, DANS la transaction (même connexion).
        self.remove_local(mailbox_id, uid)?;
        tx.commit()?;
        Ok(())
    }

    /// L'écho d'un envoi — appelé au passage à `sent` de la vidange, et
    /// SEULEMENT là : la requête refuse tout autre état, par
    /// construction ET par garde. Rend `true` si un écho est né.
    pub fn echo_envoi(&self, outbox_id: i64) -> Result<bool, Error> {
        type EnvoiRow = (
            i64,
            String,
            String,
            String,
            String,
            Option<String>,
            i64,
            String,
        );
        let row: Option<EnvoiRow> = self
            .conn()
            .query_row(
                "SELECT account_id, message_id, sender, subject, body_text, body_html,
                        queued_epoch, recipients
                 FROM outbox WHERE id = ?1 AND state = 'sent'",
                [outbox_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                    ))
                },
            )
            .optional()?;
        let Some((
            account_id,
            message_id,
            sender,
            subject,
            body_text,
            body_html,
            queued_epoch,
            recipients,
        )) = row
        else {
            return Ok(false);
        };
        if self.present_en_destination(account_id, "envoyes", &message_id)? {
            return Ok(false);
        }
        let pieces: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM outbox_attachments WHERE outbox_id = ?1",
            [outbox_id],
            |row| row.get(0),
        )?;
        // Un envoi riche montre SON HTML (PLAN-COMPOSITION-HTML) — le
        // ré-échapper afficherait les balises ; un envoi texte garde le
        // rendu échappé historique. La lecture ré-assainit dans les deux
        // cas (S1).
        let html = body_html.unwrap_or_else(|| texte_en_html(&body_text));
        let preview = crate::body::extraire_apercu(&html);
        // `outbox.recipients` est déjà joint par '\n' (TO_SEPARATOR) —
        // le format exact de `envelopes.to_addrs` : copie telle quelle.
        self.conn().execute(
            "INSERT INTO echos (account_id, destination, message_id, sender,
                sender_address, subject, date_epoch, preview, html,
                attachment_count, to_addrs, origin_outbox_id, created_epoch)
             VALUES (?1, 'envoyes', ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, unixepoch())",
            params![
                account_id,
                message_id,
                sender,
                sender,
                subject,
                queued_epoch,
                preview,
                html,
                pieces,
                recipients,
                outbox_id
            ],
        )?;
        Ok(true)
    }

    /// Les pièces d'un écho d'envoi, en MÉTADONNÉES seules (nom, mime,
    /// taille — les octets sont purgés à `sent`, PJ-D7) : de quoi
    /// afficher des puces honnêtes pendant la fenêtre de réconciliation,
    /// jamais un titre « Fichiers joints » sans rien dessous. Un écho de
    /// geste (`origin_outbox_id` NULL) n'en a pas : liste vide.
    pub fn echo_attachments(&self, echo_id: i64) -> Result<Vec<crate::OutboxAttachment>, Error> {
        let rows = self
            .conn()
            .prepare(
                "SELECT oa.name, oa.mime, oa.size
                 FROM echos ec
                 JOIN outbox_attachments oa ON oa.outbox_id = ec.origin_outbox_id
                 WHERE ec.id = ?1
                 ORDER BY oa.id",
            )?
            .query_map([echo_id], |row| {
                Ok(crate::OutboxAttachment {
                    name: row.get(0)?,
                    mime: row.get(1)?,
                    size: row.get::<_, i64>(2)? as u64,
                    bytes: None,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(rows)
    }

    /// Combien d'échos une catégorie porte — le complément des compteurs
    /// de nav et des totaux de pagination (« jamais deux vérités »).
    pub fn compte_echos(&self, destination: &str, account_id: Option<i64>) -> Result<u64, Error> {
        let count: i64 = match account_id {
            Some(id) => self.conn().query_row(
                "SELECT COUNT(*) FROM echos WHERE destination = ?1 AND account_id = ?2",
                params![destination, id],
                |row| row.get(0),
            )?,
            None => self.conn().query_row(
                "SELECT COUNT(*) FROM echos WHERE destination = ?1",
                params![destination],
                |row| row.get(0),
            )?,
        };
        Ok(count as u64)
    }

    /// Le corps d'un écho pour la Lecture : HTML (celui du message
    /// d'origine, ou le texte d'envoi rendu) et compte de pièces. `None`
    /// si l'écho a déjà été réconcilié — la vraie ligne a pris sa place.
    pub fn echo_vue(&self, echo_id: i64) -> Result<Option<(String, usize)>, Error> {
        let row: Option<(Option<String>, i64)> = self
            .conn()
            .query_row(
                "SELECT html, attachment_count FROM echos WHERE id = ?1",
                [echo_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        Ok(row.map(|(html, pieces)| (html.unwrap_or_default(), pieces as usize)))
    }

    /// La réconciliation : l'écho meurt quand la vraie ligne entre —
    /// même `message_id` dans une boîte de sa destination. Appelée après
    /// toute relève qui a pu servir une destination (cycle, passe
    /// d'après-geste). Rend le nombre d'échos retirés.
    pub fn reconcilier_echos(&self, account_id: i64) -> Result<usize, Error> {
        let echos: Vec<(i64, String, String)> = self
            .conn()
            .prepare("SELECT id, destination, message_id FROM echos WHERE account_id = ?1")?
            .query_map([account_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<Result<_, _>>()?;
        let mut retires = 0usize;
        for (id, destination, message_id) in echos {
            if self.present_en_destination(account_id, &destination, &message_id)? {
                self.conn()
                    .execute("DELETE FROM echos WHERE id = ?1", [id])?;
                retires += 1;
            }
        }
        Ok(retires)
    }

    /// Le balayage de sûreté : un écho dont l'INTENTION est soldée
    /// (action rejouée et retirée de la file, envoi parti) mais que la
    /// destination, relevée, ne montre toujours pas — on n'affiche pas
    /// ce que le serveur dément. À n'appeler qu'après une passe PROPRE
    /// (relèves sans erreur) et ses retentatives : un écho dont l'action
    /// attend encore (hors ligne, recul) VIT — il reflète l'intention.
    /// Rend un incident par écho retiré.
    pub fn balayer_echos(&self, account_id: i64) -> Result<Vec<String>, Error> {
        let perimes: Vec<(i64, String)> = self
            .conn()
            .prepare(
                "SELECT id, destination FROM echos
                 WHERE account_id = ?1
                   AND (origin_action_id IS NULL
                        OR NOT EXISTS (SELECT 1 FROM pending_actions p
                                        WHERE p.id = origin_action_id))",
            )?
            .query_map([account_id], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<_, _>>()?;
        let mut incidents = Vec::new();
        for (id, destination) in perimes {
            self.conn()
                .execute("DELETE FROM echos WHERE id = ?1", [id])?;
            incidents.push(format!(
                "copie attendue en « {destination} » jamais vue du serveur — écho retiré"
            ));
        }
        Ok(incidents)
    }

    /// Des échos attendent-ils encore leur réconciliation ? C'est le
    /// signal de retentative de la passe d'après-geste.
    pub fn echos_en_attente(&self, account_id: i64) -> Result<u64, Error> {
        let count: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM echos WHERE account_id = ?1",
            [account_id],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }

    /// Les boîtes d'un compte qui portent des actions en attente — la
    /// phase « intentions » de la passe d'après-geste : leur relève
    /// rejoue le journal MAINTENANT, au lieu d'attendre le cycle.
    pub fn mailboxes_avec_actions(&self, account_id: i64) -> Result<Vec<String>, Error> {
        let rows = self
            .conn()
            .prepare(
                "SELECT DISTINCT m.name FROM pending_actions p
                 JOIN mailboxes m ON m.id = p.mailbox_id
                 WHERE m.account_id = ?1",
            )?
            .query_map([account_id], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        Ok(rows)
    }

    /// Les comptes qui ont du travail d'après-geste — actions en attente
    /// ou échos à réconcilier. Le déclencheur du retour en ligne (R-D3)
    /// s'en sert : rien à faire = aucune connexion ouverte.
    pub fn comptes_avec_travail(&self) -> Result<Vec<i64>, Error> {
        let rows = self
            .conn()
            .prepare(
                "SELECT DISTINCT m.account_id FROM pending_actions p
                 JOIN mailboxes m ON m.id = p.mailbox_id
                 UNION
                 SELECT DISTINCT account_id FROM echos",
            )?
            .query_map([], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;
    use crate::envelope::Envelope;

    fn envelope(uid: Uid, subject: &str, epoch: i64) -> Envelope {
        Envelope {
            uid,
            subject: Some(subject.to_string()),
            sender: Some("Alice Martin".to_string()),
            sender_address: Some("alice@example.com".to_string()),
            message_id: Some(format!("<m{uid}@example.com>")),
            in_reply_to: None,
            date: Some(Utc.timestamp_opt(epoch, 0).unwrap()),
            seen: true,
            flagged: false,
            to_addrs: Vec::new(),
            cc_addrs: Vec::new(),
        }
    }

    fn store_avec_corbeille() -> (Store, i64, i64, i64) {
        let store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("t@exemple.fr", "gmail")
            .unwrap();
        let inbox = store.create_mailbox(account, "INBOX", 1).unwrap();
        let corbeille = store.create_mailbox(account, "Trash", 1).unwrap();
        store
            .replace_folders(
                account,
                &[
                    crate::Folder {
                        wire: "INBOX".into(),
                        display: "INBOX".into(),
                        selectable: true,
                    },
                    crate::Folder {
                        wire: "Trash".into(),
                        display: "Trash".into(),
                        selectable: true,
                    },
                ],
            )
            .unwrap();
        (store, account, inbox, corbeille)
    }

    /// Le geste vide la source, journalise l'action ET pose l'écho —
    /// avec la matière du message (aperçu, corps, pièces) : la
    /// destination se montre sans le serveur.
    #[test]
    fn le_geste_verse_la_matiere_a_l_echo() {
        let (mut store, account, inbox, _) = store_avec_corbeille();
        store
            .upsert_envelopes(inbox, &[envelope(1, "à jeter", 100)])
            .unwrap();
        store.save_body(inbox, 1, "<p>corps</p>", &[]).unwrap();

        store
            .geste_avec_echo(inbox, 1, Action::Delete, Some("corbeille"))
            .unwrap();

        // La source est vide, l'action journalisée.
        assert!(store.recent(account, "INBOX", 0, 10).unwrap().is_empty());
        assert_eq!(store.pending_actions(inbox).unwrap().len(), 1);
        // L'écho porte tout.
        assert_eq!(store.compte_echos("corbeille", Some(account)).unwrap(), 1);
        let (id, preview): (i64, Option<String>) = store
            .conn()
            .query_row("SELECT id, preview FROM echos", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .unwrap();
        assert_eq!(preview.as_deref(), Some("corps"));
        let (html, pieces) = store.echo_vue(id).unwrap().unwrap();
        assert_eq!(html, "<p>corps</p>");
        assert_eq!(pieces, 0);
    }

    /// Sans `message_id`, l'écho serait irréconciliable : le geste passe
    /// sans écho — le comportement d'avant E3, intact.
    #[test]
    fn sans_message_id_pas_d_echo() {
        let (mut store, account, inbox, _) = store_avec_corbeille();
        let mut sans_id = envelope(1, "anonyme", 100);
        sans_id.message_id = None;
        store.upsert_envelopes(inbox, &[sans_id]).unwrap();

        store
            .geste_avec_echo(inbox, 1, Action::Delete, Some("corbeille"))
            .unwrap();

        assert_eq!(store.compte_echos("corbeille", Some(account)).unwrap(), 0);
        assert!(store.recent(account, "INBOX", 0, 10).unwrap().is_empty());
        assert_eq!(store.pending_actions(inbox).unwrap().len(), 1);
    }

    /// Déjà présent dans la destination (Gmail : « Tous les messages »
    /// porte déjà la copie qu'un archivage démasque) : pas de doublon.
    #[test]
    fn present_en_destination_pas_de_doublon() {
        let (mut store, account, inbox, corbeille) = store_avec_corbeille();
        store
            .upsert_envelopes(inbox, &[envelope(1, "déjà là", 100)])
            .unwrap();
        store
            .upsert_envelopes(corbeille, &[envelope(1, "déjà là", 100)])
            .unwrap();

        store
            .geste_avec_echo(inbox, 1, Action::Delete, Some("corbeille"))
            .unwrap();

        assert_eq!(store.compte_echos("corbeille", Some(account)).unwrap(), 0);
    }

    /// La réconciliation : la vraie ligne entre (même `message_id` dans
    /// la destination) → l'écho meurt, la liste ne bouge pas à l'œil.
    #[test]
    fn la_vraie_ligne_tue_l_echo() {
        let (mut store, account, inbox, corbeille) = store_avec_corbeille();
        store
            .upsert_envelopes(inbox, &[envelope(1, "à jeter", 100)])
            .unwrap();
        store
            .geste_avec_echo(inbox, 1, Action::Delete, Some("corbeille"))
            .unwrap();
        assert_eq!(
            store.reconcilier_echos(account).unwrap(),
            0,
            "rien d'arrivé : rien à faire"
        );

        // La copie arrive dans la Corbeille (relève) — UID neuf, même
        // message_id.
        store
            .upsert_envelopes(corbeille, &[envelope(1, "à jeter", 100)])
            .unwrap();

        assert_eq!(store.reconcilier_echos(account).unwrap(), 1);
        assert_eq!(store.compte_echos("corbeille", Some(account)).unwrap(), 0);
    }

    /// Le balayage : l'action rejouée (retirée de la file) et toujours
    /// pas de copie → l'écho se retire, l'incident se consigne. Une
    /// action encore en file protège son écho — hors ligne, l'écho VIT.
    #[test]
    fn le_balayage_respecte_l_intention_en_attente() {
        let (mut store, account, inbox, _) = store_avec_corbeille();
        store
            .upsert_envelopes(inbox, &[envelope(1, "à jeter", 100)])
            .unwrap();
        store
            .geste_avec_echo(inbox, 1, Action::Delete, Some("corbeille"))
            .unwrap();

        // L'action attend encore : le balayage ne touche à rien.
        assert!(store.balayer_echos(account).unwrap().is_empty());
        assert_eq!(store.echos_en_attente(account).unwrap(), 1);

        // L'action est rejouée (la file se vide) — la destination
        // relevée ne montre rien : l'écho part, l'incident est dit.
        let action = store.pending_actions(inbox).unwrap().remove(0);
        store.remove_action(action.id).unwrap();
        let incidents = store.balayer_echos(account).unwrap();
        assert_eq!(incidents.len(), 1);
        assert!(incidents[0].contains("corbeille"), "{incidents:?}");
        assert_eq!(store.echos_en_attente(account).unwrap(), 0);
    }

    /// L'écho d'envoi ne naît qu'à `sent` — jamais pour une entrée en
    /// file, en échec ou en quarantaine (« jamais d'envoi fantôme »).
    #[test]
    fn l_echo_d_envoi_ne_nait_qu_a_sent() {
        let store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("t@exemple.fr", "gmail")
            .unwrap();
        let draft = crate::compose(
            "t@exemple.fr",
            "a@b.fr",
            "",
            "",
            "objet",
            "corps\nligne 2",
            None,
        )
        .unwrap();
        store.enqueue_outbox(account, &draft).unwrap();
        let id = store.outbox_to_send(account).unwrap()[0].id;

        assert!(!store.echo_envoi(id).unwrap(), "en file : pas d'écho");
        assert_eq!(store.compte_echos("envoyes", Some(account)).unwrap(), 0);

        store
            .set_outbox_state(id, crate::OutboxState::Sent)
            .unwrap();
        assert!(store.echo_envoi(id).unwrap());
        assert_eq!(store.compte_echos("envoyes", Some(account)).unwrap(), 1);
        // Le corps est le texte du journal, échappé et lisible.
        let echo_id: i64 = store
            .conn()
            .query_row("SELECT id FROM echos", [], |row| row.get(0))
            .unwrap();
        let (html, _) = store.echo_vue(echo_id).unwrap().unwrap();
        assert!(html.contains("corps<br>ligne 2"), "{html}");
    }

    /// PLAN-COMPOSITION-HTML : l'écho d'un envoi RICHE porte le HTML
    /// composé tel quel — jamais le texte ré-échappé (la mise en forme
    /// se verrait en balises dans Envoyés). L'assainissement de lecture
    /// repasse derrière comme pour tout corps (S1).
    #[test]
    fn l_echo_d_un_envoi_riche_porte_le_html_compose() {
        let store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("t@exemple.fr", "gmail")
            .unwrap();
        let mut draft =
            crate::compose("t@exemple.fr", "a@b.fr", "", "", "objet", "corps", None).unwrap();
        draft.body_html = Some("<div><b>corps</b></div>".to_string());
        store.enqueue_outbox(account, &draft).unwrap();
        let id = store.outbox_to_send(account).unwrap()[0].id;
        store
            .set_outbox_state(id, crate::OutboxState::Sent)
            .unwrap();
        assert!(store.echo_envoi(id).unwrap());

        let echo_id: i64 = store
            .conn()
            .query_row("SELECT id FROM echos", [], |row| row.get(0))
            .unwrap();
        let (html, _) = store.echo_vue(echo_id).unwrap().unwrap();
        assert!(html.contains("<b>corps</b>"), "{html}");
        assert!(
            !html.contains("&lt;b&gt;"),
            "le HTML ne doit pas être ré-échappé : {html}"
        );
    }

    /// PLAN-RETOURS-5 (terrain 2026-08-21 : « À : envoyes » pendant la
    /// fenêtre de réconciliation) : l'écho d'envoi porte les VRAIS
    /// destinataires, copiés du journal d'envoi au format des
    /// enveloppes (`\n`).
    #[test]
    fn l_echo_d_envoi_porte_les_destinataires() {
        let store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("t@exemple.fr", "gmail")
            .unwrap();
        let draft = crate::compose(
            "t@exemple.fr",
            "a@b.fr, c@d.fr",
            "",
            "",
            "objet",
            "corps",
            None,
        )
        .unwrap();
        store.enqueue_outbox(account, &draft).unwrap();
        let id = store.outbox_to_send(account).unwrap()[0].id;
        store
            .set_outbox_state(id, crate::OutboxState::Sent)
            .unwrap();
        assert!(store.echo_envoi(id).unwrap());

        let to: Option<String> = store
            .conn()
            .query_row("SELECT to_addrs FROM echos", [], |row| row.get(0))
            .unwrap();
        assert_eq!(to.as_deref(), Some("a@b.fr\nc@d.fr"));
    }

    /// Le geste verse aussi les destinataires du message déplacé — la
    /// colonne des enveloppes est déjà au bon format, copie telle quelle.
    #[test]
    fn le_geste_verse_les_destinataires_a_l_echo() {
        let (mut store, _account, inbox, _) = store_avec_corbeille();
        let mut env = envelope(1, "à jeter", 100);
        env.to_addrs = vec!["x@y.fr".to_string(), "z@w.fr".to_string()];
        store.upsert_envelopes(inbox, &[env]).unwrap();

        store
            .geste_avec_echo(inbox, 1, Action::Delete, Some("corbeille"))
            .unwrap();

        let to: Option<String> = store
            .conn()
            .query_row("SELECT to_addrs FROM echos", [], |row| row.get(0))
            .unwrap();
        assert_eq!(to.as_deref(), Some("x@y.fr\nz@w.fr"));
    }

    /// Les pièces d'un écho d'envoi se lisent en MÉTADONNÉES (nom, mime,
    /// taille) depuis le journal d'envoi — les octets sont purgés à
    /// `sent` (PJ-D7), jamais un titre « Fichiers joints » sans rien
    /// dessous. Un écho de geste n'en a pas : liste vide.
    #[test]
    fn les_pieces_de_l_echo_d_envoi_se_lisent_en_metadonnees() {
        let store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("t@exemple.fr", "gmail")
            .unwrap();
        let draft =
            crate::compose("t@exemple.fr", "a@b.fr", "", "", "objet", "corps", None).unwrap();
        let draft_id = store
            .save_draft(
                account,
                None,
                None,
                crate::DraftContent {
                    to_raw: "a@b.fr",
                    cc_raw: "",
                    bcc_raw: "",
                    body_html: None,
                    subject: "objet",
                    body: "corps",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                    important: false,
                },
            )
            .unwrap()
            .id;
        store
            .add_draft_attachment(draft_id, "rapport.pdf", "application/pdf", &[1, 2, 3])
            .unwrap();
        let id = store
            .enqueue_outbox_from_draft(account, &draft, draft_id)
            .unwrap();
        store
            .set_outbox_state(id, crate::OutboxState::Sent)
            .unwrap();
        store.purge_sent_attachment_bytes(id).unwrap();
        assert!(store.echo_envoi(id).unwrap());
        let echo_id: i64 = store
            .conn()
            .query_row("SELECT id FROM echos", [], |row| row.get(0))
            .unwrap();

        let pieces = store.echo_attachments(echo_id).unwrap();
        assert_eq!(pieces.len(), 1);
        assert_eq!(pieces[0].name, "rapport.pdf");
        assert_eq!(pieces[0].mime, "application/pdf");
        assert_eq!(pieces[0].size, 3);
        assert!(pieces[0].bytes.is_none(), "métadonnées seules");
    }

    /// Les comptes avec du travail : actions en attente OU échos — le
    /// déclencheur du retour en ligne ne réveille qu'eux.
    #[test]
    fn comptes_avec_travail_reunit_actions_et_echos() {
        let (mut store, account, inbox, _) = store_avec_corbeille();
        assert!(store.comptes_avec_travail().unwrap().is_empty());
        store
            .upsert_envelopes(inbox, &[envelope(1, "x", 100)])
            .unwrap();
        store
            .geste_avec_echo(inbox, 1, Action::Delete, Some("corbeille"))
            .unwrap();
        assert_eq!(store.comptes_avec_travail().unwrap(), vec![account]);
        assert_eq!(
            store.mailboxes_avec_actions(account).unwrap(),
            vec!["INBOX".to_string()]
        );
    }

    /// Le texte d'envoi rendu : échappé (jamais de HTML interprété
    /// depuis un texte), retours à la ligne préservés.
    #[test]
    fn le_texte_d_envoi_est_echappe() {
        assert_eq!(
            texte_en_html("a <b> & c\nd"),
            "<div>a &lt;b&gt; &amp; c<br>d</div>"
        );
    }
}
