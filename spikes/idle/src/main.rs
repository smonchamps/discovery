//! Spike E4 (PLAN-SYNCHRO) — la veille IDLE mesurée : latence
//! arrivée → événement, tenue de connexion, reconnexion après coupure,
//! veille/reprise Windows, expiration du jeton OAuth.
//!
//! JETABLE : rien d'ici ne part en production — le spike imprime des
//! horodatages, l'opérateur mesure, l'ADR tranche (où vit `idle`,
//! comment le timeout P0 cohabite avec des lectures longues).
//!
//! Protocole et gates chiffrées : voir README.md à côté.

use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use chrono::Local;

/// Chaque ligne est horodatée à la milliseconde : c'est la matière
/// première de toutes les mesures du protocole.
fn horodate(message: &str) {
    println!("[{}] {message}", Local::now().format("%H:%M:%S%.3f"));
}

/// Chaîne SASL XOAUTH2 — la même que `mail-imap`, recopiée : le spike
/// est jetable, il n'ouvre pas d'API de prod pour trois lignes.
struct XOAuth2 {
    user: String,
    token: String,
}

impl imap::Authenticator for XOAuth2 {
    type Response = String;

    fn process(&self, _challenge: &[u8]) -> Self::Response {
        format!("user={}\x01auth=Bearer {}\x01\x01", self.user, self.token)
    }
}

/// Deux voies d'accès : le trousseau de l'application (Gmail/Microsoft,
/// jeton relu à CHAQUE reconnexion — l'expiration OAuth fait partie du
/// protocole), ou hôte + mot de passe pour l'IMAP générique.
enum Acces {
    Trousseau {
        fournisseur: &'static mail_auth::Provider,
        email: String,
    },
    MotDePasse {
        host: String,
        port: u16,
        user: String,
        password: String,
    },
}

fn env(nom: &str) -> Result<String> {
    std::env::var(nom).with_context(|| format!("variable {nom} absente"))
}

fn acces_depuis_env() -> Result<Acces> {
    if let Ok(fournisseur) = std::env::var("SPIKE_FOURNISSEUR") {
        let fournisseur = match fournisseur.as_str() {
            "gmail" | "google" => &mail_auth::GOOGLE,
            "microsoft" => &mail_auth::MICROSOFT,
            autre => bail!("fournisseur inconnu : {autre} (gmail | microsoft)"),
        };
        return Ok(Acces::Trousseau {
            fournisseur,
            email: env("SPIKE_EMAIL")?,
        });
    }
    Ok(Acces::MotDePasse {
        host: env("SPIKE_HOST")?,
        port: env("SPIKE_PORT")?.parse().context("SPIKE_PORT invalide")?,
        user: env("SPIKE_USER")?,
        password: env("SPIKE_PASSWORD")?,
    })
}

fn se_connecter(acces: &Acces) -> Result<imap::Session<imap::Connection>> {
    match acces {
        Acces::Trousseau { fournisseur, email } => {
            // Le jeton est relu au trousseau à chaque connexion : une
            // session qui tombe APRÈS l'expiration doit repartir seule —
            // c'est l'une des cinq mesures du protocole.
            let jeton = mail_auth::Authenticator::from_env(fournisseur)
                .map_err(|err| anyhow!("config OAuth : {err}"))?
                .authenticate_silent(email)
                .map_err(|err| anyhow!("jeton : {err}"))?;
            let client =
                imap::ClientBuilder::new(fournisseur.imap.host, fournisseur.imap.port)
                    .connect()
                    .context("connexion")?;
            client
                .authenticate(
                    "XOAUTH2",
                    &XOAuth2 {
                        user: jeton.email.clone(),
                        token: jeton.access_token.clone(),
                    },
                )
                .map_err(|(err, _)| anyhow!("authentification : {err}"))
        }
        Acces::MotDePasse {
            host,
            port,
            user,
            password,
        } => {
            let client = imap::ClientBuilder::new(host.as_str(), *port)
                .connect()
                .context("connexion")?;
            client
                .login(user, password)
                .map_err(|(err, _)| anyhow!("authentification : {err}"))
        }
    }
}

