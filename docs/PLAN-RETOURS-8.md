# PLAN-RETOURS-8 — repères de comptes, parcours de premier démarrage, release bi-arch

> Chantier ouvert le 2026-08-22 (`/chantier`), sur trois retours CE :
> (1) feature — un repère **icône + couleur** par compte pour
> différencier les boîtes en mode « toutes les boîtes » : choix dans
> Réglages > Comptes, affiché à la place de l'icône actuelle dans le
> volet nav, et sous le rond à initiales de chaque message ; (2)
> feature — refonte du **parcours de premier démarrage** en quatre
> étapes : ajouter un ou plusieurs comptes → choisir le nombre de
> volets (aperçu visuel) → choisir un thème (aperçu visuel) → écran de
> fin avec bouton Terminer vers la fenêtre standard ; (3) processus —
> à partir de cette release, livrer **x64 ET arm64** à chaque release,
> scripts et processus mis à jour.
>
> **CHANTIER SOLDÉ le 2026-08-22 — terrain complet.** Ouvert le
> 2026-08-22, GO CE du plan le même jour (STOP 1, D1-D8), implémenté
> en TDD (RED montré sur le cœur), commit `cbf795a` (feat, 37
> fichiers, **CI verte run 32576771340**). La release **0.6.0** (D8,
> MINEURE — la première bi-arch) suit le solde : entrée CHANGELOG
> écrite, `faire-release.ps1 0.6.0` à la main du CE (mot de passe de
> la clé, deux saisies), vérification `verifier-release.ps1 0.6.0`,
> install x64 sur le second poste (D5).
>
> **Terrain VALIDÉ le 2026-08-22, en CINQ passes le même jour** (1re :
> R1 validé entièrement + 8 constats R2 ; 2e : tout validé + 4
> constats ; 3e : 4 constats ; 4e : 1 constat ; 5e : « Tout est
> ok. ») — chaque constat corrigé dans la session, gate complète
> rejouée verte à chaque passe. GO CE du plan le 2026-08-22 (STOP 1,
> D1-D8). Revue à regard neuf 8 angles : 10 trouvailles confirmées,
> toutes corrigées. Chiffres : tests Rust +3 (mail-core 357 → 358,
> wind-desktop 18 → 20), e2e 108 → **117**, contraste 2 716 → **3 052**
> paires, glyphes 64 → **76** (`?v=76`, preuve 77/77). Journal Système
> **A74-A75**, **ADR 0023**. Reste : CI verte, `/solde`, puis release
> **0.6.0** (D8) — la première bi-arch.

---

## Constat — faits vérifiés sur pièces (2026-08-22)

### 1. Repère icône + couleur par compte (R1)

- **L'icône actuelle d'un compte est `person`, en dur, identique pour
  tous** (`Nav.svelte:41-46`, décision W2-D7). Deux rendus par boîte :
  `.tuile` (boîte en cours, `--tuile`/`--tuileInk`) et `.rang` (icône
  sur `--muted`). Le glyphe est une **ligature de police** (woff2
  sous-ensemblé Material Symbols, 64 glyphes, `?v=64`), pas un SVG.
- Le mode « toutes les boîtes » est **la boîte unifiée** (`compte ===
  null`, `App.svelte:59`) ; la nav est servie par `nav_snapshot`
  (`commands.rs:1673`) qui ne porte **aucun champ de présentation**.
- **Chaque ligne de liste porte déjà `account_id` et `account_email`**
  (`MessageRow`, `commands.rs:71-114`) — servies par `list_category`,
  `search_messages`, `pinned_rows`. **Aucun changement de tranche
  backend n'est nécessaire pour la liste** : le badge se calcule côté
  UI depuis `ligne.account_id` + une table de préférences chargée une
  fois.
- L'avatar de la liste (`Liste.svelte:700`, CSS L907-912) est un rond
  28 px qui **enjambe les 3 rangs** de la grille de ligne. Il n'y a
  aujourd'hui **aucune place « sous le rond »** — le badge sera un 4e
  élément de colonne 1 (aligné au rang des puces) ou une pastille en
  chevauchement du rond.
