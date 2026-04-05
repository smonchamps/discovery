use crate::domain::*;
use chrono::{TimeZone, Utc};
use std::collections::HashMap;

pub fn seed_snapshot() -> AppSnapshot {
    let primary_account = AccountSummary {
        id: AccountId("acc_primary".into()),
        email: "smonchamps@gmail.com".into(),
        display_name: "Sebastien Monchamps".into(),
        color: "#3B82F6".into(),
        status: AccountStatus::Connected,
        unread_count: 3,
    };

    let work_account = AccountSummary {
        id: AccountId("acc_work".into()),
        email: "sebastien@discovery.mail".into(),
        display_name: "Discovery Work".into(),
        color: "#EF4444".into(),
        status: AccountStatus::Syncing,
        unread_count: 5,
    };

    let mailboxes = vec![
        MailboxRef {
            id: MailboxId("mail_unified".into()),
            account_id: None,
            kind: MailboxKind::UnifiedInbox,
            label: "Unified Inbox".into(),
            unread_count: 8,
        },
        account_mailbox(&primary_account.id, MailboxKind::Inbox, "mail_inbox_primary", "Inbox", 3),
        account_mailbox(
            &primary_account.id,
            MailboxKind::Drafts,
            "mail_drafts_primary",
            "Drafts",
            1,
        ),
        account_mailbox(&primary_account.id, MailboxKind::Sent, "mail_sent_primary", "Sent", 0),
        account_mailbox(
            &primary_account.id,
            MailboxKind::Archive,
            "mail_archive_primary",
            "Archive",
            0,
        ),
        account_mailbox(&primary_account.id, MailboxKind::Spam, "mail_spam_primary", "Spam", 0),
        account_mailbox(&work_account.id, MailboxKind::Inbox, "mail_inbox_work", "Inbox", 5),
        account_mailbox(&work_account.id, MailboxKind::Drafts, "mail_drafts_work", "Drafts", 0),
        account_mailbox(&work_account.id, MailboxKind::Sent, "mail_sent_work", "Sent", 0),
        account_mailbox(
            &work_account.id,
            MailboxKind::Archive,
            "mail_archive_work",
            "Archive",
            0,
        ),
        account_mailbox(&work_account.id, MailboxKind::Spam, "mail_spam_work", "Spam", 0),
    ];

    let threads = vec![
        ThreadSummary {
            id: ThreadId("thread_masterclass".into()),
            account_id: primary_account.id.clone(),
            mailbox_id: MailboxId("mail_inbox_primary".into()),
            subject: "Why you get overlooked at work (and how to fix it)".into(),
            snippet: "Build the influence, voice, and mindset that get you noticed.".into(),
            from: Participant {
                name: "MasterClass".into(),
                email: "news@masterclass.com".into(),
            },
            received_at: Utc.with_ymd_and_hms(2026, 4, 5, 10, 43, 0).unwrap(),
            is_unread: true,
            has_attachments: false,
            badge: "M".into(),
        },
        ThreadSummary {
            id: ThreadId("thread_stock".into()),
            account_id: work_account.id.clone(),
            mailbox_id: MailboxId("mail_inbox_work".into()),
            subject: "La Vente de Stock du mois vient d’ouvrir".into(),
            snippet: "On vous livre en quelques jours seulement.".into(),
            from: Participant {
                name: "ASPHALTE Homme".into(),
                email: "news@asphalte.com".into(),
            },
            received_at: Utc.with_ymd_and_hms(2026, 4, 5, 10, 2, 0).unwrap(),
            is_unread: true,
            has_attachments: false,
            badge: "AH".into(),
        },
        ThreadSummary {
            id: ThreadId("thread_apec".into()),
            account_id: work_account.id.clone(),
            mailbox_id: MailboxId("mail_inbox_work".into()),
            subject: "3 minutes pour évaluer l’APEC".into(),
            snippet: "Si vous avez des difficultés pour visualiser ce message, cliquez ici.".into(),
            from: Participant {
                name: "APEC".into(),
                email: "info@apec.fr".into(),
            },
            received_at: Utc.with_ymd_and_hms(2026, 4, 5, 6, 8, 0).unwrap(),
            is_unread: true,
            has_attachments: false,
            badge: "A".into(),
        },
        ThreadSummary {
            id: ThreadId("thread_spark".into()),
            account_id: primary_account.id.clone(),
            mailbox_id: MailboxId("mail_inbox_primary".into()),
            subject: "Utilisez Spark pour acquérir de meilleures habitudes".into(),
            snippet: "Centre de contrôle. Créez de meilleures habitudes email.".into(),
            from: Participant {
                name: "Team Spark".into(),
                email: "spark@readdle.com".into(),
            },
            received_at: Utc.with_ymd_and_hms(2026, 4, 5, 2, 20, 0).unwrap(),
            is_unread: false,
            has_attachments: false,
            badge: "S".into(),
        },
    ];

    let thread_detail = ThreadDetail {
        id: ThreadId("thread_masterclass".into()),
        account_id: primary_account.id.clone(),
        mailbox_id: MailboxId("mail_inbox_primary".into()),
        subject: "Why you get overlooked at work (and how to fix it)".into(),
        participants: vec![
            Participant {
                name: "MasterClass".into(),
                email: "news@masterclass.com".into(),
            },
            Participant {
                name: "Sebastien Monchamps".into(),
                email: "smonchamps@gmail.com".into(),
            },
        ],
        received_at: Utc.with_ymd_and_hms(2026, 4, 5, 10, 43, 0).unwrap(),
        badge: "M".into(),
        messages: vec![MessageView {
            id: MessageId("msg_masterclass".into()),
            from: Participant {
                name: "MasterClass".into(),
                email: "news@masterclass.com".into(),
            },
            to: vec![Participant {
                name: "Sebastien Monchamps".into(),
                email: "smonchamps@gmail.com".into(),
            }],
            sent_at: Utc.with_ymd_and_hms(2026, 4, 5, 10, 43, 0).unwrap(),
            html_body: Some("<div style='max-width:540px;margin:0 auto;background:#050505;color:#f5f5f5;font-family:Arial,sans-serif;border-radius:18px;overflow:hidden'><div style='padding:32px;text-align:center;background:#0b0b0c'><div style='font-size:28px;font-weight:700;letter-spacing:.06em;color:#ff3b6b'>MasterClass</div></div><div style='padding:32px'><h1 style='font-size:46px;line-height:1.02;margin:0 0 18px;font-weight:800;text-transform:uppercase'>How to turn your hard work into real recognition</h1><p style='font-size:18px;line-height:1.6;color:#dddddd;margin:0 0 24px'>You do the work. You have the ideas. With the right tools, your ambition becomes impossible to ignore.</p><a style='display:inline-block;padding:16px 24px;background:#ff3b6b;border-radius:14px;color:white;text-decoration:none;font-weight:700' href='https://www.masterclass.com'>Get MasterClass</a></div></div>".into()),
            text_body: "How to turn your hard work into real recognition.".into(),
            attachments: Vec::new(),
        }],
    };

    let draft = DraftDetail {
        envelope: DraftEnvelope {
            id: DraftId("draft_1".into()),
            account_id: primary_account.id.clone(),
            mailbox_id: MailboxId("mail_drafts_primary".into()),
            to: vec!["hello@example.com".into()],
            cc: Vec::new(),
            bcc: Vec::new(),
            subject: "Discovery alpha notes".into(),
            updated_at: Utc.with_ymd_and_hms(2026, 4, 5, 9, 40, 0).unwrap(),
        },
        content: DraftContent {
            html_body: Some("<p>Here is the latest Discovery alpha progress.</p>".into()),
            text_body: "Here is the latest Discovery alpha progress.".into(),
            attachments: Vec::new(),
        },
    };

    let sync_status = HashMap::from([
        (
            "acc_primary".to_string(),
            SyncStatus {
                state: SyncState::Idle,
                last_successful_sync_at: Some(Utc.with_ymd_and_hms(2026, 4, 5, 10, 42, 0).unwrap()),
                detail: Some("Mailbox is up to date.".into()),
            },
        ),
        (
            "acc_work".to_string(),
            SyncStatus {
                state: SyncState::Syncing,
                last_successful_sync_at: Some(Utc.with_ymd_and_hms(2026, 4, 5, 10, 37, 0).unwrap()),
                detail: Some("Syncing the latest inbox changes.".into()),
            },
        ),
    ]);

    AppSnapshot {
        accounts: vec![primary_account, work_account],
        mailboxes,
        selected_mailbox_id: MailboxId("mail_unified".into()),
        sync_status,
        all_threads: threads.clone(),
        threads,
        selected_thread_id: Some(ThreadId("thread_masterclass".into())),
        thread_detail: Some(thread_detail),
        drafts: vec![draft.envelope.clone()],
        active_draft: Some(draft),
    }
}

fn account_mailbox(
    account_id: &AccountId,
    kind: MailboxKind,
    mailbox_id: &str,
    label: &str,
    unread_count: u32,
) -> MailboxRef {
    MailboxRef {
        id: MailboxId(mailbox_id.into()),
        account_id: Some(account_id.clone()),
        kind,
        label: label.into(),
        unread_count,
    }
}
