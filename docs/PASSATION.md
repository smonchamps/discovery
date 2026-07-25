# Passation — reprendre Discovery dans une nouvelle conversation

> **Ce document est l'instruction de projet.** Il n'y a pas de `CLAUDE.md`
> ici : tout ce qui ne se déduit pas du code est écrit là.
>
> État au **2026-07-25**, branche `main`, commit `7d6a60a`. Arbre propre,
> **305 tests Rust · 19/19 E2E · clippy muet**. Aucun code en vol.
>
> **Phases 0 à 3 closes**, gate 3 joué, chantier de l'ADR 0009 terminé et
> validé sur le terrain. La suite est la **Phase 5** (durcissement et
> bêta), choisie devant la Phase 4 (web).

---

## 0. Comment ouvrir la conversation

Colle ceci comme premier message :

> Reprends le développement de Discovery. Tu es le Chef Ingénieur du
> projet et tu appliques la méthode décrite dans `docs/PASSATION.md` §2 —
> c'est une instruction permanente, elle prime sur tout. Lis d'abord ce
> document en entier, puis applique le §1.

Ordre de lecture, une fois :

1. **ce document** — méthode, état, pièges ;
2. [`docs/PLAN.md`](PLAN.md) — le concept paper, source de vérité produit ;
3. les ADRs dans [`docs/adr/`](adr/) — **décisions gelées**, à ne pas
   rouvrir sans mesure contraire.

Ne lis pas le code avant. Il est volumineux et abondamment commenté ; les
commentaires expliquent *pourquoi*, et supposent le contexte ci-dessous.

---

## 1. Où on en est, et quoi faire en premier

**Rien n'est cassé, rien n'est à moitié écrit, rien n'est en vol.**

### 1.1 Ne rien engager avant que le terrain ait fini de parler

Le dernier chantier livré — la portée des fils au compte et la
synchronisation d'« Envoyés » ([ADR 0009](adr/0009-portee-des-fils-au-compte.md))
— **converge encore**. La passe d'en-têtes est bornée par cycle de
synchronisation ; il restait ~1 650 messages dans l'horizon de 12 mois au
moment d'écrire ces lignes.

Sur la boîte réelle, les conversations de plus d'un message sont passées
de **15 à 248**, et le chiffre monte encore à chaque synchronisation.

**Demander d'abord à l'utilisateur de relancer le diagnostic** avant toute
conclusion sur le regroupement :

```powershell
cargo run -p mail-core --example diagnostic_fils --release -- "$env:APPDATA\dev.discovery.app\discovery.db"
```

Les deux nombres qui parlent : `lus, avec References` (257 au dernier
relevé, doit monter) et la distribution des tailles de conversations
(242 fils de 2 à 5, 6 fils de 6 à 20).

### 1.2 Les deux budgets non tenus, avec leur remède

Aucun n'est un défaut ouvert : les deux sont mesurés, expliqués, et
disposent d'un levier connu. Ils sont détaillés au §8 et dans
[PHASE3.md](PHASE3.md) §2 et §2 bis.

| Poste | Mesure | Levier |
|---|---|---|
| Adoption d'une base héritée | 3,7 s à 200 000 messages, **une seule fois** | la rendre visible et interruptible (précédent : ADR 0007) |
| Recherche | 118–208 ms à l'échelle du gate 3 | tri par date (×2, mesuré) ou `prefix=` |

**La recherche est confortable en usage réel** : le coût unitaire est de
~2,9 µs par correspondance, donc sur une boîte de 7 500 messages aucune
requête ne peut dépasser ~35 ms. Le plafond des ~35 000 correspondances
n'est atteignable qu'à l'échelle synthétique du gate 3.

### 1.3 Les arbitrages ouverts — ils appartiennent à l'utilisateur

- **Synchroniser l'archive ?** Les ancres « FANTÔME » des plus gros fils
  montrent que des messages d'origine restent hors de la base, archivés
  hors d'INBOX. Seule l'archive les ramènerait, au prix du disque et du
  plafond de recherche. **Ne pas engager avant que la passe d'en-têtes
  ait fini** (§1.1) : le chiffre bougera encore.