/// La veille elle-même : IDLE sur la boîte, relancé toutes les
/// `relance` minutes PAR NOTRE boucle (pas le keepalive de la crate),
/// chaque réponse non sollicitée horodatée. Ne rend la main que par
/// erreur — c'est la boucle de reconnexion qui décide de la suite.
///
/// Le 1ᵉʳ terrain (2026-08-14) a montré pourquoi la relance doit être
/// COURTE : le timeout de lecture de la veille EST le détecteur de
/// connexion morte. Coupure Wi-Fi et veille Windows ne produisent AUCUNE
/// erreur — la lecture bloque en silence jusqu'à l'échéance (le « hang »
/// de Thunderbird sur changement d'IP, bug Mozilla 284152). À 28 min de
/// relance, le spike est resté aveugle ; à 3 min, la mort se détecte en
/// ≤ 3 min au DONE/re-IDLE, pour 2 commandes par cycle — un coût nul.
/// Et le keepalive vit dans NOTRE boucle pour que chaque relance
/// s'imprime : un log muet pendant une coupure était l'angle mort n°1
/// du terrain.
fn veiller(
    mut session: imap::Session<imap::Connection>,
    boite: &str,
    relance: Duration,
) -> Result<()> {
    session.select(boite).context("SELECT")?;
    horodate(&format!(
        "veille ouverte sur {boite} (relance IDLE toutes les {} min — c'est \
         aussi le délai max de détection d'une connexion morte)",
        relance.as_secs() / 60
    ));
    loop {
        let mut poignee = session.idle();
        poignee.timeout(relance).keepalive(false);
        let sortie = poignee.wait_while(|reponse| {
            use imap::types::UnsolicitedResponse as R;
            match reponse {
                // LE point de mesure : l'horodatage de cette ligne,
                // comparé à l'heure de la BULLE sur le téléphone (pas à
                // l'heure d'envoi — elle inclut la livraison Gmail).
                R::Exists(n) => {
                    horodate(&format!("EXISTS {n} — nouveau courrier signalé"));
                    true
                }
                autre => {
                    horodate(&format!("réponse non sollicitée : {autre:?}"));
                    true
                }
            }
        });
        match sortie {
            // TimedOut = battement de cœur : rien depuis `relance`, le
            // DONE/re-IDLE vient de prouver que la connexion vit encore.
            Ok(sortie) => horodate(&format!("relance de veille ({sortie:?}) — connexion vivante")),
            Err(err) => return Err(anyhow!("veille rompue : {err}")),
        }
    }
}

fn main() -> Result<()> {
    let acces = acces_depuis_env()?;
    let boite = std::env::var("SPIKE_BOITE").unwrap_or_else(|_| "INBOX".to_string());
    // 3 min et non 28 (RFC 2177 permettrait 29) : la relance est AUSSI
    // le délai max de détection d'une connexion morte — terrain du
    // 2026-08-14, coupure et veille restées invisibles 28 min.
    let relance_min: u64 = std::env::var("SPIKE_RELANCE_MIN")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3);
    let relance = Duration::from_secs(relance_min * 60);

    horodate("spike idle — Ctrl+C pour arrêter ; chaque ligne est une mesure");
    // Reconnexion à délai doublé (2 s → 60 s), réarmé dès qu'une session
    // a tenu 2 min : une coupure brève repart vite, une panne durable ne
    // martèle pas le serveur.
    let mut pause = Duration::from_secs(2);
    loop {
        let debut = Instant::now();
        match se_connecter(&acces) {
            Ok(session) => {
                horodate("connecté");
                if let Err(err) = veiller(session, &boite, relance) {
                    horodate(&format!("session tombée : {err:#}"));
                }
            }
            Err(err) => horodate(&format!("connexion impossible : {err:#}")),
        }
        if debut.elapsed() > Duration::from_secs(120) {
            pause = Duration::from_secs(2);
        }
        horodate(&format!("reconnexion dans {} s", pause.as_secs()));
        std::thread::sleep(pause);
        pause = (pause * 2).min(Duration::from_secs(60));
    }
}
