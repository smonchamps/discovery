//! Stockage local SQLite : enveloppes + état de synchro (UIDVALIDITY, dernier UID).
//! Une seule boîte (INBOX) pour le spike ; la généralisation viendra en Phase 1.

use std::collections::HashSet;
use std::path::Path;

use anyhow::Context;
use rusqlite::{Connection, OptionalExtension, params};

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS sync_state (
    mailbox     TEXT PRIMARY KEY,
    uidvalidity INTEGER NOT NULL,
    last_uid    INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS envelopes (
    uid     INTEGER PRIMARY KEY,
    subject TEXT NOT NULL,
    sender  TEXT NOT NULL,
    date    TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_envelopes_date ON envelopes(date DESC);
";

pub struct Store(Connection);

pub struct EnvelopeRow {
    pub uid: u32,
    pub subject: String,
    pub sender: String,
    pub date: String,
}

pub struct SyncState {
    pub uidvalidity: u32,
    pub last_uid: u32,
}

impl Store {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).context("création du dossier de la base")?;
        }
        let conn =
            Connection::open(path).with_context(|| format!("ouverture de {}", path.display()))?;
        conn.execute_batch(SCHEMA).context("création du schéma")?;
        Ok(Self(conn))
    }

    pub fn state(&self) -> anyhow::Result<Option<SyncState>> {
        self.0
            .query_row(
                "SELECT uidvalidity, last_uid FROM sync_state WHERE mailbox = 'INBOX'",
                [],
                |row| {
                    Ok(SyncState {
                        uidvalidity: row.get(0)?,
                        last_uid: row.get(1)?,
                    })
                },
            )
            .optional()
            .context("lecture de l'état de synchro")
    }

    pub fn set_state(&self, uidvalidity: u32, last_uid: u32) -> anyhow::Result<()> {
        self.0.execute(
            "INSERT INTO sync_state (mailbox, uidvalidity, last_uid) VALUES ('INBOX', ?1, ?2)
             ON CONFLICT(mailbox) DO UPDATE SET uidvalidity = ?1, last_uid = ?2",
            params![uidvalidity, last_uid],
        )?;
        Ok(())
    }

    pub fn reset(&self) -> anyhow::Result<()> {
        self.0
            .execute_batch("DELETE FROM envelopes; DELETE FROM sync_state;")?;
        Ok(())
    }

    pub fn insert(&mut self, rows: &[EnvelopeRow]) -> anyhow::Result<()> {
        let tx = self.0.transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT OR REPLACE INTO envelopes (uid, subject, sender, date)
                 VALUES (?1, ?2, ?3, ?4)",
            )?;
            for row in rows {
                stmt.execute(params![row.uid, row.subject, row.sender, row.date])?;
            }
        }
        tx.commit().context("insertion des enveloppes")
    }

    /// Supprime les enveloppes locales absentes du serveur ; retourne leur nombre.
    pub fn retain(&mut self, server_uids: &HashSet<u32>) -> anyhow::Result<usize> {
        let local: Vec<u32> = self
            .0
            .prepare("SELECT uid FROM envelopes")?
            .query_map([], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        let stale: Vec<u32> = local
            .into_iter()
            .filter(|uid| !server_uids.contains(uid))
            .collect();
        let tx = self.0.transaction()?;
        {
            let mut stmt = tx.prepare("DELETE FROM envelopes WHERE uid = ?1")?;
            for uid in &stale {
                stmt.execute([*uid])?;
            }
        }
        tx.commit()
            .context("suppression des enveloppes disparues")?;
        Ok(stale.len())
    }

    pub fn recent(&self, limit: usize) -> anyhow::Result<Vec<EnvelopeRow>> {
        let mut stmt = self.0.prepare(
            "SELECT uid, subject, sender, date FROM envelopes ORDER BY date DESC LIMIT ?1",
        )?;
        let rows = stmt
            .query_map([limit as i64], |row| {
                Ok(EnvelopeRow {
                    uid: row.get(0)?,
                    subject: row.get(1)?,
                    sender: row.get(2)?,
                    date: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn count(&self) -> anyhow::Result<i64> {
        self.0
            .query_row("SELECT COUNT(*) FROM envelopes", [], |row| row.get(0))
            .context("comptage des enveloppes")
    }

    pub fn max_uid(&self) -> anyhow::Result<u32> {
        self.0
            .query_row("SELECT COALESCE(MAX(uid), 0) FROM envelopes", [], |row| {
                row.get(0)
            })
            .context("lecture du dernier UID")
    }
}
