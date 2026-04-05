use thiserror::Error;

#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error("thread not found: {0}")]
    ThreadNotFound(String),
    #[error("draft not found: {0}")]
    DraftNotFound(String),
    #[error("mailbox not found: {0}")]
    MailboxNotFound(String),
    #[error("configuration error: {0}")]
    Configuration(String),
    #[error("oauth error: {0}")]
    OAuth(String),
    #[error("network error: {0}")]
    Network(String),
    #[error("browser error: {0}")]
    Browser(String),
    #[error("storage error: {0}")]
    Storage(String),
}

impl From<DiscoveryError> for String {
    fn from(value: DiscoveryError) -> Self {
        value.to_string()
    }
}