- **Quand ouvrir la Phase 5**, et avec quel périmètre de durcissement.

### 1.4 Ensuite — la Phase 5

Durcissement et bêta ([PLAN.md](PLAN.md) §4). C'est la bêta qui permettra
de trancher la recherche sur de vraies boîtes plutôt que sur un corpus
synthétique — c'est exactement pourquoi elle passe devant la Phase 4.

---

## 2. La méthode — instruction permanente

Le développement suit la discipline du *shusa* (Chef Ingénieur) de Toyota.
**Elle prime sur tout le reste**, y compris sur l'envie d'avancer vite.

### 2.1 L'utilisateur est le Chef Ingénieur, pas un client
Il tranche les décisions produit et **valide chaque incrément sur ses
vrais comptes**. Tu proposes, tu mesures, tu recommandes ; il arbitre.
Ne prends jamais une décision de périmètre à sa place.

### 2.2 Front-loading — les points durs se règlent AVANT de coder
Par un **spike jetable et mesuré**, hors du workspace de production. Fait
pour : moteur de synchro, pont web, rendu HTML, OAuth, moteur de recherche.

### 2.3 Set-based — explorer, puis éliminer sur des chiffres
On compare plusieurs options et on tranche **sur des mesures, pas des
avis**. Règle de départage : l'alternative doit battre l'hypothèse
*nettement* pour la déloger. Modèle à imiter : [ADR 0004](adr/0004-moteur-de-recherche-fts5.md).

### 2.4 Jidoka — la qualité dans le processus
- **TDD** : le test échoue (RED) avant l'implémentation (GREEN).
- **Gate obligatoire avant tout commit** — et un hook `pre-push` le rejoue
  (§7.4). Un warning clippy = build rouge.
- Zéro `unwrap()`/`expect()` en production. Erreurs typées (`thiserror`)
  dans les crates, `anyhow` dans les apps.

### 2.5 Genchi genbutsu — aller voir sur le terrain
**C'est là que les défauts se trouvent.** Voir §9. Un incrément non validé
sur un vrai compte n'est pas livré. Les retours se corrigent **le jour
même**.

### 2.6 Refus de périmètre explicites
Quand une fonctionnalité serait un fantôme (résultat invisible, brique
absente), on la **reporte et on écrit pourquoi**. Dire non est le
comportement par défaut : chaque ajout se paie en vitesse et en fiabilité.

### 2.7 Traçabilité
- Décision structurante = **un ADR court** dans `docs/adr/`.
- Fin de phase = **une revue de clôture** `docs/PHASEn.md` : livré contre
  le plan, budgets re-mesurés, enseignements, reports assumés, GO/NO-GO.

### 2.8 Langue et commits
**Tout est en français** — commits, UI, docs, commentaires de code. Ce
n'est pas cosmétique : c'est la langue du client cible et du Chef
Ingénieur. Format `type: description` (`feat`, `fix`, `refactor`, `docs`,
`test`, `chore`, `perf`, `ci`). **Jamais de `Co-Authored-By`.**

⚠️ **Les messages de commit s'écrivent SANS ACCENTS** — c'est la
convention observable dans tout l'historique. Le corps du message, lui,
porte les chiffres et le raisonnement : ils valent mieux là que nulle part.

---

## 3. Le produit

**Promesse :** *« Vos mails, instantanément. »* Un client email qui démarre
en moins d'une seconde, où chaque action répond en moins de 100 ms, et qui
fonctionne hors-ligne comme en ligne.

**Cible :** professionnel ou particulier exigeant, 1 à 4 comptes (Gmail,
Microsoft 365, IMAP générique — les trois sont livrés et validés).

**Ce qu'il EST :** rapide (la performance est LA fonctionnalité), simple
(lire, trier, chercher, écrire — rien d'autre), fiable (jamais de perte,
jamais d'envoi fantôme), sûr (credentials dans le coffre de l'OS, HTML
assaini, images distantes bloquées).

**Ce qu'il N'EST PAS (v1) :** pas de calendrier, pas de chat, pas d'IA
intégrée, pas de plugins, pas de mobile.

