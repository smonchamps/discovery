use crate::domain::MailboxKind;
use crate::error::DiscoveryError;
use base64::Engine;
use rand::{distributions::Alphanumeric, Rng};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;
use url::Url;

const GOOGLE_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const GOOGLE_USERINFO_URL: &str = "https://openidconnect.googleapis.com/v1/userinfo";
const CALLBACK_PATH: &str = "/oauth/google/callback";
const ENROLLMENT_TIMEOUT_SECS: u64 = 300;

#[derive(Clone, Debug)]
pub struct GmailAccountProfile {
    pub email: String,
    pub display_name: String,
}

#[derive(Clone, Debug)]
pub struct GmailLabel {
    pub id: String,
    pub kind: MailboxKind,
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct GoogleOAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub scopes: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct OAuthSession {
    pub state: String,
    pub code_verifier: String,
    pub redirect_uri: String,
}

pub struct LoopbackListener {
    callback_url: String,
    receiver: Receiver<Result<OAuthCallback, DiscoveryError>>,
}

#[derive(Clone, Debug)]
pub struct OAuthCallback {
    pub code: String,
    pub state: String,
}

pub struct GmailEnrollmentLaunch {
    pub config: GoogleOAuthConfig,
    pub session: OAuthSession,
    pub listener: LoopbackListener,
    pub authorize_url: String,
}

#[derive(Clone, Debug, Default)]
pub struct GmailClient {
    http: reqwest::Client,
}

#[derive(Deserialize)]
pub struct GoogleTokenResponse {
    pub access_token: String,
    pub refresh_token: Option<String>,
}

#[derive(Deserialize)]
struct UserInfoResponse {
    email: String,
    name: Option<String>,
}

impl GmailClient {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
        }
    }

    pub fn oauth_config_from_env(&self) -> Result<GoogleOAuthConfig, DiscoveryError> {
        let client_id = std::env::var("DISCOVERY_GOOGLE_CLIENT_ID")
            .map_err(|_| DiscoveryError::Configuration("DISCOVERY_GOOGLE_CLIENT_ID is not set".into()))?;
        let client_secret = std::env::var("DISCOVERY_GOOGLE_CLIENT_SECRET")
            .map_err(|_| {
                DiscoveryError::Configuration("DISCOVERY_GOOGLE_CLIENT_SECRET is not set".into())
            })?;

        Ok(GoogleOAuthConfig {
            client_id,
            client_secret,
            scopes: vec![
                "openid".into(),
                "email".into(),
                "profile".into(),
                "https://www.googleapis.com/auth/gmail.modify".into(),
                "https://www.googleapis.com/auth/gmail.compose".into(),
                "https://www.googleapis.com/auth/gmail.send".into(),
            ],
        })
    }

    pub fn start_oauth_device_flow(&self) -> String {
        GOOGLE_AUTH_URL.to_string()
    }

    pub fn start_loopback_enrollment(&self) -> Result<GmailEnrollmentLaunch, DiscoveryError> {
        let config = self.oauth_config_from_env()?;
        let listener = LoopbackListener::bind()?;
        let session = OAuthSession::new(listener.callback_url.clone());
        let authorize_url = self.build_authorize_url(&config, &session)?;

        Ok(GmailEnrollmentLaunch {
            config,
            session,
            listener,
            authorize_url,
        })
    }

    pub async fn exchange_code(
        &self,
        config: &GoogleOAuthConfig,
        session: &OAuthSession,
        callback: &OAuthCallback,
    ) -> Result<GoogleTokenResponse, DiscoveryError> {
        if callback.state != session.state {
            return Err(DiscoveryError::OAuth("State mismatch in OAuth callback".into()));
        }

        let response = self
            .http
            .post(GOOGLE_TOKEN_URL)
            .form(&[
                ("code", callback.code.as_str()),
                ("client_id", config.client_id.as_str()),
                ("client_secret", config.client_secret.as_str()),
                ("redirect_uri", session.redirect_uri.as_str()),
                ("grant_type", "authorization_code"),
                ("code_verifier", session.code_verifier.as_str()),
            ])
            .send()
            .await
            .map_err(|error| DiscoveryError::Network(error.to_string()))?;

        if !response.status().is_success() {
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "unknown token exchange error".into());
            return Err(DiscoveryError::OAuth(format!(
                "Google token exchange failed: {body}"
            )));
        }

        response
            .json::<GoogleTokenResponse>()
            .await
            .map_err(|error| DiscoveryError::OAuth(error.to_string()))
    }

    pub async fn fetch_profile(
        &self,
        access_token: &str,
    ) -> Result<GmailAccountProfile, DiscoveryError> {
        let response = self
            .http
            .get(GOOGLE_USERINFO_URL)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|error| DiscoveryError::Network(error.to_string()))?;

        if !response.status().is_success() {
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "unknown userinfo error".into());
            return Err(DiscoveryError::OAuth(format!(
                "Google userinfo request failed: {body}"
            )));
        }

        let profile = response
            .json::<UserInfoResponse>()
            .await
            .map_err(|error| DiscoveryError::OAuth(error.to_string()))?;

        Ok(GmailAccountProfile {
            display_name: profile
                .name
                .unwrap_or_else(|| profile.email.split('@').next().unwrap_or("Gmail").to_string()),
            email: profile.email,
        })
    }

    pub fn open_authorize_url(&self, authorize_url: &str) -> Result<(), DiscoveryError> {
        webbrowser::open(authorize_url)
            .map(|_| ())
            .map_err(|error| DiscoveryError::Browser(error.to_string()))
    }

    pub fn system_labels(&self) -> Vec<GmailLabel> {
        vec![
            GmailLabel {
                id: "INBOX".to_string(),
                kind: MailboxKind::Inbox,
                name: "Inbox".to_string(),
            },
            GmailLabel {
                id: "DRAFT".to_string(),
                kind: MailboxKind::Drafts,
                name: "Drafts".to_string(),
            },
            GmailLabel {
                id: "SENT".to_string(),
                kind: MailboxKind::Sent,
                name: "Sent".to_string(),
            },
            GmailLabel {
                id: "SPAM".to_string(),
                kind: MailboxKind::Spam,
                name: "Spam".to_string(),
            },
            GmailLabel {
                id: "ARCHIVE".to_string(),
                kind: MailboxKind::Archive,
                name: "Archive".to_string(),
            },
        ]
    }

    fn build_authorize_url(
        &self,
        config: &GoogleOAuthConfig,
        session: &OAuthSession,
    ) -> Result<String, DiscoveryError> {
        let mut url = Url::parse(GOOGLE_AUTH_URL)
            .map_err(|error| DiscoveryError::OAuth(error.to_string()))?;
        url.query_pairs_mut()
            .append_pair("client_id", &config.client_id)
            .append_pair("redirect_uri", &session.redirect_uri)
            .append_pair("response_type", "code")
            .append_pair("scope", &config.scopes.join(" "))
            .append_pair("access_type", "offline")
            .append_pair("prompt", "consent")
            .append_pair("state", &session.state)
            .append_pair("code_challenge_method", "S256")
            .append_pair("code_challenge", &pkce_challenge(&session.code_verifier));
        Ok(url.into())
    }
}

