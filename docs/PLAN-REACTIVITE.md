# Plan — Réactivité de l'affichage : rien ne clignote, le geste se voit, l'aperçu est là

Commande (2026-08-14) : trois retours utilisateur sur l'affichage.
(1) Pendant une synchronisation, l'écran se rafraîchit en montrant des
**traits à la place de chaque email** — désagréable. (2) Après un envoi
ou une suppression, le message met **très longtemps à apparaître dans le
dossier correspondant** (Envoyés, Corbeille) — on l'attend instantané.
(3) L'**aperçu du texte** dans le volet central manque sur certains
emails qui devraient l'avoir tout de suite.

Les trois attaquent la même promesse — *« Vos mails, instantanément »*
(PLAN.md §1) — par trois trous différents du même tissu : **ce que la
base sait n'arrive pas à l'écran au moment où l'utilisateur regarde.**
La plainte 2 englobe le chantier ouvert de PLAN-PIECES-JOINTES (copie
Envoyés toujours invisible en 0.1.5 malgré `sync_sent`) : il se solde
ici, en E2.

**Verdicts du Chef Ingénieur (2026-08-14, §6) : R-D1 tranchée à
« < 1 s »** — l'écho local en base, d'abord écarté par la rédaction,
devient la pièce maîtresse (E3) ; la relève ciblée reste, mais comme
**réconciliation**, plus comme chemin d'affichage. R-D2 : les corps des
arrivées se rapatrient **dans la relève INBOX** (E4). R-D3 : la passe
d'après-geste part aussi **au retour en ligne**.

## 1. L'analyse — trois plaintes, quatre causes racines

### Plainte 1 — les traits : la recharge jette avant de resservir

`Liste.recharger()` (`Liste.svelte:303`) vide TOUTES les pages
(`pages = new Map()`) puis bump `version` : la fenêtre re-rend
immédiatement avec `ligne: null` → gabarit `.attente` (« … »,
`Liste.svelte:412`) jusqu'au retour de `list_category`. La requête
chaude répond en millisecondes, mais le rendu passe par une frame de
squelettes — le flash que le terrain décrit.

Et pendant un cycle, `recharger()` est appelé **en rafale** : à chaque
mouvement du compteur `courrier` (sonde à 1 s, `App.svelte:494`), au
bump de génération du veilleur (sonde à 5 s, `App.svelte:269`), en fin
de cycle (`App.svelte:526`), après chaque geste. Chaque appel = un
flash. Le squelette était pensé pour le défilement vers des pages
jamais servies ; la recharge l'a hérité par accident.

**Cause racine : la recharge est destructive.** Le patron correct est
*stale-while-revalidate* : les lignes déjà servies restent affichées,
la version neuve les remplace À L'ARRIVÉE — le squelette ne se montre
qu'au premier chargement d'une source et au défilement vers l'inconnu.

### Plainte 2 — le geste local est instantané, la DESTINATION ne l'est pas

La moitié qui marche : `queue_removal` (`commands.rs:1964`) fait
disparaître le message de la source **immédiatement** (`remove_local`)
et journalise l'action (offline-first, ADR du rejeu — `sync.rs:207`).

