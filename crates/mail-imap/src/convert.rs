//! Traduction des réponses IMAP vers les types du domaine.
//!
//! Les en-têtes arrivent encodés RFC 2047 (`=?UTF-8?Q?…?=`) et fragmentés :
//! le décodage est délégué à `mail-parser` (décision gelée, PHASE0.md §2.3),
//! jamais réécrit à la main.

use chrono::Utc;
use imap_proto::types::{Address, Envelope as ProtoEnvelope};
use mail_core::{Envelope, Uid};

/// Rôle spécial d'un dossier (RFC 6154), réduit à ce qui décide de
/// l'archivage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpecialUse {
    Archive,
    All,
    Other,
}

/// Ce qu'« archiver » veut dire sur CE serveur.
///
/// Déduit de ses capacités annoncées, **jamais du fournisseur** : c'est la
/// même discipline que la découverte de la corbeille et des brouillons.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ArchiveStrategy {
    /// Le serveur expose `\Archive` : y copier le message, puis l'expurger.
    MoveTo(String),
    /// Le serveur expose `\All` (sémantique Gmail) : expurger d'INBOX n'y
    /// retire que le libellé, le message survit dans « Tous les messages ».
    ExpungeOnly,
    /// Ni l'un ni l'autre : expurger DÉTRUIRAIT le message. On refuse.
    Unsupported,
}

/// Noms de repli, quand le serveur n'annonce aucun attribut d'archivage.
///
/// Exception délibérée à la règle « jamais de nom en dur », justifiée par
/// la mesure : Exchange Online annonce `\Drafts`, `\Junk`, `\Sent` et
/// `\Trash`, mais **pas** `\Archive` — alors que le dossier « Archive »
/// existe et sert (spikes/microsoft, compte réel). Sans ce repli,
/// archiver serait indisponible sur tout compte Microsoft. La liste reste
/// volontairement courte : un nom inconnu vaut mieux qu'un mauvais choix.
const ARCHIVE_FALLBACK_NAMES: [&str; 4] = ["archive", "archives", "archivé", "archivés"];

/// Choisit la stratégie d'archivage d'après les dossiers annoncés.
///
/// Ordre de priorité, du plus sûr au moins sûr :
/// 1. `\Archive` annoncé — l'intention du serveur, sans ambiguïté ;
/// 2. `\All` annoncé — sémantique Gmail, où expurger EST l'archivage ;
/// 3. un dossier nommé « Archive » — repli mesuré (voir ci-dessus) ;
/// 4. sinon : refus. « Jamais de perte de mail » (PLAN.md §1) l'emporte
///    sur le confort d'une fonctionnalité.
pub(crate) fn archive_strategy<'a>(
    folders: impl IntoIterator<Item = (&'a str, SpecialUse)>,
) -> ArchiveStrategy {
    let mut has_all = false;
    let mut named: Option<String> = None;
    for (name, role) in folders {
        match role {
            SpecialUse::Archive => return ArchiveStrategy::MoveTo(name.to_string()),
            SpecialUse::All => has_all = true,
            SpecialUse::Other => {
                // Le nom COMPLET doit correspondre : « Archive/Achats »
                // est un classement, pas la destination d'archivage.
                // Comparaison sur le nom DÉCODÉ : un serveur français
                // annonce `Archiv&AOk-s`, que la liste ne reconnaîtrait
                // jamais sous sa forme réseau. Ce qui est mémorisé reste
                // en revanche le nom réseau — c'est lui qu'on renverra.
                if named.is_none()
                    && ARCHIVE_FALLBACK_NAMES
                        .contains(&crate::mutf7::decode(name).to_lowercase().as_str())
                {
                    named = Some(name.to_string());
                }
            }
        }
    }
    if has_all {
        return ArchiveStrategy::ExpungeOnly;
    }
    match named {
        Some(folder) => ArchiveStrategy::MoveTo(folder),
        None => ArchiveStrategy::Unsupported,
    }
}

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

