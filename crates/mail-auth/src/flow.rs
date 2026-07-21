//! Mécanique du flux OAuth2 : échange par refresh token, parcours interactif
//! PKCE avec redirection loopback, vérification des scopes accordés.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};

use oauth2::basic::{BasicClient, BasicTokenResponse};
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret as OauthSecret, CsrfToken, EndpointNotSet,
    EndpointSet, PkceCodeChallenge, RedirectUrl, RefreshToken, Scope, TokenResponse, TokenUrl,
};

use crate::provider::{ClientSecret, Identity, Provider};

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("configuration : {0}")]
    Config(String),

    #[error("coffre de l'OS : {0}")]
    Vault(String),

    #[error("échange OAuth : {0}")]
    OAuth(String),

    #[error(
        "le consentement {0} n'inclut pas l'accès au courrier (accordé : {1:?}) — \
         recommencez en cochant la case correspondante sur l'écran d'autorisation"
    )]
    MissingMailScope(&'static str, Vec<String>),

    #[error("impossible d'ouvrir le navigateur — ouvrez manuellement : {0}")]
    BrowserFallback(String),

    #[error("réseau local : {0}")]
    Io(#[from] std::io::Error),
}

pub(crate) type OauthClient =
    BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointSet>;
pub(crate) type HttpClient = oauth2::reqwest::blocking::Client;

/// Construit le client OAuth2 du fournisseur.
///
/// Le secret n'est posé que si le fournisseur en attend un : Azure AD
/// **refuse** l'échange d'un client public qui en présente un.
pub(crate) fn oauth_client(
    provider: &Provider,
    client_id: &str,
    client_secret: Option<&str>,
) -> Result<OauthClient, AuthError> {
    let client = BasicClient::new(ClientId::new(client_id.to_string()))
        .set_auth_uri(AuthUrl::new(provider.auth_url.to_string()).map_err(config_err)?)
        .set_token_uri(TokenUrl::new(provider.token_url.to_string()).map_err(config_err)?);
    Ok(match (provider.client_secret, client_secret) {
        (ClientSecret::Required, Some(secret)) => {
            client.set_client_secret(OauthSecret::new(secret.to_string()))
        }
        _ => client,
    })
}

pub(crate) fn http_client() -> Result<HttpClient, AuthError> {
    oauth2::reqwest::blocking::ClientBuilder::new()
        .redirect(oauth2::reqwest::redirect::Policy::none())
        .build()
        .map_err(|err| AuthError::Config(err.to_string()))
}

pub(crate) fn refresh_access_token(
    client: &OauthClient,
    http: &HttpClient,
    refresh_token: String,
) -> Result<BasicTokenResponse, AuthError> {
    client
        .exchange_refresh_token(&RefreshToken::new(refresh_token))
        .request(http)
        .map_err(|err| AuthError::OAuth(err.to_string()))
}

/// Parcours interactif : listener loopback, consentement navigateur, échange
/// PKCE. Bloquant jusqu'à la redirection du fournisseur.
pub(crate) fn interactive_tokens(
    provider: &Provider,
    client: OauthClient,
    http: &HttpClient,
) -> Result<BasicTokenResponse, AuthError> {
    // L'écoute est TOUJOURS sur la boucle locale ; seul le nom annoncé au
    // fournisseur change (`localhost` chez Microsoft, `127.0.0.1` chez
    // Google) — les deux résolvent vers la même interface.
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    let client = client.set_redirect_uri(
        RedirectUrl::new(format!("http://{}:{port}", provider.redirect_host))
            .map_err(config_err)?,
    );

    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    let mut request = client
        .authorize_url(CsrfToken::new_random)
        .set_pkce_challenge(pkce_challenge);
    for scope in provider.scopes {
        request = request.add_scope(Scope::new((*scope).to_string()));
    }
    for (key, value) in provider.extra_auth_params {
        request = request.add_extra_param(*key, *value);
    }
    let (auth_url, csrf) = request.url();

    if webbrowser::open(auth_url.as_str()).is_err() {
        return Err(AuthError::BrowserFallback(auth_url.to_string()));
    }

    let (code, state) = wait_for_redirect(&listener)?;
    if state != *csrf.secret() {
        return Err(AuthError::OAuth("état CSRF inattendu".to_string()));
    }
    client
        .exchange_code(AuthorizationCode::new(code))
        .set_pkce_verifier(pkce_verifier)
        .request(http)
        .map_err(|err| AuthError::OAuth(err.to_string()))
}

/// Les deux fournisseurs délivrent un jeton même sur consentement partiel
/// (cases décochées chez Google, scopes refusés chez Microsoft) : seule la
/// liste des scopes *accordés* fait foi. Une réponse sans champ scope
/// (certains rafraîchissements) est acceptée : le consentement a déjà été
/// validé au moment du stockage du refresh token.
pub(crate) fn ensure_mail_scope(
    provider: &Provider,
    tokens: &BasicTokenResponse,
) -> Result<(), AuthError> {
    match tokens.scopes() {
        Some(scopes) => {
            let granted: Vec<String> = scopes.iter().map(|s| s.as_str().to_string()).collect();
            if granted
                .iter()
                .any(|scope| scope.contains(provider.granted_scope_marker))
            {
                Ok(())
            } else {
                Err(AuthError::MissingMailScope(provider.name, granted))
            }
        }
        None => Ok(()),
    }
}

/// Résout l'email du compte selon la stratégie d'identité du fournisseur.
///
/// `declared` est l'email fourni par l'utilisateur ; il n'est utilisé que
/// si le fournisseur ne sait pas livrer l'identité lui-même.
pub(crate) fn resolve_email(
    provider: &Provider,
    http: &HttpClient,
    access_token: &str,
    declared: Option<&str>,
) -> Result<String, AuthError> {
    match provider.identity {
        Identity::Userinfo(url) => fetch_email(http, url, access_token),
        Identity::Declared => declared.map(str::to_string).ok_or_else(|| {
            AuthError::Config(format!(
                "{} ne livre pas l'adresse du compte : elle doit être saisie",
                provider.name
            ))
        }),
    }
}

fn fetch_email(http: &HttpClient, url: &str, access_token: &str) -> Result<String, AuthError> {
    let body = http
        .get(url)
        .bearer_auth(access_token)
        .send()
        .map_err(network_err)?
        .error_for_status()
        .map_err(network_err)?
        .text()
        .map_err(network_err)?;
    serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|value| {
            value
                .get("email")
                .and_then(|email| email.as_str())
                .map(str::to_string)
        })
        .ok_or_else(|| AuthError::OAuth("email absent de la réponse userinfo".to_string()))
}

