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

use lettre::message::Mailbox;
use lettre::transport::smtp::authentication::{Credentials, Mechanism};
use lettre::{Message, SmtpTransport, Transport};
use mail_core::{MailTransport, OutboxMessage, SendError};

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

impl SmtpMailer {
    /// Connexion TLS (port 465) + authentification XOAUTH2, vérifiée
    /// immédiatement : on ne rend un transport que s'il sait envoyer.
    pub fn connect_xoauth2(host: &str, user: &str, access_token: &str) -> Result<Self, SendError> {
        let transport = SmtpTransport::relay(host)
            .map_err(|err| SendError::Transient(err.to_string()))?
            .authentication(vec![Mechanism::Xoauth2])
            .credentials(Credentials::new(user.to_string(), access_token.to_string()))
            .build();
        Self::test_transport(transport)
    }

    /// Connexion TLS + authentification par mot de passe (SMTP générique).
    /// Le `port` est honoré : 465 ouvre en TLS implicite, tout autre port
    /// (587 en tête) monte le TLS via STARTTLS. Vérifiée immédiatement.
    pub fn connect_password(
        host: &str,
        port: u16,
        user: &str,
        password: &str,
    ) -> Result<Self, SendError> {
        let builder = match smtp_tls_for_port(port) {
            SmtpTls::Implicit => SmtpTransport::relay(host),
            SmtpTls::StartTls => SmtpTransport::starttls_relay(host),
        }
        .map_err(|err| SendError::Transient(err.to_string()))?;
        let transport = builder
            .port(port)
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
    builder
        .body(message.body_text.clone())
        .map_err(|err| SendError::Permanent(format!("construction du message : {err}")))
}

/// Message RFC 5322 d'un brouillon, prêt pour un APPEND `\Draft` — la
/// poussée vers le dossier Brouillons Gmail (Phase 2).
///
/// Un brouillon porte du texte brut : les destinataires invalides sont
/// omis (une adresse à moitié tapée reste locale) ; si le message n'est
/// pas constructible en l'état, il n'est simplement pas poussé — le
/// local reste la référence, rien n'est perdu.
pub fn draft_bytes(
    from: &str,
    to_raw: &str,
    subject: &str,
    body: &str,
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
    builder
        .body(body.to_string())
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
    use mail_core::OutboxState;

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

    #[test]
    fn draft_bytes_keeps_valid_recipients_and_omits_the_rest() {
        let raw = draft_bytes(
            "moi@exemple.fr",
            "valide@exemple.fr, adresse-en-cours-de-fra",
            "Brouillon",
            "corps",
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
        let result = draft_bytes("moi@exemple.fr", "pas encore d'adresse", "s", "c");
        assert!(result.is_err(), "attendu : non poussable en l'état");
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
