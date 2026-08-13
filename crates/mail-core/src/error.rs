/// Erreurs du domaine.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("adresse email invalide : {0:?}")]
    InvalidEmailAddress(String),

    #[error("stockage : {0}")]
    Storage(#[from] rusqlite::Error),

    /// Erreur remontée par une implémentation de [`crate::MailServer`]
    /// (réseau, protocole, authentification…).
    #[error("serveur : {0}")]
    Server(String),

    /// Donnée locale inattendue (base modifiée hors de l'application).
    #[error("donnée locale invalide : {0}")]
    Corrupt(String),

    /// La pièce ferait déborder le plafond d'un message (PJ-D3) : rien
    /// n'est joint — le refus se joue au geste, jamais à l'envoi. Les
    /// tailles permettent à la surface de dire la place restante.
    #[error(
        "pièce trop lourde : {name:?} ({size} octets) dépasse la place restante ({remaining} octets)"
    )]
    AttachmentOverBudget {
        name: String,
        size: u64,
        remaining: u64,
    },

    /// L'utilisateur a annulé la migration d'une base héritée pendant la
    /// passe d'adoption. Tout a été défait (`ROLLBACK`), `user_version`
    /// est inchangé : la passe entière se rejouera au prochain lancement.
    #[error("migration interrompue")]
    Interrupted,
}
