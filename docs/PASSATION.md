# Passation — reprendre Discovery dans une nouvelle conversation

> **Ce document est l'instruction de projet.** Il n'y a pas de `CLAUDE.md`
> ici : tout ce qui ne se déduit pas du code est écrit là.
>
> État au **2026-07-25**, branche `main`. Arbre propre,
> **294 tests Rust · 19/19 E2E · clippy muet**. Aucun code en vol.
>
> **Phases 0 à 3 closes.** Le gate 3 est joué et sa revue écrite
> ([PHASE3.md](PHASE3.md)). Ce qui reste tient dans deux arbitrages
> produit et deux budgets non tenus, tous documentés au §8.

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

**Rien n'est cassé, rien n'est à moitié écrit, rien n'est en vol.** La
Phase 3 est close, gate joué et revue écrite ([PHASE3.md](PHASE3.md)).

Les deux arbitrages qui restaient ont été **tranchés le 2026-07-25** :

1. **« Envoyés » est synchronisé** — décision prise, spécifiée par
   [ADR 0009](adr/0009-portee-des-fils-au-compte.md) ;
2. **Phase 5 (durcissement et bêta) avant Phase 4 (web)** — la bêta est ce
   qui permettra de trancher la recherche sur de vraies boîtes plutôt que
   sur un corpus synthétique.

### Le chantier en cours : la portée des fils

**Spécifié, pas encore écrit.** Lire [ADR 0009](adr/0009-portee-des-fils-au-compte.md)
en entier avant de commencer — il porte les six décisions de conception et
les alternatives déjà écartées.

Le point dur, qui n'est pas là où on l'attend : synchroniser « Envoyés »
est de la plomberie (le moteur est déjà paramétré par nom de boîte,
`commands.rs` fixe simplement `MAILBOX = "INBOX"`). Mais les fils sont
cloisonnés par boîte, donc **une réponse ne rejoindrait jamais le fil du
message auquel elle répond**. Le chantier réel est le passage de la portée
`mailbox_id` → `account_id`.

Ordre conseillé — chaque étape se prouve avant la suivante :

1. ✅ **Le noyau** : `attach` / `lookup` passent au compte, l'agrégat gagne
   `last_mailbox_id` et `inbox_size`, `clear_mailbox` devient
   `rebuild_account`.
2. ✅ **Le schéma et sa migration** : les tables changent de clé, donc
   `rebuild_if_outdated` les **supprime** et `THREADING_VERSION` passe à 2.
3. ✅ **La requête de liste et son index PARTIEL** (ADR 0009 §4). Vérifié à
   200 000 messages : `SCAN t USING INDEX idx_threads_date_globale`, page
   à 0,71 ms — le gain du gate 3 est préservé.
4. ⬜ **La synchronisation d'« Envoyés »** : découverte par attribut
   `\Sent` puis repli par nom (ADR 0009 §7), et boucle sur deux boîtes.
   `commands.rs` fixe encore `MAILBOX = "INBOX"`.
5. ⬜ **Re-mesurer** : recherche (le corpus grandit, cf. §3) et page de
   liste. Puis **valider sur le terrain** : c'est le seul endroit où l'on
   verra si le regroupement rapporte enfin.

⚠️ **Tant que l'étape 4 n'est pas faite, aucun fil ne traverse deux boîtes
en production.** Le noyau le permet, la plomberie n'existe pas — et rien à
l'écran ne change. Ne pas conclure de l'absence d'effet que le chantier a
échoué.

Le reste de ce §1 raconte le dernier chantier clos — contexte, pas action.

### 1.1 Ce que la mesure a établi (2026-07-25)

Le diagnostic a tranché : **le tirage fonctionne**. Sur la base réelle, un
brouillon « miroir (remplaçable) » portait un `uid distant` récent (478),
rapatrié du webmail.

Il a aussi désigné la vraie cause du symptôme, et ce n'était pas un défaut
de synchronisation : les deux versions étaient **indiscernables à
l'écran**. Sujet 14 car. et destinataire 22 car. des deux côtés, seul le
corps différait — 28 contre 48 — et le bandeau ne l'affichait pas.
L'hypothèse « la consigne de test était fausse » était la bonne.

*Note incidente, non conclue :* 477 et 478 **coexistaient** côté serveur,
zéro tombstone. Le module suppose pourtant qu'éditer un brouillon ailleurs
**remplace** le message (ancien UID expurgé, nouveau créé). Soit Gmail ne
remplace pas toujours, soit un brouillon neuf avait été commencé côté web.
Les données ne départagent pas — à garder en tête.

### 1.2 Ce qui a été livré en réponse