- **Stockage : le précédent exact est la signature par compte** —
  table `prefs` clé/valeur, clés suffixées (`signature.{account_id}`,
  `commands.rs:3479-3523`). Clés livrées : `repere_icone.{account_id}`
  + `repere_teinte.{account_id}` (écrites en UNE transaction,
  `set_text_prefs`), commandes `reperes_get`/`repere_set` en
  `hors_pompe`, **aucune migration de schéma** ; les prefs suffixées
  meurent avec le compte (`delete_account`, revue — l'id SQLite se
  réutilise).
- **Deux tensions normatives, les vrais points de décision :**
  - **Couleur** : « toute couleur passe par un jeton » (W2-D1) et la
    gate `e2e/contraste.mjs` mesure texte ≥ 4,5:1 / composants ≥ 3:1
    sur les **28 thèmes**. Une couleur libre (roue chromatique) est
    hors du système de jetons et hors gate. → D1.
  - **Icône** : l'invariant A3 dit « une icône, un sens dans tout le
    produit ». Un choix libre parmi les glyphes existants briserait
    A3 ; la sortie propre est un **jeu dédié aux comptes**, réservé,
    jamais réemployé ailleurs. → D2.
- Ajouter des glyphes est une procédure rodée (58 → 61 → 64 aux deux
  chantiers précédents) : régénérer le woff2, recopier dans
  `public/icones/`, `?v=N` (= nombre de glyphes), inventaire
  `assets/icones/README.md`, preuve `apercu.html`.
- Défaut d'un compte sans préférence : `person` + rendu actuel — la
  mise à jour est invisible pour l'existant.

### 2. Parcours de premier démarrage (R2)

- **L'écran actuel (`Onboarding.svelte`, 36 lignes) est un cul-de-sac
  pour un parcours en étapes** : aucun état, aucune progression,
  affiché par la condition dérivée `comptes.length === 0`
  (`App.svelte:1393`) — il s'évanouirait dès le premier compte ajouté,
  avant les étapes 2-4.
- **Le patron architectural existe : `ModaleMigration.svelte`** (ADR
  0012) — seule surface exclusive et bloquante au démarrage : méthode
  `export async function assurer()` attendue par l'`onMount` de l'App,
  `visible` interne, boucle d'étapes par promesse résolue au clic.
  Ordre au boot à préserver (A41) : migration → pose de la langue →
  accueil ; rien ne touche la base avant `assurer()`.
- **Il faut une clé de persistance neuve** (`wind-accueil-fait`,
  localStorage — V-D4 : préférence pure UI, comme `wind-theme`,
  `wind-volets`). Sémantique pour l'existant : des comptes présents au
  boot ⇒ accueil réputé fait (aucun parcours ne se rejoue à la mise à
  jour 0.5.0 → 0.6.0).
- **Étapes 2 et 3 sans aucun coût backend** : `appliquerVolets(n)` et
  `appliquerTheme(id)` appliquent ET persistent immédiatement ; les
  aperçus se construisent sans IPC — pastilles depuis `FICHES`
  (`lib/theme.js:38-67`, 28 fiches), géométrie des volets depuis les
  défauts de `largeurs.svelte.js`. La gate `coherence-systeme.mjs`
  interdit tout hex recopié : les aperçus consomment les jetons.
- L'ajout de compte reste `GuichetCompte.svelte` (une implémentation,
  deux surfaces, A11) — l'étape 1 l'embarque tel quel.
- **Deux énoncés normatifs du Système sont contredits frontalement**
  et doivent être amendés (pas contournés), dans le même commit
  (DC-D2) : « l'accueil qui ne réclame qu'une adresse » (Principes) et
  « la modale de migration est seule de son espèce […] rien d'autre ne
  bloque jamais » (Avis et progression).
