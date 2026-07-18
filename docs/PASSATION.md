# Passation du développement — Discovery

> Document de relais destiné à **Kimi K3** (ou tout successeur) reprenant le
> développement de Discovery. Il transmet quatre choses : **comment le
> produit est conçu**, **où le travail s'est arrêté exactement**, **les
> prochaines étapes détaillées**, et **la méthode de travail à appliquer**.
>
> Rédigé le 2026-07-18. Branche `main`, dernier commit livré
> `6b94741` (fondation multi-comptes). Un chantier est **en vol** : la
> recherche FTS5 — voir §7 pour son état exact.

---

## 0. Comment lire ce document

Lis-le dans l'ordre, une fois, en entier, avant de toucher au code. Puis :

1. Ouvre [`docs/PLAN.md`](PLAN.md) — le **concept paper** du Chef Ingénieur.
   C'est la source de vérité produit. Tout le reste en découle.
2. Parcours les quatre ADRs dans [`docs/adr/`](adr/) — ce sont des
   **décisions gelées**. On ne les rouvre pas sans une raison mesurée.
3. Reviens ici pour §7 (où on s'est arrêté) et §8 (prochaines étapes).

Le code est en **Rust** (workspace Cargo, édition 2024). L'UI est en
**JavaScript/HTML/CSS vanilla** (pas de framework). Les tests E2E sont en
**Playwright** (`e2e/`). Tout — commits, UI, docs, commentaires de code —
est en **français**. Ce n'est pas cosmétique : c'est la langue du client
cible et du Chef Ingénieur.

---

## 1. Le produit en une page

**Promesse :** *« Vos mails, instantanément. »* Un client email qui démarre
en moins d'une seconde, où chaque action répond en moins de 100 ms, et qui
fonctionne hors-ligne comme en ligne.

**Client cible :** professionnel ou particulier exigeant, 1 à 4 comptes
(Gmail, puis Outlook/Microsoft 365, puis IMAP générique).

**Ce que le produit EST :** rapide (la performance est LA fonctionnalité),
simple (lire, trier, chercher, écrire — rien d'autre), fiable (jamais de
perte, jamais d'envoi fantôme, offline-first), sûr (credentials dans le
coffre de l'OS, HTML assaini, images distantes bloquées par défaut).

**Ce que le produit N'EST PAS (v1) :** pas de calendrier, pas de chat, pas
d'IA intégrée, pas de plugins, pas de mobile. **Chaque ajout se paie en
vitesse et en fiabilité — le réflexe par défaut est de dire non.**

**Budgets chiffrés — ce sont des gates BLOQUANTS** (§1 du PLAN) :

| Métrique | Cible | Statut mesuré |
|---|---|---|
| Démarrage à froid | < 1 s | 350 ms ✅ |
| Ouverture d'un message | < 50 ms | ✅ |
| Recherche sur 100 000 messages | < 100 ms | à mesurer (chantier en cours) |
| RAM en usage courant | < 200 Mo | 89,6 Mo ✅ |
| Perte de données | 0, prouvé par crash-récup | ✅ (Phase 2) |

Un budget dépassé = **on arrête la ligne** (andon). On ne « livre puis
optimise » pas : la performance est une contrainte de conception, pas une
étape ultérieure.

---

## 2. La méthode de travail — Chief Engineer Toyota

C'est **l'instruction permanente** qui prime sur tout le reste. Le
développement de Discovery suit la discipline du *shusa* (Chef Ingénieur)
de Toyota. Concrètement, pour chaque incrément :

### 2.1 Front-loading — résoudre les points durs AVANT de coder
Les problèmes difficiles se règlent par un **spike jetable et mesuré**,
pas en pleine implémentation. Exemples faits : le moteur de synchro, le
pont web, le rendu HTML, OAuth (Phase 0), et **le moteur de recherche**
(spike FTS5 vs Tantivy, [`spikes/search-engine/`](../spikes/search-engine/)
→ [ADR 0004](adr/0004-moteur-de-recherche-fts5.md)). Un spike vit hors du
workspace de production et n'entre jamais dans le `Cargo.lock` de prod.

### 2.2 Set-based concurrent engineering — explorer puis éliminer
On explore plusieurs options **en parallèle** et on converge **par
élimination sur des critères mesurés — des chiffres, pas des avis**. La
règle de départage : l'alternative doit battre l'hypothèse *nettement*
pour la déloger. C'est pourquoi FTS5 a battu Tantivy alors même que
Tantivy est plus rapide en requête pure : FTS5 gagne sur la
transactionnalité et le coût incrémental, qui structurent un client
offline-first (voir l'ADR 0004, c'est le modèle de raisonnement à imiter).

### 2.3 Jidoka — construire la qualité dans le processus
- **TDD systématique** : le test échoue (RED) avant l'implémentation
  (GREEN), puis on refactore. Le moteur se teste contre un **serveur IMAP
  simulé** (`FakeServer`, dans [`crates/mail-core/src/test_support.rs`](../crates/mail-core/src/test_support.rs)).
- **Couverture ≥ 80 %** sur `mail-core`.
- **Gate qualité obligatoire avant chaque commit** : `cargo fmt`,
  `cargo clippy -D warnings`, tous les tests Rust verts, E2E verts.
  Un warning clippy = build rouge. Zéro `unwrap()`/`expect()` en
  production (lint `warn` au niveau workspace ; `allow-unwrap-in-tests`
  dans [`clippy.toml`](../clippy.toml)).
- **Erreurs typées** (`thiserror`) dans les crates, `anyhow` dans les apps.

### 2.4 Genchi genbutsu — aller voir sur le terrain
Chaque incrément est **validé sur le compte Gmail réel de
l'utilisateur**, pas seulement en tests. L'utilisateur joue le scénario
(envoyer, couper le wifi, redémarrer, chercher) et rapporte ce qu'il
observe. **Les bugs trouvés sur le terrain se corrigent le jour même**
(kaizen, lead time de correction < 48 h). Exemple réel : le « double
brouillon » signalé par l'utilisateur a révélé deux causes racines
(tombstones purgés trop tard + epoch strictement monotone sur sauvegarde
identique) — corrigées et prouvées par test le jour même.

### 2.5 Refus de périmètre explicites
Quand une fonctionnalité serait un « fantôme » (résultat invisible, ou
qui exige une brique absente), on la **reporte explicitement et on écrit
pourquoi** dans la revue de clôture de phase. Exemple : « déplacer vers un
dossier » a été reporté en Phase 2 car il n'y avait pas de navigation de
dossiers — le résultat aurait été invisible (voir [PHASE2.md](PHASE2.md) §4).

### 2.6 Cadence et traçabilité (obeya)
- Toute décision structurante = **un ADR court** dans `docs/adr/`.
- Chaque fin de phase = **une revue de clôture** (`docs/PHASEn.md`) :
  livré contre le plan, budgets re-mesurés, enseignements, reports
  assumés, décision GO/NO-GO.
- Les commits sont en français, format `type: description` (types :
  `feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `perf`, `ci`).
  **Pas de `Co-Authored-By`** (attribution désactivée côté utilisateur).

### 2.7 Quand s'arrêter et demander
Agir sans demander pour tout ce qui est réversible et découle de la
demande. **S'arrêter et demander** seulement pour : une action
destructrice, un vrai changement de périmètre, ou une décision produit
qui appartient au Chef Ingénieur (l'utilisateur). Ne jamais bloquer le
travail sur une question à laquelle le code ou le plan répond déjà.

---

## 3. Architecture — « un seul cerveau »

Le principe directeur (§3 du PLAN) : **`mail-core` contient 100 % de la
logique métier, de la synchro et du stockage.** Le desktop l'embarque en
processus ; le web (Phase 4, pas encore commencé) l'exécutera côté
serveur. L'UI reste « bête » : elle affiche un état et émet des intentions.

### 3.1 Le workspace Cargo

```
discovery/
├── crates/
│   ├── mail-core/     # domaine + synchro + stockage + recherche (ZÉRO dépendance UI/réseau)
│   ├── mail-imap/     # adaptateur IMAP réel (implémente le trait MailServer)
│   ├── mail-auth/     # OAuth2 PKCE loopback + coffre Windows (keyring)
│   ├── mail-render/   # assainissement HTML (ammonia) + extraction texte + document CSP
│   └── mail-smtp/     # adaptateur SMTP (lettre, XOAUTH2) — implémente MailTransport
├── apps/
│   └── desktop/       # Tauri 2 : commands.rs (IPC) + main.rs (état) + ui/ (JS vanilla)
├── e2e/               # Playwright pilotant la vraie fenêtre via CDP WebView2
├── spikes/            # prototypes jetables hors workspace de prod (search-engine, web-bridge)
└── docs/              # PLAN, revues de phase, ADRs, ce document
```

Manifeste : [`Cargo.toml`](../Cargo.toml) (workspace, édition 2024,
`unsafe_code = "forbid"`).

### 3.2 La seule frontière abstraite
`mail-core` ne connaît ni Tauri, ni le web, ni IMAP. Sa **seule** frontière
abstraite est le trait `MailServer` (lecture) et le port `MailTransport`
(envoi), dans [`crates/mail-core/src/remote.rs`](../crates/mail-core/src/remote.rs)
et [`transport.rs`](../crates/mail-core/src/transport.rs). Les adaptateurs
réels (`mail-imap`, `mail-smtp`) vivent hors du noyau. **SQLite n'est PAS
derrière un trait** : c'est une décision produit gelée (les tests utilisent
une base en mémoire), donc `Store` est une struct concrète.

### 3.3 Modèle de données (inspiré JMAP, plus sain qu'IMAP)
`Account`, `Mailbox`, `Envelope` (enveloppe **séparée** du corps),
`PendingAction`, `Draft`/`SavedDraft`, `OutboxMessage`. Synchro
**« enveloppes d'abord »** : la liste est utilisable immédiatement, les
corps sont chargés à la demande au clic puis mis en cache offline
([`body.rs`](../crates/mail-core/src/body.rs)).

### 3.4 L'invariant multi-comptes (fondation livrée en `6b94741`)
Depuis la Phase 3, **tout est relatif à un compte**. L'identité d'un
message est le couple **`(account_id, uid)`** — un UID seul n'identifie
plus rien (deux comptes peuvent partager un UID). Points clés dans
[`store.rs`](../crates/mail-core/src/store.rs) :
- table `accounts`, `mailboxes(account_id, UNIQUE(account_id, name))` ;
- `adopt_or_create_account(email, provider)` : la migration Phase 2→3 crée
  un compte « en attente » (`email = ''`) que la **première connexion
  revendique** — les données Phase 2 de l'utilisateur sont ainsi adoptées
  sans perte, prouvé par test sur une base Phase 2 reconstruite ;
- `unified_recent(mailbox, offset, limit)` : la **boîte unifiée**, tous les
  INBOX fusionnés par date, chaque `UnifiedRow` portant son `account_id` +
  `account_email` ;
- les boucles (synchro, vidange d'envoi, poussée de brouillons) tournent
  **par compte** ; l'échec d'un compte ne bloque jamais les autres
  (`apps/desktop/src/commands.rs`).

Coffre : **une entrée keyring par email** (`gmail-refresh:{email}`), avec
reprise transparente de l'entrée héritée mono-compte
([`crates/mail-auth/src/lib.rs`](../crates/mail-auth/src/lib.rs)).

---

## 4. Décisions gelées — les ADRs (ne pas rouvrir sans mesure)

| ADR | Décision | Essentiel à retenir |
|---|---|---|
| [0001](adr/0001-structure-workspace.md) | Workspace Cargo multi-crates | `mail-core` sans dépendance UI/réseau ; frontière = traits |
| [0002](adr/0002-shell-desktop-tauri.md) | Shell desktop = Tauri 2 (WebView2) | La RAM qui fait foi = working set **privé**, pas le commit |
| [0003](adr/0003-boite-envoi-smtp.md) | Boîte d'envoi SMTP + règles d'or | Journal AVANT réseau ; quarantaine anti-fantôme ; texte brut assumé (v1) |
| [0004](adr/0004-moteur-de-recherche-fts5.md) | Recherche = SQLite **FTS5** (Tantivy en plan B chiffré) | L'index vit DANS la base (transactionnel) ; `unicode61 remove_diacritics 2` ; garde-fous requêtes larges |

Décisions gelées issues de la Phase 0 ([PHASE0.md](PHASE0.md) §2) : SQLite
local ; CONDSTORE pour la détection de changements (Gmail n'expose pas
QRESYNC) ; parsing MIME par `mail-parser` (Stalwart) ; OAuth2 PKCE loopback
+ coffre OS ; architecture « un seul cerveau » ; rendu HTML en défense en
profondeur (assainissement + blocage images + sandbox/CSP, données dans le
DOM via `textContent` uniquement).

---

## 5. Invariants non négociables (facile à casser — vérifier à chaque revue)

1. **Boîte d'envoi — les deux règles d'or** (ADR 0003, `outbox.rs`) :
   - *jamais d'envoi perdu* : l'intention est journalisée dans SQLite AVANT
     toute tentative réseau, le Message-ID est généré AVANT le réseau ;
   - *jamais d'envoi fantôme* : un envoi interrompu en plein vol part en
     **quarantaine** (`interrupted`) et n'est **JAMAIS** renvoyé
     automatiquement. « Le doublon est pire que le retard. »
2. **Identité message = `(account_id, uid)`** partout, jusque dans la
   sélection de l'UI. Ne jamais retomber sur un UID seul.
3. **L'index de recherche vit DANS la base** : il s'entretient dans la MÊME
   transaction que l'insertion/suppression du message. Pas de second
   magasin, pas de réconciliation après crash (c'est tout l'argument de
   l'ADR 0004 contre Tantivy).
4. **Sécurité du rendu** : HTML assaini par `ammonia`, images distantes
   bloquées par défaut, iframe sandboxée + CSP, jamais d'exécution de JS
   des mails. Données de mail injectées via `textContent`, jamais
   `innerHTML`.
5. **Credentials jamais en clair** : tokens dans le Credential Manager
   Windows via `keyring`. Aucun secret dans le code ni les logs.
6. **UIDVALIDITY** : si elle change, on repart de zéro pour cette boîte
   (`reset_mailbox`) — un UID invalidé ne veut plus rien dire. Règle
   brouillons : « un doublon est acceptable, supprimer le mauvais UID
   jamais ».

---

## 6. État d'avancement (phases)

- **Phase 0 — Kentou** : ✅ close ([PHASE0.md](PHASE0.md)). 4 spikes mesurés,
  décisions gelées.
- **Phase 1 — « je lis mes mails »** : ✅ close ([PHASE1.md](PHASE1.md)).
  OAuth, synchro enveloppes→SQLite, liste virtualisée, lecture HTML sûre.
  Budgets tenus sur 50 000 messages réels.
- **Phase 2 — « je travaille dans mes mails »** : ✅ close ([PHASE2.md](PHASE2.md)).
  Actions optimistes + file offline, composer/répondre/transférer, boîte
  d'envoi SMTP, brouillons synchronisés, raccourcis clavier. Zéro perte
  prouvée par coupure/crash (tests **et** terrain). 5 parcours E2E.
- **Phase 3 — « recherche, multi-comptes, échelle »** : 🔶 EN COURS.
  - ✅ Spike recherche + ADR 0004 (moteur gelé sur mesures).
  - ✅ **Fondation multi-comptes** (commit `6b94741`) : boîte unifiée, N
    comptes Gmail, migration sans perte, coffre par email. 136 tests Rust,
    7/7 E2E, clippy muet.
  - 🔶 **Recherche en production FTS5** : EN VOL — voir §7.
  - ⬜ Reste : Microsoft/Graph + IMAP générique, pièces jointes,
    notifications Windows, threading, dossiers/déplacer, tirage des
    brouillons. **Gate 3 : budgets tenus avec 3 comptes / 200 000 messages
    cumulés.**
- **Phases 4 (web) et 5 (durcissement/bêta)** : ⬜ non commencées.

---

## 7. OÙ ON S'EST ARRÊTÉ EXACTEMENT

Chantier en cours : **la recherche plein-texte FTS5 en production**
(tâche #31). Le raisonnement de conception est figé par l'ADR 0004 ;
l'implémentation est **partiellement écrite mais NON terminée et NON
compilable en l'état**.

### 7.1 Ce qui existe
Un fichier **non suivi par git** : [`crates/mail-core/src/search.rs`](../crates/mail-core/src/search.rs).
Il contient, rédigé et commenté :
- `migrate_search(conn)` : crée `search_docs` (docid stable par
  `(mailbox_id, uid)`, car les rowids d'`envelopes` sont instables via
  `INSERT OR REPLACE`) + la table virtuelle FTS5 **sans contenu**
  (`content=''`, `contentless_delete=1`, `tokenize='unicode61
  remove_diacritics 2'`), puis reconstruit l'index depuis les messages
  déjà en base ;
- `index_message` / `deindex_message` / `deindex_mailbox` : l'entretien de
  l'index, à appeler **dans la transaction** qui écrit le message ;
- `Store::search(input, limit) -> Vec<UnifiedRow>` : requête BM25 (le sujet
  pèse plus que le corps), **dernier terme en préfixe** (search-as-you-type),
  saisie **neutralisée** (jamais interprétée comme syntaxe FTS5), filtres
  `from:`/`de:` et `date:AAAA[-MM[-JJ]]` ;
- `indexable_text(html)` : réduction HTML → mots (balises et
  `<script>`/`<style>` retirés, entités décodées dont accents français,
  blancs effondrés) ;
- **~20 tests unitaires** couvrant : recherche multi-comptes, repli des
  accents dans les deux sens, préfixe du dernier terme, corps indexé sans
  le balisage, entités HTML, ranking sujet > corps, réindexation sur
  ré-upsert, nettoyage de l'index sur suppression locale/absente/reset
  UIDVALIDITY, saisie hostile littérale, filtres from/date, requête vide,
  LIMIT.

### 7.2 Ce qui MANQUE pour compiler et fonctionner (le raccordement)
`search.rs` référence des symboles qui n'existent pas encore et n'est
appelé nulle part. **Étape 1 des prochaines étapes = ce raccordement.**
Précisément, dans [`crates/mail-core/src/store.rs`](../crates/mail-core/src/store.rs) :

1. **Extraire deux symboles `pub(crate)`** que `search.rs` importe :
   - `pub(crate) const SELECT_UNIFIED: &str` = la liste de colonnes du
     SELECT de la boîte unifiée (`a.id, a.email, e.uid, e.subject,
     e.sender, e.sender_address, e.message_id, e.date_epoch, e.seen,
     e.flagged`) — sans le `FROM`, pour que `search()` réutilise le même
     mapping ;
   - `pub(crate) fn row_to_unified(row) -> rusqlite::Result<UnifiedRow>` :
     extraire la closure actuellement inline dans `unified_recent` en
     fonction nommée, et refactorer `unified_recent` pour l'utiliser (comme
     `row_to_envelope` l'est déjà pour les enveloppes).
2. **Appeler l'index dans chaque point de mutation**, dans la transaction
   existante (le trait `Transaction` déréférence vers `Connection`, donc
   `&tx` passe là où `&Connection` est attendu) :
   - `upsert_envelopes` : pour chaque enveloppe, récupérer le corps HTML
     déjà en cache (`bodies`) et appeler `search::index_message(...)` avec
     sujet/expéditeur/adresse/corps. ⚠️ le test `reupsert_keeps_the_indexed_body`
     exige que ré-écrire une enveloppe **préserve** le corps déjà indexé ;
   - `save_body` : réindexer le message (relire sujet/expéditeur de
     `envelopes`, passer le nouveau corps) ;
   - `remove_local` : `search::deindex_message(...)` ;
   - `remove_absent` : `search::deindex_message(...)` pour chaque UID
     obsolète, dans la transaction ;
   - `reset_mailbox` : `search::deindex_mailbox(...)`.
3. **Brancher la migration** : appeler `search::migrate_search(conn)` à la
   fin de `migrate(conn)`.
4. **Déclarer le module** dans [`crates/mail-core/src/lib.rs`](../crates/mail-core/src/lib.rs) :
   ajouter `mod search;` (aucun nouvel export public nécessaire : `search()`
   est une méthode de `Store`, déjà publique).

Une fois raccordé : `cargo test -p mail-core` doit passer au vert (les ~20
tests de `search.rs` valident le comportement). **Ne pas committer avant
le vert complet + clippy muet.**

### 7.3 État git
`git status` : seul `crates/mail-core/src/search.rs` est non suivi. Rien
d'autre n'est modifié. La branche `main` est propre au commit `6b94741`.
Le tableau des tâches : #1–#30 complétées, **#31 en cours** (cette
recherche), #32–#34 en attente.

---

## 8. Prochaines étapes détaillées (dans l'ordre)

### Étape 1 — Terminer le raccordement de l'index (tâche #31)
Faire le §7.2 ci-dessus. TDD : les tests existent déjà dans `search.rs`,
ils sont le RED ; le raccordement est le GREEN. Vérifier
`cargo test -p mail-core` + `cargo clippy -p mail-core -- -D warnings`.

### Étape 2 — Finaliser l'API de recherche si besoin (tâche #32)
L'API `Store::search` est écrite. Vérifier qu'elle couvre le plan (§4 :
« filtres from/to/date/pièce attachée »). `from:`/`de:` et `date:` sont
faits ; `to:` et `a:pièce-jointe` sont **à ajouter seulement quand ils ont
un sens produit** (le filtre pièce jointe attendra la fonctionnalité pièces
jointes — sinon c'est un filtre fantôme, cf. §2.5). Garde-fou requêtes
larges de l'ADR 0004 : LIMIT côté noyau (fait) + **déclenchement à ≥ 3
caractères + debounce côté UI** (étape 3).

### Étape 3 — Brancher la recherche dans l'UI desktop (tâche #33)
1. **Commande Tauri** dans [`apps/desktop/src/commands.rs`](../apps/desktop/src/commands.rs) :
   ajouter `search_messages(app, query: String) -> Result<Vec<MessageRow>, String>`
   qui ouvre le `Store` et appelle `store.search(&query, LIMIT)`, en
   réutilisant le mapping `MessageRow` existant (celui de `list_messages`,
   qui porte déjà `account_id` + `account_email` pour les pastilles).
2. **L'enregistrer** dans le `generate_handler![…]` de
   [`apps/desktop/src/main.rs`](../apps/desktop/src/main.rs) (sinon l'invoke
   échoue à l'exécution — la liste est explicite, ne pas l'oublier).
3. **UI** dans [`apps/desktop/ui/app.js`](../apps/desktop/ui/app.js) +
   [`index.html`](../apps/desktop/ui/index.html) + [`style.css`](../apps/desktop/ui/style.css) :
   - raccourci **`/`** ouvre un champ de recherche (respecter la garde de
     saisie existante : `/` ne doit pas déclencher quand on tape dans le
     composer) ;
   - **debounce** (~150 ms) + déclenchement à partir de **3 caractères** ;
   - la liste affiche les résultats en **conservant les pastilles de
     compte** (réutiliser `buildRow`) ;
   - **Échap** vide la recherche et revient à la boîte unifiée ;
   - ouvrir un résultat = lecture normale (le `(account_id, uid)` est déjà
     porté par la ligne).
4. Respecter les règles UI globales : données via `textContent`, jamais
   `innerHTML` ; pas de dépendance externe.

### Étape 4 — Gate qualité + mesure + commit (tâche #34)
1. **E2E** : ajouter un parcours dans [`e2e/tests/`](../e2e/tests/) (taper
   `/`, saisir une requête, voir un résultat, l'ouvrir, Échap revient à la
   liste). Le harnais E2E seed une base jetable — voir `e2e/launch.mjs` et
   `e2e/README.md`. Les E2E pilotent la **vraie fenêtre** via CDP (pas de
   tauri-driver) ; ils sont déterministes par construction.
2. **Mesurer** le surcoût de l'index : taille disque de l'index sans
   contenu (la vigilance « table external content » de l'ADR 0004) et coût
   incrémental sur la synchro (l'ADR mesurait 25–36 ms / 500 docs au
   spike). Viser la mesure sur volume réaliste ; consigner les chiffres.
3. **Gate complet** : `cargo fmt`, `cargo clippy --all-targets -- -D
   warnings`, `cargo test` (workspace), `cd e2e && npm test`.
4. **Commit** français, ex. `feat: phase 3 — recherche plein-texte FTS5
   (boîte unifiée cherchable, filtres from/date)`.
5. Proposer la **validation terrain** à l'utilisateur (chercher dans ses
   vrais mails, vérifier < 100 ms ressenti, accents, préfixe pendant la
   frappe).

### Étape 5 et au-delà — backlog Phase 3 (ordre gelé)
Après la recherche, dans l'ordre du plan et des reports assumés :
1. **Microsoft/Graph + IMAP générique** (nouveaux providers derrière les
   traits `MailServer`/`MailTransport`). ⚠️ l'enregistrement **Azure AD**
   est une action **produit-owner** (voir §10), sur le chemin critique.
2. **Pièces jointes** (puis le filtre de recherche « a une pièce jointe »).
3. **Notifications Windows.**
4. **Threading des conversations.**
5. **Dossiers / déplacer** (débloque le report Phase 2) et **tirage des
   brouillons** (éditer ici un brouillon créé ailleurs).
6. **Clôture Phase 3** : `docs/PHASE3.md` (revue de clôture), gate 3 —
   budgets tenus avec **3 comptes / 200 000 messages cumulés**.

---

## 9. Environnement & commandes (⚠️ pièges Windows/PowerShell)

**Plateforme :** Windows 11. Deux shells disponibles : **PowerShell 5.1**
(principal) et **Bash** (Git Bash, POSIX). Ils n'ont pas la même syntaxe.

### 9.1 Pièges à connaître absolument
- **PowerShell 5.1 n'a PAS l'opérateur `&&`.** Écrire les commandes sur des
  lignes séparées, ou les chaîner en Bash. `cd e2e && npm test` échoue en
  PowerShell → utiliser `cd e2e; npm test` ou le shell Bash.
- **Ne JAMAIS utiliser `Get-Content`/`Set-Content` sur les fichiers source**
  (risque de mojibake : PowerShell 5.1 réencode en UTF-16 avec BOM et
  corrompt les accents). Éditer via l'outil `Edit`, Python, ou Bash. Voir
  la mémoire projet `powershell-51-encodage-utf8`.
- Le tout est en **UTF-8** (accents français partout).

### 9.2 Commandes de développement
```bash
# Compiler / tester le noyau
cargo test -p mail-core
cargo test                       # tout le workspace
cargo build -p discovery-desktop --release   # binaire desktop

# Gate qualité (obligatoire avant commit)
cargo fmt
cargo clippy --all-targets -- -D warnings

# Lancer l'app desktop (validation terrain)
cargo run -p discovery-desktop --release

# E2E (depuis e2e/, PowerShell : deux lignes)
cd e2e
npm test

# Seed d'une base de test (corps de messages inclus)
cargo run -p mail-core --example seed_inbox -- <db> <count> <email>

# Mesures de budget
node e2e/mesure.mjs             # démarrage + page de liste
# RAM : e2e/mesure-ram.ps1 (working set privé, filtre msedgewebview2 discovery)
```

### 9.3 Déterminisme des E2E
Les E2E sont étanches par construction (voir [`e2e/README.md`](../e2e/README.md)) :
base SQLite jetable via `DISCOVERY_DB_PATH`, comptes factices via
`DISCOVERY_E2E_ACCOUNT` (liste d'emails séparés par des virgules, jetons
invalides), et `GOOGLE_CLIENT_ID`/`GOOGLE_CLIENT_SECRET` retirés de
l'environnement du process lancé. WebView2 est piloté par
`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222` +
Playwright `connectOverCDP`.

---

## 10. Contraintes produit-owner (hors code, chemin critique)

Certaines actions n'appartiennent pas au développeur mais à l'utilisateur
(Chef Ingénieur / product-owner) :

- **Audit CASA Google** (scopes restreints Gmail) : long et coûteux, sur le
  chemin critique du **lancement public**. Le projet Google Cloud est
  actuellement en **mode Testing** → refresh tokens valides **7 jours**,
  100 testeurs max. C'est une contrainte de développement acceptée, pas un
  bug. (Mémoire projet `discovery-phase0-oauth-valide`.)
- **Ajouter un 2ᵉ compte Gmail en test** : en mode Testing, le compte doit
  d'abord être inscrit comme **utilisateur de test** sur l'écran de
  consentement OAuth, sinon Google refuse le consentement.
- **Enregistrement Azure AD** (app Microsoft 365) : préalable à
  l'implémentation du provider Microsoft (backlog Phase 3, §8 étape 5).

---

## 11. Carte des fichiers (points d'entrée les plus utiles)

| Fichier | Rôle |
|---|---|
| [`docs/PLAN.md`](PLAN.md) | Concept paper — source de vérité produit |
| [`docs/PHASE0-2.md`](.) | Revues de clôture (décisions, budgets, enseignements) |
| [`docs/adr/`](adr/) | Décisions gelées (workspace, Tauri, boîte d'envoi, FTS5) |
| [`crates/mail-core/src/store.rs`](../crates/mail-core/src/store.rs) | Stockage SQLite, schéma, migrations, boîte unifiée |
| [`crates/mail-core/src/search.rs`](../crates/mail-core/src/search.rs) | **Recherche FTS5 — en vol, à raccorder (§7)** |
| [`crates/mail-core/src/sync.rs`](../crates/mail-core/src/sync.rs) | Moteur de synchro (contre `FakeServer`) |
| [`crates/mail-core/src/outbox.rs`](../crates/mail-core/src/outbox.rs) | Boîte d'envoi + règles d'or |
| [`crates/mail-core/src/drafts.rs`](../crates/mail-core/src/drafts.rs) | Brouillons locaux + poussée + tombstones |
| [`crates/mail-core/src/lib.rs`](../crates/mail-core/src/lib.rs) | Exports publics du noyau |
| [`crates/mail-auth/src/lib.rs`](../crates/mail-auth/src/lib.rs) | OAuth PKCE + coffre par email |
| [`apps/desktop/src/commands.rs`](../apps/desktop/src/commands.rs) | Commandes Tauri (IPC), boucles par compte |
| [`apps/desktop/src/main.rs`](../apps/desktop/src/main.rs) | État app + `generate_handler!` |
| [`apps/desktop/ui/app.js`](../apps/desktop/ui/app.js) | UI (liste, pastilles, composer, raccourcis) |
| [`e2e/README.md`](../e2e/README.md) | Harnais E2E déterministe (CDP) |
| [`spikes/search-engine/`](../spikes/search-engine/) | Banc FTS5 vs Tantivy (re-mesurable) |

---

## 12. Résumé pour démarrer vite

1. Lis le PLAN et les 4 ADRs. Intègre la **méthode Toyota** (§2) : elle
   prime sur tout.
2. **Reprends la tâche #31** : raccorde `search.rs` à `store.rs` (§7.2).
   Objectif immédiat : `cargo test -p mail-core` au vert.
3. Puis l'UI de recherche (§8 étape 3), le gate + mesures + commit (étape 4).
4. Travaille par **petits incréments testés**, valide **sur le terrain**
   avec l'utilisateur, corrige les retours **le jour même**, et **dis non**
   à toute dérive de périmètre. Chaque décision structurante = un ADR ;
   chaque fin de phase = une revue de clôture.

*Vos mails, instantanément. La performance et la fiabilité ne sont pas des
options — ce sont les fonctionnalités.*
