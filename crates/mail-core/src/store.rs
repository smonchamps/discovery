//! Stockage local SQLite : enveloppes et état de synchro, multi-boîtes.
//!
//! Structure concrète (pas de trait) : SQLite est une décision produit gelée
//! (PHASE0.md §2.1) et les tests utilisent une base en mémoire — l'abstraction
//! du réseau ([`crate::MailServer`]) est la seule frontière nécessaire.

use std::collections::HashSet;
use std::path::Path;

use chrono::DateTime;
use rusqlite::{Connection, OptionalExtension, params};

use crate::action::{Action, PendingAction};
use crate::envelope::{Envelope, Uid};
use crate::error::Error;

const SCHEMA: &str = "
PRAGMA foreign_keys = ON;
CREATE TABLE IF NOT EXISTS mailboxes (
    id             INTEGER PRIMARY KEY,
    name           TEXT NOT NULL UNIQUE,
    uid_validity   INTEGER NOT NULL,
    last_uid       INTEGER NOT NULL DEFAULT 0,
    highest_modseq INTEGER
);
CREATE TABLE IF NOT EXISTS envelopes (
    mailbox_id INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    uid        INTEGER NOT NULL,
    subject    TEXT,
    sender     TEXT,
    date_epoch INTEGER,
    seen       INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (mailbox_id, uid)
);
CREATE INDEX IF NOT EXISTS idx_envelopes_date ON envelopes(mailbox_id, date_epoch DESC);
CREATE TABLE IF NOT EXISTS bodies (
    mailbox_id INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    uid        INTEGER NOT NULL,
    html       TEXT NOT NULL,
    PRIMARY KEY (mailbox_id, uid)
);
CREATE TABLE IF NOT EXISTS pending_actions (
    id         INTEGER PRIMARY KEY,
    mailbox_id INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    uid        INTEGER NOT NULL,
    kind       TEXT NOT NULL
);
";

/// État de synchro persisté d'une boîte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncState {
    pub mailbox_id: i64,
    pub uid_validity: u32,
    pub last_uid: Uid,
    pub highest_modseq: Option<u64>,
}

pub struct Store(Connection);

