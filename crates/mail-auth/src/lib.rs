//! Authentification OAuth2 de production : PKCE + coffre de l'OS.
//!
//! Les enseignements des spikes de Phase 0, en qualité bibliothèque :
//! jamais de mot de passe, refresh token dans le Credential Manager Windows,
//! vérification systématique des **scopes accordés** (le consentement
//! granulaire délivre un token même sans la case courrier cochée),
//! reconnexion silencieuse au lancement suivant.
//!
//! Le parcours est UN seul, quel que soit le fournisseur ; ce qui les
//! distingue est décrit en données dans [`provider`].

mod flow;
mod provider;

use oauth2::TokenResponse;
use oauth2::basic::BasicTokenResponse;

pub use flow::AuthError;
pub use provider::{
    ALL as PROVIDERS, ClientSecret, Endpoint, GOOGLE, Identity, MICROSOFT, Provider,
    for_account_kind,
};

const KEYRING_SERVICE: &str = "discovery-mail";
/// Entrée héritée de la Phase 2 (un seul compte) — lue en repli puis
/// migrée vers l'entrée par compte : pas de ré-authentification après
/// la mise à jour multi-comptes.
const KEYRING_REFRESH_LEGACY: &str = "gmail-refresh-token";

/// Session authentifiée : de quoi ouvrir une connexion IMAP XOAUTH2.
/// L'access token expire (~1 h) : ré-authentifier silencieusement au besoin.
///
/// Le fournisseur voyage avec la session : c'est lui qui porte les serveurs
/// à joindre, plus une constante d'application.
#[derive(Debug, Clone)]
pub struct Authenticated {
    pub provider: &'static Provider,
    pub email: String,
    pub access_token: String,
}

/// Credentials d'un compte IMAP/SMTP générique (serveur, port, mot de
/// passe). Le mot de passe est en mémoire uniquement pendant la session ;
/// il est lu depuis le coffre de l'OS au démarrage.
#[derive(Debug, Clone)]
pub struct GenericCredentials {
    pub email: String,
    pub username: String,
    pub password: String,
    pub imap_host: String,
    pub imap_port: u16,
    pub smtp_host: String,
    pub smtp_port: u16,
}

/// Session d'un compte connecté, quelle que soit sa méthode
/// d'authentification. C'est ce qui circule dans l'état applicatif du
/// desktop.
#[derive(Debug, Clone)]
pub enum AccountSession {
    /// Compte authentifié par OAuth2, quel que soit le fournisseur.
    OAuth(Authenticated),
    Generic(GenericCredentials),
}

impl AccountSession {
    pub fn email(&self) -> &str {
        match self {
            AccountSession::OAuth(auth) => &auth.email,
            AccountSession::Generic(creds) => &creds.email,
        }
    }
}

/// Authentificateur OAuth2 d'UN fournisseur.
#[derive(Clone)]
pub struct Authenticator {
    provider: &'static Provider,
    client_id: String,
    /// `None` pour un client public (Microsoft) : présenter un secret y
    /// ferait refuser l'échange.
    client_secret: Option<String>,
}

impl Authenticator {
    pub fn new(
        provider: &'static Provider,
        client_id: impl Into<String>,
        client_secret: Option<String>,
    ) -> Self {
        Self {
            provider,
            client_id: client_id.into(),
            client_secret,
        }
    }

