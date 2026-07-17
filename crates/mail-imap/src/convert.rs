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
    let flagged = fetch
        .flags()
        .iter()
        .any(|flag| matches!(flag, imap::types::Flag::Flagged));
    let date = fetch.internal_date().map(|d| d.with_timezone(&Utc));
    Some(envelope_from_parts(
        uid,
        fetch.envelope(),
        date,
        seen,
        flagged,
    ))
}

/// Cœur du mapping, séparé de `Fetch` (non constructible) pour être testable.
pub(crate) fn envelope_from_parts(
    uid: Uid,
    proto: Option<&ProtoEnvelope<'_>>,
    date: Option<chrono::DateTime<Utc>>,
    seen: bool,
    flagged: bool,
) -> Envelope {
    let subject = proto
        .and_then(|envelope| envelope.subject.as_deref())
        .and_then(decode_header);
    let from = proto
        .and_then(|envelope| envelope.from.as_ref())
        .and_then(|from| from.first());
    let message_id = proto
        .and_then(|envelope| envelope.message_id.as_deref())
        .and_then(text_header);
    Envelope {
        uid,
        subject,
        sender: from.and_then(sender_display),
        sender_address: from.and_then(address_literal),
        message_id,
        date,
        seen,
        flagged,
    }
}

/// Nom d'affichage s'il existe (décodé), sinon `mailbox@host`.
fn sender_display(address: &Address<'_>) -> Option<String> {
    if let Some(name) = address.name.as_deref().and_then(decode_header) {
        return Some(name);
    }
    address_literal(address)
}

/// Adresse brute `mailbox@host` — la cible d'une réponse (Phase 2).
fn address_literal(address: &Address<'_>) -> Option<String> {
    let mailbox = address.mailbox.as_deref()?;
    let host = address.host.as_deref()?;
    Some(format!(
        "{}@{}",
        String::from_utf8_lossy(mailbox),
        String::from_utf8_lossy(host)
    ))
}

