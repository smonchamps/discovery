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

/// Sujet pré-rempli d'un transfert : « Fwd: » sans empilement, tolérant
/// aux variantes du terrain (« Tr : » d'Outlook français, « Fw: »…).
pub fn forward_subject(original: Option<&str>) -> String {
    match original
        .map(str::trim)
        .filter(|subject| !subject.is_empty())
    {
        Some(subject) => {
            let lower = subject.to_lowercase();
            let already = ["fwd:", "fwd :", "fw:", "fw :", "tr:", "tr :"]
                .iter()
                .any(|prefix| lower.starts_with(prefix));
            if already {
                subject.to_string()
            } else {
                format!("Fwd: {subject}")
            }
        }
        None => "Fwd:".to_string(),
    }
}

/// Bloc de citation d'une réponse, à placer SOUS le curseur (top-posting) :
/// une ligne d'attribution puis chaque ligne du texte préfixée de « > ».
pub fn quote_reply(sender: Option<&str>, date: Option<&str>, body_text: &str) -> String {
    if body_text.trim().is_empty() {
        return String::new();
    }
    let sender = sender.unwrap_or("(expéditeur inconnu)");
    let attribution = match date {
        Some(date) => format!("Le {date}, {sender} a écrit :"),
        None => format!("{sender} a écrit :"),
    };
    let quoted: String = body_text
        .lines()
        .map(|line| format!("> {line}\n"))
        .collect();
    format!("\n\n{attribution}\n{}", quoted.trim_end())
}

/// Bloc d'un transfert : l'en-tête d'origine (De/Date/Objet) puis le texte
/// tel quel — un transfert transmet, il ne commente pas ligne à ligne.
pub fn quote_forward(
    sender: Option<&str>,
    date: Option<&str>,
    subject: Option<&str>,
    body_text: &str,
) -> String {
    let mut block = String::from("\n\n---------- Message transféré ----------\n");
    block.push_str(&format!(
        "De : {}\n",
        sender.unwrap_or("(expéditeur inconnu)")
    ));
    if let Some(date) = date {
        block.push_str(&format!("Date : {date}\n"));
    }
    block.push_str(&format!(
        "Objet : {}\n\n{}",
        subject.unwrap_or("(sans objet)"),
        body_text.trim_end(),
    ));
    block
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

    #[test]
    fn forward_subject_prefixes_exactly_once() {
        assert_eq!(forward_subject(Some("Réunion")), "Fwd: Réunion");
        assert_eq!(forward_subject(Some("Fwd: Réunion")), "Fwd: Réunion");
        assert_eq!(forward_subject(Some("TR : Réunion")), "TR : Réunion");
        assert_eq!(forward_subject(Some("Fw: Réunion")), "Fw: Réunion");
        assert_eq!(forward_subject(None), "Fwd:");
    }

    /// Un « Re: » n'est pas un « Fwd: » : transférer une réponse préfixe.
    #[test]
    fn forward_subject_still_prefixes_a_reply_subject() {
        assert_eq!(forward_subject(Some("Re: Réunion")), "Fwd: Re: Réunion");
    }

    #[test]
    fn quote_reply_attributes_and_prefixes_every_line() {
        let quote = quote_reply(
            Some("Alice Martin"),
            Some("2026-07-17 10:23"),
            "première ligne\n\nseconde ligne",
        );
        assert!(quote.starts_with("\n\nLe 2026-07-17 10:23, Alice Martin a écrit :\n"));
        assert!(quote.contains("> première ligne"));
        assert!(quote.contains("> seconde ligne"));
        assert!(
            !quote.contains("\n\n>"),
            "les lignes vides restent citées : {quote:?}"
        );
    }

    #[test]
    fn quote_reply_degrades_gracefully_without_metadata() {
        let quote = quote_reply(None, None, "texte");
        assert!(quote.contains("(expéditeur inconnu) a écrit :"));
        assert!(quote.contains("> texte"));
    }

    #[test]
    fn quote_reply_of_empty_body_is_empty() {
        assert_eq!(quote_reply(Some("Alice"), None, "  \n "), "");
    }

    #[test]
    fn quote_forward_carries_original_headers_and_text() {
        let block = quote_forward(
            Some("Alice Martin"),
            Some("2026-07-17 10:23"),
            Some("Réunion"),
            "le corps\nsur deux lignes\n",
        );
        assert!(block.contains("---------- Message transféré ----------"));
        assert!(block.contains("De : Alice Martin"));
        assert!(block.contains("Date : 2026-07-17 10:23"));
        assert!(block.contains("Objet : Réunion"));
        assert!(block.ends_with("le corps\nsur deux lignes"));
    }

    #[test]
    fn quote_forward_uses_placeholders_for_missing_metadata() {
        let block = quote_forward(None, None, None, "corps");
        assert!(block.contains("De : (expéditeur inconnu)"));
        assert!(!block.contains("Date :"));
        assert!(block.contains("Objet : (sans objet)"));
    }
}