### Budgets — ce sont des gates BLOQUANTS

Mesurés au **gate 3** (3 comptes, 200 000 messages) puis re-mesurés après
l'ADR 0009 — [PHASE3.md](PHASE3.md) §2 et §2 bis :

| Métrique | Cible | Dernière mesure |
|---|---|---|
| Démarrage à froid | < 1 s | 360–389 ms ✅ |
| Ouverture d'un message | < 50 ms | 0,09–0,15 ms ✅ |
| Page de liste | < 100 ms | 0,71 ms ✅ |
| RAM (working set **privé**) | < 200 Mo | 92,2 Mo · 7 processus ✅ |
| Taille de la base | < 1 Go | 778 Mo / 200 000 msg + 16 002 corps ✅ |
| Perte de données | 0, prouvé par crash-récup | ✅ |
| **Recherche** | < 100 ms | **118–208 ms ❌** |
| **Adoption d'une base héritée** | < 1 s | **3,7 s ❌** (une seule fois) |

Un budget dépassé = **on arrête la ligne** (andon). Pas de « livrer puis
optimiser » : la performance est une contrainte de conception.

⚠️ **Les outils de mesure se vérifient comme le reste.** Trois d'entre eux
mentaient au gate 3 : `mesure-ram.ps1` sommait toutes les instances de
l'application, `mesure.mjs` n'isolait pas son profil WebView2, et le décor
de mesure n'exerçait pas l'index partiel qu'il était censé valider.
Corrigés — mais le réflexe reste à avoir.

---

## 4. Architecture — « un seul cerveau »

`mail-core` contient **100 % de la logique métier**, de la synchro et du
stockage. Le desktop l'embarque en processus ; le web (Phase 4) l'exécutera
côté serveur. L'UI est « bête » : elle affiche un état, elle émet des
intentions.

```
discovery/
├── crates/
│   ├── mail-core/     # domaine + synchro + stockage + recherche + fils
│   │                  # (ZÉRO dépendance UI ou réseau)
│   ├── mail-imap/     # adaptateur IMAP (implémente MailServer)
│   ├── mail-auth/     # OAuth2 PKCE loopback + coffre Windows (keyring)
│   ├── mail-render/   # assainissement HTML (ammonia) + texte + CSP
│   └── mail-smtp/     # adaptateur SMTP (lettre, XOAUTH2)
├── apps/desktop/      # Tauri 2 : commands.rs (IPC) + main.rs + ui/ (JS vanilla)
├── e2e/               # Playwright pilotant la VRAIE fenêtre via CDP WebView2
├── spikes/            # prototypes jetables, hors workspace de prod
└── docs/              # PLAN, revues de phase, ADRs, ce document
```

**La seule frontière abstraite** est le trait `MailServer` (lecture) et le
port `MailTransport` (envoi). **SQLite n'est PAS derrière un trait** :
décision gelée ; `Store` est une struct concrète, et les tests utilisent
une base en mémoire.

**Un motif récurrent, à imiter.** La décision est **pure et testable**,
l'exécution (I/O) est ailleurs : `thread::plan` pour les conversations,
`plan_draft_pull` pour les brouillons, `convert::sent_folder` pour la
découverte du dossier des envois, `notify::arrivals_to_notify` pour les
bulles. C'est ce qui permet de tester les scénarios du terrain sans réseau.

---

## 5. Décisions gelées — ne pas rouvrir sans mesure