    pub fn provider(&self) -> &'static Provider {
        self.provider
    }

    /// Configuration par variables d'environnement `{PREFIXE}_CLIENT_ID`
    /// et `{PREFIXE}_CLIENT_SECRET` (secret jamais dans le code).
    pub fn from_env(provider: &'static Provider) -> Result<Self, AuthError> {
        let id_var = format!("{}_CLIENT_ID", provider.env_prefix);
        let client_id = std::env::var(&id_var).map_err(|_| {
            AuthError::Config(format!(
                "{id_var} manquante — lancez l'application depuis un terminal \
                 où la variable est définie"
            ))
        })?;
        let client_secret = match provider.client_secret {
            ClientSecret::Required => {
                let secret_var = format!("{}_CLIENT_SECRET", provider.env_prefix);
                Some(
                    std::env::var(&secret_var)
                        .map_err(|_| AuthError::Config(format!("{secret_var} manquante")))?,
                )
            }
            ClientSecret::Forbidden => None,
        };
        Ok(Self::new(provider, client_id, client_secret))
    }

    /// Raccourci du fournisseur historique — le seul câblé à l'UI à ce jour.
    pub fn google_from_env() -> Result<Self, AuthError> {
        Self::from_env(&GOOGLE)
    }

    /// Reconnexion sans interaction d'UN compte : lit son entrée du
    /// coffre (une par email), avec repli sur l'entrée héritée de la
    /// Phase 2 — migrée vers l'entrée par compte au passage. Échoue s'il
    /// n'y a aucun jeton (→ [`Self::authenticate_interactive`]).
    pub fn authenticate_silent(&self, email: &str) -> Result<Authenticated, AuthError> {
        let (refresh, from_legacy) = match self.vault(email)?.get_password() {
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
        let client = self.client()?;
        let http = flow::http_client()?;
        let tokens = flow::refresh_access_token(&client, &http, refresh.clone())?;
        flow::ensure_mail_scope(self.provider, &tokens)?;
        let account = self.finish(&http, &tokens, Some(email), None)?;
        if from_legacy {
            // Migration du coffre : l'entrée devient par-compte, sous
            // l'email RÉEL du jeton (celui que le fournisseur confirme).
            self.vault(&account.email)?
                .set_password(&refresh)
                .map_err(|err| AuthError::Vault(err.to_string()))?;
            let _ = legacy_vault()?.delete_credential();
        }
        Ok(account)
    }

    /// Reconnexion héritée Phase 2 : quand la base ne connaît encore
    /// aucun compte, l'entrée non-keyée du coffre peut en révéler un —
    /// elle est alors migrée. L'email revient du jeton lui-même.
    ///
    /// Ce chemin est propre à Google : la Phase 2 ne connaissait que lui.
    pub fn authenticate_silent_legacy(&self) -> Result<Authenticated, AuthError> {
        let refresh = legacy_vault()?.get_password().map_err(|err| match err {
            keyring::Error::NoEntry => AuthError::Vault("aucun compte enregistré".to_string()),
            other => AuthError::Vault(other.to_string()),
        })?;
        let client = self.client()?;
        let http = flow::http_client()?;
        let tokens = flow::refresh_access_token(&client, &http, refresh.clone())?;
        flow::ensure_mail_scope(self.provider, &tokens)?;
        let account = self.finish(&http, &tokens, None, None)?;
        self.vault(&account.email)?
            .set_password(&refresh)
            .map_err(|err| AuthError::Vault(err.to_string()))?;
        let _ = legacy_vault()?.delete_credential();
        Ok(account)
    }

    /// Parcours complet : navigateur → consentement → redirection loopback
    /// → tokens. Le refresh token est stocké dans le coffre de l'OS.
    ///
    /// `declared_email` n'est utilisé que par les fournisseurs qui ne
    /// livrent pas l'identité du compte ([`Identity::Declared`]).
    pub fn authenticate_interactive(
        &self,
        declared_email: Option<&str>,
    ) -> Result<Authenticated, AuthError> {
        let client = self.client()?;
        let http = flow::http_client()?;
        let tokens = flow::interactive_tokens(self.provider, client, &http)?;
        flow::ensure_mail_scope(self.provider, &tokens)?;
        let refresh = tokens.refresh_token().map(|token| token.secret().clone());
        self.finish(&http, &tokens, declared_email, refresh)
    }

    /// Oublie UN compte : supprime son refresh token du coffre.
    pub fn forget(&self, email: &str) -> Result<(), AuthError> {
        match self.vault(email)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(AuthError::Vault(err.to_string())),
        }
    }

    fn client(&self) -> Result<flow::OauthClient, AuthError> {
        flow::oauth_client(
            self.provider,
            &self.client_id,
            self.client_secret.as_deref(),
        )
    }

    fn vault(&self, email: &str) -> Result<keyring::Entry, AuthError> {
        keyring::Entry::new(KEYRING_SERVICE, &vault_key(self.provider, email))
            .map_err(|err| AuthError::Vault(err.to_string()))
    }

    /// Chez un fournisseur qui livre l'identité, l'email n'est connu
    /// qu'APRÈS l'échange de jetons : le refresh se range donc dans
    /// l'entrée du compte une fois l'identité confirmée.
    fn finish(
        &self,
        http: &flow::HttpClient,
        tokens: &BasicTokenResponse,
        declared_email: Option<&str>,
        store_refresh: Option<String>,
    ) -> Result<Authenticated, AuthError> {
        let access_token = tokens.access_token().secret().clone();
        let email = flow::resolve_email(self.provider, http, &access_token, declared_email)?;
        if let Some(refresh) = store_refresh {
            self.vault(&email)?
                .set_password(&refresh)
                .map_err(|err| AuthError::Vault(err.to_string()))?;
        }
        Ok(Authenticated {
            provider: self.provider,
            email,
            access_token,
        })
    }
}