impl OAuthSession {
    fn new(redirect_uri: String) -> Self {
        Self {
            state: random_token(40),
            code_verifier: random_token(96),
            redirect_uri,
        }
    }
}

impl LoopbackListener {
    fn bind() -> Result<Self, DiscoveryError> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .map_err(|error| DiscoveryError::Network(error.to_string()))?;
        let port = listener
            .local_addr()
            .map_err(|error| DiscoveryError::Network(error.to_string()))?
            .port();
        let callback_url = format!("http://127.0.0.1:{port}{CALLBACK_PATH}");
        let (sender, receiver) = mpsc::channel();

        std::thread::spawn(move || {
            let callback = accept_callback(listener);
            let _ = sender.send(callback);
        });

        Ok(Self {
            callback_url,
            receiver,
        })
    }

    pub fn wait_for_callback(self) -> Result<OAuthCallback, DiscoveryError> {
        self.receiver
            .recv_timeout(Duration::from_secs(ENROLLMENT_TIMEOUT_SECS))
            .map_err(|_| DiscoveryError::OAuth("Timed out waiting for Google sign-in".into()))?
    }

    pub fn callback_url(&self) -> &str {
        &self.callback_url
    }
}

fn accept_callback(listener: TcpListener) -> Result<OAuthCallback, DiscoveryError> {
    let (mut stream, _) = listener
        .accept()
        .map_err(|error| DiscoveryError::Network(error.to_string()))?;

    let mut buffer = [0_u8; 4096];
    let bytes_read = stream
        .read(&mut buffer)
        .map_err(|error| DiscoveryError::Network(error.to_string()))?;
    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
    let request_line = request
        .lines()
        .next()
        .ok_or_else(|| DiscoveryError::OAuth("Empty OAuth callback request".into()))?;
    let path = request_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| DiscoveryError::OAuth("Malformed OAuth callback request".into()))?;
    let url = Url::parse(&format!("http://127.0.0.1{path}"))
        .map_err(|error| DiscoveryError::OAuth(error.to_string()))?;

    let params: std::collections::HashMap<String, String> =
        url.query_pairs().into_owned().collect();
    let body = "<html><body><h1>Discovery connected.</h1><p>You can close this browser window and return to the app.</p></body></html>";
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream
        .write_all(response.as_bytes())
        .map_err(|error| DiscoveryError::Network(error.to_string()))?;

    if let Some(error) = params.get("error") {
        return Err(DiscoveryError::OAuth(format!(
            "Google sign-in was not completed: {error}"
        )));
    }

    Ok(OAuthCallback {
        code: params
            .get("code")
            .cloned()
            .ok_or_else(|| DiscoveryError::OAuth("OAuth callback did not contain a code".into()))?,
        state: params
            .get("state")
            .cloned()
            .ok_or_else(|| DiscoveryError::OAuth("OAuth callback did not contain a state".into()))?,
    })
}

fn random_token(length: usize) -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .map(char::from)
        .collect()
}

fn pkce_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
}