fn wait_for_redirect(listener: &TcpListener) -> Result<(String, String), AuthError> {
    for stream in listener.incoming() {
        let mut stream = stream?;
        let mut request_line = String::new();
        BufReader::new(&mut stream).read_line(&mut request_line)?;
        let Some(params) = parse_redirect_query(&request_line) else {
            respond(&mut stream, "Requête ignorée.")?;
            continue;
        };
        if let Some(error) = params.get("error") {
            respond(&mut stream, "Autorisation refusée. Fermez cet onglet.")?;
            return Err(AuthError::OAuth(format!("autorisation refusée : {error}")));
        }
        if let (Some(code), Some(state)) = (params.get("code"), params.get("state")) {
            respond(
                &mut stream,
                "Autorisation reçue. Fermez cet onglet et revenez à Discovery.",
            )?;
            return Ok((code.clone(), state.clone()));
        }
        respond(&mut stream, "Paramètres inattendus.")?;
    }
    Err(AuthError::OAuth(
        "redirection jamais reçue sur le port loopback".to_string(),
    ))
}

/// Extrait les paramètres de requête de la première ligne HTTP de la
/// redirection (`GET /?code=…&state=… HTTP/1.1`).
fn parse_redirect_query(request_line: &str) -> Option<HashMap<String, String>> {
    let path = request_line.split_whitespace().nth(1)?;
    let url = url::Url::parse(&format!("http://127.0.0.1{path}")).ok()?;
    Some(
        url.query_pairs()
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect(),
    )
}

fn respond(stream: &mut TcpStream, message: &str) -> Result<(), AuthError> {
    let body = format!("<html><body><p>{message}</p></body></html>");
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes())?;
    Ok(())
}

