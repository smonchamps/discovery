//! Commandes Tauri : la passerelle entre l'UI et le noyau.
//!
//! Le travail bloquant (OAuth, IMAP) passe par `spawn_blocking` pour ne
//! jamais geler la fenêtre ; les lectures SQLite sont assez rapides pour
//! rester synchrones (~200 µs mesurées).

use std::path::{Path, PathBuf};
use std::time::Instant;

use mail_auth::{Authenticated, GmailAuth};
use mail_core::{Store, SyncEngine, SyncMode};
use mail_imap::ImapServer;
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::AppState;

const MAILBOX: &str = "INBOX";
const IMAP_HOST: &str = "imap.gmail.com";
const IMAP_PORT: u16 = 993;
const LIST_LIMIT_MAX: usize = 500;

#[derive(Serialize)]
pub struct AccountInfo {
    pub email: String,
}

#[derive(Serialize)]
pub struct SyncSummary {
    pub mode: String,
    pub fetched: usize,
    pub deleted: usize,
    pub total: u64,
    pub elapsed_ms: u64,
}

#[derive(Serialize)]
pub struct MessageRow {
    pub uid: u32,
    pub subject: String,
    pub sender: String,
    pub date: String,
    pub seen: bool,
}

#[tauri::command]
pub fn startup_report(state: State<'_, AppState>) -> String {
    format!(
        "fenêtre utilisable en {} ms",
        state.started_at.elapsed().as_millis()
    )
}

/// `interactive: false` = reconnexion silencieuse (refresh token du coffre) ;
/// `true` = parcours navigateur complet.
#[tauri::command]
pub async fn connect_account(
    state: State<'_, AppState>,
    interactive: bool,
) -> Result<AccountInfo, String> {
    let account = tauri::async_runtime::spawn_blocking(move || {
        let auth = GmailAuth::from_env().map_err(|err| err.to_string())?;
        let result = if interactive {
            auth.authenticate_interactive()
        } else {
            auth.authenticate_silent()
        };
        result.map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())??;

    let email = account.email.clone();
    *lock_account(&state)? = Some(account);
    Ok(AccountInfo { email })
}

#[tauri::command]
pub async fn sync_inbox(app: AppHandle, state: State<'_, AppState>) -> Result<SyncSummary, String> {
    let account = lock_account(&state)?
        .clone()
        .ok_or_else(|| "aucun compte connecté".to_string())?;
    let path = db_path(&app)?;

    let (summary, refreshed) =
        tauri::async_runtime::spawn_blocking(move || run_sync(&account, &path))
            .await
            .map_err(|err| err.to_string())??;

    if let Some(fresh) = refreshed {
        *lock_account(&state)? = Some(fresh);
    }
    Ok(summary)
}

#[tauri::command]
pub fn list_messages(app: AppHandle, limit: usize) -> Result<Vec<MessageRow>, String> {
    let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
    let rows = store
        .recent(MAILBOX, limit.min(LIST_LIMIT_MAX))
        .map_err(|err| err.to_string())?;
    Ok(rows
        .into_iter()
        .map(|envelope| MessageRow {
            uid: envelope.uid,
            subject: envelope
                .subject
                .unwrap_or_else(|| "(sans sujet)".to_string()),
            sender: envelope
                .sender
                .unwrap_or_else(|| "(expéditeur inconnu)".to_string()),
            date: envelope
                .date
                .map(|date| date.format("%Y-%m-%d").to_string())
                .unwrap_or_default(),
            seen: envelope.seen,
        })
        .collect())
}

fn run_sync(
    account: &Authenticated,
    db_path: &Path,
) -> Result<(SyncSummary, Option<Authenticated>), String> {
    let timer = Instant::now();
    let (mut server, refreshed) = connect_imap(account)?;
    let mut store = Store::open(db_path).map_err(|err| err.to_string())?;
    let report = SyncEngine::default()
        .sync(&mut server, &mut store, MAILBOX)
        .map_err(|err| err.to_string())?;
    server.logout();

    let total = store
        .sync_state(MAILBOX)
        .map_err(|err| err.to_string())?
        .map(|sync_state| store.count(sync_state.mailbox_id))
        .transpose()
        .map_err(|err| err.to_string())?
        .unwrap_or(0);

    let summary = SyncSummary {
        mode: match report.mode {
            SyncMode::Initial => "initiale",
            SyncMode::Incremental => "incrémentale",
        }
        .to_string(),
        fetched: report.fetched,
        deleted: report.deleted,
        total,
        elapsed_ms: timer.elapsed().as_millis() as u64,
    };
    Ok((summary, refreshed))
}

/// L'access token expire (~1 h) : en cas d'échec de connexion, une
/// ré-authentification silencieuse puis une seconde tentative.
fn connect_imap(account: &Authenticated) -> Result<(ImapServer, Option<Authenticated>), String> {
    match ImapServer::connect_xoauth2(IMAP_HOST, IMAP_PORT, &account.email, &account.access_token) {
        Ok(server) => Ok((server, None)),
        Err(_) => {
            let fresh = GmailAuth::from_env()
                .map_err(|err| err.to_string())?
                .authenticate_silent()
                .map_err(|err| err.to_string())?;
            let server = ImapServer::connect_xoauth2(
                IMAP_HOST,
                IMAP_PORT,
                &fresh.email,
                &fresh.access_token,
            )
            .map_err(|err| err.to_string())?;
            Ok((server, Some(fresh)))
        }
    }
}

fn lock_account<'a>(
    state: &'a State<'_, AppState>,
) -> Result<std::sync::MutexGuard<'a, Option<Authenticated>>, String> {
    state
        .account
        .lock()
        .map_err(|_| "état interne verrouillé".to_string())
}

fn db_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|err| err.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    Ok(dir.join("discovery.db"))
}
