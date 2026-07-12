//! Noyau métier du client email.
//!
//! Ce crate contient le modèle du domaine et le moteur de synchronisation,
//! indépendants de toute UI et de tout protocole réseau : il ne connaît ni
//! Tauri, ni le web, ni IMAP. Sa seule frontière abstraite est le trait
//! [`MailServer`] ; l'adaptateur IMAP réel vit hors du noyau.

mod action;
mod address;
mod body;
mod envelope;
mod error;
mod remote;
mod store;
mod sync;
#[cfg(test)]
mod test_support;

pub use action::{Action, PendingAction};
pub use address::EmailAddress;
pub use body::load_body;
pub use envelope::{Envelope, Uid};
pub use error::Error;
pub use remote::{MailServer, MailboxSnapshot};
pub use store::{Store, SyncState};
pub use sync::{SyncEngine, SyncMode, SyncReport};
