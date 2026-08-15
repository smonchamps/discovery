# PLAN-WADA-ELARGI — 28 thèmes « Wada élargi » et sombre automatique par déclinaison (A42)

**CHANTIER SOLDÉ le 2026-08-16 — terrain complet.** GO CE du même jour
sur les quatre décisions (D1-D4, toutes les recommandations, §5) ;
livraison intégrale en un commit (`241cdb2`, A42) après revue à regard
neuf (§6, 9 correctifs) et un constat terrain de passe 1 — le retour au
clair ne suivait pas — instruit aux sondes et corrigé le jour même
(racine : `prefers-color-scheme` mort dans le WebView2 de Tauri, §7).
Passe terrain 2 validée par le CE (« tout ok »), CI verte
(run 31913758538). Preuves : 28 × 25 paires de contraste vertes, 476
valeurs de jetons concordantes, 75/75 e2e dont la bascule Windows
réelle dans les deux sens.

> Instruction CE du 2026-08-16 : appliquer le paquet
> `docs/design/handoff/` (INSTRUCTIONS.md, AMENDEMENT-A42.md,
> systeme.css, fiches-themes.fr.js) — amendement A42, jetons, fiches,
> bascule Sombre automatique. DC-D2, gates avant commit.

## 1. Constat — instruction sur pièces, 2026-08-16

Le paquet livre une table de **17 rôles × 28 thèmes** (14 clairs, 14
déclinaisons `-nuit`) qui remplace les 7 thèmes actuels, et une règle
« Sombre automatique » qui suffixe le thème choisi au lieu de basculer
sur « La nuit ». Quatre faits mesurés sur pièces, dont trois écarts que
le paquet ne connaît pas :

1. **`--ink2:undefined` sur les 28 thèmes du `systeme.css` livré.** Le
   générateur du projet Design a perdu le rôle `ink2` (28 occurrences
   littérales de `undefined`). La promesse « contrastes pré-vérifiés »
   est donc fausse pour les 10 paires `ink2`/fond que mesure
   `contraste.mjs` (`ink2` sur bg, surface, panel, sel, hover — texte
   courant : expéditeurs, corps, libellés nav). Les 27 valeurs manquent
   (celles de `nature` et `nature-nuit` existent au dépôt : `#4a505a`
   et `#b7bebb`).
2. **Les deux gates sont aveugles aux identifiants à trait d'union.**
   `contraste.mjs:21` et `coherence-systeme.mjs:45,59` capturent
   `data-theme="([a-z]+)"` : un bloc `nature-nuit` n'est pas matché du
   tout — ni mesuré, ni comparé. Sans amendement des gates, les 14
   thèmes sombres passeraient en CI **sans aucune vérification**, en
   silence. (Ce n'est pas tordre la gate : c'est l'étendre à des
   identifiants qu'elle n'a jamais eu à voir.)
3. **Le `:root` livré ajoute `--marque-tuile`/`--marque-element`**
   (« exposés ici pour commodité »). La gate de cohérence exige que
   tout jeton du CSS soit dans la table du doc : les garder impose deux
   rôles de plus à la table (19 × 28) et fait entrer la marque dans la
   table des thèmes — ce que W-D3 a précisément gelé hors thèmes.
4. **La forme du paquet n'est pas celle du dépôt.** `fiches-themes.fr.js`
   porte id + nom + desc + pastilles en un seul objet ; au dépôt les
   fiches vivent en deux morceaux : `FICHES` (id + pastilles,
   `lib/theme.js:32`) et les libellés au catalogue
   (`theme.<id>.nom/desc`, `catalogue.fr.js`/`catalogue.en.js`, A15).
   L'intégration est une transposition, pas une copie.

État du code touché : `lib/theme.js` (THEMES, FICHES, `refleter()` qui
pose `nuit` en dur, migration de clés Discovery), `Reglages.svelte`
(sélecteur piloté par FICHES — rien à changer au gabarit), catalogues
fr/en (7 fiches + copie `sombreAutoDesc` qui nomme « La nuit »),
`systeme.dc.html` (prose « Sept thèmes », table 119 cellules → 476),
e2e `refonte-ecran02.spec.js` (compte 7, id `nuit` en dur, D6).

## 2. Périmètre

**Fait** : la table 17 × 28 dans `systeme.css` (ink2 dérivé, § D2), la
mécanique `-nuit` du sombre automatique, la migration `nuit` →
`nature-nuit` des valeurs persistées, les fiches et catalogues fr/en,
l'amendement A42 + table du Système régénérée, l'extension des deux
gates aux identifiants à trait d'union, les e2e à jour. **Un seul
commit** (DC-D2).

