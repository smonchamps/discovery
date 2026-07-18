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
/// Entrée héritée de la Phase 2 (un seul compte) — lue en repli puis
/// migrée vers l'entrée par compte : pas de ré-authentification après
/// la mise à jour multi-comptes.
const KEYRING_REFRESH_LEGACY: &str = "gmail-refresh-token";

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

    /// Reconnexion sans interaction d'UN compte : lit son entrée du
    /// coffre (une par email), avec repli sur l'entrée héritée de la
    /// Phase 2 — migrée vers l'entrée par compte au passage. Échoue s'il
    /// n'y a aucun jeton (→ [`Self::authenticate_interactive`]).
    pub fn authenticate_silent(&self, email: &str) -> Result<Authenticated, AuthError> {
        let (refresh, from_legacy) = match vault(email)?.get_password() {
            Ok(token) => (token, false),
            Err(keyring::Error::NoEntry) => {
                let legacy = legacy_vault()?.get_password().map_err(|err| match err {
                    keyring::Error::NoEntry => {
                        AuthError::Vault(format!("aucun jeton pour {email}"))
                    }
                    other => AuthError::Vault(other.to_string()),
                })?;
                (legacy, true)
            }
            Err(other) => return Err(AuthError::Vault(other.to_string())),
        };
        let client = flow::oauth_client(&self.client_id, &self.client_secret)?;
        let http = flow::http_client()?;
        let tokens = flow::refresh_access_token(&client, &http, refresh.clone())?;
        flow::ensure_mail_scope(&tokens)?;
        let account = self.finish(&http, &tokens, None)?;
        if from_legacy {
            // Migration du coffre : l'entrée devient par-compte, sous
            // l'email RÉEL du jeton (celui que Google vient de confirmer).
            vault(&account.email)?
                .set_password(&refresh)
                .map_err(|err| AuthError::Vault(err.to_string()))?;
            let _ = legacy_vault()?.delete_credential();
        }
        Ok(account)
    }

    /// Reconnexion héritée Phase 2 : quand la base ne connaît encore
    /// aucun compte, l'entrée non-keyée du coffre peut en révéler un —
    /// elle est alors migrée. L'email revient du jeton lui-même.
    pub fn authenticate_silent_legacy(&self) -> Result<Authenticated, AuthError> {
        let refresh = legacy_vault()?.get_password().map_err(|err| match err {
            keyring::Error::NoEntry => AuthError::Vault("aucun compte enregistré".to_string()),
            other => AuthError::Vault(other.to_string()),
        })?;
        let client = flow::oauth_client(&self.client_id, &self.client_secret)?;
        let http = flow::http_client()?;
        let tokens = flow::refresh_access_token(&client, &http, refresh.clone())?;
        flow::ensure_mail_scope(&tokens)?;
        let account = self.finish(&http, &tokens, None)?;
        vault(&account.email)?
            .set_password(&refresh)
            .map_err(|err| AuthError::Vault(err.to_string()))?;
        let _ = legacy_vault()?.delete_credential();
        Ok(account)
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

    /// Oublie UN compte : supprime son refresh token du coffre.
    pub fn forget(&self, email: &str) -> Result<(), AuthError> {
        match vault(email)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(AuthError::Vault(err.to_string())),
        }
    }

    /// L'email n'est connu qu'APRÈS l'échange de jetons : le refresh se
    /// range donc dans l'entrée du compte une fois l'identité confirmée.
    fn finish(
        &self,
        http: &flow::HttpClient,
        tokens: &BasicTokenResponse,
        store_refresh: Option<String>,
    ) -> Result<Authenticated, AuthError> {
        let access_token = tokens.access_token().secret().clone();
        let email = flow::fetch_email(http, &access_token)?;
        if let Some(refresh) = store_refresh {
            vault(&email)?
                .set_password(&refresh)
                .map_err(|err| AuthError::Vault(err.to_string()))?;
        }
        Ok(Authenticated {
            email,
            access_token,
        })
    }
}

fn vault(email: &str) -> Result<keyring::Entry, AuthError> {
    keyring::Entry::new(KEYRING_SERVICE, &format!("gmail-refresh:{email}"))
        .map_err(|err| AuthError::Vault(err.to_string()))
}

fn legacy_vault() -> Result<keyring::Entry, AuthError> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_REFRESH_LEGACY)
        .map_err(|err| AuthError::Vault(err.to_string()))
}
