# Plan — UI v2 : livrer l'UI du prototype

> Directive du Chef Ingénieur (2026-08-11) : l'UI v1 est le point noir du
> produit ; la refonte livre **exactement l'UI du prototype**
> [`docs/design/ui_prototype.html`](design/ui_prototype.html). Socle
> technique : [ADR 0015](adr/0015-socle-ui-v2-svelte.md) (Svelte 5,
> stratégie A, port de transport, budgets-gates). Méthode : le *shusa*
> ([PASSATION.md](PASSATION.md) §2) — chaque phase est un incrément validé
> au terrain sur de vrais comptes. Ce plan remplace l'ancien plan R0–R6
> (supprimé) ; les acquis de R0 sont repris ci-dessous, pas rejoués.

## 0. Le pivot — ce qui change, ce qui tient

**Ce qui change.** Le ruling R0-S2 (« le prototype est illustratif, seul
le Système lie ») est **inversé** par la directive du 2026-08-11 : le
prototype devient la **cible normative** de la refonte. Le Système
Clarity reste la référence là où le prototype est **muet** (avis et
progression A4, corps HTML en thème sombre A1, inventaire d'icônes A3,
contrastes) ; quand les deux divergent, **le prototype l'emporte**.
Premier commit du chantier : inscrire cette inversion au journal du
Système (**A6**), daté, motivé — deux documents normatifs qui se
contredisent en silence sont un défaut.

Conséquence concrète, nommée : la ligne de liste du prototype porte des
**puces de 32 px** (fil, fichiers) — la hauteur de ligne n'est plus fixe.
Le fait terrain derrière A2 (8/120 lignes débordaient de 104 px avec
puces ; 0/120 sans) ne disparaît pas : il devient un **repli documenté**
(§5, P1) si le gate perf ou le terrain casse la ligne du prototype.

**Ce qui tient — les acquis de R0, réutilisés tels quels :**

| Acquis | Ce qu'on reprend |
|---|---|
| S1 — frontière du volet de lecture | iframe `sandbox` + CSP par message, gouttière 20 px, encre bakée par thème, corps HTML sur surface claire ([`spikes/volet-lecture/`](../spikes/volet-lecture/README.md)) |
| S3 — icônes vendorisées | [`assets/icones/`](../assets/icones/README.md), 15,1 Kio, CSP `font-src 'self'`. Vérifié ce jour : les **30 glyphes du prototype sont tous couverts** par l'inventaire |
| S4 — avis et progression | la règle des 3 régions (Système A4) reste la spéc de l'hors-prototype (§6) |
| S5 — port de transport | `appel(commande, args) → Promise`, impl en-processus prouvée par 21 e2e ([`transport.js`](../apps/desktop/ui/transport.js)) |
| S6 — hooks de test | `data-testid` sur le markup généré + contrat « v2 préserve » ([`e2e/README.md`](../e2e/README.md)) |
| S2 — ligne 104 px | **rétrogradé en repli** : éprouvé au terrain, dégainé seulement sur mesure (andon P1) |

**Périmètre.** Ce que le prototype ne montre pas n'est **pas sur le
chemin critique**. Les capacités v1 absentes du prototype sont
inventoriées en **Annexe A** et raccordées plus tard — sauf les **dus de
bascule** (§6), sans lesquels v2 ne peut pas remplacer v1.

## 1. Doctrine — sept règles

1. **Le prototype est le contrat**, au pixel et au geste : jetons,
   géométries, états, enchaînements d'écrans. On ne « s'inspire » pas, on
   reproduit.
2. **Le Système complète, ne contredit plus.** Là où le prototype est
   muet, la règle Clarity s'applique ; tout conflit se tranche prototype
   et s'inscrit au journal du Système.
3. **Les invariants ne sont pas de l'UI** (§2). « Exactement l'UI » =
   exactement les pixels et les gestes — pas naïvement le DOM du
   prototype : le corps d'un mail vit dans l'iframe sandbox, pas en
   paragraphes libres.
