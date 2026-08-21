//! La boîte d'envoi persistante — le sommet de la Phase 2.
//!
//! Deux règles d'or (PLAN.md §1 et §4), prouvées par tests :
//! - **jamais d'envoi perdu** : l'intention d'envoi est journalisée dans
//!   SQLite AVANT toute tentative réseau ; coupure ou crash, elle survit
//!   et repart à la vidange suivante ;
//! - **jamais d'envoi fantôme** : un envoi interrompu en plein vol (crash
//!   entre la remise au serveur et l'accusé local) n'est JAMAIS renvoyé
//!   automatiquement — il est mis en quarantaine jusqu'à la décision
//!   explicite de l'utilisateur. Le doublon silencieux est pire que le
//!   retard : un retard se rattrape, un doublon est déjà chez le
//!   destinataire.

use chrono::Utc;
use rusqlite::params;

use crate::compose::Draft;
use crate::error::Error;
use crate::store::Store;
use crate::transport::{MailTransport, SendError};

/// Séparateur des destinataires en base : sûr par construction, car
/// [`crate::EmailAddress`] refuse tout caractère blanc.
const TO_SEPARATOR: char = '\n';

/// Reconstitue une liste d'adresses stockée (Cc, Cci) : la chaîne VIDE
/// vaut liste vide — sans quoi `"".split('\n')` rendrait un `[""]` fantôme
/// (le champ « À », lui, n'est jamais vide et n'en a pas besoin).
fn split_recipients(stored: &str) -> Vec<String> {
    if stored.is_empty() {
        Vec::new()
    } else {
        stored.split(TO_SEPARATOR).map(str::to_string).collect()
    }
}

/// Cycle de vie d'un envoi. Machine à états stricte :
///
/// ```text
/// queued ──→ sending ──→ sent
///    ↑          │
///    │          ├─ échec transitoire ──→ queued (réessai automatique)
///    │          ├─ refus permanent ───→ rejected (décision utilisateur)
///    │          └─ crash en vol ──────→ interrupted (quarantaine)
///    └────────── requeue : décision explicite de l'utilisateur
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutboxState {
    /// En attente — sera pris par la prochaine vidange.
    Queued,
    /// Remise au serveur en cours. Retrouvé dans cet état au début d'une
    /// vidange, le message vient d'un crash : direction la quarantaine.
    Sending,
    /// Accepté par le serveur d'envoi.
    Sent,
    /// Interrompu en plein vol : peut-être parti, peut-être pas.
    /// JAMAIS renvoyé sans confirmation de l'utilisateur.
    Interrupted,
    /// Refusé définitivement par le serveur.
    Rejected,
}

impl OutboxState {
    pub fn as_str(self) -> &'static str {
        match self {
            OutboxState::Queued => "queued",
            OutboxState::Sending => "sending",
            OutboxState::Sent => "sent",
            OutboxState::Interrupted => "interrupted",
            OutboxState::Rejected => "rejected",
        }
    }

    pub(crate) fn parse(kind: &str) -> Option<Self> {
        match kind {
            "queued" => Some(OutboxState::Queued),
            "sending" => Some(OutboxState::Sending),
            "sent" => Some(OutboxState::Sent),
            "interrupted" => Some(OutboxState::Interrupted),
            "rejected" => Some(OutboxState::Rejected),
            _ => None,
        }
    }
}

/// Une pièce du journal d'envoi.
///
/// `bytes` est `None` une fois le message parti (purge PJ-D7) :
/// l'historique garde le nom et le poids, jamais les octets. Tant que le
/// message peut repartir — file, quarantaine, refus — les octets sont là.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboxAttachment {
    pub name: String,
    pub mime: String,
    /// Octets DÉCODÉS — la taille que l'utilisateur reconnaît.
    pub size: u64,
    pub bytes: Option<Vec<u8>>,
}

/// Un message journalisé dans la boîte d'envoi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboxMessage {
    pub id: i64,
    /// Le compte émetteur — chaque vidange passe par SA connexion SMTP.
    pub account_id: i64,
    /// Message-ID RFC 5322 généré à la composition — l'identité stable
    /// qui relie ce journal au message réellement parti.
    pub message_id: String,
    pub from: String,
    pub to: Vec<String>,
    /// Copie carbone — paraît dans l'en-tête `Cc:` du message envoyé.
    pub cc: Vec<String>,
    /// Copie carbone invisible — JAMAIS dans les en-têtes du message
    /// servi ; l'envoi la porte dans l'enveloppe SMTP seule (mail-smtp).
    pub bcc: Vec<String>,
    pub subject: String,
    pub body_text: String,
    /// Corps riche (PLAN-COMPOSITION-HTML) — la partie text/html du
    /// multipart/alternative ; `None` = envoi texte seul (historique).
    pub body_html: Option<String>,
    pub in_reply_to: Option<String>,
    /// Marqué « important » à la composition (R3) : la remise posera
    /// les en-têtes de priorité.
    pub important: bool,
    /// Envoi différé (R2) : l'époque (secondes) avant laquelle la
    /// vidange ne prend pas ce message. `None` = tout de suite.
    pub send_at_epoch: Option<i64>,
    /// Les pièces, dans l'ordre du geste (PJ-D2).
    pub attachments: Vec<OutboxAttachment>,
    pub state: OutboxState,
    pub attempts: u32,
    pub last_error: Option<String>,
    pub queued_epoch: i64,
}

const OUTBOX_SELECT: &str = "SELECT id, account_id, message_id, sender, recipients, subject,
        body_text, in_reply_to, state, attempts, last_error, queued_epoch, cc_addrs, bcc_addrs,
        body_html, important, send_at_epoch
 FROM outbox";

