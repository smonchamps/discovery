# PLAN-UI-V3 — la revue d'annotation CE du 2026-08-16 appliquée à l'écran 02

> **CHANTIER SOLDÉ le 2026-08-16 — terrain complet.** GO CE au STOP 1
> le 2026-08-16 (D1-D5 consignées §4) ; livré en un commit UI
> `16f06e6` (A43), CI verte (run 31940303485) ; terrain CE validé 6/6
> le jour même, aucune retouche. La revue à regard neuf (§6, dix
> constats) est corrigée DANS le commit. Reports en dette : D-12
> (cascade thread_messages→corps, re-base P1), D-13 (remontage des
> iframes à l'agrandissement). Le sort DC-D4 des quatre maquettes non
> suivies de `docs/design/` reste une décision CE ouverte.

> Instruction CE du 2026-08-16 : appliquer les verdicts de la séance
> d'annotation de `prototype-classique.html`
> (`docs/design/ANNOTATIONS-V3.md`) sur l'UI v2. Nav et filtres
> conservés ; en-tête, bandeau de liste, format des lignes et volet de
> lecture repris de la maquette, moins la note privée de fil et le
> bouton « Plus ».

## 1. Constat — instruction sur pièces, 2026-08-16

Six verdicts et deux exceptions au registre. Confrontés au code :

1. **Nav (verdict 1 : conserver)** — `Nav.svelte` inchangé. Les
   sections Bibliothèque/Étiquettes/Boîtes de la maquette sont de la
   fiction de maquette, pas un dû.
2. **En-tête (verdict 2 : remplacer par la maquette)** — l'en-tête v2
   (A30, `App.svelte:990`) porte déjà la composition exacte de la
   maquette : marque 18 px + hitofude à −3 px, recherche centrale,
   Écrire accent, Réglages. L'écart réel est nul au gabarit ; seule une
   vérification au pixel (hauteur 52 px, fond panel, recherche sur
   surface max 520 px) est due. **Le verdict se solde par une passe de
   conformité, pas une réécriture.**
3. **Bandeau de liste (verdict 3 : ajouter, sans « Tout marquer lu »)**
   — n'existe pas en v2 : `Liste.svelte` commence à la première ligne.
   Ajout net d'un `listeTete` (h1 16 px, le nom de la boîte courante,
   clés catalogue existantes `boite.*`).
4. **Lignes de liste (verdict 4 : format maquette)** — la ligne v2 est
   « nue » par décision A29/A2 (pas d'avatar, pas de puces — les puces
   vivent au volet de lecture). Le verdict rouvre A29 : la ligne
   maquette porte un avatar 28 px (initiales, cliquable = sélection en
   lot) et un rang de puces (étiquette, remonté, note de ligne). Or au
   cœur : ni étiquettes, ni « remonté », ni notes de ligne, ni actions
   en lot — seule la mention Brouillon existe (variante B, déjà
   rendue). Décisions D2/D3.
5. **Filtres (verdict 5 : conserver)** — les onglets v2
   (Tous/Non lus/Brouillons, pied de colonne) restent.
6. **Volet de lecture (verdict 6 : layout maquette, sans noteFil ni
   « Plus »)** — v2 sert UN message dans le volet (`Lecture.svelte`) et
   le fil complet à l'écran 03 plein écran (`Conversation.svelte`,
   « une seule surface de lecture plein écran », V-D2). La maquette met
   le FIL dans le volet : cartes repliées une ligne (avatar · nom ·
   résumé · quand), dernier message déplié, « Tout déplier », fichiers
   joints, barre d'actions. C'est le morceau structurel : le volet
   devient un fil, le devenir de l'écran 03 est à trancher (D4).
7. **Chantier A42 « Wada élargi » non commité** — 716 lignes de diff en
   arbre de travail (systeme.css, theme.js, catalogues, gates, Système).
   L'ordonnancement des deux chantiers est à trancher (D1).

Budget sous surveillance (PASSATION §3) : l'ouverture liste → lecture
passe de `message_body` seul à `thread_messages` + corps du dernier —
le chrono de sélection sera re-mesuré (PLAN-REACTIVITE tenait la
destination < 1 s).

