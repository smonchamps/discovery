# Plan — Les volets d'affichage : un, deux ou trois, au choix

**GO du Chef Ingénieur le 2026-08-15 : V-D1 à V-D4 validées telles que
proposées (§4). E1 livrée le même jour et VALIDÉE AU TERRAIN (constat
CE : bascule au geste, zéro différence au défaut). E2 livrée dans la
foulée — voir son état à l'étape ; le plan se clôt au constat terrain
d'E2 (une journée en mode 1 volet, retour sans séquelle).** Le
prototype cliquable qui a présenté les modes
(`docs/design/prototype-volets.html`, validé au GO) mourra reversé au
Système à la clôture du plan (DC-D4).

Commande (2026-08-14) : dans une des toutes premières versions de
Discovery, on pouvait choisir si l'interface s'affichait en mode 1, 2
ou 3 volets. La fonctionnalité a disparu sans véritable justification.
La réintroduire, et loger le choix dans **Réglages > Affichage**.

## 1. L'histoire, instruite au dépôt

L'instruction honnête d'abord : **la mémoire git ne garde aucune trace
de ce sélecteur.** La v0 Tauri d'avril 2026 (l'histoire conservée au
commit 14650a4) affichait une grille FIGÉE à quatre colonnes
(`280px 360px minmax(520px,1fr) 340px`, `src/styles/app.css` de
l'époque), sans aucun choix ; la v1 vivait sur un `main.split` figé ;
la v2 est née sur la grille 236/400/1fr du prototype. Aucun commit ne
retire la fonctionnalité — elle a vraisemblablement vécu dans une
itération d'avant les premiers commits, ou d'avant l'import de
l'historique.

Conséquence pour ce plan : **il n'y a rien à restaurer, et aucune
contrainte héritée.** C'est une construction neuve sur l'écran 02,
dessinée au Système (seul normatif et exhaustif — A18), pas une
archéologie. Le constat « disparue sans justification » se solde ici :
la justification devient ce plan, dans un sens ou dans l'autre.

## 2. État des lieux

| Surface | Constat | Sort |
|---|---|---|
| `App.svelte:1028` — `.colonnes` | grille figée `236px 400px minmax(0,1fr)` : Nav, Liste, Lecture | la grille suit le mode (**E1**) |
| `App.svelte` — `surSelection` | ouvre TOUJOURS le volet Lecture (`lecture.ouvrir`), `mark_seen` au passage | route selon le mode (**E1**) |
| `Lecture.svelte` | le volet de lecture — vide « Sélectionnez un message » sinon | démonté en modes 1 et 2 ; les appels `lecture.fermer()` de l'App deviennent optionnels (`?.`) (**E1**) |
| `Conversation.svelte` | l'écran 03 plein écran — mais **`thread_id` seulement** (`thread_messages`) : un message sans fil (écho E3 compris) ne s'y ouvre pas | apprend le message seul (**E1**, selon V-D2) |
| `Nav.svelte` | composant autonome (props + `onchoisir`) — réutilisable tel quel | monté dans le tiroir en mode 1 (**E2**) |
| `Reglages.svelte:271` — groupe Affichage | deux rangées : Sombre automatique, Langue | rangée « Disposition » en plus (**E1**) |
| `lib/theme.js` | le patron localStorage des préférences pures UI (`wind-theme`, `wind-theme-auto` — D6) | le patron de `wind-volets` (**E1**, selon V-D4) |
| `lib/texte.svelte.js` | catalogues plats fr/en, parité affirmée par l'audit e2e | clés neuves fr **et** en (**E1/E2**) |
| `assets/icones/README.md` | 43 glyphes ; **`menu` n'y est pas** | régénération de la police pour le tiroir (**E2**, patron A13/A17 : inventaire, `?v=` bumpé, copie `public/`, preuve rejouée) |
| Système — Boîte de réception | l'écran 02 dessiné à TROIS volets, grille 236/400/1fr normative | carte « Les modes d'affichage » + règles (**E1/E2**, DC-D2 : même commit) |
| Système — Réglages | groupe Affichage : deux rangées dessinées | la rangée neuve s'ajoute au dessin (**E1**) |

Invariants qui ne bougent dans AUCUN mode : l'entête 60 px et la barre
d'état 36 px ; les surimpressions (Réglages, Composition, écran 01,
migration) ; les raccourcis (la table D3 est figée) ; le corps en
iframe sandbox (S1) ; le port en sondage (R0-S5) ; les budgets PLAN.md
§1 et la gate P1 (50 000 messages) — la virtualisation vit sur les
gabarits actuels, qui ne changent pas (V-D3).