1. **L'extrait du corps dans le bandeau** — le signal qui manquait. Prouvé
   par un test E2E qui crée deux brouillons de même sujet et de même
   destinataire et exige qu'ils se distinguent. Sans lui, toute consigne de
   validation portant sur les brouillons est invérifiable.
2. **Un trou latent, trouvé en enquêtant.** Sur le chemin du
   *remplacement* — celui que le module documente comme normal — le tirage
   **supprime** la ligne que le composeur croit modifier. La détection de
   conflit ne comparait que des horodatages : il n'en restait qu'un, elle
   se taisait. Voir l'enseignement au §9.
3. **La poussée n'efface plus l'avertissement.** `pushDrafts` revenait du
   réseau une seconde plus tard et posait son bilan par-dessus. Le
   brouillon conservé à part étant neuf, donc toujours à pousser, la
   collision était **certaine**, pas fortuite.

### 1.3 Validé sur le terrain (2026-07-25)

Les trois points sont vérifiés sur les vrais comptes. L'extrait se
constate directement : les deux brouillons gmail, jusque-là identiques à
l'écran, portent désormais des textes distincts.

Le chemin du remplacement ne s'est pas produit lors du run mesuré et il
est difficile à forcer via Gmail. **Le parcours déterministe qui l'exerce
sans réseau — à réutiliser :**

> ouvrir un brouillon (« Reprendre »), **laisser le composeur ouvert**,
> cliquer « Supprimer » sur cette même ligne dans le bandeau, puis fermer
> le composeur.

La ligne rouge « ce brouillon avait changé ailleurs… » apparaît **et
reste** — c'est le point 3 qui la fait rester.

### 1.4 Ensuite — le gate 3, puis la clôture

Voir §8. C'est tout ce qui reste avant de fermer la Phase 3.

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
**C'est là que les défauts se trouvent.** Voir §9 : sept chantiers, sept
défauts trouvés par la validation terrain, **aucun par la suite de
tests**. Un incrément non validé sur un vrai compte n'est pas livré. Les
retours se corrigent **le jour même**.

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

Mesurés au **gate 3** — 3 comptes, 200 000 messages
([PHASE3.md](PHASE3.md) §2) :

| Métrique | Cible | Dernière mesure |
|---|---|---|
| Démarrage à froid | < 1 s | 360–389 ms ✅ |
| Ouverture d'un message | < 50 ms | 0,09–0,16 ms ✅ |
| Page de liste | < 100 ms | 12,4 ms ✅ |
| RAM (working set **privé**) | < 200 Mo | 92,2 Mo ✅ |
| Taille de la base | < 1 Go | 778 Mo / 200 000 msg + 16 002 corps ✅ |
| Perte de données | 0, prouvé par crash-récup | ✅ |
| **Recherche** | < 100 ms | **118–208 ms ❌** |
| **Adoption d'une base héritée** | < 1 s | **4,22 s ❌** (une seule fois) |

La recherche est le seul poste non tenu, sur un corpus synthétique dont la
sélectivité est reconnue extrême. Le chiffre transférable est le **coût
unitaire : ~2,9 µs par correspondance**, soit un plafond vers **35 000
correspondances**. Deux leviers mesurés en réserve (tri par date, `prefix=`)
— arbitrage reporté en bêta, PHASE3.md §4.

⚠️ **Les outils de mesure se vérifient comme le reste.** Deux d'entre eux
mentaient au gate 3 : `mesure-ram.ps1` sommait toutes les instances de
l'application, `mesure.mjs` n'isolait pas son profil WebView2. Corrigés,
mais le réflexe reste à avoir.

Un budget dépassé = **on arrête la ligne** (andon). Pas de « livrer puis
optimiser » : la performance est une contrainte de conception.

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
`plan_draft_pull` pour les brouillons, `notify::arrivals_to_notify` pour
les bulles. C'est ce qui permet de tester les scénarios du terrain sans
réseau.

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
| [0008](adr/0008-regroupement-en-conversations.md) | Conversations = union-find sur en-têtes RFC 5322 | **Jamais de repli par sujet** ; agrégat matérialisé recalculé, jamais incrémenté |
| [0009](adr/0009-portee-des-fils-au-compte.md) | Portée d'un fil = le **compte**, pas la boîte | Révise 0008 §3 et §4 ; « Envoyés » synchronisé ; index **partiel** sinon le gate 3 est perdu |

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
2. **Identité message = `(account_id, uid)`** partout, jusque dans la
   sélection de l'UI. Un UID seul n'identifie rien.
3. **Les index et agrégats vivent DANS la base**, entretenus dans la MÊME
   transaction que le message : index FTS5, table `threads`. Pas de second
   magasin, pas de réconciliation après crash.
