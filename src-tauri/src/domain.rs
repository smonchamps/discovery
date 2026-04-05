use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct AccountId(pub String);

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct MailboxId(pub String);

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ThreadId(pub String);

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct MessageId(pub String);

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct DraftId(pub String);

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountStatus {
    Connected,
    Syncing,
    Error,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MailboxKind {
    UnifiedInbox,
    Inbox,
    Drafts,
    Sent,
    Archive,
    Spam,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncState {
    Idle,
    Syncing,
    Degraded,
    Error,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub state: SyncState,
    pub last_successful_sync_at: Option<DateTime<Utc>>,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountSummary {
    pub id: AccountId,
    pub email: String,
    pub display_name: String,
    pub color: String,
    pub status: AccountStatus,
    pub unread_count: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailboxRef {
    pub id: MailboxId,
    pub account_id: Option<AccountId>,
    pub kind: MailboxKind,
    pub label: String,
    pub unread_count: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Participant {
    pub name: String,
    pub email: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub id: String,
    pub filename: String,
    pub media_type: String,
    pub size_label: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageView {
    pub id: MessageId,
    pub from: Participant,
    pub to: Vec<Participant>,
    pub sent_at: DateTime<Utc>,
    pub html_body: Option<String>,
    pub text_body: String,
    pub attachments: Vec<Attachment>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadSummary {
    pub id: ThreadId,
    pub account_id: AccountId,
    pub mailbox_id: MailboxId,
    pub subject: String,
    pub snippet: String,
    pub from: Participant,
    pub received_at: DateTime<Utc>,
    pub is_unread: bool,
    pub has_attachments: bool,
    pub badge: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadDetail {
    pub id: ThreadId,
    pub account_id: AccountId,
    pub mailbox_id: MailboxId,
    pub subject: String,
    pub participants: Vec<Participant>,
    pub received_at: DateTime<Utc>,
    pub badge: String,
    pub messages: Vec<MessageView>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftEnvelope {
    pub id: DraftId,
    pub account_id: AccountId,
    pub mailbox_id: MailboxId,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftContent {
    pub html_body: Option<String>,
    pub text_body: String,
    pub attachments: Vec<Attachment>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftDetail {
    pub envelope: DraftEnvelope,
    pub content: DraftContent,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSnapshot {
    pub accounts: Vec<AccountSummary>,
    pub mailboxes: Vec<MailboxRef>,
    pub selected_mailbox_id: MailboxId,
    pub sync_status: HashMap<String, SyncStatus>,
    pub all_threads: Vec<ThreadSummary>,
    pub threads: Vec<ThreadSummary>,
    pub selected_thread_id: Option<ThreadId>,
    pub thread_detail: Option<ThreadDetail>,
    pub drafts: Vec<DraftEnvelope>,
    pub active_draft: Option<DraftDetail>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftUpdateInput {
    pub draft_id: String,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub html_body: Option<String>,
    pub text_body: String,
}