## 2. Périmètre

**Fait** : bandeau de titre du volet liste ; ligne de liste au format
maquette (portée selon D2/D3) ; volet de lecture en fil (portée selon
D4) ; passe de conformité en-tête ; amendement A-n au Système + gates
et e2e à jour — un seul commit UI (DC-D2).

**Refus explicites** :
- **Pas de « Tout marquer lu »** (verdict 3).
- **Pas de note privée de fil** (exception a).
- **Pas de bouton « ⋯ Plus »** (exception b) : la barre d'actions garde
  ses gestes directs v2 (Répondre, Répondre à tous, Transférer,
  Archiver, Supprimer).
- **Aucune feature de fiction de maquette** : étiquettes, remonté,
  notes de ligne/fil, collections, parcours, piles, kiosque, portier —
  rien de tout cela n'entre par ce chantier ; le RÉSERVOIR ORGANIZED
  reste fermé sans instruction CE (verdict du 2026-08-15).
- **Nav et filtres intouchés** (verdicts 1 et 5).

## 3. Étapes

- **E1 — Bandeau de liste.** `listeTete` en tête de colonne : h1 16 px
  au nom de la boîte courante. RED : e2e qui attend le bandeau et
  l'absence de « Tout marquer lu ».
- **E2 — Ligne de liste au format maquette** (selon D2/D3). Grille
  avatar/contenu/heure, avatar aux initiales de l'expéditeur ; états
  A30 inchangés (filet, 700 non-lu, survol, liseré choisi). RED : e2e
  du gabarit de ligne.
- **E3 — Volet de lecture en fil** (selon D4). Titre + puces méta +
  « Tout déplier » ; cartes repliées/dépliée, iframe sandbox par
  message déplié (invariant S1 intact, `allow-same-origin` sans
  `allow-scripts`) ; fichiers joints ; barre d'actions directe. RED :
  e2e du volet (fil de n messages, dernier déplié).
- **E4 — Conformité en-tête** : mesure au pixel contre la maquette,
  corrections éventuelles.
- **E5 — Système + gates.** Amendement A-n (journal + gabarits
  écran 02), `coherence-systeme.mjs` et e2e à jour, `/gate` complète.

## 4. Décisions CE (STOP 1)

Réponses consignées le 2026-08-16, mot pour mot.

- **D1 — Ordonnancement face au chantier A42 non commité.**
  _Réponse CE : « Solder A42 d'abord (Recommandé) »._ La v3 ne pose
  aucun code avant qu'A42 soit commité et sa CI verte.
- **D2 — Avatar de ligne : visuel seul, ou avec la sélection en lot ?**
  La maquette fait de l'avatar le point d'entrée des actions en lot —
  mécanique absente du cœur (endpoints, barre de lot).
  _Réponse CE : « Avatar visuel seul (Recommandé) »._ Le lot est une
  feature à part, à instruire plus tard sur instruction CE.
- **D3 — Rang de puces de ligne : différé jusqu'aux features, ou
  gabarit posé à vide ?** Seule la mention Brouillon existe
  aujourd'hui. _Réponse CE : « Différé (Recommandé) »._ Le rang de
  puces naîtra avec la première feature qui le remplit.
- **D4 — Le volet de lecture devient le fil : que devient l'écran 03
  plein écran (Conversation) ?** _Réponse CE : « peut-on faire une
  coexistence qui n'est en fait qu'un changement de taille des mêmes
  objets ? »_ — verdict retenu : **coexistence par composant unique**.
  Le fil en cartes est extrait en un composant (`Fil.svelte`) ; le
  volet et l'écran 03 sont deux cadres du même objet (« agrandir » ne
  change que le conteneur, état de dépliage et corps partagés). Une
  seule surface de fil à maintenir.
- **D5 — Barre d'actions du volet : les cinq gestes v2 directs
  (avec Archiver), sans « Plus » ?** _Réponse CE : « Cinq gestes v2
  (Recommandé) »._ Répondre (accent), Répondre à tous, Transférer,
  Archiver, Supprimer — directs, sans menu.

