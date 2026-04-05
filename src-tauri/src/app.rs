use crate::domain::*;
use crate::error::DiscoveryError;
use crate::gmail_client::GmailClient;
use crate::local_store::{InMemoryStore, LocalStore};
use crate::mock::seed_snapshot;
use crate::secure_store::{InMemorySecretStore, SecretStore};
use crate::sync_engine::SyncEngine;
use chrono::Utc;

pub struct DiscoveryApp {
    store: InMemoryStore,
    gmail_client: GmailClient,
    sync_engine: SyncEngine,
    secret_store: InMemorySecretStore,
}

impl DiscoveryApp {
    pub fn new() -> Self {
        let snapshot = seed_snapshot();
        Self {
            store: InMemoryStore::new(snapshot),
            gmail_client: GmailClient::new(),
            sync_engine: SyncEngine::new(),
            secret_store: InMemorySecretStore::default(),
        }
    }

    pub fn load_app_state(&self) -> Result<AppSnapshot, DiscoveryError> {
        self.store.load_snapshot()
    }

    pub fn add_account(&self) -> Result<AccountSummary, DiscoveryError> {
        let mut snapshot = self.store.load_snapshot()?;
        let account_index = snapshot.accounts.len() + 1;
        let account_id = format!("acc_demo_{account_index}");
        let account = AccountSummary {
            id: AccountId(account_id.clone()),
            email: format!("gmail{account_index}@example.com"),
            display_name: format!("Imported Gmail {account_index}"),
            color: "#22C55E".into(),
            status: AccountStatus::Syncing,
            unread_count: 0,
        };

        self.secret_store
            .save_refresh_token(&account_id, "placeholder-refresh-token")?;

        let oauth_url = self.gmail_client.start_oauth_device_flow();
        snapshot.sync_status.insert(
            account_id.clone(),
            SyncStatus {
                state: SyncState::Syncing,
                last_successful_sync_at: None,
                detail: Some(format!("OAuth handshake prepared at {oauth_url}")),
            },
        );

        snapshot.mailboxes.extend([
            mailbox(&account.id, MailboxKind::Inbox, format!("mail_inbox_{account_index}"), "Inbox"),
            mailbox(
                &account.id,
                MailboxKind::Drafts,
                format!("mail_drafts_{account_index}"),
                "Drafts",
            ),
            mailbox(&account.id, MailboxKind::Sent, format!("mail_sent_{account_index}"), "Sent"),
            mailbox(
                &account.id,
                MailboxKind::Archive,
                format!("mail_archive_{account_index}"),
                "Archive",
            ),
            mailbox(&account.id, MailboxKind::Spam, format!("mail_spam_{account_index}"), "Spam"),
        ]);
        snapshot.accounts.push(account.clone());
        self.store.save_snapshot(snapshot)?;
        Ok(account)
    }

    pub fn load_threads(&self, mailbox_id: &str) -> Result<AppSnapshot, DiscoveryError> {
        let mut snapshot = self.store.load_snapshot()?;
        if !snapshot.mailboxes.iter().any(|mailbox| mailbox.id.0 == mailbox_id) {
            return Err(DiscoveryError::MailboxNotFound(mailbox_id.into()));
        }

        snapshot.selected_mailbox_id = MailboxId(mailbox_id.into());
        snapshot.threads = visible_threads(&snapshot, mailbox_id);
        snapshot.selected_thread_id = snapshot.threads.first().map(|thread| thread.id.clone());
        snapshot.thread_detail = snapshot
            .selected_thread_id
            .as_ref()
            .and_then(|thread_id| snapshot_for_thread(&snapshot, &thread_id.0));
        self.store.save_snapshot(snapshot.clone())?;
        Ok(snapshot)
    }

    pub fn load_thread_detail(&self, thread_id: &str) -> Result<AppSnapshot, DiscoveryError> {
        let mut snapshot = self.store.load_snapshot()?;
        let detail = snapshot_for_thread(&snapshot, thread_id)
            .ok_or_else(|| DiscoveryError::ThreadNotFound(thread_id.into()))?;
        snapshot.selected_thread_id = Some(ThreadId(thread_id.into()));
        snapshot.thread_detail = Some(detail);
        self.store.save_snapshot(snapshot.clone())?;
        Ok(snapshot)
    }

    pub fn refresh_mailbox(&self, mailbox_id: &str) -> Result<AppSnapshot, DiscoveryError> {
        let mut snapshot = self.store.load_snapshot()?;
        let accounts = accounts_for_mailbox(&snapshot, mailbox_id);
        self.sync_engine.refresh_mailbox(&mut snapshot, &accounts);
        snapshot.selected_mailbox_id = MailboxId(mailbox_id.into());
        snapshot.threads = visible_threads(&snapshot, mailbox_id);
        self.store.save_snapshot(snapshot.clone())?;
        Ok(snapshot)
    }