impl Store {
    /// Journalise l'intention d'envoi — AVANT toute tentative réseau.
    /// C'est cette écriture qui fonde « jamais d'envoi perdu ».
    pub fn enqueue_outbox(&self, account_id: i64, draft: &Draft) -> Result<i64, Error> {
        let sep = TO_SEPARATOR.to_string();
        self.conn().execute(
            "INSERT INTO outbox
             (account_id, message_id, sender, recipients, cc_addrs, bcc_addrs, subject, body_text,
              body_html, in_reply_to, important, state, queued_epoch)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                account_id,
                draft.message_id,
                draft.from,
                draft.to.join(&sep),
                draft.cc.join(&sep),
                draft.bcc.join(&sep),
                draft.subject,
                draft.body_text,
                draft.body_html,
                draft.in_reply_to,
                draft.important,
                OutboxState::Queued.as_str(),
                Utc::now().timestamp(),
            ],
        )?;
        let outbox_id = self.conn().last_insert_rowid();
        // PLAN-RETOURS-5 (D4) : une adresse qu'on écrit est une adresse
        // connue — l'annuaire l'apprend dès la mise en file, sans
        // attendre qu'elle revienne par la synchro d'Envoyés.
        let maintenant = Utc::now().timestamp();
        for adresse in draft.to.iter().chain(&draft.cc).chain(&draft.bcc) {
            crate::correspondants::noter(self.conn(), adresse, None, maintenant)?;
        }
        Ok(outbox_id)
    }

    /// Journalise l'intention d'envoi ET copie les pièces du brouillon
    /// dans la MÊME transaction (PJ-D2) : « jamais d'envoi perdu » couvre
    /// les octets — le brouillon peut ensuite disparaître (il a rempli
    /// son office), le journal se suffit.
    pub fn enqueue_outbox_from_draft(
        &self,
        account_id: i64,
        draft: &Draft,
        draft_id: i64,
    ) -> Result<i64, Error> {
        self.enqueue_outbox_full(account_id, draft, Some(draft_id), None)
    }

    /// Le chemin COMPLET de la mise en file (R2, PLAN-RETOURS-6) :
    /// brouillon-ancre facultatif, échéance facultative — le tout dans
    /// UNE transaction. « Jamais d'envoi perdu » couvre aussi l'heure
    /// choisie : un crash ne laisse jamais un envoi programmé amputé de
    /// son échéance (il partirait tout de suite, contre l'intention).
    pub fn enqueue_outbox_full(
        &self,
        account_id: i64,
        draft: &Draft,
        draft_id: Option<i64>,
        send_at_epoch: Option<i64>,
    ) -> Result<i64, Error> {
        let tx = self.conn().unchecked_transaction()?;
        let outbox_id = self.enqueue_outbox(account_id, draft)?;
        if let Some(draft_id) = draft_id {
            tx.execute(
                "INSERT INTO outbox_attachments (outbox_id, name, mime, size, bytes)
                 SELECT ?1, name, mime, size, bytes FROM draft_attachments
                 WHERE draft_id = ?2 ORDER BY id",
                params![outbox_id, draft_id],
            )?;
        }
        if let Some(send_at) = send_at_epoch {
            tx.execute(
                "UPDATE outbox SET send_at_epoch = ?2 WHERE id = ?1",
                params![outbox_id, send_at],
            )?;
        }
        tx.commit()?;
        Ok(outbox_id)
    }

    /// Annule un envoi programmé (R2, décision CE D2) : l'entrée quitte
    /// le journal et un brouillon COMPLET renaît — destinataires,
    /// corps, marquage, pièces avec leurs octets. Rien ne se perd, le
    /// geste est réversible. `None` si l'entrée n'est plus en file (la
    /// vidange l'a prise entre-temps : trop tard, le message part) —
    /// l'appelant le dit honnêtement plutôt que de promettre un
    /// brouillon qui n'existe pas.
    ///
    /// Ne vise que les entrées PROGRAMMÉES et pas encore échues : une
    /// entrée échue peut être en cours de remise par une vidange
    /// concurrente (hors de la file sérialisée) — l'annuler ici
    /// recréerait un brouillon d'un message peut-être parti (doublon).
    /// L'abandon d'un envoi ordinaire reste `delete_outbox`.
    pub fn annuler_envoi_programme(&self, id: i64) -> Result<Option<i64>, Error> {
        let tx = self.conn().unchecked_transaction()?;
        let mut stmt = self.conn().prepare(&format!(
            "{OUTBOX_SELECT} WHERE id = ?1 AND state = 'queued'
               AND send_at_epoch IS NOT NULL AND send_at_epoch > ?2"
        ))?;
        let Some(message) = stmt
            .query_map(params![id, Utc::now().timestamp()], row_to_outbox)?
            .next()
            .transpose()?
        else {
            drop(stmt);
            tx.commit()?;
            return Ok(None);
        };
        drop(stmt);
        // Le brouillon renaît dans le format du composeur : adresses
        // jointes par « , » (le champ tel qu'on le tape), corps et
        // marquage tels que le journal les porte.
        let maintenant = Utc::now().timestamp_millis();
        self.conn().execute(
            "INSERT INTO drafts (account_id, to_raw, cc_raw, bcc_raw, subject, body, body_html, important, updated_epoch)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                message.account_id,
                message.to.join(", "),
                message.cc.join(", "),
                message.bcc.join(", "),
                message.subject,
                message.body_text,
                message.body_html,
                message.important,
                maintenant,
            ],
        )?;
        let draft_id = self.conn().last_insert_rowid();
        // Les octets vivent au journal tant que l'envoi n'est pas parti
        // (PJ-D7) : la copie repart entière vers le brouillon.
        self.conn().execute(
            "INSERT INTO draft_attachments (draft_id, name, mime, size, bytes)
             SELECT ?1, name, mime, size, bytes FROM outbox_attachments
             WHERE outbox_id = ?2 AND bytes IS NOT NULL ORDER BY id",
            params![draft_id, id],
        )?;
        self.conn()
            .execute("DELETE FROM outbox WHERE id = ?1", [id])?;
        tx.commit()?;
        Ok(Some(draft_id))
    }

    /// La file d'envoi d'UN compte, dans l'ordre d'émission — chaque
    /// vidange passe par la connexion SMTP de son compte. Un envoi
    /// programmé (R2) n'y paraît qu'une fois son échéance passée : le
    /// filtre vit ICI, la porte unique de la vidange — aucun appelant
    /// ne peut faire partir un programmé en avance.
    pub fn outbox_to_send(&self, account_id: i64) -> Result<Vec<OutboxMessage>, Error> {
        let mut stmt = self.conn().prepare(&format!(
            "{OUTBOX_SELECT} WHERE account_id = ?1 AND state = 'queued'
               AND (send_at_epoch IS NULL OR send_at_epoch <= ?2) ORDER BY id"
        ))?;
        let rows = stmt
            .query_map(params![account_id, Utc::now().timestamp()], row_to_outbox)?
            .collect::<Result<Vec<_>, _>>()?;
        self.load_outbox_attachments(rows)
    }

    /// Toute la boîte d'envoi, dans l'ordre d'émission.
    pub fn outbox(&self) -> Result<Vec<OutboxMessage>, Error> {
        let mut stmt = self
            .conn()
            .prepare(&format!("{OUTBOX_SELECT} ORDER BY id"))?;
        let rows = stmt
            .query_map([], row_to_outbox)?
            .collect::<Result<Vec<_>, _>>()?;
        self.load_outbox_attachments(rows)
    }

    /// Les messages dans un état donné, dans l'ordre d'émission.
    pub fn outbox_in_state(&self, state: OutboxState) -> Result<Vec<OutboxMessage>, Error> {
        let mut stmt = self
            .conn()
            .prepare(&format!("{OUTBOX_SELECT} WHERE state = ?1 ORDER BY id"))?;
        let rows = stmt
            .query_map([state.as_str()], row_to_outbox)?
            .collect::<Result<Vec<_>, _>>()?;
        self.load_outbox_attachments(rows)
    }

    /// Attache leurs pièces aux messages relus. La boîte d'envoi se
    /// compte en unités — une requête par message est sans enjeu, et le
    /// chemin de lecture reste unique pour les trois entrées.
    fn load_outbox_attachments(
        &self,
        mut messages: Vec<OutboxMessage>,
    ) -> Result<Vec<OutboxMessage>, Error> {
        let mut stmt = self.conn().prepare(
            "SELECT name, mime, size, bytes FROM outbox_attachments
             WHERE outbox_id = ?1 ORDER BY id",
        )?;
        for message in &mut messages {
            message.attachments = stmt
                .query_map([message.id], |row| {
                    Ok(OutboxAttachment {
                        name: row.get(0)?,
                        mime: row.get(1)?,
                        size: row.get(2)?,
                        bytes: row.get(3)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
        }
        Ok(messages)
    }

    /// PJ-D7 : le message est parti, ses octets quittent le journal — les
    /// métadonnées restent (l'historique se lit encore). Seul `sent` purge :
    /// quarantaine et refus gardent tout, le renvoi doit rester entier.
    pub(crate) fn purge_sent_attachment_bytes(&self, id: i64) -> Result<(), Error> {
        self.conn().execute(
            "UPDATE outbox_attachments SET bytes = NULL WHERE outbox_id = ?1",
            [id],
        )?;
        Ok(())
    }

    pub(crate) fn set_outbox_state(&self, id: i64, state: OutboxState) -> Result<(), Error> {
        self.conn().execute(
            "UPDATE outbox SET state = ?2 WHERE id = ?1",
            params![id, state.as_str()],
        )?;
        Ok(())
    }

    /// Échec transitoire : retour en file, raison et compteur retenus.
    pub(crate) fn record_transient_failure(&self, id: i64, reason: &str) -> Result<(), Error> {
        self.conn().execute(
            "UPDATE outbox
             SET state = 'queued', attempts = attempts + 1, last_error = ?2
             WHERE id = ?1",
            params![id, reason],
        )?;
        Ok(())
    }

    /// Refus permanent : l'envoi sort de la file, l'utilisateur tranchera.
    pub(crate) fn record_rejection(&self, id: i64, reason: &str) -> Result<(), Error> {
        self.conn().execute(
            "UPDATE outbox
             SET state = 'rejected', attempts = attempts + 1, last_error = ?2
             WHERE id = ?1",
            params![id, reason],
        )?;
        Ok(())
    }

    /// Met en quarantaine les envois retrouvés « en vol » : seul un crash
    /// pendant la remise laisse cet état derrière lui. Peut-être partis,
    /// peut-être pas — on ne renvoie rien sans l'utilisateur.
    ///
    /// [`flush_outbox`] l'appelle en tête de vidange ; public pour que
    /// l'hôte puisse constater un crash antérieur même hors ligne,
    /// sans ouvrir de connexion.
    pub fn quarantine_inflight(&self) -> Result<usize, Error> {
        let quarantined = self.conn().execute(
            "UPDATE outbox SET state = 'interrupted' WHERE state = 'sending'",
            [],
        )?;
        Ok(quarantined)
    }

    /// Remet en file un envoi en quarantaine ou refusé — LA décision
    /// explicite de l'utilisateur qu'exige « jamais d'envoi fantôme ».
    pub fn requeue_outbox(&self, id: i64) -> Result<(), Error> {
        self.conn().execute(
            "UPDATE outbox SET state = 'queued'
             WHERE id = ?1 AND state IN ('interrupted', 'rejected')",
            [id],
        )?;
        Ok(())
    }

    /// Abandonne un envoi (décision utilisateur). Les envois `sent` sont
    /// préservés : ils sont l'historique prouvable de la boîte d'envoi.
    pub fn delete_outbox(&self, id: i64) -> Result<(), Error> {
        self.conn()
            .execute("DELETE FROM outbox WHERE id = ?1 AND state != 'sent'", [id])?;
        Ok(())
    }
}

fn row_to_outbox(row: &rusqlite::Row<'_>) -> rusqlite::Result<OutboxMessage> {
    let state_raw: String = row.get(8)?;
    let state = OutboxState::parse(&state_raw).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            8,
            rusqlite::types::Type::Text,
            format!("état de boîte d'envoi inconnu : {state_raw}").into(),
        )
    })?;
    let recipients: String = row.get(4)?;
    let cc_addrs: String = row.get(12)?;
    let bcc_addrs: String = row.get(13)?;
    Ok(OutboxMessage {
        id: row.get(0)?,
        account_id: row.get(1)?,
        message_id: row.get(2)?,
        from: row.get(3)?,
        to: recipients.split(TO_SEPARATOR).map(str::to_string).collect(),
        cc: split_recipients(&cc_addrs),
        bcc: split_recipients(&bcc_addrs),
        subject: row.get(5)?,
        body_text: row.get(6)?,
        body_html: row.get(14)?,
        in_reply_to: row.get(7)?,
        important: row.get(15)?,
        send_at_epoch: row.get(16)?,
        // Chargées par `load_outbox_attachments`, jamais ici : une ligne
        // ne connaît pas ses pièces.
        attachments: Vec::new(),
        state,
        attempts: row.get(9)?,
        last_error: row.get(10)?,
        queued_epoch: row.get(11)?,
    })
}