**GO CE du 2026-08-16** sur ce périmètre ; l'exécution démarre à la
clôture d'A42 (D1).

## 5. Exécution (2026-08-16)

A42 soldé (241cdb2 + revue adac0c4, CI vertes) — la v3 a démarré sur
arbre propre, TDD (RED montré à chaque étape e2e) :

- **E1 livré** : bandeau `liste-titre` (clés `boite.*` de la nav),
  sans bouton.
- **E2 livré** : grille avatar 28 px aux initiales (`initiales()`),
  visuel seul ; sonde, squelettes et dossier Brouillons au même
  gabarit.
- **E3 livré** : état partagé `lib/fil.svelte.js` + composant
  `Fil.svelte` ; `Lecture.svelte` et `Conversation.svelte` réduits à
  deux cadres (`masquer`/`montrer` côté volet — un seul exemplaire
  monté à la fois, testids uniques) ; garde d'images distantes par
  message (dû de l'extraction : manquait au plein écran) ; testids
  unifiés (`fil-sujet`, gestes sans préfixe `conv-`), quatre specs
  amendés. Chrono d'ouverture conservé (`fil.derniereOuvertureMs`,
  API `__mesure` intacte).
- **E4 livré** : en-tête 52 px, gouttières 14/12, recherche max
  520 px, gestes à droite.
- **E5 livré** : Système amendé (A43 + gabarit écran 02 : bandeau,
  avatars, volet en fil, en-tête 52) dans le même commit.

Constat de suite : l'échec local du parcours brouillons (fantôme,
0956c85) rejoué 1×/2 sous charge, 9/9 en isolation — flake documenté,
CI de référence.

## 6. Revue à regard neuf (2026-08-16)

`/code-review high`, huit angles, dix constats confirmés — tous
corrigés le jour même :

1. **L'exclusivité des cadres vit au store** (`fil.cadre` :
   null/volet/plein) — les trois booléens réconciliés à la main
   (visible/cache/fil.ligne) se désynchronisaient au raccourci
   d'archivage depuis l'écran 03 (plein écran fantôme, double-montage)
   et à la bascule de disposition. `masquer`/`montrer`/`estOuverte`
   locaux supprimés ; test de régression en fin de spec ecran02.
2. **Chaque ouverture recharge le fil** — la mémoïsation rendait un
   fil périmé (sa propre réponse absente en 2 volets) et figeait un
   échec ; l'agrandissement passe par `agrandirFil()` (zéro
   rechargement), jamais par `ouvrirFil`.
3. **`vue.attachment_count` retenu par message** (`fil.nbPieces`) —
   le terrain 2026-08-14 était régressé (pièces invisibles au premier
   dépliage d'un message frais).
4. **Jeton anti-course dans `chargerMessage`** — une réponse
   images-ON tardive n'écrase plus un corps rebloqué.
5. **Purge atteignable dans tous les modes** — App importe
   `fermerFil()` (les `lecture?.fermer()` étaient des no-op en 1-2
   volets : fil d'un compte retiré ressuscitait).
6. **« Répondre » cible le dernier message d'AUTRUI** (répondre à sa
   propre copie des Envoyés composait vers soi-même).
7. **« Voir la conversation » inerte sur message sans fil** (V-D2,
   aria-disabled restauré).
8. **Chrono P1 : pièces hors mesure** (métadonnées détachées du
   chemin) ; la définition inclut désormais thread_messages — série à
   re-baser, dit au Système.
9. **En-tête écran 03 à 52 px** (le saut de 8 px à l'agrandissement).
10. **DC soldé** : section écran 03 aux géométries livrées, les quatre
    états de « Ligne de message » à l'avatar, dossier Brouillons
    (avatar destinataire, tiret), A43 nomme A29 et V-D2 amendés,
    règles de transmission A1 mises au bake de palette, orphelins
    retirés (quandLong, lecture.dernier/lecture.a, quand.aujourdhui),
    estEcho unifié, specs re-scopés au cadre + assertion d'unicité.
