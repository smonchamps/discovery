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

use lettre::message::header::ContentType;
use lettre::message::{Attachment as FilePart, Mailbox, MultiPart, SinglePart};
use lettre::transport::smtp::SmtpTransportBuilder;
use lettre::transport::smtp::authentication::{Credentials, Mechanism};
use lettre::{Message, SmtpTransport, Transport};
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
        match self.transport.send(&email) {
            Ok(_) => Ok(()),
            Err(err) if err.is_permanent() => Err(SendError::Permanent(err.to_string())),
            Err(err) => Err(SendError::Transient(err.to_string())),
        }
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
    if let Some(parent) = &message.in_reply_to {
        builder = builder
            .in_reply_to(parent.clone())
            .references(parent.clone());
    }
    if message.attachments.is_empty() {
        return builder
            .body(message.body_text.clone())
            .map_err(|err| SendError::Permanent(format!("construction du message : {err}")));
    }
    // multipart/mixed : le texte d'abord, puis chaque pièce telle que le
    // journal la porte (PJ-D2) — les octets viennent du journal, jamais
    // d'un fichier relu à l'envoi.
    let mut parts = MultiPart::mixed().singlepart(SinglePart::plain(message.body_text.clone()));
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
pub fn draft_bytes(
    from: &str,
    to_raw: &str,
    subject: &str,
    body: &str,
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
    if pieces.is_empty() {
        return builder
            .body(body.to_string())
            .map(|message| message.formatted())
            .map_err(|err| SendError::Permanent(format!("construction du brouillon : {err}")));
    }
    let mut parts = MultiPart::mixed().singlepart(SinglePart::plain(body.to_string()));
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
            subject: "Bonjour".to_string(),
            body_text: "Premier essai.\nDeuxième ligne.".to_string(),
            in_reply_to: in_reply_to.map(str::to_string),
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
            "Brouillon",
            "corps",
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
        let result = draft_bytes("moi@exemple.fr", "pas encore d'adresse", "s", "c", &[]);
        assert!(result.is_err(), "attendu : non poussable en l'état");
    }

    /// PJ-D6 : le reflet distant montre le brouillon ENTIER — pièces
    /// comprises, par le même constructeur de partie que l'envoi.
    #[test]
    fn draft_bytes_carries_pieces_as_multipart_mixed() {
        let raw = draft_bytes(
            "moi@exemple.fr",
            "valide@exemple.fr",
            "Brouillon",
            "corps",
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

    /// Un brouillon sans pièce reste mono-partie — le chemin historique
    /// ne paie pas le multipart.
    #[test]
    fn draft_bytes_without_pieces_stays_single_part() {
        let raw = draft_bytes("moi@exemple.fr", "valide@exemple.fr", "s", "c", &[])
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