/// Type MIME d'une partie, `application/octet-stream` à défaut.
fn part_mime(part: &mail_parser::MessagePart<'_>) -> String {
    use mail_parser::MimeHeaders;

    match part.content_type() {
        Some(content_type) => format!(
            "{}/{}",
            content_type.ctype(),
            content_type.subtype().unwrap_or("octet-stream")
        ),
        None => "application/octet-stream".to_string(),
    }
}

/// Cette partie est-elle une image incorporée au HTML par
/// [`inline_cid_images`] ?
///
/// **Prédicat partagé, et c'est tout son intérêt** : ce qui est incorporé
/// au corps ne doit pas être listé en pièce jointe, et réciproquement.
/// Deux règles écrites séparément finiraient par diverger — soit le logo
/// d'infolettre apparaîtrait en pièce jointe, soit un fichier
/// disparaîtrait des deux côtés.
fn is_inlined_image(part: &mail_parser::MessagePart<'_>) -> bool {
    use mail_parser::MimeHeaders;

    part.content_id().is_some() && part_mime(part).starts_with("image/")
}

fn inline_cid_images(html: String, message: &mail_parser::Message<'_>) -> String {
    use base64::Engine;
    use mail_parser::MimeHeaders;

    let mut result = html;
    for part in message.attachments().filter(|part| is_inlined_image(part)) {
        let Some(content_id) = part.content_id() else {
            continue;
        };
        let data_uri = format!(
            "data:{};base64,{}",
            part_mime(part),
            base64::engine::general_purpose::STANDARD.encode(part.contents())
        );
        let reference = format!("cid:{}", content_id.trim_matches(['<', '>']));
        result = result.replace(&reference, &data_uri);
    }
    result
}

/// Les pièces jointes RÉELLES d'un message : les fichiers que
/// l'utilisateur reconnaîtrait comme tels.
///
/// Les images déjà incorporées au corps en sont exclues ([`is_inlined_image`]).
/// Le rang renvoyé suit les pièces RETENUES : c'est lui qui servira à
/// retrouver les octets plus tard, en rejouant cette même extraction.
pub(crate) fn extract_attachments(raw: &[u8]) -> Vec<mail_core::Attachment> {
    attachment_parts(raw)
        .into_iter()
        .enumerate()
        .map(|(index, (name, mime, size))| mail_core::Attachment {
            index,
            name,
            mime,
            size,
        })
        .collect()
}

/// Les octets d'UNE pièce jointe, désignée par son rang.
///
/// Rejoue l'extraction sur le message brut : le rang est donc stable par
/// construction, sans jamais manipuler de numéro de partie IMAP.
pub(crate) fn attachment_bytes(raw: &[u8], index: usize) -> Option<Vec<u8>> {
    let message = mail_parser::MessageParser::new().parse(raw)?;
    message
        .attachments()
        .filter(|part| !is_inlined_image(part))
        .nth(index)
        .map(|part| part.contents().to_vec())
}

/// Nom, type et taille décodée de chaque pièce jointe retenue.
fn attachment_parts(raw: &[u8]) -> Vec<(String, String, u64)> {
    use mail_parser::MimeHeaders;

    let Some(message) = mail_parser::MessageParser::new().parse(raw) else {
        return Vec::new();
    };
    message
        .attachments()
        .filter(|part| !is_inlined_image(part))
        .map(|part| {
            let mime = part_mime(part);
            // `attachment_name` décode déjà le RFC 2047. Sans nom, on en
            // fabrique un : un fichier anonyme reste enregistrable.
            let name = part
                .attachment_name()
                .map(str::to_string)
                .unwrap_or_else(|| fallback_name(&mime));
            (name, mime, part.contents().len() as u64)
        })
        .collect()
}

