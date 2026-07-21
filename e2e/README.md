# E2E — parcours critiques (gate 2)

Pilote la **vraie fenêtre Tauri** via CDP (WebView2), sans `tauri-driver`
ni `msedgedriver` : l'application est lancée avec
`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222` et
Playwright s'y attache par `connectOverCDP` (spike validé le 2026-07-17 —
aucune danse de versions de driver).

Déterminisme par construction ([launch.mjs](launch.mjs)) :

- base seedée **jetable** (`DISCOVERY_DB_PATH`) — jamais celle de
  l'utilisateur ;
- compte factice au **jeton invalide** (`DISCOVERY_E2E_ACCOUNT`) — hors
  ligne garanti : la boîte d'envoi journalise sans jamais rien envoyer ;
- configuration OAuth retirée de l'environnement du processus testé —
  aucun test ne peut toucher au vrai compte, même par accident.

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
