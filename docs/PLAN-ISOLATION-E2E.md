# PLAN-ISOLATION-E2E — deux suites e2e simultanées ne doivent plus se marcher dessus

**CHANTIER SOLDÉ le 2026-08-15 — terrain complet.** Commit `ec1fe61`
(rebasé sur `554a899`, A41), CI verte (run 31894070191, 2 min 14 s).
GO CE du plan le 2026-08-15 (D1 port libre dynamique, D2 bancs alignés,
§5) ; aucune retouche terrain — la passe a été verte du premier coup.
Aucun ADR : la décision vit dans le harnais de test (ADR 0005 porte déjà
la doctrine « e2e en local ») et ce plan en est la trace.

Validation terrain CE (2026-08-15) : deux worktrees porteurs du fix
(nifty-benz-78d858 + copie jetable essai-isolation sur 554a899), deux
`npm test` simultanés — **73 + 73 verts (1,9 min et 1,5 min), zéro échec
croisé, aucune application morte**. Non-régression solo : 73 verts en
2,8 min (chauffe à froid comprise pour le Rust).

Gate complète du 2026-08-15 : fmt ✅ · build ui-v2 ✅ (0 avertissement) ·
contrastes ✅ (26 paires × 7 thèmes) · cohérence Système ✅ (119 valeurs) ·
garde thread principal ✅ (62 commandes) · clippy ✅ · 422 tests Rust ✅ ·
doc-tests ✅ · e2e ✅ (2 tests port-cdp + 73 parcours, 3,6 min, 0 échec).
Revue `/code-review high` : 2 constats (balayage inter-worktrees CONFIRMÉ,
args WebView2 changeants PLAUSIBLE), les 2 corrigés avant la gate.

## 1. Constat (2026-08-15)

Deux worktrees Claude (`xenodochial-lehmann-625764` et `sweet-swartz-fc87b1`)
ont joué leur suite e2e en même temps — chacune via son hook pre-push.
Conséquences observées :

- collisions sur le port CDP **9222**, codé en dur dans `e2e/launch.mjs`
  (`CDP_PORT = 9222`), `e2e/mesure-v2.mjs` et `e2e/diag-v2.mjs` ;
- applications mortes au démarrage (code `0xFFFFFFFF`, sans sortie) ;
- échecs qui se promènent d'un spec à l'autre ;
- trois push bloqués par le hook pre-push.

Analyse du partage d'état, fichier par fichier :

| Ressource | Portée | Collision ? |
|---|---|---|
| Base de test (`WIND_DB_PATH`) | `<worktree>/target/e2e/*.db` | non — par worktree |
| Profil WebView2 | `<worktree>/target/e2e/webview2` | non — par worktree |
| Binaire `wind-desktop.exe` | `<worktree>/target/debug/` | non — par worktree |
| **Port CDP 9222** | **TCP, machine entière** | **oui — état partagé** |
| **Balayage de zombies** (`rebuild-v2.mjs`) | `Stop-Process` sur tout `wind-desktop` sous `*\target\*` | **oui — tue l'app de l'AUTRE worktree** |

Le second point a été trouvé par la revue à regard neuf (Phase 3), pas
par le constat initial : `construireV2` — joué à CHAQUE `launchAppV2` —
abat tout `wind-desktop` issu d'un `target/`, quel que soit le worktree.
`Stop-Process -Force` produit exactement un code de sortie `0xFFFFFFFF`
sans sortie : c'est la signature des « applications mortes au
démarrage » du constat. L'isolation du port, seule, n'aurait PAS suffi.

Mécanique de l'échec croisé : `--remote-debugging-port` est passé au
WebView2 des DEUX applications ; une seule peut écouter. Ensuite,
`connectOverCDP('http://127.0.0.1:9222')` reconnaît « sa » fenêtre au seul
critère `url().includes('tauri.localhost')` — vrai pour n'importe quelle
fenêtre Wind. Une suite peut donc piloter (et fermer) la fenêtre de
l'autre : les symptômes erratiques sont exactement ceux d'un pilote
attaché au mauvais avion.

