# PLAN-RETOURS-V3 — quatre retours CE sur l'UI v3

**CHANTIER SOLDÉ le 2026-08-16 — terrain complet.** Commit f7c63e7
(A44), CI verte (run 31944140314). GO CE au STOP 1 le 2026-08-16
(verdicts D1-D4 au §4) ; terrain en deux passes le même jour : 2/3/4
OK d'emblée, le point 1 corrigé le jour même (hauteur au contenu, §6)
puis « C'est bon ». Statut antérieur : GO. Ouvert le 2026-08-16, à la
suite de la clôture de PLAN-UI-V3 (A43, 16f06e6) — les « retours
d'interface du CE » annoncés à la session de clôture.

## 1. Constat (instruction sur pièces)

### R1 — puces messages / pièces jointes absentes du volet central

- Le « volet central » est la **liste** (nav | liste | fil). Depuis
  **A29** (v2 Wada, 848f286) la ligne de liste est *nue* par décision :
  les puces fil/fichiers ont quitté la ligne pour le volet de lecture.
  Le commentaire vit dans `Liste.svelte:376`, et un e2e l'asserte
  (`refonte-parcours-portes.spec.js:114` : « la ligne est nue »).
- Les puces vivent aujourd'hui en tête du Fil (`Fil.svelte:99-104` :
  « N messages », « N fichiers ») — elles n'ont pas disparu de là.
- Les **données sont déjà sur chaque ligne** servie par
  `list_category` : `thread_size` (messages du fil, store.rs:296) et
  `attachment_count` (store.rs:282, 0 tant que le corps n'est pas lu —
  le trombone apparaît au fil du rattrapage, jamais à tort).
- **Point dur mesuré par l'histoire** : avant A29 la liste avait DEUX
  gabarits (ligne avec/sans rang de puces) et le fenêtrage portait
  toute une mécanique de correction (`chipsParPage`, `extraPuce`,
  `chipsAvant` — visible à 848f286~1). A29 l'a tuée : un gabarit, la
  géométrie est une multiplication. Ressusciter une hauteur variable
  ressusciterait cette complexité et ses bugs d'ancrage.
- Ce retour **renverse A29/A2** (le CE tranche) ; l'e2e « ligne nue »
  sera renversé dans le même commit, le Système amendé (DC-D2).

### R2 — bandeau « Boîte de réception » au format du bandeau de filtre

- Le bandeau haut (`Liste.svelte` `.bandeau`) : padding 12/16/8, sans
  fond, sans filet, titre 16 px 600.
- Le bandeau bas (`.onglets`) : **hauteur 52 px, fond `--panel`, filet
  `border-top`, padding 0 12px**.
- Aligner le haut sur ce format : 52 px, fond `--panel`, filet
  `border-bottom`, titre inchangé (16 px 600). L'en-tête du Fil est
  déjà à 52 px (A43 E4) — les trois têtes de volet seront au même rang.

### R3 — volets redimensionnables à la souris

- La grille est figée : `248px 400px minmax(0,1fr)` en trois volets,
  `248px minmax(0,1fr)` en deux (`App.svelte:1175-1181`).
- Aucune poignée n'existe. Le patron de persistance existe
  (`volets.svelte.js` : localStorage, restauration avant premier
  rendu, valeur inconnue → défaut).
- Proposition : une poignée de saisie sur chaque frontière de la
  grille (nav|liste et liste|fil en trois volets ; nav|liste en deux),
  curseur `col-resize`, largeurs bornées (nav 180–400 px, liste
  300–640 px), persistées par mode, **double-clic = retour au défaut**.
  Le fenêtrage de la liste est insensible à la largeur (pitch = hauteur
  de ligne sondée) — pas de risque sur la géométrie.

### R4 — standard des barres de défilement, celui du prototype Classique

- `prototype-classique.html` ne pose **aucune** règle de scrollbar :
  ce que le CE voit et approuve est la barre **par défaut de Chromium
  sur Windows 11** — le style *Fluent overlay* : fine, sans piste,
  posée SUR le contenu, visible au défilement et au survol seulement.
- L'app, elle, force une barre pleine de 10 px toujours visible
  (A7, `systeme.css:329-338`).
- Deux options (set-based, mesure dans l'app réelle avant verdict) :
  - **(a) La vraie barre native** : retirer les règles A7 et activer
    le style Fluent overlay de WebView2
    (`additionalBrowserArguments: --enable-features=msOverlayScrollbarWinStyle`).
    Fidélité parfaite au prototype ; à mesurer : le flag est-il honoré
    par la version WebView2 embarquée, et la barre suit-elle le thème
    (elle est native — probablement non thémée).
  - **(b) Émulation CSS** : garder `::-webkit-scrollbar` mais au
    gabarit overlay — fine (8 px), piste transparente, poignée aux
    jetons du thème, révélée au survol du cadre seulement. Thémée,
    maîtrisée, mais « identique » à l'œil seulement.
- La mesure (E4) départage ; A7 est amendé au Système quoi qu'il en
  soit (DC-D2).

## 2. Périmètre — refus explicites

- **Pas de sélection en lot** : l'avatar reste visuel seul (D2 d'A43,
  différé — rien ici ne le rouvre).
