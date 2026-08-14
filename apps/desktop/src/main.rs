#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! Shell desktop : la fenêtre Tauri branchée sur le noyau.
//!
//! L'UI est « bête » (PLAN.md §3) : elle affiche l'état et émet des
//! intentions via les commandes de [`commands`] ; toute l'intelligence vit
//! dans mail-core / mail-imap / mail-smtp / mail-auth.

mod commands;
mod demenagement;
mod telemetry;
mod veilleur;

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
    /// Génération de courrier, MONOTONE et jamais remise à zéro (E4) :
    /// bumpée à chaque relève INBOX qui a rapporté ou retiré du
    /// courrier — cycle, bouton, veilleur IDLE confondus. L'UI la lit
    /// à la sonde de `sync_progress` (5 s, déjà en place) et recharge
    /// la liste quand elle bouge : c'est ainsi que le courrier signalé
    /// par un veilleur se montre AU REPOS, sans canal neuf (R0-S5).
    pub generation: AtomicU64,
}

/// Le recul d'un compte en échec (complément P0, anti-martèlement) :
/// combien d'échecs CONSÉCUTIFS, et depuis quand. En mémoire seulement —
/// un redémarrage repart confiant, et c'est voulu : le recul protège le
/// serveur d'une boucle, pas d'un utilisateur qui relance son
/// application.
pub(crate) struct Recul {
    pub echecs: u32,
    pub depuis: Instant,
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
    /// Reculs par compte (email → échecs consécutifs) : le cycle et la
    /// passe légère SAUTENT un compte en recul — sans le taire, il reste
    /// compté injoignable. Le geste manuel force toujours la tentative.
    pub sync_reculs: Arc<Mutex<HashMap<String, Recul>>>,
    /// Un verrou de relève PAR compte (email → verrou) : cycle, bouton
    /// et veilleur IDLE peuvent vouloir relever le même INBOX en même
    /// temps — deux relèves concurrentes du même compte seraient
    /// idempotentes mais paieraient double. Un compte à la fois.
    pub verrous_releve: Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>>,
    /// Les veilleurs IDLE (ADR 0018) : email → drapeau de vie. Éteindre
    /// le drapeau arrête le veilleur à son prochain tour (≤ relance).
    pub veilleurs: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    /// L'état réseau remonté par l'UI (P0-bis) : hors ligne, les
    /// veilleurs dorment au lieu de reconnecter en boucle.
    pub en_ligne: Arc<AtomicBool>,
}

fn main() {
    // Le déménagement Discovery → Wind (PLAN-WIND E3) passe AVANT tout :
    // ni la base ni le profil WebView2 ne doivent naître côté Wind
    // pendant qu'un poste Discovery attend son rename. Échec = arrêt
    // net — continuer offrirait une application vide à un utilisateur
    // dont les données sont à un rename de là.
    if let Err(err) = demenagement::demenager() {
        eprintln!("échec du déménagement des données Discovery → Wind : {err}");
        eprintln!("Fermez toute autre instance de l'application, puis relancez.");
        std::process::exit(1);
    }
    let state = AppState {
        started_at: Instant::now(),
        accounts: Mutex::new(HashMap::new()),
        outbox_flush: Arc::new(Mutex::new(())),
        drafts_push: Arc::new(Mutex::new(())),
        bodies_backfill: Arc::new(Mutex::new(())),
        migration: Arc::new(MigrationShared::default()),
        sync_cycle: Arc::new(SyncShared::default()),
        sync_reculs: Arc::new(Mutex::new(HashMap::new())),
        verrous_releve: Arc::new(Mutex::new(HashMap::new())),
        veilleurs: Arc::new(Mutex::new(HashMap::new())),
        // En ligne par défaut : l'UI remonte le vrai état dès son
        // premier rendu (P0-bis) — d'ici là, mieux vaut tenter que dormir.
        en_ligne: Arc::new(AtomicBool::new(true)),
    };
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
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
            commands::attach_files,
            commands::detach_file,
            commands::draft_attachments,
            commands::fetch_source_attachment,
            commands::sync_drafts,
            commands::sync_progress,
            commands::sync_activity,
            commands::reseau_etat,
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