La moitié qui traîne : l'action n'est **rejouée qu'à la prochaine
synchro du dossier source** (jusqu'à 5 min), et la copie de destination
(Corbeille, Archives, dossier de déplacement) n'entre en base **qu'à la
relève de CE dossier** — dans le même cycle, après l'inventaire. Latence
structurelle : cadence de sondage + position du dossier dans le cycle.
Rien, dans le produit, ne relève un dossier PARCE QU'un geste vient d'y
envoyer quelque chose — `sync_sent` (0.1.5) est la première exception,
et elle est cassée. Pire pour l'écho à venir : `remove_local`
(`store.rs:1144`) **détruit** l'enveloppe ET le corps — le produit
jette au geste la connaissance exacte qu'il faudrait montrer dans la
destination.

**Le cas Envoyés, instruit au code** (les pistes de la conversation
précédente, départagées) :

- **Piste 3 confirmée — c'est un défaut certain** : `sync_sent`
  (`commands.rs:2338`) appelle `SyncEngine::sync` directement, **sans
  bumper `cycle.generation` ni `courrier`** (le bump vit dans
  `relever_inbox`, `commands.rs:599-608`, qu'il n'emprunte pas). Même
  relève parfaite → l'UI n'apprend jamais que la base a bougé : la
  sonde de génération ne voit rien, et `apresEnvoi()`
  (`App.svelte:735`) ne fait que `chargerNav()` — la liste Envoyés
  affichée n'est PAS resservie. La copie n'apparaît qu'au prochain
  cycle complet qui rapporte (`bilan.fetched > 0`), jusqu'à 5 min.
- **Piste 1 confirmée en l'état** : le `.catch(() => {})`
  (`Composition.svelte:367`) et les `problems` en `eprintln` seuls
  (`commands.rs:2371`) rendent tout échec invisible — l'instruction
  terrain était aveugle par construction.
- **Piste 2 affinée — une course probable en prime** : Gmail ajoute la
  copie Envoyés à l'acceptation SMTP, mais de façon **asynchrone** ;
  `sync_sent` part dans la seconde qui suit la vidange. Si la copie
  n'est pas encore dans le dossier au STATUS, `doit_relever` répond
  non — honnêtement — et rien ne retente avant le cycle.
- Piste 4 (vidange non aboutie) : couverte par la sortie de l'ombre
  des erreurs (E2) ; à vérifier au terrain par la méthode qui a déjà
  tranché deux fois — lire `%APPDATA%\dev.elements.wind\wind.db`
  (l'enveloppe est-elle ENTRÉE dans la boîte Envoyés ?).

### Plainte 3 — l'aperçu vit avec le corps, et le corps arrive en retard

L'aperçu est une colonne de `bodies` (`store.rs:1252`,
`extraire_apercu` à l'écriture du corps) : **un message sans corps
rapatrié n'a pas d'aperçu** — c'est le protocole « enveloppes
d'abord » (ADR 0007). La ligne naît donc sans aperçu, et l'aperçu
dépend de la pompe de rattrapage. Trois trous dans le déclenchement :

- **le chemin veilleur n'amorce pas la pompe** : quand la génération
  bouge au repos (arrivée IDLE), `sonderSynchro` recharge la liste
  (`App.svelte:269`) mais ne lance PAS `rattraperCorps()` — le mail
  est à l'écran en 12 s (gate E4) **sans aperçu jusqu'au prochain
  cycle complet**, jusqu'à 5 min ;
- **la pompe ne resert pas la liste** : `rattraperCorps`
  (`App.svelte:280-299`) ne fait aucun `liste.recharger()` en fin de
  passe (contrairement à `rattraperApercus`, `App.svelte:251`) — les
  aperçus rattrapés attendent une recharge fortuite pour se montrer ;
- **pendant une synchro, la pompe paie le débit du fond** (D-7) : les
  aperçus traînent des minutes après une grosse relève — la puce 📎
  tardive du terrain 0.1.4 est le même corollaire.

La pompe elle-même est saine : bornée (budget 200), du plus récent au
plus ancien (`bodies_to_backfill`, `ORDER BY date_epoch DESC`), toutes
boîtes. Mais R-D2 tranche plus profond : pour le courrier NEUF, l'aperçu
ne doit pas dépendre d'une pompe du tout — la relève qui apporte
l'enveloppe apporte le corps (E4).

## 2. État des lieux

| Surface | Constat | Sort |
|---|---|---|
| `Liste.svelte` — `recharger()` | destructif : pages jetées avant resservie → squelettes | **E1** |
| `App.svelte` — recharges en rafale pendant le cycle | chaque bump = un flash | **E1** (rendues invisibles) |
| `commands.rs` — `sync_sent` | relève muette : ni génération ni courrier bumpés | **E2** |
| `Composition.svelte` — `envoyer()` | erreurs avalées (`.catch(() => {})`) ; pas de retentative sur copie asynchrone | **E2** |
| `App.svelte` — `apresEnvoi()` | `chargerNav()` seul, liste jamais resservie | **E2** |
| `store.rs` — `remove_local` | détruit enveloppe + corps au geste — la matière de l'écho part à la poubelle | **E3** (devient un déplacement) |
| `commands.rs` — `queue_removal` | rejeu au prochain cycle seulement ; destination jamais relevée au geste | **E3** |
| `mail-core` — `outbox` à l'état `sent` | la copie d'envoi n'existe nulle part en local avant la relève | **E3** (écho d'envoi) |
| `nav.rs` — `category_page` | sert des paires (compte, boîte réelle) — le point d'entrée de l'écho en liste | **E3** (UNION) |
| `mail-imap` — `folders_with_status` (E2c) | LIST-STATUS en un aller-retour, repli testé | **E3** (réconciliation) |
| `commands.rs` — `relever_inbox` | enveloppes seules, l'aperçu attend la pompe | **E4** (corps des arrivées) |
| `App.svelte` — `sonderSynchro` / `rattraperCorps` | pompe jamais amorcée au chemin veilleur ; liste jamais resservie en fin de passe | **E4** |
| D-7 (DETTE.md) | gestes au débit du fond pendant une synchro | hors périmètre — chronos posés ici l'instruiront |

## 3. Les objectifs chiffrés

Les budgets globaux de PLAN.md §1 (démarrage < 1 s, ouverture < 50 ms,
RAM < 200 Mo) et les gates de PLAN-SYNCHRO (bulle → liste < 30 s
constaté, cycle au repos < 60 s) **ne bougent pas**. Ce plan pose les
cibles du chemin « la base a bougé → l'écran le montre », qui n'avaient
jamais été chiffrées. R-D1 les aligne sur la promesse elle-même :
**« chaque action répond en moins de 100 ms »** vaut pour la
destination du geste comme pour sa source — c'est l'écho local qui le
permet, le serveur réconcilie derrière, en silence.

| # | Ce que l'utilisateur vit | Cible | Aujourd'hui (au code) |
|---|---|---|---|
| O1 | Recharge de liste (synchro, geste, veilleur) | **zéro ligne d'attente** sur des données déjà servies — le squelette n'existe qu'au premier chargement d'une source et au défilement vers l'inconnu | un flash de squelettes à CHAQUE recharge, en rafale pendant un cycle |
| O2 | Suppression/archivage : disparition de la source | < 100 ms (déjà tenu — affirmé par e2e) | tenu, jamais affirmé |
| O3 | Suppression/archivage/déplacement : apparition dans le dossier de destination | **< 1 s** (écho local — hors ligne compris) ; réconciliation serveur ≤ 5 s au repos, invisible à l'œil | jusqu'à 5 min + durée de cycle |
| O4 | Envoi : copie visible dans Envoyés | **< 1 s** après vidange SMTP aboutie (écho d'envoi) ; palier intermédiaire E2 : ≤ 5 s par relève ciblée | invisible (défaut E2) ; sinon jusqu'à 5 min |
| O5 | Courrier neuf : aperçu dans le volet central | **né avec la ligne** (corps dans la relève, R-D2) pour les arrivées courantes ; ≤ 5 s pour les gros lots (repli pompe) | jusqu'à 5 min (chemin veilleur : aucune pompe) |
| O6 | Stock sans corps : convergence des aperçus | pompe visible à l'œil — la liste se resert au fil des lots | les aperçus rattrapés attendent une recharge fortuite |

**Périmètre : O3/O4/O5 côté écho et ligne valent PARTOUT, y compris
hors ligne — c'est le sens de l'écho local.** Seule la
*réconciliation* serveur (≤ 5 s) se mesure au repos, réseau sain :
pendant une synchro lourde, D-7 — la priorité au geste — reste le
chantier qui la tiendra ; il est étranger à ce plan, mais les chronos
posés ici (horodatage du geste contre `sync_activity`) sont exactement
la mesure que D-7 réclame.

## 4. Les étapes

### E1 — La liste qui ne clignote plus (UI seule, le correctif « du jour »)

**État : livrée le 2026-08-14 (GO CE du jour), gate complète verte —
fmt, build ui-v2, contrastes, clippy, tests Rust (workspace + doc),
81 e2e (80 existants + le parcours neuf « recharger garde les lignes
servies », joué transport retenu par la couture `__e2eRetenue`).
Amendement A23 au journal du Système + carte « La liste sous
recharge » à la section Boîte de réception (DC-D2, même commit). En
prime, la recharge resert désormais TOUT le rang visible (une fenêtre
à cheval sur deux pages laissait sa seconde page sans resservie).**

`recharger()` devient *stale-while-revalidate* :

- les pages servies restent AFFICHÉES comme fond pendant que la
  génération neuve se resert ; chaque page est REMPLACÉE à l'arrivée
  de sa version fraîche (le jeton `generation` existant garde déjà les
  pages périmées hors du mélange) ;
- `total` et la hauteur d'espace ne retombent jamais à zéro sur une
  recharge — plus de saut de défilement ;
- le squelette `.attente` ne se rend que si AUCUNE donnée n'existe
  pour l'index (premier chargement d'une source, défilement vers une
  page jamais servie) — comportement au changement de source inchangé
  (source neuve = données étrangères, le squelette y est honnête) ;
- les recharges en rafale du cycle deviennent gratuites à l'œil — ce
  qui rend E2/E3/E4 libres de resservir souvent.

e2e neuf : pendant une recharge sur source inchangée, aucune
`ligne-attente` dans la fenêtre (assertion sur le DOM entre l'appel et
le retour — la couture `window.__e2e*` du banc sait suspendre le
transport). Système (DC-D2) : la règle « une recharge ne montre jamais
l'attente sur des lignes déjà servies » s'inscrit à la section liste
de `systeme.dc.html`, même commit.

### E2 — Envoyés soldé (le constat dû de PLAN-PIECES-JOINTES)

L'instruction d'abord, la méthode qui a tranché deux fois : **lire
`wind.db` sur le poste du CE** après un envoi — l'enveloppe est-elle
entrée dans `[Gmail]/Messages envoy&AOk-s` ? Si oui, la piste 3 seule
explique tout ; si non, la course d'asynchronie (piste 2) est active
aussi. Les correctifs couvrent les deux :

- **`sync_sent` compte son courrier** : la relève passe par le même
  chemin que `relever_inbox` — `fetched + deleted > 0` → bump de
  `cycle.courrier` ET `cycle.generation`. La sonde UI existante (5 s)
  voit bouger et resert liste + nav, sans canal neuf (R0-S5) ;
- **la retentative bornée** : si la relève ciblée ne rapporte rien
  (copie asynchrone pas encore là), elle retente à +5 s puis +15 s,
  puis se tait — le cycle rattrapera. Décision pure testée
  (`retenter_apres(tentative)`) ;
- **les erreurs sortent de l'ombre** : le `.catch(() => {})` de
  `envoyer()` devient un `console.error` ; les `problems` de
  `sync_sent` remontent au bilan comme ceux du cycle ;
- `apresEnvoi()` resert la liste si la catégorie affichée est
  `envoyes` — sans attendre la sonde (le cas exact du constat : on
  envoie, on va voir Envoyés).

Gate terrain : envoi réel → copie visible dans Envoyés **≤ 5 s** après
la vidange (palier intermédiaire d'O4 — E3 l'amène à < 1 s), constat
CE. C'est la clôture de PLAN-PIECES-JOINTES. E2 se livre AVANT E3 à
dessein : petit, sûr, il solde le constat dû — et la réconciliation
qu'E3 exigera est exactement cette relève ciblée réparée.

### E3 — L'écho local : la destination se montre au geste (R-D1)

Le verdict « < 1 s » impose que la destination s'affiche depuis la
base locale, sans attendre le serveur. Décision de fond, avec ses
garde-fous — les règles d'or ne plient pas :

- **PJ du dessin : jamais de clé forgée.** L'écho ne s'écrit PAS dans
  `envelopes` — un UID inventé forgerait la clé `(mailbox, uid)` sur
  laquelle tout repose (Lecture, fils, actions, rejeu). Table neuve
  **`echos`** : identité locale, compte, **catégorie de destination**
  (corbeille, archives, envoyés, ou boîte de déplacement), enveloppe
  complète (expéditeur, objet, date, `message_id`), aperçu, corps
  HTML, pièces (métadonnées), et la référence d'ORIGINE (action
  journalisée, ou entrée outbox pour un envoi).