- **Pas de rang de puces « features »** (étiquettes, remonté, note de
  ligne — le réservoir ORGANIZED reste fermé, verdict 2026-08-15) :
  seules les deux puces factuelles (messages, fichiers) reviennent.
- **Pas de redimensionnement de la fenêtre 1 volet** (une seule
  colonne, rien à saisir) ni du tiroir (268 px, A26).
- **Pas de « Tout marquer lu »** au bandeau (déjà écarté à E1 d'A43).

## 2 bis. Verdict de mesure E4 (2026-08-16)

Banc `e2e/mesure-scrollbar.mjs` (sonde : `scrollbar-color` non-défaut
force le chemin standard — `auto` ne suffit pas, c'est la valeur par
défaut et les règles webkit tiennent). Épaisseur réservée mesurée dans
l'app réelle :

| Configuration | Épaisseur |
|---|---|
| Règles webkit A7 (avant) | 10 px |
| Barre native classique (sans flag) | 15 px |
| `msOverlayScrollbarWinStyle` (Fluent Windows) | 15 px — **non honoré** |
| `OverlayScrollbar` (Chromium générique) | **0 px — overlay, adopté** |

Verdict : option (a) au trait `OverlayScrollbar` — la barre native en
surimpression, la forme du prototype. Pièges consignés : le loader
WebView2 fait ÉCRASER `additionalBrowserArgs` (conf Tauri) par la
variable d'environnement — le lanceur e2e repose donc le flag ; le
champ de conf Tauri s'épelle `additionalBrowserArgs` et sa pose
remplace les `--disable-features` par défaut de wry, repris dans la
valeur. La poignée suit `color-scheme`, posé par theme.js (-nuit →
dark). Les règles webkit A7 sont retirées ; en poser une seule ferait
retomber l'élément hors du chemin overlay.

## 3. Étapes

- **E1 — Bandeau au format du filtre** (R2). CSS seul ; test e2e du
  gabarit (hauteur, fond) ajusté s'il existe. Gate.
- **E2 — Puces en liste** (R1, selon D1/D2). Gabarit UNIQUE préservé ;
  e2e « ligne nue » renversé (la ligne 190 porte « 2 » au trombone) ;
  contraste mesuré sur les trois fonds de rangée. Gate.
- **E3 — Poignées de redimensionnement** (R3, selon D3). Largeurs en
  variables CSS pilotées par un `largeurs.svelte.js` (patron volets) ;
  e2e : saisir, relâcher, recharger — la largeur survit. Gate.
- **E4 — Barres de défilement** (R4, selon D4) : mesure des deux
  options dans l'app réelle, verdict consigné ici, implémentation.
  Gate.
- **E5 — Système** : A44 au journal, cartes amendées (Ligne, bandeau,
  volets, A7) — au fil des commits, DC-D2.

État (2026-08-16) : E1-E5 **livrés** — RED montré (3 échecs attendus),
GREEN aux mêmes specs (58/58 sur ecran02 + parcours-portes, 9/9 sur
volets), build zéro avertissement, mesure E4 au § 2 bis, Système amendé
(A44).

## 5. Revue à regard neuf (2026-08-16, huit angles)

Dix constats confirmés, tous corrigés le jour même :

1. **Saisie des poignées sans `pointercancel`/`lostpointercapture` ni
   garde de bouton** — une saisie interrompue restait armée, le survol
   redimensionnait ensuite sans bouton. Corrigé : les quatre événements
   défont la saisie, seul le bouton principal saisit.
2. **Bornes cumulées (400 + 640) > fenêtre de 1000 px** — fil écrasé à
   0, poignée poussée hors écran, état persisté irrécupérable.
   Corrigé : plafond de fenêtre (réserve du fil 120 px) par-dessus les
   bornes, dans l'App (connaissance d'UI) ; au rétrécissement de la
   fenêtre, la liste cède (`onresize`). Le spec e2e prouve le plafond
   (nav retenue à 240 quand la liste est à 640).
3. **Iframe du corps sans `color-scheme`** — poignée overlay invisible
   sur les 14 thèmes -nuit. Corrigé dans mail-render (TDD, dérivé de la
   luminance du fond baké, Rec. 601).
4. **`color-scheme` de l'hôte posé en JS** — déplacé en CSS à côté des
   jetons (`:root[data-theme$="-nuit"]`) : tout chemin qui pose
   data-theme l'obtient.
5. **Arguments navigateur en deux exemplaires** (conf Tauri vs lanceur
   e2e, `--disable-features` perdus ; `--enable-features` répété = le
   dernier gagne) — une seule source : `args-navigateur.mjs` lit la
   conf, le lanceur, mesure-v2 et diag-v2 la reprennent.
