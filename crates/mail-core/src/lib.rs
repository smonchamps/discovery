//! Noyau métier du client email.
//!
//! Ce crate contient le modèle du domaine et le moteur de synchronisation,
//! indépendants de toute UI et de tout protocole réseau : il ne connaît ni
//! Tauri, ni le web, ni IMAP. Sa seule frontière abstraite est le trait
//! [`MailServer`] ; l'adaptateur IMAP réel vit hors du noyau.

mod action;
mod address;
mod body;
mod compose;
mod drafts;
mod envelope;
mod error;
mod outbox;
mod remote;
mod store;
mod sync;
#[cfg(test)]
mod test_support;
mod transport;

pub use action::{Action, PendingAction};
pub use address::EmailAddress;
pub use body::load_body;
pub use compose::{Draft, compose, forward_subject, quote_forward, quote_reply, reply_subject};
pub use drafts::SavedDraft;
pub use envelope::{Envelope, Uid};
pub use error::Error;
pub use outbox::{OutboxMessage, OutboxReport, OutboxState, flush_outbox};
pub use remote::{MailServer, MailboxSnapshot};
pub use store::{Account, Store, SyncState, UnifiedRow};
pub use sync::{SyncEngine, SyncMode, SyncReport};
pub use transport::{MailTransport, SendError};