/// En-tête textuel brut (Message-ID) : ASCII en pratique, jamais encodé
/// RFC 2047 — pas de décodage, juste un nettoyage.
fn text_header(raw: &[u8]) -> Option<String> {
    let value = String::from_utf8_lossy(raw);
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Extrait le corps HTML d'un message brut. `mail-parser` convertit lui-même
/// les messages texte-seul en HTML sûr (enseignement de Phase 0) — `None`
/// seulement si le message est inanalysable. Les images embarquées (`cid:`)
/// sont inlinées en `data:` URIs : elles font partie du message, leur
/// affichage ne déclenche aucun chargement réseau.
pub(crate) fn extract_html(raw: &[u8]) -> Option<String> {
    let message = mail_parser::MessageParser::new().parse(raw)?;
    let html = message.body_html(0)?.into_owned();
    Some(inline_cid_images(html, &message))
}

fn inline_cid_images(html: String, message: &mail_parser::Message<'_>) -> String {
    use base64::Engine;
    use mail_parser::MimeHeaders;

    let mut result = html;
    for part in message.attachments() {
        let Some(content_id) = part.content_id() else {
            continue;
        };
        let Some(content_type) = part.content_type() else {
            continue;
        };
        let mime = format!(
            "{}/{}",
            content_type.ctype(),
            content_type.subtype().unwrap_or("octet-stream")
        );
        if !mime.starts_with("image/") {
            continue;
        }
        let data_uri = format!(
            "data:{mime};base64,{}",
            base64::engine::general_purpose::STANDARD.encode(part.contents())
        );
        let reference = format!("cid:{}", content_id.trim_matches(['<', '>']));
        result = result.replace(&reference, &data_uri);
    }
    result
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
        let envelope = envelope_from_parts(1, Some(&proto), None, false, false);
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
        let envelope = envelope_from_parts(1, Some(&proto), None, false, false);
        assert_eq!(envelope.sender.as_deref(), Some("S\u{e9}bastien"));
    }

    #[test]
    fn sender_falls_back_to_mailbox_at_host() {
        let proto = proto_envelope(b"sujet", address(None, Some(b"seb"), Some(b"example.com")));
        let envelope = envelope_from_parts(1, Some(&proto), None, false, false);
        assert_eq!(envelope.sender.as_deref(), Some("seb@example.com"));
    }

    #[test]
    fn missing_envelope_yields_bare_fields() {
        let date = Utc.timestamp_opt(1_700_000_000, 0).unwrap();
        let envelope = envelope_from_parts(42, None, Some(date), true, true);
        assert_eq!(envelope.uid, 42);
        assert_eq!(envelope.subject, None);
        assert_eq!(envelope.sender, None);
        assert_eq!(envelope.sender_address, None);
        assert_eq!(envelope.message_id, None);
        assert_eq!(envelope.date, Some(date));
        assert!(envelope.seen);
        assert!(envelope.flagged, "l'étoile suit les flags du FETCH");
    }

    /// L'adresse brute doit rester disponible même quand un nom d'affichage
    /// existe : c'est elle qu'on met dans le « À » d'une réponse.
    #[test]
    fn keeps_raw_sender_address_alongside_display_name() {
        let proto = proto_envelope(
            b"sujet",
            address(
                Some(b"=?UTF-8?Q?S=C3=A9bastien?="),
                Some(b"seb"),
                Some(b"example.com"),
            ),
        );
        let envelope = envelope_from_parts(1, Some(&proto), None, false, false);
        assert_eq!(envelope.sender.as_deref(), Some("S\u{e9}bastien"));
        assert_eq!(envelope.sender_address.as_deref(), Some("seb@example.com"));
    }

    #[test]
    fn extracts_message_id_for_threading() {
        let mut proto = proto_envelope(b"sujet", address(None, Some(b"a"), Some(b"b.c")));
        proto.message_id = Some(Cow::Borrowed(b" <abc.123@mail.example.com> ".as_slice()));
        let envelope = envelope_from_parts(1, Some(&proto), None, false, false);
        assert_eq!(
            envelope.message_id.as_deref(),
            Some("<abc.123@mail.example.com>")
        );
    }

    #[test]
    fn blank_subject_becomes_none() {
        let proto = proto_envelope(b"   ", address(None, Some(b"a"), Some(b"b.c")));
        let envelope = envelope_from_parts(1, Some(&proto), None, false, false);
        assert_eq!(envelope.subject, None);
    }

    #[test]
    fn extracts_html_body_from_raw_message() {
        let raw = b"From: a@b.c\r\nSubject: t\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<p>Bonjour <b>monde</b></p>";
        let html = extract_html(raw).expect("corps html attendu");
        assert!(html.contains("<b>monde</b>"));
    }

    #[test]
    fn inlines_embedded_cid_images_as_data_uris() {
        let raw = b"From: a@b.c\r\nSubject: t\r\nMIME-Version: 1.0\r\n\
Content-Type: multipart/related; boundary=\"B\"\r\n\r\n\
--B\r\nContent-Type: text/html; charset=utf-8\r\n\r\n\
<p>logo : <img src=\"cid:logo123\"></p>\r\n\
--B\r\nContent-Type: image/png\r\nContent-ID: <logo123>\r\n\
Content-Transfer-Encoding: base64\r\n\r\niVBORw0KGgo=\r\n--B--\r\n";
        let html = extract_html(raw).expect("corps html attendu");
        assert!(html.contains("data:image/png;base64,"));
        assert!(!html.contains("cid:logo123"));
    }

    #[test]
    fn converts_plain_text_message_to_html() {
        let raw = b"From: a@b.c\r\nSubject: t\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nBonjour <chevron>";
        let html = extract_html(raw).expect("conversion texte vers html attendue");
        assert!(html.contains("Bonjour"));
        assert!(
            !html.contains("<chevron>"),
            "le texte doit être échappé, pas interprété"
        );
    }
}
