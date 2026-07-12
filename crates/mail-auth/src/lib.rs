//! Authentification Gmail de production : OAuth2 PKCE + coffre de l'OS.
//!
//! Les enseignements des spikes de Phase 0, en qualité bibliothèque :
//! jamais de mot de passe, refresh token dans le Credential Manager Windows,
//! vérification systématique des **scopes accordés** (le consentement
//! granulaire de Google délivre un token même sans la case Gmail cochée),
//! reconnexion silencieuse au lancement suivant.

mod flow;

use oauth2::TokenResponse;
use oauth2::basic::BasicTokenResponse;

pub use flow::AuthError;

pub(crate) const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
pub(crate) const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
pub(crate) const USERINFO_URL: &str = "https://www.googleapis.com/oauth2/v2/userinfo";
pub(crate) const SCOPE_MAIL: &str = "https://mail.google.com/";
pub(crate) const SCOPE_EMAIL: &str = "https://www.googleapis.com/auth/userinfo.email";

const KEYRING_SERVICE: &str = "discovery-mail";
const KEYRING_REFRESH: &str = "gmail-refresh-token";

/// Session authentifiée : de quoi ouvrir une connexion IMAP XOAUTH2.
/// L'access token expire (~1 h) : ré-authentifier silencieusement au besoin.
#[derive(Debug, Clone)]
pub struct Authenticated {
    pub email: String,
    pub access_token: String,
}

#[derive(Clone)]
pub struct GmailAuth {
    client_id: String,
    client_secret: String,
}

impl GmailAuth {
    pub fn new(client_id: impl Into<String>, client_secret: impl Into<String>) -> Self {
        Self {
            client_id: client_id.into(),
            client_secret: client_secret.into(),
        }
    }

    /// Configuration par variables d'environnement (secret jamais dans le code).
    pub fn from_env() -> Result<Self, AuthError> {
        let client_id = std::env::var("GOOGLE_CLIENT_ID").map_err(|_| {
            AuthError::Config(
                "GOOGLE_CLIENT_ID manquante — lancez l'application depuis un terminal \
                 où la variable est définie"
                    .to_string(),
            )
        })?;
        let client_secret = std::env::var("GOOGLE_CLIENT_SECRET")
            .map_err(|_| AuthError::Config("GOOGLE_CLIENT_SECRET manquante".to_string()))?;
        Ok(Self::new(client_id, client_secret))
    }

    /// Reconnexion sans interaction : échoue s'il n'y a pas de refresh token
    /// valide dans le coffre (→ passer par [`Self::authenticate_interactive`]).
    pub fn authenticate_silent(&self) -> Result<Authenticated, AuthError> {
        let refresh = vault()?.get_password().map_err(|err| match err {
            keyring::Error::NoEntry => AuthError::Vault("aucun compte enregistré".to_string()),
            other => AuthError::Vault(other.to_string()),
        })?;
        let client = flow::oauth_client(&self.client_id, &self.client_secret)?;
        let http = flow::http_client()?;
        let tokens = flow::refresh_access_token(&client, &http, refresh)?;
        flow::ensure_mail_scope(&tokens)?;
        self.finish(&http, &tokens, None)
    }

    /// Parcours complet : navigateur → consentement Google → redirection
    /// loopback → tokens. Le refresh token est stocké dans le coffre de l'OS.
    pub fn authenticate_interactive(&self) -> Result<Authenticated, AuthError> {
        let client = flow::oauth_client(&self.client_id, &self.client_secret)?;
        let http = flow::http_client()?;
        let tokens = flow::interactive_tokens(client, &http)?;
        flow::ensure_mail_scope(&tokens)?;
        let refresh = tokens.refresh_token().map(|token| token.secret().clone());
        self.finish(&http, &tokens, refresh)
    }

    /// Oublie le compte : supprime le refresh token du coffre.
    pub fn forget(&self) -> Result<(), AuthError> {
        match vault()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(AuthError::Vault(err.to_string())),
        }
    }

    fn finish(
        &self,
        http: &flow::HttpClient,
        tokens: &BasicTokenResponse,
        store_refresh: Option<String>,
    ) -> Result<Authenticated, AuthError> {
        if let Some(refresh) = store_refresh {
            vault()?
                .set_password(&refresh)
                .map_err(|err| AuthError::Vault(err.to_string()))?;
        }
        let access_token = tokens.access_token().secret().clone();
        let email = flow::fetch_email(http, &access_token)?;
        Ok(Authenticated {
            email,
            access_token,
        })
    }
}

fn vault() -> Result<keyring::Entry, AuthError> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_REFRESH)
        .map_err(|err| AuthError::Vault(err.to_string()))
}
