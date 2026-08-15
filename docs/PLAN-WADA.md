# Plan — Système v2 « Wada » : la palette, la signature du vent et les dessins des pistes

**Statut : GO CE le 2026-08-15 — W2-D1 à W2-D7 telles que proposées.
E1 livrée le jour même** (police au balayage, icône 42/64, preuve
45/45) ; E2 en cours ; E3 attend le terrain. Les verdicts chiffrés du
banc (jetons neufs, remèdes A8) sont consignés au §4 — ils font foi
pour E2. Source : le projet Claude Design **« Amélioration
système de couleurs »**
(<https://claude.ai/design/p/19bf2156-10a1-404f-9450-317380da9c83>),
fichier **`Systeme v2.dc.html`** — l'évolution du Système de référence
(seul normatif et exhaustif, A18), journal porté d'A27 à **A34**.

Commande (2026-08-15) : analyser les changements formalisés par ce
document, puis formaliser le plan pour les implémenter partout où ils
portent — code, document, catalogues, police, icône, gates.

## 1. La source et son périmètre

Le projet Design contient cinq fichiers. **Un seul entre au dépôt** :

| Fichier | Nature | Sort |
|---|---|---|
| `Systeme v2.dc.html` | le Système révisé, journal A28–A34 | remplace `docs/design/systeme.dc.html` au commit UI (E2), après corrections §4 |
| `uploads/systeme.dc.html` | la base servie à l'étude — notre document actuel | rien (copie de travail) |
| `Pistes couleur - Sanzo Wada.dc.html` | l'étude des combinaisons | reste au projet Design (archive d'étude) |
| `Prototype pistes - écrans.dc.html` | le prototype dont A29 reprend les dessins | reste au projet Design ; sa substance est reversée dans la v2 (l'esprit DC-D4) |
| `Lien liste-lecture - options.dc.html` | l'étude du lien liste–lecture | **ouverte** (A31 : le pont de teinte essayé puis annulé) — reste au projet |

L'analyse est un diff intégral contre `docs/design/systeme.dc.html`
(1467 lignes → 1315). Constat de structure : **les vingt sections
sont conservées**, aucun écran n'apparaît ni ne disparaît. Trois
chiffres disent la nature du changement : 131 tirets cadratins → 0
(A34) ; 23 filets gauches d'accent → 2 (A29 — seule la ligne
sélectionnée garde son liseré) ; le contrat des jetons reste à
14 rôles × 7 thèmes.

## 2. Ce que la v2 change, amendement par amendement

**A28 — la palette « Wada » et la signature du vent.** Remap à teinte
d'usage constante, thème « La nature » (défaut) :

| Jeton | Avant | Après |
|---|---|---|
| `--bg` (fond) | `#f0f2ef` | `#f2f0ea` (papier écru) |
| `--panel` | `#eaece9` | `#e9e6dd` |
| `--surface` | `#ffffff` | `#ffffff` (inchangé) |
| `--ink` (encre) | `#232725` | `#24272e` (bleutée) |
| `--ink2` | `#4d534f` | `#4a505a` |
| `--muted` | `#5c625e` | `#5c6470` |
| `--border` (filets) | `#dadfda` | `#cdc6b8` |
| `--accent` | `#2f6e5b` | `#1e7566` (vert-de-gris ; blanc dessus 5,54:1) |
| `--accentH` | `#26594a` | `#175c50` |
| `--sel` | `#e6ede9` | `#cddcd2` |
| `--alert` | `#8c2f22` | `#9e3a2c` |
| `--shadow` / `--scrim` | rgba(35,39,37,…) | rgba(36,39,46,…) |

