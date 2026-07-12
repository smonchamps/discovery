use chrono::{DateTime, Utc};

/// Identifiant IMAP d'un message au sein d'une boîte (RFC 3501).
pub type Uid = u32;

/// Enveloppe d'un message : les métadonnées suffisantes pour afficher une
/// liste sans jamais télécharger le corps (principe « enveloppes d'abord »).
///
/// `sender` est une chaîne d'affichage brute et non une [`crate::EmailAddress`]
/// validée : un client mail doit afficher ce qui existe, y compris les
/// expéditeurs malformés du monde réel. La validation stricte est réservée
/// aux adresses que NOUS produisons (composition, Phase 2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Envelope {
    pub uid: Uid,
    pub subject: Option<String>,
    pub sender: Option<String>,
    pub date: Option<DateTime<Utc>>,
    pub seen: bool,
}
