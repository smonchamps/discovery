# Plan — WIND : Discovery devient Wind, marque de la suite Elements

Commande (2026-08-14) : le client s'appelle désormais **Wind**, outil
courrier/agenda de la suite **Elements** (« ce que le vent porte, le
rythme des jours »). Il gagne le logo dessiné à la page de la suite
(`Elements - Suite (standalone).html`) : une **enveloppe** (SVG 48×48,
trait 3, bouts ronds) posée sur une **tuile** arrondie `#e2ebe8`
(icône `#365a4f`, rayon 15/64), et une **pastille « W »** en coin
(25/64, rayon 8, fond `#365a4f`, lettre blanche, décalée −6 px).

État des lieux : `git grep -i discovery` rend **147 occurrences dans
58 fichiers**. Elles ne sont pas équivalentes — quatre couches à
traiter différemment, plus les archives qu'on ne touche pas.

## 1. L'inventaire

### A. Le nom visible à l'utilisateur (renommage direct)

| Où | Quoi |
|---|---|
| `apps/desktop/tauri.conf.json:14` | titre de fenêtre `discovery` |
| `apps/desktop/ui-v2/index.html:5` | `<title>discovery</title>` |
| `apps/desktop/ui-v2/src/App.svelte:800` | `<span class="marque">Discovery</span>` (barre du haut) |
| `apps/desktop/ui-v2/src/Onboarding.svelte:15` | kicker « Discovery » |
| `apps/desktop/ui-v2/src/ModaleMigration.svelte:74` | kicker « Discovery » |
| `apps/desktop/ui-v2/src/lib/catalogue.fr.js:59-60` | `avis.crash`, `avis.telemetrie` (« Discovery a rencontré… », « Aider à améliorer Discovery ? ») |
| `apps/desktop/ui-v2/src/lib/catalogue.en.js:57-58` | mêmes clés, version anglaise |
| `crates/mail-auth/src/flow.rs:209` | page de fin d'OAuth « revenez à Discovery » |
| `docs/design/systeme.dc.html:26, 439, 506, 1104` | en-tête du Système + écrans 01/02/Migration (DC-D2 : même commit que l'UI) |

L'UI v1 (`apps/desktop/ui/` — titre, `<h1>`, texte télémétrie
`app.js:319`) est **morte** : `frontendDist` pointe `ui-v2/dist`
depuis l'ADR 0015. On ne la renomme pas — sa suppression est une
dette séparée, hors plan.

### B. Le logo (n'existe pas encore dans le dépôt)

- `apps/desktop/icons/icon.ico` : **placeholder 32×32 une seule
  taille** — à régénérer depuis la tuile (16/32/48/256 ; vérifier la
  lisibilité de la pastille à 16 px, la retirer à cette taille si
  illisible).
- Aucun asset de marque : créer `assets/marque/` (enveloppe seule,
  tuile complète, déclinaisons).
- La barre du haut ne porte que le mot ; l'écran 02 du Système fixe la
  marque à 212 px — si la mini-tuile s'y ajoute, l'écran s'amende dans
  le même commit.

### C. Les identifiants porteurs de données (renommer = migrer, jamais en silence)

