//! Ce qui change d'un fournisseur OAuth2 à l'autre — et rien d'autre.
//!
//! Tout le reste du parcours (PKCE, écoute loopback, vérification CSRF,
//! coffre de l'OS, reconnexion silencieuse) est commun et vit dans
//! [`crate::flow`]. Un fournisseur se décrit ici en données ; s'il
//! demandait du code, c'est que le seam serait au mauvais endroit.
//!
//! Les valeurs Microsoft ne sont pas déduites de la documentation : elles
//! viennent du spike [`spikes/microsoft`](../../../spikes/microsoft), joué
//! contre un compte réel. Les tests de ce module les épinglent.

/// Comment le fournisseur livre l'identité du compte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Identity {
    /// Point de terminaison JSON exposant un champ `email`, appelé avec
    /// l'access token. C'est le cas Google, mesuré dès la Phase 0.
    Userinfo(&'static str),
    /// Le fournisseur ne livre pas l'email dans le périmètre de scopes
    /// demandé : c'est l'utilisateur qui le déclare à l'ajout du compte.
    ///
    /// Piste connue pour s'en passer côté Microsoft : demander
    /// `openid profile email` et lire `https://graph.microsoft.com/oidc/userinfo`.
    /// **Non mesuré** — le spike n'a jamais demandé ces scopes. Tant que
    /// ce n'est pas vérifié sur un compte réel, on déclare.
    Declared,
}

/// Un client OAuth2 de bureau est tantôt confidentiel, tantôt public.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientSecret {
    /// Google délivre un secret même aux applications installées.
    Required,
    /// Client PUBLIC : PKCE seul. Envoyer un secret ferait **refuser**
    /// l'échange par Azure AD.
    Forbidden,
}

/// Serveur de courrier d'un fournisseur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Endpoint {
    pub host: &'static str,
    pub port: u16,
}

#[derive(Debug)]
pub struct Provider {
    /// Nom affiché à l'utilisateur, y compris dans les messages d'erreur.
    pub name: &'static str,
    /// Préfixe des variables d'environnement : `{prefix}_CLIENT_ID`.
    pub env_prefix: &'static str,
    /// Valeur stockée dans la colonne `accounts.provider`. **Ne jamais
    /// changer sans migration** : les lignes déjà écrites la portent, et
    /// un compte dont la clé n'est plus reconnue devient inconnectable.
    pub account_kind: &'static str,
    /// Préfixe de l'entrée du coffre. **Ne jamais changer sans migration**
    /// (voir `vault_key`) : ce nom relie l'app aux jetons déjà stockés.
    pub vault_prefix: &'static str,
    pub auth_url: &'static str,
    pub token_url: &'static str,
    pub scopes: &'static [&'static str],
    /// Fragment qui doit apparaître dans un scope **accordé**. Les deux
    /// fournisseurs délivrent un jeton même sur consentement partiel :
    /// seule la liste accordée fait foi (leçon Phase 0, reconfirmée par
    /// le spike Microsoft).
    pub granted_scope_marker: &'static str,
    /// Microsoft distingue `localhost` de `127.0.0.1` : avec l'URI
    /// `http://localhost` enregistrée, n'importe quel port est accepté.
    pub redirect_host: &'static str,
    /// Paramètres d'autorisation propres au fournisseur.
    pub extra_auth_params: &'static [(&'static str, &'static str)],
    pub client_secret: ClientSecret,
    pub identity: Identity,
    pub imap: Endpoint,
    pub smtp: Endpoint,
}