/// Nom de repli pour une pièce sans `filename` — dérivé du sous-type.
fn fallback_name(mime: &str) -> String {
    let extension = mime.rsplit('/').next().unwrap_or("bin");
    format!("piece-jointe.{extension}")
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

    // --- Pièces jointes -------------------------------------------

    /// Un message porteur d'un vrai fichier : nom, type et taille DÉCODÉE.
    #[test]
    fn lists_a_real_attachment_with_its_name_type_and_decoded_size() {
        let raw = b"From: a@b.c
Subject: t
MIME-Version: 1.0
Content-Type: multipart/mixed; boundary=\"B\"

--B
Content-Type: text/html

<p>voici</p>
--B
Content-Type: application/pdf; name=\"facture.pdf\"
Content-Disposition: attachment; filename=\"facture.pdf\"
Content-Transfer-Encoding: base64

SGVsbG8sIHdvcmxkIQ==
--B--
";
        let found = extract_attachments(raw);
        assert_eq!(
            found.len(),
            1,
            "une seule pièce jointe attendue : {found:?}"
        );
        assert_eq!(found[0].name, "facture.pdf");
        assert_eq!(found[0].mime, "application/pdf");
        // "Hello, world!" = 13 octets une fois le base64 décodé.
        assert_eq!(
            found[0].size, 13,
            "la taille doit être celle des octets décodés"
        );
        assert_eq!(found[0].index, 0);
    }

    /// LE piège de cette fonctionnalité. `mail_parser` range les images
    /// référencées par Content-ID parmi les `attachments()` — or elles
    /// sont DÉJÀ incorporées au HTML par `inline_cid_images`. Sans ce
    /// filtre, le logo de chaque infolettre apparaîtrait comme une pièce
    /// jointe : le trombone deviendrait du bruit permanent.
    #[test]
    fn an_inlined_cid_image_is_not_an_attachment() {
        let raw = b"From: a@b.c
Subject: t
MIME-Version: 1.0
Content-Type: multipart/related; boundary=\"B\"

--B
Content-Type: text/html; charset=utf-8

<p>logo : <img src=\"cid:logo123\"></p>
--B
Content-Type: image/png
Content-ID: <logo123>
Content-Transfer-Encoding: base64

iVBORw0KGgo=
--B--
";
        assert!(
            extract_attachments(raw).is_empty(),
            "une image déjà incorporée au HTML ne doit pas être listée"
        );
    }

    /// Le cas réel : une infolettre avec son logo ET une vraie pièce
    /// jointe. Exactement une doit ressortir.
    #[test]
    fn keeps_the_real_file_and_drops_the_logo() {
        let raw = b"From: a@b.c
Subject: t
MIME-Version: 1.0
Content-Type: multipart/mixed; boundary=\"B\"

--B
Content-Type: text/html

<img src=\"cid:logo\">
--B
Content-Type: image/png
Content-ID: <logo>
Content-Transfer-Encoding: base64

iVBORw0KGgo=
--B
Content-Type: application/pdf
Content-Disposition: attachment; filename=\"contrat.pdf\"

PDF
--B--
";
        let found = extract_attachments(raw);
        assert_eq!(found.len(), 1, "le logo doit disparaître : {found:?}");
        assert_eq!(found[0].name, "contrat.pdf");
    }

    /// Symétrique du précédent, dans l'autre sens : un fichier NON-image
    /// porteur d'un Content-ID n'est pas incorporé au HTML, donc il reste
    /// une pièce jointe. Le filtre doit être exactement celui de
    /// l'incorporation — ni plus large, ni plus étroit.
    #[test]
    fn a_non_image_with_a_content_id_stays_an_attachment() {
        let raw = b"From: a@b.c
Subject: t
MIME-Version: 1.0
Content-Type: multipart/related; boundary=\"B\"

--B
Content-Type: text/html

<p>x</p>
--B
Content-Type: application/pdf
Content-ID: <doc1>
Content-Disposition: attachment; filename=\"annexe.pdf\"

PDF
--B--
";
        let found = extract_attachments(raw);
        assert_eq!(found.len(), 1, "un PDF n'est jamais incorporé au HTML");
        assert_eq!(found[0].name, "annexe.pdf");
    }

    /// Les noms non-ASCII circulent encodés (RFC 2047). Afficher
    /// `=?UTF-8?B?...?=` à l'utilisateur serait une régression visible —
    /// c'est le meme defaut que les dossiers en UTF-7 non decode.
    #[test]
    fn decodes_an_encoded_filename() {
        let raw = "From: a@b.c
Subject: t
MIME-Version: 1.0
Content-Type: multipart/mixed; boundary=\"B\"

--B
Content-Type: text/plain

corps
--B
Content-Type: application/pdf
Content-Disposition: attachment; filename=\"=?UTF-8?B?csOpc3Vtw6kucGRm?=\"

PDF
--B--
"
        .as_bytes();
        let found = extract_attachments(raw);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "résumé.pdf", "nom RFC 2047 à décoder");
    }

    /// Un message simple n'a rien à montrer — et surtout pas son propre
    /// corps déguisé en pièce jointe.
    #[test]
    fn a_plain_message_has_no_attachments() {
        let raw = b"From: a@b.c
Subject: t

Juste du texte.
";
        assert!(extract_attachments(raw).is_empty());
    }

    /// Les rangs sont contigus et servent de cle de re-telechargement :
    /// ils doivent suivre les pieces RETENUES, pas les parties MIME.
    #[test]
    fn indexes_are_contiguous_over_the_kept_attachments() {
        let raw = b"From: a@b.c
Subject: t
MIME-Version: 1.0
Content-Type: multipart/mixed; boundary=\"B\"

--B
Content-Type: text/html

<img src=\"cid:l\">
--B
Content-Type: image/png
Content-ID: <l>

PNG
--B
Content-Type: application/pdf
Content-Disposition: attachment; filename=\"un.pdf\"

A
--B
Content-Type: text/csv
Content-Disposition: attachment; filename=\"deux.csv\"

B
--B--
";
        let found = extract_attachments(raw);
        assert_eq!(found.len(), 2);
        assert_eq!(
            found[0].index, 0,
            "le logo écarté ne doit pas décaler les rangs"
        );
        assert_eq!(found[1].index, 1);
        assert_eq!(found[1].name, "deux.csv");
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

    /// Gmail n'expose pas `\Archive` mais expose `\All` : expurger d'INBOX
    /// ne fait qu'y retirer le libellé, le message survit dans « Tous les
    /// messages ». C'est la sémantique d'origine du produit.
    #[test]
    fn gmail_archives_by_expunging_because_all_mail_catches_the_message() {
        let folders = [
            ("INBOX", SpecialUse::Other),
            ("[Gmail]/Tous les messages", SpecialUse::All),
            ("[Gmail]/Corbeille", SpecialUse::Other),
        ];
        assert_eq!(archive_strategy(folders), ArchiveStrategy::ExpungeOnly);
    }

    /// Un serveur générique qui expose `\Archive` : on y DÉPLACE le message.
    /// Dette UTF-7 soldée. Un serveur francophone annonce son dossier
    /// d'archives en UTF-7 modifié : `Archiv&AOk-s`. Sans décodage, le
    /// repli par nom ne le reconnaissait pas, et l'archivage restait
    /// indisponible sur ces comptes — exactement le cas Exchange qui a
    /// motivé le repli (ADR 0006).
    ///
    /// Ce qui est retenu reste le nom RÉSEAU : c'est lui qu'on renverra
    /// au serveur, jamais sa forme lisible.
    #[test]
    fn an_accented_archive_folder_is_recognised_through_its_encoded_name() {
        let strategy = archive_strategy([
            ("INBOX", SpecialUse::Other),
            ("Archiv&AOk-s", SpecialUse::Other),
        ]);
        assert_eq!(
            strategy,
            ArchiveStrategy::MoveTo("Archiv&AOk-s".to_string()),
            "le nom mémorisé doit rester celui du protocole"
        );
    }

    #[test]
    fn generic_server_moves_to_its_archive_folder() {
        let folders = [
            ("INBOX", SpecialUse::Other),
            ("Archive", SpecialUse::Archive),
            ("Trash", SpecialUse::Other),
        ];
        assert_eq!(
            archive_strategy(folders),
            ArchiveStrategy::MoveTo("Archive".to_string())
        );
    }

    /// LE cas qui perdait des messages : ni `\Archive`, ni `\All`. Sur un
    /// IMAP générique, expurger d'INBOX SUPPRIME définitivement — il n'y a
    /// aucun filet. On refuse plutôt que de détruire.
    #[test]
    fn refuses_to_archive_when_expunging_would_destroy_the_message() {
        let folders = [("INBOX", SpecialUse::Other), ("Trash", SpecialUse::Other)];
        assert_eq!(archive_strategy(folders), ArchiveStrategy::Unsupported);
    }

    /// Exchange Online annonce `\Drafts`, `\Junk`, `\Sent` et `\Trash`
    /// mais PAS `\Archive` — alors que le dossier « Archive » existe et
    /// sert (mesuré sur compte réel, spikes/microsoft). Sans ce repli,
    /// archiver serait indisponible sur tout compte Microsoft.
    #[test]
    fn falls_back_to_a_folder_named_archive_when_the_attribute_is_missing() {
        let exchange = [
            ("Archive", SpecialUse::Other),
            ("Archive/Achats", SpecialUse::Other),
            ("INBOX", SpecialUse::Other),
            ("Drafts", SpecialUse::Other),
            ("Deleted", SpecialUse::Other),
        ];
        assert_eq!(
            archive_strategy(exchange),
            ArchiveStrategy::MoveTo("Archive".to_string())
        );
    }

    #[test]
    fn named_archive_matches_whatever_the_case() {
        let folders = [
            ("INBOX", SpecialUse::Other),
            ("ARCHIVES", SpecialUse::Other),
        ];
        assert_eq!(
            archive_strategy(folders),
            ArchiveStrategy::MoveTo("ARCHIVES".to_string())
        );
    }

    /// Un SOUS-dossier d'archive n'est pas le dossier d'archive : on ne
    /// déverserait pas le courrier dans « Archive/Achats ».
    #[test]
    fn an_archive_subfolder_alone_does_not_count() {
        let folders = [
            ("INBOX", SpecialUse::Other),
            ("Archive/Achats", SpecialUse::Other),
        ];
        assert_eq!(archive_strategy(folders), ArchiveStrategy::Unsupported);
    }

    /// Un attribut annoncé fait toujours foi contre une simple
    /// correspondance de nom : chez Gmail, expurger EST l'archivage.
    #[test]
    fn announced_all_mail_wins_over_a_merely_named_folder() {
        let folders = [
            ("[Gmail]/Tous les messages", SpecialUse::All),
            ("Archive", SpecialUse::Other),
        ];
        assert_eq!(archive_strategy(folders), ArchiveStrategy::ExpungeOnly);
    }

    /// `\Archive` prime sur `\All` : déplacer est toujours plus sûr
    /// qu'expurger, quel que soit l'ordre d'annonce des dossiers.
    #[test]
    fn archive_folder_wins_over_all_mail_whatever_the_order() {
        let all_first = [("Tous", SpecialUse::All), ("Archives", SpecialUse::Archive)];
        let archive_first = [("Archives", SpecialUse::Archive), ("Tous", SpecialUse::All)];
        let expected = ArchiveStrategy::MoveTo("Archives".to_string());
        assert_eq!(archive_strategy(all_first), expected);
        assert_eq!(archive_strategy(archive_first), expected);
    }
}