4. **Les données réelles remplacent la fiction.** Comptes réels au lieu
   de « Travail / Personnel », compteurs réels, heures réelles, état de
   synchro réel. La fiction du prototype n'est jamais codée.
5. **Ce qui est inerte au prototype reste inerte** tant que la capacité
   n'existe pas : barre de format, « Rendre indépendante », Cc/Cci,
   « Joindre ». Pas de fantômes (PASSATION §2.6) — le prototype lui-même
   répond « à venir » sur Joindre.
6. **v1 reste l'UI expédiée jusqu'à la bascule** (strangler-fig). v2
   grandit dans `apps/desktop/ui-v2/`, câblée aux mêmes commandes par le
   même port. À tout instant, l'app est livrable.
7. **La parité se prouve, ne se déclare pas** : jeu d'essai du prototype
   dans une vraie base + banc côte à côte (§4), puis terrain du Chef
   Ingénieur sur ses vrais comptes.

## 2. Invariants qui ne cassent à AUCUNE phase

1. **`mail-core` intouché** (ADR 0001) : l'UI affiche un état, émet des
   intentions via le port de transport. Ajouter une commande de *lecture*
   côté shell est permis ; mettre de l'UI dans le cœur ne l'est pas.
2. **Sécurité du rendu** : corps de mail dans l'**iframe sandbox + CSP**,
   images distantes bloquées, `textContent` jamais `innerHTML`. Clarity
   habille le chrome, pas le HTML de l'expéditeur.
3. **Credentials au coffre de l'OS** ; **boîte d'envoi** aux deux règles
   d'or (ADR 0003) : jamais d'envoi perdu, jamais d'envoi fantôme.
4. **Identité message = (account_id, boîte, uid)** jusque dans la
   sélection UI.
5. **Non-lu par la graisse et l'encre, jamais par une pastille** — le
   prototype le respecte (expéditeur 600, encre pleine) ; on le garde.

## 3. Budgets = gates bloquants (andon)

Re-mesurés à chaque phase qui touche le rendu, sur la **base réelle**
(256 312 messages) :

| Métrique | Cible |
|---|---|
| Démarrage à froid | < 1 s |
| Ouverture d'un message | < 50 ms |
| Page de liste | < 100 ms |
| RAM (working set privé, 7 procédés) | < 200 Mo |

Un budget dépassé = **on arrête la ligne**. Le spike ADR 0015 les a tenus
en synthétique (p95 ≤ 29 ms même à CPU ×6) ; chaque phase les re-prouve
sur le vrai noyau, via `mesure.mjs` adapté à v2.

## 4. Le contrat visuel exécutable

**Pas de re-spéc papier** : le prototype est la spéc, il s'ouvre dans un
navigateur et se manipule. Ce qu'on en extrait mécaniquement (une fois,
en P1) :

- **`systeme.css`** : les 14 rôles de jetons × **7 thèmes** — valeurs
  **verbatim** du template du prototype (`bg, panel, surface, ink, ink2,
  muted, border, accent, accentH, sel, alert, onAccent, shadow, scrim` ;
  thèmes `air, feu, eau, astres, terre, nature, nuit`). Écrit une fois,
  agnostique au framework (ADR 0015). Le prototype a 7 thèmes, le
  Système 9 : on livre 7 (D6 pour la suite).
- **Les géométries du prototype** telles quelles : grille
  `236 / 400 / 1fr`, entête 60 px, pied d'onglets 52 px, barre de statut
  36 px, contrôles 32 px, rangées de nav 36 px, rangées d'en-tête de
  composition 44 px, modales 860 / 560 px, rayons 10 / 6, signature
  (surface + filet accent 2 px + ombre), échelle typo 12/13/15/18/24/40.
