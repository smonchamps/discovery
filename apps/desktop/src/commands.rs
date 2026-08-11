//! Commandes Tauri : la passerelle entre l'UI et le noyau.
//!
//! Multi-comptes (Phase 3) : l'identité d'un message est `(compte, uid)`
//! — un UID seul ne suffit plus. Chaque opération réseau passe par la
//! connexion de SON compte ; les boucles (synchro, vidange, brouillons)
//! agrègent les comptes connectés. Le travail bloquant (OAuth, IMAP,
//! SMTP) passe par `spawn_blocking` pour ne jamais geler la fenêtre.

use std::collections::HashMap;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Mutex, MutexGuard};
use std::time::Instant;

use mail_auth::{AccountSession, Authenticated, Authenticator, GenericCredentials};
use mail_core::AccountConfig;
use mail_core::{Action, MailServer, OutboxState, Store, SyncEngine};
use mail_imap::ImapServer;
use mail_smtp::SmtpMailer;
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::AppState;

const MAILBOX: &str = "INBOX";
const LIST_LIMIT_MAX: usize = 500;
const SEARCH_LIMIT: usize = 50;
/// Corps rapatriés par appel, tous comptes confondus. Borner le lot rend
/// l'interruption gratuite : l'UI cesse simplement de rappeler.
const BACKFILL_BUDGET: usize = 200;
/// En-têtes de fil rapatriés par compte et par synchronisation.
///
/// Généreux devant le budget des corps (200) parce que la dépense n'est
/// pas la même : un bloc d'en-têtes pèse ~3 ko contre ~50 ko pour un
/// message entier. Sur la boîte de l'utilisateur (~2 700 messages), deux
/// synchronisations suffisent à regrouper la boîte entière.
const THREAD_HEADER_BUDGET: usize = 2_000;
/// Arrivees remontees par compte pour les notifications. Au-dela, seul
/// le NOMBRE compte — la bulle resume de toute facon.
const NOTIFY_MAX_ARRIVALS: usize = 50;

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
    /// La boîte qui contient ce message. **Indispensable** : les UID sont
    /// attribués par boîte et repartent de 1, donc l'UID seul ne désigne
    /// plus un message dès qu'un compte en synchronise deux. Toute action
    /// de l'UI la renvoie.
    pub mailbox: String,
    pub uid: u32,
    pub subject: String,
    pub sender: String,
    pub date: String,
    pub seen: bool,
    pub flagged: bool,
    pub has_attachment: bool,
    /// La conversation à laquelle appartient la ligne — c'est par elle
    /// qu'on demande le reste de l'échange.
    pub thread_id: Option<i64>,
    /// Messages de la conversation présents dans la boîte. 1 = isolé.
    pub thread_size: u32,
    /// Non-lus de la conversation : c'est LUI qui décide du gras, et non
    /// l'état du seul message affiché.
    pub thread_unseen: u32,
}

#[tauri::command]
pub fn startup_report(state: State<'_, AppState>) -> String {
    format!(
        "fenêtre utilisable en {} ms",
        state.started_at.elapsed().as_millis()
    )
}

/// Bilan d'une reconnexion : ce qui est revenu, et POURQUOI le reste ne
/// l'est pas. Un compte muet est pire qu'un compte en erreur — sans cette
/// liste, l'utilisateur voit une pastille manquer sans savoir quoi faire.
#[derive(Serialize)]
pub struct ConnectReport {
    pub accounts: Vec<AccountInfo>,
    pub problems: Vec<String>,
}

