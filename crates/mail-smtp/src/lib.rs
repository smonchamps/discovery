//! Adaptateur SMTP : l'implémentation réelle de [`mail_core::MailTransport`].
//!
//! Le noyau ne connaît que le trait ; ce crate traduit un
//! [`OutboxMessage`] en message RFC 5322 (crate `lettre`) et le remet au
//! serveur en XOAUTH2 — jamais de mot de passe, comme pour IMAP.
//!
//! Classification des échecs (le contrat du port) :
//! - l'authentification se joue à la CONNEXION (`test_connection`) : un
//!   token expiré fait échouer l'ouverture, jamais un envoi — sinon un
//!   simple token périmé enverrait des messages sains en quarantaine ;
//! - pendant l'envoi, une réponse 5xx du serveur est un refus du MESSAGE
//!   (`Permanent`), tout le reste (réseau, 4xx) est `Transient`.
//!
//! Note Gmail : un message accepté en SMTP est ajouté par Gmail lui-même
//! au dossier « Envoyés » — aucun APPEND IMAP à faire. D'autres
//! fournisseurs l'exigeront (Phase 3, multi-comptes).

use lettre::address::Envelope;
use lettre::message::header::{ContentType, Header, HeaderName, HeaderValue};
use lettre::message::{Attachment as FilePart, Mailbox, MultiPart, SinglePart};
use lettre::transport::smtp::SmtpTransportBuilder;
use lettre::transport::smtp::authentication::{Credentials, Mechanism};
use lettre::{Address, Message, SmtpTransport, Transport};
use mail_core::{DraftAttachmentFull, MailTransport, OutboxMessage, SendError};

pub struct SmtpMailer {
    transport: SmtpTransport,
}

/// Mode TLS déduit du port de soumission SMTP. 465 est le port SMTPS
/// (TLS implicite dès l'ouverture) ; 587 et les autres ports de
/// soumission montent le chiffrement via STARTTLS. Jamais de repli en
/// clair — la règle sécurité « TLS partout » tient (un serveur sans
/// STARTTLS fait échouer l'ouverture, ce qui est le comportement voulu).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SmtpTls {
    Implicit,
    StartTls,
}

fn smtp_tls_for_port(port: u16) -> SmtpTls {
    match port {
        465 => SmtpTls::Implicit,
        _ => SmtpTls::StartTls,
    }
}

/// Ouvre le transport vers `host:port` selon la politique TLS du port.
///
/// Chemin UNIQUE des deux modes d'authentification : c'est ce qui garantit
/// qu'un correctif sur la politique de port ne peut plus profiter à un
/// seul des deux. Le bug #3 était né de cette duplication.
fn transport_builder(host: &str, port: u16) -> Result<SmtpTransportBuilder, SendError> {
    match smtp_tls_for_port(port) {
        SmtpTls::Implicit => SmtpTransport::relay(host),
        SmtpTls::StartTls => SmtpTransport::starttls_relay(host),
    }
    .map(|builder| builder.port(port))
    .map_err(|err| SendError::Transient(err.to_string()))
}

impl SmtpMailer {
    /// Connexion TLS + authentification XOAUTH2, vérifiée immédiatement :
    /// on ne rend un transport que s'il sait envoyer.
    ///
    /// Le `port` est honoré — Gmail écoute en 465 (TLS implicite),
    /// `smtp.office365.com` uniquement en 587 (STARTTLS).
    pub fn connect_xoauth2(
        host: &str,
        port: u16,
        user: &str,
        access_token: &str,
    ) -> Result<Self, SendError> {
        let transport = transport_builder(host, port)?
            .authentication(vec![Mechanism::Xoauth2])
            .credentials(Credentials::new(user.to_string(), access_token.to_string()))
            .build();
        Self::test_transport(transport)
    }

    /// Connexion TLS + authentification par mot de passe (SMTP générique).
    /// Même politique de port que XOAUTH2. Vérifiée immédiatement.
    pub fn connect_password(
        host: &str,
        port: u16,
        user: &str,
        password: &str,
    ) -> Result<Self, SendError> {
        let transport = transport_builder(host, port)?
            .credentials(Credentials::new(user.to_string(), password.to_string()))
            .build();
        Self::test_transport(transport)
    }

