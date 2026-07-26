//! Télémétrie de crash — capture LOCALE des panics, opt-in (ADR 0014).
//!
//! Destination : un fichier par plantage dans `app_data_dir/crashes/`.
//! **Aucun réseau, aucun tiers** — l'app montre les rapports, l'utilisateur
//! décide de les envoyer. Le contenu est rédigé par [`mail_core::redact`],
//! qui écarte le message du panic (seul vecteur de donnée personnelle) et
//! ne garde que des artefacts de code.
//!
//! **Deux règles dures :**
//! 1. Le panic hook ne touche JAMAIS la base (elle est peut-être la cause
//!    du panic, ou tient un verrou empoisonné) : consentement en fichier,
//!    lu au démarrage dans un atomique, rapport écrit en `std::fs` pur.
//! 2. Le hook ne doit jamais paniquer à son tour (un panic pendant un
//!    panic = `abort`) : tout y est enveloppé et non-`unwrap`.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use mail_core::{RawPanic, redact};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

/// Consentement effectif, lu par le panic hook. Un atomique et non la
/// base : le hook doit rester cheap et sûr même si SQLite est en cause.
static ENABLED: AtomicBool = AtomicBool::new(false);

/// Numéro d'ordre des rapports d'une même exécution : il rend le nom de
/// fichier unique. Un plantage sur le thread principal en produit DEUX
/// (le panic d'origine, puis le `cannot unwind` de la frontière FFI),
/// dans la même seconde — sans ce compteur, le second écraserait le
/// premier, le seul utile. Trouvé au terrain (ADR 0014).
static SEQ: AtomicU32 = AtomicU32::new(0);

/// Le crochet E2E : sous test, jamais de bandeau ni d'écriture réelle
/// (mêmes étanchéités que le reste, passation §7.5).
fn is_e2e() -> bool {
    std::env::var("DISCOVERY_DB_PATH").is_ok()
}

/// Le dossier de base des données de l'app — où vivent le consentement et
/// les rapports. En E2E, co-localisé avec la base jetable.
fn base_dir(app: &AppHandle) -> Result<PathBuf, String> {
    if let Ok(db) = std::env::var("DISCOVERY_DB_PATH") {
        return Ok(PathBuf::from(db)
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from(".")));
    }
    app.path().app_data_dir().map_err(|err| err.to_string())
}

fn crashes_dir(base: &Path) -> PathBuf {
    base.join("crashes")
}

fn consent_file(base: &Path) -> PathBuf {
    base.join("telemetry.json")
}

#[derive(Serialize, Deserialize)]
struct ConsentFile {
    crash_reports: bool,
}

/// À appeler une fois au démarrage (dans `.setup`) : charge le
/// consentement et installe le panic hook.
pub fn init(app: &tauri::App) {
    let Ok(base) = base_dir(app.handle()) else {
        return;
    };
    let _ = std::fs::create_dir_all(&base);
    // Charge le consentement persisté dans l'atomique. Absent = jamais
    // demandé = désactivé : rien ne s'écrit tant que l'utilisateur n'a
    // pas dit oui.
    if let Ok(text) = std::fs::read_to_string(consent_file(&base))
        && let Ok(c) = serde_json::from_str::<ConsentFile>(&text)
    {
        ENABLED.store(c.crash_reports, Ordering::Relaxed);
    }
    install_hook(crashes_dir(&base), app.package_info().version.to_string());
}

/// Installe le hook. Il chaîne le hook par défaut : un panic garde son
/// comportement normal (trace sur stderr), on ajoute juste l'écriture du
/// rapport quand le consentement est actif.
fn install_hook(dir: PathBuf, app_version: String) {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if ENABLED.load(Ordering::Relaxed) {
            // Un panic dans le hook aborterait le process : on borne.
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                write_report(&dir, &app_version, info);
            }));
        }
        default_hook(info);
    }));
}

fn write_report(dir: &Path, app_version: &str, info: &std::panic::PanicHookInfo<'_>) {
    let message = payload_message(info);
    // Un panic sur le thread principal traverse la frontière FFI de
    // WebView2 (nounwind) et déclenche un SECOND panic « cannot unwind »
    // qui aborte. Ce second panic pointe le runtime, pas le bug
    // d'origine — que le premier panic a déjà rapporté. On ne l'écrit
    // donc pas.
    if is_secondary_nounwind(&message) {
        return;
    }
    let raw = RawPanic {
        message,
        location: info
            .location()
            .map(|loc| format!("{}:{}", loc.file(), loc.line())),
        backtrace: std::backtrace::Backtrace::force_capture()
            .to_string()
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect(),
        app_version: app_version.to_string(),
        os: os_label(),
        timestamp: now_iso8601(),
    };
    let _ = persist(dir, raw);
}

