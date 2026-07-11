//! Logique de synchronisation « enveloppes d'abord », protocole UID :
//! - synchro initiale par lots de séquences ;
//! - synchro incrémentale : nouveaux messages via `UID FETCH last+1:*`,
//!   suppressions via diff avec `UID SEARCH ALL` ;
//! - resynchronisation complète si UIDVALIDITY change.
//!
//! Flags non gérés dans ce spike : la réponse de production est CONDSTORE.

use std::collections::HashSet;

use anyhow::Context;

use crate::db::{EnvelopeRow, Store};
use crate::gmail::ImapSession;

const BATCH_SIZE: u32 = 1000;
const FETCH_QUERY: &str = "(UID ENVELOPE INTERNALDATE)";

pub struct SyncReport {
    pub mode: &'static str,
    pub fetched: usize,
    pub deleted: usize,
    pub total_on_server: u32,
}

pub fn sync_inbox(session: &mut ImapSession, store: &mut Store) -> anyhow::Result<SyncReport> {
    let mailbox = session.select("INBOX").context("SELECT INBOX")?;
    let uidvalidity = mailbox
        .uid_validity
        .context("UIDVALIDITY absent de la réponse SELECT")?;
    let exists = mailbox.exists;

    let last_uid = match store.state()? {
        Some(state) if state.uidvalidity == uidvalidity => state.last_uid,
        Some(_) => {
            println!("UIDVALIDITY a changé : resynchronisation complète.");
            store.reset()?;
            0
        }
        None => 0,
    };

    let report = if last_uid == 0 {
        initial_sync(session, store, exists)?
    } else {
        incremental_sync(session, store, last_uid, exists)?
    };

    store.set_state(uidvalidity, store.max_uid()?)?;
    Ok(report)
}

fn initial_sync(
    session: &mut ImapSession,
    store: &mut Store,
    exists: u32,
) -> anyhow::Result<SyncReport> {
    let mut fetched = 0;
    let mut start = 1u32;
    while start <= exists {
        let end = start.saturating_add(BATCH_SIZE - 1).min(exists);
        let fetches = session
            .fetch(format!("{start}:{end}"), FETCH_QUERY)
            .with_context(|| format!("FETCH {start}:{end}"))?;
        let rows = to_rows(&fetches);
        store.insert(&rows)?;
        fetched += rows.len();
        println!("  synchro initiale : {fetched}/{exists} enveloppes…");
        start = end + 1;
    }
    Ok(SyncReport {
        mode: "initiale",
        fetched,
        deleted: 0,
        total_on_server: exists,
    })
}

fn incremental_sync(
    session: &mut ImapSession,
    store: &mut Store,
    last_uid: u32,
    exists: u32,
) -> anyhow::Result<SyncReport> {
    // `n:*` renvoie toujours au moins le dernier message : filtrer sur l'UID.
    let rows: Vec<EnvelopeRow> = if exists == 0 {
        Vec::new()
    } else {
        let fetches = session
            .uid_fetch(format!("{}:*", last_uid + 1), FETCH_QUERY)
            .context("UID FETCH des nouveaux messages")?;
        to_rows(&fetches)
            .into_iter()
            .filter(|row| row.uid > last_uid)
            .collect()
    };
    store.insert(&rows)?;

    let server_uids: HashSet<u32> = if exists == 0 {
        HashSet::new()
    } else {
        session.uid_search("ALL").context("UID SEARCH ALL")?
    };
    let deleted = store.retain(&server_uids)?;

    Ok(SyncReport {
        mode: "incrémentale",
        fetched: rows.len(),
        deleted,
        total_on_server: exists,
    })
}

fn to_rows(fetches: &imap::types::Fetches) -> Vec<EnvelopeRow> {
    fetches
        .iter()
        .filter_map(|fetch| {
            let uid = fetch.uid?;
            let envelope = fetch.envelope();
            let subject = envelope
                .and_then(|e| e.subject.as_deref())
                .map(decode_header)
                .unwrap_or_else(|| "(sans sujet)".to_string());
            let sender = envelope
                .and_then(|e| e.from.as_ref())
                .and_then(|from| from.first())
                .map(|addr| {
                    let mailbox = lossy(addr.mailbox.as_deref());
                    let host = lossy(addr.host.as_deref());
                    format!("{mailbox}@{host}")
                })
                .unwrap_or_else(|| "(expéditeur inconnu)".to_string());
            let date = fetch
                .internal_date()
                .map(|d| d.to_rfc3339())
                .unwrap_or_default();
            Some(EnvelopeRow {
                uid,
                subject,
                sender,
                date,
            })
        })
        .collect()
}

/// Décode un en-tête RFC 2047 (`=?UTF-8?Q?…?=`) via mail-parser, en le
/// présentant comme un message synthétique — pas de décodage maison.
fn decode_header(raw: &[u8]) -> String {
    let synthetic = [b"Subject: ".as_slice(), raw, b"\r\n\r\n".as_slice()].concat();
    mail_parser::MessageParser::new()
        .parse(&synthetic)
        .and_then(|message| message.subject().map(str::to_string))
        .unwrap_or_else(|| String::from_utf8_lossy(raw).into_owned())
}

fn lossy(bytes: Option<&[u8]>) -> String {
    bytes
        .map(|b| String::from_utf8_lossy(b).into_owned())
        .unwrap_or_default()
}