pub static GOOGLE: Provider = Provider {
    name: "Google",
    env_prefix: "GOOGLE",
    account_kind: "gmail",
    vault_prefix: "gmail",
    auth_url: "https://accounts.google.com/o/oauth2/v2/auth",
    token_url: "https://oauth2.googleapis.com/token",
    scopes: &[
        "https://mail.google.com/",
        "https://www.googleapis.com/auth/userinfo.email",
    ],
    granted_scope_marker: "https://mail.google.com/",
    redirect_host: "127.0.0.1",
    // Sans ces deux paramètres, Google ne délivre pas de refresh token :
    // pas de reconnexion silencieuse au lancement suivant.
    extra_auth_params: &[("access_type", "offline"), ("prompt", "consent")],
    client_secret: ClientSecret::Required,
    identity: Identity::Userinfo("https://www.googleapis.com/oauth2/v2/userinfo"),
    imap: Endpoint {
        host: "imap.gmail.com",
        port: 993,
    },
    smtp: Endpoint {
        host: "smtp.gmail.com",
        port: 465,
    },
};

pub static MICROSOFT: Provider = Provider {
    name: "Microsoft",
    env_prefix: "MICROSOFT",
    account_kind: "microsoft",
    vault_prefix: "microsoft",
    // Point de terminaison « common » : comptes professionnels ET personnels.
    auth_url: "https://login.microsoftonline.com/common/oauth2/v2.0/authorize",
    token_url: "https://login.microsoftonline.com/common/oauth2/v2.0/token",
    // Scopes de la RESSOURCE Outlook, surtout pas les noms courts de Graph.
    // C'est le piège n°1 de cette intégration (ADR 0006).
    scopes: &[
        "https://outlook.office.com/IMAP.AccessAsUser.All",
        "https://outlook.office.com/SMTP.Send",
        "offline_access",
    ],
    granted_scope_marker: "IMAP.AccessAsUser",
    redirect_host: "localhost",
    // `offline_access` tient le rôle de `access_type=offline`.
    extra_auth_params: &[],
    client_secret: ClientSecret::Forbidden,
    identity: Identity::Declared,
    imap: Endpoint {
        host: "outlook.office365.com",
        port: 993,
    },
    // 587 + STARTTLS : Office 365 n'écoute pas en TLS implicite 465.
    smtp: Endpoint {
        host: "smtp.office365.com",
        port: 587,
    },
};

/// Tous les fournisseurs OAuth2 connus. Un compte générique IMAP/SMTP
/// n'en fait pas partie : il n'a pas de fournisseur, il a des serveurs.
pub static ALL: &[&Provider] = &[&GOOGLE, &MICROSOFT];