impl Store {
    pub fn open(path: &Path) -> Result<Self, Error> {
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self, Error> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self, Error> {
        conn.execute_batch(SCHEMA)?;
        Ok(Self(conn))
    }

    pub fn sync_state(&self, mailbox: &str) -> Result<Option<SyncState>, Error> {
        let state = self
            .0
            .query_row(
                "SELECT id, uid_validity, last_uid, highest_modseq
                 FROM mailboxes WHERE name = ?1",
                [mailbox],
                |row| {
                    Ok(SyncState {
                        mailbox_id: row.get(0)?,
                        uid_validity: row.get(1)?,
                        last_uid: row.get(2)?,
                        highest_modseq: row.get::<_, Option<i64>>(3)?.map(|m| m as u64),
                    })
                },
            )
            .optional()?;
        Ok(state)
    }

    pub fn create_mailbox(&self, mailbox: &str, uid_validity: u32) -> Result<i64, Error> {
        self.0.execute(
            "INSERT INTO mailboxes (name, uid_validity) VALUES (?1, ?2)",
            params![mailbox, uid_validity],
        )?;
        Ok(self.0.last_insert_rowid())
    }

    /// Repart de zéro pour une boîte dont l'UIDVALIDITY a changé : les UIDs
    /// ne veulent plus rien dire — corps et actions en attente compris (une
    /// intention sur un UID invalidé est irréalisable par construction).
    pub fn reset_mailbox(&self, mailbox_id: i64, uid_validity: u32) -> Result<(), Error> {
        self.0.execute(
            "DELETE FROM pending_actions WHERE mailbox_id = ?1",
            [mailbox_id],
        )?;
        self.0
            .execute("DELETE FROM bodies WHERE mailbox_id = ?1", [mailbox_id])?;
        self.0
            .execute("DELETE FROM envelopes WHERE mailbox_id = ?1", [mailbox_id])?;
        self.0.execute(
            "UPDATE mailboxes
             SET uid_validity = ?2, last_uid = 0, highest_modseq = NULL
             WHERE id = ?1",
            params![mailbox_id, uid_validity],
        )?;
        Ok(())
    }

    pub fn update_state(
        &self,
        mailbox_id: i64,
        last_uid: Uid,
        highest_modseq: Option<u64>,
    ) -> Result<(), Error> {
        self.0.execute(
            "UPDATE mailboxes SET last_uid = ?2, highest_modseq = ?3 WHERE id = ?1",
            params![mailbox_id, last_uid, highest_modseq.map(|m| m as i64)],
        )?;
        Ok(())
    }

    pub fn upsert_envelopes(
        &mut self,
        mailbox_id: i64,
        envelopes: &[Envelope],
    ) -> Result<(), Error> {
        let tx = self.0.transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT OR REPLACE INTO envelopes
                 (mailbox_id, uid, subject, sender, date_epoch, seen)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            for envelope in envelopes {
                stmt.execute(params![
                    mailbox_id,
                    envelope.uid,
                    envelope.subject,
                    envelope.sender,
                    envelope.date.map(|d| d.timestamp()),
                    envelope.seen,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Supprime les enveloppes absentes du serveur ; retourne leur nombre.
    pub fn remove_absent(
        &mut self,
        mailbox_id: i64,
        present: &HashSet<Uid>,
    ) -> Result<usize, Error> {
        let local: Vec<Uid> = self
            .0
            .prepare("SELECT uid FROM envelopes WHERE mailbox_id = ?1")?
            .query_map([mailbox_id], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        let stale: Vec<Uid> = local
            .into_iter()
            .filter(|uid| !present.contains(uid))
            .collect();
        let tx = self.0.transaction()?;
        {
            let mut envelopes =
                tx.prepare("DELETE FROM envelopes WHERE mailbox_id = ?1 AND uid = ?2")?;
            let mut bodies = tx.prepare("DELETE FROM bodies WHERE mailbox_id = ?1 AND uid = ?2")?;
            let mut actions =
                tx.prepare("DELETE FROM pending_actions WHERE mailbox_id = ?1 AND uid = ?2")?;
            for uid in &stale {
                envelopes.execute(params![mailbox_id, uid])?;
                bodies.execute(params![mailbox_id, uid])?;
                actions.execute(params![mailbox_id, uid])?;
            }
        }
        tx.commit()?;
        Ok(stale.len())
    }

    /// Applique localement un changement lu/non-lu (optimisme UI).
    /// Retourne `false` si l'enveloppe était déjà dans cet état.
    pub fn set_seen_local(&self, mailbox_id: i64, uid: Uid, seen: bool) -> Result<bool, Error> {
        let changed = self.0.execute(
            "UPDATE envelopes SET seen = ?3
             WHERE mailbox_id = ?1 AND uid = ?2 AND seen != ?3",
            params![mailbox_id, uid, seen],
        )?;
        Ok(changed > 0)
    }

    /// Journalise une intention à rejouer vers le serveur.
    pub fn enqueue_action(&self, mailbox_id: i64, uid: Uid, action: Action) -> Result<(), Error> {
        self.0.execute(
            "INSERT INTO pending_actions (mailbox_id, uid, kind) VALUES (?1, ?2, ?3)",
            params![mailbox_id, uid, action.as_str()],
        )?;
        Ok(())
    }

    /// La file d'actions, dans l'ordre d'émission.
    pub fn pending_actions(&self, mailbox_id: i64) -> Result<Vec<PendingAction>, Error> {
        let mut stmt = self.0.prepare(
            "SELECT id, uid, kind FROM pending_actions WHERE mailbox_id = ?1 ORDER BY id",
        )?;
        let rows = stmt
            .query_map([mailbox_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get(1)?, row.get::<_, String>(2)?))
            })?
            .collect::<Result<Vec<(i64, Uid, String)>, _>>()?;
        rows.into_iter()
            .map(|(id, uid, kind)| {
                let action = Action::parse(&kind)
                    .ok_or_else(|| Error::Corrupt(format!("action inconnue : {kind}")))?;
                Ok(PendingAction { id, uid, action })
            })
            .collect()
    }

    pub fn remove_action(&self, action_id: i64) -> Result<(), Error> {
        self.0
            .execute("DELETE FROM pending_actions WHERE id = ?1", [action_id])?;
        Ok(())
    }

    /// Corps HTML brut (pré-assainissement) d'un message, s'il est en cache.
    pub fn body(&self, mailbox: &str, uid: Uid) -> Result<Option<String>, Error> {
        let body = self
            .0
            .query_row(
                "SELECT b.html FROM bodies b JOIN mailboxes m ON m.id = b.mailbox_id
                 WHERE m.name = ?1 AND b.uid = ?2",
                params![mailbox, uid],
                |row| row.get(0),
            )
            .optional()?;
        Ok(body)
    }

    pub fn save_body(&self, mailbox_id: i64, uid: Uid, html: &str) -> Result<(), Error> {
        self.0.execute(
            "INSERT OR REPLACE INTO bodies (mailbox_id, uid, html) VALUES (?1, ?2, ?3)",
            params![mailbox_id, uid, html],
        )?;
        Ok(())
    }

    /// Une page d'enveloppes, les plus récentes d'abord (date, puis UID en
    /// repli). `offset` permet la virtualisation de la liste côté UI.
    pub fn recent(
        &self,
        mailbox: &str,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<Envelope>, Error> {
        let mut stmt = self.0.prepare(
            "SELECT e.uid, e.subject, e.sender, e.date_epoch, e.seen
             FROM envelopes e JOIN mailboxes m ON m.id = e.mailbox_id
             WHERE m.name = ?1
             ORDER BY e.date_epoch DESC, e.uid DESC
             LIMIT ?2 OFFSET ?3",
        )?;
        let rows = stmt
            .query_map(params![mailbox, limit as i64, offset as i64], |row| {
                Ok(Envelope {
                    uid: row.get(0)?,
                    subject: row.get(1)?,
                    sender: row.get(2)?,
                    date: row
                        .get::<_, Option<i64>>(3)?
                        .and_then(|epoch| DateTime::from_timestamp(epoch, 0)),
                    seen: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn count(&self, mailbox_id: i64) -> Result<u64, Error> {
        let count: i64 = self.0.query_row(
            "SELECT COUNT(*) FROM envelopes WHERE mailbox_id = ?1",
            [mailbox_id],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }

    pub fn max_uid(&self, mailbox_id: i64) -> Result<Uid, Error> {
        let max: Uid = self.0.query_row(
            "SELECT COALESCE(MAX(uid), 0) FROM envelopes WHERE mailbox_id = ?1",
            [mailbox_id],
            |row| row.get(0),
        )?;
        Ok(max)
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;

    fn envelope(uid: Uid, subject: &str, epoch: i64, seen: bool) -> Envelope {
        Envelope {
            uid,
            subject: Some(subject.to_string()),
            sender: Some("alice@example.com".to_string()),
            date: Some(Utc.timestamp_opt(epoch, 0).unwrap()),
            seen,
        }
    }

    fn store_with_mailbox() -> (Store, i64) {
        let store = Store::open_in_memory().unwrap();
        let id = store.create_mailbox("INBOX", 1).unwrap();
        (store, id)
    }

    #[test]
    fn roundtrips_all_envelope_fields() {
        let (mut store, id) = store_with_mailbox();
        let original = envelope(7, "Sujet accentué : été", 1_700_000_000, true);
        store
            .upsert_envelopes(id, std::slice::from_ref(&original))
            .unwrap();
        assert_eq!(store.recent("INBOX", 0, 10).unwrap(), vec![original]);
    }

    #[test]
    fn roundtrips_envelope_without_optional_fields() {
        let (mut store, id) = store_with_mailbox();
        let bare = Envelope {
            uid: 1,
            subject: None,
            sender: None,
            date: None,
            seen: false,
        };
        store
            .upsert_envelopes(id, std::slice::from_ref(&bare))
            .unwrap();
        assert_eq!(store.recent("INBOX", 0, 10).unwrap(), vec![bare]);
    }

    #[test]
    fn upsert_replaces_existing_envelope() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(id, &[envelope(1, "avant", 100, false)])
            .unwrap();
        store
            .upsert_envelopes(id, &[envelope(1, "après", 100, true)])
            .unwrap();
        let rows = store.recent("INBOX", 0, 10).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].subject.as_deref(), Some("après"));
        assert!(rows[0].seen);
    }

    #[test]
    fn recent_orders_by_date_then_uid_descending() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "ancien", 100, false),
                    envelope(3, "récent", 300, false),
                    envelope(2, "milieu", 200, false),
                ],
            )
            .unwrap();
        let uids: Vec<Uid> = store
            .recent("INBOX", 0, 2)
            .unwrap()
            .iter()
            .map(|e| e.uid)
            .collect();
        assert_eq!(uids, vec![3, 2]);
    }

    #[test]
    fn remove_absent_deletes_only_missing_uids() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "a", 100, false),
                    envelope(2, "b", 200, false),
                    envelope(3, "c", 300, false),
                ],
            )
            .unwrap();
        let present: HashSet<Uid> = [1, 3].into_iter().collect();
        assert_eq!(store.remove_absent(id, &present).unwrap(), 1);
        assert_eq!(store.count(id).unwrap(), 2);
    }

    #[test]
    fn sync_state_roundtrips_including_modseq() {
        let (store, id) = store_with_mailbox();
        assert_eq!(
            store.sync_state("INBOX").unwrap(),
            Some(SyncState {
                mailbox_id: id,
                uid_validity: 1,
                last_uid: 0,
                highest_modseq: None,
            })
        );
        store.update_state(id, 42, Some(9000)).unwrap();
        let state = store.sync_state("INBOX").unwrap().unwrap();
        assert_eq!(state.last_uid, 42);
        assert_eq!(state.highest_modseq, Some(9000));
    }

    #[test]
    fn sync_state_is_none_for_unknown_mailbox() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(store.sync_state("INBOX").unwrap(), None);
    }

