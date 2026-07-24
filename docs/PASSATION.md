# Passation du développement — Discovery

> Document de relais destiné à **Kimi K3** (ou tout successeur) reprenant le
> développement de Discovery. Il transmet quatre choses : **comment le
> produit est conçu**, **où le travail s'est arrêté exactement**, **les
> prochaines étapes détaillées**, et **la méthode de travail à appliquer**.
>
> Rédigé le 2026-07-18, **remis à jour le 2026-07-25**. Branche `main`,
> dernier commit livré `a738246`. Le code est **vert et poussé** ; ce qui
> est en vol est une **validation terrain**, pas du code — voir §7.

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
| [0008](adr/0008-regroupement-en-conversations.md) | Conversations = union-find sur en-têtes RFC 5322 | JAMAIS de repli par sujet ; agrégat matérialisé recalculé, jamais incrémenté ; `In-Reply-To` gratuit, `References` par passe bornée |

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
  - ✅ **Recherche en production FTS5**, complétée par le **rattrapage des
    corps** ([ADR 0007](adr/0007-rattrapage-des-corps.md)) : sans lui, la
    recherche « plein-texte » ne portait que sur les sujets. Validé sur le
    terrain — base de 97 Mo pour ~2 730 messages, budget < 1 Go tenu à 10 %.
  - ✅ **IMAP générique** puis **Microsoft 365** : la couche OAuth se décrit
    désormais par fournisseur en données (`mail-auth::provider`), et les
    deux parcours sont validés sur des comptes réels — envoi compris.
  - ✅ **Pièces jointes** (lecture), **notifications Windows**,
    **dossiers/déplacer**, **regroupement en conversations**
    ([ADR 0008](adr/0008-regroupement-en-conversations.md)) — tous validés
    sur les comptes réels.
  - 🔶 **Tirage des brouillons** : code livré et vert, **validation terrain
    inachevée** (§7).
  - ⬜ **Gate 3** : budgets tenus avec 3 comptes / 200 000 messages
    cumulés, puis revue de clôture `docs/PHASE3.md`.
- **Phases 4 (web) et 5 (durcissement/bêta)** : ⬜ non commencées.

---

## 7. OÙ ON S'EST ARRÊTÉ EXACTEMENT

**Aucun code n'est en vol.** `main` est vert au commit `a738246` :
292 tests Rust, 18/18 E2E, clippy muet, gate pré-push passé. Ce qui est
inachevé, c'est une **boucle de validation terrain** avec le Chef
Ingénieur (l'utilisateur), sur le dernier chantier livré.

### 7.1 Le chantier : tirage des brouillons

Éditer dans Discovery un brouillon commencé ailleurs (webmail, téléphone).
La synchronisation des brouillons était à sens unique — on poussait, on ne
tirait pas.

Ce qui est livré :

- `mail_core::plan_draft_pull` — la **décision**, pure et testée : quels
  UIDs distants rapatrier, quels miroirs locaux périmés retirer ;
- `ImapServer::draft_uids` / `fetch_draft`, `convert::draft_from_raw` ;
- `Store::import_remote_draft` / `drop_stale_draft` / `drafts_of` ;
- l'exécution dans `pull_drafts` (`apps/desktop/src/commands.rs`), appelée
  **depuis `run_sync`** et non depuis le cycle de brouillons : celui-ci
  s'arrête tôt quand il n'y a rien à pousser — à raison, sinon chaque
  frappe ouvrirait une connexion — et un brouillon venu d'ailleurs
  n'arriverait jamais. Zéro aller-retour ajouté.
- la **détection de conflit** : l'éditeur renvoie l'`updated_epoch` qu'il
  croit modifier (`save_draft(base_epoch)`) ; si la base a bougé, son
  texte est conservé **à part** au lieu d'écraser.

Validé sur le compte réel : points 1 à 3 et 6 du parcours (créer un
brouillon dans Gmail web, le retrouver, l'éditer, le repousser sans
doublon ; aucun bruit sur le compte Zoho).

### 7.2 CE QU'IL RESTE À TRANCHER — première chose à faire

Un symptôme du terrain n'est **pas expliqué**, et deux issues restent
ouvertes. Ne pas coder avant de l'avoir tranché.

**Le symptôme.** Composeur ouvert dans Discovery sur un brouillon,
modification du même brouillon dans Gmail web, puis synchronisation :
*« la liste des brouillons ne se met pas à jour »*.

**Deux explications possibles, et elles n'appellent pas le même travail :**

| Hypothèse | Conséquence |
|---|---|
| La consigne de test était fausse. Le bandeau n'affiche que **sujet et destinataire** : deux versions successives du même brouillon y sont *visuellement identiques*, seul le corps change et il n'est pas affiché. | Rien à corriger dans le tirage. Éventuellement montrer un extrait du corps dans le bandeau. |
| Le tirage n'a rien fait. | Vrai défaut, à diagnostiquer. |

**Comment trancher — l'outil est écrit et poussé :**

```powershell
cargo run -p mail-core --example diagnostic_brouillons --release -- "$env:APPDATA\dev.discovery.app\discovery.db"
```

Lecture : un brouillon marqué **« miroir (remplaçable) »** avec un `uid
distant` récent prouve que le tirage fonctionne. Des brouillons tous
« jamais poussé » prouveraient le contraire.

**Et rejouer le parcours** avec le binaire courant : ouvrir un brouillon,
laisser le composeur ouvert, modifier dans Gmail web, synchroniser, **puis
fermer le composeur** — l'avertissement rouge (« votre version a été
conservée à part ») doit apparaître **et rester**. Il était auparavant
écrasé une ligne plus tard par « brouillon conservé » ; corrigé en
`a738246`, non revérifié sur le terrain.