    fn test_transport(transport: SmtpTransport) -> Result<Self, SendError> {
        match transport.test_connection() {
            Ok(true) => Ok(Self { transport }),
            Ok(false) => Err(SendError::Transient(
                "le serveur SMTP ne répond pas".to_string(),
            )),
            // Échec d'ouverture (réseau OU authentification) : transitoire
            // par définition — le message n'a même pas été présenté.
            Err(err) => Err(SendError::Transient(err.to_string())),
        }
    }
}

impl MailTransport for SmtpMailer {
    fn send(&mut self, message: &OutboxMessage) -> Result<(), SendError> {
        let email = build_message(message)?;
        // Cci : l'enveloppe SMTP porte TOUS les destinataires (À + Cc +
        // Cci), mais le message servi (`build_message`) n'a PAS d'en-tête
        // Bcc — c'est pourquoi on envoie via `send_raw` (enveloppe
        // explicite) plutôt que `send` (qui dériverait l'enveloppe des
        // en-têtes ET laisserait le Bcc fuiter dans le corps servi à tous).
        let envelope = build_envelope(message)?;
        let raw = email.formatted();
        match self.transport.send_raw(&envelope, &raw) {
            Ok(_) => Ok(()),
            Err(err) if err.is_permanent() => Err(SendError::Permanent(err.to_string())),
            Err(err) => Err(SendError::Transient(err.to_string())),
        }
    }
}

/// L'enveloppe SMTP : expéditeur + TOUS les destinataires (À, Cc, Cci).
/// C'est elle, et non les en-têtes du message, qui commande les `RCPT
/// TO` — le seul endroit où une adresse Cci a le droit de paraître.
fn build_envelope(message: &OutboxMessage) -> Result<Envelope, SendError> {
    let parse = |addr: &str| -> Result<Address, SendError> {
        addr.parse::<Address>()
            .map_err(|err| SendError::Permanent(format!("adresse invalide {addr:?} : {err}")))
    };
    let from = parse(&message.from)?;
    let recipients = message
        .to
        .iter()
        .chain(&message.cc)
        .chain(&message.bcc)
        .map(|addr| parse(addr))
        .collect::<Result<Vec<_>, _>>()?;
    Envelope::new(Some(from), recipients)
        .map_err(|err| SendError::Permanent(format!("enveloppe SMTP : {err}")))
}

/// En-tête `X-Priority: 1` d'un envoi marqué important (R3,
/// PLAN-RETOURS-6). `lettre` n'a pas d'en-tête de priorité intégré ;
/// la paire X-Priority + Importance est celle que posent Outlook et
/// Thunderbird — et que lisent Gmail et les autres. Toujours « 1 » :
/// Wind ne connaît qu'un cran (important), pas une échelle.
#[derive(Debug, Clone)]
struct XPriority;

impl Header for XPriority {
    fn name() -> HeaderName {
        HeaderName::new_from_ascii_str("X-Priority")
    }

    fn parse(_s: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Self)
    }

    fn display(&self) -> HeaderValue {
        HeaderValue::new(Self::name(), "1".to_string())
    }
}

/// En-tête `Importance: high` — le second de la paire (RFC 2156/4021).
#[derive(Debug, Clone)]
struct Importance;

impl Header for Importance {
    fn name() -> HeaderName {
        HeaderName::new_from_ascii_str("Importance")
    }

    fn parse(_s: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Self)
    }

    fn display(&self) -> HeaderValue {
        HeaderValue::new(Self::name(), "high".to_string())
    }
}