- **Le geste devient un déplacement, plus une destruction** :
  `remove_local` détruit aujourd'hui enveloppe et corps
  (`store.rs:1144`) — la transaction du geste versera cette matière à
  l'écho AVANT de la retirer de la source, dans la MÊME transaction
  que `enqueue_action`. Un crash entre deux ne perd rien et ne
  fabrique rien : l'écho reflète exactement une intention journalisée.
- **L'écho d'envoi naît à `sent`** — au passage à l'état `sent` de la
  vidange, jamais avant l'acceptation SMTP : un envoi refusé ne laisse
  RIEN dans Envoyés (« jamais d'envoi fantôme », le contrat tient).
  Contenu : le journal outbox (texte, pièces en métadonnées).
- **Servi en liste par `category_page`** (UNION avec les enveloppes,
  tri par date commun) — visuellement identique : c'est le même
  message, pas un état. L'ouverture sert le corps de l'écho (local,
  hors ligne compris). Les AUTRES gestes sur un écho (supprimer depuis
  la Corbeille dans la seconde qui suit…) sont différés à la
  réconciliation — fenêtre de quelques secondes, cas rare, dit par un
  toast si tenté.
- **La réconciliation : la passe d'après-geste.** Commande
  `sync_apres_geste(account_id, source_mailbox)` :
  1. relève de la SOURCE — les actions en attente forcent la relève
     (`faut_relever` le fait déjà) : le rejeu part MAINTENANT ;
  2. `folders_with_status` (LIST-STATUS, E2c — un aller-retour ;
     repli par dossier déjà testé, coût borné car la passe ne part
     qu'au geste) ;
  3. `faut_relever` sur chaque dossier : seuls ceux qui ont bougé se
     relèvent — la destination n'est jamais devinée (Corbeille
     RFC 6154, label Gmail, effets de bord serveur : tout se voit au
     STATUS) ;
  4. **l'écho meurt quand la vraie ligne entre** : enveloppe au même
     `message_id` dans la destination → l'écho se retire dans la même
     transaction — la ligne ne bouge pas à l'œil (même contenu, même
     date) ; courrier compté, génération bumpée, la sonde resert.
