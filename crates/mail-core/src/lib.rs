//! Noyau métier du client email.
//!
//! Ce crate contient le modèle du domaine et le moteur de synchronisation,
//! indépendants de toute UI et de tout protocole réseau : il ne connaît ni
//! Tauri, ni le web, ni IMAP. Sa seule frontière abstraite est le trait
//! [`MailServer`] ; l'adaptateur IMAP réel vit hors du noyau.

mod action;
mod address;
mod attachment;
mod backfill;
mod body;
mod compose;
mod crash;
mod drafts;
mod envelope;
mod error;
mod notify;
mod outbox;
mod remote;
mod search;
mod store;
mod sync;
#[cfg(test)]
mod test_support;
mod thread;
mod transport;

pub use action::{Action, PendingAction};
pub use address::EmailAddress;
pub use attachment::Attachment;
pub use backfill::{
    BACKFILL_BATCH, BackfillReport, NO_HORIZON, THREAD_HEADER_BATCH, backfill_bodies,
    backfill_thread_headers,
};
pub use body::load_body;
pub use compose::{Draft, compose, forward_subject, quote_forward, quote_reply, reply_subject};
pub use crash::{CrashReport, RawPanic, redact};
pub use drafts::{DraftContent, DraftPull, DraftSaved, SavedDraft, plan_draft_pull};
pub use envelope::{Envelope, Uid};
pub use error::Error;
pub use notify::{Notification, arrivals_to_notify, notification_for};
pub use outbox::{OutboxMessage, OutboxReport, OutboxState, flush_outbox};
pub use remote::{FetchedBody, Folder, MailServer, MailboxSnapshot, RemoteDraft, ThreadHeaders};
pub use store::{Account, AccountConfig, AdoptionProgress, Store, SyncState, UnifiedRow};
pub use sync::{
    SYNC_BYTES_PER_MESSAGE, SyncEngine, SyncMode, SyncReport, disk_shortfall, sync_order,
    sync_percent,
};
pub use transport::{MailTransport, SendError};