`--alert` suit aussi dans **air, eau, astres et terre** (`#8c2f22 →
#9e3a2c` au contrat) ; le feu (`#7a2617`) et la nuit (`#ea9a90`, A19)
gardent leurs valeurs. **Tout le reste des six autres thèmes est
inchangé.** Couleurs de marque (W-D3) inchangées. Nouvelle signature :
le trait **hitofude** (un coup de pinceau, couleur d'accent), deux
emplacements normés — statique à droite du mot « Wind » (52 × 10,
+3 px sous la ligne de base), animé dans le bouton de relève pendant
un cycle (boucle 4 s : tracé 2 s, plein 1 s, fondu ; `hitofudeDraw` /
`hitofudeFade` ; `prefers-reduced-motion` : plein, fondu seul).

**A29 — quatre décisions.** (1) La signature historique (surface
ivoire + filet d'accent 2 px au bord gauche) est **retirée** partout ;
le hitofude est la seule signature. (2) La **Nav** adopte le dessin du
prototype de pistes : rangées 14 px à rayon 8 (padding 8/10), item
actif en teinte de sélection **bordée d'accent** (border 1 px, plus de
filet gauche ni d'ombre), **pastille de non-lus pleine** (pilule
accent, blanc 700) ; la partie « Boîtes » garde son nom, la **boîte en
cours** reprend le dessin de la tuile d'événement (paille `#f1e7d3`,
ocre `#7a5a30`). (3) La **Liste** adopte le dessin du prototype :
lignes continues séparées au filet (plus de cartes), sélection en
teinte + liseré d'accent, non-lus en graisse 700, l'objet passe de
**16 à 14 px** (amende A9) ; les **trois filtres passent en bas** du
volet (barre de 52 px). (4) Le hitofude ne boucle que pendant un
cycle ; au repos il reste dessiné et immobile.

**A30 — mise en cohérence des dessins.** Entête : fond au jeton des
panneaux, **marque sans tuile-enveloppe** (« Wind » 18 px + trait),
recherche sur blanc. Ligne de message : quatre états au gabarit
prototype (survol en **teinte légère `#e4ded1`**, sans surface ivoire
ni ombre). Typographie : rangée « Objet » à 14 px, 400/700. Dossier
Brouillons : mini-liste au gabarit continu. Tiroir : entête sans
tuile (mot + trait). A2 (repli 104 px) mis à jour.

**A31 — le pont de teinte annulé.** Rien à implémenter ; l'étude
liste–lecture reste ouverte au projet Design.

**A32 — l'icône, enveloppe agrandie.** L'enveloppe passe à **42/64**
de la tuile (34/64 avant), soit 32/48 et 16/24 aux déclinaisons ;
trait, rayons, pastille, couleurs W-D3 inchangés. Régénération par
`scripts/faire-icone.ps1`.

**A33 — marges symétriques.** Règle nouvelle : dans toute puce, tout
bouton, tout onglet, un seul padding horizontal — une icône de tête ou
de fin ne réduit jamais la marge de son côté. L'audit du document a
corrigé les 5 puces de pièces jointes du composeur (12/8 → 12/12 px) ;
le même audit est dû au code.

**A34 — révision éditoriale.** Plus aucun tiret cadratin, libellés
d'interface compris : `liste.prefixeBrouillon` devient **« Brouillon : »**,
les états de la barre d'état passent au **point médian**
(« Synchronisation impossible · nouvelle tentative automatique »,
« {n} compte sur {m} injoignable · nouvelle tentative automatique »…).
Énoncés d'intention remplacés par des énoncés vérifiables. Aucun
changement de géométrie ni de couleur.