fn config_err(err: url::ParseError) -> AuthError {
    AuthError::Config(err.to_string())
}

fn network_err(err: oauth2::reqwest::Error) -> AuthError {
    AuthError::OAuth(err.to_string())
}

#[cfg(test)]
mod tests {
    use oauth2::basic::BasicTokenType;
    use oauth2::{AccessToken, EmptyExtraTokenFields, StandardTokenResponse};

    use super::*;
    use crate::provider::{GOOGLE, MICROSOFT};

    fn token_response(scopes: Option<Vec<&str>>) -> BasicTokenResponse {
        let mut response = StandardTokenResponse::new(
            AccessToken::new("jeton-de-test".to_string()),
            BasicTokenType::Bearer,
            EmptyExtraTokenFields {},
        );
        response.set_scopes(scopes.map(|list| {
            list.into_iter()
                .map(|s| Scope::new(s.to_string()))
                .collect()
        }));
        response
    }

    #[test]
    fn parses_code_and_state_from_redirect_line() {
        let params =
            parse_redirect_query("GET /?state=xyz&code=abc123 HTTP/1.1").expect("params attendus");
        assert_eq!(params.get("code").map(String::as_str), Some("abc123"));
        assert_eq!(params.get("state").map(String::as_str), Some("xyz"));
    }

    #[test]
    fn decodes_percent_encoding_in_redirect() {
        let params =
            parse_redirect_query("GET /?error=access%20denied HTTP/1.1").expect("params attendus");
        assert_eq!(
            params.get("error").map(String::as_str),
            Some("access denied")
        );
    }

    #[test]
    fn rejects_garbage_request_line() {
        assert!(parse_redirect_query("").is_none());
        assert!(parse_redirect_query("GET").is_none());
    }

    #[test]
    fn accepts_token_with_mail_scope() {
        let tokens = token_response(Some(vec![
            "https://mail.google.com/",
            "https://www.googleapis.com/auth/userinfo.email",
        ]));
        assert!(ensure_mail_scope(&GOOGLE, &tokens).is_ok());
    }

    #[test]
    fn rejects_token_missing_mail_scope() {
        let tokens = token_response(Some(vec!["https://www.googleapis.com/auth/userinfo.email"]));
        let err = ensure_mail_scope(&GOOGLE, &tokens).expect_err("scope manquant attendu");
        assert!(matches!(err, AuthError::MissingMailScope(_, _)));
    }

    #[test]
    fn accepts_refresh_response_without_scope_field() {
        let tokens = token_response(None);
        assert!(ensure_mail_scope(&GOOGLE, &tokens).is_ok());
    }

    /// Le consentement Microsoft se vérifie sur SES scopes à lui. Le
    /// marqueur Google n'y apparaît jamais : sans règle par fournisseur,
    /// tout compte Microsoft serait refusé alors qu'il est parfaitement
    /// autorisé.
    #[test]
    fn accepts_microsoft_token_with_its_own_imap_scope() {
        let tokens = token_response(Some(vec![
            "https://outlook.office.com/IMAP.AccessAsUser.All",
            "https://outlook.office.com/SMTP.Send",
        ]));
        assert!(ensure_mail_scope(&MICROSOFT, &tokens).is_ok());
        assert!(
            ensure_mail_scope(&GOOGLE, &tokens).is_err(),
            "les règles ne doivent pas être interchangeables"
        );
    }

    /// Le cas réellement dangereux : un consentement partiel où seul
    /// l'envoi est accordé. La synchro serait morte, et le message
    /// d'erreur doit nommer le bon fournisseur.
    #[test]
    fn rejects_microsoft_token_granted_only_for_sending() {
        let tokens = token_response(Some(vec!["https://outlook.office.com/SMTP.Send"]));
        let err = ensure_mail_scope(&MICROSOFT, &tokens).expect_err("scope IMAP manquant attendu");
        match err {
            AuthError::MissingMailScope(name, _) => assert_eq!(name, "Microsoft"),
            other => panic!("attendu un scope manquant, obtenu {other:?}"),
        }
    }
}
