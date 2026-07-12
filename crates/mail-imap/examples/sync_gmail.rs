//! Validation de bout en bout : le `SyncEngine` de mail-core branché sur
//! l'adaptateur IMAP réel, contre votre compte Gmail.
//!
//! Prérequis identiques aux spikes : le refresh token du spike `oauth-gmail`
//! dans le Credential Manager, et `GOOGLE_CLIENT_ID`/`GOOGLE_CLIENT_SECRET`
//! dans l'environnement.
//!
//! ```powershell
//! cargo run -p mail-imap --example sync_gmail --release
//! ```

use std::time::Instant;

use anyhow::Context;
use mail_core::{Store, SyncEngine};
use mail_imap::ImapServer;
use oauth2::basic::BasicClient;
use oauth2::{AuthUrl, ClientId, ClientSecret, RefreshToken, TokenResponse, TokenUrl};

const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const USERINFO_URL: &str = "https://www.googleapis.com/oauth2/v2/userinfo";
const KEYRING_SERVICE: &str = "discovery-spike-oauth";
const KEYRING_USER: &str = "gmail-refresh-token";

fn main() -> anyhow::Result<()> {
    let (access_token, email) = gmail_access()?;

    let timer = Instant::now();
    let mut server = ImapServer::connect_xoauth2("imap.gmail.com", 993, &email, &access_token)?;
    println!("Connecté ({email}) en {:?}", timer.elapsed());

    let db_path = std::path::PathBuf::from("target/mail-imap-example.db");
    let mut store = Store::open(&db_path)?;

    let timer = Instant::now();
    let report = SyncEngine::default().sync(&mut server, &mut store, "INBOX")?;
    println!(
        "Synchronisation {:?} : {} enveloppe(s) récupérée(s)/mise(s) à jour, {} supprimée(s), en {:?}",
        report.mode,
        report.fetched,
        report.deleted,
        timer.elapsed()
    );
    server.logout();

    let timer = Instant::now();
    let recent = store.recent("INBOX", 10)?;
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

/// Access token via le refresh token stocké par le spike oauth-gmail.
fn gmail_access() -> anyhow::Result<(String, String)> {
    let client_id = std::env::var("GOOGLE_CLIENT_ID")
        .context("GOOGLE_CLIENT_ID manquante (voir spikes/oauth-gmail/README.md)")?;
    let client_secret = std::env::var("GOOGLE_CLIENT_SECRET")
        .context("GOOGLE_CLIENT_SECRET manquante (voir spikes/oauth-gmail/README.md)")?;
    let refresh = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .context("accès au Credential Manager")?
        .get_password()
        .context("aucun refresh token : lancez d'abord `cargo run -p spike-oauth-gmail`")?;

    let oauth = BasicClient::new(ClientId::new(client_id))
        .set_client_secret(ClientSecret::new(client_secret))
        .set_auth_uri(AuthUrl::new(AUTH_URL.to_string())?)
        .set_token_uri(TokenUrl::new(TOKEN_URL.to_string())?);
    let http = oauth2::reqwest::blocking::ClientBuilder::new()
        .redirect(oauth2::reqwest::redirect::Policy::none())
        .build()
        .context("construction du client HTTP")?;
    let tokens = oauth
        .exchange_refresh_token(&RefreshToken::new(refresh))
        .request(&http)
        .context("échange du refresh token (expiré ? relancez le spike oauth-gmail)")?;
    let access = tokens.access_token().secret().clone();

    let body = http
        .get(USERINFO_URL)
        .bearer_auth(&access)
        .send()
        .context("appel userinfo")?
        .error_for_status()
        .context("réponse userinfo en erreur")?
        .text()?;
    let email = serde_json::from_str::<serde_json::Value>(&body)
        .context("parsing userinfo")?
        .get("email")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .context("champ email absent de userinfo")?;
    Ok((access, email))
}
