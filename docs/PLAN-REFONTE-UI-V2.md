# Plan de phase — refonte UI v2 (« Douceur »)

> Plan **prospectif** d'exécution. Source de la décision de socle :
> [ADR 0015](adr/0015-socle-ui-v2-svelte.md). Méthode : le *shusa*
> ([PASSATION.md](PASSATION.md) §2). Chaque phase est un **incrément validé
> sur un vrai compte** — un incrément non validé au terrain n'est pas livré
> (§2.5). Rien n'est engagé tant que le Chef Ingénieur n'a pas dit go.

## Principe directeur — strangler-fig, jamais big-bang

L'app v1 (`apps/desktop/ui/`, JS vanilla, 1 638 lignes) **fonctionne** et est
validée sur 256 312 vrais messages. On ne la réécrit pas d'un bloc. On
construit v2 **en parallèle** (`apps/desktop/ui-v2/`, Svelte), câblée aux
**mêmes commandes Tauri**, et on la fait grandir écran par écran. **v1 reste
l'UI expédiée jusqu'à la bascule** (R6), qui n'a lieu qu'à parité
fonctionnelle + budgets tenus + signature terrain. À tout instant, l'app est
livrable.

## Invariants qui ne cassent à AUCUNE phase

Faciles à casser en silence (§6 PASSATION). À vérifier à chaque revue :

1. **`mail-core` intouché** (ADR 0001) : la refonte est UI seule. L'UI affiche
   un état, émet des intentions — via le **port de transport** (ADR 0015 §4).
2. **Sécurité du rendu** : corps de mail dans l'**iframe sandbox + CSP**,
   images distantes bloquées, `textContent` jamais `innerHTML`. La « Douceur »
   habille le chrome, pas le HTML de l'expéditeur.
3. **Credentials au coffre de l'OS** ; **boîte d'envoi** aux deux règles d'or
   (ADR 0003) : jamais d'envoi perdu, jamais d'envoi fantôme.
4. **Identité message = (account_id, boîte, uid)** jusque dans la sélection UI.
5. **Non-lu par la graisse et l'encre, jamais par une pastille colorée.**

## Budgets = gates bloquants (andon)

Re-mesurés à chaque phase qui touche le rendu, sur la **base réelle** :

| Métrique | Cible |
|---|---|
| Démarrage à froid | < 1 s |
| Ouverture d'un message | < 50 ms |
| Page de liste | < 100 ms |
| RAM (working set privé, 7 procédés) | < 200 Mo |

Un budget dépassé = **on arrête la ligne**. Le spike les a tenus en
synthétique (page p95 ≤ 29 ms même à CPU ×6) ; **chaque phase les re-prouve
sur le vrai noyau**.

---

## R0 — Front-loading : régler les points durs AVANT les écrans

Aucun écran de production. On sort des **décisions écrites** ou des **chiffres**
(§2.2). Rien ne se code tant que R0 n'est pas soldé.

- **S1 — Frontière du volet de lecture.** Micro-spike : injecter la typo du
  Système dans le `srcdoc` de l'iframe sur un vrai mail. Confirmer que le corps
  HTML **reste dans le bac à sable** et que la Douceur habille le chrome +
  le texte brut, sans jamais surcharger les styles de l'expéditeur.
- **S2 — Ligne à hauteur fixe.** Vérifier sur contenu réel que la ligne 104 px
  tient (objets/aperçus réels tronqués, puces logées). L'ADR 0015 a tranché
  *fixe* ; ceci le prouve au terrain. Sinon : virtualisation mesurée (coût
  **égal aux trois familles**, sans effet sur le socle).
- **S3 — Icônes vendorisées.** Sous-ensemble Material Symbols Rounded (34
  glyphes du Système), **local** (offline + CSP interdisent le CDN). Mesurer
  le poids.
- **S4 — Règle de relogement des bandeaux.** Une **région système unique,
  priorisée** (au plus un bandeau visible), cohérente avec la signature,
  **hors de tout `<header>`** + garde `#id[hidden]{display:none}`. Doit
  survivre aux 7 bandeaux conditionnels (outbox, brouillons, MAJ, synchro,
  rattrapage, télémétrie, crash) + la modale de migration.
- **S5 — Port de transport.** Définir l'interface UI↔cœur ; livrer l'impl
  **en-processus** (Tauri IPC). L'impl **distante** (HTTP/WS) est esquissée,
  pas due ici.
