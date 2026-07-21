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
use crate::attachment::Attachment;
use crate::envelope::{Envelope, Uid};
use crate::error::Error;
use crate::search;

const SCHEMA: &str = "
PRAGMA foreign_keys = ON;
CREATE TABLE IF NOT EXISTS accounts (
    id       INTEGER PRIMARY KEY,
    email    TEXT NOT NULL UNIQUE,
    provider TEXT NOT NULL DEFAULT 'gmail'
);
CREATE TABLE IF NOT EXISTS mailboxes (
    id             INTEGER PRIMARY KEY,
    account_id     INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    name           TEXT NOT NULL,
    uid_validity   INTEGER NOT NULL,
    last_uid       INTEGER NOT NULL DEFAULT 0,
    highest_modseq INTEGER,
    UNIQUE (account_id, name)
);
CREATE TABLE IF NOT EXISTS envelopes (
    mailbox_id     INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    uid            INTEGER NOT NULL,
    subject        TEXT,
    sender         TEXT,
    sender_address TEXT,
    message_id     TEXT,
    date_epoch     INTEGER,
    seen           INTEGER NOT NULL DEFAULT 0,
    flagged        INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (mailbox_id, uid)
);
CREATE INDEX IF NOT EXISTS idx_envelopes_date ON envelopes(mailbox_id, date_epoch DESC);
CREATE TABLE IF NOT EXISTS bodies (
    mailbox_id INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    uid        INTEGER NOT NULL,
    html       TEXT NOT NULL,
    -- 0 = corps rapatrie AVANT que les pieces jointes existent : son MIME
    -- n'a jamais ete inspecte, et l'information n'est PAS recuperable
    -- depuis le HTML stocke. Il faut le relire (voir bodies_to_backfill).
    scanned    INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (mailbox_id, uid)
);
-- Métadonnées seules : jamais les octets. Ils se retéléchargent à la
-- demande (ADR 0007 — le budget disque ne survivrait pas aux fichiers).
CREATE TABLE IF NOT EXISTS attachments (
    mailbox_id INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    uid        INTEGER NOT NULL,
    idx        INTEGER NOT NULL,
    name       TEXT NOT NULL,
    mime       TEXT NOT NULL,
    size       INTEGER NOT NULL,
    PRIMARY KEY (mailbox_id, uid, idx)
);
CREATE TABLE IF NOT EXISTS pending_actions (
    id         INTEGER PRIMARY KEY,
    mailbox_id INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
    uid        INTEGER NOT NULL,
    kind       TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS drafts (
    id            INTEGER PRIMARY KEY,
    account_id    INTEGER NOT NULL DEFAULT 1,
    to_raw        TEXT NOT NULL,
    subject       TEXT NOT NULL,
    body          TEXT NOT NULL,
    reply_to_uid  INTEGER,
    updated_epoch INTEGER NOT NULL,
    remote_uid    INTEGER,
    pushed_epoch  INTEGER
);
CREATE TABLE IF NOT EXISTS draft_tombstones (
    account_id INTEGER NOT NULL,
    remote_uid INTEGER NOT NULL,
    PRIMARY KEY (account_id, remote_uid)
);
CREATE TABLE IF NOT EXISTS drafts_remote (
    account_id   INTEGER PRIMARY KEY,
    uid_validity INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS outbox (
    id           INTEGER PRIMARY KEY,
    account_id   INTEGER NOT NULL DEFAULT 1,
    message_id   TEXT NOT NULL,
    sender       TEXT NOT NULL,
    recipients   TEXT NOT NULL,
    subject      TEXT NOT NULL,
    body_text    TEXT NOT NULL,
    in_reply_to  TEXT,
    state        TEXT NOT NULL DEFAULT 'queued',
    attempts     INTEGER NOT NULL DEFAULT 0,
    last_error   TEXT,
    queued_epoch INTEGER NOT NULL
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

/// Un compte connecté au client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    pub id: i64,
    pub email: String,
    pub provider: String,
}

/// Une ligne de la boîte unifiée : l'enveloppe ET son compte — un UID
/// seul n'identifie plus un message en multi-comptes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnifiedRow {
    pub account_id: i64,
    pub account_email: String,
    pub envelope: Envelope,
    /// Le message porte-t-il au moins une piece jointe ?
    ///
    /// Faux tant que son corps n'a pas ete lu — meme condition que la
    /// recherche dans le texte. Le trombone apparait donc au fil du
    /// rattrapage, jamais a tort.
    pub has_attachment: bool,
}

/// Colonnes du SELECT unifié, partagées par [`Store::unified_recent`] et
/// [`Store::search`] — l'ordre est celui de [`row_to_unified`].
/// La derniere colonne est un EXISTS sur `attachments` : la liste doit
/// pouvoir afficher le trombone sans une requete par ligne. La cle
/// primaire (mailbox_id, uid, idx) rend ce test indexe.
pub(crate) const SELECT_UNIFIED: &str = "SELECT a.id, a.email, e.uid, e.subject, e.sender, e.sender_address, e.message_id, e.date_epoch, e.seen, e.flagged, EXISTS(SELECT 1 FROM attachments att WHERE att.mailbox_id = e.mailbox_id AND att.uid = e.uid)";

pub struct Store(Connection);

impl Store {
    /// Accès réservé aux modules du crate qui étendent le stockage
    /// (boîte d'envoi, dans `outbox.rs`) sans grossir ce fichier.
    pub(crate) fn conn(&self) -> &Connection {
        &self.0
    }

    pub fn open(path: &Path) -> Result<Self, Error> {
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self, Error> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self, Error> {
        // Plusieurs commandes ouvrent chacune leur connexion : patienter
        // plutôt que d'échouer en SQLITE_BUSY sur une écriture concurrente.
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.execute_batch(SCHEMA)?;
        migrate(&conn)?;
        Ok(Self(conn))
    }

    pub fn sync_state(&self, account_id: i64, mailbox: &str) -> Result<Option<SyncState>, Error> {
        let state = self
            .0
            .query_row(
                "SELECT id, uid_validity, last_uid, highest_modseq
                 FROM mailboxes WHERE account_id = ?1 AND name = ?2",
                params![account_id, mailbox],
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

    pub fn create_mailbox(
        &self,
        account_id: i64,
        mailbox: &str,
        uid_validity: u32,
    ) -> Result<i64, Error> {
        self.0.execute(
            "INSERT INTO mailboxes (account_id, name, uid_validity) VALUES (?1, ?2, ?3)",
            params![account_id, mailbox, uid_validity],
        )?;
        Ok(self.0.last_insert_rowid())
    }

    /// Enregistre un compte, ou revendique le compte « en attente
    /// d'adoption » créé par la migration Phase 2 → 3 (email vide) : la
    /// première connexion après la mise à jour est, en pratique, le même
    /// compte Gmail qu'avant — ses données l'attendent.
    pub fn adopt_or_create_account(&self, email: &str, provider: &str) -> Result<i64, Error> {
        if let Some(id) = self.account_id(email)? {
            return Ok(id);
        }
        let claimed = self.0.execute(
            "UPDATE accounts SET email = ?1, provider = ?2
             WHERE email = '' AND id = (SELECT MIN(id) FROM accounts WHERE email = '')",
            params![email, provider],
        )?;
        if claimed == 0 {
            self.0.execute(
                "INSERT INTO accounts (email, provider) VALUES (?1, ?2)",
                params![email, provider],
            )?;
            return Ok(self.0.last_insert_rowid());
        }
        self.account_id(email)?
            .ok_or_else(|| Error::Corrupt("compte revendiqué introuvable".to_string()))
    }

    fn account_id(&self, email: &str) -> Result<Option<i64>, Error> {
        let id = self
            .0
            .query_row("SELECT id FROM accounts WHERE email = ?1", [email], |row| {
                row.get(0)
            })
            .optional()?;
        Ok(id)
    }

    /// Les comptes connus — sans l'éventuel compte en attente d'adoption.
    pub fn accounts(&self) -> Result<Vec<Account>, Error> {
        let mut stmt = self
            .0
            .prepare("SELECT id, email, provider FROM accounts WHERE email != '' ORDER BY id")?;
        let rows = stmt
            .query_map([], |row| {
                Ok(Account {
                    id: row.get(0)?,
                    email: row.get(1)?,
                    provider: row.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Configuration serveur d'un compte (Gmail ou IMAP générique).
    pub fn account_config(&self, account_id: i64) -> Result<AccountConfig, Error> {
        let config = self
            .0
            .query_row(
                "SELECT imap_host, imap_port, smtp_host, smtp_port, username
                 FROM accounts WHERE id = ?1",
                [account_id],
                |row| {
                    Ok(AccountConfig {
                        imap_host: row.get(0)?,
                        imap_port: row.get(1)?,
                        smtp_host: row.get(2)?,
                        smtp_port: row.get(3)?,
                        username: row.get(4)?,
                    })
                },
            )
            .optional()?
            .unwrap_or(AccountConfig {
                imap_host: None,
                imap_port: None,
                smtp_host: None,
                smtp_port: None,
                username: None,
            });
        Ok(config)
    }

    /// Crée ou met à jour un compte IMAP/SMTP générique.
    pub fn create_generic_account(
        &self,
        email: &str,
        username: &str,
        imap_host: &str,
        imap_port: u16,
        smtp_host: &str,
        smtp_port: u16,
    ) -> Result<i64, Error> {
        self.0.execute(
            "INSERT INTO accounts (email, provider, username, imap_host, imap_port, smtp_host, smtp_port)
             VALUES (?1, 'imap', ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(email) DO UPDATE SET
                provider = 'imap',
                username = excluded.username,
                imap_host = excluded.imap_host,
                imap_port = excluded.imap_port,
                smtp_host = excluded.smtp_host,
                smtp_port = excluded.smtp_port",
            params![
                email,
                username,
                imap_host,
                imap_port,
                smtp_host,
                smtp_port
            ],
        )?;
        // JAMAIS `last_insert_rowid()` : sur le chemin UPDATE (ré-ajout),
        // aucune ligne n'est insérée et il renverrait 0 (ou un id d'une
        // autre écriture de la connexion). L'id fait toujours foi en base.
        self.account_id(email)?.ok_or_else(|| {
            Error::Corrupt("compte générique introuvable après écriture".to_string())
        })
    }

    /// Repart de zéro pour une boîte dont l'UIDVALIDITY a changé : les UIDs
    /// ne veulent plus rien dire — corps et actions en attente compris (une
    /// intention sur un UID invalidé est irréalisable par construction).
    pub fn reset_mailbox(&self, mailbox_id: i64, uid_validity: u32) -> Result<(), Error> {
        search::deindex_mailbox(&self.0, mailbox_id)?;
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
                 (mailbox_id, uid, subject, sender, sender_address, message_id,
                  date_epoch, seen, flagged)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            )?;
            let mut body_stmt =
                tx.prepare("SELECT html FROM bodies WHERE mailbox_id = ?1 AND uid = ?2")?;
            for envelope in envelopes {
                stmt.execute(params![
                    mailbox_id,
                    envelope.uid,
                    envelope.subject,
                    envelope.sender,
                    envelope.sender_address,
                    envelope.message_id,
                    envelope.date.map(|d| d.timestamp()),
                    envelope.seen,
                    envelope.flagged,
                ])?;
                let html: Option<String> = body_stmt
                    .query_row(params![mailbox_id, envelope.uid], |row| row.get(0))
                    .optional()?;
                search::index_message(
                    &tx,
                    mailbox_id,
                    envelope.uid,
                    envelope.subject.as_deref(),
                    envelope.sender.as_deref(),
                    envelope.sender_address.as_deref(),
                    html.as_deref(),
                )?;
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
                search::deindex_message(&tx, mailbox_id, *uid)?;
                envelopes.execute(params![mailbox_id, uid])?;
                bodies.execute(params![mailbox_id, uid])?;
                actions.execute(params![mailbox_id, uid])?;
            }
        }
        tx.commit()?;
        Ok(stale.len())
    }

    /// Retire localement une enveloppe et son corps (archivage/suppression
    /// optimiste) ; le serveur suivra via la file d'actions.
    pub fn remove_local(&self, mailbox_id: i64, uid: Uid) -> Result<(), Error> {
        search::deindex_message(&self.0, mailbox_id, uid)?;
        self.0.execute(
            "DELETE FROM bodies WHERE mailbox_id = ?1 AND uid = ?2",
            params![mailbox_id, uid],
        )?;
        self.0.execute(
            "DELETE FROM envelopes WHERE mailbox_id = ?1 AND uid = ?2",
            params![mailbox_id, uid],
        )?;
        Ok(())
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

    /// Applique localement l'étoile (optimisme UI).
    /// Retourne `false` si l'enveloppe était déjà dans cet état.
    pub fn set_flagged_local(
        &self,
        mailbox_id: i64,
        uid: Uid,
        flagged: bool,
    ) -> Result<bool, Error> {
        let changed = self.0.execute(
            "UPDATE envelopes SET flagged = ?3
             WHERE mailbox_id = ?1 AND uid = ?2 AND flagged != ?3",
            params![mailbox_id, uid, flagged],
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
    pub fn body(&self, account_id: i64, mailbox: &str, uid: Uid) -> Result<Option<String>, Error> {
        let body = self
            .0
            .query_row(
                "SELECT b.html FROM bodies b JOIN mailboxes m ON m.id = b.mailbox_id
                 WHERE m.account_id = ?1 AND m.name = ?2 AND b.uid = ?3",
                params![account_id, mailbox, uid],
                |row| row.get(0),
            )
            .optional()?;
        Ok(body)
    }

    /// Enregistre un corps, son index de recherche et la description de
    /// ses pièces jointes — **dans une seule transaction**.
    ///
    /// Les trois se lisent dans les mêmes octets et n'ont de sens
    /// qu'ensemble : un corps sans son index sortirait des recherches, un
    /// corps sans ses pièces jointes les rendrait invisibles jusqu'au
    /// prochain re-téléchargement. Un crash entre deux écritures ne doit
    /// jamais pouvoir produire cet état.
    pub fn save_body(
        &self,
        mailbox_id: i64,
        uid: Uid,
        html: &str,
        attachments: &[Attachment],
    ) -> Result<(), Error> {
        let tx = self.0.unchecked_transaction()?;
        tx.execute(
            "INSERT OR REPLACE INTO bodies (mailbox_id, uid, html, scanned)
             VALUES (?1, ?2, ?3, 1)",
            params![mailbox_id, uid, html],
        )?;
        // Remplacement intégral : un message re-téléchargé dont une pièce
        // aurait disparu ne doit pas garder l'ancienne ligne fantôme.
        tx.execute(
            "DELETE FROM attachments WHERE mailbox_id = ?1 AND uid = ?2",
            params![mailbox_id, uid],
        )?;
        for attachment in attachments {
            tx.execute(
                "INSERT INTO attachments (mailbox_id, uid, idx, name, mime, size)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    mailbox_id,
                    uid,
                    attachment.index as i64,
                    attachment.name,
                    attachment.mime,
                    attachment.size as i64
                ],
            )?;
        }
        if let Some((subject, sender, sender_address)) = tx
            .query_row(
                "SELECT subject, sender, sender_address
                 FROM envelopes WHERE mailbox_id = ?1 AND uid = ?2",
                params![mailbox_id, uid],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .optional()?
        {
            search::index_message(
                &tx,
                mailbox_id,
                uid,
                subject.as_deref(),
                sender.as_deref(),
                sender_address.as_deref(),
                Some(html),
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Les pièces jointes connues d'un message, dans l'ordre du MIME.
    ///
    /// Vide tant que le corps n'a pas été rapatrié : c'est la même
    /// condition que la recherche dans le texte, et le rattrapage la
    /// lève pour tout l'horizon de récence.
    pub fn attachments(
        &self,
        account_id: i64,
        mailbox: &str,
        uid: Uid,
    ) -> Result<Vec<Attachment>, Error> {
        let mut statement = self.0.prepare(
            "SELECT a.idx, a.name, a.mime, a.size
             FROM attachments a
             JOIN mailboxes m ON m.id = a.mailbox_id
             WHERE m.account_id = ?1 AND m.name = ?2 AND a.uid = ?3
             ORDER BY a.idx",
        )?;
        let rows = statement.query_map(params![account_id, mailbox, uid], |row| {
            Ok(Attachment {
                index: row.get::<_, i64>(0)? as usize,
                name: row.get(1)?,
                mime: row.get(2)?,
                size: row.get::<_, i64>(3)? as u64,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Les messages RÉCENTS dont le corps manque encore, du plus récent au
    /// plus ancien — le travail du rattrapage ([ADR 0007](../../../docs/adr/0007-rattrapage-des-corps.md)).
    ///
    /// `since_epoch` borne le coût : c'est l'horizon de récence. L'ordre
    /// décroissant rend la reprise après coupure naturelle — on redemande
    /// simplement la liste, les corps déjà écrits n'y sont plus.
    ///
    /// Un message sans date est ignoré : il n'est pas situable dans
    /// l'horizon, et l'inclure ouvrirait la porte à du courrier
    /// arbitrairement ancien — ce que l'horizon existe précisément pour
    /// empêcher. En pratique l'INTERNALDATE d'IMAP est toujours présent.
    pub fn bodies_to_backfill(
        &self,
        account_id: i64,
        mailbox: &str,
        since_epoch: i64,
        limit: usize,
    ) -> Result<Vec<Uid>, Error> {
        let mut stmt = self.0.prepare(
            "SELECT e.uid
             FROM envelopes e
             JOIN mailboxes m ON m.id = e.mailbox_id
             WHERE m.account_id = ?1 AND m.name = ?2
               AND e.date_epoch IS NOT NULL AND e.date_epoch >= ?3
               AND NOT EXISTS (
                   SELECT 1 FROM bodies b
                    WHERE b.mailbox_id = e.mailbox_id AND b.uid = e.uid
                      AND b.scanned = 1
               )
             ORDER BY e.date_epoch DESC, e.uid DESC
             LIMIT ?4",
        )?;
        let uids = stmt
            .query_map(
                params![account_id, mailbox, since_epoch, limit as i64],
                |row| row.get(0),
            )?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(uids)
    }

    /// Combien de messages attendent encore leur corps dans l'horizon —
    /// de quoi afficher un avancement honnête.
    pub fn bodies_pending_count(
        &self,
        account_id: i64,
        mailbox: &str,
        since_epoch: i64,
    ) -> Result<u64, Error> {
        let count: i64 = self.0.query_row(
            "SELECT COUNT(*)
             FROM envelopes e
             JOIN mailboxes m ON m.id = e.mailbox_id
             WHERE m.account_id = ?1 AND m.name = ?2
               AND e.date_epoch IS NOT NULL AND e.date_epoch >= ?3
               AND NOT EXISTS (
                   SELECT 1 FROM bodies b
                    WHERE b.mailbox_id = e.mailbox_id AND b.uid = e.uid
                      AND b.scanned = 1
               )",
            params![account_id, mailbox, since_epoch],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }

    /// Une page d'enveloppes d'UN compte, les plus récentes d'abord.
    pub fn recent(
        &self,
        account_id: i64,
        mailbox: &str,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<Envelope>, Error> {
        let mut stmt = self.0.prepare(
            "SELECT e.uid, e.subject, e.sender, e.sender_address, e.message_id,
                    e.date_epoch, e.seen, e.flagged
             FROM envelopes e JOIN mailboxes m ON m.id = e.mailbox_id
             WHERE m.account_id = ?1 AND m.name = ?2
             ORDER BY e.date_epoch DESC, e.uid DESC
             LIMIT ?3 OFFSET ?4",
        )?;
        let rows = stmt
            .query_map(
                params![account_id, mailbox, limit as i64, offset as i64],
                row_to_envelope,
            )?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// La boîte unifiée : la même boîte (INBOX) de TOUS les comptes,
    /// fusionnée par date — le cœur produit du multi-comptes. Chaque
    /// ligne porte son compte : un UID seul n'identifie plus un message.
    pub fn unified_recent(
        &self,
        mailbox: &str,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<UnifiedRow>, Error> {
        let mut stmt = self.0.prepare(&format!(
            "{SELECT_UNIFIED}
             FROM envelopes e
             JOIN mailboxes m ON m.id = e.mailbox_id
             JOIN accounts a ON a.id = m.account_id
             WHERE m.name = ?1
             ORDER BY e.date_epoch DESC, e.uid DESC, a.id
             LIMIT ?2 OFFSET ?3"
        ))?;
        let rows = stmt
            .query_map(
                params![mailbox, limit as i64, offset as i64],
                row_to_unified,
            )?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Total de la boîte unifiée, tous comptes confondus.
    pub fn unified_count(&self, mailbox: &str) -> Result<u64, Error> {
        let count: i64 = self.0.query_row(
            "SELECT COUNT(*) FROM envelopes e JOIN mailboxes m ON m.id = e.mailbox_id
             WHERE m.name = ?1",
            [mailbox],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }

    /// Une enveloppe précise — le contexte nécessaire pour répondre
    /// (adresse brute de l'expéditeur, Message-ID du fil).
    pub fn envelope(
        &self,
        account_id: i64,
        mailbox: &str,
        uid: Uid,
    ) -> Result<Option<Envelope>, Error> {
        let envelope = self
            .0
            .query_row(
                "SELECT e.uid, e.subject, e.sender, e.sender_address, e.message_id,
                        e.date_epoch, e.seen, e.flagged
                 FROM envelopes e JOIN mailboxes m ON m.id = e.mailbox_id
                 WHERE m.account_id = ?1 AND m.name = ?2 AND e.uid = ?3",
                params![account_id, mailbox, uid],
                row_to_envelope,
            )
            .optional()?;
        Ok(envelope)
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

/// Fait évoluer en place une base d'une version précédente : les colonnes
/// s'ajoutent sans perdre ce qui est déjà là, et la bascule multi-comptes
/// (Phase 3) reconstruit les tables dont les contraintes changent.
/// Configuration serveur d'un compte IMAP/SMTP générique.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountConfig {
    pub imap_host: Option<String>,
    pub imap_port: Option<u16>,
    pub smtp_host: Option<String>,
    pub smtp_port: Option<u16>,
    pub username: Option<String>,
}

fn migrate(conn: &Connection) -> Result<(), Error> {
    migrate_multi_account(conn)?;
    add_missing_columns(
        conn,
        "drafts",
        &[("account_id", "INTEGER NOT NULL DEFAULT 1")],
    )?;
    add_missing_columns(
        conn,
        "outbox",
        &[("account_id", "INTEGER NOT NULL DEFAULT 1")],
    )?;
    add_missing_columns(
        conn,
        "envelopes",
        &[
            ("sender_address", "TEXT"),
            ("message_id", "TEXT"),
            ("flagged", "INTEGER NOT NULL DEFAULT 0"),
        ],
    )?;
    add_missing_columns(
        conn,
        "drafts",
        &[("remote_uid", "INTEGER"), ("pushed_epoch", "INTEGER")],
    )?;
    // Les corps deja en base valent 0 : ils datent d'avant les pieces
    // jointes, et le rattrapage devra les relire une fois.
    add_missing_columns(conn, "bodies", &[("scanned", "INTEGER NOT NULL DEFAULT 0")])?;
    add_missing_columns(
        conn,
        "accounts",
        &[
            ("imap_host", "TEXT"),
            ("imap_port", "INTEGER"),
            ("smtp_host", "TEXT"),
            ("smtp_port", "INTEGER"),
            ("username", "TEXT"),
        ],
    )?;
    search::migrate_search(conn)
}

/// Bascule Phase 2 → 3 : les contraintes de trois tables changent
/// (UNIQUE et clés par compte) — SQLite exige une reconstruction. Les
/// données existantes sont adoptées par un compte « en attente » (email
/// vide) que la première connexion revendiquera : en pratique, le même
/// compte Gmail qu'avant la mise à jour. Zéro perte, prouvé par test.
fn migrate_multi_account(conn: &Connection) -> Result<(), Error> {
    if table_columns(conn, "mailboxes")?.contains("account_id") {
        return Ok(());
    }
    conn.execute_batch(
        "PRAGMA foreign_keys = OFF;
         BEGIN;
         INSERT INTO accounts (id, email, provider) VALUES (1, '', 'gmail');

         CREATE TABLE mailboxes_v3 (
             id             INTEGER PRIMARY KEY,
             account_id     INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
             name           TEXT NOT NULL,
             uid_validity   INTEGER NOT NULL,
             last_uid       INTEGER NOT NULL DEFAULT 0,
             highest_modseq INTEGER,
             UNIQUE (account_id, name)
         );
         INSERT INTO mailboxes_v3 (id, account_id, name, uid_validity, last_uid, highest_modseq)
             SELECT id, 1, name, uid_validity, last_uid, highest_modseq FROM mailboxes;
         DROP TABLE mailboxes;
         ALTER TABLE mailboxes_v3 RENAME TO mailboxes;

         CREATE TABLE drafts_remote_v3 (
             account_id   INTEGER PRIMARY KEY,
             uid_validity INTEGER NOT NULL
         );
         INSERT INTO drafts_remote_v3 (account_id, uid_validity)
             SELECT 1, uid_validity FROM drafts_remote;
         DROP TABLE drafts_remote;
         ALTER TABLE drafts_remote_v3 RENAME TO drafts_remote;

         CREATE TABLE draft_tombstones_v3 (
             account_id INTEGER NOT NULL,
             remote_uid INTEGER NOT NULL,
             PRIMARY KEY (account_id, remote_uid)
         );
         INSERT INTO draft_tombstones_v3 (account_id, remote_uid)
             SELECT 1, remote_uid FROM draft_tombstones;
         DROP TABLE draft_tombstones;
         ALTER TABLE draft_tombstones_v3 RENAME TO draft_tombstones;

         COMMIT;
         PRAGMA foreign_keys = ON;",
    )?;
    Ok(())
}

fn table_columns(conn: &Connection, table: &str) -> Result<HashSet<String>, Error> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = stmt
        .query_map([], |row| row.get(1))?
        .collect::<Result<_, _>>()?;
    Ok(columns)
}

fn add_missing_columns(
    conn: &Connection,
    table: &str,
    columns: &[(&str, &str)],
) -> Result<(), Error> {
    let existing = table_columns(conn, table)?;
    for (column, ddl) in columns {
        if !existing.contains(*column) {
            conn.execute(
                &format!("ALTER TABLE {table} ADD COLUMN {column} {ddl}"),
                [],
            )?;
        }
    }
    Ok(())
}

/// Mapping partagé par toutes les lectures d'enveloppes — l'ordre des
/// colonnes est celui des SELECT ci-dessus.
fn row_to_envelope(row: &rusqlite::Row<'_>) -> rusqlite::Result<Envelope> {
    Ok(Envelope {
        uid: row.get(0)?,
        subject: row.get(1)?,
        sender: row.get(2)?,
        sender_address: row.get(3)?,
        message_id: row.get(4)?,
        date: row
            .get::<_, Option<i64>>(5)?
            .and_then(|epoch| DateTime::from_timestamp(epoch, 0)),
        seen: row.get(6)?,
        flagged: row.get(7)?,
    })
}

/// Mapping partagé par les lectures de la boîte unifiée — l'ordre des
/// colonnes est celui de [`SELECT_UNIFIED`].
pub(crate) fn row_to_unified(row: &rusqlite::Row<'_>) -> rusqlite::Result<UnifiedRow> {
    Ok(UnifiedRow {
        account_id: row.get(0)?,
        account_email: row.get(1)?,
        envelope: Envelope {
            uid: row.get(2)?,
            subject: row.get(3)?,
            sender: row.get(4)?,
            sender_address: row.get(5)?,
            message_id: row.get(6)?,
            date: row
                .get::<_, Option<i64>>(7)?
                .and_then(|epoch| DateTime::from_timestamp(epoch, 0)),
            seen: row.get(8)?,
            flagged: row.get(9)?,
        },
        has_attachment: row.get(10)?,
    })
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;

    fn envelope(uid: Uid, subject: &str, epoch: i64, seen: bool) -> Envelope {
        Envelope {
            uid,
            subject: Some(subject.to_string()),
            sender: Some("Alice Martin".to_string()),
            sender_address: Some("alice@example.com".to_string()),
            message_id: Some(format!("<m{uid}@example.com>")),
            date: Some(Utc.timestamp_opt(epoch, 0).unwrap()),
            seen,
            flagged: uid.is_multiple_of(2),
        }
    }

    fn test_account(store: &Store) -> i64 {
        store
            .adopt_or_create_account("test@exemple.fr", "gmail")
            .unwrap()
    }

    fn store_with_mailbox() -> (Store, i64) {
        let store = Store::open_in_memory().unwrap();
        let account = test_account(&store);
        let id = store.create_mailbox(account, "INBOX", 1).unwrap();
        (store, id)
    }

    fn recent(store: &Store, offset: usize, limit: usize) -> Vec<Envelope> {
        store
            .recent(test_account(store), "INBOX", offset, limit)
            .unwrap()
    }

    #[test]
    fn roundtrips_all_envelope_fields() {
        let (mut store, id) = store_with_mailbox();
        let original = envelope(7, "Sujet accentué : été", 1_700_000_000, true);
        store
            .upsert_envelopes(id, std::slice::from_ref(&original))
            .unwrap();
        assert_eq!(recent(&store, 0, 10), vec![original]);
    }

    #[test]
    fn roundtrips_envelope_without_optional_fields() {
        let (mut store, id) = store_with_mailbox();
        let bare = Envelope {
            uid: 1,
            subject: None,
            sender: None,
            sender_address: None,
            message_id: None,
            date: None,
            seen: false,
            flagged: false,
        };
        store
            .upsert_envelopes(id, std::slice::from_ref(&bare))
            .unwrap();
        assert_eq!(recent(&store, 0, 10), vec![bare]);
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
        let rows = recent(&store, 0, 10);
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
        let uids: Vec<Uid> = recent(&store, 0, 2).iter().map(|e| e.uid).collect();
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
            store.sync_state(test_account(&store), "INBOX").unwrap(),
            Some(SyncState {
                mailbox_id: id,
                uid_validity: 1,
                last_uid: 0,
                highest_modseq: None,
            })
        );
        store.update_state(id, 42, Some(9000)).unwrap();
        let state = store
            .sync_state(test_account(&store), "INBOX")
            .unwrap()
            .unwrap();
        assert_eq!(state.last_uid, 42);
        assert_eq!(state.highest_modseq, Some(9000));
    }

    #[test]
    fn sync_state_is_none_for_unknown_mailbox() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(
            store.sync_state(test_account(&store), "INBOX").unwrap(),
            None
        );
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
        let state = store
            .sync_state(test_account(&store), "INBOX")
            .unwrap()
            .unwrap();
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
        let page: Vec<Uid> = recent(&store, 2, 2).iter().map(|e| e.uid).collect();
        assert_eq!(page, vec![3, 2], "offset 2 saute les deux plus récents");
        assert!(recent(&store, 10, 5).is_empty());
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
        assert!(recent(&store, 0, 1)[0].seen);
        assert!(
            !store.set_seen_local(id, 1, true).unwrap(),
            "déjà lu : aucun changement à journaliser"
        );
    }

    #[test]
    fn set_flagged_local_updates_and_reports_actual_change() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(id, &[envelope(1, "a", 100, false)])
            .unwrap();

        assert!(store.set_flagged_local(id, 1, true).unwrap());
        assert!(recent(&store, 0, 1)[0].flagged);
        assert!(
            !store.set_flagged_local(id, 1, true).unwrap(),
            "déjà étoilé : aucun changement à journaliser"
        );
    }

    #[test]
    fn remove_local_drops_envelope_and_body() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(id, &[envelope(1, "a", 100, false)])
            .unwrap();
        store.save_body(id, 1, "<p>x</p>", &[]).unwrap();

        store.remove_local(id, 1).unwrap();

        assert!(recent(&store, 0, 10).is_empty());
        assert_eq!(store.body(test_account(&store), "INBOX", 1).unwrap(), None);
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
        assert_eq!(store.body(test_account(&store), "INBOX", 1).unwrap(), None);
        store.save_body(id, 1, "<p>bonjour</p>", &[]).unwrap();
        assert_eq!(
            store
                .body(test_account(&store), "INBOX", 1)
                .unwrap()
                .as_deref(),
            Some("<p>bonjour</p>")
        );
    }

    fn pdf(index: usize, name: &str) -> Attachment {
        Attachment {
            index,
            name: name.to_string(),
            mime: "application/pdf".to_string(),
            size: 2048,
        }
    }

    /// Le defaut livre : un corps rapatrie AVANT les pieces jointes n'a
    /// jamais eu son MIME inspecte, et l'information n'est pas
    /// recuperable depuis le HTML stocke. Comme le rattrapage ne
    /// selectionnait que les corps ABSENTS, ces messages n'auraient
    /// jamais montre leurs pieces jointes — soit, en pratique, la
    /// totalite d'une boite deja rattrapee.
    #[test]
    fn a_body_fetched_before_attachments_existed_is_queued_for_a_re_read() {
        let (mut store, id) = store_with_mailbox();
        let account = test_account(&store);
        store
            .upsert_envelopes(id, &[envelope(1, "sujet", 100, false)])
            .unwrap();
        store.save_body(id, 1, "<p>corps</p>", &[]).unwrap();

        // Rien a faire : le corps a ete lu par la version courante.
        assert!(
            store
                .bodies_to_backfill(account, "INBOX", 0, 10)
                .unwrap()
                .is_empty()
        );
        assert_eq!(store.bodies_pending_count(account, "INBOX", 0).unwrap(), 0);

        // On simule l'heritage : corps present, MIME jamais inspecte.
        store
            .conn()
            .execute("UPDATE bodies SET scanned = 0", [])
            .unwrap();

        assert_eq!(
            store.bodies_to_backfill(account, "INBOX", 0, 10).unwrap(),
            vec![1],
            "un corps jamais inspecte doit revenir dans le rattrapage"
        );
        assert_eq!(store.bodies_pending_count(account, "INBOX", 0).unwrap(), 1);

        // Le relire le sort definitivement de la file.
        store.save_body(id, 1, "<p>corps</p>", &[]).unwrap();
        assert!(
            store
                .bodies_to_backfill(account, "INBOX", 0, 10)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn attachments_are_saved_with_the_body_and_read_back_in_order() {
        let (store, id) = store_with_mailbox();
        let account = test_account(&store);
        assert!(
            store.attachments(account, "INBOX", 1).unwrap().is_empty(),
            "rien tant que le corps n'est pas rapatrié"
        );

        store
            .save_body(
                id,
                1,
                "<p>ci-joint</p>",
                &[pdf(0, "un.pdf"), pdf(1, "deux.pdf")],
            )
            .unwrap();

        let found = store.attachments(account, "INBOX", 1).unwrap();
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].name, "un.pdf");
        assert_eq!(found[1].name, "deux.pdf");
        assert_eq!(found[1].size, 2048);
    }

    /// Un message re-téléchargé dont une pièce a disparu ne doit pas
    /// garder l'ancienne ligne : l'utilisateur cliquerait sur un fichier
    /// que le serveur ne sert plus, et l'échec n'arriverait qu'au
    /// téléchargement — loin de la cause.
    #[test]
    fn re_saving_replaces_the_attachment_list_instead_of_accumulating() {
        let (store, id) = store_with_mailbox();
        let account = test_account(&store);
        store
            .save_body(id, 1, "<p>x</p>", &[pdf(0, "un.pdf"), pdf(1, "deux.pdf")])
            .unwrap();

        store
            .save_body(id, 1, "<p>x</p>", &[pdf(0, "un.pdf")])
            .unwrap();

        let found = store.attachments(account, "INBOX", 1).unwrap();
        assert_eq!(found.len(), 1, "la pièce disparue doit l'être aussi ici");
        assert_eq!(found[0].name, "un.pdf");
    }

    /// Les pièces jointes appartiennent à un message d'un COMPTE : la
    /// même paire (boîte, uid) chez un autre compte ne doit rien voir.
    #[test]
    fn attachments_never_leak_across_accounts() {
        let (store, id) = store_with_mailbox();
        store
            .save_body(id, 1, "<p>x</p>", &[pdf(0, "prive.pdf")])
            .unwrap();

        let other = store
            .adopt_or_create_account("autre@exemple.fr", "gmail")
            .unwrap();
        store.create_mailbox(other, "INBOX", 1).unwrap();

        assert!(store.attachments(other, "INBOX", 1).unwrap().is_empty());
    }

    #[test]
    fn reset_mailbox_clears_bodies_too() {
        let (store, id) = store_with_mailbox();
        store.save_body(id, 1, "<p>x</p>", &[]).unwrap();
        store.reset_mailbox(id, 2).unwrap();
        assert_eq!(store.body(test_account(&store), "INBOX", 1).unwrap(), None);
    }

    #[test]
    fn envelope_returns_reply_context_fields() {
        let (mut store, id) = store_with_mailbox();
        let original = envelope(7, "sujet", 100, false);
        store
            .upsert_envelopes(id, std::slice::from_ref(&original))
            .unwrap();

        assert_eq!(
            store.envelope(test_account(&store), "INBOX", 7).unwrap(),
            Some(original)
        );
        assert_eq!(
            store.envelope(test_account(&store), "INBOX", 99).unwrap(),
            None
        );
    }

    /// Une base Phase 1 (sans les colonnes de réponse) doit s'ouvrir et
    /// s'enrichir sans perdre les enveloppes déjà synchronisées.
    #[test]
    fn opens_and_migrates_a_phase1_database() {
        let path = std::env::temp_dir().join(format!(
            "discovery-test-migration-{}.db",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE mailboxes (
                    id             INTEGER PRIMARY KEY,
                    name           TEXT NOT NULL UNIQUE,
                    uid_validity   INTEGER NOT NULL,
                    last_uid       INTEGER NOT NULL DEFAULT 0,
                    highest_modseq INTEGER
                );
                CREATE TABLE envelopes (
                    mailbox_id INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
                    uid        INTEGER NOT NULL,
                    subject    TEXT,
                    sender     TEXT,
                    date_epoch INTEGER,
                    seen       INTEGER NOT NULL DEFAULT 0,
                    PRIMARY KEY (mailbox_id, uid)
                );
                INSERT INTO mailboxes (id, name, uid_validity) VALUES (1, 'INBOX', 1);
                INSERT INTO envelopes (mailbox_id, uid, subject, sender, date_epoch, seen)
                VALUES (1, 42, 'hérité de la phase 1', 'Alice', 100, 1);",
            )
            .unwrap();
        }

        let store = Store::open(&path).unwrap();
        let rows = recent(&store, 0, 10);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].uid, 42);
        assert_eq!(rows[0].subject.as_deref(), Some("hérité de la phase 1"));
        assert_eq!(
            rows[0].sender_address, None,
            "colonne ajoutée par migration : valeur inconnue pour l'existant"
        );
        assert!(
            !rows[0].flagged,
            "étoile absente par défaut après migration"
        );

        drop(store);
        let _ = std::fs::remove_file(&path);
    }

    /// Migration Phase 2 → 3 sur une base complète : toutes les données
    /// (enveloppes, corps, actions, brouillons, tombstones, boîte d'envoi)
    /// sont adoptées par le compte en attente — zéro perte, et la première
    /// connexion revendique le tout.
    #[test]
    fn migrates_a_full_phase2_database_and_adopts_everything() {
        let path = std::env::temp_dir().join(format!(
            "discovery-test-migration-p2-{}.db",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE mailboxes (
                    id             INTEGER PRIMARY KEY,
                    name           TEXT NOT NULL UNIQUE,
                    uid_validity   INTEGER NOT NULL,
                    last_uid       INTEGER NOT NULL DEFAULT 0,
                    highest_modseq INTEGER
                );
                CREATE TABLE envelopes (
                    mailbox_id     INTEGER NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
                    uid            INTEGER NOT NULL,
                    subject        TEXT,
                    sender         TEXT,
                    sender_address TEXT,
                    message_id     TEXT,
                    date_epoch     INTEGER,
                    seen           INTEGER NOT NULL DEFAULT 0,
                    flagged        INTEGER NOT NULL DEFAULT 0,
                    PRIMARY KEY (mailbox_id, uid)
                );
                CREATE TABLE bodies (
                    mailbox_id INTEGER NOT NULL,
                    uid        INTEGER NOT NULL,
                    html       TEXT NOT NULL,
                    PRIMARY KEY (mailbox_id, uid)
                );
                CREATE TABLE pending_actions (
                    id INTEGER PRIMARY KEY, mailbox_id INTEGER NOT NULL,
                    uid INTEGER NOT NULL, kind TEXT NOT NULL
                );
                CREATE TABLE drafts (
                    id INTEGER PRIMARY KEY, to_raw TEXT NOT NULL,
                    subject TEXT NOT NULL, body TEXT NOT NULL,
                    reply_to_uid INTEGER, updated_epoch INTEGER NOT NULL,
                    remote_uid INTEGER, pushed_epoch INTEGER
                );
                CREATE TABLE draft_tombstones (remote_uid INTEGER PRIMARY KEY);
                CREATE TABLE drafts_remote (
                    id INTEGER PRIMARY KEY CHECK (id = 1),
                    uid_validity INTEGER NOT NULL
                );
                CREATE TABLE outbox (
                    id INTEGER PRIMARY KEY, message_id TEXT NOT NULL,
                    sender TEXT NOT NULL, recipients TEXT NOT NULL,
                    subject TEXT NOT NULL, body_text TEXT NOT NULL,
                    in_reply_to TEXT, state TEXT NOT NULL DEFAULT 'queued',
                    attempts INTEGER NOT NULL DEFAULT 0, last_error TEXT,
                    queued_epoch INTEGER NOT NULL
                );
                INSERT INTO mailboxes (id, name, uid_validity) VALUES (1, 'INBOX', 7);
                INSERT INTO envelopes (mailbox_id, uid, subject, seen, flagged)
                    VALUES (1, 42, 'hérité', 1, 1);
                INSERT INTO bodies VALUES (1, 42, '<p>corps</p>');
                INSERT INTO pending_actions (mailbox_id, uid, kind) VALUES (1, 42, 'mark_seen');
                INSERT INTO drafts (to_raw, subject, body, updated_epoch, remote_uid, pushed_epoch)
                    VALUES ('x@y.fr', 'précieux', 'texte', 10, 77, 10);
                INSERT INTO draft_tombstones VALUES (99);
                INSERT INTO drafts_remote VALUES (1, 1234);
                INSERT INTO outbox (message_id, sender, recipients, subject, body_text, queued_epoch)
                    VALUES ('<m@x>', 'moi@y.fr', 'toi@y.fr', 's', 'b', 20);",
            )
            .unwrap();
        }

        let store = Store::open(&path).unwrap();
        let account = store
            .adopt_or_create_account("legacy@exemple.fr", "gmail")
            .unwrap();
        assert_eq!(account, 1, "la revendication prend le compte en attente");

        assert_eq!(store.recent(account, "INBOX", 0, 10).unwrap()[0].uid, 42);
        assert_eq!(
            store.body(1, "INBOX", 42).unwrap().as_deref(),
            Some("<p>corps</p>")
        );
        let drafts = store.drafts().unwrap();
        assert_eq!(drafts[0].account_id, 1);
        assert_eq!(drafts[0].remote_uid, Some(77));
        assert_eq!(store.draft_tombstones(1).unwrap(), vec![99]);
        assert!(
            !store.align_drafts_uidvalidity(1, 1234).unwrap(),
            "l'UIDVALIDITY des brouillons a survécu : pas de réinitialisation"
        );
        assert_eq!(store.outbox_to_send(1).unwrap().len(), 1);
        assert_eq!(store.accounts().unwrap().len(), 1);

        let second = store
            .adopt_or_create_account("deux@exemple.fr", "gmail")
            .unwrap();
        assert_ne!(second, 1, "le placeholder ne se revendique qu'une fois");

        drop(store);
        let _ = std::fs::remove_file(&path);
    }

    /// Le cœur produit du multi-comptes : la même boîte de tous les
    /// comptes, fusionnée par date — chaque ligne connaît son compte.
    #[test]
    fn unified_recent_merges_accounts_by_date() {
        let store = Store::open_in_memory().unwrap();
        let first = store
            .adopt_or_create_account("a@exemple.fr", "gmail")
            .unwrap();
        let second = store
            .adopt_or_create_account("b@exemple.fr", "gmail")
            .unwrap();
        let inbox_a = store.create_mailbox(first, "INBOX", 1).unwrap();
        let inbox_b = store.create_mailbox(second, "INBOX", 1).unwrap();

        let mut store = store;
        store
            .upsert_envelopes(
                inbox_a,
                &[
                    envelope(1, "a-ancien", 100, false),
                    envelope(2, "a-récent", 300, false),
                ],
            )
            .unwrap();
        store
            .upsert_envelopes(
                inbox_b,
                &[
                    envelope(1, "b-milieu", 200, false),
                    envelope(2, "b-dernier", 400, false),
                ],
            )
            .unwrap();

        let rows = store.unified_recent("INBOX", 0, 10).unwrap();
        let order: Vec<(&str, &str)> = rows
            .iter()
            .map(|row| {
                (
                    row.account_email.as_str(),
                    row.envelope.subject.as_deref().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            order,
            vec![
                ("b@exemple.fr", "b-dernier"),
                ("a@exemple.fr", "a-récent"),
                ("b@exemple.fr", "b-milieu"),
                ("a@exemple.fr", "a-ancien"),
            ],
            "fusion par date, chaque ligne porte son compte"
        );
        assert_eq!(store.unified_count("INBOX").unwrap(), 4);
        // Même UID dans deux comptes : deux messages distincts.
        assert!(store.envelope(first, "INBOX", 1).unwrap().is_some());
        assert!(store.envelope(second, "INBOX", 1).unwrap().is_some());
    }

    #[test]
    fn remove_absent_drops_orphaned_bodies() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(id, &[envelope(1, "a", 100, false)])
            .unwrap();
        store.save_body(id, 1, "<p>x</p>", &[]).unwrap();
        assert_eq!(store.remove_absent(id, &HashSet::new()).unwrap(), 1);
        assert_eq!(store.body(test_account(&store), "INBOX", 1).unwrap(), None);
    }

    /// Régression (bug #2) : ré-ajouter un compte générique déjà connu
    /// doit renvoyer le MÊME id et appliquer la nouvelle configuration.
    /// Sur le chemin UPDATE de l'upsert, `last_insert_rowid()` renvoyait
    /// 0 — un id fantôme que l'UI récupérait pour la pastille et la
    /// sélection. Chaque commande ouvre SA connexion : on modélise donc
    /// le ré-ajout par deux `Store` distincts sur la même base fichier,
    /// car c'est la connexion fraîche (sans INSERT préalable) qui emprunte
    /// le chemin UPDATE et exhibe le 0.
    #[test]
    fn re_adding_a_generic_account_returns_the_same_id_and_updates_config() {
        let path = std::env::temp_dir().join(format!(
            "discovery-test-generic-{}-{}.sqlite",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let first = {
            let store = Store::open(&path).unwrap();
            store
                .create_generic_account(
                    "compte@exemple.fr",
                    "compte",
                    "imap.a.fr",
                    993,
                    "smtp.a.fr",
                    465,
                )
                .unwrap()
        };
        let second = {
            let store = Store::open(&path).unwrap();
            store
                .create_generic_account(
                    "compte@exemple.fr",
                    "login",
                    "imap.b.fr",
                    143,
                    "smtp.b.fr",
                    587,
                )
                .unwrap()
        };
        let (count, config) = {
            let store = Store::open(&path).unwrap();
            (
                store.accounts().unwrap().len(),
                store.account_config(first).unwrap(),
            )
        };
        // Nettoyage avant les assertions : un échec ne doit pas laisser de
        // fichier temporaire derrière lui.
        let _ = std::fs::remove_file(&path);

        assert!(first > 0, "la primo-création doit renvoyer un id réel");
        assert_eq!(
            second, first,
            "le ré-ajout doit renvoyer l'id existant, jamais 0"
        );
        assert_eq!(count, 1, "un seul compte, pas un doublon");
        assert_eq!(config.username.as_deref(), Some("login"));
        assert_eq!(config.imap_host.as_deref(), Some("imap.b.fr"));
        assert_eq!(config.imap_port, Some(143));
        assert_eq!(config.smtp_host.as_deref(), Some("smtp.b.fr"));
        assert_eq!(config.smtp_port, Some(587));
    }

    /// Le rattrapage vise les messages RÉCENTS sans corps, du plus récent
    /// au plus ancien : c'est l'ordre où la recherche a le plus de valeur,
    /// et celui qui rend la reprise après coupure naturelle.
    #[test]
    fn backfill_lists_recent_bodyless_messages_newest_first() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "ancien", 1_000, false),
                    envelope(2, "milieu", 2_000, false),
                    envelope(3, "récent", 3_000, false),
                ],
            )
            .unwrap();
        let account = test_account(&store);

        let todo = store.bodies_to_backfill(account, "INBOX", 0, 10).unwrap();
        assert_eq!(todo, vec![3, 2, 1]);
    }

    #[test]
    fn backfill_skips_messages_that_already_have_a_body() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "sans corps", 1_000, false),
                    envelope(2, "avec corps", 2_000, false),
                ],
            )
            .unwrap();
        store.save_body(id, 2, "<p>déjà là</p>", &[]).unwrap();
        let account = test_account(&store);

        assert_eq!(
            store.bodies_to_backfill(account, "INBOX", 0, 10).unwrap(),
            vec![1]
        );
    }

    /// L'horizon de récence est ce qui BORNE le coût (ADR 0007) : au-delà,
    /// on ne rapatrie rien.
    #[test]
    fn backfill_respects_the_recency_horizon() {
        let (mut store, id) = store_with_mailbox();
        store
            .upsert_envelopes(
                id,
                &[
                    envelope(1, "hors horizon", 1_000, false),
                    envelope(2, "dans l'horizon", 5_000, false),
                ],
            )
            .unwrap();
        let account = test_account(&store);

        assert_eq!(
            store
                .bodies_to_backfill(account, "INBOX", 4_000, 10)
                .unwrap(),
            vec![2]
        );
    }

    #[test]
    fn backfill_honours_the_batch_limit() {
        let (mut store, id) = store_with_mailbox();
        let envelopes: Vec<Envelope> = (1..=10)
            .map(|uid| envelope(uid, "message", uid as i64 * 100, false))
            .collect();
        store.upsert_envelopes(id, &envelopes).unwrap();
        let account = test_account(&store);

        assert_eq!(
            store
                .bodies_to_backfill(account, "INBOX", 0, 3)
                .unwrap()
                .len(),
            3
        );
    }

    #[test]
    fn backfill_never_leaks_another_accounts_messages() {
        let (mut store, mine) = store_with_mailbox();
        let other = store
            .adopt_or_create_account("autre@exemple.fr", "gmail")
            .unwrap();
        let theirs = store.create_mailbox(other, "INBOX", 1).unwrap();
        store
            .upsert_envelopes(mine, &[envelope(1, "à moi", 1_000, false)])
            .unwrap();
        store
            .upsert_envelopes(theirs, &[envelope(1, "à l'autre", 2_000, false)])
            .unwrap();
        let account = test_account(&store);

        assert_eq!(
            store.bodies_to_backfill(account, "INBOX", 0, 10).unwrap(),
            vec![1],
            "un seul message : celui du compte demandé"
        );
        assert_eq!(
            store.bodies_to_backfill(other, "INBOX", 0, 10).unwrap(),
            vec![1]
        );
    }
}