- e2e : `refonte-onboarding.spec.js` (6 tests, `vierge: true`) couvre
  l'écran 01 actuel — à faire évoluer, pas à jeter (le contrat IPC
  `add_generic_account` et la porte par domaine restent). Dette D-10
  (ordre d'`onMount` non asserté) se rouvre mécaniquement : ce
  chantier touchera `onMount` — l'occasion de la solder par un e2e.

### 3. Release bi-arch x64 + arm64 (R3)

- **Fait central : Wind ne livre aujourd'hui QUE arm64.** Le canal x64
  a été retiré en 0.1.3 (PLAN-WIND E4) — le seul poste utilisateur est
  ARM64 (Snapdragon X, `rustup` host `aarch64-pc-windows-msvc`). Le
  retour est un **retour du canal x64**, déjà annoncé « chantier à
  part » (`installer-poste.ps1:236`).
- `faire-release.ps1` câble l'architecture en dur : `cargo tauri
  build` sans `--target` (= hôte arm64), chemin
  `target/release/bundle/nsis`, un seul exe
  `Wind_<v>_arm64-setup.exe`, `latest.json` à une clé
  `windows-aarch64`, `gh release create` à 3 assets.
- **Un seul `latest.json` sert les deux canaux** : l'updater Tauri lit
  la clé `{os}-{arch}` de SON binaire et ignore le reste — il suffit
  d'ajouter `windows-x86_64` (signature + url propres). Aucune
  bascule d'endpoint. **Rien à modifier côté Rust** : le chantier est
  entièrement outillage + documentation.
- La version est **globale** au manifeste : les deux architectures
  sortent à la même version, ou pas du tout. → D7.
- **Cross-compilation x64 depuis ARM64 : profil de risque bas** —
  sqlite bundlé (C pur via `cc`), `ring` 0.17 (objets pré-générés
  msvc, à prouver par un build), `native-tls` → schannel (API
  système), pas d'OpenSSL sur le chemin Windows. La CI `quality`
  tourne déjà sur `windows-latest` **x64** : compile + tests verts en
  continu. Ce qu'elle ne prouve pas : le **lien** de `wind-desktop` et
  le **bundle** NSIS x64. Manque sur le poste : la cible rustup
  `x86_64-pc-windows-msvc` et (à vérifier) le composant VS « MSVC v143
  C++ x64/x86 build tools ». Le patron du miroir existe déjà :
  `installer-poste.ps1` fait exactement l'inverse (x64 → arm64) avec
  option de preuve `-CrossArm64Check`.
- **Piège documenté qui se rejouera** : `.cargo/config.toml` scope
  `linker = "lld-link"` au SEUL triple aarch64 ; en Git Bash
  (pre-push), le triple x64 nu retomberait sur le mauvais `link.exe`
  (`/usr/bin/link.exe`). Remède symétrique évident : le même override
  pour `x86_64-pc-windows-msvc`.
- **Piège nouveau, silencieux** : une clé de plateforme manquante ou
  des signatures **croisées** dans `latest.json` ne produisent aucune
  erreur — l'updater conclut « pas de mise à jour ». Même famille que
  BOM et tag `v` (ADR 0013) : à encoder dans le script, jamais laissé
  à la vigilance.
- **Le point dur méthodologique : la preuve terrain du canal x64.**
  §2.10 : « la preuve définitive est l'auto-update n-1 → n constaté au
  terrain ; ne jamais forger un PASS ». Sur un poste ARM64, une app
  x64 tourne en émulation — ce qui ne prouve pas un vrai poste x64
  (précisément le motif du retrait en 0.1.3). Et il n'existe **aucun
  n-1 x64** : le premier auto-update x64 ne sera constatable qu'à la
  release SUIVANTE. → D5.
- Normatif à amender : STANDARD §2.9 (critère MAJEUR à évaluer **par
  canal**), §2.10 (« trois assets » → cinq, contrôles dédoublés par
  plateforme + garde anti-croisement), §10 (carte des fichiers déjà
  périmée sur `faire-release.ps1`), ADR 0013 (« trois assets »,
  « publication manuelle » — périmés), `README.md` (« arm64 natif »,
  « 0.1.7 »), `installer-poste.ps1:236`.
- **Il n'existe aucun script de vérification §2.10** — tout est
  manuel. Avec 5 assets et 2 plateformes, les contrôles doublent :
  un `scripts/verifier-release.ps1` est le candidat naturel (« la
  friction est encodée une fois, plus jamais repayée », ADR 0013).

## Périmètre — et refus explicites

**On fait** : R1 (prefs par compte + Réglages > Comptes + nav +
badge de liste), R2 (parcours 4 étapes au patron ModaleMigration,
aperçus volets/thèmes sans IPC), R3 (build bi-arch local,
`faire-release.ps1` à 5 assets, `latest.json` à 2 clés,
`verifier-release.ps1`, normatif amendé).

**On ne fait pas** :
- **Couleur libre** (roue chromatique) — hors système de jetons, hors
  gate de contraste (sous réserve D1).
- **Choix d'icône parmi les glyphes existants** — briserait A3 (sous
  réserve D2).
- **Repère dans le fil de lecture** (avatars de `Fil.svelte`) — le
  retour vise la nav et la liste ; le fil est ouvert depuis une ligne
  déjà identifiée. À rouvrir si le terrain le demande.
- **Synchro serveur du repère** — préférence locale (`prefs`), comme
  la signature.
- **Rejouer le parcours à la demande** (bouton « revoir l'accueil ») —
  les étapes 2 et 3 existent déjà dans Réglages.
- **Étape signature / import de réglages dans l'accueil** — hors
  retour.
- **Release buildée en CI** — la clé de signature reste locale
  (`C:\Keys\wind.key`, zéro secret hors poste), sous réserve D6.
- **Signature de code Windows (Authenticode)** — reportée à la bêta
  (ADR 0013), inchangé.

## Options et verdicts

### O1 — la couleur du repère (R1)

- **O1a — nuancier fixe dédié, mesuré** : teintes choisies pour tenir
  ≥ 3:1 (composant) sur les fonds où le repère se pose (`panel`, `bg`,
  `sel`, `hover`, `surface`) à travers les 28 thèmes, ajoutées à la
  gate `contraste.mjs`. Verdict : **conforme au contrat des jetons,
  gate automatique.** **Fait mesuré (2026-08-22, banc jetable sur les
  jetons réels de `systeme.css`)** : aucune teinte UNIQUE ne peut
  tenir 3:1 à la fois sur les fonds clairs (pire : `nature/sel`,
  L=0,69) et sur les fonds des thèmes nuit (pire : `iris-nuit/sel`,
  L=0,05) — l'écart exige L ≤ 0,196 ET L ≥ 0,25, contradiction. La
  forme retenue : **12 familles × 2 déclinaisons** (sombre servie aux
  14 thèmes clairs, claire aux 14 `-nuit`, bascule par
  `[data-theme$="-nuit"]` — la logique même des jetons), glyphe blanc
  sur la sombre, encre `#1c1b1b` sur la claire. Mesure : les 24 hex
  passent tous ≥ 4,3:1 au pire fond de leur famille, glyphe ≥ 6:1.
- **O1b — couleur libre** : liberté totale, mais aucun contraste
  garanti (une teinte claire sur thème clair rend le repère
  invisible), hors gate. Verdict : **écartée** (D1).

### O2 — le badge sous le rond (R1)

- **O2a — pastille dédiée sous l'avatar** (colonne 1, sous le rond,
  alignée au rang des puces) : petit rond ~14-16 px au fond de la
  teinte du compte portant le glyphe choisi. Lisible, ne touche pas
  l'avatar. Verdict : **retenue** — c'est la lettre du retour (« en
  dessous du rond »).
