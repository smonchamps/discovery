#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! Shell desktop : la fenêtre Tauri branchée sur le noyau.
//!
//! L'UI est « bête » (PLAN.md §3) : elle affiche l'état et émet des
//! intentions via les commandes de [`commands`] ; toute l'intelligence vit
//! dans mail-core / mail-imap / mail-smtp / mail-auth.

mod commands;
mod telemetry;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// L'avancement de la migration d'une base héritée, partagé entre la
/// passe (qui écrit) et l'UI (qui sonde en boucle et peut annuler).
/// Des atomiques, pas un Mutex : la passe écrit tous les 1 000 messages,
/// le sondage ne doit jamais la faire attendre.
#[derive(Default)]
pub(crate) struct MigrationShared {
    pub done: AtomicU64,
    pub total: AtomicU64,
    pub cancel: AtomicBool,
}

/// L'activité du cycle de synchronisation (PLAN-SYNCHRO E1), partagée
/// entre la boucle (qui écrit) et l'UI (qui sonde à la seconde pendant
/// le cycle). Des atomiques, comme la migration : le sondage ne doit
/// jamais faire attendre la boucle. Le compte courant — seul texte —
/// vit sous un Mutex écrit une fois par compte, jamais dans une boucle
/// chaude.
#[derive(Default)]
pub(crate) struct SyncShared {
    pub en_cours: AtomicBool,
    pub fait: AtomicU64,
    pub total: AtomicU64,
    pub compte: Mutex<String>,
    /// La boîte en cours de relève DANS le compte — terrain du
    /// 2026-08-13 : « 2/2 · compte » figé 7 minutes pendant le balayage
    /// des dossiers, sans aucune information. Vide entre deux boîtes.
    pub boite: Mutex<String>,
    /// L'étape sans boîte (clé de catalogue côté UI : `inventaire`,
    /// `fils`, `brouillons`) — second terrain du 2026-08-13 : « INBOX »
    /// couvrait quatre phases distinctes, l'observation était aveugle.
    /// Exclusif avec `boite` ; vide sinon.
    pub phase: Mutex<String>,
    /// Courrier d'INBOX déjà visible en base DANS le cycle courant
    /// (arrivées + retraits, cumulés compte après compte) — P1
    /// (PLAN-SYNCHRO) : la sonde le lit et recharge la liste dès la
    /// relève INBOX d'un compte soldée, sans attendre la fin du cycle.
    /// Un compteur sondé, pas un canal : le port UI reste R0-S5.
    pub courrier: AtomicU64,
}

pub(crate) struct AppState {
    pub started_at: Instant,
    /// Sessions des comptes connectés, par email (multi-comptes).
    pub accounts: Mutex<HashMap<String, mail_auth::AccountSession>>,
    /// Sérialise les vidanges de la boîte d'envoi : deux pompes
    /// concurrentes mettraient en quarantaine les envois l'une de l'autre.
    pub outbox_flush: Arc<Mutex<()>>,
    /// Sérialise la poussée des brouillons vers Gmail : deux poussées
    /// concurrentes créeraient des copies distantes en double.
    pub drafts_push: Arc<Mutex<()>>,
    /// Sérialise le rattrapage des corps : deux pompes concurrentes
    /// se disputeraient la bande passante et les mêmes messages.
    pub bodies_backfill: Arc<Mutex<()>>,
    /// Avancement et annulation de la migration visible (Phase 5).
    pub migration: Arc<MigrationShared>,
    /// Activité du cycle de synchronisation, pour la barre d'état (E1).
    pub sync_cycle: Arc<SyncShared>,
}

fn main() {
    let state = AppState {
        started_at: Instant::now(),
        accounts: Mutex::new(HashMap::new()),
        outbox_flush: Arc::new(Mutex::new(())),
        drafts_push: Arc::new(Mutex::new(())),
        bodies_backfill: Arc::new(Mutex::new(())),
        migration: Arc::new(MigrationShared::default()),
        sync_cycle: Arc::new(SyncShared::default()),
    };
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(state)
        // Installe le hook de panic et charge le consentement AVANT tout
        // le reste : un plantage precoce doit pouvoir etre capture (si
        // l'utilisateur a consenti). Ne touche jamais la base (ADR 0014).
        .setup(|app| {
            telemetry::init(app);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::startup_report,
            commands::connect_accounts,
            commands::add_account,
            commands::add_microsoft_account,
            commands::add_generic_account,
            commands::remove_account,
            commands::sync_inbox,
            commands::sync_inbox_light,
            commands::list_messages,
            commands::nav_snapshot,
            commands::list_category,
            commands::preview_catchup,
            commands::thread_messages,
            commands::search_messages,
            commands::message_body,
            commands::message_attachments,
            commands::save_attachment,
            commands::mark_seen,
            commands::mark_flagged,
            commands::archive_message,
            commands::list_folders,
            commands::move_message,
            commands::delete_message,
            commands::reply_context,
            commands::reply_all_context,
            commands::forward_context,
            commands::queue_send,
            commands::flush_outbox,
            commands::outbox_status,
            commands::outbox_requeue,
            commands::outbox_delete,
            commands::save_draft,
            commands::list_drafts,
            commands::delete_draft,
            commands::sync_drafts,
            commands::sync_progress,
            commands::sync_activity,
            commands::backfill_status,
            commands::backfill_bodies,
            commands::migration_check,
            commands::migration_run,
            commands::migration_progress,
            commands::migration_cancel,
            commands::update_check,
            commands::update_install,
            commands::app_version,
            commands::notif_pref_get,
            commands::notif_pref_set,
            commands::lang_get,
            commands::lang_set,
            telemetry::telemetry_consent_get,
            telemetry::telemetry_consent_set,
            telemetry::telemetry_pending,
            telemetry::telemetry_open_folder,
            telemetry::telemetry_selftest_panic,
        ])
        .run(tauri::generate_context!());
    if let Err(err) = result {
        eprintln!("échec du démarrage de la fenêtre : {err}");
        std::process::exit(1);
    }
}
