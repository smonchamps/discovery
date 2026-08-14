//! Le veilleur IDLE (ADR 0018) — le temps réel, par compte.
//!
//! Un thread par compte connecté, sur une connexion IMAP DÉDIÉE (jamais
//! celle du cycle : la poignée `idle` de la crate efface le timeout P0
//! en sortant — l'isoler protège le cycle de vie du reste). Le veilleur
//! ne touche JAMAIS la base : il SIGNALE, et la passe légère du compte
//! ([`crate::commands::passe_legere_compte`]) fait le travail — un seul
//! chemin de relève, celui du bouton et du cycle.
//!
//! Tout ce qui suit sort du spike mesuré (`spikes/idle/`, terrains des
//! 2026-08-14) : relance courte car elle est AUSSI le détecteur de
//! connexion morte, passe à chaque (re)connexion car un mail arrivé
//! pendant une coupure n'émet jamais d'EXISTS, reconnexion à délai
//! doublé, jeton relu au trousseau à chaque connexion.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use tauri::Manager;

use crate::AppState;
use crate::commands;

/// Relance IDLE : le délai max de détection d'une connexion morte
/// (2ᵉ terrain : coupure et veille Windows ne produisent AUCUNE erreur,
/// la lecture bloque en silence jusqu'à cette échéance). 3 min — loin
/// des 29 permises par la RFC 2177, et 2 commandes par cycle : rien.
const RELANCE: Duration = Duration::from_secs(3 * 60);
/// Reconnexion à délai doublé : 2 s → 60 s, réarmée après 2 min de
/// session stable (repris du spike, prouvé aux terrains).
const PAUSE_MIN: Duration = Duration::from_secs(2);
const PAUSE_MAX: Duration = Duration::from_secs(60);
const SESSION_STABLE: Duration = Duration::from_secs(120);
/// Cadence de re-vérification quand le veilleur DORT (hors ligne,
/// compte en recul) : une lecture d'atomique, aucun octet réseau.
const SOMMEIL: Duration = Duration::from_secs(5);

/// Réconcilie les veilleurs avec les comptes connectés : un veilleur
/// par session, ceux des comptes partis s'éteignent à leur prochain
/// tour. Idempotente — appelée après chaque connexion, ajout et retrait
/// de compte.
pub(crate) fn reconcilier(app: &tauri::AppHandle) {
    let state = app.state::<AppState>();
    let connectes: Vec<String> = match state.accounts.lock() {
        Ok(accounts) => accounts.keys().cloned().collect(),
        Err(_) => return,
    };
    let Ok(mut veilleurs) = state.veilleurs.lock() else {
        return;
    };
    veilleurs.retain(|email, vivant| {
        let garde = connectes.iter().any(|connecte| connecte == email);
        if !garde {
            vivant.store(false, Ordering::Relaxed);
        }
        garde
    });
    for email in connectes {
        if veilleurs.contains_key(&email) {
            continue;
        }
        let vivant = Arc::new(AtomicBool::new(true));
        veilleurs.insert(email.clone(), vivant.clone());
        let app = app.clone();
        // Un thread nommé : dans un vidage de pile, « veilleur-idle »
        // se lit ; l'email n'y figure pas (§6.8).
        let _ = std::thread::Builder::new()
            .name("veilleur-idle".to_string())
            .spawn(move || boucle(app, email, vivant));
    }
}

