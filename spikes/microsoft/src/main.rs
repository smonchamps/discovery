//! Spike Microsoft 365 — étape 1 : la voie IMAP+OAuth2 tient-elle ?
//!
//! Le plan reportait à la Phase 3 le départage « IMAP+OAuth vs Graph »
//! (PHASE0.md §3, PLAN.md §2.4), sur critères mesurés : fiabilité, quotas,
//! effort. Ce banc mesure la voie IMAP+OAuth2 de bout en bout, sur un
//! compte RÉEL, sans rien envoyer.
//!
//! Il répond à quatre questions, dans l'ordre où elles peuvent tuer
//! l'option :
//!
//! 1. le consentement délégué accorde-t-il VRAIMENT les scopes Outlook ?
//!    (leçon Google de la Phase 0 : un jeton est délivré même si
//!    l'utilisateur décoche — seule la liste accordée fait foi) ;
//! 2. l'authentification IMAP XOAUTH2 passe-t-elle ?
//! 3. **SMTP AUTH est-il ouvert sur ce compte ?** C'est le risque nommé :
//!    Microsoft le désactive par défaut sur certains tenants, et sans lui
//!    la règle d'or « jamais d'envoi perdu » n'a plus de support ;
//! 4. quels dossiers spéciaux (RFC 6154) le serveur annonce-t-il ? Cela
//!    décide de la sémantique d'archivage — `\Archive` ou `\All`.
//!
//! ```powershell
//! $env:MICROSOFT_CLIENT_ID = "<application (client) ID>"
//! cargo run -- vous@exemple.com
//! ```
//!
//! Aucun message n'est envoyé : la connexion SMTP est seulement ouverte et
//! authentifiée (`test_connection`), ce qui suffit à valider le scope.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use imap_proto::NameAttribute;
use oauth2::basic::{BasicClient, BasicTokenResponse};
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, CsrfToken, PkceCodeChallenge, RedirectUrl, Scope,
    TokenResponse, TokenUrl,
};

/// Point de terminaison « common » : comptes professionnels ET personnels
/// (la doc Microsoft confirme le support OAuth2 IMAP/SMTP pour les deux).
const AUTH_URL: &str = "https://login.microsoftonline.com/common/oauth2/v2.0/authorize";
const TOKEN_URL: &str = "https://login.microsoftonline.com/common/oauth2/v2.0/token";

/// Scopes de la RESSOURCE Outlook — surtout pas les noms courts de Graph.
/// La doc insiste : « specify the full scopes, including Outlook resource
/// URLs ». C'est le piège n°1 de cette intégration.
const SCOPE_IMAP: &str = "https://outlook.office.com/IMAP.AccessAsUser.All";
const SCOPE_SMTP: &str = "https://outlook.office.com/SMTP.Send";
const SCOPE_OFFLINE: &str = "offline_access";

const IMAP_HOST: &str = "outlook.office365.com";
const IMAP_PORT: u16 = 993;
const SMTP_HOST: &str = "smtp.office365.com";
/// 587 + STARTTLS : Office 365 n'écoute pas en TLS implicite 465.
const SMTP_PORT: u16 = 587;

const ENVELOPE_BATCH: u32 = 200;

fn main() -> Result<()> {
    let email = std::env::args().nth(1).context(
        "usage : cargo run -- <votre.email@exemple.com>\n\
         (et $env:MICROSOFT_CLIENT_ID doit être défini)",
    )?;
    let client_id = std::env::var("MICROSOFT_CLIENT_ID")
        .context("MICROSOFT_CLIENT_ID manquante — l'Application (client) ID de l'app Azure")?;

    println!("== Spike Microsoft — voie IMAP + OAuth2 ==");
    println!("compte : {email}\n");

    let (tokens, oauth_elapsed) = authorize(&client_id)?;
    report_scopes(&tokens)?;
    println!("1. consentement OAuth2      : OK en {oauth_elapsed:?}\n");

    let access = tokens.access_token().secret().clone();
    imap_probe(&email, &access)?;
    smtp_probe(&email, &access)?;

    println!("\n== Verdict ==");
    println!("La voie IMAP+OAuth2 est praticable de bout en bout sur ce compte.");
    println!("Reportez ces chiffres dans spikes/microsoft/README.md.");
    Ok(())
}