/// Connexion silencieuse de TOUS les comptes du registre. Registre vide
/// (base migrée de Phase 2) : l'entrée héritée du coffre peut révéler le
/// compte — elle est alors migrée et le compte en attente revendiqué.
///
/// **Chaque compte est isolé** : la configuration manquante ou le jeton
/// périmé de l'un ne doit jamais empêcher les autres de revenir. Même
/// principe que [`sync_inbox`].
#[tauri::command]
pub async fn connect_accounts(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ConnectReport, String> {
    let path = db_path(&app)?;

    // Crochet E2E : comptes factices (emails séparés par des virgules),
    // jetons invalides par construction — hors ligne garanti.
    if let Ok(list) = std::env::var("DISCOVERY_E2E_ACCOUNT") {
        let store = Store::open(&path).map_err(|err| err.to_string())?;
        let mut infos = Vec::new();
        for email in list.split(',').map(str::trim).filter(|e| !e.is_empty()) {
            let id = store
                .adopt_or_create_account(email, mail_auth::GOOGLE.account_kind)
                .map_err(|err| err.to_string())?;
            lock_accounts(&state)?.insert(
                email.to_string(),
                AccountSession::OAuth(Authenticated {
                    provider: &mail_auth::GOOGLE,
                    email: email.to_string(),
                    access_token: "jeton-e2e-invalide".to_string(),
                }),
            );
            infos.push(AccountInfo {
                id,
                email: email.to_string(),
            });
        }
        return Ok(ConnectReport {
            accounts: infos,
            problems: Vec::new(),
        });
    }

    let accounts = {
        let store = Store::open(&path).map_err(|err| err.to_string())?;
        store.accounts().map_err(|err| err.to_string())?
    };

    let path_for_spawn = path.clone();
    let (connected, mut problems) = tauri::async_runtime::spawn_blocking(move || {
        let mut list = Vec::new();
        let mut problems: Vec<String> = Vec::new();
        for account in accounts {
            // La configuration OAuth manquante d'un fournisseur ne concerne
            // QUE ses comptes : elle ne doit empêcher aucun autre de revenir.
            let outcome = match account.provider.as_str() {
                "imap" => connect_generic(&path_for_spawn, &account),
                kind => match mail_auth::for_account_kind(kind) {
                    Some(provider) => Authenticator::from_env(provider)
                        .and_then(|auth| auth.authenticate_silent(&account.email))
                        .map(|session| Some(AccountSession::OAuth(session)))
                        .map_err(|err| err.to_string()),
                    None => Err(format!("fournisseur inconnu : {kind}")),
                },
            };
            match outcome {
                Ok(Some(session)) => list.push(session),
                Ok(None) => problems.push(format!(
                    "{} : configuration serveur incomplète",
                    account.email
                )),
                Err(reason) => problems.push(format!("{} : {reason}", account.email)),
            }
        }
        // Repli hérité Phase 2 : un compte Gmail sans provider explicite.
        // Propre à Google — la Phase 2 ne connaissait que lui.
        if list.is_empty()
            && let Ok(auth) = Authenticator::google_from_env()
            && let Ok(account) = auth.authenticate_silent_legacy()
        {
            list.push(AccountSession::OAuth(account));
        }
        (list, problems)
    })
    .await
    .map_err(|err| err.to_string())?;

    let store = Store::open(&path).map_err(|err| err.to_string())?;
    let mut infos = Vec::new();
    for session in connected {
        let email = session.email().to_string();
        let provider = match &session {
            AccountSession::OAuth(auth) => auth.provider.account_kind,
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
    problems.sort();
    Ok(ConnectReport {
        accounts: infos,
        problems,
    })
}

/// Reconnecte un compte IMAP générique depuis le coffre et sa
/// configuration. `Ok(None)` : la configuration serveur est incomplète.
fn connect_generic(
    db_path: &Path,
    account: &mail_core::Account,
) -> Result<Option<AccountSession>, String> {
    let password =
        mail_auth::fetch_generic_password(&account.email).map_err(|err| err.to_string())?;
    let config = Store::open(db_path)
        .map_err(|err| err.to_string())?
        .account_config(account.id)
        .map_err(|err| err.to_string())?;
    Ok(build_generic_session(&account.email, &password, &config))
}

/// Ajoute un compte Gmail — parcours navigateur complet, répétable.
/// Google livre l'identité du compte : rien à déclarer.
#[tauri::command]
pub async fn add_account(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<AccountInfo, String> {
    add_oauth_account(app, state, &mail_auth::GOOGLE, None).await
}

/// Ajoute un compte Microsoft 365 / Outlook.com.
///
/// L'adresse est SAISIE : dans le périmètre de scopes mesuré par le spike,
/// Microsoft ne livre pas l'identité du compte ([`mail_auth::Identity`]).
#[tauri::command]
pub async fn add_microsoft_account(
    app: AppHandle,
    state: State<'_, AppState>,
    email: String,
) -> Result<AccountInfo, String> {
    let email = email.trim().to_string();
    // Validation à la frontière : l'adresse déclarée devient la clé du
    // compte ET l'identifiant XOAUTH2. Une saisie vide produirait un
    // compte fantôme que rien ne pourrait plus joindre.
    if !is_plausible_address(&email) {
        return Err("adresse invalide — saisissez l'adresse complète du compte".to_string());
    }
    add_oauth_account(app, state, &mail_auth::MICROSOFT, Some(email)).await
}

/// Le tronc commun des ajouts OAuth2 : consentement navigateur, puis
/// enregistrement du compte sous la clé de SON fournisseur.
async fn add_oauth_account(
    app: AppHandle,
    state: State<'_, AppState>,
    provider: &'static mail_auth::Provider,
    declared_email: Option<String>,
) -> Result<AccountInfo, String> {
    let account = tauri::async_runtime::spawn_blocking(move || {
        Authenticator::from_env(provider)
            .map_err(|err| err.to_string())?
            .authenticate_interactive(declared_email.as_deref())
            .map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())??;

    let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
    let id = store
        .adopt_or_create_account(&account.email, account.provider.account_kind)
        .map_err(|err| err.to_string())?;
    let info = AccountInfo {
        id,
        email: account.email.clone(),
    };
    lock_accounts(&state)?.insert(account.email.clone(), AccountSession::OAuth(account));
    Ok(info)
}

/// Filtre minimal d'adresse : ce qui suit est vérifié par le fournisseur
/// lui-même au consentement. On ne cherche pas à valider RFC 5322 ici,
/// seulement à refuser ce qui ne peut manifestement pas être une adresse.
fn is_plausible_address(email: &str) -> bool {
    match email.split_once('@') {
        Some((local, domain)) => {
            !local.is_empty() && domain.contains('.') && !domain.starts_with('.')
        }
        None => false,
    }
}

/// Les champs arrivent de l'UI en camelCase. Tauri ne convertit que les
/// ARGUMENTS de commande, pas les champs d'une struct imbriquée : sans ce
/// `rename_all`, `imapHost` ne trouverait pas `imap_host`.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
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

    let (accounts, fetched, deleted, replayed, mut errors, refreshed, arrivals) =
        tauri::async_runtime::spawn_blocking(move || {
            let mut accounts = 0;
            let mut fetched = 0;
            let mut deleted = 0;
            let mut replayed = 0;
            let mut errors = Vec::new();
            let mut refreshed = Vec::new();
            // Les arrivees de TOUS les comptes sont agregees : une bulle
            // par compte serait la meme nuisance qu'une bulle par message.
            let mut arrivals = Vec::new();
            for (account_id, session) in jobs {
                let email = session.email().to_string();
                match run_sync(&session, account_id, &path) {
                    Ok(mut outcome) => {
                        accounts += 1;
                        fetched += outcome.report.fetched;
                        deleted += outcome.report.deleted;
                        replayed += outcome.report.replayed;
                        arrivals.append(&mut outcome.arrivals);
                        if let Some(fresh) = outcome.refreshed {
                            refreshed.push(fresh);
                        }
                        for problem in outcome.problems {
                            errors.push(format!("{email} : {problem}"));
                        }
                    }
                    Err(err) => errors.push(format!("{email} : {err}")),
                }
            }
            (
                accounts, fetched, deleted, replayed, errors, refreshed, arrivals,
            )
        })
        .await
        .map_err(|err| err.to_string())?;

    for fresh in refreshed {
        lock_accounts(&state)?.insert(fresh.email().to_string(), fresh);
    }
    if let Some(problem) = arrival_notification_problem(&app, &arrivals) {
        errors.push(problem);
    }
    let total = Store::open(&db_path(&app)?)
        .and_then(|store| store.unified_count())
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

/// « 1,2 Go » ou « 850 Mo » — l'utilisateur doit savoir COMBIEN libérer,
/// pas convertir des octets de tête. Préfixes décimaux (ceux de
/// l'Explorateur serait Gio, mais Go est ce que le grand public lit sur
/// une boîte de disque), virgule française.
fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_000_000_000 {
        format!("{:.1} Go", bytes as f64 / 1e9).replace('.', ",")
    } else {
        // Arrondi au Mo SUPÉRIEUR : annoncer « 0 Mo » à libérer serait
        // absurde, et sous-annoncer ferait échouer la re-tentative.
        format!("{} Mo", bytes.div_ceil(1_000_000).max(1))
    }
}

/// Ce qu'une synchronisation de compte rapporte, au-delà des décomptes.
struct SyncOutcome {
    report: mail_core::SyncReport,
    /// Session dont le jeton vient d'être renouvelé, à remettre en cache.
    refreshed: Option<AccountSession>,
    /// Arrivées à annoncer, déjà filtrées par les règles du noyau.
    arrivals: Vec<mail_core::Envelope>,
    /// Incidents non bloquants : la synchronisation a réussi, mais un
    /// travail de fond qui l'accompagne a échoué. Rapportés, jamais
    /// avalés — un symptôme sans trace est indiagnosticable.
    problems: Vec<String>,
}

fn run_sync(
    session: &AccountSession,
    account_id: i64,
    db_path: &Path,
) -> Result<SyncOutcome, String> {
    let (mut server, refreshed) = connect_imap(session)?;
    let mut store = Store::open(db_path).map_err(|err| err.to_string())?;
    // L'UID le plus haut AVANT la synchro : c'est lui qui separe
    // « nouveau » de « deja connu ». Releve avant, sinon la synchro
    // l'aurait deja deplace.
    let last_uid_before = store
        .sync_state(account_id, MAILBOX)
        .map_err(|err| err.to_string())?
        .map(|state| state.last_uid)
        .unwrap_or(0);
    let report = SyncEngine::default()
        .sync(&mut server, &mut store, account_id, MAILBOX)
        .map_err(|err| err.to_string())?;

    let mut problems: Vec<String> = Vec::new();

    // « Envoyés » : sans lui, un fil ne porte que la moitié reçue de
    // l'échange. Mesuré sur la boîte réelle — 15 conversations de plus
    // d'un message avant, 234 après.
    //
    // Ne devient sûr que parce que l'identité d'un message porte
    // désormais sa BOÎTE (ADR 0009, étape 4b) : sans cela, un UID de ce
    // dossier serait lu dans INBOX, et les UID repartant de 1 dans chaque
    // boîte, la collision serait la norme.
    //
    // Best effort, comme les passes voisines : le courrier ENTRANT est le
    // résultat qui compte, et un serveur sans dossier d'envois doit
    // continuer à fonctionner. L'échec est rapporté, jamais avalé — une
    // boîte qui refuserait de se regrouper serait sinon indiagnosticable.
    let sent = match server.sent_folder_name() {
        Ok(found) => found,
        Err(reason) => {
            problems.push(format!("dossier envoyés : {reason}"));
            None
        }
    };

    // La PORTÉE du regroupement, déclarée avant de verser quoi que ce soit
    // d'autre dans le compte (ADR 0010 §3). Sans elle, les messages des
    // dossiers qu'on s'apprête à synchroniser rejoindraient les fils tout
    // seuls : un spam ferait remonter une conversation en tête de liste.
    //
    // Re-déclarée à CHAQUE synchronisation, et pas seulement à la création
    // du compte : un serveur peut renommer son dossier d'envois.
    //
    // AVANT la boucle, et c'est tout l'objet : le store en garde mémoire
    // sur le compte, donc les boîtes que la boucle va CRÉER naissent déjà
    // du bon côté de la portée. La déclarer après les ferait naître sans
    // fil, et leurs messages attendraient le prochain démarrage.
    if let Err(reason) = store.set_thread_scope(account_id, sent.as_deref()) {
        problems.push(format!("portée des conversations : {reason}"));
    }

    // TOUS les autres dossiers — archive, corbeille, spam, dossiers de
    // l'utilisateur (ADR 0010 §1). INBOX vient d'être faite ; `sync_order`
    // la remet en tête et l'évite en double.
    //
    // Best effort dossier par dossier, et c'est délibéré : un serveur
    // refuse volontiers UN dossier (quota, corruption, droits) et sert tous
    // les autres. Faire échouer la synchronisation entière pour lui
    // priverait l'utilisateur de son courrier à cause d'un dossier qu'il ne
    // regarde jamais.
    let folders = match server.folders() {
        Ok(folders) => folders,
        Err(reason) => {
            problems.push(format!("liste des dossiers : {reason}"));
            Vec::new()
        }
    };
    let order = mail_core::sync_order(&folders, sent.as_deref());

    // La garde d'espace disque (ADR 0010 §4) : estimer AVANT de
    // s'engager, refuser en le chiffrant s'il manque. STATUS interroge
    // chaque dossier sans le sélectionner — quelques allers-retours
    // légers, contre des heures de téléchargement qui échoueraient à
    // mi-chemin sur un disque plein.
    //
    // INBOX est comptée des deux côtés (annonce ET base locale) : la
    // retirer d'un seul ferait sous-estimer le restant.
    let mut announced: u64 = 0;
    for boite in &order {
        match server.message_count(boite) {
            Ok(count) => announced += u64::from(count),
            // Un dossier qui refuse le comptage rend l'estimation basse.
            // On continue quand même : la garde est une protection, pas
            // un droit de veto — et l'échec est consigné.
            Err(reason) => problems.push(format!("comptage « {boite} » : {reason}")),
        }
    }
    let local = store
        .account_message_count(account_id)
        .map_err(|err| err.to_string())?;
    let pending = announced.saturating_sub(local);
    // L'espace se mesure sur le VOLUME de la base : c'est lui qui
    // encaissera les écritures, pas le disque système.
    let shortfall = match fs4::available_space(db_path.parent().unwrap_or(db_path)) {
        Ok(available) => mail_core::disk_shortfall(pending, available),
        Err(reason) => {
            // Mesure impossible ≠ espace insuffisant. Bloquer le courrier
            // parce qu'un appel système a échoué serait pire que le
            // risque couvert ; l'échec est dit, et SQLite signalera de
            // toute façon un disque plein, écriture par écriture.
            problems.push(format!("espace disque non mesurable : {reason}"));
            None
        }
    };
    if let Some(missing) = shortfall {
        problems.push(format!(
            "espace disque insuffisant : ~{} nécessaires pour {} message(s) \
             restants, il manque {} — récupération des dossiers suspendue \
             jusqu'à ce que de la place soit libérée",
            format_bytes(pending.saturating_mul(mail_core::SYNC_BYTES_PER_MESSAGE)),
            pending,
            format_bytes(missing),
        ));
    } else {
        for boite in order.into_iter().skip(1) {
            if let Err(reason) =
                SyncEngine::default().sync(&mut server, &mut store, account_id, &boite)
            {
                problems.push(format!("dossier « {boite} » : {reason}"));
            }
        }
    }

    // La passe d'en-têtes profite de la connexion déjà ouverte : c'est ce
    // qui la rend gratuite en allers-retours. Son échec ne doit PAS faire
    // échouer la synchronisation — le courrier est arrivé, c'est le seul
    // résultat qui compte — mais il est rapporté, jamais avalé. Sans
    // trace, une boîte qui refuse de se regrouper serait indiagnosticable.
    //
    // Elle passe sur les DEUX boîtes. `References` porte la racine du fil
    // là où `In-Reply-To` ne désigne que le parent immédiat : sans elle,
    // une réponse dont le message d'origine a été archivé hors d'INBOX ne
    // peut pas se raccrocher. L'ADR 0008 (mesure 2) est explicite —
    // `References` est obligatoire, pas un raffinement.
    //
    // Le budget est PARTAGÉ, pas doublé : la seconde boîte ne consomme que
    // ce que la première a laissé. Le coût réseau d'une synchronisation
    // reste donc exactement celui d'avant, et la passe étant reprenable,
    // le reliquat part au tour suivant.
    //
    // SANS horizon depuis l'ADR 0010 : le diagnostic terrain a montré la
    // passe convergée à 1 656 messages lus sur 1 656 éligibles — et 5 883
    // messages hors des 12 mois qui ne seraient JAMAIS lus. La borne
    // venait du budget disque des corps ; un bloc d'en-têtes pèse ~3 ko et
    // ne se range pas sur le disque comme un corps.
    //
    // La passe reste sur INBOX + Envoyés, elle : `References` est le
    // carburant du regroupement, et le regroupement s'arrête à cette
    // portée (ADR 0010 §3). Lire les en-têtes du Spam paierait des
    // allers-retours pour des messages qui ne se rattachent à rien.
    let mut budget = THREAD_HEADER_BUDGET;
    for boite in std::iter::once(MAILBOX).chain(sent.as_deref()) {
        if budget == 0 {
            break;
        }
        match mail_core::backfill_thread_headers(
            &mut server,
            &mut store,
            account_id,
            boite,
            mail_core::NO_HORIZON,
            budget,
        ) {
            Ok(report) => budget = budget.saturating_sub(report.fetched),
            Err(err) => problems.push(format!("conversations incomplètes : {err}")),
        }
    }

    // Le tirage des brouillons profite lui aussi de la connexion ouverte.
    // Il ne peut PAS vivre dans le cycle de poussée : celui-ci s'arrête
    // tôt quand il n'y a rien à pousser — à raison, sinon chaque frappe
    // ouvrirait une connexion. Un brouillon commencé ailleurs n'arriverait
    // donc jamais.
    if let Err(reason) = pull_drafts(&mut server, &store, account_id) {
        problems.push(format!("brouillons distants : {reason}"));
    }

    server.logout();

    let arrivals = store
        .new_unread_after(account_id, MAILBOX, last_uid_before, NOTIFY_MAX_ARRIVALS)
        .map_err(|err| err.to_string())?;
    Ok(SyncOutcome {
        report,
        refreshed,
        arrivals: mail_core::arrivals_to_notify(report.mode, arrivals),
        problems,
    })
}

/// Rapatrie les brouillons commencés ailleurs, et retire les miroirs
/// devenus périmés.
///
/// La décision appartient au noyau ([`mail_core::plan_draft_pull`], pur et
/// testé) ; ici on ne fait que l'exécuter.
fn pull_drafts(server: &mut ImapServer, store: &Store, account_id: i64) -> Result<(), String> {
    // La garde des repères d'abord, comme dans le cycle de poussée : si
    // l'UIDVALIDITY a changé, les `remote_uid` enregistrés ne désignent
    // plus rien. Comparer la liste distante à des repères périmés ferait
    // passer TOUS les miroirs pour caducs et réimporterait toute la
    // boîte.
    // Pas de dossier Brouillons annoncé : rien à tirer, et rien à
    // signaler. Le serveur n'est pas en panne, il n'a pas la capacité.
    if server
        .drafts_folder_name()
        .map_err(|err| err.to_string())?
        .is_none()
    {
        return Ok(());
    }
    let validity = server.drafts_uidvalidity().map_err(|err| err.to_string())?;
    let reset = store
        .align_drafts_uidvalidity(account_id, validity)
        .map_err(|err| err.to_string())?;
    if reset {
        // Repères abandonnés : rien ne distingue plus nos propres copies
        // de celles d'ailleurs. On laisse le cycle de poussée les
        // rétablir et on tirera au passage suivant. Un doublon reste
        // possible — c'est la règle d'or déjà en vigueur, et elle
        // préfère un doublon à une perte.
        return Ok(());
    }

    let remote = server.draft_uids().map_err(|err| err.to_string())?;
    let local = store.drafts_of(account_id).map_err(|err| err.to_string())?;
    let tombstones = store
        .draft_tombstones(account_id)
        .map_err(|err| err.to_string())?;
    let plan = mail_core::plan_draft_pull(&local, &remote, &tombstones);

    for id in plan.stale {
        store.drop_stale_draft(id).map_err(|err| err.to_string())?;
    }
    for uid in plan.fetch {
        let Some(draft) = server.fetch_draft(uid).map_err(|err| err.to_string())? else {
            // Disparu entre la liste et la lecture : sans conséquence.
            continue;
        };
        // Le corps arrive sous les deux formes MIME possibles ; c'est ici,
        // et pas dans l'adaptateur, qu'on sait rendre du HTML en texte.
        let body = draft.text.unwrap_or_else(|| {
            draft
                .html
                .as_deref()
                .map(mail_render::body_text)
                .unwrap_or_default()
        });
        store
            .import_remote_draft(account_id, uid, &draft.to_raw, &draft.subject, &body)
            .map_err(|err| err.to_string())?;
    }
    Ok(())
}

/// Affiche la bulle système d'un lot d'arrivées, s'il y a lieu.
///
/// Un échec — permission refusée, identité applicative non enregistrée —
/// ne doit JAMAIS faire échouer une synchro : le courrier est arrivé,
/// c'est le seul résultat qui compte. Mais il est **rapporté**.
///
/// La première version avalait l'erreur en silence. Sur un poste où
/// Windows refusait le toast, le symptôme était « rien ne se passe » :
/// indiagnosticable. Absorber un échec est une chose, en effacer la
/// trace en est une autre.
fn arrival_notification_problem(
    app: &AppHandle,
    arrivals: &[mail_core::Envelope],
) -> Option<String> {
    use tauri_plugin_notification::NotificationExt;

    let notification = mail_core::notification_for(arrivals)?;
    app.notification()
        .builder()
        .title(notification.title)
        .body(notification.body)
        .show()
        .err()
        .map(|err| format!("notification non affichée : {err}"))
}

#[derive(Serialize)]
pub struct MessagePage {
    pub total: u64,
    pub offset: usize,
    pub rows: Vec<MessageRow>,
    pub elapsed_us: u64,
}

/// Les messages d'une conversation, du plus ancien au plus récent.
///
/// Purement locale : ouvrir un fil ne demande jamais le réseau, comme
/// choisir un dossier de destination. C'est la leçon des dossiers, qui
/// avaient été livrés en interrogeant le serveur — inutilisables dès la
/// première coupure.
#[tauri::command]
pub fn thread_messages(app: AppHandle, thread_id: i64) -> Result<Vec<MessageRow>, String> {
    let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
    Ok(store
        .thread_messages(thread_id)
        .map_err(|err| err.to_string())?
        .into_iter()
        .map(to_message_row)
        .collect())
}

/// Mapping partagé entre la boîte unifiée et les résultats de recherche.
fn to_message_row(row: mail_core::UnifiedRow) -> MessageRow {
    MessageRow {
        has_attachment: row.has_attachment,
        account_id: row.account_id,
        account_email: row.account_email,
        mailbox: row.mailbox,
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
        thread_id: row.thread_id,
        thread_size: row.thread_size,
        thread_unseen: row.thread_unseen,
    }
}

/// Une page de la BOÎTE UNIFIÉE : tous les comptes fusionnés par date.
/// L'UI ne matérialise que les lignes visibles (virtualisation).
#[tauri::command]
pub fn list_messages(app: AppHandle, offset: usize, limit: usize) -> Result<MessagePage, String> {
    let timer = Instant::now();
    let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
    let total = store.unified_count().map_err(|err| err.to_string())?;
    let rows = store
        .unified_recent(offset, limit.min(LIST_LIMIT_MAX))
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

/// Un compte de la nav v2 (écran 02), avec ses compteurs — dossiers
/// canoniques résolus côté cœur (`nav.rs`), l'UI ne voit jamais un nom
/// de boîte réseau.
#[derive(Serialize)]
pub struct NavAccount {
    pub account_id: i64,
    pub email: String,
    pub reception_total: u64,
    pub reception_non_lues: u64,
    pub envoyes: u64,
    pub brouillons: u64,
    pub indesirables_total: u64,
    pub indesirables_non_lus: u64,
    pub archives: u64,
    pub corbeille: u64,
}

/// L'état complet de la nav en UN appel : comptes et compteurs par
/// catégorie. « Toutes les boîtes » s'agrège côté UI.
#[tauri::command]
pub fn nav_snapshot(app: AppHandle) -> Result<Vec<NavAccount>, String> {
    let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
    let mut sortie = Vec::new();
    for compte in store.accounts().map_err(|err| err.to_string())? {
        let dossiers = store
            .canonical_folders(compte.id)
            .map_err(|err| err.to_string())?;
        let compteurs = store
            .nav_counts(compte.id, &dossiers)
            .map_err(|err| err.to_string())?;
        sortie.push(NavAccount {
            account_id: compte.id,
            email: compte.email,
            reception_total: compteurs.reception_total,
            reception_non_lues: compteurs.reception_non_lues,
            envoyes: compteurs.envoyes,
            brouillons: compteurs.brouillons,
            indesirables_total: compteurs.indesirables_total,
            indesirables_non_lus: compteurs.indesirables_non_lus,
            archives: compteurs.archives,
            corbeille: compteurs.corbeille,
        });
    }
    Ok(sortie)
}

/// Une page d'une catégorie de la nav, bornée ou non à un compte.
/// `reception` = la boîte unifiée (conversations) ; les autres = les
/// messages des boîtes canoniques résolues, fusionnés par date.
#[tauri::command]
pub fn list_category(
    app: AppHandle,
    category: String,
    account_id: Option<i64>,
    offset: usize,
    limit: usize,
) -> Result<MessagePage, String> {
    let timer = Instant::now();
    let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
    let limit = limit.min(LIST_LIMIT_MAX);
    if category == "reception" {
        let total = store
            .unified_count_scoped(account_id)
            .map_err(|err| err.to_string())?;
        let rows = store
            .unified_recent_scoped(account_id, offset, limit)
            .map_err(|err| err.to_string())?
            .into_iter()
            .map(to_message_row)
            .collect();
        return Ok(MessagePage {
            total,
            offset,
            rows,
            elapsed_us: timer.elapsed().as_micros() as u64,
        });
    }
    let comptes: Vec<i64> = match account_id {
        Some(id) => vec![id],
        None => store
            .accounts()
            .map_err(|err| err.to_string())?
            .into_iter()
            .map(|compte| compte.id)
            .collect(),
    };
    let mut boites = Vec::new();
    for compte in comptes {
        let dossiers = store
            .canonical_folders(compte)
            .map_err(|err| err.to_string())?;
        if let Some(nom) = dossiers.boite(&category)
            && let Some(state) = store
                .sync_state(compte, &nom)
                .map_err(|err| err.to_string())?
        {
            boites.push(state.mailbox_id);
        }
    }
    let (total, _) = store
        .category_totals(&boites)
        .map_err(|err| err.to_string())?;
    let rows = store
        .category_page(&boites, offset, limit)
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
    mailbox: String,
    uid: u32,
    show_images: bool,
) -> Result<BodyView, String> {
    let html = raw_body(&app, &state, account_id, &mailbox, uid).await?;

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
    mailbox: String,
    uid: u32,
) -> Result<String, String> {
    let (mut server, _refreshed) = connect_imap(session)?;
    let mut store = Store::open(db_path).map_err(|err| err.to_string())?;
    let body = mail_core::load_body(&mut server, &mut store, account_id, &mailbox, uid)
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
    mailbox: &str,
    uid: u32,
) -> Result<String, String> {
    let path = db_path(app)?;
    let cached = Store::open(&path)
        .and_then(|store| store.body(account_id, mailbox, uid))
        .map_err(|err| err.to_string())?;
    match cached {
        Some(html) => Ok(html),
        None => {
            let session = auth_for(&path, state, account_id)?;
            // Copie possedee : la fermeture part sur un autre fil.
            let boite = mailbox.to_string();
            tauri::async_runtime::spawn_blocking(move || {
                fetch_body(&session, &path, account_id, boite, uid)
            })
            .await
            .map_err(|err| err.to_string())?
        }
    }
}

/// Une pièce jointe telle que l'UI la présente.
#[derive(Serialize)]
pub struct AttachmentRow {
    pub index: usize,
    pub name: String,
    pub mime: String,
    pub size: String,
}

/// Les pièces jointes connues d'un message — lecture LOCALE, aucun
/// réseau. Vide tant que le corps n'a pas été rapatrié : même condition
/// que la recherche dans le texte, et le rattrapage la lève.
#[tauri::command]
pub fn message_attachments(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
) -> Result<Vec<AttachmentRow>, String> {
    let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
    let found = store
        .attachments(account_id, &mailbox, uid)
        .map_err(|err| err.to_string())?;
    Ok(found
        .into_iter()
        .map(|attachment| AttachmentRow {
            index: attachment.index,
            size: attachment.human_size(),
            name: attachment.name,
            mime: attachment.mime,
        })
        .collect())
}

/// Enregistre une pièce jointe dans le dossier Téléchargements et
/// retourne son chemin.
///
/// Les octets ne sont jamais en cache : ils sont retéléchargés ici, une
/// fois, à la demande de l'utilisateur.
#[tauri::command]
pub async fn save_attachment(
    app: AppHandle,
    state: State<'_, AppState>,
    account_id: i64,
    mailbox: String,
    uid: u32,
    index: usize,
) -> Result<String, String> {
    let path = db_path(&app)?;
    let store = Store::open(&path).map_err(|err| err.to_string())?;
    let attachment = store
        .attachments(account_id, &mailbox, uid)
        .map_err(|err| err.to_string())?
        .into_iter()
        .find(|candidate| candidate.index == index)
        .ok_or_else(|| "pièce jointe inconnue".to_string())?;
    drop(store);

    let directory = app
        .path()
        .download_dir()
        .map_err(|err| format!("dossier Téléchargements introuvable : {err}"))?;
    let session = auth_for(&path, &state, account_id)?;
    let bytes = tauri::async_runtime::spawn_blocking(move || -> Result<Vec<u8>, String> {
        let (mut server, _refreshed) = connect_imap(&session)?;
        let bytes = server
            .fetch_attachment(&mailbox, uid, index)
            .map_err(|err| err.to_string())?;
        server.logout();
        bytes.ok_or_else(|| "pièce jointe absente du message".to_string())
    })
    .await
    .map_err(|err| err.to_string())??;

    let target = unique_path(&directory, &safe_file_name(&attachment.name));
    std::fs::write(&target, &bytes).map_err(|err| format!("écriture impossible : {err}"))?;
    Ok(target.to_string_lossy().into_owned())
}

/// Réduit un nom venu du RÉSEAU à un nom de fichier inoffensif.
///
/// Un nom de pièce jointe est une chaîne choisie par l'expéditeur. Tel
/// quel, `../../.ssh/authorized_keys` écrirait hors du dossier voulu :
/// c'est une écriture arbitraire de fichier, déclenchée par un simple
/// clic sur un message reçu. Rien de ce qui suit n'est de la prudence
/// excessive.
fn safe_file_name(raw: &str) -> String {
    // Ne garder que le dernier segment : tout séparateur, toute
    // remontée `..` et tout préfixe de lecteur disparaissent avec lui.
    let base = raw
        .rsplit(['/', '\\', ':'])
        .next()
        .unwrap_or("")
        .trim()
        .trim_matches('.');
    let cleaned: String = base
        .chars()
        .map(|c| match c {
            // Interdits par Windows, plus les caractères de contrôle.
            '<' | '>' | '"' | '|' | '?' | '*' => '_',
            c if (c as u32) < 0x20 => '_',
            c => c,
        })
        .take(120)
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').to_string();
    if cleaned.is_empty() || is_reserved_device_name(&cleaned) {
        return "piece-jointe".to_string();
    }
    cleaned
}

/// Noms réservés par Windows : un fichier nommé `CON` ou `LPT1` est
/// refusé par l'OS, quelle que soit l'extension.
fn is_reserved_device_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name).to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.ends_with(|c: char| c.is_ascii_digit() && c != '0'))
}

/// Chemin libre dans `directory` : `facture.pdf`, puis `facture (2).pdf`…
/// Enregistrer deux fois ne doit jamais écraser le premier fichier.
fn unique_path(directory: &Path, name: &str) -> PathBuf {
    let candidate = directory.join(name);
    if !candidate.exists() {
        return candidate;
    }
    let (stem, extension) = match name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() => (stem, format!(".{extension}")),
        _ => (name, String::new()),
    };
    for n in 2..1000 {
        let candidate = directory.join(format!("{stem} ({n}){extension}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    directory.join(format!("{stem} ({}){extension}", std::process::id()))
}

/// Archive : disparition locale immédiate + journalisation, le serveur
/// du compte suivra au prochain sync.
#[tauri::command]
pub fn archive_message(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
) -> Result<(), String> {
    queue_removal(&app, account_id, mailbox, uid, Action::Archive)
}

/// Un dossier proposé à l'utilisateur.
#[derive(Serialize)]
pub struct FolderRow {
    /// Nom RÉSEAU — c'est lui que l'UI renverra pour un déplacement.
    pub wire: String,
    /// Nom lisible, décodé de l'UTF-7 modifié.
    pub display: String,
}

/// Les dossiers d'un compte où un message peut être déplacé.
///
/// Lecture **purement locale** : le cache est rempli par la synchro.
/// Déplacer un message doit marcher hors ligne — l'action est journalisée
/// et rejouée, comme archiver. Interroger le serveur ici rendrait le tri
/// dépendant du réseau, ce que le produit refuse (PLAN.md §1).
///
/// La boîte courante est exclue : « déplacer vers INBOX » depuis INBOX
/// n'a pas de sens, et certains serveurs le refusent.
#[tauri::command]
pub fn list_folders(app: AppHandle, account_id: i64) -> Result<Vec<FolderRow>, String> {
    let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
    Ok(store
        .folders(account_id)
        .map_err(|err| err.to_string())?
        .into_iter()
        .filter(|folder| folder.selectable && folder.wire != MAILBOX)
        .map(|folder| FolderRow {
            wire: folder.wire,
            display: folder.display,
        })
        .collect())
}

/// Déplace un message : disparition locale immédiate + journalisation,
/// le serveur suivra au prochain sync — même boucle qu'archiver.
#[tauri::command]
pub fn move_message(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
    folder: String,
) -> Result<(), String> {
    // Le nom vient de l'UI, qui le tient de `list_folders` : il est déjà
    // en forme réseau. Le décoder ici ferait échouer le rejeu.
    if folder.trim().is_empty() {
        return Err("dossier de destination manquant".to_string());
    }
    queue_removal(&app, account_id, mailbox, uid, Action::MoveTo(folder))
}

/// Suppression : disparition locale immédiate + journalisation, mise à
/// la corbeille du serveur du compte au prochain sync.
#[tauri::command]
pub fn delete_message(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
) -> Result<(), String> {
    queue_removal(&app, account_id, mailbox, uid, Action::Delete)
}

fn queue_removal(
    app: &AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
    action: Action,
) -> Result<(), String> {
    let store = Store::open(&db_path(app)?).map_err(|err| err.to_string())?;
    let Some(state) = store
        .sync_state(account_id, &mailbox)
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
pub fn mark_seen(
    app: AppHandle,
    account_id: i64,
    mailbox: String,
    uid: u32,
    seen: bool,
) -> Result<(), String> {
    let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
    let Some(state) = store
        .sync_state(account_id, &mailbox)
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
    mailbox: String,
    uid: u32,
    flagged: bool,
) -> Result<(), String> {
    let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
    let Some(state) = store
        .sync_state(account_id, &mailbox)
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
    /// La boîte du message auquel on répond. Elle repart avec l'envoi :
    /// sans elle, l'UID ne suffit plus à retrouver le `Message-ID` à citer.
    pub mailbox: String,
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
    mailbox: String,
    uid: u32,
) -> Result<ComposeContext, String> {
    let envelope = envelope_of(&app, account_id, &mailbox, uid)?;
    let to = envelope
        .sender_address
        .clone()
        .ok_or_else(|| "adresse de l'expéditeur inconnue — resynchronisez la boîte".to_string())?;
    let body = match raw_body(&app, &state, account_id, &mailbox, uid).await {
        Ok(html) => mail_core::quote_reply(
            envelope.sender.as_deref(),
            quote_date(&envelope).as_deref(),
            &mail_render::body_text(&html),
        ),
        Err(_) => String::new(),
    };
    Ok(ComposeContext {
        account_id,
        mailbox,
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
    mailbox: String,
    uid: u32,
) -> Result<ComposeContext, String> {
    let envelope = envelope_of(&app, account_id, &mailbox, uid)?;
    let html = raw_body(&app, &state, account_id, &mailbox, uid).await?;
    Ok(ComposeContext {
        account_id,
        mailbox,
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

fn envelope_of(
    app: &AppHandle,
    account_id: i64,
    mailbox: &str,
    uid: u32,
) -> Result<mail_core::Envelope, String> {
    let store = Store::open(&db_path(app)?).map_err(|err| err.to_string())?;
    store
        .envelope(account_id, mailbox, uid)
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
    reply_to_mailbox: Option<String>,
    reply_to_uid: Option<u32>,
) -> Result<(), String> {
    let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
    let from = account_email(&store, account_id)?;
    // Sans la boîte, on ne résout RIEN — on ne devine pas.
    //
    // Un UID seul ne désigne plus un message depuis que le compte en a
    // deux (ADR 0009) : le n°1 d'INBOX et le n°1 d'« Envoyés » sont deux
    // messages. Deviner produirait un `In-Reply-To` pointant sur un
    // inconnu, donc une réponse greffée sur la conversation de quelqu'un
    // d'autre. L'omettre coupe un fil — « un fil coupé en deux est
    // réparable et honnête ; deux messages étrangers réunis ne le sont
    // pas » (ADR 0008 §2).
    let in_reply_to = reply_to_uid
        .zip(reply_to_mailbox)
        .and_then(|(uid, mailbox)| store.envelope(account_id, &mailbox, uid).ok().flatten())
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
    /// L'éditeur le renvoie à la sauvegarde : c'est ce qui lui permet de
    /// détecter qu'un autre a écrit entre-temps.
    pub updated_epoch: i64,
}

/// Ce qu'une sauvegarde a fait — l'éditeur en a besoin pour la suivante.
#[derive(Serialize)]
pub struct DraftSavedRow {
    pub id: i64,
    pub updated_epoch: i64,
    /// Le brouillon avait changé ailleurs : le texte de l'éditeur a été
    /// conservé à part. À dire à l'utilisateur, jamais à taire.
    pub forked: bool,
}

/// Sauvegarde un brouillon — texte brut, jamais validé : c'est un filet.
/// Le contenu tel que l'éditeur l'envoie — regroupé pour la même raison
/// qu'au noyau : quatre chaînes voisines invitent à en intervertir deux.
///
/// `camelCase` : Tauri ne convertit les noms qu'au premier niveau des
/// arguments. Sans cette annotation, l'UI devrait envoyer `reply_to_uid`
/// ici et `replyToUid` ailleurs — une incohérence qui ne se voit qu'à
/// l'exécution.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftContentArg {
    to: String,
    subject: String,
    body: String,
    reply_to_uid: Option<u32>,
}

#[tauri::command]
pub fn save_draft(
    app: AppHandle,
    account_id: i64,
    id: Option<i64>,
    base_epoch: Option<i64>,
    content: DraftContentArg,
) -> Result<DraftSavedRow, String> {
    let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
    let saved = store
        .save_draft(
            account_id,
            id,
            base_epoch,
            mail_core::DraftContent {
                to_raw: &content.to,
                subject: &content.subject,
                body: &content.body,
                reply_to_uid: content.reply_to_uid,
            },
        )
        .map_err(|err| err.to_string())?;
    Ok(DraftSavedRow {
        id: saved.id,
        updated_epoch: saved.updated_epoch,
        forked: saved.forked,
    })
}

#[tauri::command]
pub fn list_drafts(app: AppHandle) -> Result<Vec<DraftRow>, String> {
    let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
    Ok(store
        .drafts()
        .map_err(|err| err.to_string())?
        .into_iter()
        .map(|draft| DraftRow {
            updated_epoch: draft.updated_epoch,
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

/// Ouvre une connexion SMTP adaptée au type de compte. Pour un compte
/// OAuth2, un échec déclenche un refresh silencieux ; pour un compte
/// générique, le mot de passe est fixe (pas de retry possible).
///
/// Les serveurs viennent du fournisseur de la session, jamais d'une
/// constante d'application : c'est ce qui rend un deuxième fournisseur
/// possible sans toucher à cette fonction.
fn connect_smtp(session: &AccountSession) -> Result<(SmtpMailer, Option<AccountSession>), String> {
    match session {
        AccountSession::OAuth(auth) => {
            let smtp = auth.provider.smtp;
            match SmtpMailer::connect_xoauth2(smtp.host, smtp.port, &auth.email, &auth.access_token)
            {
                Ok(mailer) => Ok((mailer, None)),
                Err(_) => {
                    let fresh = Authenticator::from_env(auth.provider)
                        .map_err(|err| err.to_string())?
                        .authenticate_silent(&auth.email)
                        .map_err(|err| err.to_string())?;
                    let mailer = SmtpMailer::connect_xoauth2(
                        smtp.host,
                        smtp.port,
                        &fresh.email,
                        &fresh.access_token,
                    )
                    .map_err(|err| err.to_string())?;
                    Ok((mailer, Some(AccountSession::OAuth(fresh))))
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

/// Ouvre une connexion IMAP adaptée au type de compte. Pour un compte
/// OAuth2, un échec déclenche un refresh silencieux ; pour un compte
/// générique, le mot de passe est fixe.
fn connect_imap(session: &AccountSession) -> Result<(ImapServer, Option<AccountSession>), String> {
    match session {
        AccountSession::OAuth(auth) => {
            let imap = auth.provider.imap;
            match ImapServer::connect_xoauth2(imap.host, imap.port, &auth.email, &auth.access_token)
            {
                Ok(server) => Ok((server, None)),
                Err(_) => {
                    let fresh = Authenticator::from_env(auth.provider)
                        .map_err(|err| err.to_string())?
                        .authenticate_silent(&auth.email)
                        .map_err(|err| err.to_string())?;
                    let server = ImapServer::connect_xoauth2(
                        imap.host,
                        imap.port,
                        &fresh.email,
                        &fresh.access_token,
                    )
                    .map_err(|err| err.to_string())?;
                    Ok((server, Some(AccountSession::OAuth(fresh))))
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

// ---------------------------------------------------------------------
// Rattrapage des corps (ADR 0007, horizon levé par l'ADR 0010).
// ---------------------------------------------------------------------

/// Combien de messages attendent encore leur corps, tous comptes et
/// TOUTES boîtes confondus (ADR 0010 §1). Purement local : aucune
/// connexion réseau.
fn pending_total(store: &Store) -> Result<u64, String> {
    let mut total = 0;
    for account in store.accounts().map_err(|err| err.to_string())? {
        for boite in store
            .mailbox_names(account.id)
            .map_err(|err| err.to_string())?
        {
            total += store
                .bodies_pending_count(account.id, &boite, mail_core::NO_HORIZON)
                .map_err(|err| err.to_string())?;
        }
    }
    Ok(total)
}

#[derive(Serialize)]
pub struct SyncProgress {
    /// Messages en base, toutes boîtes déjà visitées confondues.
    pub local: u64,
    /// Messages annoncés par les serveurs pour ces mêmes boîtes.
    pub remote: u64,
    /// `None` tant qu'aucune boîte n'a été sélectionnée : l'interface
    /// n'affiche alors rien, plutôt qu'un « 0 % » qui ferait croire à une
    /// synchronisation en panne.
    pub percent: Option<u8>,
}

/// Avancement de la synchronisation intégrale (ADR 0010 §5).
///
/// Purement local — aucune connexion réseau : l'interface peut l'appeler
/// en boucle pendant qu'une synchronisation tourne, sans lui coûter un
/// seul aller-retour.
#[tauri::command]
pub fn sync_progress(app: AppHandle) -> Result<SyncProgress, String> {
    let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
    let (local, remote) = store.sync_progress().map_err(|err| err.to_string())?;
    Ok(SyncProgress {
        local,
        remote,
        percent: mail_core::sync_percent(local, remote),
    })
}

#[derive(Serialize)]
pub struct BackfillStatus {
    pub remaining: u64,
}

/// État du rattrapage, sans rien télécharger — de quoi afficher
/// « N messages sans corps » avant même de commencer.
#[tauri::command]
pub fn backfill_status(app: AppHandle) -> Result<BackfillStatus, String> {
    let store = Store::open(&db_path(&app)?).map_err(|err| err.to_string())?;
    Ok(BackfillStatus {
        remaining: pending_total(&store)?,
    })
}

// ---------------------------------------------------------------------
// Migration visible et interruptible (Phase 5, ADR 0012).
//
// Chaque commande ouvre sa propre connexion : sans cet écran, c'est la
// PREMIÈRE commande venue qui paierait l'adoption d'une base héritée —
// en silence, dans un gel d'interface. L'UI appelle donc
// `migration_check` AVANT toute commande qui touche la base ; s'il y a
// du travail, elle affiche l'écran, lance `migration_run`, sonde
// `migration_progress`, et `migration_cancel` rembobine tout (§8 de la
// passation : jamais d'adoption partielle persistée).
// ---------------------------------------------------------------------

#[derive(Serialize)]
pub struct MigrationCheck {
    /// Messages à adopter — `null` si l'ouverture sera silencieuse.
    pub pending: Option<u64>,
}

/// Sonde en lecture seule : rien n'est déclenché, rien n'est créé.
#[tauri::command]
pub fn migration_check(app: AppHandle) -> Result<MigrationCheck, String> {
    Ok(MigrationCheck {
        pending: Store::pending_adoption(&db_path(&app)?).map_err(|err| err.to_string())?,
    })
}

#[derive(Serialize)]
pub struct MigrationProgress {
    pub done: u64,
    pub total: u64,
    /// `None` tant que la passe n'a rien annoncé : l'écran n'affiche
    /// alors rien plutôt qu'un « 0 % » qui ferait croire à une panne.
    pub percent: Option<u8>,
}

/// Avancement de la passe en cours. Purement local et sans verrou :
/// la passe écrit des atomiques, le sondage ne la fait jamais attendre.
#[tauri::command]
pub fn migration_progress(state: State<'_, AppState>) -> MigrationProgress {
    let done = state.migration.done.load(Ordering::Relaxed);
    let total = state.migration.total.load(Ordering::Relaxed);
    MigrationProgress {
        done,
        total,
        percent: mail_core::sync_percent(done, total),
    }
}

/// Demande l'annulation : la passe la constate à son prochain palier et
/// rembobine tout — `migration_run` rendra alors `false`.
#[tauri::command]
pub fn migration_cancel(state: State<'_, AppState>) {
    state.migration.cancel.store(true, Ordering::Relaxed);
}

/// Joue la passe d'adoption, visible et interruptible.
///
/// Rend `true` si la base est migrée (ou n'avait rien à faire), `false`
/// si l'utilisateur a annulé — tout est alors défait, `user_version`
/// inchangé, et la passe entière se rejouera au prochain lancement.
#[tauri::command]
pub async fn migration_run(app: AppHandle, state: State<'_, AppState>) -> Result<bool, String> {
    let path = db_path(&app)?;
    let shared = state.migration.clone();
    shared.cancel.store(false, Ordering::Relaxed);
    shared.done.store(0, Ordering::Relaxed);
    shared.total.store(0, Ordering::Relaxed);

    tauri::async_runtime::spawn_blocking(move || {
        let result = Store::open_with_progress(&path, |progress| {
            shared.done.store(progress.done, Ordering::Relaxed);
            shared.total.store(progress.total, Ordering::Relaxed);
            if shared.cancel.load(Ordering::Relaxed) {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        });
        match result {
            // Le Store se referme aussitôt : les commandes suivantes
            // ouvriront le leur, comme d'habitude — mais sans passe à
            // payer, elle est faite.
            Ok(_store) => Ok(true),
            Err(mail_core::Error::Interrupted) => Ok(false),
            Err(err) => Err(err.to_string()),
        }
    })
    .await
    .map_err(|err| err.to_string())?
}

#[derive(Serialize)]
pub struct BackfillSummary {
    pub fetched: usize,
    pub remaining: u64,
    pub errors: Vec<String>,
}

/// UN lot de rattrapage, tous comptes connectés confondus.
///
/// Volontairement borné : l'UI rappelle tant qu'il reste du travail, et
/// s'arrête quand l'utilisateur le demande. L'interruption est ainsi
/// gratuite — aucun jeton d'annulation à propager — et une coupure ne
/// coûte jamais plus qu'un lot.
#[tauri::command]
pub async fn backfill_bodies(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<BackfillSummary, String> {
    let path = db_path(&app)?;
    let jobs = connected_jobs(&path, &state)?;
    let lock = state.bodies_backfill.clone();

    let (summary, refreshed) =
        tauri::async_runtime::spawn_blocking(move || run_backfill_all(jobs, &path, &lock))
            .await
            .map_err(|err| err.to_string())??;

    for fresh in refreshed {
        lock_accounts(&state)?.insert(fresh.email().to_string(), fresh);
    }
    Ok(summary)
}

fn run_backfill_all(
    jobs: Vec<(i64, AccountSession)>,
    db_path: &Path,
    lock: &Mutex<()>,
) -> Result<(BackfillSummary, Vec<AccountSession>), String> {
    let _guard = lock
        .lock()
        .map_err(|_| "rattrapage précédent interrompu".to_string())?;

    let mut store = Store::open(db_path).map_err(|err| err.to_string())?;
    let mut summary = BackfillSummary {
        fetched: 0,
        remaining: 0,
        errors: Vec::new(),
    };
    let mut refreshed_list = Vec::new();
    // Le budget est PARTAGÉ entre les comptes : un lot reste un lot, même
    // avec trois comptes connectés.
    let mut budget = BACKFILL_BUDGET;

    for (account_id, session) in jobs {
        if budget == 0 {
            break;
        }
        let email = session.email().to_string();
        // TOUTES les boîtes du compte (ADR 0010 §1), dans l'ordre du
        // store : réception d'abord, envois ensuite, le reste après. Le
        // budget est partagé entre elles comme entre les comptes — un
        // dossier d'archive de 80 000 messages ne confisque pas le lot,
        // il consomme ce que les boîtes prioritaires ont laissé.
        let boites = store
            .mailbox_names(account_id)
            .map_err(|err| err.to_string())?;
        // Ne pas ouvrir une connexion pour un compte qui n'a rien à faire.
        let mut pending = 0;
        for boite in &boites {
            pending += store
                .bodies_pending_count(account_id, boite, mail_core::NO_HORIZON)
                .map_err(|err| err.to_string())?;
        }
        if pending == 0 {
            continue;
        }
        match connect_imap(&session) {
            Err(reason) => summary.errors.push(format!("{email} : {reason}")),
            Ok((mut server, refreshed)) => {
                if let Some(fresh) = refreshed {
                    refreshed_list.push(fresh);
                }
                for boite in &boites {
                    if budget == 0 {
                        break;
                    }
                    match mail_core::backfill_bodies(
                        &mut server,
                        &mut store,
                        account_id,
                        boite,
                        mail_core::NO_HORIZON,
                        budget,
                    ) {
                        Ok(report) => {
                            summary.fetched += report.fetched;
                            budget = budget.saturating_sub(report.fetched);
                        }
                        // L'échec d'UNE boîte ne prive pas les suivantes :
                        // même règle que la synchronisation des dossiers.
                        Err(err) => summary.errors.push(format!("{email}, « {boite} » : {err}")),
                    }
                }
                server.logout();
            }
        }
    }

    summary.remaining = pending_total(&store)?;
    summary.errors.sort();
    Ok((summary, refreshed_list))
}

// ---------------------------------------------------------------------
// Mise a jour automatique signee (ADR 0013).
//
// Pilotee depuis Rust, comme les notifications : la webview n'appelle
// jamais l'API updater, seulement ces deux commandes — les capabilities
// restent `core:default`. La signature minisign est verifiee par le
// plugin AVANT toute installation ; sans elle, `download_and_install`
// echoue plutot que d'appliquer un paquet falsifie.
// ---------------------------------------------------------------------

use tauri_plugin_updater::UpdaterExt;

#[derive(Serialize)]
pub struct UpdateInfo {
    pub version: String,
    /// Notes de version, si la Release en porte.
    pub notes: Option<String>,
    /// Date de publication ISO 8601, telle qu'annoncee par le manifeste.
    pub date: Option<String>,
}

/// Y a-t-il une mise a jour ? `None` = a jour, ou hors ligne.
///
/// Appelee UNE fois au demarrage, en silence : un controle que
/// l'utilisateur doit reclamer n'aurait pas lieu (lecon de l'ADR 0007).
/// Hors ligne, l'endpoint est injoignable — ce n'est pas un defaut, donc
/// l'erreur remonte a l'UI qui reste muette plutot que de harceler ;
/// elle n'est jamais AVALEE (§9), seulement jugee sans gravite par
/// l'appelant.
#[tauri::command]
pub async fn update_check(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    // Les E2E ne parlent a AUCUN serveur (passation §7.5). Sans cette
    // garde, des qu'une Release existe, l'endpoint `latest.json`
    // repondrait et le bandeau apparaitrait en plein test — un flake.
    // `DISCOVERY_DB_PATH` n'est pose que par le harnais : c'est le meme
    // signal d'isolation que la base jetable.
    if std::env::var("DISCOVERY_DB_PATH").is_ok() {
        return Ok(None);
    }
    let updater = app.updater().map_err(|err| err.to_string())?;
    match updater.check().await.map_err(|err| err.to_string())? {
        Some(update) => Ok(Some(UpdateInfo {
            version: update.version.clone(),
            notes: update.body.clone(),
            date: update.date.map(|d| d.to_string()),
        })),
        None => Ok(None),
    }
}

/// Telecharge, verifie la signature, installe, puis redemarre.
///
/// `download_and_install` remplace le binaire en place ; `restart` rend
/// la main a la version neuve. La base ne bouge pas de `%APPDATA%`
/// (NSIS, pas MSIX — ADR 0013) : une mise a jour ne peut pas orpheliner
/// les messages.
#[tauri::command]
pub async fn update_install(app: AppHandle) -> Result<(), String> {
    let updater = app.updater().map_err(|err| err.to_string())?;
    let update = updater
        .check()
        .await
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "aucune mise a jour a installer".to_string())?;
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|err| err.to_string())?;
    app.restart();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un nom de pièce jointe est une chaîne choisie par l'EXPÉDITEUR.
    /// Écrit tel quel, il permet une écriture arbitraire de fichier
    /// déclenchée par un simple clic sur un message reçu. Ces cas ne
    /// sont pas théoriques : ce sont ceux des archives d'exploitation
    /// de clients mail.
    #[test]
    fn a_hostile_attachment_name_can_never_escape_its_folder() {
        assert_eq!(
            safe_file_name("../../.ssh/authorized_keys"),
            "authorized_keys"
        );
        assert_eq!(
            safe_file_name(r"..\..\Windows\System32\evil.dll"),
            "evil.dll"
        );
        assert_eq!(safe_file_name(r"C:\Windows\notepad.exe"), "notepad.exe");
        assert_eq!(safe_file_name("/etc/passwd"), "passwd");
        assert_eq!(safe_file_name(".."), "piece-jointe");
        assert_eq!(safe_file_name("/"), "piece-jointe");
        assert_eq!(safe_file_name(""), "piece-jointe");
    }

    /// Windows refuse ces noms quelle que soit l'extension : sans repli,
    /// l'enregistrement échouerait avec une erreur incompréhensible.
    #[test]
    fn windows_device_names_fall_back() {
        assert_eq!(safe_file_name("CON"), "piece-jointe");
        assert_eq!(safe_file_name("nul.txt"), "piece-jointe");
        assert_eq!(safe_file_name("COM1.pdf"), "piece-jointe");
        assert_eq!(safe_file_name("LPT9"), "piece-jointe");
        // Ni réservé, ni piégeux : doit passer tel quel.
        assert_eq!(safe_file_name("COM0.pdf"), "COM0.pdf");
        assert_eq!(safe_file_name("console.log"), "console.log");
    }

    /// Un nom légitime, même accentué, doit traverser INTACT — un filtre
    /// qui mutile les noms normaux serait payé tous les jours.
    #[test]
    fn a_legitimate_name_passes_through_untouched() {
        assert_eq!(safe_file_name("facture.pdf"), "facture.pdf");
        assert_eq!(safe_file_name("résumé 2026.docx"), "résumé 2026.docx");
        assert_eq!(
            safe_file_name("rapport-final_v2.xlsx"),
            "rapport-final_v2.xlsx"
        );
    }

    #[test]
    fn control_characters_and_wildcards_are_neutralised() {
        assert_eq!(safe_file_name("a<b>c.txt"), "a_b_c.txt");
        assert_eq!(safe_file_name("fac\u{7}ture?.pdf"), "fac_ture_.pdf");
    }

    /// Enregistrer deux fois la même pièce ne doit jamais écraser le
    /// premier fichier — la perte serait silencieuse.
    #[test]
    fn a_second_save_never_overwrites_the_first() {
        let dir = std::env::temp_dir().join(format!("discovery-pj-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let _ = std::fs::remove_file(dir.join("facture.pdf"));

        let first = unique_path(&dir, "facture.pdf");
        assert_eq!(first.file_name().unwrap(), "facture.pdf");
        std::fs::write(&first, b"x").unwrap();

        let second = unique_path(&dir, "facture.pdf");
        assert_eq!(second.file_name().unwrap(), "facture (2).pdf");

        std::fs::remove_file(&first).unwrap();
        let _ = std::fs::remove_dir(&dir);
    }

    /// L'adresse déclarée devient la clé du compte ET l'identifiant
    /// XOAUTH2 : elle n'est vérifiable par personne d'autre avant le
    /// consentement. Un filtre minimal évite le compte fantôme, sans
    /// prétendre valider la RFC 5322 — le fournisseur tranchera.
    #[test]
    fn declared_address_must_be_plausible() {
        assert!(is_plausible_address("moi@exemple.fr"));
        assert!(is_plausible_address("prenom.nom@outlook.com"));

        assert!(!is_plausible_address(""), "vide");
        assert!(!is_plausible_address("moi"), "sans arobase");
        assert!(!is_plausible_address("@exemple.fr"), "sans partie locale");
        assert!(!is_plausible_address("moi@"), "sans domaine");
        assert!(!is_plausible_address("moi@exemple"), "domaine sans point");
        assert!(
            !is_plausible_address("moi@.fr"),
            "domaine commençant par un point"
        );
    }
}