/// La boucle d'un veilleur : dormir quand il le faut (hors ligne,
/// recul), sinon connexion dédiée → passe de (re)connexion → veille —
/// et reconnexion à délai doublé quand la session tombe.
fn boucle(app: tauri::AppHandle, email: String, vivant: Arc<AtomicBool>) {
    // L'identifiant NUMÉRIQUE du compte, pour la console (§6.8 : jamais
    // d'adresse dans les traces). Introuvable = on trace « ? ».
    let compte_id = id_du_compte(&app, &email);
    let mut pause = PAUSE_MIN;
    while vivant.load(Ordering::Relaxed) {
        {
            let state = app.state::<AppState>();
            // Hors ligne (P0-bis) : dormir, ne pas marteler.
            if !state.en_ligne.load(Ordering::Relaxed) {
                std::thread::sleep(SOMMEIL);
                continue;
            }
            // Le recul se respecte, en LECTURE seule : le veilleur ne
            // l'aggrave jamais (son délai doublé suffit à sa propre
            // politesse), mais il n'insiste pas sur un compte en échec.
            if commands::recul_en_cours(&state.sync_reculs, &email).is_some() {
                std::thread::sleep(SOMMEIL);
                continue;
            }
        }
        let debut = Instant::now();
        match veille_session(&app, &email, &vivant) {
            // Sortie propre : le drapeau est tombé (compte retiré) ou
            // le réseau est parti — la boucle décidera.
            Ok(()) => continue,
            Err(err) => {
                eprintln!("veilleur compte {compte_id} : session tombée : {err}");
            }
        }
        if !vivant.load(Ordering::Relaxed) {
            break;
        }
        if debut.elapsed() > SESSION_STABLE {
            pause = PAUSE_MIN;
        }
        eprintln!(
            "veilleur compte {compte_id} : reconnexion dans {} s",
            pause.as_secs()
        );
        std::thread::sleep(pause);
        pause = (pause * 2).min(PAUSE_MAX);
    }
}

/// Une session de veille : connexion dédiée (jeton relu au trousseau
/// par `connect_imap`), passe de (re)connexion, puis tours d'IDLE.
/// `Ok(())` = sortie volontaire ; `Err` = la connexion est morte,
/// l'appelant reconnecte.
fn veille_session(
    app: &tauri::AppHandle,
    email: &str,
    vivant: &Arc<AtomicBool>,
) -> Result<(), String> {
    let session = {
        let state = app.state::<AppState>();
        let Some(session) = commands::lock_accounts(&state)?.get(email).cloned() else {
            // Plus de session (compte retiré) : sortie propre, la
            // réconciliation a déjà éteint le drapeau ou le fera.
            return Ok(());
        };
        session
    };
    let (mut server, refreshed) = commands::connect_imap(&session)?;
    if let Some(fresh) = refreshed {
        let state = app.state::<AppState>();
        commands::lock_accounts(&state)?.insert(fresh.email().to_string(), fresh);
    }
    // La passe de (RE)CONNEXION, jamais optionnelle : un mail arrivé
    // pendant l'absence est déjà dans la boîte — aucun EXISTS ne le
    // signalera (2ᵉ terrain). Best effort : son échec n'abat pas la
    // veille, le courrier suivant la déclenchera.
    if let Err(err) = commands::passe_legere_compte(app, email) {
        eprintln!("veilleur : passe de connexion en échec : {err}");
    }
    loop {
        if !vivant.load(Ordering::Relaxed) {
            server.logout();
            return Ok(());
        }
        {
            let state = app.state::<AppState>();
            if !state.en_ligne.load(Ordering::Relaxed) {
                // L'OS a dit « hors ligne » (P0-bis) : on rend la
                // connexion — elle est probablement déjà morte — et la
                // boucle dormira jusqu'au retour.
                server.logout();
                return Ok(());
            }
        }
        match server.veiller(commands::MAILBOX, RELANCE) {
            Ok(mail_imap::Veille::Courrier) => {
                // Du courrier ! La passe légère du compte le relève —
                // sur SA connexion à elle (timeouts P0 intacts), pendant
                // que celle-ci retourne veiller.
                if let Err(err) = commands::passe_legere_compte(app, email) {
                    eprintln!("veilleur : passe légère en échec : {err}");
                }
            }
            // Battement de cœur : le DONE/re-IDLE du prochain tour
            // prouvera que la connexion vit.
            Ok(mail_imap::Veille::Echeance) => {}
            Err(err) => return Err(err.to_string()),
        }
    }
}

/// L'identifiant numérique du compte — le seul nom qu'une trace a le
/// droit de porter (§6.8).
fn id_du_compte(app: &tauri::AppHandle, email: &str) -> String {
    let trouve = || -> Option<i64> {
        let path = commands::db_path(app).ok()?;
        let store = mail_core::Store::open(&path).ok()?;
        store
            .accounts()
            .ok()?
            .into_iter()
            .find(|compte| compte.email == email)
            .map(|compte| compte.id)
    };
    trouve().map_or_else(|| "?".to_string(), |id| id.to_string())
}