/// Bilan d'une vidange de la boîte d'envoi.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct OutboxReport {
    /// Acceptés par le serveur d'envoi.
    pub sent: usize,
    /// Reportés sur échec transitoire — toujours en file, retentés plus tard.
    pub deferred: usize,
    /// Refusés définitivement — sortis de la file, décision utilisateur.
    pub rejected: usize,
    /// Envois « en vol » d'un crash antérieur, mis en quarantaine.
    pub quarantined: usize,
}

/// Vide la boîte d'envoi vers le serveur, dans l'ordre d'émission.
///
/// La quarantaine passe D'ABORD : un envoi interrompu par un crash ne
/// repart jamais tout seul. Ensuite, chaque message en file est marqué
/// « en vol » (persisté) avant la remise, puis « envoyé » après l'accusé
/// du serveur — la fenêtre d'ambiguïté est réduite à la remise elle-même.
/// Au premier échec transitoire la pompe s'arrête : le réseau est tombé,
/// inutile d'insister, la file survit telle quelle.
pub fn flush_outbox(
    transport: &mut dyn MailTransport,
    store: &mut Store,
    account_id: i64,
) -> Result<OutboxReport, Error> {
    let mut report = OutboxReport {
        quarantined: store.quarantine_inflight()?,
        ..OutboxReport::default()
    };

    for message in store.outbox_to_send(account_id)? {
        store.set_outbox_state(message.id, OutboxState::Sending)?;
        match transport.send(&message) {
            Ok(()) => {
                store.set_outbox_state(message.id, OutboxState::Sent)?;
                store.purge_sent_attachment_bytes(message.id)?;
                // E3 (PLAN-REACTIVITE) : la copie Envoyés se montre TOUT
                // DE SUITE — l'écho local naît au passage à `sent`, jamais
                // avant (« jamais d'envoi fantôme »). Best effort : le
                // message EST parti, un échec d'écho ne doit pas le faire
                // passer pour perdu.
                let _ = store.echo_envoi(message.id);
                report.sent += 1;
            }
            Err(SendError::Transient(reason)) => {
                store.record_transient_failure(message.id, &reason)?;
                report.deferred += 1;
                break;
            }
            Err(SendError::Permanent(reason)) => {
                // Le refus d'UN message ne doit pas bloquer les autres.
                store.record_rejection(message.id, &reason)?;
                report.rejected += 1;
            }
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compose::compose;

    /// Transport simulé : accepte, coupe le réseau, ou refuse par sujet.
    #[derive(Default)]
    struct FakeTransport {
        accepted: Vec<String>,
        calls: usize,
        network_down: bool,
        reject_subjects: Vec<String>,
    }

    impl MailTransport for FakeTransport {
        fn send(&mut self, message: &OutboxMessage) -> Result<(), SendError> {
            self.calls += 1;
            if self.network_down {
                return Err(SendError::Transient("coupure réseau simulée".to_string()));
            }
            if self.reject_subjects.contains(&message.subject) {
                return Err(SendError::Permanent("550 refus simulé".to_string()));
            }
            self.accepted.push(message.message_id.clone());
            Ok(())
        }
    }

    fn draft(subject: &str) -> Draft {
        compose(
            "moi@exemple.fr",
            "vous@exemple.fr",
            "",
            "",
            subject,
            "corps",
            None,
        )
        .unwrap()
    }

    fn store() -> (Store, i64) {
        let store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("test@exemple.fr", "gmail")
            .unwrap();
        (store, account)
    }

    #[test]
    fn enqueue_journals_everything_before_any_network() {
        let (store, account) = store();
        let composed = compose(
            "moi@exemple.fr",
            "a@exemple.fr, b@exemple.fr",
            "",
            "",
            "Sujet",
            "Corps\nsur deux lignes",
            Some("<origine@exemple.fr>"),
        )
        .unwrap();
        let id = store.enqueue_outbox(account, &composed).unwrap();

        let queued = store.outbox_in_state(OutboxState::Queued).unwrap();
        assert_eq!(queued.len(), 1);
        let message = &queued[0];
        assert_eq!(message.id, id);
        assert_eq!(message.message_id, composed.message_id);
        assert_eq!(message.from, "moi@exemple.fr");
        assert_eq!(message.to, vec!["a@exemple.fr", "b@exemple.fr"]);
        assert_eq!(message.subject, "Sujet");
        assert_eq!(message.body_text, "Corps\nsur deux lignes");
        assert_eq!(message.in_reply_to.as_deref(), Some("<origine@exemple.fr>"));
        assert_eq!(message.attempts, 0);
        assert_eq!(message.last_error, None);
    }

    /// PLAN-COMPOSITION-HTML : le corps riche survit à l'enqueue et à la
    /// relecture — c'est lui que la vidange remettra à mail-smtp pour la
    /// partie text/html. Un envoi texte relit `None`, chemin historique.
    #[test]
    fn enqueue_roundtrips_body_html() {
        let (store, account) = store();
        let mut riche = draft("Sujet");
        riche.body_html = Some("<b>corps</b>".to_string());
        store.enqueue_outbox(account, &riche).unwrap();
        let nu = draft("Sujet 2");
        store.enqueue_outbox(account, &nu).unwrap();

        let queued = store.outbox_to_send(account).unwrap();
        assert_eq!(queued[0].body_html.as_deref(), Some("<b>corps</b>"));
        assert_eq!(queued[0].body_text, "corps", "le texte reste le repli");
        assert_eq!(queued[1].body_html, None);
    }

    /// R3 (PLAN-RETOURS-6) : le marquage « important » survit à
    /// l'enqueue et à la relecture — c'est le journal que la vidange
    /// remet à mail-smtp, les en-têtes de priorité en dépendent.
    #[test]
    fn enqueue_roundtrips_important() {
        let (store, account) = store();
        let mut urgent = draft("urgent");
        urgent.important = true;
        store.enqueue_outbox(account, &urgent).unwrap();
        store.enqueue_outbox(account, &draft("ordinaire")).unwrap();

        let queued = store.outbox_to_send(account).unwrap();
        assert!(queued[0].important, "le journal porte le marquage");
        assert!(!queued[1].important, "l'envoi ordinaire reste ordinaire");
    }

    /// A54 : Cc/Cci du journal survivent à l'enqueue et à la relecture ;
    /// un envoi sans copie les relit VIDES, jamais un `[""]` fantôme (le
    /// garde de `split_recipients`).
    #[test]
    fn enqueue_roundtrips_cc_and_bcc() {
        let (avec_store, compte) = store();
        let avec = compose(
            "moi@exemple.fr",
            "a@exemple.fr",
            "b@exemple.fr, c@exemple.fr",
            "secret@exemple.fr",
            "Sujet",
            "corps",
            None,
        )
        .unwrap();
        avec_store.enqueue_outbox(compte, &avec).unwrap();
        let releve = avec_store.outbox_to_send(compte).unwrap();
        assert_eq!(releve[0].cc, vec!["b@exemple.fr", "c@exemple.fr"]);
        assert_eq!(releve[0].bcc, vec!["secret@exemple.fr"]);

        let (nu_store, compte) = store();
        nu_store.enqueue_outbox(compte, &draft("nu")).unwrap();
        let nu = nu_store.outbox_to_send(compte).unwrap();
        assert!(nu[0].cc.is_empty(), "pas de destinataire Cc fantôme");
        assert!(nu[0].bcc.is_empty(), "pas de destinataire Cci fantôme");
    }

    /// R2 (PLAN-RETOURS-6) : un envoi programmé attend son heure — la
    /// vidange l'ignore tant que l'échéance n'est pas passée, puis le
    /// prend comme n'importe quel envoi en file. L'échéance se relit
    /// (elle survit, règle d'or n°1 étendue à l'heure choisie).
    #[test]
    fn scheduled_send_waits_for_its_hour() {
        let (mut store, account) = store();
        let futur = Utc::now().timestamp() + 3600;
        let programme = store
            .enqueue_outbox_full(account, &draft("plus tard"), None, Some(futur))
            .unwrap();
        store
            .enqueue_outbox_full(
                account,
                &draft("échu"),
                None,
                Some(Utc::now().timestamp() - 60),
            )
            .unwrap();
        let mut transport = FakeTransport::default();

        let report = flush_outbox(&mut transport, &mut store, account).unwrap();

        assert_eq!(report.sent, 1, "seul l'échu part");
        let sent = store.outbox_in_state(OutboxState::Sent).unwrap();
        assert_eq!(sent[0].subject, "échu");
        let queued = store.outbox_in_state(OutboxState::Queued).unwrap();
        assert_eq!(queued.len(), 1, "le programmé attend toujours");
        assert_eq!(queued[0].id, programme);
        assert_eq!(queued[0].send_at_epoch, Some(futur), "l'échéance se relit");
    }

    /// Règle d'or n°1 : l'intention d'envoi survit à l'arrêt du processus.
    #[test]
    fn queued_send_survives_process_restart() {
        let path = std::env::temp_dir().join(format!("wind-test-outbox-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let store = Store::open(&path).unwrap();
            let account = store
                .adopt_or_create_account("test@exemple.fr", "gmail")
                .unwrap();
            store.enqueue_outbox(account, &draft("survivant")).unwrap();
        } // « crash » : le processus s'arrête avant tout envoi.

        let reopened = Store::open(&path).unwrap();
        let queued = reopened.outbox_in_state(OutboxState::Queued).unwrap();
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].subject, "survivant");

        drop(reopened);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn flush_sends_in_emission_order_and_marks_sent() {
        let (mut store, account) = store();
        let first = draft("premier");
        let second = draft("second");
        store.enqueue_outbox(account, &first).unwrap();
        store.enqueue_outbox(account, &second).unwrap();
        let mut transport = FakeTransport::default();

        let report = flush_outbox(&mut transport, &mut store, account).unwrap();

        assert_eq!(report.sent, 2);
        assert_eq!(
            transport.accepted,
            vec![first.message_id, second.message_id],
            "l'ordre d'émission doit être préservé"
        );
        assert!(
            store
                .outbox_in_state(OutboxState::Queued)
                .unwrap()
                .is_empty()
        );
        assert_eq!(store.outbox_in_state(OutboxState::Sent).unwrap().len(), 2);
        // E3 : chaque envoi parti a son écho Envoyés — la copie se
        // montre sans attendre la relève du serveur.
        assert_eq!(store.compte_echos("envoyes", Some(account)).unwrap(), 2);
    }

    /// Règle d'or n°1 : une coupure réseau ne perd rien — la file survit
    /// et repart à la vidange suivante.
    #[test]
    fn network_cut_keeps_message_queued_then_next_flush_sends_it() {
        let (mut store, account) = store();
        store.enqueue_outbox(account, &draft("à retenter")).unwrap();

        let mut down = FakeTransport {
            network_down: true,
            ..FakeTransport::default()
        };
        let cut = flush_outbox(&mut down, &mut store, account).unwrap();
        assert_eq!((cut.sent, cut.deferred), (0, 1));
        let queued = store.outbox_in_state(OutboxState::Queued).unwrap();
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].attempts, 1);
        assert_eq!(
            queued[0].last_error.as_deref(),
            Some("coupure réseau simulée")
        );

        let mut up = FakeTransport::default();
        let recovered = flush_outbox(&mut up, &mut store, account).unwrap();
        assert_eq!(recovered.sent, 1);
        assert_eq!(store.outbox_in_state(OutboxState::Sent).unwrap().len(), 1);
    }

    /// Réseau tombé : inutile de marteler le serveur pour chaque message.
    #[test]
    fn transient_failure_stops_the_pump_after_one_attempt() {
        let (mut store, account) = store();
        store.enqueue_outbox(account, &draft("a")).unwrap();
        store.enqueue_outbox(account, &draft("b")).unwrap();
        let mut down = FakeTransport {
            network_down: true,
            ..FakeTransport::default()
        };

        flush_outbox(&mut down, &mut store, account).unwrap();

        assert_eq!(down.calls, 1, "un seul essai suffit à constater la coupure");
        assert_eq!(store.outbox_in_state(OutboxState::Queued).unwrap().len(), 2);
    }

    #[test]
    fn permanent_rejection_steps_aside_and_the_rest_still_goes() {
        let (mut store, account) = store();
        store.enqueue_outbox(account, &draft("mauvais")).unwrap();
        store.enqueue_outbox(account, &draft("bon")).unwrap();
        let mut transport = FakeTransport {
            reject_subjects: vec!["mauvais".to_string()],
            ..FakeTransport::default()
        };

        let report = flush_outbox(&mut transport, &mut store, account).unwrap();

        assert_eq!((report.sent, report.rejected), (1, 1));
        let rejected = store.outbox_in_state(OutboxState::Rejected).unwrap();
        assert_eq!(rejected.len(), 1);
        assert_eq!(rejected[0].last_error.as_deref(), Some("550 refus simulé"));

        // Le refus est définitif : la vidange suivante ne le retente pas.
        let mut second = FakeTransport::default();
        let idle = flush_outbox(&mut second, &mut store, account).unwrap();
        assert_eq!(second.calls, 0);
        assert_eq!(idle, OutboxReport::default());
    }

    /// Règle d'or n°2 : un envoi interrompu en plein vol (crash pendant la
    /// remise) n'est JAMAIS renvoyé automatiquement — quarantaine.
    #[test]
    fn inflight_message_is_quarantined_never_resent() {
        let (mut store, account) = store();
        let id = store.enqueue_outbox(account, &draft("ambigu")).unwrap();
        // Crash simulé : l'état « sending » persiste, l'accusé n'est
        // jamais revenu. Peut-être parti, peut-être pas.
        store.set_outbox_state(id, OutboxState::Sending).unwrap();

        let mut transport = FakeTransport::default();
        let report = flush_outbox(&mut transport, &mut store, account).unwrap();

        assert_eq!(report.quarantined, 1);
        assert_eq!(transport.calls, 0, "rien ne doit repartir tout seul");
        let interrupted = store.outbox_in_state(OutboxState::Interrupted).unwrap();
        assert_eq!(interrupted.len(), 1);
        assert_eq!(interrupted[0].id, id);
    }

    /// La sortie de quarantaine est une décision de l'utilisateur — et
    /// alors seulement, l'envoi repart.
    #[test]
    fn user_requeue_is_the_only_way_out_of_quarantine() {
        let (mut store, account) = store();
        let id = store.enqueue_outbox(account, &draft("confirmé")).unwrap();
        store.set_outbox_state(id, OutboxState::Sending).unwrap();
        let mut transport = FakeTransport::default();
        flush_outbox(&mut transport, &mut store, account).unwrap();
        assert!(transport.accepted.is_empty());

        store.requeue_outbox(id).unwrap();
        let report = flush_outbox(&mut transport, &mut store, account).unwrap();

        assert_eq!(report.sent, 1);
        assert_eq!(store.outbox_in_state(OutboxState::Sent).unwrap().len(), 1);
    }

    #[test]
    fn requeue_ignores_states_that_are_not_user_decisions() {
        let (mut store, account) = store();
        let id = store.enqueue_outbox(account, &draft("déjà parti")).unwrap();
        let mut transport = FakeTransport::default();
        flush_outbox(&mut transport, &mut store, account).unwrap();

        store.requeue_outbox(id).unwrap();

        assert_eq!(
            store.outbox_in_state(OutboxState::Sent).unwrap().len(),
            1,
            "un envoi accepté ne redevient jamais candidat à l'envoi"
        );
    }

    #[test]
    fn delete_abandons_pending_but_preserves_sent_history() {
        let (mut store, account) = store();
        let kept = store.enqueue_outbox(account, &draft("parti")).unwrap();
        let mut transport = FakeTransport::default();
        flush_outbox(&mut transport, &mut store, account).unwrap();
        let abandoned = store.enqueue_outbox(account, &draft("abandonné")).unwrap();

        store.delete_outbox(abandoned).unwrap();
        store.delete_outbox(kept).unwrap();

        let all = store.outbox().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].state, OutboxState::Sent);
    }

    /// Chaque compte vide SA file par SA connexion SMTP : la vidange
    /// d'un compte ne touche jamais la file d'un autre.
    #[test]
    fn flush_only_sends_the_given_accounts_queue() {
        let (mut store, account) = store();
        let other = store
            .adopt_or_create_account("autre@exemple.fr", "gmail")
            .unwrap();
        store
            .enqueue_outbox(account, &draft("du compte A"))
            .unwrap();
        store.enqueue_outbox(other, &draft("du compte B")).unwrap();
        let mut transport = FakeTransport::default();

        let report = flush_outbox(&mut transport, &mut store, account).unwrap();

        assert_eq!(report.sent, 1);
        assert_eq!(
            store.outbox_to_send(other).unwrap().len(),
            1,
            "la file de B attend SA connexion"
        );
    }

    #[test]
    fn outbox_state_labels_roundtrip() {
        for state in [
            OutboxState::Queued,
            OutboxState::Sending,
            OutboxState::Sent,
            OutboxState::Interrupted,
            OutboxState::Rejected,
        ] {
            assert_eq!(OutboxState::parse(state.as_str()), Some(state));
        }
        assert_eq!(OutboxState::parse("inconnu"), None);
    }
}