4. **Sécurité du rendu** : HTML assaini par `ammonia`, images distantes
   bloquées par défaut, iframe sandboxée + CSP. Données de mail injectées
   par `textContent`, **jamais** `innerHTML`.
5. **Credentials jamais en clair** : Credential Manager Windows via
   `keyring`. Aucun secret dans le code ni les logs.
6. **UIDVALIDITY** : si elle change, on repart de zéro pour cette boîte.
   Règle brouillons : *« un doublon est acceptable, supprimer le mauvais
   UID jamais »*.
7. **Une fonctionnalité neuve doit ADOPTER les données anciennes** — voir
   §9 : le piège s'est présenté trois fois.

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
  (`%LOCALAPPDATA%\Packages\Claude_…\LocalCache\Roaming\`). Mesuré le
  2026-07-25 : cette copie datait du 12/07 et n'avait même pas de table
  `drafts`. La vue est *fusionnée*, pas gelée — les fichiers voisins
  (`discovery-banc.db`) apparaissent normalement, seul `discovery.db` est
  masqué, ce qui rend le piège discret.

  **Conséquence :** les diagnostics du §9 doivent être lancés **par
  l'utilisateur**, qui colle la sortie. Corollaire de méthode : annoncer
  d'abord ce qu'on s'attend à y lire, pour que l'aller-retour soit une
  mesure et non une collecte.

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
cargo test -p mail-core                       # noyau
cargo test --workspace                        # tout
cargo build -p discovery-desktop --release    # binaire
cargo run -p discovery-desktop --release      # lancer (sans notifications)

cargo fmt
cargo clippy --all-targets -- -D warnings

cd e2e
npm test                                      # PowerShell : deux lignes

# Jeu d'essai (corps + pièces jointes + conversations)
cargo run -p mail-core --example seed_inbox -- <db> <count> <email>

# Installateur (nécessaire pour les notifications)
cd apps/desktop
cargo tauri build
```

Mesures : `node e2e/mesure.mjs` (démarrage + page de liste),
`e2e/mesure-ram.ps1` (working set privé).

### 7.4 Le gate pré-push
`.githooks/pre-push` (via `core.hooksPath`) rejoue, dans l'ordre : `fmt` →
`clippy -D warnings` → `cargo test --workspace` → `npm test` dans `e2e/`.
Il existe parce qu'un runner GitHub **ne peut pas** ouvrir WebView2
(ADR 0005) : les E2E ne tournent que depuis cette machine.

Il a déjà rattrapé des livraisons annoncées vertes qui ne l'étaient pas.
`--no-verify` existe ; s'en servir est une décision, pas un raccourci.

### 7.5 Déterminisme des E2E
Étanches par construction : base SQLite jetable (`DISCOVERY_DB_PATH`),
comptes factices aux jetons invalides (`DISCOVERY_E2E_ACCOUNT`), et
`GOOGLE_CLIENT_ID`/`SECRET` **retirés** de l'environnement du process.
WebView2 est piloté via `--remote-debugging-port=9222` + `connectOverCDP`.

**Conséquence à garder en tête :** les E2E ne parlent à aucun serveur. Tout
ce qui touche au réseau réel — OAuth, tirage des brouillons, passes de
fond — n'est couvert que par des tests unitaires sur la partie pure. Le
chemin réseau complet ne se prouve que sur le terrain.

---

## 8. Ce qui reste — après la Phase 3

Phases 0 à 3 : **closes** ([PHASE0](PHASE0.md), [PHASE1](PHASE1.md),
[PHASE2](PHASE2.md), [PHASE3](PHASE3.md)).

Le gate 3 a été joué : 3 comptes, 200 000 messages. **Six budgets sur huit
tenus**, et les deux défauts qu'il a trouvés sont corrigés — le tri
matérialisé de la boîte unifiée (987 ms → 12,4 ms) et le coût de
l'adoption des fils (11,1 s → 4,2 s). Tous deux étaient invisibles à
l'échelle du terrain, et aucun test fonctionnel ne pouvait les voir.

### Les deux budgets non tenus, avec leur remède

| Poste | Mesure | Levier connu |
|---|---|---|
| Recherche | 118–208 ms | tri par date (×2, mesuré) ou `prefix=` (−73 ms) — arbitrage produit reporté en bêta |
| Adoption d'une base héritée | 4,22 s, **une seule fois** | la rendre visible et interruptible, comme le rattrapage des corps (ADR 0007) |

Le démarrage courant, lui, reste à **2,5 ms** : c'est la migration qui
coûte, pas l'ouverture.

### Ce qu'il ne faut PAS faire pour l'adoption
Adopter par tranches à chaque démarrage. La liste part de `threads` : une
adoption partielle afficherait une boîte à moitié vide — le piège du §9,
la fonctionnalité fausse dès la première ouverture.

### Avant d'ouvrir la Phase 4
Deux arbitrages appartiennent au Chef Ingénieur :

1. la **synchronisation du dossier « Envoyés »** — voir ci-dessous ;
2. l'**ordre entre Phase 4 (web) et Phase 5 (durcissement, bêta)**, la
   bêta étant précisément ce qui permettrait de trancher la recherche sur
   de vraies boîtes.

### Décision produit en suspens (appartient à l'utilisateur)
Le regroupement en conversations est correct mais **rapporte peu** sur la
boîte réelle : 40 messages regroupés en 15 conversations sur 2 813. La
cause est une décision assumée (ADR 0008 §3) — *on ne regroupe que ce que
la boîte contient*, et les réponses de l'utilisateur vivent dans
« Envoyés », que la v1 ne synchronise pas.

L'utilisateur avait tranché : **on décide après le gate 3**, pour connaître
le coût à l'échelle avant d'engager un second dossier. **Le gate est joué,
le coût est connu** ([PHASE3.md](PHASE3.md) §5) :

- la RAM ne dépend pas du volume (+2,6 Mo pour ×4 de messages) ;
- le coût d'une page ne dépend plus de la taille de la boîte ;
- le disque tient avec 2,7× la charge modélisée ;
- **mais la recherche se paie au nombre de correspondances** : ajouter
  « Envoyés » agrandit le corpus, donc rapproche le plafond des 35 000
  correspondances.

La décision est donc **ouverte, et elle attend l'utilisateur**.

### Dette connue, non corrigée
`apps/desktop/ui/style.css` : la règle d'élément `header { display: flex }`
(destinée à la barre du haut) s'applique **aussi** à `#detail-header`, qui
est un `<header>`. Tout enfant pleine largeur qu'on y ajoute devient un
item flex écrasé à 0 px et poussé hors écran — mesuré. Le bandeau de
conversation a dû être sorti de `#detail-header` pour cette raison.
`#attachments` et `#detail-note` y sont toujours et ne fonctionnent que
par chance.

---

## 9. Enseignements de la Phase 3 — à lire avant de reprendre

Ils ont coûté cher. Les ignorer les fera repayer.

### Les défauts se trouvent sur le terrain, pas dans les tests
**Sept chantiers, sept défauts trouvés par la validation terrain, aucun
par la suite de tests.** Et jamais des erreurs de logique : toujours des
**hypothèses fausses sur l'environnement ou sur l'usage** — migration de
données oubliée, contrainte de plateforme, principe du produit non
appliqué, deux écrivains sur une même ressource. Une suite de tests ne
peut pas les attraper.

### Une fonctionnalité neuve doit ADOPTER les données anciennes
Le piège s'est présenté **trois fois** : pièces jointes (métadonnées
écrites par le seul chemin neuf), conversations (`thread_id` NULL → liste
vide), en-têtes de fil. À chaque fois la fonctionnalité est **fausse dès
la première ouverture, et pour toujours**. Écrire la migration **en même
temps** que la fonctionnalité, et la prouver par un test qui rembobine la
base à son état antérieur.