6. **`persister()` à chaque pointermove** — le glissement RÈGLE, le
   relâchement PERSISTE (module scindé).
7. **Recherche : `thread_size` codé 1 côté cœur** — la puce de fil n'y
   figure pas ; consigné comme règle (un résultat est un message), au
   Système et au code.
8. **Commentaires « ligne nue » contredits par le code** — redressés ;
   le rang réservé devient un TRACK de la grille (plus de div vide à
   recopier par variante de rangée).
9. **Poignées en copier-coller** — un seul gabarit (snippet Svelte).
10. **Commentaire épelant `additionalBrowserArguments`** (le champ
    livré est `additionalBrowserArgs`) — corrigé ; et la gate de
    cohérence gagne la garde n°5 : aucune règle de barre de défilement
    dans ui-v2/src (commentaires exclus), la régression silencieuse de
    l'overlay est fermée.

Refus maintenus : puces `.puce` non factorisées entre Liste/Fil/
Composition (trois gabarits aux cotes différentes, une classe commune
coûterait plus qu'elle ne rend — constat de revue) ; divergence
liste (compte d'avant) / tête du Fil (compte d'après scan) assumée et
documentée (A44) ; chevauchement poignée / barre overlay de la nav
(3 px) accepté — le compromis standard des séparateurs.

Gate complète du 2026-08-16 : **verte** — fmt, build ui-v2 0
avertissement, contraste 700 paires / 28 thèmes, cohérence 476 valeurs
(garde n°5 comprise), garde 62 commandes, clippy muet, 426 tests Rust,
80 e2e (1,7 min, sans flake).

## 6. Terrain (STOP 2, 2026-08-16)

Verdict CE, mot pour mot : « 1 : OK pour affichage des puces ; le
visuel du rang réservé n'est finalement pas agréable, implémente le
même comportement que sur le prototype (la présence de puce augmente
la hauteur). 2 : OK. 3 : OK. 4 : OK. »

**D1 est renversé au terrain** : hauteur AU CONTENU, pas de rang
réservé. Correction le jour même : la mécanique de fenêtrage à deux
gabarits d'avant A29 (chipsParPage, extraPuce, chipsAvant, correction
itérative de l'index, ancrage au delta d'une page resservie) est
ressuscitée à l'identique depuis l'historique (848f286~1), adaptée au
dessin sans marges (PAD/GAP morts) ; deux sondes (h1 nue, h2
porteuse) ; le rang n'existe que sur les porteurs, l'avatar revient
sur trois rangs ; e2e renversé (porteur PLUS HAUT que la nue, pas de
rang sur la nue) ; Système amendé (carte Ligne, journal A44, mocks).
Re-gate puis nouvelle passe terrain sur le seul point 1.

## 4. Décisions CE

- **D1 — Forme des puces en liste.** (a) *Compacte dans le gabarit
  fixe* : icône + nombre discrets en colonne droite de la ligne, sous
  l'heure (la grille `auto 1fr auto` du prototype) — hauteur de ligne
  inchangée, fenêtrage intact — **recommandée** ; (b) rang de puces
  24 px du prototype, à hauteur RÉSERVÉE sur toutes les lignes (les
  lignes sans puces portent un vide de 30 px) ; (c) rang à hauteur
  variable (ressuscite la mécanique à deux gabarits tuée à A29 —
  déconseillée).
- **D2 — Quelles puces.** « N messages » si `thread_size > 1` et
  « N fichiers » si `attachment_count > 0` (les mêmes règles que la
  tête du Fil) — ou autre chose ?
- **D3 — Frontières et bornes du redimensionnement.** Les deux
  frontières (nav et liste) ou la seule frontière liste|fil ? Bornes
  proposées : nav 180–400, liste 300–640 ; double-clic = défaut.
- **D4 — Barre de défilement.** Mandat à la mesure (a) native Fluent
  si le flag est honoré, sinon (b) émulation CSS — ou un choix ferme
  d'emblée.

### Verdicts CE (STOP 1, 2026-08-16)

- **D1** : « Rang de puces, hauteur réservée » — le rang de puces
  24 px du prototype, présent sur TOUTES les lignes (vide sur les
  lignes sans puces). Gabarit unique gardé, lignes plus hautes partout.
- **D2** : « Les mêmes que le Fil (Recommandé) » — « N messages » si
  `thread_size > 1`, « N fichiers » si `attachment_count > 0`.
- **D3** : « Les deux frontières (Recommandé) » — poignées sur
  nav|liste ET liste|fil (nav|liste seule en deux volets) ; bornes nav
  180–400 px, liste 300–640 px ; double-clic = défaut ; persistées.
- **D4** : « Mandat à la mesure (Recommandé) » — native Fluent si le
  flag WebView2 est honoré et rend bien sur les thèmes sombres, sinon
  émulation CSS overlay fine aux jetons ; verdict consigné ici.

Statut du plan : **GO** — implémentation ouverte le 2026-08-16.
