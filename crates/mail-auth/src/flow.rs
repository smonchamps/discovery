//! Mécanique du flux OAuth2 : échange par refresh token, parcours interactif
//! PKCE avec redirection loopback, vérification des scopes accordés.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};

use oauth2::basic::{BasicClient, BasicTokenResponse};
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointNotSet, EndpointSet,
    PkceCodeChallenge, RedirectUrl, RefreshToken, Scope, TokenResponse, TokenUrl,
};

use crate::{AUTH_URL, SCOPE_EMAIL, SCOPE_MAIL, TOKEN_URL, USERINFO_URL};

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("configuration : {0}")]
    Config(String),

    #[error("coffre de l'OS : {0}")]
    Vault(String),

    #[error("échange OAuth : {0}")]
    OAuth(String),

    #[error(
        "le consentement Google n'inclut pas l'accès Gmail (accordé : {0:?}) — \
         recommencez en cochant la case Gmail sur l'écran d'autorisation"
    )]
    MissingMailScope(Vec<String>),

    #[error("impossible d'ouvrir le navigateur — ouvrez manuellement : {0}")]
    BrowserFallback(String),

    #[error("réseau local : {0}")]
    Io(#[from] std::io::Error),
}

pub(crate) type OauthClient =
    BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointSet>;
pub(crate) type HttpClient = oauth2::reqwest::blocking::Client;

pub(crate) fn oauth_client(client_id: &str, client_secret: &str) -> Result<OauthClient, AuthError> {
    Ok(BasicClient::new(ClientId::new(client_id.to_string()))
        .set_client_secret(ClientSecret::new(client_secret.to_string()))
        .set_auth_uri(AuthUrl::new(AUTH_URL.to_string()).map_err(config_err)?)
        .set_token_uri(TokenUrl::new(TOKEN_URL.to_string()).map_err(config_err)?))
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
/// PKCE. Bloquant jusqu'à la redirection de Google.
pub(crate) fn interactive_tokens(
    client: OauthClient,
    http: &HttpClient,
) -> Result<BasicTokenResponse, AuthError> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    let client = client.set_redirect_uri(
        RedirectUrl::new(format!("http://127.0.0.1:{port}")).map_err(config_err)?,
    );

    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    let (auth_url, csrf) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new(SCOPE_MAIL.to_string()))
        .add_scope(Scope::new(SCOPE_EMAIL.to_string()))
        .add_extra_param("access_type", "offline")
        .add_extra_param("prompt", "consent")
        .set_pkce_challenge(pkce_challenge)
        .url();

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

/// Google accepte le consentement même avec des cases décochées : seule la
/// liste des scopes *accordés* fait foi. Une réponse sans champ scope
/// (certains rafraîchissements) est acceptée : le consentement a déjà été
/// validé au moment du stockage du refresh token.
pub(crate) fn ensure_mail_scope(tokens: &BasicTokenResponse) -> Result<(), AuthError> {
    match tokens.scopes() {
        Some(scopes) => {
            let granted: Vec<String> = scopes.iter().map(|s| s.as_str().to_string()).collect();
            if granted.iter().any(|scope| scope == SCOPE_MAIL) {
                Ok(())
            } else {
                Err(AuthError::MissingMailScope(granted))
            }
        }
        None => Ok(()),
    }
}

pub(crate) fn fetch_email(http: &HttpClient, access_token: &str) -> Result<String, AuthError> {
    let body = http
        .get(USERINFO_URL)
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
        let tokens = token_response(Some(vec![SCOPE_MAIL, SCOPE_EMAIL]));
        assert!(ensure_mail_scope(&tokens).is_ok());
    }

    #[test]
    fn rejects_token_missing_mail_scope() {
        let tokens = token_response(Some(vec![SCOPE_EMAIL]));
        let err = ensure_mail_scope(&tokens).expect_err("scope manquant attendu");
        assert!(matches!(err, AuthError::MissingMailScope(_)));
    }

    #[test]
    fn accepts_refresh_response_without_scope_field() {
        let tokens = token_response(None);
        assert!(ensure_mail_scope(&tokens).is_ok());
    }
}