Un changement dessiné **sans amendement qui le dise** : l'écran 02
passe la colonne de Nav de **236 à 248 px** (les trois modes suivent :
« 248 / 400 / 1fr », « 248 / 1fr » en deux volets — la carte des modes
d'affichage est réécrite en ce sens), et le dossier Archives y porte
le glyphe **`inventory_2`** (aujourd'hui `archive`, qui reste employé
ailleurs). Voir W2-D3/W2-D4.

## 3. État des lieux

| Surface | Constat | Sort |
|---|---|---|
| `apps/desktop/ui-v2/src/systeme.css:12-19` | bloc nature = les 11 valeurs d'avant + shadow/scrim | remap A28 ; `--alert` des blocs air/eau/astres/terre ; cache-buster `?v=45` si W2-D3 (**E2**, police à **E1**) |
| `lib/theme.js:39` | pastilles du sélecteur, nature en valeurs v1 | `['#1e7566','#f2f0ea','#e9e6dd','#ffffff','#24272e']` (**E2**) |
| `App.svelte:935,1012` | marque-tuile (SVG enveloppe) à l'entête ET au tiroir | tombe (A30) ; « Wind » + trait hitofude statique (**E2**) |
| `App.svelte:1097-1104` | grilles `236px 400px…` / `236px…` (volets) | 248 px aux modes trois et deux volets (**E2**) |
| `App.svelte` — barre d'état | bouton de relève S-D1 ; pas de trait | hitofude animé pendant le cycle, plein et immobile au repos, reduced-motion (**E2**) |
| `Nav.svelte` | rangées 36 px/13 px/rayon 6, actif = surface + filet gauche + ombre, compteurs héros « 4 / 18 » | redessin A29.2 : 14 px/rayon 8, actif = sel + border accent, pastilles pleines, tuile boîte en cours (**E2**, W2-D4/D5) |
| `Liste.svelte:483-536` | cartes rayon 10, actif = filet gauche + ombre, objet 16 px, onglets en tête | redessin A29.3 : lignes continues, 14 px, survol teinte légère, filtres en bas (52 px) ; mini-liste Brouillons ; A2 repli (**E2**) |
| `Composition.svelte:647`, `Conversation.svelte:328,385`, `FenteAvis.svelte:29-32`, `GuichetCompte.svelte:137`, `Lecture.svelte:223`, `Reglages.svelte:402-524`, `Toast.svelte:20` | **15 filets gauches** d'accent/alerte (la signature historique) | instruits un à un contre les dessins v2 : tous tombent sauf le liseré de sélection de la Liste (23 → 2 au document) (**E2**) |
| `Composition.svelte` — puces de pièces | padding 12/8 (asymétrique) | 12/12 (A33) + audit complet puces/boutons/onglets de ui-v2 (**E2**) |
| `lib/catalogue.fr.js` / `catalogue.en.js` | `'Brouillon — '` / `'Draft — '`, états statut au cadratin, ~20 autres chaînes au cadratin | A34 : alignement verbatim sur le document, les deux langues (**E2**, portée W2-D6) |
| `assets/icones/README.md` + police | 44 glyphes ; `inventory_2` absent, `work` présent | régénération 44 → 45 (W2-D3), inventaire, `?v=45`, copie `public/`, preuve ligatures (**E1** — la police d'abord, patron 0.1.4) |
| `scripts/faire-icone.ps1:53,76-79` | enveloppe 34/64 (le 16 déjà à 42/64) | fractions 42/64, `icon.ico` régénéré (A32) (**E1**) |
| `assets/icones/apercu.html:18-19` | chrome de l'aperçu aux valeurs v1 | repeint (cosmétique, même commit que la police) (**E1**) |
| `e2e/contraste.mjs` | 17 paires ; rien sur survol/paille/ocre | paires nouvelles selon W2-D1 ; le banc reste la gate (**E2**) |
| `e2e/coherence-systeme.mjs` | 98 cellules (14 × 7) | suit le contrat : 119 si W2-D1 ajoute 3 jetons (**E2**) |
| `e2e/tests/refonte-ecran02.spec.js:340` | asserte `'Brouillon — '` | `'Brouillon : '` + toute assertion de géométrie touchée (**E2**) |
| `e2e/tests/refonte-volets.spec.js` | « mêmes rangées et mêmes compteurs » au tiroir | suit le verdict W2-D4 (**E2**) |
| `docs/design/systeme.dc.html` | le normatif actuel (A1–A27, CRLF) | remplacé par la v2 corrigée §4, journal A28–A34 (+A35 si corrections) (**E2**) |
| `docs/design/prototype-{accueils,organized,portier}.html` (non committés) | dessinés en palette v1 | rien ici — ils suivront leur propre GO (CONCEPTION-ORGANIZED §6) ; noter l'écart au moment de ce GO-là |
| `spikes/`, archives datées | valeurs v1 | rien (archives — W-D2, même logique) |

## 4. Les décisions — tranchées au GO du 2026-08-15, verdicts du banc

Le GO a retenu les sept décisions telles que proposées. Le banc de
conception (`banc-wada` : toutes les paires des sept thèmes, jetons
neufs compris) a ensuite arrêté les valeurs — elles font foi pour E2 :

| Thème | `--hover` | `--tuile` | `--tuileInk` | remède `--muted` (A8) |
|---|---|---|---|---|
| nature | `#e4ded1` | `#f1e7d3` | `#7a5a30` | `#5c6470` → **`#575f6a`** (muted/sel 4,20 → 4,54) |
| air | `#e6ecf1` | `#dce9f2` | `#2f5670` | — |
| feu | `#eee4d8` | `#f6e3c8` | `#7d4a1f` | — |
| eau | `#e0eae8` | `#d9ece1` | `#1d5c49` | — |
| astres | `#e7e9f1` | `#e5e3f6` | `#47427e` | — |
| terre | `#e9e4d8` | `#efe2c8` | `#6d5424` | — |
| nuit | `#24292c` | `#3b382a` | `#dfc893` | `#929a96` → **`#a0a7a4`** (muted/sel **3,83** découvert au banc → 4,50) |

Trois faits d'exécution s'ajoutent aux verdicts : (1) le banc a trouvé
un TROISIÈME défaut de contraste, `muted`/`sel` de la nuit à 3,83 —
même remède A8, éclaircir (le sens de la nuit, A19) ; (2) W2-D3 suivi
du balayage DC-D3 : `inventory_2` entre ET `work` sort (plus employé
nulle part après W2-D5) — le compte reste à **44 glyphes**, pas 45, le
cache-buster passe bien à `?v=45` ; (3) la phrase du doc « avec
prefers-reduced-motion, le trait reste plein et seul le fondu
subsiste » contredit la règle A8 globale (toute animation coupée) —
corrigée à la source en « le trait reste plein et immobile » (A35).

**W2-D1 — trois teintes hors contrat.** Le survol `#e4ded1`, la
paille `#f1e7d3` et l'ocre `#7a5a30` sont **dessinés mais absents du
contrat des jetons** (resté à 14 × 7). Or « toute couleur passe par un
jeton » (bascule de thème O(1)) — et ces teintes chaudes n'existent
que pour la nature : en l'air, la nuit ou les astres, elles jureraient
telles quelles. Proposition : **trois jetons nouveaux** (`--hover`,
`--tuile`, `--tuileInk`), déclinés aux sept thèmes, table du contrat
amendée à la source AVANT l'entrée au dépôt (98 → 119 cellules — la
gate suit mécaniquement), chaque déclinaison passée au banc.
L'alternative (valeurs figées au thème nature) casse la bascule.

**W2-D2 — deux contrastes en défaut au dessin, une phrase
invérifiable.** Mesures au calcul WCAG du banc : **`muted`/`sel`
4,20:1** et **`muted`/`survol` 4,46:1** — l'heure 12 px sur rangée
sélectionnée/survolée passe sous 4,5. Remède A8 (même teinte,
luminosité ajustée : `--muted` assombri, ou teintes éclaircies, ou
l'heure en `ink2` sur rangée teintée) à trancher et à dater au
journal. Et la phrase d'A28 « filets ≥ 3:1 aux frontières utiles »
mesure **1,49:1** (`#cdc6b8`/`#f2f0ea`) : à reformuler en énoncé
vérifiable — l'esprit d'A34 — ou à retirer.

**W2-D3 — le glyphe des Archives.** Le dessin passe à `inventory_2`
sans amendement qui le dise. Suivre le dessin (police 44 → 45,
`archive` reste employé ailleurs), ou corriger le dessin et garder
`archive` (aucune régénération). Proposition : suivre le dessin, et le
dire au journal.

**W2-D4 — les compteurs de la Nav.** Le dessin ne montre plus que des
pastilles de non-lus (Réception, Indésirables) : les totaux
(« 4 / 18 ») et les compteurs simples (Envoyés, Brouillons, Archives,
Corbeille) disparaissent, « Toutes les boîtes » n'a plus de chiffre.
Le dessin fait foi — confirmer, car c'est une perte d'information
réelle (et les e2e du tiroir suivent).

**W2-D5 — la tuile de la boîte en cours, aux données réelles.** Le
dessin la montre « Travail » avec le glyphe `work` — la fiction
d'exemple du document (elle prédate la v2). Wind n'a pas de nom de
compte (D7 : icône `person`, libellé = adresse). Proposition : la
tuile porte l'adresse (13 px, 600) et « N non lus » (12 px), icône
`person` — D7 tenu, pas de fonctionnalité « nom de compte » cachée
dans une refonte de couleurs.

**W2-D6 — la portée d'A34 sur les catalogues.** Les chaînes dessinées
au document s'alignent verbatim, c'est acquis. Les chaînes jamais
dessinées (toasts, erreurs, descriptions de thèmes…) portent ~20
cadratins de plus, `catalogue.en.js` aussi. Proposition : la passe
complète, les deux langues — « tous les textes, libellés d'interface
compris », A34 ne dit rien d'autre.

**W2-D7 — le rythme des commits.** Les sept amendements
s'entrelacent (la palette appelle la signature, la signature appelle
les dessins) : un découpage par amendement laisserait le document
dire ce qui n'est pas livré (A18). Proposition : **trois étapes,
trois commits** (§5) — la police et l'icône d'abord (préalables sans
géométrie), puis UN commit UI qui livre tout et fait entrer le
document entier (DC-D2 au sens fort), puis le terrain.

## 5. Les étapes

**E1 — la police et l'icône (préalables).** `inventory_2` entre au
sous-ensemble (44 → 45, sous réserve W2-D3) : régénération, inventaire
`assets/icones/README.md`, `?v=45` dans `systeme.css`, copie
`public/`, preuve ligatures rejouée (canal msedge). A32 :
`faire-icone.ps1` aux fractions 42/64, `apps/desktop/icons/icon.ico`
régénéré, vérification visuelle aux quatre tailles. `apercu.html`
repeint. Le journal du Système gagne **A32 seul** à ce commit (rangée
datée — le reste du document ne bouge pas encore). Gates complètes.

**E2 — la v2 à l'écran (le commit UI).** Tout le §3 : jetons,
pastilles, entête et tiroir sans tuile, hitofude aux deux
emplacements, retrait de la signature historique (les 15 filets
instruits), Nav et Liste aux dessins des pistes (248 px, pastilles,
tuile, filtres en bas, 14 px, mini-liste, A2), marges symétriques
auditées, catalogues fr/en, e2e ajustés (textes, géométrie,
compteurs), banc de contraste étendu, gate de cohérence au contrat
élargi. `docs/design/systeme.dc.html` est **remplacé par la v2
corrigée des verdicts W2-D1/W2-D2** (journal A28–A34, plus la rangée
qui date ces corrections). Fins de ligne CRLF conservées. Gates
complètes — le banc et la cohérence AVANT le commit, pas après.

**E3 — le terrain et la clôture.** Le constat CE (une journée en
usage réel : la palette sous les deux luminosités, le trait pendant de
vrais cycles, la Nav aux pastilles, la Liste aux filets), retouches au
verdict dans le même esprit que toujours (DC-D2 : le document
s'amende au commit qui retouche). Le plan se clôt au constat ; les
trois études du projet Design restent ouvertes (A31 le dit pour le
lien liste–lecture).

## 6. Ce qui ne bouge pas

Les six autres thèmes (hors `--alert` de quatre d'entre eux) ; les
couleurs de marque et l'icône dans ses couleurs (W-D3 — seule la
géométrie de l'enveloppe change) ; la colonne Liste (400 px) et la
Lecture ; le tiroir à 268 px et toute la grammaire des surimpressions
(Échap, scrim-bouton) ; les raccourcis (table D3) ; le corps en iframe
sandbox (S1) ; les textes hors cadratins ; les budgets PLAN.md ; la
règle des trois régions et la fente d'avis (qui perd seulement son
filet gauche).

## 7. Le constat de clôture

Vérifiable, pas déclaratif : (1) `docs/design/systeme.dc.html` égale
la source v2 corrigée, au diff près des fins de ligne ; (2)
`coherence-systeme.mjs` vert au contrat élargi ; (3) `contraste.mjs`
vert avec les paires nouvelles — plus aucune paire dessinée sous son
seuil ; (4) la suite e2e verte (CI de référence — le local flake) ;
(5) l'icône régénérée visible aux quatre tailles ; (6) le constat
terrain CE consigné ici avec sa date.