**Refus explicites** :
- Pas de re-délibération des décisions gelées du paquet : suffixe
  `-nuit`, `--alert` commun (#9e3a2c / #ea9a90), marque hors thèmes,
  pastilles `[accent, bg, panel, surface, ink]`.
- Pas d'API Tauri pour écouter l'OS : `matchMedia` +
  `addEventListener('change')` existe déjà (`theme.js:82`) et fonctionne
  dans WebView2 — on ne remplace pas ce qui marche.
- Pas de retouche des teintes livrées hors `ink2` : les 16 autres rôles
  s'appliquent verbatim ; si la gate contraste en refuse un, correction
  locale minimale (la règle du paquet lui-même, point 7).
- Pas de suppression du dossier `handoff/` dans ce commit : il n'est pas
  au dépôt (non suivi) ; son sort appartient au CE (esprit DC-D4 : un
  document d'étude ne rentre pas au dépôt).

## 3. Options et verdicts

**ink2 manquant** — deux voies : (a) retour au projet Design pour un
ré-export corrigé ; (b) dérivation locale des 27 valeurs, même grammaire
que la paire existante (`nature` : ink #24272e → ink2 #4a505a, même
teinte, luminosité relevée vers muted ; `nature-nuit` : ink #edefed →
ink2 #b7bebb, abaissée), chaque valeur validée au banc `contraste.mjs`
(4,5:1 sur bg, surface, panel, sel, hover). La voie (b) se mesure ici et
maintenant avec la gate qui fait foi ; la voie (a) coûte un aller-retour
pour 27 nombres que le banc sait juger. Verdict proposé : (b) — § D2.

**Table du doc (476 cellules)** — génération par script jetable
(scratchpad) depuis `systeme.css`, jamais à la main : la gate de
cohérence compare valeur pour valeur, une coquille manuelle est un andon
garanti. Le script ne rentre pas au dépôt.

## 4. Étapes

- **E1 — Les gates voient les tirets.** `contraste.mjs` et
  `coherence-systeme.mjs` : `[a-z]+` → `[a-z-]+`. RED montré : un bloc
  `-nuit` posé dans le CSS n'apparaît dans aucun des deux rapports
  avant le correctif, il y apparaît après. Gate : les deux scripts sur
  l'état 7 thèmes restent verts (aucun faux écart).
- **E2 — La table des jetons.** Remplacer la table de
  `apps/desktop/ui-v2/src/systeme.css` par les 28 thèmes du paquet,
  moins les jetons marque (selon D3), plus les 27 `ink2` dérivés ;
  conserver tout le reste du fichier (police, focus, barres, base).
  Gate : `node e2e/contraste.mjs` vert sur 28 thèmes × 25 paires.
- **E3 — La mécanique de thème.** `lib/theme.js` : THEMES (28), FICHES
  (id + pastilles du paquet), migration des valeurs persistées (D4),
  `refleter()` pose `<thème>-nuit` quand suivi OS actif et OS sombre —
  et laisse tel quel un thème déjà `-nuit` (si D1 = 28). e2e d'abord
  (RED) : compte de fiches, choix `nature-nuit`, D6 réécrit sur le
  suffixe, migration `nuit` → `nature-nuit`.
- **E4 — Catalogues.** `catalogue.fr.js` : 28 (ou 14, D1) paires
  `theme.<id>.nom/desc` du paquet, mot pour mot ; copie
  `sombreAutoDesc` du paquet (INSTRUCTIONS point 5) ; miroir anglais
  dans `catalogue.en.js` (le français est la référence). Gate : e2e
  langues existants verts.
- **E5 — Le Système dit le livré.** `systeme.dc.html` : A42 au journal
  (texte du paquet), prose « Sept thèmes » réécrite (28, grammaire
  nuit, sombre automatique), table du contrat régénérée par script
  (17 × 28). Gate : `node e2e/coherence-systeme.mjs` vert.
- **E6 — Qualité et livraison.** Revue `/code-review high` sur le diff
  complet ; `/gate` ; **STOP 2 terrain** (checklist : sélecteur,
  bascule sombre auto en direct, migration d'un profil portant `nuit`,
  échantillon de thèmes clairs/sombres à l'œil) ; commit unique, push,
  `gh run watch` jusqu'à CI verte ; `/solde`.

## 5. Décisions CE

- **D1 — Le sélecteur montre quoi ?** Le paquet laisse le choix :
  **14 fiches claires** (les sombres ne s'atteignent que par Sombre
  automatique — l'utilisateur perd la possibilité de forcer un thème
  sombre en permanence, qu'il a aujourd'hui avec « La nuit ») ou
  **28 fiches** (les déclinaisons nuit se choisissent aussi à la main ;
  le suivi OS ne suffixe que les thèmes de base et laisse en paix un
  `-nuit` choisi). Recommandation : **28** — ne pas retirer une
  capacité vivante.
- **D2 — `ink2` perdu par le générateur.** Dériver localement les 27
  valeurs (grammaire de la paire `nature`, banc `contraste.mjs` faisant
  foi) ou retourner au projet Design pour un ré-export. Recommandation :
  **dériver localement** — la gate du dépôt est le juge que le paquet
  lui-même désigne.
- **D3 — Jetons marque dans `:root`.** Les retirer de la table livrée
  (la marque reste en dur hors thèmes, W-D3, table à 17 rôles) ou les
  garder et élargir table + doc à 19 rôles. Recommandation :
  **retirer** — W-D3 a gelé la marque hors thèmes.
- **D4 — Migration des valeurs persistées.** `nuit` → `nature-nuit`
  explicite (le choix survit, comme à PLAN-WIND E3) ; les cinq thèmes
  retirés (air, feu, eau, astres, terre) → repli `nature` (le garde-fou
  de `themeActuel()` le fait déjà, silencieusement). Recommandation :
  **oui aux deux** — migration explicite pour `nuit`, repli existant
  pour les cinq retirés.

**Réponses CE du 2026-08-16 (STOP 1)** — les quatre recommandations,
mot pour mot :
- D1 : « 28 fiches (Recommandé) » — clairs et sombres choisissables ;
  le suivi OS laisse en paix un thème `-nuit` choisi.
- D2 : « Dériver localement (Recommandé) » — 27 valeurs calculées ici,
  la gate du dépôt fait foi.
- D3 : « Retirer (Recommandé) » — la marque reste hors thèmes (W-D3),
  table à 17 rôles.
- D4 : « Oui aux deux (Recommandé) » — migration explicite `nuit` →
  `nature-nuit`, repli silencieux `nature` pour les cinq retirés.

## 6. Exécution (2026-08-16)

- **E1** : RED prouvé (la regex `[a-z]+` matchait 14 blocs sur 28 du
  CSS livré, les 14 `-nuit` invisibles) ; correctif dans les deux
  gates, vertes sur l'état 7 thèmes.
- **E2** : les 27 `ink2` dérivés à t=0,72 passent tous (pire paire
  5,31:1). La promesse « pré-vérifié » du paquet est tombée au banc :
  **20 paires sous le seuil**, concentrées sur `muted`/`sel` (la paire
  du remède A35, jamais mesurée par le projet Design) + `accent`/`sel`
  de grenade-nuit (2,90:1). Remède A8 appliqué en génération : `--muted`
  ajusté sur 18 thèmes, `--sel` de grenade-nuit assombri — même teinte,
  luminosité minimale. Gate contraste : « Tout passe » (28 × 25 paires).
- **E3-E5** : mécanique `-nuit` + migration, 28 fiches aux deux
  catalogues, copie sombreAuto réécrite (fr/en + maquette du doc), A42
  au journal, table du contrat régénérée (476 cellules, deux tables
  clairs/sombres). Cohérence : « Tout concorde — 28 thèmes, 476
  valeurs ».
- **Revue à regard neuf** (`/code-review high`, 8 angles) : 10
  constats, 9 corrigés le jour même — dont un vrai rouge e2e (le test
  D6 cliquait la bascule d'un groupe démonté) et la copie sombreAuto
  périmée dans la maquette du Système. Consolidations nées de la
  revue : `e2e/jetons.mjs` (parseur unique des deux gates + plancher
  `NOMBRE_ATTENDU = 28`), vérification n° 4 de la cohérence (pastilles
  `FICHES` ↔ jetons CSS), `THEMES` dérivé de `FICHES` (la liste
  jumelle `BASES` supprimée), garde d'appartenance dans `refleter()`,
  migrations fusionnées en un bloc.
- **Dette** (constat de revue, hors périmètre) : `e2e/mesure-v2.mjs`
  garde 60 itérations et des commentaires « les 7 thèmes » — depuis
  A42 l'échantillon par thème tombe de ~8 à ~2, le chiffre « bascule
  par thème » n'est plus comparable à la ligne de base. À reprendre à
  la prochaine passe de mesure (D-7 s'en souviendra).
- Gate complète du 2026-08-16, tout vert : fmt OK, build ui-v2 zéro
  avertissement, contraste OK (28 × 25 paires), cohérence OK (28
  thèmes, 476 valeurs + pastilles), garde du thread principal OK (62
  commandes), clippy zéro warning, 423 tests Rust + doc-tests, **74/74
  e2e** (1,8 min, local — la CI restera la référence après commit).

## 7. Constat terrain — passe 1 du 2026-08-16 (point 4 KO)

Verdict CE : « 1 OK 2 OK 3 OK 4 KO » — le retour au clair ne suivait
pas. Instruction aux sondes (banc e2e isolé, vraie application) :

1. `Set-ItemProperty` seul ne diffuse pas `WM_SETTINGCHANGE` — aucune
   application n'est prévenue ; les commandes de la passe ne pouvaient
   rien déclencher (le « 3 OK » venait de la fiche sombre choisie au
   point 1, pas du suivi).
2. Sous une VRAIE bascule (registre + diffusion « ImmersiveColorSet ») :
   `matchMedia('(prefers-color-scheme: dark)')` est **mort** dans le
   WebView2 de Tauri — `matches` jamais vrai, zéro événement, trois
   bascules dans les deux sens. L'API Tauri `theme()`/`onThemeChanged`
   est **vivante** — `light`, `dark`, `light` reçus à chaque bascule.

**Racine** : le suivi OS écoutait un canal qui n'existe pas en
production. **Remède le jour même** : `theme.js` lit l'API fenêtre
Tauri (état initial + `onThemeChanged`), `matchMedia` reste en OU —
repli hors Tauri et poignée du banc (emulateMedia). Filet : nouveau
test e2e à bascule réelle (`bascule-sombre.ps1`, registre + diffusion,
Windows seulement, restauration garantie), le sens du retour au clair
compris — le point 4 exact du constat.

Deux enseignements de banc au passage : (a) le premier remède (OU
permanent `tauri || matchMedia`) rendait `emulateMedia('light')` à
jamais perdant sur une machine hôte sombre — remplacé par « les deux
canaux écrivent le même état, le dernier signal gagne » ; (b) le profil
WebView2 de la suite (`target/e2e/webview2`) persiste entre les runs :
un test qui meurt après avoir armé un réglage empoisonne les relances —
purge du profil, et le savoir est consigné. Re-gate après remède :
**75/75 e2e** (1,6 min), fmt/build/contraste/cohérence/garde/clippy/423
tests Rust déjà verts.

## 8. Revue à regard neuf (E6, 2026-08-16)

`/code-review high`, huit angles, dix constats confirmés. Corrections
appliquées le jour même :

1. **`e2e/jetons.mjs` et ce PLAN étaient non suivis** alors que les
   gates les importent/citent — un commit des seuls fichiers suivis
   cassait la CI en `ERR_MODULE_NOT_FOUND` avec gate locale verte
   (le piège PASSATION §7.4). `git add` des deux.
2. **La coche du sélecteur suit désormais la fiche AFFICHÉE**
   (`themeAffiche()`, signal `wind:theme-affiche` posé par `poser()`) :
   sous suivi OS + OS sombre, l'écran et la coche disent tous deux la
   déclinaison `-nuit` — avant, le clic de « correction » sur la fiche
   `-nuit` enfermait dans le sombre permanent. Plus jamais `actif = id`
   après `appliquerTheme` (qui refuse en silence).
3. **`sombreAutoDesc` dit l'exception** (« Un thème nuit choisi à la
   main reste tel quel ») — fr, en, et l'écran Réglages du Système.
4. **Le corps de message bake la palette du thème** (décision CE du
   jour, périmètre élargi) : `mail_render::Palette` (encre + fond,
   validés `#rrggbb`, défaut clair), passée par `message_body`/
   `echo_body` (`PaletteLecture`), lue aux jetons calculés
   (`paletteLecture()`), iframes au jeton `--surface`. TDD : 2 tests
   RED montrés puis verts (20/20 mail-render).
5. **Assertion pleine au retour clair** : `not.toHaveAttribute('data-theme')`
   au lieu de « autre chose que nature-nuit ».
6. **La transition vers le clair avec un `-nuit` choisi est assertée**
   (estampe-nuit reste posé quand l'OS repasse au clair).
7. Ids retirés jamais purgés du storage : **assumé** — décision CE D4
   (repli silencieux à la lecture), consigné, pas de changement.
8. Pastilles FICHES ↔ jetons livrés : contrôle n°4 de
   `coherence-systeme.mjs` (posé en séance).
9. **Contrôle n°5 ajouté** : chaque thème livré a son
   `theme.<id>.nom` aux deux catalogues (la parité fr↔en ne le disait
   pas — clé brute rendue en vert partout).
10. **La gate contraste n'imprime plus que les écarts** (28 × 25 = 700
    lignes « ok » noyaient l'échec) ; le verdict final porte les
    comptes.

Écart de méthode assumé : les amendements e2e (5, 6, coche) ont été
écrits avec leur implémentation, sans RED isolé — le RED aurait exigé
un run complet de la suite par assertion ; la gate d'E6 fait foi.