#[cfg(test)]
mod tests_pieces {
    use super::*;
    use crate::compose::compose;
    use crate::drafts::DraftContent;

    /// Transport simulé, réduit à ce que ce module vérifie : les pièces
    /// vues à la remise — ce que le transport reçoit est ce qui part.
    #[derive(Default)]
    struct FakeTransport {
        attachments_seen: Vec<(String, bool)>,
    }

    impl MailTransport for FakeTransport {
        fn send(&mut self, message: &OutboxMessage) -> Result<(), SendError> {
            for piece in &message.attachments {
                self.attachments_seen
                    .push((piece.name.clone(), piece.bytes.is_some()));
            }
            Ok(())
        }
    }

    fn store() -> (Store, i64) {
        let store = Store::open_in_memory().unwrap();
        let account = store
            .adopt_or_create_account("test@exemple.fr", "gmail")
            .unwrap();
        (store, account)
    }

    fn draft_with_pieces(store: &Store, account: i64) -> i64 {
        let id = store
            .save_draft(
                account,
                None,
                None,
                DraftContent {
                    to_raw: "vous@exemple.fr",
                    cc_raw: "",
                    bcc_raw: "",
                    body_html: None,
                    subject: "Photos",
                    body: "corps",
                    reply_to_uid: None,
                    reply_to_mailbox: None,
                    important: false,
                },
            )
            .unwrap()
            .id;
        store
            .add_draft_attachment(id, "facade.jpg", "image/jpeg", &[1, 2, 3])
            .unwrap();
        store
            .add_draft_attachment(id, "devis.pdf", "application/pdf", &[4, 5])
            .unwrap();
        id
    }

