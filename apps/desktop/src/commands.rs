//! Commandes Tauri : la passerelle entre l'UI et le noyau.
//!
//! Multi-comptes (Phase 3) : l'identité d'un message est `(compte, uid)`
//! — un UID seul ne suffit plus. Chaque opération réseau passe par la
//! connexion de SON compte ; les boucles (synchro, vidange, brouillons)
//! agrègent les comptes connectés. Le travail bloquant (OAuth, IMAP,
//! SMTP) passe par `spawn_blocking` pour ne jamais geler la fenêtre.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::Instant;

use mail_auth::{AccountSession, Authenticated, GenericCredentials, GmailAuth};
use mail_core::AccountConfig;
use mail_core::{Action, OutboxState, Store, SyncEngine};
use mail_imap::ImapServer;
use mail_smtp::SmtpMailer;
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::AppState;

const MAILBOX: &str = "INBOX";
const IMAP_HOST: &str = "imap.gmail.com";
const IMAP_PORT: u16 = 993;
const SMTP_HOST: &str = "smtp.gmail.com";
const LIST_LIMIT_MAX: usize = 500;
const SEARCH_LIMIT: usize = 50;

#[derive(Serialize)]
pub struct AccountInfo {
    pub id: i64,
    pub email: String,
}

#[derive(Serialize)]
pub struct SyncSummary {
    /// Comptes synchronisés avec succès.
    pub accounts: usize,
    pub fetched: usize,
    pub deleted: usize,
    pub replayed: usize,
    pub total: u64,
    pub elapsed_ms: u64,
    /// Échecs par compte — les autres comptes ne sont pas bloqués.
    pub errors: Vec<String>,
}

#[derive(Serialize)]
pub struct MessageRow {
    pub account_id: i64,
    pub account_email: String,
    pub uid: u32,
    pub subject: String,
    pub sender: String,
    pub date: String,
    pub seen: bool,
    pub flagged: bool,
}

#[tauri::command]
pub fn startup_report(state: State<'_, AppState>) -> String {
    format!(
        "fenêtre utilisable en {} ms",
        state.started_at.elapsed().as_millis()
    )
}

