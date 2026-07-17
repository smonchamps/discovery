use std::fmt;

use crate::Error;

/// Adresse email validée : impossible d'en construire une invalide.
///
/// La validation est volontairement pragmatique (RFC 5321 simplifiée) :
/// une partie locale non vide, un seul `@`, un domaine contenant au moins
/// un point et ne commençant ni ne finissant par un point.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EmailAddress(String);

impl EmailAddress {
    /// Longueur maximale d'une adresse (RFC 5321).
    const MAX_LEN: usize = 254;

    pub fn parse(input: &str) -> Result<Self, Error> {
        let trimmed = input.trim();
        let invalid = || Error::InvalidEmailAddress(input.to_string());

        if trimmed.is_empty() || trimmed.len() > Self::MAX_LEN {
            return Err(invalid());
        }
        // Blancs, contrôles, séparateurs de listes et chevrons : soit de
        // l'injection d'en-têtes, soit une liste mal découpée. Les refuser
        // ici permet aussi de stocker des listes de destinataires avec un
        // séparateur sûr (boîte d'envoi, Phase 2).
        if trimmed
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || matches!(c, ',' | ';' | '<' | '>'))
        {
            return Err(invalid());
        }
        let (local, domain) = trimmed.split_once('@').ok_or_else(invalid)?;
        if local.is_empty() || domain.is_empty() || domain.contains('@') {
            return Err(invalid());
        }
        if !domain.contains('.') || domain.starts_with('.') || domain.ends_with('.') {
            return Err(invalid());
        }
        Ok(Self(trimmed.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for EmailAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_simple_address() {
        let address = EmailAddress::parse("alice@example.com").unwrap();
        assert_eq!(address.as_str(), "alice@example.com");
    }

    #[test]
    fn trims_surrounding_whitespace() {
        let address = EmailAddress::parse("  bob@example.org \n").unwrap();
        assert_eq!(address.as_str(), "bob@example.org");
    }

    #[test]
    fn accepts_subdomains_and_plus_tag() {
        assert!(EmailAddress::parse("a+tag@mail.example.co.uk").is_ok());
    }

    #[test]
    fn rejects_empty_input() {
        assert!(EmailAddress::parse("   ").is_err());
    }

    #[test]
    fn rejects_missing_at_sign() {
        assert!(EmailAddress::parse("alice.example.com").is_err());
    }

    #[test]
    fn rejects_multiple_at_signs() {
        assert!(EmailAddress::parse("a@b@example.com").is_err());
    }

    #[test]
    fn rejects_empty_local_part() {
        assert!(EmailAddress::parse("@example.com").is_err());
    }

    #[test]
    fn rejects_domain_without_dot() {
        assert!(EmailAddress::parse("alice@localhost").is_err());
    }

    #[test]
    fn rejects_domain_with_leading_or_trailing_dot() {
        assert!(EmailAddress::parse("alice@.example.com").is_err());
        assert!(EmailAddress::parse("alice@example.com.").is_err());
    }

    #[test]
    fn rejects_interior_whitespace_separators_and_angle_brackets() {
        for bad in [
            "a b@example.com",
            "a\tb@example.com",
            "a\nb@example.com",
            "a,b@example.com",
            "a;b@example.com",
            "<a@example.com>",
            "a@exam ple.com",
        ] {
            assert!(
                EmailAddress::parse(bad).is_err(),
                "{bad:?} devrait être refusée"
            );
        }
    }

    #[test]
    fn rejects_address_longer_than_254_chars() {
        let long_local = "a".repeat(250);
        assert!(EmailAddress::parse(&format!("{long_local}@example.com")).is_err());
    }

    #[test]
    fn displays_the_normalized_address() {
        let address = EmailAddress::parse(" carol@example.net ").unwrap();
        assert_eq!(address.to_string(), "carol@example.net");
    }
}