/// Rédige puis écrit le rapport sur disque. Séparé de [`write_report`]
/// pour être TESTABLE : `PanicHookInfo` ne se construit pas à la main, et
/// ce qui compte n'est pas que la rédaction soit correcte en mémoire mais
/// que les octets écrits ne portent aucune donnée personnelle.
fn persist(dir: &Path, raw: RawPanic) -> std::io::Result<PathBuf> {
    // C'est ICI que la donnée personnelle est écartée — avant toute
    // écriture. Ce qui suit ne manipule plus que le rapport rédigé.
    let report = redact(raw);

    #[derive(Serialize)]
    struct ReportFile<'a> {
        app_version: &'a str,
        os: &'a str,
        location: Option<&'a str>,
        backtrace: &'a [String],
        timestamp: &'a str,
    }

    std::fs::create_dir_all(dir)?;
    // ':' est interdit dans un nom de fichier Windows ; le compteur `SEQ`
    // rend le nom unique même pour deux plantages dans la même seconde.
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let name = format!("crash-{}-{seq}.json", report.timestamp.replace(':', "-"));
    let path = dir.join(name);
    let payload = ReportFile {
        app_version: &report.app_version,
        os: &report.os,
        location: report.location.as_deref(),
        backtrace: &report.backtrace,
        timestamp: &report.timestamp,
    };
    let json = serde_json::to_string_pretty(&payload).map_err(std::io::Error::other)?;
    std::fs::write(&path, json)?;
    Ok(path)
}

/// Un panic secondaire du runtime — « panic in a function that cannot
/// unwind », déclenché quand un panic traverse une frontière FFI. Il ne
/// désigne pas le bug d'origine ; on l'écarte. Filtre au mieux : si le
/// message du runtime change un jour, on écrit un rapport de trop
/// (jamais un de moins), et le compteur `SEQ` empêche tout écrasement.
fn is_secondary_nounwind(message: &str) -> bool {
    message.contains("cannot unwind")
}

fn payload_message(info: &std::panic::PanicHookInfo<'_>) -> String {
    let payload = info.payload();
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "panic (charge non textuelle)".to_string()
    }
}

fn os_label() -> String {
    format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)
}

fn now_iso8601() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

// --- Commandes Tauri -------------------------------------------------

/// État du consentement pour l'UI : `unset` déclenche la demande opt-in,
/// une seule fois. En E2E, toujours `disabled` — pas de bandeau en test.
#[tauri::command]
pub fn telemetry_consent_get(app: AppHandle) -> String {
    if is_e2e() {
        return "disabled".to_string();
    }
    let Ok(base) = base_dir(&app) else {
        return "disabled".to_string();
    };
    match std::fs::read_to_string(consent_file(&base)) {
        Ok(text) => match serde_json::from_str::<ConsentFile>(&text) {
            Ok(c) if c.crash_reports => "enabled".to_string(),
            Ok(_) => "disabled".to_string(),
            Err(_) => "unset".to_string(),
        },
        Err(_) => "unset".to_string(),
    }
}

/// Pose le consentement (opt-in ou refus), et l'applique immédiatement au
/// hook via l'atomique.
#[tauri::command]
pub fn telemetry_consent_set(app: AppHandle, enabled: bool) -> Result<(), String> {
    ENABLED.store(enabled, Ordering::Relaxed);
    let base = base_dir(&app)?;
    std::fs::create_dir_all(&base).map_err(|err| err.to_string())?;
    let json = serde_json::to_string(&ConsentFile {
        crash_reports: enabled,
    })
    .map_err(|err| err.to_string())?;
    std::fs::write(consent_file(&base), json).map_err(|err| err.to_string())
}

/// Combien de rapports attendent d'être envoyés.
#[tauri::command]
pub fn telemetry_pending(app: AppHandle) -> u32 {
    if is_e2e() {
        return 0;
    }
    let Ok(base) = base_dir(&app) else {
        return 0;
    };
    std::fs::read_dir(crashes_dir(&base))
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
                .count() as u32
        })
        .unwrap_or(0)
}