## 3. La cible — la sémantique des trois modes

Le choix est **global, explicite, appliqué immédiatement** (le geste du
thème : pas de rechargement, pas de redémarrage), persisté, restauré au
lancement. Défaut : **3 volets** — qui ne touche à rien ne voit rien
changer, pas même une frame.

| Mode | Grille | L'ouverture d'un message |
|---|---|---|
| **3 volets** (défaut, l'existant) | Nav 236 · Liste 400 · Lecture 1fr | dans le volet Lecture — rien ne change |
| **2 volets** | Nav 236 · Liste 1fr | **plein écran** (écran 03) ; Échap ou « Retour » rend la liste intacte — défilement, pages, sélection |
| **1 volet** | Liste 1fr seule | plein écran, comme en 2 volets ; la **nav vit en tiroir** — bouton dans l'entête, surimpression 236 px à gauche sous scrim, Échap ferme, choisir un dossier ferme |

Ce qui reste vrai partout : `mark_seen` part à l'ouverture (le chemin
`surSelection` est commun, seule la surface de destination change) ;
les raccourcis r/f/e/Suppr agissent sur la sélection courante ; les
gestes sur un écho restent différés et dits (toast) ; la ligne de liste
garde son gabarit à trois rangées — en pleine largeur, l'aperçu gagne
simplement de la place (V-D3).

Objectifs : bascule vécue **au geste** (< 100 ms, la promesse PLAN.md
§1 vaut pour les Réglages aussi) ; **zéro coût au défaut** (mode 3 :
aucun rendu, aucune mesure, aucun octet de différence) ; démarrage,
RAM, P1 inchangés.

## 4. Décisions du Chef Ingénieur

**Verdict du 2026-08-15 : les quatre validées telles que proposées.**

| # | Décision | Proposition (retenue) |
|---|---|---|
| V-D1 | **La sémantique des trois modes.** | Celle du §3 : 2 volets = nav + liste pleine largeur (le patron Gmail/Hey — la nav est le seul accès aux dossiers, la masquer relève du mode 1) ; 1 volet = liste seule + tiroir. Alternatives écartées à la rédaction : « liste + lecture sans nav » (exigerait le tiroir aussi — c'est le mode 1 plus la lecture, un quatrième mode déguisé) ; « lecture SOUS la liste » (Outlook — surface entière neuve, aucun gabarit du Système ne la couvre : chantier à part si le terrain le réclame un jour). |
| V-D2 | **La surface d'ouverture en modes 1 et 2.** | **L'écran 03 étendu au message seul** : sans `thread_id` (et pour un écho), le fil servi est la ligne elle-même — corps par `message_body`/`echo_body`, la grammaire existante de la Lecture. UNE seule surface de lecture plein écran dans le produit ; l'alternative (une Lecture en surimpression plein écran) ferait vivre deux lectures aux règles jumelles — refusée sauf verdict contraire. |
| V-D3 | **La ligne de liste en pleine largeur.** | Le gabarit actuel, étiré : trois rangées, l'aperçu respire. Le gabarit « large » à une rangée (expéditeur · objet · aperçu · heure) est un chantier de virtualisation (les deux gabarits déterministes de P1 sont le socle de la fenêtre) — consigné, pas construit ici. |
| V-D4 | **Où vit la préférence.** | localStorage `wind-volets` (le patron exact de D6 et du thème) : préférence pure UI, le shell n'a rien à en lire — la base est pour ce que le shell consomme (langue, bulles). |

## 5. Les étapes

### E1 — Le socle : la préférence, les modes 3 et 2, l'écran 03 au message seul

**État : livrée le 2026-08-15 (GO CE du jour), au mot du plan — gate
complète (fmt, build ui-v2, contrastes, coherence-systeme, clippy,
tests Rust, e2e dont 6 neufs), amendement A26 + carte « Les modes
d'affichage » + rangée au dessin des Réglages (DC-D2, même commit).
Deux précisions d'implémentation, dites : (1) l'option « Un volet »
n'apparaît PAS dans le sélecteur tant que le tiroir n'existe pas
(E2) — jamais un mode cassé à l'écran ; (2) au retour en trois
volets, la sélection courante rouvre son volet de lecture —
l'écran ne revient pas vide quand une ligne est encore choisie (le
comportement montré par le prototype validé). Terrain CE dû :
bascule vécue au geste, zéro différence au défaut.**