La CI hébergée ne joue PAS les e2e (ADR 0005 ; `ci.yml` l'écrit) : seul le
hook pre-push les joue, en local. La contrainte réelle est donc « N suites
sur la même machine », pas « reproduire un port en CI ».

## 2. Périmètre

- `e2e/launch.mjs` : le port CDP cesse d'être une constante partagée.
- `e2e/mesure-v2.mjs` et `e2e/diag-v2.mjs` : mêmes consommateurs du port
  en dur, même remède (D2).
- `e2e/README.md` : la phrase qui documente `--remote-debugging-port=9222`.

## 3. Refus de périmètre

- **Pas de verrou inter-processus** en plus du port isolé : une fois le
  port unique par lancement, deux suites cohabitent sans se voir. Un
  verrou sérialiserait les gates (attente de plusieurs minutes par
  worktree) pour ne couvrir aucun risque restant identifié. Si le terrain
  révèle une autre ressource partagée (charge machine, WebView2 lui-même),
  on rouvrira.
- **Pas de changement côté Rust** : l'application ne connaît pas le port,
  il transite par `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`. Rien à toucher.
- **Pas de parallélisme intra-suite** : `workers: 1` reste ; une seule
  fenêtre pilotée par suite, comme aujourd'hui.

## 4. Options et verdicts

| Option | Principe | Verdict |
|---|---|---|
| A. Port dérivé du chemin (hash du worktree → 9222+n) | déterministe par worktree | collision de hash possible ; ne voit pas un port occupé par un tiers ; toujours un état implicite partagé (la plage) |
| B. **Port libre dynamique** (bind éphémère sur 0, l'OS choisit, port passé à `connectOverCDP`) | aucun état partagé, aucune configuration | fenêtre TOCTOU minuscule (entre la sonde et le bind WebView2) — bénigne : l'échec est bruyant et la relance choisit un autre port |
| C. Verrou inter-processus | sérialise les suites | gates sérialisées = minutes d'attente par worktree ; ne corrige pas la racine (l'état partagé demeure) |

**Recommandation : B.** C'est la seule option qui supprime l'état partagé
au lieu de le gérer. A est un B moins sûr ; C paie en latence ce que B
obtient gratuitement. B rend aussi `connectOverCDP` sans ambiguïté : sur
un port privé, la seule page `tauri.localhost` est la nôtre.

## 5. Décisions CE

- **D1 — stratégie d'isolation** : port libre dynamique (B, recommandé),
  port dérivé du chemin (A), ou verrou sérialisant (C) ?
  **CE, 2026-08-15 : « Port libre dynamique (Recommandé) ».**
- **D2 — périmètre des bancs manuels** : appliquer le même port dynamique
  à `mesure-v2.mjs` et `diag-v2.mjs` (recommandé — mêmes consommateurs,
  même machine, un banc peut tourner pendant qu'une gate joue), ou les
  laisser en 9222 (bancs joués un à la fois, à la main) ?
  **CE, 2026-08-15 : « Oui, mêmes ports isolés (Recommandé) ».**

## 6. Étapes

- **E1** — `e2e/port-cdp.mjs` : `allouerPortCdp()` (bind éphémère,
  port de l'OS, fermeture). Test node RED→GREEN : le port rendu est
  liable ; deux allocations pendant qu'un port est tenu ne rendent pas le
  port tenu. Gate : `node --test` vert.
- **E2** — `launch.mjs` : le port devient une variable de la SUITE (un
  port par processus, mémoïsé — deux lancements de la même gate partagent
  le profil WebView2 et doivent porter des arguments navigateur
  identiques) ; messages d'échec (`startupFailure`) et `closeApp`
  inchangés dans leur contrat, mais portent le port réel. Selon D2 :
  `mesure-v2.mjs` et `diag-v2.mjs` alignés. Gate : suite e2e complète.
- **E2bis** (issu de la revue) — `rebuild-v2.mjs` : le balayage de
  zombies borné au `target/` du worktree courant (`$_.Path -like
  '<root>\target\*'`). Pas de test unitaire : la seule assertion utile
  exigerait deux binaires vivants dans deux `target/` distincts — c'est
  précisément le scénario de la validation terrain (STOP 2), un RED
  simulé n'apprendrait rien.
- **E3** — `e2e/README.md` amendé ; gate complète (`/gate`).
- **Validation terrain (STOP 2)** — deux worktrees, deux `npm test`
  simultanés, zéro échec croisé.

Aucune UI touchée : pas d'amendement du Système (DC-D2 sans objet).
