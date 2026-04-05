mod app;
mod domain;
mod error;
mod gmail_client;
mod local_store;
mod mock;
mod secure_store;
mod sync_engine;

use app::DiscoveryApp;
use domain::{AppSnapshot, DraftDetail, DraftUpdateInput};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, State};

struct AppState {
    app: Mutex<DiscoveryApp>,
}

impl AppState {
    fn new() -> Self {
        Self {
            app: Mutex::new(DiscoveryApp::new()),
        }
    }
}

#[tauri::command]
fn load_app_state(state: State<'_, AppState>) -> Result<AppSnapshot, String> {
    state
        .app
        .lock()
        .map_err(|_| "application state lock poisoned".to_string())?
        .load_app_state()
        .map_err(Into::into)
}

#[tauri::command]
fn add_account(app: AppHandle, state: State<'_, AppState>) -> Result<AppSnapshot, String> {
    let guard = state
        .app
        .lock()
        .map_err(|_| "application state lock poisoned".to_string())?;
    guard.add_account().map_err::<String, _>(Into::into)?;
    let snapshot = guard.load_app_state().map_err::<String, _>(Into::into)?;
    let _ = app.emit("discovery://accounts-updated", &snapshot.accounts);
    Ok(snapshot)
}

#[tauri::command]
fn load_threads(state: State<'_, AppState>, mailbox_id: String) -> Result<AppSnapshot, String> {
    state
        .app
        .lock()
        .map_err(|_| "application state lock poisoned".to_string())?
        .load_threads(&mailbox_id)
        .map_err(Into::into)
}

#[tauri::command]
fn load_thread_detail(
    state: State<'_, AppState>,
    thread_id: String,
) -> Result<AppSnapshot, String> {
    state
        .app
        .lock()
        .map_err(|_| "application state lock poisoned".to_string())?
        .load_thread_detail(&thread_id)
        .map_err(Into::into)
}

#[tauri::command]
fn refresh_mailbox(
    app: AppHandle,
    state: State<'_, AppState>,
    mailbox_id: String,
) -> Result<AppSnapshot, String> {
    let snapshot = state
        .app
        .lock()
        .map_err(|_| "application state lock poisoned".to_string())?
        .refresh_mailbox(&mailbox_id)
        .map_err::<String, _>(Into::into)?;
    let _ = app.emit("discovery://sync-status", &snapshot.sync_status);
    Ok(snapshot)
}

#[tauri::command]
fn archive_thread(
    app: AppHandle,
    state: State<'_, AppState>,
    thread_id: String,
) -> Result<AppSnapshot, String> {
    let snapshot = state
        .app
        .lock()
        .map_err(|_| "application state lock poisoned".to_string())?
        .archive_thread(&thread_id)
        .map_err::<String, _>(Into::into)?;
    let _ = app.emit("discovery://threads-updated", &snapshot.threads);
    Ok(snapshot)
}

#[tauri::command]
fn mark_spam(
    app: AppHandle,
    state: State<'_, AppState>,
    thread_id: String,
) -> Result<AppSnapshot, String> {
    let snapshot = state
        .app
        .lock()
        .map_err(|_| "application state lock poisoned".to_string())?
        .mark_spam(&thread_id)
        .map_err::<String, _>(Into::into)?;
    let _ = app.emit("discovery://threads-updated", &snapshot.threads);
    Ok(snapshot)
}

#[tauri::command]
fn create_draft(state: State<'_, AppState>) -> Result<DraftDetail, String> {
    state
        .app
        .lock()
        .map_err(|_| "application state lock poisoned".to_string())?
        .create_draft()
        .map_err(Into::into)
}

#[tauri::command]
fn update_draft(
    app: AppHandle,
    state: State<'_, AppState>,
    input: DraftUpdateInput,
) -> Result<DraftDetail, String> {
    let draft = state
        .app
        .lock()
        .map_err(|_| "application state lock poisoned".to_string())?
        .update_draft(input)
        .map_err::<String, _>(Into::into)?;
    let _ = app.emit("discovery://draft-saved", &draft);
    Ok(draft)
}

#[tauri::command]
fn send_draft(
    app: AppHandle,
    state: State<'_, AppState>,
    draft_id: String,
) -> Result<AppSnapshot, String> {
    let snapshot = state
        .app
        .lock()
        .map_err(|_| "application state lock poisoned".to_string())?
        .send_draft(&draft_id)
        .map_err::<String, _>(Into::into)?;
    let _ = app.emit("discovery://draft-sent", &draft_id);
    Ok(snapshot)
}

pub fn run() {
    tauri::Builder::default()
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            add_account,
            load_app_state,
            load_threads,
            load_thread_detail,
            refresh_mailbox,
            archive_thread,
            mark_spam,
            create_draft,
            update_draft,
            send_draft
        ])
        .run(tauri::generate_context!())
        .expect("error while running Discovery");
}