- **`lib/volets.svelte.js`** : `$state` partagé (le patron de
  `texte.svelte.js`), `voletsActuels()` / `appliquerVolets(n)`,
  valeurs `3 | 2 | 1`, défaut 3, toute valeur inconnue retombe sur 3
  (le patron `themeActuel`) ; persistance `localStorage['wind-volets']`
  sous try/catch. Aucune clé héritée à migrer — la préférence n'a
  jamais existé en base ni en stockage.
- **Réglages > Affichage** : rangée « Disposition » — libellé +
  description atténuée, **sélecteur natif** à droite (la grammaire
  exacte de la rangée Langue : 32 px, jetons, clavier et lecteur
  d'écran compris ; un contrôle segmenté serait une forme neuve à
  dessiner — refusé). Options courtes : « Trois volets », « Deux
  volets », « Un volet » ; la description de la rangée porte le sens.
  Application immédiate, sans confirmation — le geste du thème.
  Clés neuves aux DEUX catalogues (l'audit de parité e2e y veille).
- **`App.svelte`** : la grille suit le mode (`colonnes--2` :
  `236px minmax(0,1fr)`) ; en mode 2 la Lecture est **démontée**
  (`{#if}`), les appels `lecture.fermer()` deviennent `lecture?.` ;
  `surSelection` route : mode 3 → `lecture.ouvrir`, mode 2 →
  `conversation.ouvrir` (le `mark_seen` ne bouge pas d'une ligne).
- **`Conversation.svelte`** (V-D2) : sans `thread_id`, le fil est la
  ligne seule — corps `message_body`, écho par `echo_body`, actions de
  la barre inchangées ; « 1 message » est honnête.
- **Système, même commit (DC-D2)** : la rangée Disposition au dessin
  des Réglages ; carte « Les modes d'affichage » à la Boîte de
  réception — géométrie 2 volets, la règle d'ouverture (« en modes 1
  et 2, l'ouverture est l'écran 03 ; Échap rend la liste intacte »),
  la règle du défaut ; amendement au journal (numéro au commit).
- **e2e neufs** : bascule 3 → 2 aux Réglages (le volet Lecture
  disparaît de la grille, la liste s'étire) ; clic sur une ligne →
  plein écran, sujet affiché, Échap → liste intacte (défilement et
  sélection) ; message SANS fil ouvert en mode 2 (le repli V-D2) ;
  persistance (contexte rechargé → mode 2 restauré) ; retour à 3 —
  et la suite écran 02 existante SANS régression, jouée au défaut.

Gate : gate complète (fmt + build ui-v2 + contrastes +
coherence-systeme + clippy + tests Rust + e2e) ; terrain CE — bascule
vécue au geste, ouverture plein écran fluide, **aucune différence
constatée au défaut**.

### E2 — Le mode 1 volet : le tiroir de nav

**État : livrée le 2026-08-15 (terrain E1 validé le même jour), au mot
du plan — gate complète, amendement A27 + carte « Le tiroir de
navigation » + carte des modes complétée (DC-D2, même commit). Police
régénérée 43 → 44 (`menu`), preuve rejouée 45/45 sous CSP. Écarts
d'implémentation, dits : (1) le scrim du tiroir est un BOUTON — le
clic ferme comme au prototype, et le clavier aussi (A8, la grammaire
du produit ne connaissait pas le scrim cliquable) ; (2) le tiroir
reprend la géométrie du prototype validé (268 px, en-tête 60 px avec
tuile de marque + fermer) — la Nav y est montée telle quelle, zéro
fork ; (3) quitter le mode un volet emporte le tiroir. Terrain CE dû :
une journée en mode 1, retour sans séquelle.**

- **Le glyphe d'abord** : `menu` n'est pas dans la police —
  régénération (43 → 44), inventaire tenu, `?v=` bumpé, copie
  `public/`, preuve rejouée par balayage des sources (la leçon du
  terrain 0.1.4 : plus jamais un glyphe de mémoire).
- **L'entête** : en mode 1 seulement, le bouton tiroir à gauche de la
  marque (32 px, la grammaire des boutons d'entête) ; `aria-expanded`,
  libellé au catalogue.
- **Le tiroir** : surimpression 236 px à gauche sous scrim (z-index de
  la famille des surimpressions), la **Nav réutilisée telle quelle** —
  même composant, mêmes props, zéro fork ; choisir un dossier ou un
  compte ferme le tiroir (le geste accompli n'a plus besoin du
  panneau) ; Échap ferme (ordre dans `surTouche` : composition,
  réglages, conversation, tiroir, recherche) ; clic sur le scrim ferme.
- **La grille** : `colonnes--1` : `minmax(0,1fr)` seule ; l'ouverture
  reste celle d'E1 (plein écran).
- **Système, même commit** : géométrie 1 volet et le tiroir dessinés
  (carte dédiée : bouton, surimpression, scrim, règles clavier A8) ;
  amendement au journal.
- **e2e neufs** : mode 1 — nav absente de la grille ; tiroir s'ouvre
  au bouton, un choix de dossier filtre la liste ET ferme ; Échap
  ferme ; le reste du parcours (ouverture plein écran) déjà couvert
  par E1.

Gate : gate complète ; terrain CE — le tiroir au quotidien (une
journée en mode 1), le retour à 3 volets sans séquelle.

## 6. Ce qu'on ne fait PAS (PASSATION §2.6)

- **Pas de largeurs ajustables** (poignées de redimensionnement,
  mémoire des largeurs) : chantier distinct, consigné — le choix ici
  est le NOMBRE de volets, pas leur géométrie fine.
- **Pas de bascule automatique** selon la largeur de la fenêtre : le
  choix est explicite et stable — une interface qui change de forme
  toute seule contredit « Anticiper, puis s'effacer ». Si le terrain
  réclame un mode « auto », il se décidera sur constat.
- **Pas de raccourci clavier de bascule** : la table D3 est figée en
  référence ; un réglage d'installation n'a pas besoin d'une touche.
- **Pas de gabarit de ligne « large »** (V-D3) : la fenêtre P1 vit sur
  les deux gabarits déterministes — consigné pour un chantier de liste.
- **Pas de préférence par compte ni par dossier** : un seul choix,
  global — la grammaire des Réglages actuels.
- **Pas de sort particulier pour ORGANIZED** : le réglage s'applique à
  l'écran 02 (le futur mode Classique). Si le GO d'ORGANIZED vient,
  le sort des volets dans ses surfaces se tranchera à PLAN-ORGANIZED —
  pas ici, pas par avance.

## 7. Ordre de livraison et gates

E1 (le socle — la préférence, 2 volets, l'écran 03 au message seul)
→ E2 (1 volet — le tiroir et son glyphe). Chaque étape : gate
complète avant commit (fmt + build ui-v2 + contrastes +
coherence-systeme + clippy + tests Rust + e2e), `systeme.dc.html`
amendé dans le MÊME commit que tout changement visible (DC-D2),
constat terrain CE avant de déclarer l'étape soldée.

| Étape | Gate |
|---|---|
| E1 | e2e neufs verts (bascule, plein écran, message seul, persistance) + suite écran 02 sans régression au défaut ; DC amendé (rangée Réglages, carte des modes, règle d'ouverture) ; terrain CE : bascule au geste, zéro différence au défaut |
| E2 | e2e neufs verts (tiroir) ; police 44 glyphes prouvée par balayage ; DC amendé (tiroir, géométrie 1 volet) ; terrain CE : une journée en mode 1, retour sans séquelle |

La ligne s'arrête quand une gate casse — c'est elle qui commande.
