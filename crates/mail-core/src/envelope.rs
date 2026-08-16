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
    /// Adresse brute de l'expéditeur (`mailbox@host`) — pour répondre,
    /// là où `sender` est la chaîne d'affichage (nom décodé).
    pub sender_address: Option<String>,
    /// Destinataires bruts À / Cc (`mailbox@host` chacun), tirés de la
    /// MÊME ENVELOPE que l'expéditeur — gratuits, jamais un octet de plus
    /// sur le réseau (R4, PLAN-RETOURS-MAIL). Ils servent à afficher « à X »
    /// dans un dossier d'envois (l'expéditeur y est SOI) et à « Répondre à
    /// tous » hors ligne. Vides quand l'ENVELOPE n'en porte pas.
    pub to_addrs: Vec<String>,
    pub cc_addrs: Vec<String>,
    /// `Message-ID` RFC 5322 — pour répondre dans le fil (`In-Reply-To`).
    pub message_id: Option<String>,
    /// `In-Reply-To` : l'ancêtre direct, tel que l'annonce l'expéditeur.
    ///
    /// Il arrive **gratuitement** avec l'ENVELOPE IMAP, dans les mêmes
    /// octets que le sujet et l'expéditeur. C'est ce qui rend le premier
    /// niveau de regroupement sans coût réseau ; `References`, lui, exige
    /// une passe séparée sur les en-têtes complets.
    pub in_reply_to: Option<String>,
    pub date: Option<DateTime<Utc>>,
    pub seen: bool,
    /// `\Flagged` — l'étoile chez Gmail.
    pub flagged: bool,
}
