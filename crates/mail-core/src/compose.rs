//! Composition d'un message sortant : la frontière de validation.
//!
//! Tout ce que NOUS produisons est strict (à l'inverse de l'affichage,
//! qui tolère le monde réel) : adresses validées une à une, sujet ramené
//! sur une seule ligne (aucune injection d'en-têtes possible), Message-ID
//! généré par nous AVANT l'envoi — c'est lui qui rend un envoi interrompu
//! corrélable au message réellement parti (règle « jamais de fantôme »).

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::address::EmailAddress;
use crate::error::Error;

/// Un message prêt à entrer dans la boîte d'envoi : tout y est validé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    /// Message-ID RFC 5322, chevrons compris, généré par nous.
    pub message_id: String,
    pub from: String,
    /// Destinataires validés — jamais vide.
    pub to: Vec<String>,
    pub subject: String,
    pub body_text: String,
    /// Message-ID du message auquel on répond (fil de discussion).
    pub in_reply_to: Option<String>,
}

/// Valide et assemble un brouillon prêt à journaliser.
///
/// `to_raw` accepte plusieurs adresses séparées par des virgules ou des
/// points-virgules ; chacune doit être valide, sinon tout est refusé
/// (fail fast à la frontière). `in_reply_to` est le Message-ID du message
/// d'origine tel que rapporté par le serveur — normalisé ici.
pub fn compose(
    from: &str,
    to_raw: &str,
    subject: &str,
    body_text: &str,
    in_reply_to: Option<&str>,
) -> Result<Draft, Error> {
    let from = EmailAddress::parse(from)?;
    let to: Vec<String> = to_raw
        .split([',', ';'])
        .filter(|part| !part.trim().is_empty())
        .map(|part| EmailAddress::parse(part).map(|address| address.to_string()))
        .collect::<Result<_, _>>()?;
    if to.is_empty() {
        return Err(Error::InvalidEmailAddress(to_raw.to_string()));
    }
    Ok(Draft {
        message_id: generate_message_id(&from),
        from: from.to_string(),
        to,
        subject: single_line(subject),
        body_text: body_text.to_string(),
        in_reply_to: in_reply_to.and_then(normalize_message_id),
    })
}

/// Sujet pré-rempli d'une réponse : « Re: » sans empilement — jamais de
/// « Re: Re: », y compris face au « RE : » à la française d'Outlook.
pub fn reply_subject(original: Option<&str>) -> String {
    match original
        .map(str::trim)
        .filter(|subject| !subject.is_empty())
    {
        Some(subject) => {
            let lower = subject.to_lowercase();
            if lower.starts_with("re:") || lower.starts_with("re :") {
                subject.to_string()
            } else {
                format!("Re: {subject}")
            }
        }
        None => "Re:".to_string(),
    }
}

/// Un sujet vit sur une seule ligne : tout caractère de contrôle devient
/// une espace — la voie de l'injection d'en-têtes est coupée à la source.
fn single_line(subject: &str) -> String {
    subject
        .trim()
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

/// Un Message-ID s'utilise chevrons compris (RFC 5322) ; certains serveurs
/// les omettent dans leurs réponses ENVELOPE — normaliser ici, une fois.
fn normalize_message_id(id: &str) -> Option<String> {
    let bare = id.trim().trim_matches(['<', '>']);
    if bare.is_empty() {
        None
    } else {
        Some(format!("<{bare}>"))
    }
}

/// Message-ID unique, généré AVANT toute tentative d'envoi.
///
/// `RandomState` est semé aléatoirement par le système à chaque instance :
/// combiné à l'horloge en nanosecondes, l'unicité est assurée sans
/// dépendance supplémentaire.
fn generate_message_id(from: &EmailAddress) -> String {
    let domain = from.as_str().rsplit('@').next().unwrap_or("localhost");
    let epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let random = RandomState::new().build_hasher().finish();
    format!("<{}.{:016x}@{}>", epoch.as_nanos(), random, domain)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compose_simple(to_raw: &str) -> Result<Draft, Error> {
        compose("moi@exemple.fr", to_raw, "Sujet", "Corps", None)
    }

    #[test]
    fn composes_multiple_recipients_from_comma_or_semicolon_list() {
        let draft = compose_simple(" a@exemple.fr , b@exemple.fr ; c@exemple.fr ").unwrap();
        assert_eq!(
            draft.to,
            vec!["a@exemple.fr", "b@exemple.fr", "c@exemple.fr"]
        );
        assert_eq!(draft.from, "moi@exemple.fr");
    }

    #[test]
    fn rejects_the_whole_draft_if_any_recipient_is_invalid() {
        assert!(compose_simple("a@exemple.fr, pas-une-adresse").is_err());
    }

    #[test]
    fn rejects_empty_recipient_list() {
        assert!(compose_simple("").is_err());
        assert!(compose_simple("  ,  ; ").is_err());
    }

    #[test]
    fn rejects_invalid_sender() {
        assert!(compose("pas-une-adresse", "a@exemple.fr", "s", "c", None).is_err());
    }

    #[test]
    fn generates_unique_well_formed_message_ids() {
        let first = compose_simple("a@exemple.fr").unwrap();
        let second = compose_simple("a@exemple.fr").unwrap();
        assert_ne!(first.message_id, second.message_id);
        assert!(first.message_id.starts_with('<'));
        assert!(first.message_id.ends_with("@exemple.fr>"));
    }

    /// L'injection d'en-têtes par le sujet est neutralisée à la source.
    #[test]
    fn subject_is_flattened_to_a_single_line() {
        let draft = compose(
            "moi@exemple.fr",
            "a@exemple.fr",
            "Alerte\r\nBcc: espion@mal.example",
            "Corps",
            None,
        )
        .unwrap();
        assert!(!draft.subject.contains('\r'));
        assert!(!draft.subject.contains('\n'));
        assert!(draft.subject.contains("Bcc: espion@mal.example"));
    }

    #[test]
    fn body_newlines_are_preserved_verbatim() {
        let draft = compose(
            "moi@exemple.fr",
            "a@exemple.fr",
            "s",
            "ligne 1\nligne 2",
            None,
        )
        .unwrap();
        assert_eq!(draft.body_text, "ligne 1\nligne 2");
    }

    #[test]
    fn normalizes_in_reply_to_with_angle_brackets() {
        let with = compose("moi@exemple.fr", "a@exemple.fr", "s", "c", Some("<id@x.y>")).unwrap();
        assert_eq!(with.in_reply_to.as_deref(), Some("<id@x.y>"));
        let without = compose("moi@exemple.fr", "a@exemple.fr", "s", "c", Some("id@x.y")).unwrap();
        assert_eq!(without.in_reply_to.as_deref(), Some("<id@x.y>"));
        let blank = compose("moi@exemple.fr", "a@exemple.fr", "s", "c", Some("  ")).unwrap();
        assert_eq!(blank.in_reply_to, None);
    }

    #[test]
    fn reply_subject_prefixes_exactly_once() {
        assert_eq!(reply_subject(Some("Réunion")), "Re: Réunion");
        assert_eq!(reply_subject(Some("Re: Réunion")), "Re: Réunion");
        assert_eq!(reply_subject(Some("RE : Réunion")), "RE : Réunion");
        assert_eq!(reply_subject(Some("  ")), "Re:");
        assert_eq!(reply_subject(None), "Re:");
    }
}