### Mesurer avant de corriger
Sur le faux regroupement (43 messages étrangers dans un fil), mes trois
hypothèses étaient fausses ; le diagnostic a désigné la cause en une
commande. Trois outils existent, même modèle — lecture seule, **aucun
sujet, aucun expéditeur, aucun contenu**, seulement des formes et des
compteurs :

| Outil | Répond à |
|---|---|
| `diagnostic_index.rs` | les messages sont-ils dans l'index de recherche ? |
| `diagnostic_fils.rs` | quel identifiant réunit les messages d'un fil ? |
| `diagnostic_brouillons.rs` | le tirage des brouillons fait-il son travail ? |

En écrire un nouveau coûte 40 lignes et fait gagner un aller-retour.

### ⚠️ Vérifier qu'un signal demandé est OBSERVABLE
**Cinq consignes de validation envoyées à l'utilisateur étaient fausses** :
elles lui demandaient de constater un signal que l'interface ne produit
pas — message de démarrage écrasé par le compteur de liste, changement
invisible dans un bandeau qui n'affiche pas le champ modifié. Cela lui
coûte du temps et pollue le diagnostic.

**Avant d'envoyer un parcours de validation : vérifier dans le code que
chaque signal demandé est réellement affiché, et qu'il n'est pas écrasé
une ligne plus loin.**