/// Retrouve le fournisseur d'un compte à partir de la valeur stockée en
/// base. `None` pour `"imap"` (compte générique) comme pour une valeur
/// inconnue — l'appelant traite les deux cas distinctement.
pub fn for_account_kind(kind: &str) -> Option<&'static Provider> {
    ALL.iter().copied().find(|p| p.account_kind == kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Régression : la généralisation par fournisseur ne doit RIEN changer
    /// au parcours Google déjà en production. Ces valeurs sont celles qui
    /// étaient câblées en constantes avant l'extraction.
    #[test]
    fn google_keeps_the_endpoints_it_had_before_extraction() {
        assert_eq!(
            GOOGLE.auth_url,
            "https://accounts.google.com/o/oauth2/v2/auth"
        );
        assert_eq!(GOOGLE.token_url, "https://oauth2.googleapis.com/token");
        assert_eq!(
            GOOGLE.scopes,
            [
                "https://mail.google.com/",
                "https://www.googleapis.com/auth/userinfo.email"
            ]
        );
        assert_eq!(GOOGLE.redirect_host, "127.0.0.1");
        assert_eq!(
            GOOGLE.identity,
            Identity::Userinfo("https://www.googleapis.com/oauth2/v2/userinfo")
        );
        assert_eq!(GOOGLE.client_secret, ClientSecret::Required);
    }

    /// Sans `access_type=offline` ET `prompt=consent`, Google ne renvoie
    /// pas de refresh token : la reconnexion silencieuse disparaît, et
    /// l'utilisateur reconsent à chaque lancement. Défaut discret, coût
    /// visible — d'où l'épingle.
    #[test]
    fn google_asks_for_a_refresh_token() {
        assert!(
            GOOGLE
                .extra_auth_params
                .contains(&("access_type", "offline"))
        );
        assert!(GOOGLE.extra_auth_params.contains(&("prompt", "consent")));
    }

    /// Les valeurs mesurées par le spike, pas celles de la documentation.
    #[test]
    fn microsoft_matches_what_the_spike_measured() {
        assert_eq!(
            MICROSOFT.auth_url,
            "https://login.microsoftonline.com/common/oauth2/v2.0/authorize"
        );
        assert_eq!(
            MICROSOFT.scopes,
            [
                "https://outlook.office.com/IMAP.AccessAsUser.All",
                "https://outlook.office.com/SMTP.Send",
                "offline_access"
            ]
        );
        assert_eq!(MICROSOFT.imap.host, "outlook.office365.com");
        assert_eq!(MICROSOFT.smtp.host, "smtp.office365.com");
    }

    /// Les deux pièges gelés par l'ADR 0006, chacun tenu par une assertion :
    /// un client public ne doit envoyer aucun secret, et `127.0.0.1` n'est
    /// pas `localhost` pour Azure AD.
    #[test]
    fn microsoft_is_a_public_client_redirecting_to_localhost() {
        assert_eq!(MICROSOFT.client_secret, ClientSecret::Forbidden);
        assert_eq!(MICROSOFT.redirect_host, "localhost");
    }

    /// Le port SMTP de Microsoft est 587/STARTTLS. C'est ce que le bug #3
    /// rendait injoignable ; la donnée est maintenant portée par le
    /// fournisseur, plus par une constante d'application.
    #[test]
    fn microsoft_submits_mail_on_587_not_465() {
        assert_eq!(MICROSOFT.smtp.port, 587);
        assert_eq!(GOOGLE.smtp.port, 465);
    }

    /// Deux fournisseurs ne doivent jamais se disputer une entrée du
    /// coffre : leurs préfixes sont distincts, et celui de Google reste
    /// `gmail` — le renommer orphelinerait les jetons déjà stockés.
    #[test]
    fn vault_prefixes_are_distinct_and_google_keeps_its_historical_one() {
        assert_eq!(GOOGLE.vault_prefix, "gmail");
        assert_ne!(GOOGLE.vault_prefix, MICROSOFT.vault_prefix);
    }

    /// Même classe de piège que la clé du coffre, côté base cette fois :
    /// les lignes `accounts` déjà écrites portent `"gmail"`. Renommer
    /// rendrait les comptes existants inconnectables, sans qu'aucun test
    /// ne s'en aperçoive — d'où l'épingle.
    #[test]
    fn account_kinds_are_frozen_and_resolvable() {
        assert_eq!(GOOGLE.account_kind, "gmail");
        assert!(std::ptr::eq(
            for_account_kind("gmail").expect("Google"),
            &GOOGLE
        ));
        assert!(std::ptr::eq(
            for_account_kind("microsoft").expect("Microsoft"),
            &MICROSOFT
        ));
    }

    /// Un compte générique n'a pas de fournisseur OAuth2 : la table ne
    /// doit surtout pas lui en inventer un.
    #[test]
    fn generic_accounts_have_no_oauth_provider() {
        assert!(for_account_kind("imap").is_none());
        assert!(for_account_kind("").is_none());
    }

    /// Deux fournisseurs qui partageraient une clé se voleraient leurs
    /// comptes au démarrage. La table est petite ; l'invariant, lui, doit
    /// survivre au troisième fournisseur.
    #[test]
    fn no_two_providers_share_an_account_kind() {
        for (index, provider) in ALL.iter().enumerate() {
            for other in &ALL[index + 1..] {
                assert_ne!(provider.account_kind, other.account_kind);
                assert_ne!(provider.vault_prefix, other.vault_prefix);
                assert_ne!(provider.env_prefix, other.env_prefix);
            }
        }
    }
}