- **O2b — badge en chevauchement du rond** (coin bas-droit, position
  absolue) : compact mais rogne l'initiale et complique le contraste
  (bord nécessaire). Verdict : écartée.

### O3 — où bâtir le canal x64 (R3)

- **O3a — cross-build local sur le poste ARM64** : `cargo tauri build
  --target x86_64-pc-windows-msvc` ×1 en plus du build natif ; la clé
  ne quitte jamais le poste ; temps de release ×2 (~8 min de build).
  Verdict : **recommandée** — zéro secret exporté, un seul processus.
- **O3b — build x64 en CI GitHub** : runner x64 natif, mais la clé de
  signature devient un secret GitHub Actions et la release un
  processus en deux lieux. Verdict : écartée sauf décision CE
  contraire (à rouvrir si le cross-build local échoue à l'E1).

## Étapes

- **E1 — la preuve du cross-build x64** (spike outillé, avant tout
  code) : `rustup target add x86_64-pc-windows-msvc`, composant VS
  vérifié/posé, override linker symétrique dans `.cargo/config.toml`,
  puis `cargo tauri build --target x86_64-pc-windows-msvc` jusqu'au
  bundle NSIS. Gate : l'exe x64 + son `.sig` existent, noms d'assets
  constatés (pas supposés). Un échec ici rouvre O3b avant d'écrire la
  moindre ligne de script.
  **✓ Faite (2026-08-22)** : cible rustup posée, toolset MSVC 14.50 du
  poste porte déjà les libs x64 (aucun composant VS à installer),
  override `lld-link` ajouté pour `x86_64-pc-windows-msvc` ; `cargo
  tauri build --target x86_64-pc-windows-msvc` **lie et bundle en
  1 min 45 s** → `target/x86_64-pc-windows-msvc/release/bundle/nsis/
  Wind_0.5.0_x64-setup.exe` (nom constaté). Seule marche restante à la
  release : la signature (clé + mot de passe CE) — l'erreur « no
  private key » est attendue hors release. O3b fermée.
- **E2 — R1 cœur** : commandes `reperes_get`/`repere_set` (gabarit
  signature, `hors_pompe`, TDD sur le cœur prefs), nuancier + jeu
  d'icônes dédiés (glyphes neufs : woff2 régénéré, `?v=N`, inventaire,
  preuve apercu). Gate : tests Rust + contraste vert sur les teintes
  ajoutées.
  **✓ Faite (2026-08-22)** : RED montré (fonctions absentes) puis
  GREEN ; 12 glyphes ajoutés (64 → 76, `?v=76`, preuve apercu
  **77/77** rejouée sous CSP, piège appris : le subsetteur exige
  `icon_names` TRIÉ) ; nuancier 12×2 en CSS + gate contraste étendue
  (section REPERES, hex et encres LUS du CSS expédié, fonds panel/bg/
  sel/hover/surface/**tuile** — 3 052 paires).
- **E3 — R1 UI** : sélecteur icône + teinte dans Réglages > Comptes ;
  nav (`Nav.svelte` : glyphe + teinte à la place de `person`) ; badge
  de liste (`Liste.svelte`, pastille O2a, portée selon D3). Système
  amendé (section Boîte de réception, Ligne de message, Réglages,
  journal A-n) dans le même commit. e2e : réglage → nav → liste.
  **✓ Faite (2026-08-22)** : A74 au journal ; badge aussi en
  **recherche** (toujours multi-comptes — revue) et DIT aux lecteurs
  d'écran (aria-label = adresse) ; e2e pose/badge D3/retrait verts.
- **E4 — R2 parcours** : `Accueil.svelte` au patron ModaleMigration
  (4 étapes, `assurer()` après migration et langue), étape 1 =
  GuichetCompte (multi-comptes), étapes 2/3 = aperçus visuels
  (jetons/`FICHES`, zéro hex), étape 4 = Terminer ; clé
  `wind-accueil-fait`, existant réputé fait. Système amendé (Écran 1
  réécrit + les deux énoncés normatifs). e2e : parcours complet sur
  base vierge (D-10 soldée au passage), non-réapparition à la mise à
  jour.
  **✓ Faite (2026-08-22)**, deux écarts au plan assumés : (a)
  l'affichage reste dérivé (`Onboarding.svelte` refondu, pas une
  modale impérative — le patron `assurer()` n'était pas nécessaire,
  l'ordre A41 tient par construction dans `chargerNav`) ; (b) D-10
  reste OUVERTE (l'assertion `prefs.lang` n'est pas écrite — pas de
  PASS forgé), la dette est annotée. Ajouts de revue : marque
  `wind-accueil-commence` (un parcours abandonné REPREND, jamais
  réputé accueilli), couture `__e2eAccueil` dans `lib/accueil.js`
  (jamais dans la décision produit ; rien ne s'écrit sous elle) ; e2e :
  parcours complet + reprise (vrai chemin, sans couture) + guichet
  seul + « installation existante réputée accueillie ».
- **E5 — R3 outillage de release** : `faire-release.ps1` bi-arch (2
  builds `--target`, chemins `target/<triple>/…`, 2 paires exe/sig,
  manifeste 2 clés, garde anti-croisement des signatures encodée,
  publication 5 assets tout-ou-rien selon D7) ;
  `scripts/verifier-release.ps1` scriptant §2.10 ×2 plateformes.
  **✓ Faite (2026-08-22)**, un écart au plan (revue) : le mot de passe
  de la clé reste demandé par Tauri **à chaque build** (deux saisies)
  — le poser en variable d'environnement l'aurait exposé à tous les
  processus enfants des builds, l'invariant ADR 0013 prime sur le
  confort. Assets dérivés de `$cibles` (jamais d'indexation en dur),
  BOM UTF-8 restauré (piège PS 5.1 payé et gravé), vérificateur
  prouvé sur la 0.5.0 (arm64 PASS, x64 ECHEC attendu — mono-arch),
  contrôles au TAG de la version (jamais Latest), échec en verdict.
- **E6 — normatif** : STANDARD §2.9/§2.10/§10, ADR court « retour du
  canal x64 » (ou amendement 0013), README, `installer-poste.ps1`,
  ETAT.
  **✓ Faite (2026-08-22)** : STANDARD §2.9 (MAJEUR par canal), §2.10
  (cinq assets nommés, deux clés, garde anti-croisement,
  `verifier-release.ps1`), §10 (carte corrigée — elle était déjà
  périmée) ; **ADR 0023** ; ADR 0013 annoté (mentions d'époque) ;
  README ; `installer-poste.ps1` retourné ; gate de cohérence étendue
  (contrôle 7 : le jeu dédié UNE liste sur quatre porteurs — Rust,
  reperes.js, systeme.css, catalogues — prouvé mordant par test
  négatif) ; ETAT au solde.
- **E7 — qualité** : revue à regard neuf sur le diff complet, gate
  complète, puis **STOP 2 terrain** avec checklist et commandes.
  **Revue faite (2026-08-22)** : 8 angles, 10 trouvailles confirmées,
  toutes corrigées (dont : fond `tuile` absent du banc des repères ;
  spécificité CSS qui éteignait le glyphe « Déconnecté » ; parcours
  abandonné jamais repris ; repère survivant au retrait d'un compte —
  id SQLite réutilisé ; paire icône/teinte non atomique ; BOM perdu de
  `faire-release.ps1` ; mot de passe de signature exporté à l'env des
  builds ; vérificateur mourant sans verdict ; badge absent de la
  recherche ; couture e2e dans la décision produit). Gate complète
  verte (508 tests Rust, 116 e2e, 3 052 paires).
  **Terrain, 1re passe (2026-08-22)** : **R1 validé entièrement** ;
  R2 : 8 constats, **corrigés le jour même** — (1) « Bienvenue dans
  Wind » + hitofude, « Étape 1/4 », invite, barre + « Ajouter »
  secondaire ; (2) barre repliée derrière « Ajouter une autre adresse
  email » ; (3) « Retour » + « Ajouter » secondaires sur le guichet
  générique révélé ; (4) titres/sous-textes de l'étape 2 ; (5)
  **captures réelles** de l'app (décor Clarity,
  `e2e/capture-accueil.mjs`, recadrées au-dessus de la barre d'état)
  + sélection en `--sel` + contour épaissi ; (6) vignettes de thème
  dans la disposition choisie ; (7) même sélection à l'étape 3 ; (8)
  étape 4 = récapitulatif en rangées-portes vers leur étape. e2e
  108 → **117** (gate entière rejouée verte après les corrections).
  **Terrain, 2e passe (2026-08-22)** : tout validé + 4 constats,
  **corrigés le jour même** — (1) « Ajouter » PRIMAIRE tant que le
  Continuer de la marche est grisé, secondaire ensuite ; (2) point
  final à l'invite, note « serveur détecté » retirée de l'accueil,
  barre à 40 px (la hauteur de son bouton) ; (3) étape 2 : UNE image
  d'aperçu qui suit le bouton survolé/focalisé et retombe sur le
  choix, trois boutons dessous ; (4) étape 4 : miniatures (capture de
  la disposition, fenêtre du thème), voile « Revenir à cette étape »
  au survol/focus (règles d'A70, glyphe `arrow_back` — rangée ajoutée
  au relevé du Système), phrase « Vérifiez vos choix avant de
  continuer. ».
  **Terrain, 3e passe (2026-08-22)** : 4 constats, **corrigés le jour
  même** — (1) « Continuer » ne s'affiche JAMAIS grisé : absent tant
  qu'aucun compte n'existe ; (2) le guichet générique révélé masque
  « Continuer », son « Ajouter » est toujours primaire ; (3) étape 2 :
  image + boutons centrés dans UNE élévation (surface + ombre) ; (4)
  étape 4 : les trois récaps côte à côte en cartes-colonnes,
  « Thèmes » → « Thème ».
  **Terrain, 4e passe (2026-08-22)** : 1 constat, corrigé — étape 4 :
  le texte AU-DESSUS des miniatures (Disposition et Thème).
  **Terrain, 5e passe (2026-08-22) : « Tout est ok. » — VALIDÉ.**

La release 0.6.0 elle-même (MINEURE, D8) se fait après le solde,
comme d'habitude — ce sera la première release bi-arch.

## § Décisions CE — tranchées le 2026-08-22 (STOP 1)

- **D1 — couleur du repère** : **« Nuancier fixe mesuré »** (O1a) —
  ~8-12 teintes dédiées aux comptes, mesurées ≥ 3:1 sur les fonds
  concernés à travers les 28 thèmes, ajoutées à la gate de contraste.
- **D2 — jeu d'icônes** : **« Jeu dédié ~10 glyphes »** — glyphes
  neufs ajoutés au sous-ensemble woff2, réservés aux comptes, A3
  respecté.
- **D3 — portée du badge de liste** : **« Boîte unifiée seule »** —
  le badge n'apparaît que quand « Toutes les boîtes » est
  sélectionnée, là où identifier le compte a un sens.
- **D4 — étape 1 du parcours** : **« Au moins un compte exigé »** —
  le bouton Continuer de l'étape 1 ne s'active qu'avec ≥ 1 compte
  ajouté.
- **D5 — preuve terrain du canal x64** : **« Second poste x64 »** —
  la validation terrain du canal x64 (install + auto-update) se fera
  sur une vraie machine x64. La checklist terrain du STOP 2 et la
  release 0.6.0 en tiendront compte (le premier auto-update x64 ne
  sera constatable qu'à la release suivante ; l'install 0.6.0 x64,
  elle, se constate dès la 0.6.0).
- **D6 — lieu du build x64** : **« Cross-build local »** (O3a) —
  `--target x86_64-pc-windows-msvc` sur ce poste dans
  `faire-release.ps1`, clé jamais exportée, temps de release ×2.
- **D7 — publication** : **« Tout-ou-rien »** — un build en échec
  bloque toute la release ; le script échoue franchement avant toute
  publication, jamais un canal décalé ni un manifeste partiel.
- **D8 — version** : **« 0.6.0 MINEUR »** — capacités nouvelles,
  auto-update arm64 intact depuis 0.5.0, critère §2.9 respecté.