/// Ouvre le dossier des rapports dans l'explorateur — de quoi les
/// retrouver et les envoyer soi-même (destination locale, ADR 0014).
#[tauri::command]
pub fn telemetry_open_folder(app: AppHandle) -> Result<(), String> {
    let base = base_dir(&app)?;
    let dir = crashes_dir(&base);
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    std::process::Command::new("explorer")
        .arg(&dir)
        .spawn()
        .map_err(|err| err.to_string())?;
    Ok(())
}

/// Provoque un panic pour VÉRIFIER la capture au terrain (ADR 0014 §5).
///
/// Deux définitions : en **debug** elle panique, en **release** elle
/// refuse — le binaire livré ne peut pas paniquer sur commande. Le
/// message porte une fausse donnée personnelle : le rapport écrit doit
/// prouver qu'elle a disparu. À invoquer depuis la console de la WebView
/// (`window.__TAURI__.core.invoke('telemetry_selftest_panic')`).
#[cfg(debug_assertions)]
#[tauri::command]
pub fn telemetry_selftest_panic() -> Result<(), String> {
    panic!("selftest telemetrie: faux@exemple.fr — sujet « secret »")
}

#[cfg(not(debug_assertions))]
#[tauri::command]
pub fn telemetry_selftest_panic() -> Result<(), String> {
    Err("indisponible hors build debug".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// La garantie qui compte : les OCTETS ÉCRITS sur disque ne portent
    /// aucune donnée du message de panic. Le test de `mail-core` prouve la
    /// rédaction en mémoire ; celui-ci prouve le fichier réel, sérialisé.
    #[test]
    fn le_fichier_ecrit_ne_contient_aucune_donnee_du_message() {
        let dir = std::env::temp_dir().join(format!("disc-telemetry-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        let raw = RawPanic {
            message: "boom: victime@exemple.fr — sujet « Dossier medical confidentiel »"
                .to_string(),
            location: Some("apps/desktop/src/commands.rs:1".to_string()),
            backtrace: vec!["discovery_desktop::commands::quelque_chose".to_string()],
            app_version: "0.1.2".to_string(),
            os: "windows x86_64".to_string(),
            timestamp: "2026-07-26T15:00:00Z".to_string(),
        };

        let path = persist(&dir, raw).unwrap();
        let written = std::fs::read_to_string(&path).unwrap();

        assert!(
            !written.contains("victime@exemple.fr"),
            "l'adresse est dans le fichier écrit"
        );
        assert!(
            !written.contains('@'),
            "une arobase subsiste dans le fichier"
        );
        assert!(
            !written.to_lowercase().contains("medical"),
            "le sujet est dans le fichier écrit"
        );
        // Mais le fichier GARDE de quoi situer le bug.
        assert!(
            written.contains("apps/desktop/src/commands.rs:1"),
            "la localisation doit rester"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Le second panic du runtime (« cannot unwind », à la frontière FFI
    /// de WebView2) ne pointe pas le bug d'origine : on l'écarte. Défaut
    /// trouvé au terrain — le self-test produisait deux rapports, et
    /// l'abort écrasait le premier, seul utile.
    #[test]
    fn le_panic_secondaire_du_runtime_est_ecarte() {
        assert!(is_secondary_nounwind(
            "panic in a function that cannot unwind"
        ));
        assert!(!is_secondary_nounwind(
            "index out of bounds: len 3 but index 5"
        ));
        assert!(!is_secondary_nounwind(
            "selftest telemetrie: faux@exemple.fr"
        ));
    }

    /// Deux rapports écrits dans la même seconde ne se marchent pas
    /// dessus : le compteur `SEQ` rend chaque nom unique. Sans lui, le
    /// double panic d'un crash sur le thread principal perdait le rapport
    /// utile.
    #[test]
    fn deux_rapports_de_la_meme_seconde_ne_s_ecrasent_pas() {
        let dir = std::env::temp_dir().join(format!("disc-telemetry-seq-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        let raw = || RawPanic {
            message: "peu importe".to_string(),
            location: Some("apps/desktop/src/telemetry.rs:1".to_string()),
            backtrace: vec![],
            app_version: "0.1.2".to_string(),
            os: "windows x86_64".to_string(),
            timestamp: "2026-07-26T16:43:47Z".to_string(),
        };

        let un = persist(&dir, raw()).unwrap();
        let deux = persist(&dir, raw()).unwrap();
        assert_ne!(un, deux, "même horodatage, mais noms distincts");
        assert_eq!(
            std::fs::read_dir(&dir).unwrap().count(),
            2,
            "les deux rapports coexistent"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
