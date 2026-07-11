//! Spike Phase 0 — authentification Gmail sans mot de passe.
//!
//! Prouve la chaîne complète : OAuth2 PKCE (redirection loopback) →
//! refresh token stocké dans le Credential Manager Windows → IMAP XOAUTH2.
//! Code jetable : il valide des décisions, il ne rejoindra pas `mail-core` tel quel.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};

use anyhow::{Context, bail};
use oauth2::basic::{BasicClient, BasicTokenResponse};
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointNotSet, EndpointSet,
    PkceCodeChallenge, RedirectUrl, RefreshToken, Scope, TokenResponse, TokenUrl,
};

const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const USERINFO_URL: &str = "https://www.googleapis.com/oauth2/v2/userinfo";
const SCOPE_MAIL: &str = "https://mail.google.com/";
const SCOPE_EMAIL: &str = "https://www.googleapis.com/auth/userinfo.email";
const KEYRING_SERVICE: &str = "discovery-spike-oauth";
const KEYRING_USER: &str = "gmail-refresh-token";
const IMAP_HOST: &str = "imap.gmail.com";
const IMAP_PORT: u16 = 993;

type OauthClient =
    BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointSet>;
type HttpClient = oauth2::reqwest::blocking::Client;

fn main() -> anyhow::Result<()> {
    let client = oauth_client(
        require_env("GOOGLE_CLIENT_ID")?,
        require_env("GOOGLE_CLIENT_SECRET")?,
    )?;
    let http = oauth2::reqwest::blocking::ClientBuilder::new()
        .redirect(oauth2::reqwest::redirect::Policy::none())
        .build()
        .context("construction du client HTTP")?;
    let vault = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .context("accès au Credential Manager Windows")?;

    let access_token = match refresh_stored_token(&client, &http, &vault) {
        Some(token) => {
            println!("Session restaurée via le refresh token du Credential Manager.");
            token
        }
        None => authorize_in_browser(client, &http, &vault)?,
    };

    let email = fetch_email_address(&http, &access_token)?;
    println!("Autorisé en tant que {email} — vérification IMAP XOAUTH2…");
    list_recent_subjects(&email, &access_token)?;
    println!("Spike concluant : aucun mot de passe n'a transité.");
    Ok(())
}

fn require_env(name: &str) -> anyhow::Result<String> {
    std::env::var(name)
        .with_context(|| format!("variable d'environnement {name} manquante (voir README.md)"))
}

fn oauth_client(client_id: String, client_secret: String) -> anyhow::Result<OauthClient> {
    Ok(BasicClient::new(ClientId::new(client_id))
        .set_client_secret(ClientSecret::new(client_secret))
        .set_auth_uri(AuthUrl::new(AUTH_URL.to_string())?)
        .set_token_uri(TokenUrl::new(TOKEN_URL.to_string())?))
}

/// Tente de restaurer la session sans navigateur grâce au refresh token stocké.
fn refresh_stored_token(
    client: &OauthClient,
    http: &HttpClient,
    vault: &keyring::Entry,
) -> Option<String> {
    let stored = vault.get_password().ok()?;
    match client
        .exchange_refresh_token(&RefreshToken::new(stored))
        .request(http)
    {
        Ok(tokens) if has_mail_scope(&tokens) => Some(tokens.access_token().secret().clone()),
        Ok(_) => {
            eprintln!(
                "Le token stocké n'inclut pas l'accès Gmail : nouvelle autorisation nécessaire."
            );
            let _ = vault.delete_credential();
            None
        }
        Err(err) => {
            eprintln!("Refresh token inutilisable ({err}) : nouvelle autorisation nécessaire.");
            None
        }
    }
}

fn granted_scopes(tokens: &BasicTokenResponse) -> Vec<String> {
    tokens
        .scopes()
        .map(|scopes| scopes.iter().map(|s| s.as_str().to_string()).collect())
        .unwrap_or_default()
}

/// Google accepte le consentement même si l'utilisateur décoche des cases :
/// il faut vérifier ce qui a réellement été accordé, pas ce qui a été demandé.
fn has_mail_scope(tokens: &BasicTokenResponse) -> bool {
    granted_scopes(tokens).iter().any(|s| s == SCOPE_MAIL)
}

