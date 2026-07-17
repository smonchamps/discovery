//! Le port d'envoi du noyau : le pendant SMTP de [`crate::MailServer`].
//!
//! La distinction transitoire/permanent est LA décision que le noyau
//! délègue à l'adaptateur : d'elle dépend le sort d'un message de la
//! boîte d'envoi (réessayer tel quel, ou s'arrêter et laisser
//! l'utilisateur trancher).

use crate::outbox::OutboxMessage;

pub trait MailTransport {
    /// Remet le message au serveur d'envoi. Ne retourner `Ok` que si le
    /// serveur a ACCEPTÉ le message en entier — c'est cet accusé qui
    /// autorise la boîte d'envoi à marquer l'envoi comme fait.
    fn send(&mut self, message: &OutboxMessage) -> Result<(), SendError>;
}

/// Échec d'envoi, classé selon la conduite à tenir.
#[derive(Debug, thiserror::Error)]
pub enum SendError {
    /// Réseau coupé, serveur injoignable ou saturé : l'envoi sera retenté
    /// tel quel à la prochaine vidange de la boîte d'envoi.
    #[error("échec transitoire : {0}")]
    Transient(String),

    /// Refus définitif du serveur (destinataire inexistant, message
    /// rejeté) : réessayer ne servirait à rien — l'utilisateur décide.
    #[error("refus permanent : {0}")]
    Permanent(String),
}