| Identifiant | Où il vit | Ce qu'il porte |
|---|---|---|
| `dev.discovery.app` | `tauri.conf.json:5` | `%APPDATA%\dev.discovery.app\` — la base (715 Mo au terrain, ADR 0013) et ses compagnons `-wal`/`-shm` ; le profil WebView2 (donc le localStorage) ; matché par `e2e/mesure-ram.ps1:2,22` |
| `discovery.db` | `commands.rs:3035` | le fichier de la base |
| service keyring `discovery-mail` | `mail-auth/src/lib.rs:24` | **les refresh tokens** de tous les comptes ; le test `lib.rs:334` épingle ce nom exprès — il existe pour forcer cette conversation |
| `discovery-theme`, `discovery-theme-auto` | `theme.js:9-10` | le thème choisi ; asserté par `e2e/tests/refonte-ecran02.spec.js:424,486,491` |

Renommer l'un d'eux sans migration = base orpheline, comptes à
reconnecter, thème perdu. Chaque renommage embarque son code de
migration dans le même commit.

### D. L'outillage et la chaîne de release (mécanique, mais large)

- **Crate/binaire** `discovery-desktop` (`apps/desktop/Cargo.toml:2`,
  `Cargo.lock`) → l'exe `discovery-desktop.exe` que lancent
  `e2e/launch.mjs:119`, `mesure.mjs`, `mesure-v2.mjs`, `diag-v2.mjs`,
  `rebuild-v2.mjs` (build + `Get-Process`), `mesure-ram.ps1` ; la
  fixture `telemetry.rs:295` (`discovery_desktop::commands::…`) suit
  le nom du crate.
- **Variables du harnais** : `DISCOVERY_DB_PATH`,
  `DISCOVERY_E2E_ACCOUNT` (`commands.rs`, `telemetry.rs`, tout `e2e/`,
  `spikes/terrain-r0/extraire.mjs`), `DISCOVERY_ACCOUNT` (exemples
  `sync_gmail.rs`, `send_gmail.rs`).
- **Paquets** : `discovery-ui-v2`, `discovery-e2e`
  (`package.json` + locks).
- **Release** : `tauri.conf.json` `productName`/`publisher` (nomme
  l'installeur NSIS, le dossier d'installation, le raccourci),
  endpoint updater `github.com/smonchamps/discovery` (`:43`),
  `scripts/faire-release.ps1` (`$repo`, `$exe`, `url`).
- **Noms internes sans enjeu** (renommage sec) : répertoires temp
  `discovery-pj-*` (`commands.rs:3603`), bases de test
  `discovery-test-*` (`store.rs`, `outbox.rs`), service keyring du
  spike `discovery-spike-oauth`.
- **Dépôt GitHub** `smonchamps/discovery` : renommage **optionnel et
  dernier** — GitHub redirige git, web et assets de release, l'updater
  des postes installés continue de résoudre.

### E. La documentation

- **Vivante — à renommer** : `docs/PASSATION.md` (10 occ., dont les
  commandes et les chemins), `docs/PLAN.md:127` (l'arbre),
  `e2e/README.md`, `docs/design/systeme.dc.html`, et
  `docs/PLAN-SYNCHRO.md` (encore ouvert : terrain E4 dû).
- **Archives datées — on ne réécrit pas l'histoire** : ADR
  0010/0011/0013/0014/0015, plans soldés (BROUILLONS, PIECES-JOINTES,
  DC, UI-V2, REGLAGES, LANGUES, PHASE1/2), `spikes/*`, le journal des
  amendements du Système. Elles disent « Discovery » comme un fait
  daté ; le présent plan documente la bascule.

## 2. Les décisions (W-D1 à W-D6)

Confirmées par le CE le 2026-08-14 : W-D3 (l'accent ne bouge pas),
W-D5 (`dev.elements.wind`), et le renommage du dépôt GitHub se fait
**en dernier** — étape E5 à part entière.

- **W-D1 — par couches, jamais en silence.** Le nom visible se
  renomme sec (couche A) ; un identifiant qui porte des données
  (couche C) ne se renomme **qu'accompagné de sa migration dans le
  même commit**. Aucun renommage « en passant ».
- **W-D2 — les archives restent.** ADR, plans soldés, spikes et
  journal gardent « Discovery » : ce sont des faits datés. Critère de
  fin : `git grep -i discovery` ne rend plus que ces archives.
- **W-D3 — l'accent ne bouge pas.** La page de la suite l'atteste :
  Elements partage l'accent Clarity `#2f6e5b` (ses propres liens le
  portent) ; `#365a4f` est **la couleur de l'élément Wind**, réservée
  à la marque. `#e2ebe8`/`#365a4f` sont des couleurs **figées, hors
  thèmes, hors table des jetons** — inline dans les assets, donc
  invisibles de `coherence-systeme.mjs` et sans paire nouvelle pour
  `contraste.mjs` (le blanc sur `#365a4f` de la pastille tient à
  ~7:1). L'alternative — rebaser l'accent sur `#365a4f` — coûterait la
  table des jetons × 7 thèmes et une repasse complète du banc de
  contraste ; personne ne l'a demandée.
- **W-D4 — la casse.** « Wind » à l'écran (fenêtre, marque, kickers,
  avis) ; `wind` minuscule dans les identifiants (`wind-desktop`,
  `wind.db`, `wind-mail`, `wind-theme`, `WIND_*`), comme `discovery`
  aujourd'hui.
- **W-D5 — l'identifiant applicatif.** `dev.elements.wind` (la suite
  comme espace de noms) — confirmé le 2026-08-14. C'est le nom du
  dossier de données pour les années à venir.
- **W-D6 — la marque entre au Système.** Le commit qui introduit le
  logo ajoute une section « Marque » à `systeme.dc.html` (l'élément,
  la tuile, la pastille, les deux couleurs figées et leur statut
  hors-thèmes, les usages : icône d'application, barre, onboarding) —
  DC-D2 s'applique comme à tout pixel livré.

## 3. Les étapes

### E1 — Le nom (couche A, sans risque)

Tous les textes visibles : marque de la barre, deux kickers, quatre
chaînes de catalogue (fr **et** en), page OAuth, `<title>`, titre de
fenêtre. Le Système s'amende dans le même commit (en-tête, écrans 01,
02, Migration). Gates : suite e2e, `contraste.mjs`,
`coherence-systeme.mjs` — rien ne doit bouger côté jetons.

### E2 — La marque (couche B + W-D6)

Créer `assets/marque/` (SVG de l'enveloppe, de la tuile complète) ;
régénérer `icons/icon.ico` en multi-tailles ; poser la mini-tuile dans
la barre du haut à côté du mot. Section « Marque » au Système + écran
02 amendé (géométrie de la barre) dans le même commit. Vérification au
navigateur : barre, onboarding, modale, icône de fenêtre et de barre
des tâches.

**Livrée le 2026-08-14** : `assets/marque/` (tuile complète 70×70,
glyphe seul — le « W » tracé en traits ronds, pas en fonte, pour un
rendu identique partout), `scripts/faire-icone.ps1` (GDI+, mêmes
formes que le SVG), `icon.ico` en quatre tailles — 256/48 avec
pastille, 32/16 enveloppe seule (en dessous de 48 px la pastille est
illisible ; l'enveloppe s'élargit à 16 et le trait garde un plancher).
Mini-tuile de 24 (rayon 6) posée dans la barre ; kickers laissés en
texte seul. Système : section « Marque » (construction + déclinaisons)
entre Principes et Couleurs, écran 02 amendé, A22 au journal. Vérifié
au navigateur (section, écran 02) et sur les PNG de contrôle des
quatre tailles ; gates coherence + contraste vertes, build ui-v2
propre. Reste un constat d'un lancement : l'icône de fenêtre et de
barre des tâches (le .ico embarqué se vérifie au prochain build).

### E3 — Les données suivent (couche C, terrain obligatoire)

Un commit par identifiant, chacun avec sa migration :

1. `identifier` → valeur W-D5 + **déménagement au démarrage** : si le
   nouveau dossier n'existe pas et que `dev.discovery.app` existe, le
   renommer (même volume, un seul rename, compagnons WAL inclus,
   **avant** toute ouverture de la base). Le profil WebView2 suit le
   même chemin. `mesure-ram.ps1` suit.
2. `discovery.db` → `wind.db` dans la même passe de déménagement.
3. Keyring `wind-mail` : lecture avec repli sur `discovery-mail` et
   recopie à la première utilisation — personne ne reconnecte un
   compte. Le test qui épingle le nom du service est amendé dans le
   même commit : il épingle désormais `wind-mail` **et** l'existence
   du repli.
4. `wind-theme`/`wind-theme-auto` avec recopie depuis les anciennes
   clés dans `theme.js` ; `refonte-ecran02.spec.js` suit.

**Terrain CE** : lancer sur le poste réel (base 715 Mo) — comptes
toujours connectés, messages intacts, thème conservé, RAM mesurée.

**Terrain CE joué le 2026-08-14 — VERT, E3 soldée** : dossier
`dev.elements.wind\wind.db` en place (l'ancien disparu), comptes
connectés sans reconsentement, thème conservé, messages intacts.

**Code livré le 2026-08-14 (commits 9196f92, 32241a8, 6e298ee).**
`apps/desktop/src/demenagement.rs` : le déménagement passe avant tout
dans `main()` (échec = arrêt net — jamais d'application vide devant des
données à un rename de là) ; un `rename` par dossier (`%APPDATA%` et
`%LOCALAPPDATA%`, atomique, aucun octet copié), puis `discovery.db` →
`wind.db` compagnons d'abord et fichier maître en dernier (le `.db`
encore ancien est le marqueur de reprise) ; jamais d'écrasement d'un
état Wind existant ; court-circuit `DISCOVERY_DB_PATH` (harnais).
Quatre tests (poste complet, jamais-écrasé, reprise interrompue, poste
neuf). Coffre : `coffre_lire` (repli + recopie + retrait, le geste de
la migration Phase 2), `coffre_oublier` (purge les deux services,
répétable) ; l'épingle des noms garde les DEUX services ; pont vérifié
en réel contre le Credential Manager du poste de dev (test `#[ignore]`,
`cargo test -p mail-auth -- --ignored`). Thème : recopie
`discovery-theme(-auto)` → `wind-theme(-auto)` au chargement du module,
anciennes clés retirées. Gates : 407 tests Rust, 80 e2e (la suite
reconstruit et relance l'application — le court-circuit du déménagement
est exercé de fait), build ui-v2 propre.

### E4 — L'outillage et la release de bascule (couche D)

Crate `wind-desktop` (exe, fixture télémétrie, scripts e2e, mesure,
rebuild), `WIND_*` (code + harnais + `PASSATION.md`), paquets npm,
noms temp/test, `productName`/`publisher`, `faire-release.ps1`, docs
vivantes (couche E). Puis **la release de bascule** (v0.1.3
« Wind ») : l'endpoint updater actuel la sert aux postes
« discovery ». **Risque à vérifier au terrain avant de publier** : le
changement de `productName` change le nom d'installeur, le dossier
d'installation et le raccourci — rejouer la mise à jour
ancien-poste → Wind sur une machine d'essai et constater ce que NSIS
laisse derrière (entrée de désinstallation « discovery » ?) ; décider
alors nettoyage automatique ou note de release.

**Outillage livré le 2026-08-14** : crate `wind-desktop` (exe, fixture
télémétrie, scripts e2e/mesure/rebuild, Cargo.lock), `WIND_DB_PATH` /
`WIND_E2E_ACCOUNT` / `WIND_ACCOUNT` (code, harnais, docs vivantes),
paquets `wind-ui-v2` / `wind-e2e`, noms temp/test `wind-*`,
`productName` « Wind » (menu Démarrer, artefact
`Wind_<version>_x64-setup.exe`), `publisher` « Elements »,
`faire-release.ps1` aligné. Gates : 407 Rust + 80 e2e (suite jouée
avec `wind-desktop.exe`). Critère W-D2 tenu : hors archives ne
restent que le pont (déménagement, coffre, thème), l'endpoint et
`$repo` (E5), l'arbre du dépôt (E5) et l'UI v1 morte (dette).
**Reste d'E4 : la release de bascule elle-même** — bump de version,
`cargo tauri build` signé, rejouer la mise à jour ancien-poste,
publier.

### E5 — Le dépôt (en dernier, décision CE du 2026-08-14)

Renommer `smonchamps/discovery` → `smonchamps/wind` sur GitHub une
fois la release de bascule publiée et constatée saine. GitHub redirige
git, web et assets de release — les postes installés continuent de
résoudre l'ancien endpoint. Dans le même commit : l'endpoint updater
(`tauri.conf.json`) et `$repo` (`faire-release.ps1`) passent à la
nouvelle adresse. Le dossier local (`Repositories/discovery`) suit au
choix du CE.

## 4. Les gardes

- Ne pas committer pendant la suite e2e (échange de conf du banc).
- e2e flaky en local : la CI fait foi.
- À chaque étape : `cargo test` (403), suite e2e (79),
  `contraste.mjs`, `coherence-systeme.mjs`.
- Critère de clôture (W-D2) : `git grep -i discovery` ne rend plus
  que les archives datées.