- **Le jeu d'essai « Clarity »** : les 10 conversations fictives du
  prototype (contrat Vantis, planning semaine 33…) seedées dans une base
  jetable (variante de
  [`seed_inbox.rs`](../crates/mail-core/examples/seed_inbox.rs)). v2 sur
  ce décor, côte à côte avec le prototype, mêmes états → la parité se
  juge d'un regard et se capture. C'est le banc de chaque revue de phase.
- Le **runtime du prototype n'est pas repris** (player React embarqué —
  ADR 0015 l'avait déjà écarté) : ses gabarits `sc-if`/`sc-for`
  deviennent des composants Svelte ; sa police pleine de 5,3 Mo est
  remplacée par le sous-ensemble vendorisé de 15,1 Kio.

## 5. Phases

Chaque phase se clôt par : parité visuelle sur le banc (§4), budgets
re-mesurés si le rendu a bougé, terrain du Chef Ingénieur, GO/NO-GO.

### P1 — Gate perf : la ligne du prototype à 256 312 réels — ✓ CLOS (2026-08-11, GO Chef Ingénieur)

**Objectif :** prouver que la ligne **exacte** du prototype — hauteur
variable, puces comprises — tient les budgets sur la vraie base, via le
vrai IPC. C'est LE point dur né du pivot ; il se règle avant tout écran
(front-loading, PASSATION §2.2).