- **S6 — Hooks de test. ✓ FAIT** (2026-08-10). Le markup **généré** par
  `app.js` (lignes, résultats, fil, pièces jointes, puces, boutons de
  listes) porte des **`data-testid`** stables, et le gate E2E les
  sélectionne ainsi (e2e 21/21 à chaque incrément). **GO arbitré :** les IDs
  sémantiques de haut niveau (`#compose`, `#detail-subject`, dialogues,
  bandeaux) et les classes d'état (`flagged`, `current`) restent des
  sélecteurs directs — un **contrat « v2 préserve »**, pas un défaut ; les
  migrer serait du churn sans gain. Contrat complet et opposable dans
  [`e2e/README.md`](../e2e/README.md).

**GO/NO-GO :** chaque point a sa décision ou son chiffre. Budgets non touchés.
**S6 est clos** ; S1–S5 restent à jouer avant R1.

## R1 — Socle de jetons sur v1 (le socle invisible, prouvé à bas coût)

**Objectif :** valider l'actif le plus détaillé du Système — les **9 palettes**,
la **bascule à chaud**, la **persistance** — sur l'app RÉELLE, sans changer ni
layout ni framework.

**Livré :** `systeme.css` (14 rôles × 9 palettes) ; migration de **toute**
couleur en dur de v1 vers jetons ; modale Réglages (sélecteur 9 thèmes) ;
persistance `localStorage['discovery-theme']`, restaurée au montage (repli
`nature`), OS sombre → `nuit`.

**Validation :** terrain — l'utilisateur bascule les thèmes sur ses vrais
mails, lisibilité jugée ; contraste ≥ 4,5:1 re-vérifié ; E2E : un test de
bascule.

**Gate :** repeinture pure → budgets inchangés, zéro régression.

**Valeur :** livre le **mode sombre + thèmes** à l'utilisateur tout de suite.
Le `systeme.css` est **réutilisé verbatim par v2** (les jetons sont du CSS,
agnostiques au framework) — seule la migration des sélecteurs v1 est jetable.

**Refus :** pas de layout, pas de framework ici.

## R2 — Socle Svelte + port de transport, câblé au vrai cœur

**Objectif :** prouver que Svelte parle à `mail-core` par le port de transport
et **tient les budgets sur 256 k RÉELS** — le spike l'a prouvé en synthétique ;
ici c'est le vrai IPC + le vrai noyau. **C'est LE gate perf de la refonte.**

**Livré :** projet Svelte/Vite dans `apps/desktop/ui-v2/` ; le shell boote, lit
l'état du cœur via le port, affiche la **liste virtualisée réelle** + une
lecture minimale ; `systeme.css` de R1 importé tel quel.

**Validation :** terrain — démarrage, page, ouverture, RAM mesurés sur la vraie
base (`mesure.mjs` adapté à v2). **Andon si un budget saute.**

**Refus :** pas encore les 3 colonnes complètes — juste de quoi mesurer.

## R3 — Boîte de réception 3 colonnes (le cœur de l'app)

**Objectif :** l'écran principal au Système, avec le **vrai modèle d'état**.

**Livré :**
- Colonne nav : dossiers + **comptes réels** + boîte unifiée. **Réconciliation
  du modèle** : « Toutes les boîtes » = unifiée ; chaque boîte = un compte
  (email réel) — la fiction « Travail / Personnel » du proto n'existe pas.
- Liste virtualisée : ligne riche du Système ; états (non-lu par graisse/encre) ;
  survol `--sel` ; sélectionné = signature.
- Volet de lecture : **iframe sandbox conservée**, chrome + méta + barre
  d'actions au Système, signature.
