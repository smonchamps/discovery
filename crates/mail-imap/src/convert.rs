//! Traduction des réponses IMAP vers les types du domaine.
//!
//! Les en-têtes arrivent encodés RFC 2047 (`=?UTF-8?Q?…?=`) et fragmentés :
//! le décodage est délégué à `mail-parser` (décision gelée, PHASE0.md §2.3),
//! jamais réécrit à la main.

use chrono::Utc;
use imap_proto::types::{Address, Envelope as ProtoEnvelope};
use mail_core::{Envelope, Uid};

/// Compacte une liste d'UIDs en ensemble IMAP : `[1,2,3,5]` → `"1:3,5"`.
pub(crate) fn uid_set(uids: &[Uid]) -> String {
    let mut sorted = uids.to_vec();
    sorted.sort_unstable();
    sorted.dedup();

    let mut parts: Vec<String> = Vec::new();
    let mut run: Option<(Uid, Uid)> = None;
    for uid in sorted {
        run = match run {
            Some((start, end)) if uid == end + 1 => Some((start, uid)),
            Some((start, end)) => {
                parts.push(format_run(start, end));
                Some((uid, uid))
            }
            None => Some((uid, uid)),
        };
    }
    if let Some((start, end)) = run {
        parts.push(format_run(start, end));
    }
    parts.join(",")
}

fn format_run(start: Uid, end: Uid) -> String {
    if start == end {
        start.to_string()
    } else {
        format!("{start}:{end}")
    }
}

pub(crate) fn fetch_to_envelope(fetch: &imap::types::Fetch) -> Option<Envelope> {
    let uid = fetch.uid?;
    let seen = fetch
        .flags()
        .iter()
        .any(|flag| matches!(flag, imap::types::Flag::Seen));
    let date = fetch.internal_date().map(|d| d.with_timezone(&Utc));
    Some(envelope_from_parts(uid, fetch.envelope(), date, seen))
}

/// Cœur du mapping, séparé de `Fetch` (non constructible) pour être testable.
pub(crate) fn envelope_from_parts(
    uid: Uid,
    proto: Option<&ProtoEnvelope<'_>>,
    date: Option<chrono::DateTime<Utc>>,
    seen: bool,
) -> Envelope {
    let subject = proto
        .and_then(|envelope| envelope.subject.as_deref())
        .and_then(decode_header);
    let sender = proto
        .and_then(|envelope| envelope.from.as_ref())
        .and_then(|from| from.first())
        .and_then(sender_display);
    Envelope {
        uid,
        subject,
        sender,
        date,
        seen,
    }
}

/// Nom d'affichage s'il existe (décodé), sinon `mailbox@host`.
fn sender_display(address: &Address<'_>) -> Option<String> {
    if let Some(name) = address.name.as_deref().and_then(decode_header) {
        return Some(name);
    }
    let mailbox = address.mailbox.as_deref()?;
    let host = address.host.as_deref()?;
    Some(format!(
        "{}@{}",
        String::from_utf8_lossy(mailbox),
        String::from_utf8_lossy(host)
    ))
}

/// Décode un en-tête RFC 2047 en le présentant à `mail-parser` comme un
/// message synthétique. Retourne `None` pour un en-tête vide.
fn decode_header(raw: &[u8]) -> Option<String> {
    let synthetic = [b"Subject: ".as_slice(), raw, b"\r\n\r\n".as_slice()].concat();
    let decoded = mail_parser::MessageParser::new()
        .parse(&synthetic)
        .and_then(|message| message.subject().map(str::to_string))
        .unwrap_or_else(|| String::from_utf8_lossy(raw).into_owned());
    let trimmed = decoded.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use chrono::TimeZone;

    use super::*;

    fn address<'a>(
        name: Option<&'a [u8]>,
        mailbox: Option<&'a [u8]>,
        host: Option<&'a [u8]>,
    ) -> Address<'a> {
        Address {
            name: name.map(Cow::Borrowed),
            adl: None,
            mailbox: mailbox.map(Cow::Borrowed),
            host: host.map(Cow::Borrowed),
        }
    }

    fn proto_envelope<'a>(subject: &'a [u8], from: Address<'a>) -> ProtoEnvelope<'a> {
        ProtoEnvelope {
            date: None,
            subject: Some(Cow::Borrowed(subject)),
            from: Some(vec![from]),
            sender: None,
            reply_to: None,
            to: None,
            cc: None,
            bcc: None,
            in_reply_to: None,
            message_id: None,
        }
    }

    #[test]
    fn uid_set_compacts_consecutive_runs() {
        assert_eq!(uid_set(&[1, 2, 3, 5, 7, 8]), "1:3,5,7:8");
    }

    #[test]
    fn uid_set_handles_single_and_unordered_duplicates() {
        assert_eq!(uid_set(&[4]), "4");
        assert_eq!(uid_set(&[9, 7, 8, 8, 1]), "1,7:9");
    }

    #[test]
    fn decodes_rfc2047_subject() {
        let proto = proto_envelope(
            b"=?UTF-8?Q?R=C3=A9union_de_demain?=",
            address(None, Some(b"seb"), Some(b"example.com")),
        );
        let envelope = envelope_from_parts(1, Some(&proto), None, false);
        assert_eq!(envelope.subject.as_deref(), Some("R\u{e9}union de demain"));
    }

    #[test]
    fn sender_prefers_decoded_display_name() {
        let proto = proto_envelope(
            b"sujet",
            address(
                Some(b"=?UTF-8?Q?S=C3=A9bastien?="),
                Some(b"seb"),
                Some(b"example.com"),
            ),
        );
        let envelope = envelope_from_parts(1, Some(&proto), None, false);
        assert_eq!(envelope.sender.as_deref(), Some("S\u{e9}bastien"));
    }

    #[test]
    fn sender_falls_back_to_mailbox_at_host() {
        let proto = proto_envelope(b"sujet", address(None, Some(b"seb"), Some(b"example.com")));
        let envelope = envelope_from_parts(1, Some(&proto), None, false);
        assert_eq!(envelope.sender.as_deref(), Some("seb@example.com"));
    }

    #[test]
    fn missing_envelope_yields_bare_fields() {
        let date = Utc.timestamp_opt(1_700_000_000, 0).unwrap();
        let envelope = envelope_from_parts(42, None, Some(date), true);
        assert_eq!(envelope.uid, 42);
        assert_eq!(envelope.subject, None);
        assert_eq!(envelope.sender, None);
        assert_eq!(envelope.date, Some(date));
        assert!(envelope.seen);
    }

    #[test]
    fn blank_subject_becomes_none() {
        let proto = proto_envelope(b"   ", address(None, Some(b"a"), Some(b"b.c")));
        let envelope = envelope_from_parts(1, Some(&proto), None, false);
        assert_eq!(envelope.subject, None);
    }
}