**Livré :** projet Svelte 5 + Vite dans `apps/desktop/ui-v2/` (lancement
dev sans toucher l'UI expédiée ; CI et gate pre-push étendus) ; port de
transport importé tel quel ; `systeme.css` (14 × 7, verbatim) ; liste
fenêtrée avec la ligne du prototype ; bascule de thème à chaud ;
`mesure.mjs` adapté.

**Point d'ingénierie nommé :** la hauteur de ligne est **déterministe
depuis les données** — deux gabarits (sans puces / avec puces, puces si
fil > 1 ou fichiers > 0) → offsets calculables en O(1), pas de mesure
par ligne au défilement.

**Validation :** démarrage, page p95, RAM, bascule de thème sur la base
réelle. **Andon :** si un budget saute, repli documenté = ligne A2
(104 px fixe, marqueurs en ligne 1 — éprouvée 0/120 au terrain) ;
décision du Chef Ingénieur **sur les chiffres**, pas d'avis.

**Refus :** pas de nav ni de volet complets — juste la liste et une
lecture minimale, de quoi mesurer.

**Résultats (2026-08-11, banc `e2e/mesure-v2.mjs` + `diag-v2.mjs`,
256 312 messages seedés = 205 050 conversations, machine ARM64) :**

| Poste | Cible | v2 | v1 iso-décor | Verdict |
|---|---|---|---|---|
| Rendu de page (Svelte, pages servies) | <100 ms | méd. 0,4 ms · max 2,1 ms | — | **VERT** |
| Défilement proche (fenêtre à fenêtre) | <100 ms | méd. 0,6 ms · p95 1,4 ms | — | **VERT** |
| Saut profond aléatoire (service + rendu) | <100 ms | p95 307,6 ms | même commande : 240,8 ms dès la 1ʳᵉ page à froid | **ANDON — étage cœur** |
| Bascule de thème | — | p50 0,2 · p95 0,4 ms | — | **VERT** |
| Ouverture d'un message | <50 ms | p95 8,2 ms | — | **VERT** |
| RAM au repos (ADR 0002) | <200 Mo | **110,9 Mo** / 7 proc. | 115,8 Mo / 7 proc. | **VERT** — v2 plus légère que v1 |
| Démarrage à froid | <1 s | 1 386–1 547 ms (mur) | 1 203 ms (interne) | **ANDON partagé** — voir caveat |

Gabarits mesurés de la ligne prototype : **101 / 132 px**. La ligne du
prototype est **innocentée** : le décompose (`diag-v2.mjs`) montre que le
saut profond paie la requête cœur (`elapsed_us` : 10,5 ms à l'offset 0 →
252,6 ms à 200 000, linéaire — l'`OFFSET` exécute la triple jointure ET
l'`EXISTS` corrélé sur `attachments` pour chaque ligne sautée), l'IPC
~3 ms, le rendu Svelte 0,4 ms. C'est la dette nommée « défilement
profond » des reports assumés (PASSATION §8), identique en v1.

**Caveats nommés :** base du banc dans `target/e2e` (OneDrive — la
synchro peut gonfler démarrage et première page à froid, l'avertissement
de `mesure.mjs` vaut aussi ici) ; démarrage v1 mesuré par son horloge
interne, v2 au mur — même ordre de grandeur, pas la même règle. Les deux
se re-mesurent au terrain sur la vraie base, hors OneDrive.

**Arbitrage Chef Ingénieur (andon) :**
1. **Saut profond — TRANCHÉ ✓ (2026-08-11) : correctif cœur appliqué**
   (commit `135cd49`). Pagination en sous-requête sur `threads` seul,
   portée par l'index partiel `idx_threads_date_globale` ; jointures et
   `EXISTS` sur les seules lignes retenues. Contre-preuve, même banc :
   cœur **9,0 → 14,6 ms** de l'offset 0 à 200 000 (avant : 10,5 →
   252,6 — courbe plate, ×17 au plus profond) ; **page p95 307,6 →
   38,4 ms**, max 50,3 ms. Budget < 100 ms **tenu, sauts profonds
   compris**. Sémantique tenue par les 224 tests du crate ; profite
   aussi à v1. La dette « défilement profond » des reports assumés est
   soldée. Kaizen noté, non dû : ~9 ms résiduels par appel =
   `Store::open` + `COUNT(*)` — à ne toucher que si un budget le
   demande.
2. **Démarrage — TRANCHÉ ✓ (2026-08-11, terrain sur copie de la vraie
   base).** La re-mesure a montré une SECONDE dette cœur, invisible en
   synthétique : `orphans()` énumérait à chaque `Store::open` les
   247 835 enveloppes hors portée (`thread_id` NULL pour toujours,
   ADR 0010 §3) — ~400 ms par commande. Corrigé sur GO du Chef
   Ingénieur (`0acbe0b`) : le balayage est piloté par les boîtes en
   portée (3 229 enveloppes). `Store::open` 428 → **3,3 ms** ; première
   page sur la vraie base 1 944 (froid) / 389 (chaud) → **51,8 ms**.
   Démarrage au mur : **1 167 ms, dont ~52 ms d'application** — la
   masse restante est le spawn + l'init WebView2, partagée avec v1,
   gonflée par le port CDP du banc, **hors du périmètre du front**.
   Statut : les budgets imputables à l'UI sont tous verts sur la vraie
   base ; l'écart résiduel du démarrage est une piste séparée
   (coquille/WebView2), à instruire hors refonte si le terrain le
   réclame. Diagnostic conservé : `diagnostic_ouverture`.

**Notes portées à P2 :** aperçu de liste et compte de fichiers par fil
absents de `MessageRow` (le port doit les exposer pour la ligne et les
puces complètes) ; éviction LRU des pages en mémoire (longue session :
la rétention se voit — 270 Mo après 300 sauts + 20 iframes) ; a11y
clavier de la ligne ; le renderer de l'iframe sandbox est un 8ᵉ processus
quand un message est ouvert — la méthodologie RAM le comptera.

### P2 — Boîte de réception (écran 02), câblée au réel

**Objectif :** l'écran principal, pixel-exact, sur les 4 comptes réels.

**Livré :**
- **Entête 60 px** : titre, champ de recherche **visuel** (le câblage
  est D1), « Écrire », « Réglages ».
- **Nav 236 px** : les 6 dossiers (réception, envoyés, brouillons,
  indésirables, archives, corbeille) avec compteurs réels — héros non-lu
  sur réception et indésirables, compteur simple ailleurs ; section
  **Boîtes** = « Toutes les boîtes » (unifiée, existante) + **une entrée
  par compte réel** — la fiction « Travail / Personnel » disparaît
  (icônes : D7). Tâche genchi genbutsu : relever le mapping réel des six
  dossiers canoniques par fournisseur (`list_folders`, `sent_mailbox`)
  avant de coder la nav.
- **Liste 400 px** : lignes P1, états du prototype (non-lu par
  graisse/encre, survol `--sel`, sélection = signature), vides (« Aucun
  message ici. ») ; **onglets** Tous / Non lus / Brouillons (ce dernier
  ouvre le dossier, comme au prototype).
- **Volet de lecture** : carte signature, titre 24 px tronqué, puce méta
  + « Dernier message · … » + « Voir la conversation » ; auteur + méta ;
  **corps = iframe sandbox** dans la zone corps de la carte (S1 :
  gouttière 20 px, encre bakée, HTML sur surface claire) ; barre des
  4 actions **réelles** (Répondre/Transférer → P4 ; Archiver, Supprimer
  câblés) + toasts du prototype ; sélection → `mark_seen` réel.
- **Barre de statut 36 px** : état de synchro réel (sondage existant),
  formulations du prototype.
- `data-testid` neufs + contrat « v2 préserve » (S6) ; e2e du périmètre
  réécrits.

**Validation :** banc Clarity (états canoniques : repos / non-lu /
survol / sélection, chaque dossier, chaque onglet, écran principal dans
les 7 thèmes) ; terrain 4 comptes ; budgets.

**Refus :** conversation, composition, réglages, onboarding, recherche
câblée.

### P3 — Conversation (écran 03)

**Objectif :** le fil plein écran du prototype — c'est un **écran**, pas
un volet : entête propre (« ← Boîte de réception », « Écrire »), carte
unique signature.

**État : ✓ clos** (GO du Chef Ingénieur à l'ouverture de P4). Écarts
assumés et dits : hauteur du corps bornée (le bac à sable opaque
interdit de mesurer le contenu — relâcher le sandbox serait un troc
refusé) ; « À » approximé par la règle du prototype (les destinataires
ne sont pas stockés par le cœur).

**Livré :** repli/dépli par message (dernier déplié par défaut), « Tout
déplier », puces fil/fichiers, bloc De/À/Objet, **corps par message
déplié = une iframe sandbox par message**, section « Fichiers joints »
(puces visuelles ; l'ouverture réelle est un dû de bascule), barre des
4 actions.

**Point d'ingénierie nommé :** N iframes sur « Tout déplier » d'un fil
de 20+ — montage paresseux, mesuré au budget d'ouverture.

**Validation :** terrain sur les vrais fils (577 fils de 2–5, le fil de
20+) ; budgets ; banc Clarity (replié / déplié / tout déplié).

**Refus :** rien de neuf côté cœur (ADR 0008/0009/0010 suffisent).

### P4 — Composition, Réglages, Onboarding (écrans 04, réglages, 01)

**Objectif :** les surfaces restantes du prototype, câblées aux flux
réels.

**État : livré, en validation terrain.** 17 parcours e2e verts (14 sur
le décor Clarity + 3 écran 01 sur base vierge) ; paires de parité
onboarding / composition / réglages / nuit au banc. Écarts assumés et
dits : la ligne « De » montre l'adresse seule (le cœur ne stocke ni nom
d'affichage ni étiquette de compte) ; le toast « Message envoyé. » est
celui du prototype et confirme la REMISE à la boîte d'envoi — l'incident
d'envoi visible est la fente d'avis (P5) ; la citation de réponse est
réelle (le prototype s'arrêtait à l'amorce) ; un brouillon local
n'apparaît dans le dossier Brouillons qu'au retour du reflet serveur
(comportement v1 conservé).

**Livré :**
- **Composition** (surimpression 860 px) : modes nouveau / répondre /
  transférer avec les préremplissages exacts du prototype (Re :/Tr :,
  amorce, fichiers du dernier message en puces) ; **envoi réel par la
  boîte d'envoi** (règles d'or) + toast « Message envoyé. » ;
  « Enregistrer le brouillon » → `save_draft` + toast ; l'**autosave v1
  est conservé** sous le bouton (le conflit d'édition
  `composeDraftEpoch` reste couvert). Inertes comme au prototype : barre
  G/I/S/Liste/Lien/Citation, « Rendre indépendante », Cc/Cci,
  « Joindre » (même réponse que le prototype : « Sélecteur de fichiers —
  à venir. »).
- **Réglages** (surimpression 560 px) : les 7 thèmes, pastilles ×5,
  coche sur l'actif, « Terminé » ; persistance
  `localStorage['discovery-theme']`, défaut `nature` (comportement
  prototype — l'OS sombre automatique est en D6).
- **Onboarding** (écran 01) : affiché à zéro compte ; « Continuer »
  branche les flux d'ajout existants (D4).
- **Toast** générique (bas centré, signature).

**Validation :** terrain — un vrai envoi, un vrai brouillon, bascule des
7 thèmes sur de vrais mails (lisibilité jugée, contraste ≥ 4,5:1
re-vérifié, thème `nuit` en premier) ; banc Clarity.

### P5 — Dus de bascule, décisions soldées, cutover

**Objectif :** v2 devient l'UI expédiée — sans régression sur ce qui
protège l'utilisateur.

**Livré :**
- Les **dus de bascule** (§6), en style Clarity spécifié par le Système
  (A4) là où le prototype est muet.
- Les décisions D1–D5 **tranchées et câblées ou écrites** (un report =
  une ligne motivée, PASSATION §2.6).
- **Parité poste à poste** sur l'Annexe A, signée par le Chef Ingénieur :
  chaque capacité v1 = câblée, due plus tard, ou coupée en connaissance.
- E2E : les 21 parcours équivalents sur v2 + les nouveaux (thèmes,
  conversation, onglets), CDP réel (ADR 0005).

**Gate de bascule :** budgets sur base réelle + 4 comptes + **deux
semaines sans défaut critique** → `apps/desktop/ui/` retiré, e2e
entièrement sur v2, revue de clôture `docs/PHASE-REFONTE.md` (livré vs
plan, budgets, enseignements, reports, GO/NO-GO).

## 6. Dus de bascule — hors prototype, mais sans eux v2 ne s'expédie pas

Le prototype ne montre ni incident ni attente ; l'application réelle en
a. Couper ces capacités à la bascule serait une régression de sécurité ou
de correction, pas de confort :

| Dû | Pourquoi c'est dû |
|---|---|
| **Fente d'avis** (haut, au plus UN avis, priorité : échec d'envoi > mise à jour > crash > télémétrie > brouillons) | l'**échec d'envoi doit se voir** — corollaire UI des règles d'or ; porte aussi mise à jour signée (ADR 0013) et consentement télémétrie (ADR 0014) |
| **Ligne de progression** (bas, au plus UNE : synchro OU rattrapage, `--muted`) | remplace 2 bandeaux v1 ; l'attente non fautive de la boîte d'envoi vit ici |
| **Modale de migration** (exclusive, bloquante au démarrage) | ADR 0012 — sans elle, la première commande paierait l'adoption d'une base héritée dans un gel d'interface |
| **Afficher les images** (opt-in par message, dans le volet) | les images distantes restent bloquées par défaut (invariant) ; sans le bouton, les mails riches sont illisibles sans recours |
| **Pièces jointes : ouvrir / enregistrer** (`message_attachments`, `save_attachment`) | valeur quotidienne déjà acquise en v1 ; les puces du prototype deviennent cliquables, visuel inchangé |

Ces cinq-là héritent du style Clarity via le Système (signature pour les
avis, atténué pour la progression) — invention minimale, jamais
d'empilement.

## 7. Décisions ouvertes au Chef Ingénieur

Aucune ne bloque P1. Chacune se pose à l'ouverture de sa phase.

| # | Phase | Décision | Recommandation |
|---|---|---|---|
| D1 | P5 | **Recherche** (FTS5, v1 l'a ; le prototype montre le champ, pas les résultats) : câblée à la bascule ? | Oui — résultats rendus **aux lignes mêmes du prototype** dans le volet liste ; aucune UI neuve |
| D2 | P5 | **Étoile + Déplacer** (v1 les a ; barre = 4 actions max au Système) | Trancher à l'usage réel pendant P2–P5 : coupés si non utilisés, sinon logés « dans le message » (spéc courte au Système) |
| D3 | P5 | **Raccourcis clavier** (r/f/e/v/s/c/Suppr/`/`) repris ? | Oui — gratuits, v1 les a, zéro pixel |
| D4 | P4 | **Onboarding** : porte simple vers les 3 dialogues existants, ou vraie auto-détection de serveur ? | Porte simple ; l'auto-détection est une capacité neuve, à instruire séparément |
| D5 | P5 | **Bouton « Synchroniser »** manuel (v1) | Coupé — synchro automatique + ligne de progression + barre de statut suffisent |
| D6 | après bascule | Thèmes « Le vent » et « Tournesol » (Système : 9, prototype : 7) ; OS sombre → `nuit` automatique | Après bascule — deux jeux de jetons et une media query, aucun impact de structure |
| D7 | P2 | **Icônes des comptes réels** (la fiction `work`/`person` disparaît) | `person` par défaut pour tous, libellé = adresse du compte ; personnalisation plus tard |

## Annexe A — Inventaire de raccordement (v1 présent, prototype muet)

La liste opposable pour la parité poste à poste de P5. « Appui » = ce qui
existe déjà (commandes du port, UI v1).

| Capacité v1 | Appui existant | Destin |
|---|---|---|
| Recherche plein texte (≥ 3 car., debounce, résultats) | `search_messages`, ADR 0004 | **D1** |
| Ajout de comptes Gmail / Microsoft / IMAP | 3 dialogues v1, `add_account`, `add_microsoft_account`, `add_generic_account` | derrière l'onboarding — **D4** |
| Étoile | `mark_flagged` | **D2** |
| Déplacer vers… | `move_message`, `list_folders`, dialogue v1 | **D2** |
| Afficher les images (opt-in) | v1, garde d'images distantes | **dû de bascule** (§6) |
| Pièces jointes : ouvrir / enregistrer | `message_attachments`, `save_attachment` | **dû de bascule** (§6) |
| 7 bandeaux (envoi, brouillons, màj, synchro, rattrapage, télémétrie, crash) | v1 + ADR 0012/0013/0014 | **dû de bascule** (§6) — refondus en 3 régions A4 |
| Migration visible et interruptible | modale v1, ADR 0012 | **dû de bascule** (§6) |
| Raccourcis clavier | v1 | **D3** |
| Synchronisation manuelle (bouton) | v1, `sync_inbox` | **D5** |
| Autosave brouillon + conflit d'édition | v1, `composeDraftEpoch` | conservé en **P4**, sous le bouton du prototype |
| Multi-fenêtre composition (« Rendre indépendante ») | n'existe pas en v1 | reporté ; affordance inerte, comme au prototype |
| Composition HTML riche (barre G/I/S/…) | n'existe pas en v1 | reportée ; barre inerte, comme au prototype — l'activer rouvre l'envoi HTML, décision séparée |
| Envoi de pièces jointes (« Joindre ») | capacité cœur absente (report assumé, PASSATION §8) | reporté ; même toast que le prototype |
| Cc / Cci réels | v1 ne les expose pas | reportés ; puces inertes, comme au prototype |
| Compteur de perf (`#perf`) | outillage v1 | mode dev seulement, hors UI expédiée |

---

Rien ne se code tant que P1 n'a pas ses chiffres ; rien ne s'expédie tant
que le Chef Ingénieur n'a pas signé au terrain. La ligne s'arrête quand
un budget casse — c'est elle qui commande.