### Un statut posé sans regarder en efface un autre
Deux fois : le bandeau de confirmation d'action écrasé par le message
suivant, et l'avertissement de conflit de brouillon écrasé par
« brouillon conservé ». Quand une fonction pose un message d'état,
l'appelant doit **décider** du sien à partir de son bilan, jamais en poser
un aveuglément.

### Ne jamais avaler une erreur
`let _ = …show()` sur les notifications a protégé la synchro et détruit la
preuve : le symptôme était « rien ne se passe », indiagnosticable.
Absorber un échec est une chose, en effacer la trace en est une autre. Les
échecs non bloquants remontent dans le bilan de synchro.

### Ajouter un écrivain exige une coordination explicite
Le tirage des brouillons a été branché sur une ressource que le composeur
écrivait déjà. Résultat : la copie tenue en mémoire écrasait la version
venue d'ailleurs. Corrigé par détection de conflit — l'éditeur renvoie
l'horodatage qu'il croit modifier, et son texte est **conservé à part**
plutôt qu'écrasé.

### Un test vert peut encoder un modèle FAUX de l'autre écrivain

Cette détection était éprouvée en simulant l'autre écrivain par une
**réécriture en place**. Or le vrai autre écrivain — le tirage — ne
réécrit pas : il **retire** le miroir périmé et importe la version fraîche
sous un nouvel identifiant (`plan_draft_pull`). La ligne que le composeur
croyait modifier ayant disparu, la détection ne comparait plus qu'un seul
horodatage, et se taisait. Test vert, terrain muet.

Deux règles en découlent :

1. simuler l'autre écrivain en **appelant son vrai chemin** — ici
   `plan_draft_pull`, puis `drop_stale_draft`/`import_remote_draft` — et
   jamais par une approximation qui lui ressemble ;
2. se méfier des défauts que le hasard masque. SQLite attribue
   `max(rowid) + 1` : quand le brouillon édité était le dernier, l'import
   reprenait l'identifiant qu'on venait de libérer, la ligne réapparaissait
   sous le composeur et la détection retombait sur ses pieds **par
   accident**. Un défaut qui ne se manifeste qu'une fois sur deux est un
   défaut qu'on croit corrigé.

Corollaire du §2.5, vérifié une fois de plus : c'est le terrain qui a
signalé « le message rouge ne s'affiche pas », et aucun des 292 tests.

---

## 10. Carte des fichiers

| Fichier | Rôle |
|---|---|
| [`docs/PLAN.md`](PLAN.md) | Concept paper — source de vérité produit |
| [`docs/adr/`](adr/) | Les 8 décisions gelées |
| [`docs/PHASE0.md`](PHASE0.md) → [`PHASE2.md`](PHASE2.md) | Revues de clôture (décisions, budgets, enseignements) |
| [`crates/mail-core/src/store.rs`](../crates/mail-core/src/store.rs) | Stockage SQLite, schéma, migrations, boîte unifiée |
| [`crates/mail-core/src/sync.rs`](../crates/mail-core/src/sync.rs) | Moteur de synchro (contre `FakeServer`) |
| [`crates/mail-core/src/thread.rs`](../crates/mail-core/src/thread.rs) | Conversations : union-find pur + persistance |
| [`crates/mail-core/src/drafts.rs`](../crates/mail-core/src/drafts.rs) | Brouillons : poussée, tirage, conflit d'édition |
| [`crates/mail-core/src/outbox.rs`](../crates/mail-core/src/outbox.rs) | Boîte d'envoi + règles d'or |
| [`crates/mail-core/src/search.rs`](../crates/mail-core/src/search.rs) | Index FTS5 contentless, transactionnel |
| [`crates/mail-core/src/test_support.rs`](../crates/mail-core/src/test_support.rs) | `FakeServer` — rejoue les bizarreries du terrain |
| [`crates/mail-core/examples/`](../crates/mail-core/examples/) | Diagnostics terrain + `seed_inbox` |
| [`crates/mail-auth/src/provider.rs`](../crates/mail-auth/src/provider.rs) | Fournisseurs OAuth décrits **en données** |
| [`apps/desktop/src/commands.rs`](../apps/desktop/src/commands.rs) | Commandes Tauri (IPC), boucles par compte |
| [`apps/desktop/ui/app.js`](../apps/desktop/ui/app.js) | UI : liste virtualisée, composeur, raccourcis |
| [`e2e/README.md`](../e2e/README.md) | Harnais E2E déterministe (CDP) |

---

*Vos mails, instantanément. La performance et la fiabilité ne sont pas des
options — ce sont les fonctionnalités.*
