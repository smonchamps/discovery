# Spike Phase 0 — OAuth2 PKCE + IMAP XOAUTH2 (Gmail)

**Question à trancher :** peut-on authentifier le client sur Gmail sans que le
produit ne voie jamais un mot de passe, avec une reconnexion silencieuse au
deuxième lancement ?

**Statut : jetable.** Ce code valide des décisions ; il sera supprimé une fois
la Phase 0 conclue (voir `docs/PLAN.md`, §2.3).

## Ce que le spike démontre

1. Flux OAuth2 *authorization code + PKCE* avec redirection loopback
   (`http://127.0.0.1:<port aléatoire>`), le flux recommandé pour les apps desktop.
2. Stockage du refresh token dans le **Credential Manager Windows** (crate `keyring`) —
   aucun secret sur disque, aucun secret dans le code.
3. Reconnexion silencieuse : au 2ᵉ lancement, pas de navigateur, le refresh token suffit.
4. Connexion IMAP `AUTHENTICATE XOAUTH2` à `imap.gmail.com` et lecture des
   5 derniers sujets d'INBOX (l'équivalent de l'ancien spike, sans mot de passe).

## Prérequis (une fois, ~10 minutes)

1. <https://console.cloud.google.com> → créer un projet (ex. `discovery-dev`).
2. **APIs & Services → OAuth consent screen** : type *External*, statut *Testing*,
   et ajoutez votre adresse Gmail comme *test user*. (En mode Testing, le scope
   restreint `https://mail.google.com/` fonctionne sans audit — limite : 100 testeurs,
   refresh tokens expirant après 7 jours. L'audit CASA reste nécessaire pour le
   lancement public, voir PLAN.md §7.)
3. **Credentials → Create credentials → OAuth client ID** : type **Desktop app**.
4. Notez le *Client ID* et le *Client secret*.

## Lancer

```powershell
$env:GOOGLE_CLIENT_ID = "…"
$env:GOOGLE_CLIENT_SECRET = "…"
cargo run -p spike-oauth-gmail
```

Premier lancement : le navigateur s'ouvre, vous consentez, le spike affiche les
5 derniers sujets. Deuxième lancement : plus de navigateur, même résultat.

> **Piège fréquent :** sur l'écran de consentement, Google affiche des **cases à
> cocher** par autorisation. Si la case Gmail (« Lire, rédiger, envoyer… vos
> e-mails ») n'est pas cochée, Google délivre quand même un token — mais l'IMAP
> échouera avec `Invalid credentials`. Le spike vérifie les scopes réellement
> accordés et vous le signale explicitement dans ce cas.

## Verdict (validé le 2026-07-11 sur compte réel)

**Question tranchée : oui**, l'authentification Gmail sans mot de passe fonctionne
de bout en bout, reconnexion silencieuse comprise. Enseignements pour la suite :

1. **Le consentement granulaire de Google est un piège UX de premier ordre.**
   Google délivre un token même si l'utilisateur décoche la case Gmail ; seul
   l'examen des scopes *accordés* (pas demandés) le révèle. Le vrai client devra
   vérifier les scopes après chaque autorisation et guider l'utilisateur pour
   re-consentir — ce spike contient le flux auto-réparateur de référence.
2. **Les sujets IMAP arrivent encodés RFC 2047** (`=?UTF-8?Q?…?=`) et découpés
   en fragments. Décoder ça à la main serait une erreur : cela confirme
   l'adoption de `mail-parser` (Stalwart) prévue au plan pour la Phase 1.
3. **En mode Testing, le refresh token expire après 7 jours** : une
   ré-autorisation hebdomadaire est normale pendant le développement.
   L'audit CASA reste indispensable avant tout lancement public (PLAN.md §7).

## Nettoyage

- Révoquer l'accès : <https://myaccount.google.com/permissions>.
- Supprimer le token local : Gestionnaire d'identification Windows →
  entrée `discovery-spike-oauth`.
