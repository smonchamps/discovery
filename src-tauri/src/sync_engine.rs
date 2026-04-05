use crate::domain::{AppSnapshot, SyncState};

#[derive(Clone, Debug, Default)]
pub struct SyncEngine;

impl SyncEngine {
    pub fn new() -> Self {
        Self
    }

    pub fn refresh_mailbox(&self, snapshot: &mut AppSnapshot, account_ids: &[String]) {
        for account_id in account_ids {
            if let Some(status) = snapshot.sync_status.get_mut(account_id) {
                status.state = SyncState::Idle;
                status.detail = Some("Mailbox refreshed from local cache.".into());
                status.last_successful_sync_at = Some(chrono::Utc::now());
            }
        }
    }
}