- **Un vol par compte, coalescé** : archiver dix messages n'ouvre pas
  dix passes — une passe en vol, une demande en attente au plus
  (verrou de relève E4 + drapeau « à rejouer »).
- **Retour en ligne (R-D3)** : des actions journalisées attendent →
  `sync_apres_geste` se greffe sur la relève `online` de P0-bis. Les
  échos posés hors ligne vivent jusqu'à leur réconciliation — c'est
  l'offline-first appliqué à la destination.
- **Le balayage de sûreté** : un écho dont l'action est REJOUÉE et
  dont la destination, relevée, ne montre pas de copie après les
  retentatives (E2) → l'écho se retire et l'incident se consigne au
  bilan — on n'affiche pas indéfiniment ce que le serveur dément. Un
  écho dont l'action attend encore (hors ligne, recul) vit : il
  reflète l'intention.
- Chronos par passe en console (§6.8 : durées et décomptes seuls) :
  `passe geste : source x s · inventaire x s · n dossiers relevés
  x s` — la mesure qui instruira D-7.

Gate : suppression → Corbeille **< 1 s**, archivage → Archives
**< 1 s**, déplacement → dossier cible **< 1 s**, envoi → Envoyés
**< 1 s** après vidange (O3/O4) — câble débranché compris (écho seul,
réconciliation au retour, R-D3) ; réconciliation ≤ 5 s au repos
constatée à la trace ; disparition source < 100 ms affirmée e2e (O2).
Tests Rust : transaction geste→écho (crash simulé : rien de perdu,
rien de fabriqué), réconciliation par `message_id`, balayage (serveur
qui dément), écho d'envoi jamais avant `sent`. Système (DC-D2) : la
règle « le geste se voit dans la destination immédiatement » et le
sort des gestes sur un écho s'inscrivent au Système, même commit.