/// Connexion silencieuse de TOUS les comptes du registre. Registre vide
/// (base migrée de Phase 2) : l'entrée héritée du coffre peut révéler le
/// compte — elle est alors migrée et le compte en attente revendiqué.
#[tauri::command]
pub async fn connect_accounts(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<AccountInfo>, String> {
    let path = db_path(&app)?;

    // Crochet E2E : comptes factices (emails séparés par des virgules),
    // jetons invalides par construction — hors ligne garanti.
    if let Ok(list) = std::env::var("DISCOVERY_E2E_ACCOUNT") {
        let store = Store::open(&path).map_err(|err| err.to_string())?;
        let mut infos = Vec::new();
        for email in list.split(',').map(str::trim).filter(|e| !e.is_empty()) {
            let id = store
                .adopt_or_create_account(email, "gmail")
                .map_err(|err| err.to_string())?;
            lock_accounts(&state)?.insert(
                email.to_string(),
                AccountSession::Gmail(Authenticated {
                    email: email.to_string(),
                    access_token: "jeton-e2e-invalide".to_string(),
                }),
            );
            infos.push(AccountInfo {
                id,
                email: email.to_string(),
            });
        }
        return Ok(infos);
    }

    let accounts = {
        let store = Store::open(&path).map_err(|err| err.to_string())?;
        store.accounts().map_err(|err| err.to_string())?
    };

    let path_for_spawn = path.clone();
    let connected = tauri::async_runtime::spawn_blocking(move || {
        let mut list = Vec::new();
        let gmail_auth = GmailAuth::from_env();
        for account in accounts {
            match account.provider.as_str() {
                "gmail" => {
                    let auth = gmail_auth.as_ref().map_err(|err| err.to_string())?;
                    if let Ok(session) = auth.authenticate_silent(&account.email) {
                        list.push(AccountSession::Gmail(session));
                    }
                }
                "imap" => {
                    if let Ok(password) = mail_auth::fetch_generic_password(&account.email) {
                        let config = Store::open(&path_for_spawn)
                            .map_err(|err| err.to_string())?
                            .account_config(account.id)
                            .map_err(|err| err.to_string())?;
                        if let Some(session) =
                            build_generic_session(&account.email, &password, &config)
                        {
                            list.push(session);
                        }
                    }
                }
                _ => {}
            }
        }
        // Repli hérité Phase 2 : un compte Gmail sans provider explicite.
        if list.is_empty()
            && let Ok(auth) = gmail_auth
            && let Ok(account) = auth.authenticate_silent_legacy()
        {
            list.push(AccountSession::Gmail(account));
        }
        Ok::<_, String>(list)
    })
    .await
    .map_err(|err| err.to_string())??;

    let store = Store::open(&path).map_err(|err| err.to_string())?;
    let mut infos = Vec::new();
    for session in connected {
        let email = session.email().to_string();
        let provider = match &session {
            AccountSession::Gmail(_) => "gmail",
            AccountSession::Generic(_) => "imap",
        };
        let id = store
            .adopt_or_create_account(&email, provider)
            .map_err(|err| err.to_string())?;
        infos.push(AccountInfo {
            id,
            email: email.clone(),
        });
        lock_accounts(&state)?.insert(email, session);
    }
    Ok(infos)
}

/// Ajoute un compte — parcours navigateur complet, répétable à volonté.
#[tauri::command]
pub async fn add_account(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<AccountInfo, String> {
    let account = tauri::async_runtime::spawn_blocking(move || {
        GmailAuth::from_env()
            .map_err(|err| err.to_string())?
            .authenticate_interactive()
            .map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())??;

    let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
    let id = store
        .adopt_or_create_account(&account.email, "gmail")
        .map_err(|err| err.to_string())?;
    let info = AccountInfo {
        id,
        email: account.email.clone(),
    };
    lock_accounts(&state)?.insert(account.email.clone(), AccountSession::Gmail(account));
    Ok(info)
}

#[derive(serde::Deserialize)]
pub struct GenericAccountInput {
    pub email: String,
    pub username: Option<String>,
    pub password: String,
    pub imap_host: String,
    pub imap_port: u16,
    pub smtp_host: String,
    pub smtp_port: u16,
}

/// Ajoute un compte IMAP/SMTP générique : teste la connexion, stocke le
/// mot de passe dans le coffre, puis enregistre le compte en base.
#[tauri::command]
pub async fn add_generic_account(
    app: AppHandle,
    state: State<'_, AppState>,
    input: GenericAccountInput,
) -> Result<AccountInfo, String> {
    let username = input.username.unwrap_or_else(|| input.email.clone());
    let email = input.email.clone();
    let imap_host = input.imap_host.clone();
    let imap_port = input.imap_port;
    let smtp_host = input.smtp_host.clone();
    let smtp_port = input.smtp_port;
    let password = input.password.clone();

    // Test IMAP immédiat : on ne stocke rien tant que la connexion ne
    // fonctionne pas.
    tauri::async_runtime::spawn_blocking({
        let email = email.clone();
        let username = username.clone();
        let imap_host = imap_host.clone();
        let password = password.clone();
        move || {
            let server = mail_imap::ImapServer::connect_password(
                &imap_host, imap_port, &username, &password,
            )
            .map_err(|err| format!("connexion IMAP impossible : {err}"))?;
            server.logout();
            mail_auth::store_generic_password(&email, &password).map_err(|err| err.to_string())
        }
    })
    .await
    .map_err(|err| err.to_string())??;

    let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
    let id = store
        .create_generic_account(
            &email, &username, &imap_host, imap_port, &smtp_host, smtp_port,
        )
        .map_err(|err| err.to_string())?;

    let session = AccountSession::Generic(GenericCredentials {
        email: email.clone(),
        username: username.clone(),
        password,
        imap_host,
        imap_port,
        smtp_host,
        smtp_port,
    });
    lock_accounts(&state)?.insert(email.clone(), session);

    Ok(AccountInfo { id, email })
}

/// Construit une session générique à partir du mot de passe et de la
/// configuration stockée. Retourne `None` si la configuration est incomplète.
fn build_generic_session(
    email: &str,
    password: &str,
    config: &AccountConfig,
) -> Option<AccountSession> {
    Some(AccountSession::Generic(GenericCredentials {
        email: email.to_string(),
        username: config.username.clone().unwrap_or_else(|| email.to_string()),
        password: password.to_string(),
        imap_host: config.imap_host.clone()?,
        imap_port: config.imap_port?,
        smtp_host: config.smtp_host.clone()?,
        smtp_port: config.smtp_port?,
    }))
}

/// Synchronise TOUS les comptes connectés — l'échec d'un compte ne
/// bloque pas les autres (il est consigné dans le bilan).
#[tauri::command]
pub async fn sync_inbox(app: AppHandle, state: State<'_, AppState>) -> Result<SyncSummary, String> {
    let path = db_path(&app)?;
    let jobs = connected_jobs(&path, &state)?;
    let timer = Instant::now();

    let (accounts, fetched, deleted, replayed, errors, refreshed) =
        tauri::async_runtime::spawn_blocking(move || {
            let mut accounts = 0;
            let mut fetched = 0;
            let mut deleted = 0;
            let mut replayed = 0;
            let mut errors = Vec::new();
            let mut refreshed = Vec::new();
            for (account_id, session) in jobs {
                let email = session.email().to_string();
                match run_sync(&session, account_id, &path) {
                    Ok((report, fresh)) => {
                        accounts += 1;
                        fetched += report.fetched;
                        deleted += report.deleted;
                        replayed += report.replayed;
                        if let Some(fresh) = fresh {
                            refreshed.push(fresh);
                        }
                    }
                    Err(err) => errors.push(format!("{email} : {err}")),
                }
            }
            (accounts, fetched, deleted, replayed, errors, refreshed)
        })
        .await
        .map_err(|err| err.to_string())?;

    for fresh in refreshed {
        lock_accounts(&state)?.insert(fresh.email().to_string(), fresh);
    }
    let total = Store::open(&db_path(&app)?)
        .and_then(|store| store.unified_count(MAILBOX))
        .map_err(|err| err.to_string())?;

    Ok(SyncSummary {
        accounts,
        fetched,
        deleted,
        replayed,
        total,
        elapsed_ms: timer.elapsed().as_millis() as u64,
        errors,
    })
}

fn run_sync(
    session: &AccountSession,
    account_id: i64,
    db_path: &Path,
) -> Result<(mail_core::SyncReport, Option<AccountSession>), String> {
    let (mut server, refreshed) = connect_imap(session)?;
    let mut store = Store::open(db_path).map_err(|err| err.to_string())?;
    let report = SyncEngine::default()
        .sync(&mut server, &mut store, account_id, MAILBOX)
        .map_err(|err| err.to_string())?;
    server.logout();
    Ok((report, refreshed))
}

#[derive(Serialize)]
pub struct MessagePage {
    pub total: u64,
    pub offset: usize,
    pub rows: Vec<MessageRow>,
    pub elapsed_us: u64,
}

/// Mapping partagé entre la boîte unifiée et les résultats de recherche.
fn to_message_row(row: mail_core::UnifiedRow) -> MessageRow {
    MessageRow {
        account_id: row.account_id,
        account_email: row.account_email,
        uid: row.envelope.uid,
        subject: row
            .envelope
            .subject
            .unwrap_or_else(|| "(sans sujet)".to_string()),
        sender: row
            .envelope
            .sender
            .unwrap_or_else(|| "(expéditeur inconnu)".to_string()),
        date: row
            .envelope
            .date
            .map(|date| date.format("%Y-%m-%d").to_string())
            .unwrap_or_default(),
        seen: row.envelope.seen,
        flagged: row.envelope.flagged,
    }
}

/// Une page de la BOÎTE UNIFIÉE : tous les comptes fusionnés par date.
/// L'UI ne matérialise que les lignes visibles (virtualisation).
#[tauri::command]
pub fn list_messages(app: AppHandle, offset: usize, limit: usize) -> Result<MessagePage, String> {
    let timer = Instant::now();
    let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
    let total = store
        .unified_count(MAILBOX)
        .map_err(|err| err.to_string())?;
    let rows = store
        .unified_recent(MAILBOX, offset, limit.min(LIST_LIMIT_MAX))
        .map_err(|err| err.to_string())?
        .into_iter()
        .map(to_message_row)
        .collect();
    Ok(MessagePage {
        total,
        offset,
        rows,
        elapsed_us: timer.elapsed().as_micros() as u64,
    })
}

/// Recherche plein-texte sur tous les comptes. Le déclenchement à partir
/// de 3 caractères et le debounce sont de la responsabilité de l'UI.
#[tauri::command]
pub fn search_messages(app: AppHandle, query: String) -> Result<Vec<MessageRow>, String> {
    let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
    let rows = store
        .search(&query, SEARCH_LIMIT)
        .map_err(|err| err.to_string())?
        .into_iter()
        .map(to_message_row)
        .collect();
    Ok(rows)
}

#[derive(Serialize)]
pub struct BodyView {
    pub document: String,
    pub remote_images_blocked: usize,
}

/// Corps d'un message : cache local d'abord (aucun réseau), serveur du
/// compte sinon. Document auto-CSP chargé dans une iframe `sandbox` —
/// les trois couches de défense de la Phase 0.
#[tauri::command]
pub async fn message_body(
    app: AppHandle,
    state: State<'_, AppState>,
    account_id: i64,
    uid: u32,
    show_images: bool,
) -> Result<BodyView, String> {
    let html = raw_body(&app, &state, account_id, uid).await?;

    let policy = if show_images {
        mail_render::ImagePolicy::AllowRemote
    } else {
        mail_render::ImagePolicy::BlockRemote
    };
    let sanitized = mail_render::sanitize_with(&html, policy);
    Ok(BodyView {
        document: mail_render::email_document(&sanitized.html, policy),
        remote_images_blocked: sanitized.remote_images_blocked,
    })
}

fn fetch_body(
    session: &AccountSession,
    db_path: &Path,
    account_id: i64,
    uid: u32,
) -> Result<String, String> {
    let (mut server, _refreshed) = connect_imap(session)?;
    let mut store = Store::open(db_path).map_err(|err| err.to_string())?;
    let body = mail_core::load_body(&mut server, &mut store, account_id, MAILBOX, uid)
        .map_err(|err| err.to_string())?;
    server.logout();
    body.ok_or_else(|| "message introuvable sur le serveur".to_string())
}

/// Corps HTML brut d'un message : cache local d'abord (aucun réseau),
/// serveur du compte sinon — chemin partagé lecture/réponse/transfert.
async fn raw_body(
    app: &AppHandle,
    state: &State<'_, AppState>,
    account_id: i64,
    uid: u32,
) -> Result<String, String> {
    let path = db_path(app)?;
    let cached = Store::open(&path)
        .and_then(|store| store.body(account_id, MAILBOX, uid))
        .map_err(|err| err.to_string())?;
    match cached {
        Some(html) => Ok(html),
        None => {
            let session = auth_for(&path, state, account_id)?;
            tauri::async_runtime::spawn_blocking(move || {
                fetch_body(&session, &path, account_id, uid)
            })
            .await
            .map_err(|err| err.to_string())?
        }
    }
}

/// Archive : disparition locale immédiate + journalisation, le serveur
/// du compte suivra au prochain sync.
#[tauri::command]
pub fn archive_message(app: AppHandle, account_id: i64, uid: u32) -> Result<(), String> {
    queue_removal(&app, account_id, uid, Action::Archive)
}

/// Suppression : disparition locale immédiate + journalisation, mise à
/// la corbeille du serveur du compte au prochain sync.
#[tauri::command]
pub fn delete_message(app: AppHandle, account_id: i64, uid: u32) -> Result<(), String> {
    queue_removal(&app, account_id, uid, Action::Delete)
}

fn queue_removal(app: &AppHandle, account_id: i64, uid: u32, action: Action) -> Result<(), String> {
    let store = Store::open(&db_path(app)?).map_err(|err| err.to_string())?;
    let Some(state) = store
        .sync_state(account_id, MAILBOX)
        .map_err(|err| err.to_string())?
    else {
        return Ok(());
    };
    store
        .remove_local(state.mailbox_id, uid)
        .map_err(|err| err.to_string())?;
    store
        .enqueue_action(state.mailbox_id, uid, action)
        .map_err(|err| err.to_string())
}

/// Marque lu/non-lu : application locale immédiate (optimisme UI) +
/// journalisation — la prochaine synchro du compte rejoue vers le serveur.
#[tauri::command]
pub fn mark_seen(app: AppHandle, account_id: i64, uid: u32, seen: bool) -> Result<(), String> {
    let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
    let Some(state) = store
        .sync_state(account_id, MAILBOX)
        .map_err(|err| err.to_string())?
    else {
        return Ok(());
    };
    let changed = store
        .set_seen_local(state.mailbox_id, uid, seen)
        .map_err(|err| err.to_string())?;
    if changed {
        let action = if seen {
            Action::MarkSeen
        } else {
            Action::MarkUnseen
        };
        store
            .enqueue_action(state.mailbox_id, uid, action)
            .map_err(|err| err.to_string())?;
    }
    Ok(())
}

/// Étoile/désétoile : même contrat que lu/non-lu, même file rejouable.
#[tauri::command]
pub fn mark_flagged(
    app: AppHandle,
    account_id: i64,
    uid: u32,
    flagged: bool,
) -> Result<(), String> {
    let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
    let Some(state) = store
        .sync_state(account_id, MAILBOX)
        .map_err(|err| err.to_string())?
    else {
        return Ok(());
    };
    let changed = store
        .set_flagged_local(state.mailbox_id, uid, flagged)
        .map_err(|err| err.to_string())?;
    if changed {
        let action = if flagged {
            Action::MarkFlagged
        } else {
            Action::MarkUnflagged
        };
        store
            .enqueue_action(state.mailbox_id, uid, action)
            .map_err(|err| err.to_string())?;
    }
    Ok(())
}

// ---------------------------------------------------------------------
// Composer, répondre, envoyer — la boîte d'envoi (Phases 2-3).
// ---------------------------------------------------------------------

#[derive(Serialize)]
pub struct ComposeContext {
    pub account_id: i64,
    pub uid: u32,
    /// Vide pour un transfert : l'utilisateur choisit le destinataire.
    pub to: String,
    pub subject: String,
    /// Citation pré-remplie ; l'utilisateur écrit au-dessus (top-posting).
    pub body: String,
    /// `true` : l'envoi portera In-Reply-To (réponse dans le fil).
    pub reply: bool,
}

#[derive(Serialize)]
pub struct OutboxSummary {
    pub sent: usize,
    pub deferred: usize,
    pub rejected: usize,
    pub quarantined: usize,
    /// Restant en file après la vidange (tous comptes).
    pub queued: usize,
    /// Connexion SMTP impossible (hors ligne, token…) — la file attend.
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct OutboxEntry {
    pub id: i64,
    pub subject: String,
    pub to: String,
    pub state: String,
    pub attempts: u32,
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct OutboxStatus {
    pub queued: usize,
    pub interrupted: usize,
    pub rejected: usize,
    /// Tout sauf les envois aboutis, dans l'ordre d'émission.
    pub entries: Vec<OutboxEntry>,
}

/// Pré-remplissage d'une réponse : destinataire = adresse brute de
/// l'expéditeur, sujet « Re: » une seule fois, corps cité. La citation
/// est un confort : corps inaccessible = on répond sans elle.
#[tauri::command]
pub async fn reply_context(
    app: AppHandle,
    state: State<'_, AppState>,
    account_id: i64,
    uid: u32,
) -> Result<ComposeContext, String> {
    let envelope = envelope_of(&app, account_id, uid)?;
    let to = envelope
        .sender_address
        .clone()
        .ok_or_else(|| "adresse de l'expéditeur inconnue — resynchronisez la boîte".to_string())?;
    let body = match raw_body(&app, &state, account_id, uid).await {
        Ok(html) => mail_core::quote_reply(
            envelope.sender.as_deref(),
            quote_date(&envelope).as_deref(),
            &mail_render::body_text(&html),
        ),
        Err(_) => String::new(),
    };
    Ok(ComposeContext {
        account_id,
        uid,
        to,
        subject: mail_core::reply_subject(envelope.subject.as_deref()),
        body,
        reply: true,
    })
}

/// Pré-remplissage d'un transfert : sans corps, un transfert ne
/// transmettrait rien — ici l'échec est bloquant. Nouveau fil : pas
/// d'In-Reply-To. Les pièces jointes ne suivent pas encore (Phase 3).
#[tauri::command]
pub async fn forward_context(
    app: AppHandle,
    state: State<'_, AppState>,
    account_id: i64,
    uid: u32,
) -> Result<ComposeContext, String> {
    let envelope = envelope_of(&app, account_id, uid)?;
    let html = raw_body(&app, &state, account_id, uid).await?;
    Ok(ComposeContext {
        account_id,
        uid,
        to: String::new(),
        subject: mail_core::forward_subject(envelope.subject.as_deref()),
        body: mail_core::quote_forward(
            envelope.sender.as_deref(),
            quote_date(&envelope).as_deref(),
            envelope.subject.as_deref(),
            &mail_render::body_text(&html),
        ),
        reply: false,
    })
}

fn envelope_of(app: &AppHandle, account_id: i64, uid: u32) -> Result<mail_core::Envelope, String> {
    let store = Store::open(&db_path(app)?).map_err(|err| err.to_string())?;
    store
        .envelope(account_id, MAILBOX, uid)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "message introuvable".to_string())
}

/// Date au format de la ligne d'attribution d'une citation.
fn quote_date(envelope: &mail_core::Envelope) -> Option<String> {
    envelope
        .date
        .map(|date| date.format("%Y-%m-%d %H:%M").to_string())
}

/// Journalise l'envoi dans la boîte d'envoi du compte émetteur — AVANT
/// toute tentative réseau (règle « jamais d'envoi perdu »).
#[tauri::command]
pub fn queue_send(
    app: AppHandle,
    account_id: i64,
    to: String,
    subject: String,
    body: String,
    reply_to_uid: Option<u32>,
) -> Result<(), String> {
    let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
    let from = account_email(&store, account_id)?;
    let in_reply_to = reply_to_uid
        .and_then(|uid| store.envelope(account_id, MAILBOX, uid).ok().flatten())
        .and_then(|envelope| envelope.message_id);
    let draft = mail_core::compose(&from, &to, &subject, &body, in_reply_to.as_deref())
        .map_err(|err| err.to_string())?;
    store
        .enqueue_outbox(account_id, &draft)
        .map_err(|err| err.to_string())?;
    Ok(())
}

/// Vide les boîtes d'envoi de TOUS les comptes connectés — chacun par
/// SA connexion SMTP. Hors ligne = bilan, pas une erreur. Réentrance
/// interdite (verrou).
#[tauri::command]
pub async fn flush_outbox(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<OutboxSummary, String> {
    let path = db_path(&app)?;
    let jobs = connected_jobs(&path, &state)?;
    let lock = state.outbox_flush.clone();

    let (summary, refreshed) =
        tauri::async_runtime::spawn_blocking(move || run_flush_all(jobs, &path, &lock))
            .await
            .map_err(|err| err.to_string())??;

    for fresh in refreshed {
        lock_accounts(&state)?.insert(fresh.email().to_string(), fresh);
    }
    Ok(summary)
}

fn run_flush_all(
    jobs: Vec<(i64, AccountSession)>,
    db_path: &Path,
    lock: &Mutex<()>,
) -> Result<(OutboxSummary, Vec<AccountSession>), String> {
    let _guard = lock
        .lock()
        .map_err(|_| "vidange précédente interrompue".to_string())?;
    let mut store = Store::open(db_path).map_err(|err| err.to_string())?;

    // Un crash antérieur se constate même hors ligne : quarantaine d'abord.
    let mut summary = OutboxSummary {
        sent: 0,
        deferred: 0,
        rejected: 0,
        quarantined: store.quarantine_inflight().map_err(|err| err.to_string())?,
        queued: 0,
        error: None,
    };
    let mut refreshed_list = Vec::new();

    for (account_id, session) in jobs {
        if store
            .outbox_to_send(account_id)
            .map_err(|err| err.to_string())?
            .is_empty()
        {
            continue;
        }
        match connect_smtp(&session) {
            // Hors ligne : la file de ce compte survit telle quelle.
            Err(reason) => summary.error = Some(reason),
            Ok((mut mailer, refreshed)) => {
                let report = mail_core::flush_outbox(&mut mailer, &mut store, account_id)
                    .map_err(|err| err.to_string())?;
                summary.sent += report.sent;
                summary.deferred += report.deferred;
                summary.rejected += report.rejected;
                summary.quarantined += report.quarantined;
                if let Some(fresh) = refreshed {
                    refreshed_list.push(fresh);
                }
            }
        }
    }
    summary.queued = store
        .outbox_in_state(OutboxState::Queued)
        .map_err(|err| err.to_string())?
        .len();
    Ok((summary, refreshed_list))
}

/// L'état de la boîte d'envoi pour l'UI : tout ce qui n'est pas parti,
/// tous comptes confondus.
#[tauri::command]
pub fn outbox_status(app: AppHandle) -> Result<OutboxStatus, String> {
    let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
    let mut status = OutboxStatus {
        queued: 0,
        interrupted: 0,
        rejected: 0,
        entries: Vec::new(),
    };
    for message in store.outbox().map_err(|err| err.to_string())? {
        match message.state {
            OutboxState::Sent => continue,
            OutboxState::Queued | OutboxState::Sending => status.queued += 1,
            OutboxState::Interrupted => status.interrupted += 1,
            OutboxState::Rejected => status.rejected += 1,
        }
        status.entries.push(OutboxEntry {
            id: message.id,
            subject: message.subject,
            to: message.to.join(", "),
            state: message.state.as_str().to_string(),
            attempts: message.attempts,
            error: message.last_error,
        });
    }
    Ok(status)
}

/// Renvoi d'un envoi en quarantaine ou refusé : LA décision explicite
/// de l'utilisateur qu'exige la règle « jamais d'envoi fantôme ».
#[tauri::command]
pub fn outbox_requeue(app: AppHandle, id: i64) -> Result<(), String> {
    let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
    store.requeue_outbox(id).map_err(|err| err.to_string())
}

/// Abandon d'un envoi (décision utilisateur) ; l'historique `sent`
/// est préservé par le noyau.
#[tauri::command]
pub fn outbox_delete(app: AppHandle, id: i64) -> Result<(), String> {
    let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
    store.delete_outbox(id).map_err(|err| err.to_string())
}

// ---------------------------------------------------------------------
// Brouillons locaux + reflet Gmail par compte (Phases 2-3).
// ---------------------------------------------------------------------

#[derive(Serialize)]
pub struct DraftRow {
    pub id: i64,
    pub account_id: i64,
    pub to: String,
    pub subject: String,
    pub body: String,
    pub reply_to_uid: Option<u32>,
}

/// Sauvegarde un brouillon — texte brut, jamais validé : c'est un filet.
#[tauri::command]
pub fn save_draft(
    app: AppHandle,
    account_id: i64,
    id: Option<i64>,
    to: String,
    subject: String,
    body: String,
    reply_to_uid: Option<u32>,
) -> Result<i64, String> {
    let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
    store
        .save_draft(account_id, id, &to, &subject, &body, reply_to_uid)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_drafts(app: AppHandle) -> Result<Vec<DraftRow>, String> {
    let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
    Ok(store
        .drafts()
        .map_err(|err| err.to_string())?
        .into_iter()
        .map(|draft| DraftRow {
            id: draft.id,
            account_id: draft.account_id,
            to: draft.to_raw,
            subject: draft.subject,
            body: draft.body,
            reply_to_uid: draft.reply_to_uid,
        })
        .collect())
}

#[tauri::command]
pub fn delete_draft(app: AppHandle, id: i64) -> Result<(), String> {
    let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
    store.delete_draft(id).map_err(|err| err.to_string())
}

#[derive(Serialize)]
pub struct DraftSyncSummary {
    pub pushed: usize,
    pub purged: usize,
    /// Brouillons non poussables en l'état — ils restent locaux.
    pub kept_local: usize,
    /// Réseau indisponible — rien de changé, le cycle suivant retentera.
    pub error: Option<String>,
}

/// Reflète les brouillons de TOUS les comptes connectés dans leurs
/// dossiers Brouillons respectifs (poussée seule, v1). Sans travail,
/// aucun réseau. Réentrance interdite (verrou).
#[tauri::command]
pub async fn sync_drafts(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<DraftSyncSummary, String> {
    let path = db_path(&app)?;
    let jobs = connected_jobs(&path, &state)?;
    let lock = state.drafts_push.clone();

    let (summary, refreshed) =
        tauri::async_runtime::spawn_blocking(move || run_draft_sync_all(jobs, &path, &lock))
            .await
            .map_err(|err| err.to_string())??;

    for fresh in refreshed {
        lock_accounts(&state)?.insert(fresh.email().to_string(), fresh);
    }
    Ok(summary)
}

fn run_draft_sync_all(
    jobs: Vec<(i64, AccountSession)>,
    db_path: &Path,
    lock: &Mutex<()>,
) -> Result<(DraftSyncSummary, Vec<AccountSession>), String> {
    let _guard = lock
        .lock()
        .map_err(|_| "poussée précédente interrompue".to_string())?;
    let store = Store::open(db_path).map_err(|err| err.to_string())?;
    let mut summary = DraftSyncSummary {
        pushed: 0,
        purged: 0,
        kept_local: 0,
        error: None,
    };
    let mut refreshed_list = Vec::new();

    for (account_id, session) in jobs {
        let nothing_to_do = store
            .drafts_to_push(account_id)
            .map_err(|err| err.to_string())?
            .is_empty()
            && store
                .draft_tombstones(account_id)
                .map_err(|err| err.to_string())?
                .is_empty();
        if nothing_to_do {
            continue;
        }

        let (mut server, refreshed) = match connect_imap(&session) {
            Ok(pair) => pair,
            Err(reason) => {
                summary.error = Some(reason);
                continue;
            }
        };
        if let Some(fresh) = refreshed {
            refreshed_list.push(fresh);
        }

        // La garde des repères : UIDVALIDITY d'abord, toute purge ensuite.
        match server.drafts_uidvalidity() {
            Ok(validity) => {
                store
                    .align_drafts_uidvalidity(account_id, validity)
                    .map_err(|err| err.to_string())?;
            }
            Err(err) => {
                summary.error = Some(err.to_string());
                server.logout();
                continue;
            }
        }

        if !purge_draft_tombstones(&mut server, &store, account_id, &mut summary)? {
            server.logout();
            continue;
        }

        for draft in store
            .drafts_to_push(account_id)
            .map_err(|err| err.to_string())?
        {
            let bytes = match mail_smtp::draft_bytes(
                session.email(),
                &draft.to_raw,
                &draft.subject,
                &draft.body,
            ) {
                Ok(bytes) => bytes,
                // Pas poussable en l'état : le local reste la référence.
                Err(_) => {
                    summary.kept_local += 1;
                    continue;
                }
            };
            match server.append_draft(&bytes) {
                Ok(remote_uid) => {
                    store
                        .record_draft_pushed(draft.id, remote_uid, draft.updated_epoch)
                        .map_err(|err| err.to_string())?;
                    summary.pushed += 1;
                }
                Err(err) => {
                    summary.error = Some(err.to_string());
                    break;
                }
            }
        }

        // Les remplacements de CE cycle viennent de créer leurs
        // tombstones : purge immédiate — pas de copie double visible.
        if summary.error.is_none() {
            purge_draft_tombstones(&mut server, &store, account_id, &mut summary)?;
        }
        server.logout();
    }
    Ok((summary, refreshed_list))
}

/// Purge les copies distantes en tombstone d'UN compte. Retourne `false`
/// si le réseau a lâché — la dette reste enregistrée pour le cycle suivant.
fn purge_draft_tombstones(
    server: &mut ImapServer,
    store: &Store,
    account_id: i64,
    summary: &mut DraftSyncSummary,
) -> Result<bool, String> {
    for uid in store
        .draft_tombstones(account_id)
        .map_err(|err| err.to_string())?
    {
        match server.delete_draft_remote(uid) {
            Ok(()) => {
                store
                    .clear_draft_tombstone(account_id, uid)
                    .map_err(|err| err.to_string())?;
                summary.purged += 1;
            }
            Err(err) => {
                summary.error = Some(err.to_string());
                return Ok(false);
            }
        }
    }
    Ok(true)
}

// ---------------------------------------------------------------------
// Connexions et état partagé.
// ---------------------------------------------------------------------

/// Ouvre une connexion SMTP adaptée au type de compte. Pour Gmail, un
/// échec déclenche un refresh silencieux ; pour un compte générique, le
/// mot de passe est fixe (pas de retry possible).
fn connect_smtp(session: &AccountSession) -> Result<(SmtpMailer, Option<AccountSession>), String> {
    match session {
        AccountSession::Gmail(auth) => {
            match SmtpMailer::connect_xoauth2(SMTP_HOST, &auth.email, &auth.access_token) {
                Ok(mailer) => Ok((mailer, None)),
                Err(_) => {
                    let fresh = GmailAuth::from_env()
                        .map_err(|err| err.to_string())?
                        .authenticate_silent(&auth.email)
                        .map_err(|err| err.to_string())?;
                    let mailer =
                        SmtpMailer::connect_xoauth2(SMTP_HOST, &fresh.email, &fresh.access_token)
                            .map_err(|err| err.to_string())?;
                    Ok((mailer, Some(AccountSession::Gmail(fresh))))
                }
            }
        }
        AccountSession::Generic(creds) => {
            let mailer = SmtpMailer::connect_password(
                &creds.smtp_host,
                creds.smtp_port,
                &creds.username,
                &creds.password,
            )
            .map_err(|err| err.to_string())?;
            Ok((mailer, None))
        }
    }
}

/// Ouvre une connexion IMAP adaptée au type de compte. Pour Gmail, un
/// échec déclenche un refresh silencieux ; pour un compte générique, le
/// mot de passe est fixe.
fn connect_imap(session: &AccountSession) -> Result<(ImapServer, Option<AccountSession>), String> {
    match session {
        AccountSession::Gmail(auth) => {
            match ImapServer::connect_xoauth2(IMAP_HOST, IMAP_PORT, &auth.email, &auth.access_token)
            {
                Ok(server) => Ok((server, None)),
                Err(_) => {
                    let fresh = GmailAuth::from_env()
                        .map_err(|err| err.to_string())?
                        .authenticate_silent(&auth.email)
                        .map_err(|err| err.to_string())?;
                    let server = ImapServer::connect_xoauth2(
                        IMAP_HOST,
                        IMAP_PORT,
                        &fresh.email,
                        &fresh.access_token,
                    )
                    .map_err(|err| err.to_string())?;
                    Ok((server, Some(AccountSession::Gmail(fresh))))
                }
            }
        }
        AccountSession::Generic(creds) => {
            let server = ImapServer::connect_password(
                &creds.imap_host,
                creds.imap_port,
                &creds.username,
                &creds.password,
            )
            .map_err(|err| err.to_string())?;
            Ok((server, None))
        }
    }
}

/// Les comptes du registre qui sont connectés (session en mémoire) —
/// l'unité de travail des boucles synchro/vidange/brouillons.
fn connected_jobs(
    path: &Path,
    state: &State<'_, AppState>,
) -> Result<Vec<(i64, AccountSession)>, String> {
    let store = Store::open(path).map_err(|err| err.to_string())?;
    let known = store.accounts().map_err(|err| err.to_string())?;
    let connected = lock_accounts(state)?;
    Ok(known
        .into_iter()
        .filter_map(|account| {
            connected
                .get(&account.email)
                .cloned()
                .map(|session| (account.id, session))
        })
        .collect())
}

fn auth_for(
    path: &Path,
    state: &State<'_, AppState>,
    account_id: i64,
) -> Result<AccountSession, String> {
    let store = Store::open(path).map_err(|err| err.to_string())?;
    let email = account_email(&store, account_id)?;
    lock_accounts(state)?
        .get(&email)
        .cloned()
        .ok_or_else(|| format!("compte non connecté : {email}"))
}

fn account_email(store: &Store, account_id: i64) -> Result<String, String> {
    store
        .accounts()
        .map_err(|err| err.to_string())?
        .into_iter()
        .find(|account| account.id == account_id)
        .map(|account| account.email)
        .ok_or_else(|| "compte inconnu".to_string())
}

fn lock_accounts<'a>(
    state: &'a State<'_, AppState>,
) -> Result<MutexGuard<'a, HashMap<String, AccountSession>>, String> {
    state
        .accounts
        .lock()
        .map_err(|_| "état interne verrouillé".to_string())
}

fn db_path(app: &AppHandle) -> Result<PathBuf, String> {
    // Crochet E2E : base isolée fournie par le pilote de test — la vraie
    // base de l'utilisateur ne doit jamais être touchée par un test.
    if let Ok(path) = std::env::var("DISCOVERY_DB_PATH") {
        return Ok(PathBuf::from(path));
    }
    let dir = app.path().app_data_dir().map_err(|err| err.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    Ok(dir.join("discovery.db"))
}