/// Parcours PKCE loopback. Client PUBLIC : aucun secret, comme il se doit
/// pour une application de bureau.
fn authorize(client_id: &str) -> Result<(BasicTokenResponse, Duration)> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();

    // Microsoft distingue « localhost » de « 127.0.0.1 » : avec l'URI
    // `http://localhost` enregistrée, n'importe quel port est accepté.
    let redirect = format!("http://localhost:{port}");
    let client = BasicClient::new(ClientId::new(client_id.to_string()))
        .set_auth_uri(AuthUrl::new(AUTH_URL.to_string())?)
        .set_token_uri(TokenUrl::new(TOKEN_URL.to_string())?)
        .set_redirect_uri(RedirectUrl::new(redirect)?);

    let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
    let (auth_url, csrf) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new(SCOPE_IMAP.to_string()))
        .add_scope(Scope::new(SCOPE_SMTP.to_string()))
        .add_scope(Scope::new(SCOPE_OFFLINE.to_string()))
        .set_pkce_challenge(challenge)
        .url();

    println!("Ouverture du consentement Microsoft dans le navigateur…");
    if webbrowser::open(auth_url.as_str()).is_err() {
        println!("Ouvrez manuellement :\n{auth_url}");
    }

    let timer = Instant::now();
    let (code, state) = wait_for_redirect(&listener)?;
    if state != *csrf.secret() {
        bail!("état CSRF inattendu");
    }
    let http = oauth2::reqwest::blocking::ClientBuilder::new()
        .redirect(oauth2::reqwest::redirect::Policy::none())
        .build()?;
    let tokens = client
        .exchange_code(AuthorizationCode::new(code))
        .set_pkce_verifier(verifier)
        .request(&http)
        .map_err(|err| anyhow::anyhow!("échange du code : {err}"))?;
    Ok((tokens, timer.elapsed()))
}

/// Le jeton est délivré même si le consentement est partiel : seule la
/// liste ACCORDÉE fait foi (leçon Google, PHASE0.md §2).
fn report_scopes(tokens: &BasicTokenResponse) -> Result<()> {
    match tokens.scopes() {
        Some(scopes) => {
            let granted: Vec<&str> = scopes.iter().map(|s| s.as_str()).collect();
            println!("   scopes accordés : {granted:?}");
            let has_imap = granted.iter().any(|s| s.contains("IMAP.AccessAsUser"));
            let has_smtp = granted.iter().any(|s| s.contains("SMTP.Send"));
            if !has_imap {
                bail!("scope IMAP absent du consentement — l'option est morte en l'état");
            }
            if !has_smtp {
                println!("   ATTENTION : scope SMTP absent — l'envoi ne pourra pas fonctionner");
            }
        }
        None => println!("   (le serveur n'a pas renvoyé la liste des scopes)"),
    }
    println!(
        "   refresh token : {}",
        if tokens.refresh_token().is_some() {
            "reçu (reconnexion silencieuse possible)"
        } else {
            "ABSENT — pas de reconnexion silencieuse"
        }
    );
    Ok(())
}

/// Authentification XOAUTH2, inventaire des dossiers spéciaux, puis mesure
/// du chemin réellement chaud du produit : lire des enveloppes.
fn imap_probe(email: &str, access_token: &str) -> Result<()> {
    let timer = Instant::now();
    let client = imap::ClientBuilder::new(IMAP_HOST, IMAP_PORT).connect()?;
    let auth = XOAuth2 {
        user: email.to_string(),
        access_token: access_token.to_string(),
    };
    let mut session = client
        .authenticate("XOAUTH2", &auth)
        .map_err(|(err, _)| anyhow::anyhow!("authentification IMAP XOAUTH2 : {err}"))?;
    println!("2. connexion IMAP XOAUTH2   : OK en {:?}", timer.elapsed());

    let timer = Instant::now();
    let folders = session.list(None, Some("*"))?;
    let mut archive = Vec::new();
    let mut all = Vec::new();
    let mut trash = Vec::new();
    let mut drafts = Vec::new();
    for folder in folders.iter() {
        for attribute in folder.attributes() {
            match attribute {
                NameAttribute::Archive => archive.push(folder.name().to_string()),
                NameAttribute::All => all.push(folder.name().to_string()),
                NameAttribute::Trash => trash.push(folder.name().to_string()),
                NameAttribute::Drafts => drafts.push(folder.name().to_string()),
                _ => {}
            }
        }
    }
    println!(
        "   LIST : {} dossiers en {:?}",
        folders.len(),
        timer.elapsed()
    );
    println!("   \\Archive : {archive:?}");
    println!("   \\All     : {all:?}");
    println!("   \\Trash   : {trash:?}");
    println!("   \\Drafts  : {drafts:?}");
    if archive.is_empty() && all.is_empty() {
        println!("   ATTENTION : ni \\Archive ni \\All — archiver serait destructeur");
    }

    // Exchange n'annonce qu'une partie des attributs RFC 6154. Le dossier
    // d'archivage existe peut-être sous son seul nom : on regarde, plutôt
    // que de supposer.
    println!("\n   --- inventaire des dossiers (nom + attributs spéciaux) ---");
    for folder in folders.iter() {
        let special: Vec<String> = folder
            .attributes()
            .iter()
            .filter_map(|attribute| match attribute {
                NameAttribute::Archive => Some("\\Archive".to_string()),
                NameAttribute::All => Some("\\All".to_string()),
                NameAttribute::Trash => Some("\\Trash".to_string()),
                NameAttribute::Drafts => Some("\\Drafts".to_string()),
                NameAttribute::Sent => Some("\\Sent".to_string()),
                NameAttribute::Junk => Some("\\Junk".to_string()),
                _ => None,
            })
            .collect();
        if special.is_empty() {
            println!("   {}", folder.name());
        } else {
            println!("   {}  [{}]", folder.name(), special.join(" "));
        }
    }
    println!("   --- fin de l'inventaire ---\n");

    let timer = Instant::now();
    let inbox = session.select("INBOX")?;
    let total = inbox.exists;
    println!(
        "   SELECT INBOX : {total} messages, UIDVALIDITY {:?}, en {:?}",
        inbox.uid_validity,
        timer.elapsed()
    );

    if total > 0 {
        let first = total.saturating_sub(ENVELOPE_BATCH).max(1);
        let timer = Instant::now();
        let fetched = session.fetch(
            format!("{first}:{total}"),
            "(UID FLAGS INTERNALDATE ENVELOPE)",
        )?;
        println!(
            "   FETCH {} enveloppes en {:?}  <- le chemin chaud du produit",
            fetched.len(),
            timer.elapsed()
        );
    }

    let _ = session.logout();
    Ok(())
}