    fn composed() -> Draft {
        compose(
            "moi@exemple.fr",
            "vous@exemple.fr",
            "",
            "",
            "Photos",
            "corps",
            None,
        )
        .unwrap()
    }

    /// PJ-D2 : le geste copie les pièces au journal — le brouillon peut
    /// ensuite disparaître (envoi = il a rempli son office), le journal
    /// se suffit à lui-même.
    #[test]
    fn enqueue_copies_pieces_and_survives_draft_deletion() {
        let (store, account) = store();
        let draft_id = draft_with_pieces(&store, account);
        store
            .enqueue_outbox_from_draft(account, &composed(), draft_id)
            .unwrap();

        store.delete_draft(draft_id).unwrap();

        let queued = store.outbox_in_state(OutboxState::Queued).unwrap();
        assert_eq!(queued.len(), 1);
        let pieces = &queued[0].attachments;
        assert_eq!(pieces.len(), 2);
        assert_eq!(pieces[0].name, "facade.jpg");
        assert_eq!(pieces[0].mime, "image/jpeg");
        assert_eq!(pieces[0].size, 3);
        assert_eq!(pieces[0].bytes.as_deref(), Some(&[1u8, 2, 3][..]));
        assert_eq!(pieces[1].name, "devis.pdf");
        assert_eq!(pieces[1].bytes.as_deref(), Some(&[4u8, 5][..]));
    }