    pub fn archive_thread(&self, thread_id: &str) -> Result<AppSnapshot, DiscoveryError> {
        self.move_thread(thread_id, MailboxKind::Archive)
    }

    pub fn mark_spam(&self, thread_id: &str) -> Result<AppSnapshot, DiscoveryError> {
        self.move_thread(thread_id, MailboxKind::Spam)
    }

    pub fn create_draft(&self) -> Result<DraftDetail, DiscoveryError> {
        let mut snapshot = self.store.load_snapshot()?;
        let draft = DraftDetail {
            envelope: DraftEnvelope {
                id: DraftId(format!("draft_{}", snapshot.drafts.len() + 1)),
                account_id: snapshot.accounts[0].id.clone(),
                mailbox_id: MailboxId("mail_drafts_primary".into()),
                to: Vec::new(),
                cc: Vec::new(),
                bcc: Vec::new(),
                subject: "New draft".into(),
                updated_at: Utc::now(),
            },
            content: DraftContent {
                html_body: Some("<p></p>".into()),
                text_body: String::new(),
                attachments: Vec::new(),
            },
        };

        snapshot.drafts.push(draft.envelope.clone());
        snapshot.active_draft = Some(draft.clone());
        self.store.save_snapshot(snapshot)?;
        Ok(draft)
    }

    pub fn update_draft(&self, input: DraftUpdateInput) -> Result<DraftDetail, DiscoveryError> {
        let mut snapshot = self.store.load_snapshot()?;
        let current = snapshot
            .active_draft
            .clone()
            .ok_or_else(|| DiscoveryError::DraftNotFound(input.draft_id.clone()))?;

        if current.envelope.id.0 != input.draft_id {
            return Err(DiscoveryError::DraftNotFound(input.draft_id));
        }

        let updated = DraftDetail {
            envelope: DraftEnvelope {
                id: DraftId(input.draft_id.clone()),
                account_id: current.envelope.account_id,
                mailbox_id: current.envelope.mailbox_id,
                to: input.to,
                cc: input.cc,
                bcc: input.bcc,
                subject: input.subject,
                updated_at: Utc::now(),
            },
            content: DraftContent {
                html_body: input.html_body,
                text_body: input.text_body,
                attachments: current.content.attachments,
            },
        };

        if let Some(existing) = snapshot
            .drafts
            .iter_mut()
            .find(|draft| draft.id.0 == updated.envelope.id.0)
        {
            *existing = updated.envelope.clone();
        }

        snapshot.active_draft = Some(updated.clone());
        self.store.save_snapshot(snapshot)?;
        Ok(updated)
    }

    pub fn send_draft(&self, draft_id: &str) -> Result<AppSnapshot, DiscoveryError> {
        let mut snapshot = self.store.load_snapshot()?;
        snapshot.drafts.retain(|draft| draft.id.0 != draft_id);
        if snapshot
            .active_draft
            .as_ref()
            .is_some_and(|draft| draft.envelope.id.0 == draft_id)
        {
            snapshot.active_draft = None;
        }

        self.store.save_snapshot(snapshot.clone())?;
        Ok(snapshot)
    }

    fn move_thread(
        &self,
        thread_id: &str,
        destination_kind: MailboxKind,
    ) -> Result<AppSnapshot, DiscoveryError> {
        let mut snapshot = self.store.load_snapshot()?;
        let destination = destination_mailbox_id(&snapshot, thread_id, destination_kind)?;
        let thread = snapshot
            .all_threads
            .iter_mut()
            .find(|entry| entry.id.0 == thread_id)
            .ok_or_else(|| DiscoveryError::ThreadNotFound(thread_id.into()))?;
        thread.mailbox_id = destination;

        snapshot.threads = visible_threads(&snapshot, &snapshot.selected_mailbox_id.0);
        snapshot.selected_thread_id = snapshot.threads.first().map(|entry| entry.id.clone());
        snapshot.thread_detail = snapshot
            .selected_thread_id
            .as_ref()
            .and_then(|id| snapshot_for_thread(&snapshot, &id.0));
        self.store.save_snapshot(snapshot.clone())?;
        Ok(snapshot)
    }
}

