mod app;
mod domain;
mod error;
mod gmail_client;
mod local_store;
mod mock;
mod secure_store;
mod sync_engine;

use app::DiscoveryApp;
use domain::{AppSnapshot, DraftDetail, DraftUpdateInput, GmailEnrollmentPhase, GmailEnrollmentStatus};
use error::DiscoveryError;
use gmail_client::GmailClient;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};

struct AppState {
    app: Arc<Mutex<DiscoveryApp>>,
}

impl AppState {
    fn new() -> Self {
        Self {
            app: Arc::new(Mutex::new(DiscoveryApp::new())),
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
fn start_gmail_enrollment(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<GmailEnrollmentStatus, String> {
    let gmail_client = GmailClient::new();
    let launch = match gmail_client.start_loopback_enrollment() {
        Ok(launch) => launch,
        Err(DiscoveryError::Configuration(message)) => {
            let status = GmailEnrollmentStatus {
                phase: GmailEnrollmentPhase::ConfigurationRequired,
                message,
                authorize_url: None,
                callback_url: None,
                enrolled_email: None,
            };
            let _ = app.emit("discovery://gmail-enrollment-updated", &status);
            return Ok(status);
        }
        Err(error) => return Err(error.into()),
    };

    let authorize_url = launch.authorize_url.clone();
    let callback_url = launch.listener.callback_url().to_string();
    gmail_client.open_authorize_url(&authorize_url)?;

    let initial_status = GmailEnrollmentStatus {
        phase: GmailEnrollmentPhase::WaitingForCallback,
        message: "Browser opened. Complete Google sign-in to add your Gmail account.".into(),
        authorize_url: Some(authorize_url.clone()),
        callback_url: Some(callback_url.clone()),
        enrolled_email: None,
    };
    let _ = app.emit("discovery://gmail-enrollment-updated", &initial_status);

    let app_handle = app.clone();
    let app_state = state.app.clone();
    let config = launch.config;
    let session = launch.session;
    let listener = launch.listener;
    let gmail_task = gmail_client.clone();
    tauri::async_runtime::spawn(async move {
        let callback = match tauri::async_runtime::spawn_blocking(move || listener.wait_for_callback()).await {
            Ok(result) => match result {
                Ok(callback) => callback,
                Err(error) => {
                    emit_enrollment_error(&app_handle, error);
                    return;
                }
            },
            Err(error) => {
                emit_enrollment_error(&app_handle, DiscoveryError::OAuth(error.to_string()));
                return;
            }
        };

        let exchanging_status = GmailEnrollmentStatus {
            phase: GmailEnrollmentPhase::ExchangingCode,
            message: "Google approved sign-in. Discovery is exchanging the authorization code.".into(),
            authorize_url: Some(authorize_url),
            callback_url: Some(callback_url),
            enrolled_email: None,
        };
        let _ = app_handle.emit("discovery://gmail-enrollment-updated", &exchanging_status);

        let completion = async {
            let token = gmail_task.exchange_code(&config, &session, &callback).await?;
            let refresh_token = token.refresh_token.ok_or_else(|| {
                DiscoveryError::OAuth(
                    "Google did not return a refresh token. Recreate the desktop OAuth client and ensure consent is requested."
                        .into(),
                )
            })?;
            let profile = gmail_task.fetch_profile(&token.access_token).await?;
            let snapshot = app_state
                .lock()
                .map_err(|_| DiscoveryError::Storage("application state lock poisoned".into()))?
                .connect_google_account(profile.clone(), refresh_token)?;
            Ok::<_, DiscoveryError>((profile.email, snapshot))
        }
        .await;

        match completion {
            Ok((email, snapshot)) => {
                let success_status = GmailEnrollmentStatus {
                    phase: GmailEnrollmentPhase::Success,
                    message: format!("Connected Gmail account {email}."),
                    authorize_url: None,
                    callback_url: None,
                    enrolled_email: Some(email),
                };
                let _ = app_handle.emit("discovery://gmail-enrollment-updated", &success_status);
                let _ = app_handle.emit("discovery://snapshot-updated", &snapshot);
            }
            Err(error) => emit_enrollment_error(&app_handle, error),
        }
    });

    Ok(initial_status)
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
    let _ = app.emit("discovery://snapshot-updated", &snapshot);
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
    let _ = app.emit("discovery://snapshot-updated", &snapshot);
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
    let _ = app.emit("discovery://snapshot-updated", &snapshot);
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
    let _ = app.emit("discovery://snapshot-updated", &snapshot);
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
    let _ = app.emit("discovery://snapshot-updated", &snapshot);
    Ok(snapshot)
}

fn emit_enrollment_error(app: &AppHandle, error: DiscoveryError) {
    let status = GmailEnrollmentStatus {
        phase: GmailEnrollmentPhase::Error,
        message: error.to_string(),
        authorize_url: None,
        callback_url: None,
        enrolled_email: None,
    };
    let _ = app.emit("discovery://gmail-enrollment-updated", &status);
}

pub fn run() {
    tauri::Builder::default()
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            add_account,
            start_gmail_enrollment,
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
