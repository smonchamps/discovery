#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! Shell desktop (Phase 1) — première brique du squelette marchant.
//!
//! Ce binaire valide l'hypothèse Tauri 2 du plan contre ses budgets
//! (démarrage < 1 s, RAM < 200 Mo — PLAN.md §1) : la mesure du démarrage
//! est écrite dans un fichier du dossier temporaire à la première requête
//! du frontend, pour être lisible même en build release sans console.

use std::time::Instant;

struct StartClock(Instant);

/// Appelée par le frontend dès que le DOM est prêt : l'écart avec le début
/// de `main` est notre définition de « fenêtre utilisable ».
#[tauri::command]
fn startup_report(clock: tauri::State<'_, StartClock>) -> String {
    let elapsed_ms = clock.0.elapsed().as_millis();
    let core_status = match mail_core::EmailAddress::parse("demo@example.com") {
        Ok(address) => format!("noyau mail-core relié ({address})"),
        Err(err) => format!("noyau en erreur : {err}"),
    };
    let report = format!("{core_status} — fenêtre utilisable en {elapsed_ms} ms");
    let _ = std::fs::write(std::env::temp_dir().join("discovery-startup.txt"), &report);
    report
}

fn main() {
    let clock = StartClock(Instant::now());
    let result = tauri::Builder::default()
        .manage(clock)
        .invoke_handler(tauri::generate_handler![startup_report])
        .run(tauri::generate_context!());
    if let Err(err) = result {
        eprintln!("échec du démarrage de la fenêtre : {err}");
        std::process::exit(1);
    }
}