fn visible_threads(snapshot: &AppSnapshot, mailbox_id: &str) -> Vec<ThreadSummary> {
    let mut threads: Vec<ThreadSummary> = if mailbox_id == "mail_unified" {
        let inbox_mailbox_ids: Vec<&str> = snapshot
            .mailboxes
            .iter()
            .filter(|mailbox| mailbox.kind == MailboxKind::Inbox)
            .map(|mailbox| mailbox.id.0.as_str())
            .collect();
        snapshot
            .all_threads
            .iter()
            .filter(|thread| inbox_mailbox_ids.contains(&thread.mailbox_id.0.as_str()))
            .cloned()
            .collect()
    } else {
        snapshot
            .all_threads
            .iter()
            .filter(|thread| thread.mailbox_id.0 == mailbox_id)
            .cloned()
            .collect()
    };
    threads.sort_by(|left, right| right.received_at.cmp(&left.received_at));
    threads
}

fn snapshot_for_thread(snapshot: &AppSnapshot, thread_id: &str) -> Option<ThreadDetail> {
    snapshot
        .thread_detail
        .clone()
        .filter(|detail| detail.id.0 == thread_id)
        .or_else(|| {
            snapshot
                .all_threads
                .iter()
                .find(|thread| thread.id.0 == thread_id)
                .map(|thread| ThreadDetail {
                    id: thread.id.clone(),
                    account_id: thread.account_id.clone(),
                    mailbox_id: thread.mailbox_id.clone(),
                    subject: thread.subject.clone(),
                    participants: vec![
                        thread.from.clone(),
                        Participant {
                            name: "You".into(),
                            email: snapshot
                                .accounts
                                .iter()
                                .find(|account| account.id == thread.account_id)
                                .map(|account| account.email.clone())
                                .unwrap_or_default(),
                        },
                    ],
                    received_at: thread.received_at,
                    badge: thread.badge.clone(),
                    messages: vec![MessageView {
                        id: MessageId(format!("{}_message", thread.id.0)),
                        from: thread.from.clone(),
                        to: vec![Participant {
                            name: "You".into(),
                            email: snapshot
                                .accounts
                                .iter()
                                .find(|account| account.id == thread.account_id)
                                .map(|account| account.email.clone())
                                .unwrap_or_default(),
                        }],
                        sent_at: thread.received_at,
                        html_body: None,
                        text_body: thread.snippet.clone(),
                        attachments: Vec::new(),
                    }],
                })
        })
}

fn accounts_for_mailbox(snapshot: &AppSnapshot, mailbox_id: &str) -> Vec<String> {
    if mailbox_id == "mail_unified" {
        snapshot
            .accounts
            .iter()
            .map(|account| account.id.0.clone())
            .collect()
    } else {
        snapshot
            .mailboxes
            .iter()
            .find(|mailbox| mailbox.id.0 == mailbox_id)
            .and_then(|mailbox| mailbox.account_id.as_ref())
            .map(|account_id| vec![account_id.0.clone()])
            .unwrap_or_default()
    }
}

fn destination_mailbox_id(
    snapshot: &AppSnapshot,
    thread_id: &str,
    destination_kind: MailboxKind,
) -> Result<MailboxId, DiscoveryError> {
    let thread = snapshot
        .all_threads
        .iter()
        .find(|entry| entry.id.0 == thread_id)
        .ok_or_else(|| DiscoveryError::ThreadNotFound(thread_id.into()))?;

    snapshot
        .mailboxes
        .iter()
        .find(|mailbox| {
            mailbox.account_id.as_ref() == Some(&thread.account_id) && mailbox.kind == destination_kind
        })
        .map(|mailbox| mailbox.id.clone())
        .ok_or_else(|| DiscoveryError::MailboxNotFound(format!("{destination_kind:?}")))
}

fn mailbox(account_id: &AccountId, kind: MailboxKind, id: String, label: &str) -> MailboxRef {
    MailboxRef {
        id: MailboxId(id),
        account_id: Some(account_id.clone()),
        kind,
        label: label.into(),
        unread_count: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unified_inbox_is_sorted_by_latest_activity() {
        let snapshot = seed_snapshot();
        let threads = visible_threads(&snapshot, "mail_unified");
        let ids: Vec<String> = threads.into_iter().map(|thread| thread.id.0).collect();
        assert_eq!(
            ids,
            vec![
                "thread_masterclass".to_string(),
                "thread_stock".to_string(),
                "thread_apec".to_string(),
                "thread_spark".to_string()
            ]
        );
    }

    #[test]
    fn archive_moves_thread_to_matching_account_mailbox() {
        let app = DiscoveryApp::new();
        let snapshot = app.archive_thread("thread_masterclass").unwrap();
        let thread = snapshot
            .all_threads
            .iter()
            .find(|thread| thread.id.0 == "thread_masterclass")
            .unwrap();
        assert_eq!(thread.mailbox_id.0, "mail_archive_primary");
    }
}