| ADR | Décision | À retenir |
|---|---|---|
| [0001](adr/0001-structure-workspace.md) | Workspace Cargo multi-crates | `mail-core` sans dépendance UI/réseau |
| [0002](adr/0002-shell-desktop-tauri.md) | Shell desktop = Tauri 2 (WebView2) | La RAM qui fait foi = working set **privé** |
| [0003](adr/0003-boite-envoi-smtp.md) | Boîte d'envoi SMTP + règles d'or | Journal AVANT réseau ; quarantaine anti-fantôme ; texte brut assumé |
| [0004](adr/0004-moteur-de-recherche-fts5.md) | Recherche = SQLite **FTS5** | L'index vit DANS la base (transactionnel) |
| [0005](adr/0005-gate-e2e-hors-ci-hebergee.md) | E2E hors CI hébergée | Un runner GitHub ne peut pas ouvrir WebView2 — d'où le hook `pre-push` |
| [0006](adr/0006-microsoft-imap-oauth2.md) | Microsoft via IMAP+OAuth2, pas Graph | Graph reste le plan B, avec ses signaux de bascule |
| [0007](adr/0007-rattrapage-des-corps.md) | Rattrapage des corps borné (12 mois) | Sans lui, la recherche ne portait que sur les sujets |
| [0008](adr/0008-regroupement-en-conversations.md) | Conversations = union-find sur en-têtes RFC 5322 | **Jamais de repli par sujet** ; agrégat recalculé, jamais incrémenté ; un identifiant exige une arobase |
| [0009](adr/0009-portee-des-fils-au-compte.md) | Portée d'un fil = le **compte** | Révise 0008 §3 et §4 ; « Envoyés » synchronisé ; **index partiel** sinon le gate 3 est perdu |