### 7.3 Décision produit en suspens (appartient à l'utilisateur)

Le regroupement en conversations est correct mais **rapporte peu sur la
boîte réelle** : 40 messages regroupés en 15 conversations sur 2 813, soit
25 lignes économisées. La cause est une décision assumée (ADR 0008 §3) —
*on ne regroupe que ce que la boîte contient*, et nos propres réponses
vivent dans « Envoyés », que la v1 ne synchronise pas.

L'utilisateur a tranché : **on décide après le gate 3**, pour connaître le
coût à l'échelle avant d'engager la synchronisation d'un second dossier.
**Ne pas rouvrir cette question avant.**

---

## 8. Prochaines étapes détaillées (dans l'ordre)

### Étape 1 — Clore la validation du tirage des brouillons
Faire le §7.2. Si un défaut est confirmé : le corriger **le jour même**
(kaizen), avec un test qui échoue d'abord.

### Étape 2 — Gate 3 : budgets tenus à l'échelle
Le gate bloquant de la Phase 3 : **3 comptes, 200 000 messages cumulés**.
Re-mesurer les budgets du §1 et les consigner :

| Métrique | Cible |
|---|---|
| Démarrage à froid | < 1 s |
| Ouverture d'un message | < 50 ms |
| Recherche | < 100 ms |
| Défilement de la liste | 60 fps |
| RAM (working set **privé**) | < 200 Mo |
| Taille de la base | < 1 Go |

Outils existants : `node e2e/mesure.mjs` (démarrage + page de liste),
`e2e/mesure-ram.ps1`, `cargo run -p mail-core --example seed_inbox` pour
fabriquer un volume.

Points de vigilance **nouveaux depuis la dernière mesure**, à surveiller
en priorité :

- la liste part désormais de la table `threads` (agrégat matérialisé) et
  non d'`envelopes` : le coût d'une page ne doit plus dépendre de la
  taille de la boîte — c'est l'argument de l'ADR 0008 §4, il faut le
  vérifier ;
- `thread::migrate_threads` adopte tous les messages à l'ouverture d'une
  base héritée. Mesuré instantané sur 2 800 messages, **jamais mesuré sur
  200 000** — c'est le risque principal pour le budget de démarrage ;
- la passe d'en-têtes de fils ajoute ~3 ko par message, bornée à 2 000 par
  compte et par synchronisation.

### Étape 3 — Revue de clôture Phase 3
Écrire `docs/PHASE3.md` sur le modèle de [PHASE2.md](PHASE2.md) : livré
contre le plan, budgets re-mesurés, enseignements, **reports assumés**,
décision GO/NO-GO. Reports à consigner explicitement :