### E4 — L'aperçu né avec la ligne (R-D2)

- **la relève INBOX rapatrie les corps des arrivées** : dans
  `relever_inbox`, après l'upsert des enveloppes et AVANT le bump de
  génération, un `fetch_bodies_html` des arrivées du lot — **borné aux
  N plus récentes** (N = 10, à mesurer) sur la MÊME connexion : la
  ligne naît avec son aperçu, au cycle comme à la passe légère comme
  au veilleur. Au-delà de N (rattrapage après coupure), le bump part
  d'abord — les lignes vite — et le reste échoit à la pompe ;
- **le chemin veilleur amorce la pompe** : dans `sonderSynchro`, quand
  la génération bouge → `rattraperCorps()` en plus de la recharge —
  couvre le débordement de N et le stock ;
- **la pompe resert la liste** : après chaque lot qui a rapporté
  (`fetched > 0`), `liste.recharger()` — invisible grâce à E1 ; les
  aperçus du stock se remplissent sous les yeux (O6) ;
- **la gate E4 de PLAN-SYNCHRO se re-mesure** : bulle → liste < 30 s
  constaté reste la borne ; la valeur vécue (12 s) peut glisser de
  l'ordre d'un aller-retour de corps (~1 s sur le terrain
  `spikes/body-backfill` : ~192 ms/message amorti par lot) — attendu
  ≤ 15 s, à constater.

