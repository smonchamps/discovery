#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! Shell desktop : la fenêtre Tauri branchée sur le noyau.
//!
//! L'UI est « bête » (PLAN.md §3) : elle affiche l'état et émet des
//! intentions via les commandes de [`commands`] ; toute l'intelligence vit
//! dans mail-core / mail-imap / mail-auth.

mod commands;

use std::sync::Mutex;
use std::time::Instant;

pub(crate) struct AppState {
    pub started_at: Instant,
    pub account: Mutex<Option<mail_auth::Authenticated>>,
}

fn main() {
    let state = AppState {
        started_at: Instant::now(),
        account: Mutex::new(None),
    };
    let result = tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::startup_report,
            commands::connect_account,
            commands::sync_inbox,
            commands::list_messages,
            commands::message_body,
        ])
        .run(tauri::generate_context!());
    if let Err(err) = result {
        eprintln!("échec du démarrage de la fenêtre : {err}");
        std::process::exit(1);
    }
}
