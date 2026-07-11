/// Erreurs du domaine.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("adresse email invalide : {0:?}")]
    InvalidEmailAddress(String),
}
