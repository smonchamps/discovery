//! Connexion Gmail silencieuse : réutilise le refresh token stocké dans le
//! Credential Manager par le spike `oauth-gmail` (à lancer une fois avant).

use anyhow::Context;
use oauth2::basic::BasicClient;
use oauth2::{AuthUrl, ClientId, ClientSecret, RefreshToken, TokenResponse, TokenUrl};

const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const USERINFO_URL: &str = "https://www.googleapis.com/oauth2/v2/userinfo";
const KEYRING_SERVICE: &str = "discovery-spike-oauth";
const KEYRING_USER: &str = "gmail-refresh-token";
const IMAP_HOST: &str = "imap.gmail.com";
const IMAP_PORT: u16 = 993;

pub type ImapSession = imap::Session<Box<dyn imap::ImapConnection>>;

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

pub fn connect() -> anyhow::Result<(ImapSession, String)> {
    let (token, email) = access_token_and_email()?;
    let client = imap::ClientBuilder::new(IMAP_HOST, IMAP_PORT)
        .connect()
        .with_context(|| format!("connexion à {IMAP_HOST}:{IMAP_PORT}"))?;
    let session = client
        .authenticate(
            "XOAUTH2",
            &XOAuth2 {
                user: email.clone(),
                token,
            },
        )
        .map_err(|(err, _)| anyhow::anyhow!("authentification XOAUTH2 : {err}"))?;
    Ok((session, email))
}

fn access_token_and_email() -> anyhow::Result<(String, String)> {
    let client_id = std::env::var("GOOGLE_CLIENT_ID")
        .context("GOOGLE_CLIENT_ID manquante (voir spikes/oauth-gmail/README.md)")?;
    let client_secret = std::env::var("GOOGLE_CLIENT_SECRET")
        .context("GOOGLE_CLIENT_SECRET manquante (voir spikes/oauth-gmail/README.md)")?;
    let refresh = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .context("accès au Credential Manager Windows")?
        .get_password()
        .context("aucun refresh token stocké : lancez d'abord `cargo run -p spike-oauth-gmail`")?;

    let oauth = BasicClient::new(ClientId::new(client_id))
        .set_client_secret(ClientSecret::new(client_secret))
        .set_auth_uri(AuthUrl::new(AUTH_URL.to_string())?)
        .set_token_uri(TokenUrl::new(TOKEN_URL.to_string())?);
    let http = oauth2::reqwest::blocking::ClientBuilder::new()
        .redirect(oauth2::reqwest::redirect::Policy::none())
        .build()
        .context("construction du client HTTP")?;

    let tokens = oauth
        .exchange_refresh_token(&RefreshToken::new(refresh))
        .request(&http)
        .context("échange du refresh token (expiré ? relancez le spike oauth-gmail)")?;
    let access = tokens.access_token().secret().clone();

    let body = http
        .get(USERINFO_URL)
        .bearer_auth(&access)
        .send()
        .context("appel userinfo")?
        .error_for_status()
        .context("réponse userinfo en erreur")?
        .text()?;
    let email = serde_json::from_str::<serde_json::Value>(&body)
        .context("parsing userinfo")?
        .get("email")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .context("champ email absent de userinfo")?;
    Ok((access, email))
}