/// Flux interactif : navigateur → consentement Google → redirection loopback.
fn authorize_in_browser(
    client: OauthClient,
    http: &HttpClient,
    vault: &keyring::Entry,
) -> anyhow::Result<String> {
    let listener = TcpListener::bind("127.0.0.1:0").context("ouverture du port loopback")?;
    let port = listener.local_addr()?.port();
    let client = client.set_redirect_uri(RedirectUrl::new(format!("http://127.0.0.1:{port}"))?);

    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    let (auth_url, csrf) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new(SCOPE_MAIL.to_string()))
        .add_scope(Scope::new(SCOPE_EMAIL.to_string()))
        .add_extra_param("access_type", "offline")
        .add_extra_param("prompt", "consent")
        .set_pkce_challenge(pkce_challenge)
        .url();

    println!("Ouverture du navigateur pour autoriser l'accès Gmail…");
    if webbrowser::open(auth_url.as_str()).is_err() {
        println!("Ouvrez cette URL manuellement :\n{auth_url}");
    }

    let (code, state) = wait_for_redirect(&listener)?;
    if state != *csrf.secret() {
        bail!("état CSRF inattendu : flux interrompu");
    }

    let tokens = client
        .exchange_code(AuthorizationCode::new(code))
        .set_pkce_verifier(pkce_verifier)
        .request(http)
        .context("échange du code d'autorisation contre des tokens")?;

    if !has_mail_scope(&tokens) {
        let _ = vault.delete_credential();
        bail!(
            "le consentement Google n'inclut pas l'accès Gmail.\n\
             Scopes accordés : {:?}\n\
             Relancez le spike et, sur l'écran « Sélectionnez les autorisations », \
             cochez bien la case Gmail (« Lire, rédiger, envoyer… vos e-mails »).",
            granted_scopes(&tokens)
        );
    }

    if let Some(refresh) = tokens.refresh_token() {
        vault
            .set_password(refresh.secret())
            .context("stockage du refresh token dans le Credential Manager")?;
        println!("Refresh token stocké dans le Credential Manager Windows.");
    }
    Ok(tokens.access_token().secret().clone())
}

/// Attend la redirection de Google sur le port loopback et en extrait code + state.
fn wait_for_redirect(listener: &TcpListener) -> anyhow::Result<(String, String)> {
    for stream in listener.incoming() {
        let mut stream = stream.context("connexion loopback")?;
        let Some(params) = read_query_params(&mut stream)? else {
            respond(&mut stream, "Requête ignorée.")?;
            continue;
        };
        if let Some(error) = params.get("error") {
            respond(&mut stream, "Autorisation refusée. Fermez cet onglet.")?;
            bail!("autorisation refusée par Google : {error}");
        }
        if let (Some(code), Some(state)) = (params.get("code"), params.get("state")) {
            respond(
                &mut stream,
                "Autorisation reçue. Vous pouvez fermer cet onglet.",
            )?;
            return Ok((code.clone(), state.clone()));
        }
        respond(&mut stream, "Paramètres inattendus.")?;
    }
    bail!("le listener loopback s'est arrêté sans recevoir de redirection")
}

fn read_query_params(stream: &mut TcpStream) -> anyhow::Result<Option<HashMap<String, String>>> {
    let mut request_line = String::new();
    BufReader::new(&mut *stream)
        .read_line(&mut request_line)
        .context("lecture de la requête loopback")?;
    let Some(path) = request_line.split_whitespace().nth(1) else {
        return Ok(None);
    };
    let url = url::Url::parse(&format!("http://127.0.0.1{path}"))
        .context("parsing de l'URL de redirection")?;
    Ok(Some(
        url.query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect(),
    ))
}

fn respond(stream: &mut TcpStream, message: &str) -> anyhow::Result<()> {
    let body = format!("<html><body><p>{message}</p></body></html>");
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes())?;
    Ok(())
}

/// L'adresse du compte est nécessaire pour la chaîne XOAUTH2 (`user=…`).
fn fetch_email_address(http: &HttpClient, access_token: &str) -> anyhow::Result<String> {
    let body = http
        .get(USERINFO_URL)
        .bearer_auth(access_token)
        .send()
        .context("appel du endpoint userinfo")?
        .error_for_status()
        .context("réponse userinfo en erreur")?
        .text()?;
    let value: serde_json::Value = serde_json::from_str(&body).context("parsing userinfo")?;
    value
        .get("email")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .context("champ email absent de la réponse userinfo")
}

struct XOAuth2<'a> {
    user: &'a str,
    access_token: &'a str,
}

impl imap::Authenticator for XOAuth2<'_> {
    type Response = String;

    fn process(&self, _challenge: &[u8]) -> Self::Response {
        format!(
            "user={}\x01auth=Bearer {}\x01\x01",
            self.user, self.access_token
        )
    }
}

/// Reproduit l'ancien spike (lister des sujets d'INBOX), mais sans mot de passe.
fn list_recent_subjects(email: &str, access_token: &str) -> anyhow::Result<()> {
    let client = imap::ClientBuilder::new(IMAP_HOST, IMAP_PORT)
        .connect()
        .with_context(|| format!("connexion à {IMAP_HOST}:{IMAP_PORT}"))?;
    let auth = XOAuth2 {
        user: email,
        access_token,
    };
    let mut session = client
        .authenticate("XOAUTH2", &auth)
        .map_err(|(err, _)| anyhow::anyhow!("authentification XOAUTH2 : {err}"))?;

    let mailbox = session.select("INBOX").context("ouverture d'INBOX")?;
    if mailbox.exists == 0 {
        println!("INBOX est vide.");
    } else {
        let first = mailbox.exists.saturating_sub(4).max(1);
        let range = format!("{first}:{}", mailbox.exists);
        let messages = session
            .fetch(&range, "ENVELOPE")
            .context("lecture des enveloppes")?;
        println!("Les {} derniers sujets d'INBOX :", messages.len());
        for msg in messages.iter() {
            let subject = msg
                .envelope()
                .and_then(|e| e.subject.as_ref())
                .map(|s| String::from_utf8_lossy(s).into_owned())
                .unwrap_or_else(|| "(sans sujet)".to_string());
            println!("- {subject}");
        }
    }
    let _ = session.logout();
    Ok(())
}
