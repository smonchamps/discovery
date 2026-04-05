use thiserror::Error;

#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error("thread not found: {0}")]
    ThreadNotFound(String),
    #[error("draft not found: {0}")]
    DraftNotFound(String),
    #[error("mailbox not found: {0}")]
    MailboxNotFound(String),
    #[error("storage error: {0}")]
    Storage(String),
}

impl From<DiscoveryError> for String {
    fn from(value: DiscoveryError) -> Self {
        value.to_string()
    }
}