- envoi de pièces jointes (lecture seule en v1) ;
- filtre de recherche « a une pièce jointe » ;
- synchronisation du dossier « Envoyés » (§7.3) ;
- `to:` dans la recherche.

---

## 8 bis. Enseignements de la Phase 3 — à lire avant de reprendre

Ils ont coûté cher ; les ignorer les fera repayer.

### Les défauts se trouvent sur le terrain, pas dans les tests
**Sept chantiers, sept défauts trouvés par la validation terrain, aucun
par la suite de tests.** Et ce ne sont jamais des erreurs de logique : ce
sont des **hypothèses fausses sur l'environnement ou sur l'usage** —
migration de données oubliée, contrainte de plateforme, principe du
produit non appliqué, deux écrivains sur une même ressource. Une suite de
tests ne peut pas les attraper. Le genchi genbutsu, si.

### Une fonctionnalité neuve doit ADOPTER les données anciennes
Le piège s'est présenté trois fois : pièces jointes (métadonnées écrites
par le seul chemin neuf), conversations (`thread_id` NULL → liste vide),
en-têtes de fil. À chaque fois, la fonctionnalité est **fausse dès la
première ouverture, et pour toujours**. Écrire la migration **en même
temps** que la fonctionnalité, et la prouver par un test qui rembobine la
base à son état antérieur.

### Mesurer avant de corriger
Sur le faux regroupement (43 messages étrangers dans un fil), mes trois
hypothèses étaient fausses. Le diagnostic a désigné la vraie cause en une
commande. Deux outils existent, sur le même modèle — lecture seule,
**aucun sujet, aucun expéditeur, aucun contenu**, seulement des formes et
des compteurs :

- `crates/mail-core/examples/diagnostic_index.rs` — index de recherche ;
- `crates/mail-core/examples/diagnostic_fils.rs` — conversations et ancres ;
- `crates/mail-core/examples/diagnostic_brouillons.rs` — brouillons.

En écrire un nouveau coûte 40 lignes et fait gagner un aller-retour.

### ⚠️ Vérifier qu'un signal demandé est OBSERVABLE
**Cinq consignes de validation envoyées à l'utilisateur étaient fausses** :
elles lui demandaient de constater un signal que l'interface ne produit
pas — message de démarrage écrasé par le compteur de liste, changement
invisible dans un bandeau qui n'affiche pas le champ modifié. Cela lui
coûte du temps et pollue le diagnostic. **Avant d'envoyer un parcours de
validation, vérifier dans le code que chaque signal demandé est
réellement affiché, et qu'il n'est pas écrasé une ligne plus loin.**

### Un statut posé sans regarder en efface un autre
Deux fois : le bandeau de confirmation d'action écrasé par le message
suivant, et l'avertissement de conflit de brouillon écrasé par
« brouillon conservé ». Quand une fonction pose un message d'état,
l'appelant doit **décider** du sien à partir de son bilan, jamais en poser
un aveuglément.

### Dette connue, non corrigée
`apps/desktop/ui/style.css` : la règle d'élément `header { display: flex }`
(destinée à la barre du haut) s'applique **aussi** à `#detail-header`, qui
est un `<header>`. Tout enfant pleine largeur qu'on y ajoute devient un
item flex écrasé à 0 px et poussé hors écran — mesuré. Le bandeau de
conversation a dû être sorti de `#detail-header` pour cette raison.
`#attachments` et `#detail-note` y sont toujours et ne fonctionnent que
par chance.

---

## 9. Environnement & commandes (⚠️ pièges Windows/PowerShell)

**Plateforme :** Windows 11. Deux shells disponibles : **PowerShell 5.1**
(principal) et **Bash** (Git Bash, POSIX). Ils n'ont pas la même syntaxe.

### 9.1 Les notifications exigent l'application INSTALLÉE

Piège coûteux, découvert sur le terrain. `tauri-plugin-notification`
s'appuie sur `tauri-winrt-notification`, qui exige une **identité
applicative (AppUserModelID)**. Sous Windows, cette identité n'existe que
si un **raccourci du menu Démarrer** la porte — donc uniquement après
installation.