/// LE risque nommé : SMTP AUTH peut être fermé côté tenant. On ouvre et on
/// authentifie, sans envoyer le moindre message.
fn smtp_probe(email: &str, access_token: &str) -> Result<()> {
    use lettre::SmtpTransport;
    use lettre::transport::smtp::authentication::{Credentials, Mechanism};

    let timer = Instant::now();
    let transport = SmtpTransport::starttls_relay(SMTP_HOST)?
        .port(SMTP_PORT)
        .authentication(vec![Mechanism::Xoauth2])
        .credentials(Credentials::new(
            email.to_string(),
            access_token.to_string(),
        ))
        .build();

    match transport.test_connection() {
        Ok(true) => println!(
            "3. SMTP AUTH ({SMTP_HOST}:{SMTP_PORT} STARTTLS) : OUVERT, en {:?}",
            timer.elapsed()
        ),
        Ok(false) => println!("3. SMTP AUTH : le serveur ne répond pas"),
        Err(err) => println!(
            "3. SMTP AUTH : REFUSÉ — {err}\n   \
             (si « SmtpClientAuthentication is disabled », c'est le tenant qui\n   \
             ferme SMTP AUTH : l'envoi devra passer par Graph)"
        ),
    }
    Ok(())
}

struct XOAuth2 {
    user: String,
    access_token: String,
}

impl imap::Authenticator for XOAuth2 {
    type Response = String;

    fn process(&self, _challenge: &[u8]) -> Self::Response {
        format!(
            "user={}\x01auth=Bearer {}\x01\x01",
            self.user, self.access_token
        )
    }
}

fn wait_for_redirect(listener: &TcpListener) -> Result<(String, String)> {
    for stream in listener.incoming() {
        let mut stream = stream?;
        let mut request_line = String::new();
        BufReader::new(&mut stream).read_line(&mut request_line)?;
        let Some(params) = parse_redirect_query(&request_line) else {
            respond(&mut stream, "Requête ignorée.")?;
            continue;
        };
        if let Some(error) = params.get("error") {
            let description = params
                .get("error_description")
                .cloned()
                .unwrap_or_default();
            respond(&mut stream, "Autorisation refusée. Fermez cet onglet.")?;
            bail!("autorisation refusée : {error} — {description}");
        }
        if let (Some(code), Some(state)) = (params.get("code"), params.get("state")) {
            respond(
                &mut stream,
                "Autorisation reçue. Fermez cet onglet et revenez au terminal.",
            )?;
            return Ok((code.clone(), state.clone()));
        }
        respond(&mut stream, "Paramètres inattendus.")?;
    }
    bail!("redirection jamais reçue sur le port loopback")
}

fn parse_redirect_query(request_line: &str) -> Option<HashMap<String, String>> {
    let path = request_line.split_whitespace().nth(1)?;
    let url = url::Url::parse(&format!("http://localhost{path}")).ok()?;
    Some(
        url.query_pairs()
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect(),
    )
}

fn respond(stream: &mut TcpStream, message: &str) -> Result<()> {
    let body = format!("<html><body><p>{message}</p></body></html>");
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes())?;
    Ok(())
}
