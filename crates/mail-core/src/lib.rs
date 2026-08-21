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
mod correspondants;
mod crash;
mod drafts;
mod echo;
mod envelope;
mod error;
mod imap_quoted;
mod nav;
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
pub use attachment::{Attachment, human_size};
pub use backfill::{
    BACKFILL_BATCH, BackfillReport, NO_HORIZON, THREAD_HEADER_BATCH, backfill_bodies,
    backfill_percent, backfill_recipients, backfill_thread_headers,
};
pub use body::load_body;
pub use compose::{
    Draft, compose, forward_subject, quote_forward, quote_forward_html, quote_reply,
    quote_reply_html, reply_all_split, reply_subject, reply_to,
};
pub use correspondants::Correspondant;
pub use crash::{CrashReport, RawPanic, redact};
pub use drafts::{
    DraftAttachmentFull, DraftAttachmentMeta, DraftAttachmentSaved, DraftContent, DraftPull,
    DraftSaved, MAX_ATTACHMENTS_BYTES, SavedDraft, plan_draft_pull,
};
pub use echo::DESTINATIONS_ECHO;
pub use envelope::{Envelope, Uid};
pub use error::Error;
pub use imap_quoted::{unescape_imap_quoted, unescape_imap_quoted_str};
pub use nav::{CanonicalFolders, NavCounts};
pub use notify::{Lang, Notification, arrivals_to_notify, notification_for};
pub use outbox::{OutboxAttachment, OutboxMessage, OutboxReport, OutboxState, flush_outbox};
pub use remote::{
    FetchedBody, Folder, FolderStatus, FolderWithStatus, MailServer, MailboxSnapshot,
    MessageRecipients, RemoteDraft, ThreadHeaders,
};
pub use search::WIDE_QUERY_THRESHOLD;
pub use store::{Account, AccountConfig, AdoptionProgress, Store, SyncState, UnifiedRow};
pub use sync::{
    RepereLocal, SYNC_BYTES_PER_MESSAGE, SyncEngine, SyncMode, SyncReport, disk_shortfall,
    faut_relever, sync_order, sync_percent,
};
pub use transport::{MailTransport, SendError};