/// Traduit un message de la boîte d'envoi en message RFC 5322.
///
/// Le Message-ID est CELUI du journal — jamais regénéré : c'est lui qui
/// relie l'entrée de la boîte d'envoi au message réellement parti
/// (règle « jamais d'envoi fantôme »).
fn build_message(message: &OutboxMessage) -> Result<Message, SendError> {
    let mut builder = Message::builder()
        .from(parse_mailbox(&message.from)?)
        .subject(&message.subject)
        .message_id(Some(message.message_id.clone()))
        .date_now();
    for recipient in &message.to {
        builder = builder.to(parse_mailbox(recipient)?);
    }
    // Cc paraît dans les en-têtes ; Cci JAMAIS (elle vit dans l'enveloppe
    // SMTP seule, `build_envelope`) — c'est toute sa raison d'être.
    for recipient in &message.cc {
        builder = builder.cc(parse_mailbox(recipient)?);
    }
    if let Some(parent) = &message.in_reply_to {
        builder = builder
            .in_reply_to(parent.clone())
            .references(parent.clone());
    }
    // R3 : le marquage vient du journal — c'est lui qui part, jamais un
    // état d'écran. Ordinaire = aucun en-tête (chemin historique intact).
    if message.important {
        builder = builder.header(XPriority).header(Importance);
    }
    if message.attachments.is_empty() {
        // Corps riche : multipart/alternative — le texte d'abord (RFC
        // 2046, du plus simple au plus fidèle), le HTML ensuite. Sans
        // HTML, le chemin texte historique, octet pour octet.
        return match &message.body_html {
            None => builder.body(message.body_text.clone()),
            Some(html) => builder.multipart(corps_alternatif(&message.body_text, html)),
        }
        .map_err(|err| SendError::Permanent(format!("construction du message : {err}")));
    }
    // multipart/mixed : le corps d'abord (texte seul, ou alternative
    // texte+HTML emboîtée), puis chaque pièce telle que le journal la
    // porte (PJ-D2) — les octets viennent du journal, jamais d'un
    // fichier relu à l'envoi.
    let mut parts = match &message.body_html {
        None => MultiPart::mixed().singlepart(SinglePart::plain(message.body_text.clone())),
        Some(html) => MultiPart::mixed().multipart(corps_alternatif(&message.body_text, html)),
    };
    for piece in &message.attachments {
        // Des octets absents signent un journal purgé (PJ-D7) : par
        // construction, un message purgé est `sent`, donc jamais revenu
        // en file. Si l'invariant casse, refus franc — renvoyer un
        // message amputé de ses pièces serait pire.
        let bytes = piece.bytes.clone().ok_or_else(|| {
            SendError::Permanent(format!(
                "pièce {:?} sans octets : journal purgé, envoi refusé",
                piece.name
            ))
        })?;
        parts = parts.singlepart(file_part(&piece.name, &piece.mime, bytes)?);
    }
    builder
        .multipart(parts)
        .map_err(|err| SendError::Permanent(format!("construction du message : {err}")))
}

/// L'alternative texte+HTML d'un corps riche — LE constructeur commun
/// des quatre chemins (envoi/brouillon × avec/sans pièces) : l'ordre des
/// parties et le repli se décident ICI, une fois.
fn corps_alternatif(texte: &str, html: &str) -> MultiPart {
    MultiPart::alternative_plain_html(texte.to_string(), html.to_string())
}

/// La partie MIME d'une pièce — LE constructeur commun de l'envoi et du
/// reflet IMAP : un correctif (nom RFC 2231, repli de type) profite aux
/// deux, la leçon du bug #3.
///
/// Un type inconnu ne bloque rien : le flux d'octets générique dit
/// honnêtement « je ne sais pas ».
fn file_part(name: &str, mime: &str, bytes: Vec<u8>) -> Result<SinglePart, SendError> {
    let content_type = ContentType::parse(mime)
        .or_else(|_| ContentType::parse("application/octet-stream"))
        .map_err(|err| SendError::Permanent(format!("type de pièce : {err}")))?;
    Ok(FilePart::new(name.to_string()).body(bytes, content_type))
}

