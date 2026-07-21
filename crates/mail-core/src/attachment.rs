//! Pièces jointes : ce qu'on sait d'un fichier AVANT de le télécharger.
//!
//! Le modèle suit la même règle que le reste du produit : les métadonnées
//! sont locales et gratuites, les octets se paient à la demande.
//!
//! Elles ne coûtent aucun aller-retour réseau supplémentaire : le corps
//! d'un message est déjà rapatrié en entier ([ADR 0007](../../../docs/adr/0007-rattrapage-des-corps.md)),
//! et ces métadonnées se lisent dans les mêmes octets. Les **octets** de
//! la pièce, eux, ne sont jamais stockés : à 62 Ko par corps le budget
//! disque tient, il ne tiendrait pas en y ajoutant les fichiers.

/// Une pièce jointe telle qu'on peut la décrire sans l'avoir téléchargée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attachment {
    /// Rang de la pièce dans le message, dans l'ordre où le MIME la livre.
    ///
    /// C'est la **clé de re-téléchargement** : rejouer la même extraction
    /// sur le même message redonne le même rang. Volontairement PAS le
    /// numéro de partie IMAP — l'arithmétique des `BODY[2.1.3]` est une
    /// source de bugs classique, et on n'en a pas besoin ici.
    pub index: usize,
    /// Nom de fichier, décodé (RFC 2047) et assaini à l'enregistrement.
    pub name: String,
    pub mime: String,
    /// Taille des octets DÉCODÉS — celle que l'utilisateur reconnaît,
    /// pas celle de la source base64.
    pub size: u64,
}

impl Attachment {
    /// Taille lisible, à l'usage de l'UI.
    pub fn human_size(&self) -> String {
        const KO: u64 = 1024;
        const MO: u64 = KO * 1024;
        match self.size {
            0..=1023 => format!("{} o", self.size),
            n if n < MO => format!("{:.0} Ko", n as f64 / KO as f64),
            n => format!("{:.1} Mo", n as f64 / MO as f64),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sized(size: u64) -> Attachment {
        Attachment {
            index: 0,
            name: "f".to_string(),
            mime: "application/pdf".to_string(),
            size,
        }
    }

    #[test]
    fn human_size_changes_unit_where_it_becomes_readable() {
        assert_eq!(sized(0).human_size(), "0 o");
        assert_eq!(sized(1023).human_size(), "1023 o");
        assert_eq!(sized(1024).human_size(), "1 Ko");
        assert_eq!(sized(1_048_576).human_size(), "1.0 Mo");
        assert_eq!(sized(2_600_000).human_size(), "2.5 Mo");
    }
}