/// Nom de l'entrée du coffre pour le refresh token d'un compte.
///
/// **Ne jamais changer sans migration.** Ce nom est la seule chose qui
/// relie l'application aux jetons déjà stockés sur la machine de
/// l'utilisateur : le modifier ne casse aucun test mais force une
/// ré-authentification silencieuse de tous les comptes. C'est pour cela
/// que le préfixe de Google reste `gmail`, hérité de la Phase 2.
fn vault_key(provider: &Provider, email: &str) -> String {
    format!("{}-refresh:{email}", provider.vault_prefix)
}

fn legacy_vault() -> Result<keyring::Entry, AuthError> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_REFRESH_LEGACY)
        .map_err(|err| AuthError::Vault(err.to_string()))
}

const KEYRING_GENERIC_PASSWORD: &str = "generic-password";

fn generic_vault(email: &str) -> Result<keyring::Entry, AuthError> {
    keyring::Entry::new(
        KEYRING_SERVICE,
        &format!("{KEYRING_GENERIC_PASSWORD}:{email}"),
    )
    .map_err(|err| AuthError::Vault(err.to_string()))
}

/// Stocke le mot de passe d'un compte IMAP/SMTP générique dans le coffre.
pub fn store_generic_password(email: &str, password: &str) -> Result<(), AuthError> {
    generic_vault(email)?
        .set_password(password)
        .map_err(|err| AuthError::Vault(err.to_string()))
}

/// Récupère le mot de passe d'un compte IMAP/SMTP générique depuis le coffre.
pub fn fetch_generic_password(email: &str) -> Result<String, AuthError> {
    generic_vault(email)?
        .get_password()
        .map_err(|err| match err {
            keyring::Error::NoEntry => AuthError::Vault(format!("aucun mot de passe pour {email}")),
            other => AuthError::Vault(other.to_string()),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test de caractérisation, écrit AVANT la généralisation par
    /// fournisseur : il fige les noms d'entrée du coffre.
    ///
    /// Aucun test ne peut échouer si on les renomme — le coffre est dans
    /// l'OS, pas dans le dépôt. Le symptôme serait silencieux et différé :
    /// tous les comptes déjà connectés redemanderaient un consentement au
    /// prochain lancement. D'où cette épingle.
    #[test]
    fn vault_entry_names_are_frozen() {
        assert_eq!(
            vault_key(&GOOGLE, "moi@exemple.fr"),
            "gmail-refresh:moi@exemple.fr"
        );
        assert_eq!(KEYRING_SERVICE, "discovery-mail");
        assert_eq!(KEYRING_REFRESH_LEGACY, "gmail-refresh-token");
        assert_eq!(KEYRING_GENERIC_PASSWORD, "generic-password");
    }

    /// Deux fournisseurs pour la même adresse ne doivent jamais écrire
    /// dans la même entrée : le second écraserait le jeton du premier, et
    /// la panne — une déconnexion silencieuse — arriverait bien plus tard.
    #[test]
    fn two_providers_never_share_a_vault_entry() {
        assert_ne!(
            vault_key(&GOOGLE, "moi@exemple.fr"),
            vault_key(&MICROSOFT, "moi@exemple.fr")
        );
    }
}