Conséquences pratiques :

- lancée par `cargo run`, l'application n'a **aucune** identité : Windows
  refuse le toast, silencieusement ;
- il faut donc `cargo tauri build` puis installer, et lancer **depuis le
  menu Démarrer** — pas depuis `target/release` ;
- les variables `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET` et
  `MICROSOFT_CLIENT_ID` doivent alors être définies **au niveau
  utilisateur**, pas seulement dans le terminal courant.

**Windows n'inscrit une application dans Paramètres → Notifications
qu'APRÈS sa première notification réussie.** Vérifier la liste avant
d'avoir déclenché une bulle ne prouve donc rien — c'est ce qui a produit
un faux négatif pendant la validation.

Pour trancher sans attendre un vrai message, tester l'identité seule :

```powershell
[void][Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime]
$n = [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('dev.discovery.app')
$n.Setting   # « Enabled » = l'identité est enregistrée
```

### 9.2 Pièges à connaître absolument
- **PowerShell 5.1 n'a PAS l'opérateur `&&`.** Écrire les commandes sur des
  lignes séparées, ou les chaîner en Bash. `cd e2e && npm test` échoue en
  PowerShell → utiliser `cd e2e; npm test` ou le shell Bash.
- **Ne JAMAIS utiliser `Get-Content`/`Set-Content` sur les fichiers source**
  (risque de mojibake : PowerShell 5.1 réencode en UTF-16 avec BOM et
  corrompt les accents). Éditer via l'outil `Edit`, Python, ou Bash. Voir
  la mémoire projet `powershell-51-encodage-utf8`.
- Le tout est en **UTF-8** (accents français partout).

### 9.3 Commandes de développement
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

### 9.4 Déterminisme des E2E
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
- ~~**Enregistrement Azure AD**~~ ✅ fait. Retenir pour la suite :
  `MICROSOFT_CLIENT_ID` doit être défini dans l'environnement, **sans
  secret** (client public, PKCE), et l'URI de redirection enregistrée est
  `http://localhost` — Azure AD la distingue de `http://127.0.0.1`.

---

## 11. Carte des fichiers (points d'entrée les plus utiles)

| Fichier | Rôle |
|---|---|
| [`docs/PLAN.md`](PLAN.md) | Concept paper — source de vérité produit |
| [`docs/PHASE0-2.md`](.) | Revues de clôture (décisions, budgets, enseignements) |
| [`docs/adr/`](adr/) | Décisions gelées (workspace, Tauri, boîte d'envoi, FTS5) |
| [`crates/mail-core/src/store.rs`](../crates/mail-core/src/store.rs) | Stockage SQLite, schéma, migrations, boîte unifiée |
| [`crates/mail-core/src/search.rs`](../crates/mail-core/src/search.rs) | Recherche FTS5 (index contentless, transactionnel) |
| [`crates/mail-core/src/thread.rs`](../crates/mail-core/src/thread.rs) | Conversations : union-find pur + persistance ([ADR 0008](adr/0008-regroupement-en-conversations.md)) |
| [`crates/mail-core/src/drafts.rs`](../crates/mail-core/src/drafts.rs) | Brouillons : poussée, **tirage**, conflit d'édition |
| [`crates/mail-core/examples/`](../crates/mail-core/examples/) | Diagnostics terrain (index, fils, brouillons) + `seed_inbox` |
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
2. **Lis le §8 bis** — les enseignements de la Phase 3. Ils ont coûté cher.
3. **Tranche le §7.2** : lance `diagnostic_brouillons`, rejoue le parcours.
   Ne code pas avant d'avoir la réponse.
4. Puis le **gate 3** (§8 étape 2) et la **revue de clôture** (étape 3).
4. Travaille par **petits incréments testés**, valide **sur le terrain**
   avec l'utilisateur, corrige les retours **le jour même**, et **dis non**
   à toute dérive de périmètre. Chaque décision structurante = un ADR ;
   chaque fin de phase = une revue de clôture.

*Vos mails, instantanément. La performance et la fiabilité ne sont pas des
options — ce sont les fonctionnalités.*