/// Message RFC 5322 d'un brouillon, prêt pour un APPEND `\Draft` — la
/// poussée vers le dossier Brouillons Gmail (Phase 2).
///
/// Un brouillon porte du texte brut : les destinataires invalides sont
/// omis (une adresse à moitié tapée reste locale) ; si le message n'est
/// pas constructible en l'état, il n'est simplement pas poussé — le
/// local reste la référence, rien n'est perdu.
///
/// Les pièces suivent (PJ-D6) : le reflet distant montre le brouillon
/// ENTIER, même constructeur de partie que l'envoi.
// Les champs plats d'un brouillon, tous des chaînes du même registre —
// le même compromis que `insert_draft` côté cœur.
#[allow(clippy::too_many_arguments)]
pub fn draft_bytes(
    from: &str,
    to_raw: &str,
    cc_raw: &str,
    bcc_raw: &str,
    subject: &str,
    body: &str,
    body_html: Option<&str>,
    pieces: &[DraftAttachmentFull],
) -> Result<Vec<u8>, SendError> {
    let mut builder = Message::builder()
        .from(parse_mailbox(from)?)
        .subject(subject)
        .date_now();
    for candidate in to_raw.split([',', ';']) {
        if let Ok(mailbox) = candidate.trim().parse::<Mailbox>() {
            builder = builder.to(mailbox);
        }
    }
    // Un brouillon est le message tel que l'utilisateur le prépare : il
    // porte SES Cc et Cci (le dossier Brouillons est le sien seul, rien
    // n'est encore envoyé). Adresses tolérées, comme le champ À.
    for candidate in cc_raw.split([',', ';']) {
        if let Ok(mailbox) = candidate.trim().parse::<Mailbox>() {
            builder = builder.cc(mailbox);
        }
    }
    for candidate in bcc_raw.split([',', ';']) {
        if let Ok(mailbox) = candidate.trim().parse::<Mailbox>() {
            builder = builder.bcc(mailbox);
        }
    }
    if pieces.is_empty() {
        // Même bascule que l'envoi : le reflet montre le brouillon riche
        // en multipart/alternative, le brouillon texte reste mono-partie.
        return match body_html {
            None => builder.body(body.to_string()),
            Some(html) => builder.multipart(corps_alternatif(body, html)),
        }
        .map(|message| message.formatted())
        .map_err(|err| SendError::Permanent(format!("construction du brouillon : {err}")));
    }
    let mut parts = match body_html {
        None => MultiPart::mixed().singlepart(SinglePart::plain(body.to_string())),
        Some(html) => MultiPart::mixed().multipart(corps_alternatif(body, html)),
    };
    for piece in pieces {
        parts = parts.singlepart(file_part(&piece.name, &piece.mime, piece.bytes.clone())?);
    }
    builder
        .multipart(parts)
        .map(|message| message.formatted())
        .map_err(|err| SendError::Permanent(format!("construction du brouillon : {err}")))
}