    #[test]
    fn reset_mailbox_clears_envelopes_and_state() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(id, &[envelope(1, "a", 100, false)])
            .unwrap();
        store.update_state(id, 1, Some(5)).unwrap();
        store.reset_mailbox(id, 2).unwrap();
        assert_eq!(store.count(id).unwrap(), 0);
        let state = store.sync_state("INBOX").unwrap().unwrap();
        assert_eq!(state.uid_validity, 2);
        assert_eq!(state.last_uid, 0);
        assert_eq!(state.highest_modseq, None);
    }

    #[test]
    fn max_uid_is_zero_for_empty_mailbox() {
        let (store, id) = store_with_mailbox();
        assert_eq!(store.max_uid(id).unwrap(), 0);
    }

    #[test]
    fn recent_pages_with_offset() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &(1..=5)
                    .map(|uid| envelope(uid, "sujet", 100 * i64::from(uid), false))
                    .collect::<Vec<_>>(),
            )
            .unwrap();
        let page: Vec<Uid> = store
            .recent("INBOX", 2, 2)
            .unwrap()
            .iter()
            .map(|e| e.uid)
            .collect();
        assert_eq!(page, vec![3, 2], "offset 2 saute les deux plus récents");
        assert!(store.recent("INBOX", 10, 5).unwrap().is_empty());
    }

    #[test]
    fn action_queue_roundtrips_in_emission_order() {
        let (store, id) = store_with_mailbox();
        store.enqueue_action(id, 5, Action::MarkSeen).unwrap();
        store.enqueue_action(id, 3, Action::MarkUnseen).unwrap();

        let queued = store.pending_actions(id).unwrap();
        assert_eq!(queued.len(), 2);
        assert_eq!((queued[0].uid, queued[0].action), (5, Action::MarkSeen));
        assert_eq!((queued[1].uid, queued[1].action), (3, Action::MarkUnseen));

        store.remove_action(queued[0].id).unwrap();
        assert_eq!(store.pending_actions(id).unwrap().len(), 1);
    }

    #[test]
    fn set_seen_local_updates_and_reports_actual_change() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(id, &[envelope(1, "a", 100, false)])
            .unwrap();

        assert!(store.set_seen_local(id, 1, true).unwrap());
        assert!(store.recent("INBOX", 0, 1).unwrap()[0].seen);
        assert!(
            !store.set_seen_local(id, 1, true).unwrap(),
            "déjà lu : aucun changement à journaliser"
        );
    }

    #[test]
    fn reset_mailbox_clears_pending_actions() {
        let (store, id) = store_with_mailbox();
        store.enqueue_action(id, 1, Action::MarkSeen).unwrap();
        store.reset_mailbox(id, 2).unwrap();
        assert!(store.pending_actions(id).unwrap().is_empty());
    }

    #[test]
    fn body_roundtrips_and_is_none_when_absent() {
        let (store, id) = store_with_mailbox();
        assert_eq!(store.body("INBOX", 1).unwrap(), None);
        store.save_body(id, 1, "<p>bonjour</p>").unwrap();
        assert_eq!(
            store.body("INBOX", 1).unwrap().as_deref(),
            Some("<p>bonjour</p>")
        );
    }

    #[test]
    fn reset_mailbox_clears_bodies_too() {
        let (store, id) = store_with_mailbox();
        store.save_body(id, 1, "<p>x</p>").unwrap();
        store.reset_mailbox(id, 2).unwrap();
        assert_eq!(store.body("INBOX", 1).unwrap(), None);
    }

    #[test]
    fn remove_absent_drops_orphaned_bodies() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(id, &[envelope(1, "a", 100, false)])
            .unwrap();
        store.save_body(id, 1, "<p>x</p>").unwrap();
        assert_eq!(store.remove_absent(id, &HashSet::new()).unwrap(), 1);
        assert_eq!(store.body("INBOX", 1).unwrap(), None);
    }
}