    /// Règle d'or n°1, étendue : un crash entre le geste et la vidange ne
    /// perd aucun octet — les pièces survivent à l'arrêt du processus.
    #[test]
    fn queued_pieces_survive_process_restart() {
        let path =
            std::env::temp_dir().join(format!("wind-test-outbox-pieces-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let store = Store::open(&path).unwrap();
            let account = store
                .adopt_or_create_account("test@exemple.fr", "gmail")
                .unwrap();
            let draft_id = draft_with_pieces(&store, account);
            store
                .enqueue_outbox_from_draft(account, &composed(), draft_id)
                .unwrap();
        } // « crash » : le processus s'arrête avant toute vidange.

        let reopened = Store::open(&path).unwrap();
        let queued = reopened.outbox_in_state(OutboxState::Queued).unwrap();
        assert_eq!(queued[0].attachments.len(), 2);
        assert!(queued[0].attachments.iter().all(|p| p.bytes.is_some()));

        drop(reopened);
        let _ = std::fs::remove_file(&path);
    }

    /// La remise reçoit les octets ; PJ-D7 : sitôt parti, le journal les
    /// purge — les métadonnées restent, l'historique se lit encore.
    #[test]
    fn sent_pieces_are_purged_to_metadata_only() {
        let (mut store, account) = store();
        let draft_id = draft_with_pieces(&store, account);
        store
            .enqueue_outbox_from_draft(account, &composed(), draft_id)
            .unwrap();
        let mut transport = FakeTransport::default();

        let report = flush_outbox(&mut transport, &mut store, account).unwrap();

        assert_eq!(report.sent, 1);
        assert_eq!(
            transport.attachments_seen,
            vec![
                ("facade.jpg".to_string(), true),
                ("devis.pdf".to_string(), true)
            ],
            "la remise part avec les octets"
        );
        let sent = store.outbox_in_state(OutboxState::Sent).unwrap();
        let pieces = &sent[0].attachments;
        assert_eq!(pieces.len(), 2, "les métadonnées restent");
        assert_eq!(pieces[0].name, "facade.jpg");
        assert_eq!(pieces[0].size, 3);
        assert!(
            pieces.iter().all(|p| p.bytes.is_none()),
            "les octets ont quitté le journal"
        );
    }

    /// PJ-D7, l'autre moitié : la quarantaine GARDE ses octets — le
    /// renvoi sur décision de l'utilisateur doit rester entier.
    #[test]
    fn quarantined_pieces_keep_their_bytes_and_requeue_sends_them() {
        let (mut store, account) = store();
        let draft_id = draft_with_pieces(&store, account);
        let id = store
            .enqueue_outbox_from_draft(account, &composed(), draft_id)
            .unwrap();
        // Crash simulé pendant la remise : l'état « sending » persiste.
        store.set_outbox_state(id, OutboxState::Sending).unwrap();

        let mut transport = FakeTransport::default();
        flush_outbox(&mut transport, &mut store, account).unwrap();
        let interrupted = store.outbox_in_state(OutboxState::Interrupted).unwrap();
        assert!(
            interrupted[0].attachments.iter().all(|p| p.bytes.is_some()),
            "la quarantaine garde tout"
        );

        store.requeue_outbox(id).unwrap();
        let report = flush_outbox(&mut transport, &mut store, account).unwrap();
        assert_eq!(report.sent, 1);
        assert!(
            transport.attachments_seen.iter().all(|(_, bytes)| *bytes),
            "le renvoi part entier"
        );
    }

    /// L'abandon d'un envoi en file emporte ses blobs (cascade) — pas
    /// d'octets orphelins dans le journal.
    #[test]
    fn deleting_a_pending_send_cascades_to_its_pieces() {
        let (store, account) = store();
        let draft_id = draft_with_pieces(&store, account);
        let id = store
            .enqueue_outbox_from_draft(account, &composed(), draft_id)
            .unwrap();

        store.delete_outbox(id).unwrap();

        let orphans: i64 = store
            .conn()
            .query_row("SELECT COUNT(*) FROM outbox_attachments", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(orphans, 0);
    }

    /// R2, décision CE D2 : annuler un envoi programmé recrée le
    /// brouillon ENTIER — destinataires, corps, marquage « important »,
    /// pièces avec leurs octets — et l'entrée quitte le journal. Rien
    /// ne se perd, le geste est réversible.
    #[test]
    fn annuler_un_programme_recree_le_brouillon() {
        let (store, account) = store();
        let draft_id = draft_with_pieces(&store, account);
        let mut urgent = composed();
        urgent.important = true;
        let id = store
            .enqueue_outbox_full(
                account,
                &urgent,
                Some(draft_id),
                Some(chrono::Utc::now().timestamp() + 3600),
            )
            .unwrap();
        // Le flux réel supprime le brouillon sitôt l'envoi journalisé.
        store.delete_draft(draft_id).unwrap();

        let recree = store
            .annuler_envoi_programme(id)
            .unwrap()
            .expect("un brouillon recréé");

        assert!(
            store.outbox().unwrap().is_empty(),
            "l'entrée quitte le journal"
        );
        let drafts = store.drafts().unwrap();
        assert_eq!(drafts.len(), 1);
        let brouillon = &drafts[0];
        assert_eq!(brouillon.id, recree);
        assert_eq!(brouillon.to_raw, "vous@exemple.fr");
        assert_eq!(brouillon.subject, "Photos");
        assert_eq!(brouillon.body, "corps");
        assert!(brouillon.important, "le marquage revient");
        let pieces = store.draft_attachments_full(recree).unwrap();
        assert_eq!(pieces.len(), 2);
        assert_eq!(pieces[0].name, "facade.jpg");
        assert_eq!(
            pieces[0].bytes,
            vec![1, 2, 3],
            "les octets reviennent entiers"
        );
    }

    /// D2, l'autre moitié : une entrée déjà PARTIE (la vidange l'a prise
    /// avant le geste) ne s'annule pas — `None`, et l'historique reste.
    /// Et une entrée ordinaire (sans échéance) non plus : elle peut être
    /// en cours de remise par une vidange concurrente — l'abandon d'un
    /// envoi ordinaire reste `delete_outbox`.
    #[test]
    fn annuler_un_envoi_parti_ne_fait_rien() {
        let (mut store, account) = store();
        let draft_id = draft_with_pieces(&store, account);
        let id = store
            .enqueue_outbox_from_draft(account, &composed(), draft_id)
            .unwrap();
        assert_eq!(
            store.annuler_envoi_programme(id).unwrap(),
            None,
            "une entrée SANS échéance ne passe pas par cette voie"
        );
        let mut transport = FakeTransport::default();
        flush_outbox(&mut transport, &mut store, account).unwrap();

        assert_eq!(store.annuler_envoi_programme(id).unwrap(), None);
        assert_eq!(
            store.outbox_in_state(OutboxState::Sent).unwrap().len(),
            1,
            "l'historique d'envoi ne bouge pas"
        );
    }

    /// Un envoi sans brouillon (composition jamais sauvée) reste
    /// possible : le chemin historique n'exige aucune pièce.
    #[test]
    fn plain_enqueue_still_carries_no_pieces() {
        let (store, account) = store();
        store.enqueue_outbox(account, &composed()).unwrap();
        let queued = store.outbox_in_state(OutboxState::Queued).unwrap();
        assert!(queued[0].attachments.is_empty());
    }
}