Décisions Phase 0 ([PHASE0.md](PHASE0.md) §2) : SQLite local ; CONDSTORE
(Gmail n'expose pas QRESYNC) ; parsing MIME par `mail-parser` ; OAuth2 PKCE
loopback + coffre OS ; rendu HTML en défense en profondeur.

---

## 6. Invariants non négociables

Faciles à casser **en silence**. À vérifier à chaque revue.

1. **Boîte d'envoi — les deux règles d'or** (ADR 0003) : jamais d'envoi
   perdu (l'intention est journalisée AVANT tout réseau) ; jamais d'envoi
   fantôme (un envoi interrompu part en **quarantaine** et n'est jamais
   renvoyé automatiquement). *« Le doublon est pire que le retard. »*
2. **Identité message = `(account_id, boîte, uid)`** partout, jusque dans
   la sélection de l'UI. Un UID seul n'identifie rien, **et le couple
   compte + UID non plus** : les UID sont attribués par boîte et repartent
   de 1, donc le message n°1 d'INBOX et le n°1 d'« Envoyés » sont deux
   messages différents du même compte.

   Le compilateur ne protège pas cet invariant — une boîte est une chaîne
   comme une autre. C'est un test qui le tient
   (`chaque_ligne_dit_dans_quelle_boite_elle_habite`).
3. **Les index et agrégats vivent DANS la base**, entretenus dans la MÊME
   transaction que le message : index FTS5, table `threads`. Pas de second
   magasin, pas de réconciliation après crash.
4. **Sécurité du rendu** : HTML assaini par `ammonia`, images distantes
   bloquées par défaut, iframe sandboxée + CSP. Données de mail injectées
   par `textContent`, **jamais** `innerHTML`.
5. **Credentials jamais en clair** : Credential Manager Windows via
   `keyring`. Aucun secret dans le code ni les logs.
6. **UIDVALIDITY** : si elle change, on repart de zéro pour cette boîte —
   et comme un fil peut désormais réunir deux boîtes, c'est **tout le
   compte** qui refait ses fils (`thread::rebuild_account`). Règle
   brouillons : *« un doublon est acceptable, supprimer le mauvais UID
   jamais »*.
7. **Une fonctionnalité neuve doit ADOPTER les données anciennes** — le
   piège s'est présenté **quatre fois** (§9), la dernière sur le SCHÉMA et
   non sur les données.
8. **Les diagnostics ne divulguent rien** : ni sujet, ni expéditeur, ni
   contenu, et les identifiants techniques sont **masqués** — on n'en
   montre que la forme. Un diagnostic qui fuit est un diagnostic qu'on
   n'ose plus coller dans une conversation.

---

## 7. Environnement & commandes

Windows 11. Deux shells : **PowerShell 5.1** (principal) et **Bash** (Git
Bash). Syntaxes différentes.

### 7.1 Pièges qui coûtent cher

- **PowerShell 5.1 n'a pas `&&`.** `cd e2e && npm test` échoue → deux
  lignes, ou passer par Bash.
- **Ne JAMAIS utiliser `Get-Content`/`Set-Content` sur les sources** :
  PowerShell 5.1 réencode en UTF-16 avec BOM et corrompt les accents.
  Éditer via l'outil `Edit`, Python, ou Bash. Tout est en **UTF-8**.
- Pour un affichage non-ASCII depuis Python : `PYTHONIOENCODING=utf-8`.
- **L'assistant ne voit PAS la vraie base.** L'application de bureau Claude
  est empaquetée MSIX : le shell qu'elle lance lit un `%APPDATA%`
  **redirigé**, et `%APPDATA%\dev.discovery.app\discovery.db` y résout vers
  une copie privée périmée
  (`%LOCALAPPDATA%\Packages\Claude_…\LocalCache\Roaming\`). La vue est
  *fusionnée*, pas gelée — les fichiers voisins apparaissent normalement,
  seul `discovery.db` est masqué, ce qui rend le piège discret.

  **Conséquence :** les diagnostics du §9 doivent être lancés **par
  l'utilisateur**, qui colle la sortie. Corollaire de méthode : annoncer
  d'abord ce qu'on s'attend à y lire, pour que l'aller-retour soit une
  mesure et non une collecte.
- **Un commit ne peut pas être chaîné avec `git --no-pager …`** : le hook
  `block-no-verify` reconnaît le préfixe `--no-` et bloque. Séparer les
  commandes.

### 7.2 Les notifications exigent l'application INSTALLÉE

`tauri-winrt-notification` exige une **identité applicative
(AppUserModelID)**, qui n'existe que si un **raccourci du menu Démarrer**
la porte. Donc :

- lancée par `cargo run`, l'application n'a aucune identité — Windows
  refuse le toast, **silencieusement** ;
- il faut `cargo tauri build`, installer, et lancer **depuis le menu
  Démarrer** ;
- `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET` et `MICROSOFT_CLIENT_ID`
  doivent alors être définis **au niveau utilisateur**.

⚠️ **Windows n'inscrit une application dans Paramètres → Notifications
qu'APRÈS sa première notification réussie.** Vérifier la liste avant a
produit un faux négatif pendant une validation.

### 7.3 Commandes

```bash
cargo test --workspace --all-targets           # tout, EXEMPLES COMPRIS
cargo test --workspace --doc                   # les doc-tests, exclus ci-dessus
cargo build -p discovery-desktop --release     # binaire
cargo run -p discovery-desktop --release       # lancer (sans notifications)

cargo fmt
cargo clippy --all-targets -- -D warnings

cd e2e
npm test                                       # PowerShell : deux lignes

# Jeu d'essai — <db> <nombre> <email> [corps] [ko/corps] [boîte]
cargo run -p mail-core --example seed_inbox --release -- <db> 33000 un@exemple.fr 0 0 INBOX

# Installateur (nécessaire pour les notifications)
cd apps/desktop
cargo tauri build
```

Mesures : `node e2e/mesure.mjs` (démarrage, page, RAM — paramétrable par
`MESURE_DB`, `MESURE_COMPTES`, `MESURE_REUTILISER`),
`e2e/mesure-ram.ps1 -AppPid <id> -Profil <dossier>`.

⚠️ **La base de mesure se place HORS du dépôt** : celui-ci vit dans
OneDrive, dont la synchronisation perturberait la mesure en cours.

### 7.4 Le gate pré-push

`.githooks/pre-push` (via `core.hooksPath`) rejoue, dans l'ordre : `fmt` →
`clippy -D warnings` → `cargo test --workspace --all-targets` →
`cargo test --workspace --doc` → `npm test` dans `e2e/`.

Il existe parce qu'un runner GitHub **ne peut pas** ouvrir WebView2
(ADR 0005) : les E2E ne tournent que depuis cette machine.

**`--all-targets` n'est pas décoratif** : sans lui, cargo ignore les tests
des EXEMPLES — or les diagnostics du terrain vivent là et portent leurs
propres tests. Deux d'entre eux ont été écrits, verts, et n'auraient jamais
été exécutés.

Il a déjà rattrapé des livraisons annoncées vertes qui ne l'étaient pas.
`--no-verify` existe ; s'en servir est une décision, pas un raccourci.

### 7.5 Déterminisme des E2E

Étanches par construction : base SQLite jetable (`DISCOVERY_DB_PATH`),
comptes factices aux jetons invalides (`DISCOVERY_E2E_ACCOUNT`),
`GOOGLE_CLIENT_ID`/`SECRET` **retirés** de l'environnement, et un **profil
WebView2 dédié** — sans lui, une fenêtre déjà ouverte par l'utilisateur
fait ignorer `--remote-debugging-port` et le port CDP ne s'ouvre jamais.

**Conséquence à garder en tête :** les E2E ne parlent à aucun serveur. Tout
ce qui touche au réseau réel — OAuth, tirage des brouillons, dossier des
envois, passes de fond — n'est couvert que par des tests unitaires sur la
partie pure. Le chemin réseau complet ne se prouve que sur le terrain.

---

## 8. Ce qui reste

Phases 0 à 3 : **closes** ([PHASE0](PHASE0.md), [PHASE1](PHASE1.md),
[PHASE2](PHASE2.md), [PHASE3](PHASE3.md)). Le chantier de l'ADR 0009 est
clos et validé sur le terrain.

### Les deux budgets non tenus

**Adoption d'une base héritée — 3,7 s à 200 000 messages.** Ne se paie
qu'une fois ; le démarrage courant reste à **2,8 ms**. Deux correctifs
mesurés l'ont déjà ramenée de 11,1 s (un `Vec::contains` quadratique, puis
l'absence de `prepare_cached` sur ~1 million de requêtes). Le reste est le
coût intrinsèque d'un union-find message par message.

*Remède, avec précédent dans le projet :* la rendre **visible et
interruptible**, comme le rattrapage des corps (ADR 0007). Une migration
qui s'annonce vaut mieux qu'une fenêtre figée.

*Ce qu'il ne faut PAS faire :* adopter par tranches à chaque démarrage. La
liste part de `threads`, donc une adoption partielle afficherait une boîte
à moitié vide — le piège du §9.

**Recherche — 118 à 208 ms sur corpus synthétique.** Le poste dominant est
le classement BM25 sur **toutes** les correspondances, devant l'expansion
de préfixe. Deux leviers chiffrés : le **tri par date** (×2, quatre
requêtes sur six repassent sous le budget) et l'option **`prefix=`** de
FTS5 (−73 ms d'expansion). Le premier est un arbitrage produit — récence
contre pertinence — que le Chef Ingénieur a choisi de **trancher en bêta**,
sur de vraies boîtes.

### Reports assumés

- **Défilement profond** : `OFFSET` parcourt puis jette *n* lignes, d'où
  ~230 ms à 150 000 conversations. Seule une pagination **par curseur**
  l'effacerait, au prix de l'API du store et de la liste virtualisée.
- **Envoi de pièces jointes** (lecture seule en v1).
- **Filtre « a une pièce jointe »**, **`to:` dans la recherche**.
- **Synchronisation de l'archive** — voir §1.3.
- **CONDSTORE réel, IDLE/push** — reports de Phase 1 inchangés.
- **Dossier CASA Google** — côté produit-owner, chemin critique du
  lancement public.

### Dette connue, non corrigée

`apps/desktop/ui/style.css` : la règle d'élément `header { display: flex }`
(destinée à la barre du haut) s'applique **aussi** à `#detail-header`, qui
est un `<header>`. Tout enfant pleine largeur qu'on y ajoute devient un
item flex écrasé à 0 px et poussé hors écran — mesuré. Le bandeau de
conversation a dû être sorti de `#detail-header` pour cette raison.
`#attachments` et `#detail-note` y sont toujours et ne fonctionnent que
par chance.

### La Phase 5

Durcissement et bêta ([PLAN.md](PLAN.md) §4) : installeur et mise à jour
signée, télémétrie de crash opt-in, bêta fermée 20-50 utilisateurs, kaizen
hebdomadaire sur les frictions **observées**. Gate 5 : deux semaines sans
défaut critique.

---

## 9. Enseignements — à lire avant de reprendre

Ils ont coûté cher. Les ignorer les fera repayer.

### Les défauts se trouvent sur le terrain, pas dans les tests

Jamais des erreurs de logique : toujours des **hypothèses fausses sur
l'environnement ou sur l'usage**. Une suite de tests ne peut pas les
attraper, parce qu'elle partage l'hypothèse.

### Une fonctionnalité neuve doit ADOPTER les données anciennes

Le piège s'est présenté **quatre fois** : pièces jointes, conversations,
en-têtes de fil, puis — la dernière — le **schéma** lui-même.
`CREATE TABLE IF NOT EXISTS` ne touche pas une table existante, mais
l'index partiel, lui, était bien créé : il échouait sur une colonne
absente et **l'application ne démarrait plus**. Aucun test ne pouvait le
voir : ils créent tous une base neuve.

**Écrire la migration en même temps que la fonctionnalité, et la prouver
par un test qui rembobine une vraie base de fichier à son état antérieur.**

### Mesurer avant de corriger — y compris ses propres hypothèses

Sur le faux regroupement (43 messages étrangers), trois hypothèses étaient
fausses ; le diagnostic a désigné la cause en une commande. Sur le coût de
l'adoption, le `Vec::contains` quadratique a été annoncé comme cause
dominante **avant** d'être mesuré : il ne valait qu'un quart du coût, le
reste étant l'absence de cache de requêtes.

Six outils existent, même modèle — lecture seule, **aucun sujet, aucun
expéditeur, aucun contenu** :

| Outil | Répond à |
|---|---|
| `diagnostic_index` | les messages sont-ils dans l'index de recherche ? |
| `diagnostic_fils` | quel identifiant réunit les messages d'un fil ? |
| `diagnostic_brouillons` | le tirage des brouillons fait-il son travail ? |
| `banc_page_liste` | le coût d'une page dépend-il de la taille de la boîte ? |
| `banc_migration_fils` | que coûte l'adoption d'une base héritée ? |
| `banc_recherche` | recherche et ouverture tiennent-elles leurs budgets ? |

En écrire un nouveau coûte 40 lignes et fait gagner un aller-retour.

### Un test vert peut encoder un modèle FAUX de l'autre écrivain

La détection de conflit des brouillons était éprouvée en simulant le
tirage par une *réécriture en place*. Le vrai tirage **remplace** : il
retire la ligne et en importe une neuve. La ligne visée ayant disparu, la
détection ne comparait plus qu'un horodatage et se taisait.

**Simuler l'autre écrivain en appelant SON VRAI CHEMIN**, jamais par une
approximation qui lui ressemble.

### Une promesse d'index ne vaut que pour la requête qu'on avait en tête

L'ADR 0008 §4 raisonnait sur une boîte ; le produit interroge la boîte
unifiée. SQLite matérialisait le tri de 160 000 conversations à chaque
page — 987 ms, invisible à l'échelle du terrain.

**Un test de PLAN D'EXÉCUTION attrape cette classe de régression** : une
durée dépend de la machine, un plan non.

### Un décor de mesure peut ne jamais exercer ce qu'on croit valider

L'index partiel `WHERE inbox_size > 0` a vécu plusieurs jours sans qu'un
seul fil ne soit jamais écarté : le décor du gate 3 n'avait qu'une boîte
par compte. **Vérifier que le décor produit la condition que le code
prétend traiter**, sinon la mesure ne prouve rien.

### Un test qui ne tourne pas n'est pas un test

`cargo test --workspace` ignore les tests des exemples. Deux tests écrits
et verts n'auraient jamais été exécutés par le gate. Voir §7.4.

### Le compilateur ne protège pas une identité faite de chaînes

`account_id` et `mailbox_id` sont tous deux des `i64` ; une boîte est une
`String` comme une autre. Après un changement de signature, le code
**compilait** en visant le mauvais message. Reprendre les appelants un par
un, et tenir l'invariant par un test.

### Un signal demandé doit être OBSERVABLE

Plusieurs consignes de validation demandaient de constater un signal que
l'interface ne produit pas — deux brouillons rigoureusement identiques à
l'écran parce que le bandeau n'affichait pas le corps. **Vérifier dans le
code que chaque signal demandé est réellement affiché, et qu'il n'est pas
écrasé une ligne plus loin.**

### Un statut posé sans regarder en efface un autre

Trois fois. La dernière : l'avertissement de conflit recouvert par le
bilan de la poussée, revenu du réseau une seconde plus tard — et la
collision était **certaine**, le brouillon conservé à part étant neuf donc
toujours à pousser. Quand une fonction pose un message d'état, l'appelant
doit **décider** du sien à partir de son bilan.

### Ne jamais avaler une erreur

`let _ = …show()` sur les notifications a protégé la synchro et détruit la
preuve : le symptôme était « rien ne se passe », indiagnosticable. Les
échecs non bloquants remontent dans le bilan de synchro.

### Un outil de mesure se vérifie comme le reste

`mesure-ram.ps1` sommait toutes les instances de l'application — 202 Mo
pour deux applications additionnées, annonçant un budget dépassé qui tient
largement. `mesure.mjs` n'isolait pas son profil WebView2. Et un
diagnostic divulguait en clair les identifiants qu'il promettait de
masquer, parce qu'il découpait un en-tête `References` entier sur son
premier `@`.

---

## 10. Carte des fichiers

| Fichier | Rôle |
|---|---|
| [`docs/PLAN.md`](PLAN.md) | Concept paper — source de vérité produit |
| [`docs/adr/`](adr/) | Les 9 décisions gelées |
| [`docs/PHASE0.md`](PHASE0.md) → [`PHASE3.md`](PHASE3.md) | Revues de clôture (décisions, budgets, enseignements) |
| [`crates/mail-core/src/store.rs`](../crates/mail-core/src/store.rs) | Stockage SQLite, schéma, migrations, boîte unifiée |
| [`crates/mail-core/src/sync.rs`](../crates/mail-core/src/sync.rs) | Moteur de synchro (contre `FakeServer`) |
| [`crates/mail-core/src/thread.rs`](../crates/mail-core/src/thread.rs) | Conversations : union-find pur + persistance, portée compte |
| [`crates/mail-core/src/drafts.rs`](../crates/mail-core/src/drafts.rs) | Brouillons : poussée, tirage, conflit d'édition |
| [`crates/mail-core/src/outbox.rs`](../crates/mail-core/src/outbox.rs) | Boîte d'envoi + règles d'or |
| [`crates/mail-core/src/search.rs`](../crates/mail-core/src/search.rs) | Index FTS5 contentless, transactionnel |
| [`crates/mail-core/src/backfill.rs`](../crates/mail-core/src/backfill.rs) | Rattrapage des corps ET passe d'en-têtes de fils |
| [`crates/mail-core/src/test_support.rs`](../crates/mail-core/src/test_support.rs) | `FakeServer` — rejoue les bizarreries du terrain |
| [`crates/mail-core/examples/`](../crates/mail-core/examples/) | 3 diagnostics + 3 bancs + `seed_inbox` |
| [`crates/mail-imap/src/convert.rs`](../crates/mail-imap/src/convert.rs) | Traduction IMAP → domaine ; découverte archive et envois |
| [`crates/mail-auth/src/provider.rs`](../crates/mail-auth/src/provider.rs) | Fournisseurs OAuth décrits **en données** |
| [`apps/desktop/src/commands.rs`](../apps/desktop/src/commands.rs) | Commandes Tauri (IPC), boucles par compte |
| [`apps/desktop/ui/app.js`](../apps/desktop/ui/app.js) | UI : liste virtualisée, composeur, raccourcis |
| [`e2e/README.md`](../e2e/README.md) | Harnais E2E déterministe (CDP) |

---

*Vos mails, instantanément. La performance et la fiabilité ne sont pas des
options — ce sont les fonctionnalités.*
