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
    pub subject: String,
    pub body_text: String,
    pub in_reply_to: Option<String>,
    pub state: OutboxState,
    pub attempts: u32,
    pub last_error: Option<String>,
    pub queued_epoch: i64,
}

const OUTBOX_SELECT: &str = "SELECT id, account_id, message_id, sender, recipients, subject,
        body_text, in_reply_to, state, attempts, last_error, queued_epoch
 FROM outbox";

impl Store {
    /// Journalise l'intention d'envoi — AVANT toute tentative réseau.
    /// C'est cette écriture qui fonde « jamais d'envoi perdu ».
    pub fn enqueue_outbox(&self, account_id: i64, draft: &Draft) -> Result<i64, Error> {
        self.conn().execute(
            "INSERT INTO outbox
             (account_id, message_id, sender, recipients, subject, body_text,
              in_reply_to, state, queued_epoch)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                account_id,
                draft.message_id,
                draft.from,
                draft.to.join(&TO_SEPARATOR.to_string()),
                draft.subject,
                draft.body_text,
                draft.in_reply_to,
                OutboxState::Queued.as_str(),
                Utc::now().timestamp(),
            ],
        )?;
        Ok(self.conn().last_insert_rowid())
    }

    /// La file d'envoi d'UN compte, dans l'ordre d'émission — chaque
    /// vidange passe par la connexion SMTP de son compte.
    pub fn outbox_to_send(&self, account_id: i64) -> Result<Vec<OutboxMessage>, Error> {
        let mut stmt = self.conn().prepare(&format!(
            "{OUTBOX_SELECT} WHERE account_id = ?1 AND state = 'queued' ORDER BY id"
        ))?;
        let rows = stmt
            .query_map([account_id], row_to_outbox)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Toute la boîte d'envoi, dans l'ordre d'émission.
    pub fn outbox(&self) -> Result<Vec<OutboxMessage>, Error> {
        let mut stmt = self
            .conn()
            .prepare(&format!("{OUTBOX_SELECT} ORDER BY id"))?;
        let rows = stmt
            .query_map([], row_to_outbox)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Les messages dans un état donné, dans l'ordre d'émission.
    pub fn outbox_in_state(&self, state: OutboxState) -> Result<Vec<OutboxMessage>, Error> {
        let mut stmt = self
            .conn()
            .prepare(&format!("{OUTBOX_SELECT} WHERE state = ?1 ORDER BY id"))?;
        let rows = stmt
            .query_map([state.as_str()], row_to_outbox)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
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
    Ok(OutboxMessage {
        id: row.get(0)?,
        account_id: row.get(1)?,
        message_id: row.get(2)?,
        from: row.get(3)?,
        to: recipients.split(TO_SEPARATOR).map(str::to_string).collect(),
        subject: row.get(5)?,
        body_text: row.get(6)?,
        in_reply_to: row.get(7)?,
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
        compose("moi@exemple.fr", "vous@exemple.fr", subject, "corps", None).unwrap()
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

    /// Règle d'or n°1 : l'intention d'envoi survit à l'arrêt du processus.
    #[test]
    fn queued_send_survives_process_restart() {
        let path =
            std::env::temp_dir().join(format!("discovery-test-outbox-{}.db", std::process::id()));
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
