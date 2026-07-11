//! Noyau métier du client email.
//!
//! Ce crate contient le modèle du domaine, indépendant de toute UI et de
//! tout protocole réseau : il ne connaît ni Tauri, ni le web, ni IMAP.
//! Les protocoles et le moteur de synchronisation le rejoindront au fil
//! des phases décrites dans `docs/PLAN.md`.

mod address;
mod error;

pub use address::EmailAddress;
pub use error::Error;
