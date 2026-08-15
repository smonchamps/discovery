# E2E — parcours critiques (gate 2)

Pilote la **vraie fenêtre Tauri** via CDP (WebView2), sans `tauri-driver`
ni `msedgedriver` : l'application est lancée avec
`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222` et
Playwright s'y attache par `connectOverCDP` (spike validé le 2026-07-17 —
aucune danse de versions de driver).

Déterminisme par construction ([launch.mjs](launch.mjs)) :

- base seedée **jetable** (`WIND_DB_PATH`) — jamais celle de
  l'utilisateur ;
- compte factice au **jeton invalide** (`WIND_E2E_ACCOUNT`) — hors
  ligne garanti : la boîte d'envoi journalise sans jamais rien envoyer ;
- configuration OAuth retirée de l'environnement du processus testé —
  aucun test ne peut toucher au vrai compte, même par accident. La liste
  des variables vit dans [isolation-oauth.json](isolation-oauth.json)
  (contrat unique, appliqué par [isolation.mjs](isolation.mjs)) : tout
  lanceur — suite, banc de mesure, sonde — la purge en entier.

## Lancer

Prérequis : Node ≥ 20, Rust, WebView2 (présent sur Windows 11).

```powershell
cd e2e
npm install
npm test
```

La suite construit l'application (debug), seed 200 messages avec corps,
ouvre la fenêtre et déroule les parcours en ~10 s.

## Le gate : hook pré-push (à armer sur chaque machine)

Ces parcours **ne tournent pas dans la CI hébergée** : un runner GitHub
n'ouvre pas de fenêtre WebView2 (mesuré — [ADR 0005](../docs/adr/0005-gate-e2e-hors-ci-hebergee.md)).
Ils sont donc joués par un hook `pre-push` versionné. Sur un dépôt
fraîchement cloné, l'armer une fois :

```powershell
git config core.hooksPath .githooks
```

Le hook ([.githooks/pre-push](../.githooks/pre-push)) enchaîne `cargo fmt
--check`, `cargo clippy -D warnings`, `cargo test --workspace`, puis ces
E2E. S'il passe, la CI est verte par construction.

En cas d'urgence : `git push --no-verify` — en connaissance de cause.

## Parcours couverts

| Parcours | Vérifié |
|---|---|
| Lire | liste virtualisée, plus récent d'abord, corps affiché dans l'iframe sandbox |
| Trier | `e` archive, décompte mis à jour, auto-avance au message suivant |
| Répondre | À / « Re: » / citation pré-remplis ; envoi hors ligne → **journalisé, « 1 en attente »** (la règle d'or, visible à l'écran) |
| Brouillon | Échap conserve le texte, Reprendre le restitue intact |

## Contrat de sélecteurs de test (R0-S6)

Le gate sélectionne l'UI par trois moyens **stables**, pour survivre à la
refonte v2 sans réécrire les tests (*lockstep*). **v2 doit honorer ce
contrat — toute modification ici est une modification du gate, à traiter
comme une API.**

1. **`data-testid` sur le markup généré** (`app.js` le pose ; les composants
   v2 portent les mêmes valeurs) :
   `message-row`, `search-result`, `thread-item`, `attachment`, `subject`,
   `thread-count`, `clip`, `account-dot`, `account-chip`, `move-target`,
   `draft-row`.
2. **IDs sémantiques préservés** pour les structures de haut niveau — v2
   porte les mêmes `id` :
   - composeur : `#compose` `#compose-title` `#compose-to` `#compose-subject`
     `#compose-body` `#compose-send` `#compose-from` `#compose-from-row`
   - lecture : `#detail` `#detail-subject` `#detail-frame` `#attachments`
     `#star` `#move`
   - liste / recherche : `#rows` `#scroll-space` `#perf` `#search`
     `#search-results` `#status`
   - ajout de compte : `#connect` `#add-menu` `#add-gmail` `#add-microsoft`
     `#add-imap` `#ms-dialog` `#ms-email` `#imap-dialog` `#imap-email`
     `#imap-host` `#imap-password` `#smtp-host` `#imap-form`
   - dialogues / bandeaux : `#move-dialog` `#outbox-bar` `#outbox-summary`
     `#drafts-bar` `#drafts-summary` `#drafts-list` `#update-bar`
     `#telemetry-optin-bar` `#crash-report-bar` `#backfill-bar`
3. **Classes d'état conservées** : `flagged` (sur `message-row`), `current`
   (sur `thread-item`).
4. **Nom accessible** : le bouton « Reprendre » (sélectionné par rôle).

**Décision (GO S6).** On désolidarise ce qui *bouge* — le markup généré,
passé en `data-testid`. Ce qui est *déjà stable* — IDs sémantiques et
classes d'état — reste sélectionné directement, comme contrat explicite
plutôt que churn sans gain. Départage assumé : « la justesse par la
retenue » vaut aussi pour le testing.