fn parse_mailbox(address: &str) -> Result<Mailbox, SendError> {
    address
        .parse()
        .map_err(|err| SendError::Permanent(format!("adresse invalide {address:?} : {err}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mail_core::{OutboxAttachment, OutboxState};
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    fn outbox_message(in_reply_to: Option<&str>) -> OutboxMessage {
        OutboxMessage {
            id: 1,
            account_id: 1,
            message_id: "<test.abc123@exemple.fr>".to_string(),
            from: "moi@exemple.fr".to_string(),
            to: vec!["a@exemple.fr".to_string(), "b@exemple.fr".to_string()],
            cc: vec![],
            bcc: vec![],
            subject: "Bonjour".to_string(),
            body_text: "Premier essai.\nDeuxième ligne.".to_string(),
            body_html: None,
            in_reply_to: in_reply_to.map(str::to_string),
            important: false,
            send_at_epoch: None,
            attachments: vec![],
            state: OutboxState::Queued,
            attempts: 0,
            last_error: None,
            queued_epoch: 1_700_000_000,
        }
    }

    fn formatted(message: &OutboxMessage) -> String {
        let email = build_message(message).expect("message construisible");
        String::from_utf8(email.formatted()).expect("en-têtes ASCII")
    }

    #[test]
    fn builds_message_with_our_message_id_never_a_generated_one() {
        let raw = formatted(&outbox_message(None));
        assert!(
            raw.contains("Message-ID: <test.abc123@exemple.fr>"),
            "le Message-ID du journal doit être celui du message :\n{raw}"
        );
    }

    #[test]
    fn addresses_every_recipient() {
        let raw = formatted(&outbox_message(None));
        assert!(raw.contains("From: moi@exemple.fr"));
        assert!(raw.contains("a@exemple.fr"));
        assert!(raw.contains("b@exemple.fr"));
    }

    /// Le cœur de la correctness du Cci : le Cc paraît dans les en-têtes du
    /// message SERVI, le Cci JAMAIS — il ne vit que dans l'enveloppe SMTP
    /// (les RCPT TO). Un Cci qui fuit dans le corps servi à tous n'est plus
    /// un Cci. C'est pourquoi l'envoi passe par `send_raw` + `build_envelope`.
    #[test]
    fn cc_dans_les_entetes_cci_dans_l_enveloppe_seule() {
        let mut message = outbox_message(None);
        message.cc = vec!["copie@exemple.fr".to_string()];
        message.bcc = vec!["invisible@exemple.fr".to_string()];

        let raw = formatted(&message);
        assert!(
            raw.contains("Cc: copie@exemple.fr"),
            "le Cc doit paraître :\n{raw}"
        );
        assert!(
            !raw.contains("invisible@exemple.fr"),
            "le Cci ne doit JAMAIS paraître dans le message servi :\n{raw}"
        );
        assert!(
            !raw.to_lowercase().contains("bcc:"),
            "aucun en-tête Bcc dans le message servi :\n{raw}"
        );

        // L'enveloppe, elle, porte TOUS les destinataires — Cci compris :
        // sans quoi le Cci ne recevrait rien.
        let envelope = build_envelope(&message).expect("enveloppe construisible");
        let rcpts: Vec<String> = envelope.to().iter().map(ToString::to_string).collect();
        assert!(rcpts.iter().any(|a| a == "a@exemple.fr"), "{rcpts:?}");
        assert!(rcpts.iter().any(|a| a == "copie@exemple.fr"), "{rcpts:?}");
        assert!(
            rcpts.iter().any(|a| a == "invisible@exemple.fr"),
            "le Cci DOIT être un destinataire d'enveloppe : {rcpts:?}"
        );
    }

    /// R3 (PLAN-RETOURS-6) : un envoi marqué important porte LA paire
    /// d'en-têtes que lisent les clients mûrs — `X-Priority: 1` et
    /// `Importance: high`. Un envoi ordinaire n'en porte aucun.
    #[test]
    fn important_message_carries_priority_headers() {
        let mut message = outbox_message(None);
        message.important = true;
        let raw = formatted(&message);
        assert!(raw.contains("X-Priority: 1"), "{raw}");
        assert!(raw.contains("Importance: high"), "{raw}");
    }

    #[test]
    fn ordinary_message_has_no_priority_headers() {
        let raw = formatted(&outbox_message(None));
        assert!(!raw.contains("X-Priority"), "{raw}");
        assert!(!raw.contains("Importance"), "{raw}");
    }

    #[test]
    fn reply_carries_threading_headers() {
        let raw = formatted(&outbox_message(Some("<origine@exemple.fr>")));
        assert!(raw.contains("In-Reply-To: <origine@exemple.fr>"));
        assert!(raw.contains("References: <origine@exemple.fr>"));
    }

    #[test]
    fn fresh_message_has_no_threading_headers() {
        let raw = formatted(&outbox_message(None));
        assert!(!raw.contains("In-Reply-To"));
        assert!(!raw.contains("References"));
    }

    #[test]
    fn body_is_plain_text_with_preserved_lines() {
        let raw = formatted(&outbox_message(None));
        assert!(raw.contains("Premier essai."));
        assert!(raw.contains("Deuxi=C3=A8me ligne.") || raw.contains("Deuxième ligne."));
    }

    /// Un message sans pièce reste mono-partie : le chemin historique ne
    /// paie pas le multipart.
    #[test]
    fn message_without_pieces_stays_single_part() {
        let raw = formatted(&outbox_message(None));
        assert!(!raw.contains("multipart/mixed"));
    }

    /// PLAN-COMPOSITION-HTML E3 : un corps riche part en
    /// multipart/alternative — le texte d'abord (RFC 2046 : du plus
    /// simple au plus fidèle), le HTML ensuite. Jamais de HTML seul :
    /// le repli texte est systématique.
    #[test]
    fn html_body_travels_as_multipart_alternative_with_plain_fallback() {
        let mut message = outbox_message(None);
        message.body_html = Some("<b>Premier essai.</b>".to_string());
        let raw = formatted(&message);

        assert!(raw.contains("multipart/alternative"), "{raw}");
        assert!(raw.contains("text/plain"), "{raw}");
        assert!(raw.contains("text/html"), "{raw}");
        assert!(raw.contains("<b>Premier essai.</b>"), "{raw}");
        assert!(
            raw.contains("Premier essai.\r\n") || raw.contains("Premier essai.\n"),
            "le repli texte doit partir aussi : {raw}"
        );
        let plain = raw.find("text/plain").unwrap();
        let html = raw.find("text/html").unwrap();
        assert!(plain < html, "le texte précède le HTML : {raw}");
        assert!(!raw.contains("multipart/mixed"), "sans pièce : {raw}");
    }

    /// Avec pièces, l'alternative s'emboîte dans le mixed :
    /// mixed(alternative(texte, html), pièce…) — la forme canonique.
    #[test]
    fn html_body_with_pieces_nests_alternative_inside_mixed() {
        let mut message = outbox_message(None);
        message.body_html = Some("<b>corps</b>".to_string());
        message.attachments = vec![piece("rapport.pdf", Some(vec![0xFF, 0xD8, 0xFF, 0xE0]))];
        let raw = formatted(&message);

        assert!(raw.contains("multipart/mixed"), "{raw}");
        assert!(raw.contains("multipart/alternative"), "{raw}");
        assert!(raw.contains("<b>corps</b>"), "{raw}");
        assert!(raw.contains("Content-Disposition: attachment"), "{raw}");
        assert!(raw.contains("rapport.pdf"), "{raw}");
        let mixed = raw.find("multipart/mixed").unwrap();
        let alternative = raw.find("multipart/alternative").unwrap();
        assert!(
            mixed < alternative,
            "l'alternative vit DANS le mixed : {raw}"
        );
    }

    fn piece(name: &str, bytes: Option<Vec<u8>>) -> OutboxAttachment {
        OutboxAttachment {
            name: name.to_string(),
            mime: "application/pdf".to_string(),
            size: bytes.as_ref().map_or(0, |b| b.len() as u64),
            bytes,
        }
    }

    /// PJ-D2 côté fil : le message part en multipart/mixed — le texte
    /// d'abord, puis chaque pièce en `attachment` avec son nom et ses
    /// octets. Des octets binaires (haut bit posé) forcent le base64 :
    /// FF D8 FF E0 s'encode « /9j/4A== ».
    #[test]
    fn pieces_travel_as_multipart_mixed_attachments() {
        let mut message = outbox_message(None);
        message.attachments = vec![piece("rapport.pdf", Some(vec![0xFF, 0xD8, 0xFF, 0xE0]))];
        let raw = formatted(&message);

        assert!(raw.contains("multipart/mixed"), "{raw}");
        assert!(raw.contains("Content-Disposition: attachment"), "{raw}");
        assert!(raw.contains("rapport.pdf"), "{raw}");
        assert!(
            raw.contains("/9j/4A=="),
            "les octets doivent partir : {raw}"
        );
        assert!(
            raw.contains("Premier essai."),
            "le texte reste la première partie : {raw}"
        );
    }

    /// Un nom non-ASCII ne part jamais cru dans les en-têtes : `lettre`
    /// l'encode RFC 2231 (`filename*0*=utf-8''…`) — encodé, pas perdu.
    #[test]
    fn non_ascii_piece_names_are_encoded_in_headers() {
        let mut message = outbox_message(None);
        message.attachments = vec![piece("résumé années.pdf", Some(vec![1]))];
        let email = build_message(&message).expect("message construisible");
        let raw = String::from_utf8_lossy(&email.formatted()).to_string();

        assert!(
            !raw.contains("résumé"),
            "le nom ne part pas cru dans les en-têtes : {raw}"
        );
        assert!(
            raw.contains("filename*") || raw.contains("=?utf-8?"),
            "le nom doit être encodé, pas perdu : {raw}"
        );
        assert!(
            raw.contains("r%C3%A9sum%C3%A9") || raw.contains("=?utf-8?"),
            "les octets UTF-8 du nom doivent se retrouver : {raw}"
        );
    }

    /// L'invariant PJ-D7 tient au fil aussi : des octets purgés (message
    /// déjà parti) ne construisent JAMAIS un message amputé — refus franc.
    #[test]
    fn purged_piece_is_a_permanent_refusal_never_an_amputated_message() {
        let mut message = outbox_message(None);
        message.attachments = vec![piece("parti.pdf", None)];
        match build_message(&message) {
            Err(SendError::Permanent(reason)) => {
                assert!(reason.contains("parti.pdf"), "{reason}");
            }
            Err(other) => panic!("attendu un refus permanent, obtenu {other:?}"),
            Ok(_) => panic!("attendu un refus permanent, obtenu un message construit"),
        }
    }

    /// Un type MIME illisible ne bloque pas l'envoi : flux d'octets
    /// générique — le refus au geste ne vit pas ici.
    #[test]
    fn unparseable_mime_falls_back_to_octet_stream() {
        let mut message = outbox_message(None);
        message.attachments = vec![OutboxAttachment {
            name: "brut.bin".to_string(),
            mime: "pas un type".to_string(),
            size: 1,
            bytes: Some(vec![7]),
        }];
        let raw = formatted(&message);
        assert!(raw.contains("application/octet-stream"), "{raw}");
    }

    #[test]
    fn draft_bytes_keeps_valid_recipients_and_omits_the_rest() {
        let raw = draft_bytes(
            "moi@exemple.fr",
            "valide@exemple.fr, adresse-en-cours-de-fra",
            "",
            "",
            "Brouillon",
            "corps",
            None,
            &[],
        )
        .expect("brouillon constructible");
        let text = String::from_utf8_lossy(&raw);
        assert!(text.contains("valide@exemple.fr"));
        assert!(!text.contains("adresse-en-cours-de-fra"));
        assert!(text.contains("Subject: Brouillon"));
    }

    /// Un brouillon sans destinataire (encore) valide n'est pas poussable :
    /// il reste local, rien n'est perdu — comportement documenté par test.
    #[test]
    fn draft_without_any_valid_recipient_stays_local() {
        let result = draft_bytes(
            "moi@exemple.fr",
            "pas encore d'adresse",
            "",
            "",
            "s",
            "c",
            None,
            &[],
        );
        assert!(result.is_err(), "attendu : non poussable en l'état");
    }

    /// PJ-D6 : le reflet distant montre le brouillon ENTIER — pièces
    /// comprises, par le même constructeur de partie que l'envoi.
    #[test]
    fn draft_bytes_carries_pieces_as_multipart_mixed() {
        let raw = draft_bytes(
            "moi@exemple.fr",
            "valide@exemple.fr",
            "",
            "",
            "Brouillon",
            "corps",
            None,
            &[DraftAttachmentFull {
                name: "devis.pdf".to_string(),
                mime: "application/pdf".to_string(),
                bytes: vec![0xFF, 0xD8, 0xFF, 0xE0],
            }],
        )
        .expect("brouillon constructible");
        let text = String::from_utf8_lossy(&raw);
        assert!(text.contains("multipart/mixed"), "{text}");
        assert!(text.contains("Content-Disposition: attachment"), "{text}");
        assert!(text.contains("devis.pdf"), "{text}");
        assert!(
            text.contains("/9j/4A=="),
            "les octets doivent suivre : {text}"
        );
        assert!(
            text.contains("corps"),
            "le texte reste la première partie : {text}"
        );
    }

    /// PLAN-COMPOSITION-HTML : le reflet Brouillons montre le brouillon
    /// riche ENTIER — multipart/alternative, comme l'envoi.
    #[test]
    fn draft_bytes_with_html_is_multipart_alternative() {
        let raw = draft_bytes(
            "moi@exemple.fr",
            "valide@exemple.fr",
            "",
            "",
            "Brouillon",
            "corps",
            Some("<b>corps</b>"),
            &[],
        )
        .expect("brouillon constructible");
        let text = String::from_utf8_lossy(&raw);
        assert!(text.contains("multipart/alternative"), "{text}");
        assert!(text.contains("text/html"), "{text}");
        assert!(text.contains("<b>corps</b>"), "{text}");
        assert!(
            text.contains("corps"),
            "le repli texte doit suivre : {text}"
        );
    }

    /// Un brouillon sans pièce reste mono-partie — le chemin historique
    /// ne paie pas le multipart.
    #[test]
    fn draft_bytes_without_pieces_stays_single_part() {
        let raw = draft_bytes(
            "moi@exemple.fr",
            "valide@exemple.fr",
            "",
            "",
            "s",
            "c",
            None,
            &[],
        )
        .expect("brouillon constructible");
        assert!(!String::from_utf8_lossy(&raw).contains("multipart/mixed"));
    }

    /// Régression (bug #1) : le port de soumission SMTP était ignoré —
    /// `connect_password` câblait `relay()` = TLS implicite 465 en dur,
    /// et le port saisi par l'utilisateur était jeté. La politique doit
    /// distinguer 465 (SMTPS, TLS implicite) de 587 et des autres ports
    /// de soumission (STARTTLS). Jamais de repli en clair.
    #[test]
    fn smtp_tls_policy_follows_the_submission_port() {
        assert_eq!(smtp_tls_for_port(465), SmtpTls::Implicit);
        assert_eq!(smtp_tls_for_port(587), SmtpTls::StartTls);
        assert_eq!(smtp_tls_for_port(25), SmtpTls::StartTls);
        assert_eq!(smtp_tls_for_port(2525), SmtpTls::StartTls);
    }

    /// Écoute sur un port éphémère, accepte une connexion puis raccroche.
    /// Renvoie le port et un canal qui signale l'arrivée : ce qu'on teste
    /// est l'ARRIVÉE de la connexion, pas le dialogue SMTP — un faux
    /// serveur qui raccroche suffit, et rend le test hors-ligne.
    fn fake_smtp_server() -> (u16, mpsc::Receiver<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("port éphémère");
        let port = listener.local_addr().expect("adresse locale").port();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            if listener.accept().is_ok() {
                let _ = tx.send(());
            }
        });
        (port, rx)
    }

    fn connection_arrived(rx: &mpsc::Receiver<()>) -> bool {
        rx.recv_timeout(Duration::from_secs(5)).is_ok()
    }

    /// Régression (bug #3) : `connect_xoauth2` câblait `relay()` — TLS
    /// implicite sur 465 — et n'offrait aucun port. Le défaut jumeau de
    /// celui corrigé pour les mots de passe, invisible parce qu'un seul
    /// fournisseur était branché : Gmail écoute bien en 465, mais
    /// `smtp.office365.com` n'écoute qu'en 587/STARTTLS.
    #[test]
    fn xoauth2_connects_to_the_port_it_is_given() {
        let (port, arrived) = fake_smtp_server();
        // La connexion échoue forcément (le faux serveur raccroche) ;
        // seule son arrivée sur LE port demandé est en jeu.
        let _ = SmtpMailer::connect_xoauth2("127.0.0.1", port, "moi@exemple.fr", "jeton");
        assert!(
            connection_arrived(&arrived),
            "XOAUTH2 doit joindre le port demandé, pas un 465 câblé en dur"
        );
    }

    /// Le pendant pour le mot de passe : garde le correctif du bug #1 de
    /// régresser quand les deux chemins seront unifiés.
    #[test]
    fn password_connects_to_the_port_it_is_given() {
        let (port, arrived) = fake_smtp_server();
        let _ = SmtpMailer::connect_password("127.0.0.1", port, "moi@exemple.fr", "secret");
        assert!(
            connection_arrived(&arrived),
            "le mot de passe doit joindre le port demandé"
        );
    }

    #[test]
    fn malformed_stored_address_is_a_permanent_error() {
        let mut message = outbox_message(None);
        message.to = vec!["pas une adresse".to_string()];
        match build_message(&message) {
            Err(SendError::Permanent(_)) => {}
            Err(other) => panic!("attendu un refus permanent, obtenu {other:?}"),
            Ok(_) => panic!("attendu un refus permanent, obtenu un message construit"),
        }
    }
}