Gate : au repos, mail reçu au téléphone → ligne AVEC aperçu en un seul
affichage (O5), sous la borne < 30 s de PLAN-SYNCHRO ; requête de
contrôle sur `wind.db` : plus d'enveloppe récente d'INBOX sans corps
5 min après l'arrivée, au repos.

## 5. Ce qu'on ne fait PAS (PASSATION §2.6)

- **Pas d'écho dans `envelopes`** : l'écho vit dans sa table, jamais
  sous une clé `(mailbox, uid)` forgée — c'est la condition qui rend
  R-D1 compatible avec les règles d'or. (L'écho local lui-même,
  écarté à la rédaction, a été TRANCHÉ par R-D1 — la ligne d'origine
  ne vaut plus.)
- **Pas d'écho pour les drapeaux** : lu/non-lu et étoile sont déjà
  optimistes en place (`set_seen_local`, `set_flagged_local`) — rien
  à construire.
- **Pas de canal d'événements Tauri** — le port reste du sondage
  (R0-S5) : génération monotone + sondes existantes suffisent, comme
  pour P1 et E4 de PLAN-SYNCHRO.
- **Pas de résolution D-7 ici** (priorité au geste pendant une
  synchro) : chantier de synchronisation séparé — mais les chronos
  d'E3 posent la mesure qu'il attend, et l'écho local rend D-7 moins
  brûlante à l'écran (la destination ne dépend plus du débit du fond).
- **Pas d'APPEND Envoyés** pour les serveurs qui ne copient pas
  d'eux-mêmes à l'acceptation SMTP (Microsoft/générique à vérifier en
  usage réel) : le balayage de sûreté d'E3 le rendra VISIBLE (écho
  retiré + incident consigné) — si le terrain le montre, ce sera un
  chantier d'envoi, pas d'affichage ; consigné, pas construit.
- **Pas de squelette « amélioré »** (shimmer, fondu) : la bonne
  correction est de ne pas le montrer, pas de le rendre joli.

## 6. Décisions du Chef Ingénieur

Verdict du 2026-08-14 :

| # | Décision | Verdict |
|---|---|---|
| R-D1 | ≤ 5 s à la destination (relève ciblée seule) ou < 1 s (écho local) | **Tranchée : < 1 s** — l'écho local est le cœur d'E3 ; la relève ciblée devient la réconciliation ; la proposition initiale (≤ 5 s d'abord) ne vaut plus |
| R-D2 | Corps des arrivées dans la relève INBOX (cœur) ou pompe seule | **Tranchée : dans la relève** — la ligne naît avec son aperçu (E4) ; la pompe reste pour le stock et le débordement |
| R-D3 | La passe d'après-geste part-elle au retour en ligne ? | **Tranchée : oui** — greffée sur la relève `online` de P0-bis quand des actions attendent |

## 7. Ordre de livraison et gates

E1 (UI seule, débloque l'œil) → E2 (le constat dû, clôt
PLAN-PIECES-JOINTES — et répare la relève ciblée qu'E3 réutilise) →
E3 (l'écho local, le gros œuvre) → E4 (corps dans la relève). Chaque
étape : gate complète avant commit (fmt + clippy + tests Rust + e2e),
`systeme.dc.html` amendé dans le même commit que tout changement
visible (DC-D2).

| Étape | Gate |
|---|---|
| E1 | e2e neuf « recharge sans ligne-attente » vert + suite écran 02 sans régression ; DC amendé (règle de recharge) |
| E2 | terrain CE : envoi réel → copie Envoyés ≤ 5 s (palier O4) ; erreurs visibles au bilan ; PLAN-PIECES-JOINTES clos |
| E3 | terrain CE : suppression/archivage/déplacement/envoi → destination **< 1 s** (O3/O4), câble débranché compris ; réconciliation ≤ 5 s au repos à la trace ; source < 100 ms affirmée e2e (O2) ; tests crash/réconciliation/balayage verts ; DC amendé |
| E4 | terrain CE : arrivée au repos → ligne AVEC aperçu en un seul affichage (O5), borne < 30 s de PLAN-SYNCHRO tenue ; contrôle `wind.db` : zéro enveloppe INBOX récente sans corps à +5 min (au repos) |

La ligne s'arrête quand une gate casse — c'est elle qui commande.
