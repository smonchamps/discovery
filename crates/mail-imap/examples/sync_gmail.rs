//! Validation de bout en bout : le `SyncEngine` de mail-core branché sur
//! l'adaptateur IMAP réel, contre votre compte Gmail.
//!
//! Prérequis : `GOOGLE_CLIENT_ID`/`GOOGLE_CLIENT_SECRET` dans l'environnement
//! et un compte déjà connecté via l'application Discovery (le refresh token
//! vit dans le Credential Manager, service « discovery-mail »).
//!
//! ```powershell
//! cargo run -p mail-imap --example sync_gmail --release
//! ```

use std::time::Instant;

use anyhow::Context;
use mail_auth::GmailAuth;
use mail_core::{Store, SyncEngine};
use mail_imap::ImapServer;

fn main() -> anyhow::Result<()> {
    let auth = GmailAuth::from_env().context("configuration OAuth")?;
    let account = match std::env::var("DISCOVERY_ACCOUNT") {
        Ok(email) => auth.authenticate_silent(&email),
        Err(_) => auth.authenticate_silent_legacy(),
    }
    .context("connectez d'abord un compte via Discovery (ou définissez DISCOVERY_ACCOUNT)")?;

    let timer = Instant::now();
    let mut server =
        ImapServer::connect_xoauth2("imap.gmail.com", 993, &account.email, &account.access_token)?;
    println!("Connecté ({}) en {:?}", account.email, timer.elapsed());

    let db_path = std::path::PathBuf::from("target/mail-imap-example.db");
    let mut store = Store::open(&db_path)?;
    let account_id = store.adopt_or_create_account(&account.email, "gmail")?;

    let timer = Instant::now();
    let report = SyncEngine::default().sync(&mut server, &mut store, account_id, "INBOX")?;
    println!(
        "Synchronisation {:?} : {} enveloppe(s) récupérée(s)/mise(s) à jour, {} supprimée(s), en {:?}",
        report.mode,
        report.fetched,
        report.deleted,
        timer.elapsed()
    );
    server.logout();

    let timer = Instant::now();
    let recent = store.recent(account_id, "INBOX", 0, 10)?;
    println!(
        "Les 10 plus récents (lus depuis SQLite en {:?}) :",
        timer.elapsed()
    );
    for envelope in recent {
        let marker = if envelope.seen { " " } else { "●" };
        let date = envelope
            .date
            .map(|d| d.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| "????-??-??".to_string());
        println!(
            "{marker} {date}  {:28}  {}",
            truncate(envelope.sender.as_deref().unwrap_or("(inconnu)"), 28),
            truncate(envelope.subject.as_deref().unwrap_or("(sans sujet)"), 58),
        );
    }
    Ok(())
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let cut: String = text.chars().take(max).collect();
        format!("{cut}…")
    }
}
