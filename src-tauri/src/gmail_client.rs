use crate::domain::{AccountSummary, MailboxKind};

#[derive(Clone, Debug)]
pub struct GmailAccountProfile {
    pub email: String,
    pub display_name: String,
}

#[derive(Clone, Debug)]
pub struct GmailLabel {
    pub id: String,
    pub kind: MailboxKind,
    pub name: String,
}

#[derive(Clone, Debug, Default)]
pub struct GmailClient;

impl GmailClient {
    pub fn new() -> Self {
        Self
    }

    pub fn start_oauth_device_flow(&self) -> String {
        "https://accounts.google.com/o/oauth2/v2/auth".to_string()
    }

    pub fn fetch_profile(&self, seed: &AccountSummary) -> GmailAccountProfile {
        GmailAccountProfile {
            email: seed.email.clone(),
            display_name: seed.display_name.clone(),
        }
    }

    pub fn system_labels(&self) -> Vec<GmailLabel> {
        vec![
            GmailLabel {
                id: "INBOX".to_string(),
                kind: MailboxKind::Inbox,
                name: "Inbox".to_string(),
            },
            GmailLabel {
                id: "DRAFT".to_string(),
                kind: MailboxKind::Drafts,
                name: "Drafts".to_string(),
            },
            GmailLabel {
                id: "SENT".to_string(),
                kind: MailboxKind::Sent,
                name: "Sent".to_string(),
            },
            GmailLabel {
                id: "SPAM".to_string(),
                kind: MailboxKind::Spam,
                name: "Spam".to_string(),
            },
            GmailLabel {
                id: "ARCHIVE".to_string(),
                kind: MailboxKind::Archive,
                name: "Archive".to_string(),
            },
        ]
    }
}