- **Bandeaux relogés** selon S4. **Recherche réelle** (FTS5, ≥ 3 caractères,
  debounce — reprend les gardes de l'ADR 0004).

**Validation :** terrain sur les **4 comptes réels** ; **parité fonctionnelle
avec v1** vérifiée poste par poste ; E2E réécrits sur `data-testid`.

**Décisions Chef Ingénieur (voir liste finale) :** Étoile + Déplacer
conservés ou coupés ? Raccourcis clavier repris ?

**Refus :** composition (R5) et conversation (R4) hors de cette phase.

## R4 — Conversation (fil)

**Objectif :** lire un fil multi-messages, replier/déplier, signature sur le
message actif.

**Livré :** vue conversation ; dernier message déplié par défaut ; « Tout
déplier » ; barre d'actions identique au volet de lecture.

**Validation :** terrain sur de vrais fils (les 577 fils de 2–5, le fil de +20).

**Refus :** rien de neuf côté cœur — le regroupement existe déjà (ADR
0008/0009/0010).

## R5 — Composition (modale)

**Objectif :** rédiger / répondre / transférer au Système.

**Livré :** modale 860 px ; De/À/Objet ; corps ; pré-remplissage
réponse/transfert (Re/Tr, amorce, pièces jointes du dernier message) ; envoi
par la **boîte d'envoi** (règles d'or, ADR 0003) ; toasts ; autosave brouillon
(conflit d'édition existant, `composeDraftEpoch`).

**Décisions Chef Ingénieur — refus de périmètre explicites :**
- **Barre de format (G/I/S/liste/lien/citation) = composition HTML riche**, une
  **capacité NEUVE** hors périmètre v1. Par défaut : **on reste texte brut** et
  on masque la barre de format. La compo riche = décision séparée (elle rouvre
  le chemin d'envoi HTML).
- **« Rendre indépendante »** (compo multi-fenêtre Tauri) → **reporté**.
- **« Joindre »** alors que l'envoi de pièces jointes est en lecture seule v1
  (report assumé, PASSATION §8) → affordance désactivée/différée.

**Validation :** terrain — un vrai envoi, un vrai brouillon, sur un vrai compte.

## R6 — Onboarding (porte d'entrée) + bascule (cutover)

**Objectif :** la porte calme du Système, branchée sur les vrais flux ; puis
basculer v2 comme UI expédiée.

**Livré :** champ unique « Votre adresse » → branchement Gmail OAuth /
Microsoft (adresse manuelle, l'API ne la donne pas) / IMAP (host/port/mdp —
dialogues v1 **conservés** derrière la porte).

**Décision Chef Ingénieur :** ampleur de l'**auto-détection** — simple porte
devant les 3 dialogues existants, ou vraie détection de serveur ?

**Bascule (cutover) :** v2 atteint **parité fonctionnelle + budgets + signature
terrain sur les 4 comptes** → v2 remplace v1 ; `apps/desktop/ui/` retiré ; E2E
entièrement sur v2. **Gate de bascule = deux semaines sans défaut critique**
(miroir du gate 5).

**Revue de clôture :** `docs/PHASE-REFONTE.md` (livré vs plan, budgets
re-mesurés, enseignements, reports assumés, GO/NO-GO).

---

## Pistes séparées, gated — PAS sur le chemin critique du desktop v2

Le socle (ADR 0015, Stratégie A) rend ces cibles **atteignables** ; elles ne
sont pas *dues* avec le desktop v2. Chacune a ses prérequis.

- **Web (Phase 4 produit)** : impl **distante** du port de transport +
  `mail-core` côté serveur. Le **même** front Svelte s'y exécute → quasi
  gratuit côté UI ; le coût est le serveur.
- **Mobile (iOS / Android)** — prérequis DUS, chacun un jalon :
  1. **Déclinaison compacte/tactile du Système** (design dû) : colonne unique,
     tiroir de nav, cibles ≥ 44 px, gestes. Les jetons/typo/signature sont
     portables ; les **layouts 3 colonnes ne le sont pas**.
  2. **ADR mobile-storage** : la synchro intégrale (base ~13 Go, ADR 0010) est
     intenable sur téléphone → synchro fenêtrée / à la demande. **Décision de
     cœur, pas d'UI.**
  3. **Coffre & push par OS** : iOS Keychain / Android Keystore ; APNs/FCM au
     lieu d'IDLE.
  4. **Validation terrain iOS / WKWebView** : moteur WebKit **différent** de
     Blink — l'inconnue nommée par l'ADR 0015, à lever sur matériel Apple réel
     ou ferme d'appareils **avant tout envoi iOS**.

---

## Décisions ouvertes au Chef Ingénieur (à trancher au fil des phases)

| # | Phase | Décision |
|---|---|---|
| 1 | R5 | Composition **texte brut** (défaut) vs **HTML riche** (capacité neuve) |
| 2 | R6 | Onboarding : simple **porte** devant les 3 dialogues vs **auto-détection** de serveur |
| 3 | R3 | **Étoile + Déplacer** (présents en v1) : conservés ou coupés ? |
| 4 | R3 | **Raccourcis clavier** (r/f/e/v/s/c/Suppr/`/`) : repris ? |
| 5 | R5 | **« Rendre indépendante »** (multi-fenêtre) : reporté (proposé) ou dans le périmètre ? |

Aucune n'est bloquante pour démarrer R0/R1 — elles se posent quand leur phase
s'ouvre.
