# Passation — reprendre Wind dans une nouvelle conversation

> **Ce document est l'instruction de projet.** Depuis le 2026-08-15, un
> `CLAUDE.md` de dix lignes à la racine le charge automatiquement à
> chaque session — il ne porte que le rôle et le renvoi vers ce
> document : **toute la substance reste ici**. (La décision « pas de
> `CLAUDE.md` » est renversée à cette date, décision CE, pour supprimer
> le rituel d'ouverture manuel.)
>
> État au **2026-07-26** (soir), branche `main`.
> **~340 tests Rust · 21/21 E2E · clippy muet**.
>
> **Phases 0 à 3 closes**, gate 3 joué. **Trois chantiers de la Phase 5
> sont TERMINÉS et validés au terrain** : la migration visible et
> interruptible (**ADR 0012**), l'**installeur NSIS + mise à jour signée**
> (**ADR 0013**, boucle 0.1.1 → 0.1.2 appliquée sur l'app installée) et
> la **télémétrie de crash locale et opt-in** (**ADR 0014**, rédaction
> prouvée sur la vraie machine). Reste, avant le gate 5 : la **bêta
> fermée 20-50 utilisateurs**.
>
> ⚠️ **Un chantier attend le commit** : la télémétrie (ADR 0014) est
> implémentée, testée, validée au terrain, mais **non commitée** — accord
> du Chef Ingénieur en attente. Si l'arbre est sale à la reprise, c'est
> elle.

---

## 0. Comment ouvrir la conversation

Depuis le 2026-08-15, `CLAUDE.md` (racine) charge le rôle et le renvoi
vers ce document à chaque session : **plus rien à coller**. Les
workflows standardisés vivent dans `.claude/skills/` (commitées,
décision CE du même jour) : `/chantier` déroule un bug ou une feature
de bout en bout avec ses deux validations manuelles (plan, terrain),
`/terrain` traite un constat terrain le jour même, `/gate` rejoue la
gate complète, `/solde` clôt un chantier. L'agent `spike`
(`.claude/agents/`) porte l'exploration set-based en worktree isolé.
Le mode d'emploi complet : [WORKFLOW.md](WORKFLOW.md).

Si le contexte est perdu malgré tout, l'ancien rituel reste valable :

> Reprends le développement de Wind. Tu es le Chef Ingénieur du
> projet et tu appliques la méthode décrite dans `docs/PASSATION.md` §2 —
> c'est une instruction permanente, elle prime sur tout. Lis d'abord ce
> document en entier, puis applique le §1.

Ordre de lecture, une fois :

1. **ce document** — méthode, état, pièges ;
2. [`docs/PLAN.md`](PLAN.md) — le concept paper, source de vérité produit ;
3. les ADRs dans [`docs/adr/`](adr/) — **décisions gelées**, à ne pas
   rouvrir sans mesure contraire. Les trois dernières (0010, 0011, 0012)
   portent l'état le plus récent du produit.

Ne lis pas le code avant. Il est volumineux et abondamment commenté ; les
commentaires expliquent *pourquoi*, et supposent le contexte ci-dessous.

---

## 1. Où on en est, et quoi faire en premier

**Rien n'est cassé, rien n'est à moitié écrit, rien n'est en vol.**

### 1.1 L'état du terrain — chiffres du 2026-07-26, boîte réelle

La synchronisation intégrale (ADR 0010) a tout ramené : **256 312
messages** (7 539 avant), 4 comptes, tous dossiers — spam et corbeille
compris, décision explicite du Chef Ingénieur.

**La passe d'en-têtes a convergé à zéro** : `diagnostic_fils` affiche
`jamais lus : 0` dans la portée du regroupement. Ce chiffre est final —
plus rien n'est en train de bouger côté fils. Résultat du regroupement :

| | avant ADR 0009 | avant ADR 0010 | **final** |
|---|---|---|---|
| fils de 2 à 5 | 15 (tous confondus) | 242 | **577** |
| fils de 6 à 20 | — | 6 | **35** |
| fils de plus de 20 | — | 0 | **1** |

**La portée tient à l'échelle** : 248 771 messages hors portée n'ont créé
aucun fil et n'ont fait remonter aucune conversation — c'est l'invariant
§6.9, tenu par test.

**Ce qui bouge encore : le rattrapage des corps.** ~250 000 messages
attendent leur corps, à 200 par lot, au fil de l'usage — une longue
traîne de plusieurs jours ou semaines, reprenable, visible dans le
bandeau ocre de l'application. **La base grandira vers ~13 Go**
(256 312 × ~50 ko) ; le budget « < 1 Go » est levé (ADR 0010 §2) et la
garde d'espace disque veille avant chaque engagement.

**Premier réflexe d'une nouvelle session :** demander à l'utilisateur où
en est le bandeau de rattrapage et ce que pèse
`%APPDATA%\dev.elements.wind\wind.db` (avec ses compagnons `-wal` et
`-shm`). Rappel du §7.1 : tu ne peux pas lire sa base toi-même.
(Avant PLAN-WIND E3 : `dev.discovery.app\discovery.db` — le déménagement
est automatique au premier lancement Wind.)

### 1.2 Les budgets non tenus, avec leur remède

| Poste | Mesure (2026-07-26) | Levier |
|---|---|---|
| Adoption d'une base héritée | 3,66 s à 200 000 messages, une seule fois | **réglé en forme par l'ADR 0012** : visible, annulable, rembobinable — la durée est assumée, la passe est unique |
| Recherche | 113–210 ms à l'échelle du gate 3 | tri par date (**×1,8–2,9, re-validé**) ou `prefix=` — le Chef Ingénieur tranche **en bêta**, sur de vraies boîtes |

À l'échelle réelle la recherche reste confortable (~2,9 µs par
correspondance), mais le corpus intégral (×34) rapproche le plafond des
~35 000 correspondances : le tri par date est sur le chemin critique de
la bêta.

### 1.3 Arbitrages — tranchés et ouverts

**Tranchés** (ne pas rouvrir sans mesure) :
- ~~Synchroniser l'archive ?~~ → **Tout est synchronisé** (ADR 0010),
  spam et corbeille compris, sans quota. La question est soldée.
- ~~Périmètre de la Phase 5 ?~~ → La migration visible et interruptible
  d'abord — **faite** (ADR 0012). Suivent, dans l'ordre : installeur,
  télémétrie, bêta.

**Ouverts** (au Chef Ingénieur) :
- **Tri par date de la recherche** — en bêta.
- **Doublons multi-boîtes dans la recherche** — observé au terrain : le
  même message vit copié dans plusieurs boîtes (« 19 messages partagent
  un Message-ID »), et la recherche renverra chaque copie. Dédoublonner à
  l'affichage ? À observer en usage réel avant de décider.

### 1.4 Ensuite — la Phase 5

Durcissement et bêta ([PLAN.md](PLAN.md) §4). Ordre arbitré : migration
visible et interruptible **✓ faite (ADR 0012)** → installeur + mise à
jour signée **✓ faite (ADR 0013)** → télémétrie de crash opt-in **✓
faite (ADR 0014)** → **bêta fermée 20-50 utilisateurs (prochain)**.
Gate 5 : deux semaines sans défaut critique.

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
- **TDD** : le test échoue (RED) avant l'implémentation (GREEN). Quand un
  RED ne peut rien apprendre (fonction pure triviale), le dire, pas le
  simuler.
- **Gate obligatoire avant tout commit** — et un hook `pre-push` le rejoue
  (§7.4). Un warning clippy = build rouge.
- Zéro `unwrap()`/`expect()` en production. Erreurs typées (`thiserror`)
  dans les crates, `anyhow` dans les apps.

### 2.5 Genchi genbutsu — aller voir sur le terrain
**C'est là que les défauts se trouvent.** Voir §9. Un incrément non validé
sur un vrai compte n'est pas livré. Les retours se corrigent **le jour
même** — le WAL (ADR 0011) en est le dernier exemple : défaut au premier
essai terrain, corrigé et commité dans la journée.

### 2.6 Refus de périmètre explicites
Quand une fonctionnalité serait un fantôme (résultat invisible, brique
absente), on la **reporte et on écrit pourquoi**. Dire non est le
comportement par défaut : chaque ajout se paie en vitesse et en fiabilité.

### 2.7 Traçabilité
- Décision structurante = **un ADR court** dans `docs/adr/`.
- Fin de phase = **une revue de clôture** `docs/PHASEn.md` : livré contre
  le plan, budgets re-mesurés, enseignements, reports assumés, GO/NO-GO.

### 2.8 Langue et commits
**Tout est en français** — commits, UI, docs, commentaires de code. Format
`type: description` (`feat`, `fix`, `refactor`, `docs`, `test`, `chore`,
`perf`, `ci`). **Jamais de `Co-Authored-By`.**

⚠️ **Les messages de commit s'écrivent SANS ACCENTS** — convention
observable dans tout l'historique. Le corps du message porte les chiffres
et le raisonnement.

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
assaini, images distantes bloquées). Depuis l'ADR 0010 : **complet** —
toute la boîte est locale et cherchable, spam et corbeille compris.

**Ce qu'il N'EST PAS (v1) :** pas de calendrier, pas de chat, pas d'IA
intégrée, pas de plugins, pas de mobile.

### Budgets — ce sont des gates BLOQUANTS

Re-mesurés le 2026-07-26 après l'ADR 0010, sur les bases du gate 3
(3 comptes, 200 000 messages) :

| Métrique | Cible | Dernière mesure |
|---|---|---|
| Démarrage à froid | < 1 s | 337 ms ✅ |
| Ouverture d'un message | < 50 ms | 1–3 ms ✅ |
| Page de liste | < 100 ms | 0,58 ms ✅ |
| RAM (working set **privé**) | < 200 Mo | 95,5 Mo · 7 processus ✅ |
| Taille de la base | **levé** (ADR 0010 §2) | garde d'espace disque à ~50 ko/message |
| Perte de données | 0, prouvé par crash-récup | ✅ |
| **Gel de la pompe de messages** | aucun gel > 150 ms (fenêtre toujours déplaçable) | 0 gel sur 40 s, décor 251 k enveloppes (PLAN-GELS, `e2e/sonde-gel.py`) ✅ |
| **Recherche** | < 100 ms | **113–210 ms ❌** (levier ×1,8–2,9 validé, tranché en bêta) |
| **Adoption d'une base héritée** | < 1 s | **3,66 s — assumé** (ADR 0012 : une seule fois, visible, annulable, rembobinable) |

Un budget dépassé = **on arrête la ligne** (andon). Le gate « base
< 1 Go » n'est pas un oubli : il est **levé explicitement** par
l'ADR 0010 §2, remplacé par la garde d'espace disque.

⚠️ **Les outils de mesure se vérifient comme le reste.** Trois d'entre eux
mentaient au gate 3 (RAM sommée sur toutes les instances, profil WebView2
non isolé, décor qui n'exerçait pas l'index partiel). Corrigés — mais le
réflexe reste à avoir.

---

## 4. Architecture — « un seul cerveau »

`mail-core` contient **100 % de la logique métier**, de la synchro et du
stockage. Le desktop l'embarque en processus ; le web (Phase 4) l'exécutera
côté serveur. L'UI est « bête » : elle affiche un état, elle émet des
intentions.

```
wind/
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
décision gelée ; `Store` est une struct concrète, les tests utilisent une
base en mémoire, et le journal est en **WAL** sur fichier (ADR 0011).

**Un motif récurrent, à imiter.** La décision est **pure et testable**,
l'exécution (I/O) est ailleurs : `thread::plan` (conversations),
`plan_draft_pull` (brouillons), `convert::sent_folder` (dossier des
envois), `notify::arrivals_to_notify` (bulles), et depuis l'ADR 0010 :
`sync_order` (ordre des boîtes), `sync_percent` (avancement),
`disk_shortfall` (garde d'espace). C'est ce qui permet de tester les
scénarios du terrain sans réseau.

---

## 5. Décisions gelées — ne pas rouvrir sans mesure

| ADR | Décision | À retenir |
|---|---|---|
| [0001](adr/0001-structure-workspace.md) | Workspace Cargo multi-crates | `mail-core` sans dépendance UI/réseau |
| [0002](adr/0002-shell-desktop-tauri.md) | Shell desktop = Tauri 2 (WebView2) | La RAM qui fait foi = working set **privé** |
| [0003](adr/0003-boite-envoi-smtp.md) | Boîte d'envoi SMTP + règles d'or | Journal AVANT réseau ; quarantaine anti-fantôme |
| [0004](adr/0004-moteur-de-recherche-fts5.md) | Recherche = SQLite **FTS5** | L'index vit DANS la base (transactionnel) |
| [0005](adr/0005-gate-e2e-hors-ci-hebergee.md) | E2E hors CI hébergée | Un runner GitHub ne peut pas ouvrir WebView2 — d'où le hook `pre-push` |
| [0006](adr/0006-microsoft-imap-oauth2.md) | Microsoft via IMAP+OAuth2, pas Graph | Graph reste le plan B chiffré |
| [0007](adr/0007-rattrapage-des-corps.md) | Rattrapage des corps borné, reprenable, groupé | **Horizon levé par l'ADR 0010** ; la forme (bornée/reprenable/groupée) demeure |
| [0008](adr/0008-regroupement-en-conversations.md) | Conversations = union-find sur en-têtes RFC 5322 | **Jamais de repli par sujet** ; agrégat recalculé ; un identifiant exige une arobase |
| [0009](adr/0009-portee-des-fils-au-compte.md) | Portée d'un fil = le **compte** | « Envoyés » synchronisé ; **index partiel** sinon le gate 3 est perdu |
| [0010](adr/0010-synchronisation-integrale.md) | **Synchronisation intégrale** — tout, sans horizon ni quota | Gate < 1 Go **levé** ; **stocker ≠ regrouper** (portée = INBOX + Envoyés) ; garde d'espace disque ; avancement en % |
| [0011](adr/0011-journal-wal.md) | Journal SQLite en **WAL** | Une lecture ne bloque plus une synchro longue ; persistant, bases héritées converties |
| [0012](adr/0012-migration-visible-interruptible.md) | Migration **visible et interruptible** | L'adoption est UNE transaction rembobinable — annuler laisse `user_version` inchangé, jamais d'adoption partielle ; sonde `pending_adoption` en lecture seule, qui annonce la **portée** |
| [0013](adr/0013-installeur-nsis-maj-signee.md) | Installeur **NSIS** + mise à jour signée | **Pas MSIX** (virtualiserait `%APPDATA%`, orphelinerait la base) ; updater signé minisign, piloté depuis Rust ; signature Windows reportée ; tag GitHub = **version nue**, `latest.json` sans BOM (`scripts/faire-release.ps1`) |
| [0014](adr/0014-telemetrie-de-crash-locale.md) | Télémétrie de crash **locale, opt-in** | Fichier local seul (aucun réseau/tiers) ; panics seuls ; **message du panic supprimé** (seul vecteur de PII) ; hook qui ne touche jamais la base ; un crash thread principal fait un **double panic** (compteur `SEQ` + filtre `cannot unwind`) |
| [0015](adr/0015-socle-ui-v2-svelte.md) | **Socle UI v2 = Svelte**, front web unique porté partout (Tauri 2 desktop+mobile + navigateur) | Départage set-based (vanilla / Svelte / WASM) **sur mesure** : liste 256 k + bascule de thème, deux moteurs (Blink desktop, Android-classe CPU ×6) — rendu neutralisé par fenêtrage + thème CSS. **Système écrit une fois** (Stratégie A) ; WASM écarté, vanilla en repli ; **iOS/WKWebView : validation terrain due** ; frontière UI↔cœur = port de transport ; `mail-core` intouché (ADR 0001) |
| [0019](adr/0019-commandes-hors-du-thread-principal.md) | **Commandes bloquantes hors du thread principal**, une à la fois (`hors_pompe` = spawn_blocking + verrou global) | La pompe ne fait que pomper (gel mesuré : 25,2 s/40 s → 0) ; la sérialisation d'avant est CONSERVÉE ; gate `garde-thread-principal.mjs` + budget « aucun gel > 150 ms » (`sonde-gel.py`) |

Décisions Phase 0 ([PHASE0.md](PHASE0.md) §2) : SQLite local ; CONDSTORE ;
parsing MIME par `mail-parser` ; OAuth2 PKCE loopback + coffre OS ; rendu
HTML en défense en profondeur.

---

## 6. Invariants non négociables

Faciles à casser **en silence**. À vérifier à chaque revue.

1. **Boîte d'envoi — les deux règles d'or** (ADR 0003) : jamais d'envoi
   perdu (l'intention est journalisée AVANT tout réseau) ; jamais d'envoi
   fantôme (quarantaine, jamais de renvoi automatique). *« Le doublon est
   pire que le retard. »*
2. **Identité message = `(account_id, boîte, uid)`** partout, jusque dans
   la sélection de l'UI. Les UID sont attribués par boîte et repartent de
   1 — et depuis l'ADR 0010, un compte porte des DIZAINES de boîtes. Le
   compilateur ne protège pas cet invariant ; un test le tient
   (`chaque_ligne_dit_dans_quelle_boite_elle_habite`).
3. **Les index et agrégats vivent DANS la base**, entretenus dans la MÊME
   transaction que le message : index FTS5, table `threads`.
4. **Sécurité du rendu** : HTML assaini par `ammonia`, images distantes
   bloquées, iframe sandboxée + CSP, `textContent` jamais `innerHTML`.
5. **Credentials jamais en clair** : Credential Manager Windows via
   `keyring`.
6. **UIDVALIDITY** : si elle change, la boîte repart de zéro et **tout le
   compte** refait ses fils (`thread::rebuild_account`). Brouillons :
   *« un doublon est acceptable, supprimer le mauvais UID jamais »*.
7. **Une fonctionnalité neuve doit ADOPTER les données anciennes** — le
   piège s'est présenté quatre fois (§9). Migration écrite en même temps
   que la fonctionnalité, prouvée par un test qui rembobine une vraie
   base de fichier.
8. **Les diagnostics ne divulguent rien** : ni sujet, ni expéditeur, ni
   contenu ; identifiants **masqués** (forme seule).
9. **On stocke tout, on ne regroupe que la portée** (ADR 0010 §3). Un
   message hors de INBOX + Envoyés garde `thread_id = NULL` pour
   toujours : sans cela, un spam accroché à un fil le ferait remonter en
   tête de liste (`size`, `unseen`, `last_epoch` corrompus). Porté par
   `mailboxes.threaded` + `accounts.sent_mailbox`, tenu par
   `un_message_hors_portee_ne_rejoint_pas_le_fil`. **La portée se déclare
   sur le compte AVANT que les boîtes existent** — la boucle de synchro
   les crée (`une_portee_declaree_avant_la_creation_de_la_boite_vaut_quand_meme`).
10. **Rien ne touche la base avant `migration_check`** (ADR 0012, A41).
    Toute commande jouée avant la modale de migration doit être une
    sonde qui n'adopte pas (`Store::pending_adoption`,
    `Store::text_pref_readonly`) ; une écriture différée ne part que si
    la sonde de migration a répondu, et un échec de lecture vaut repli
    de session — jamais une écriture. Tenu par
    `la_langue_se_lit_sans_adopter_la_base` (base de fichier
    rembobinée) ; l'ordre côté UI (`main.js` → `assurer()` →
    `poserLangueDetectee()`) n'a pas de garde structurelle — à vérifier
    à chaque commande de démarrage ajoutée.

---

## 7. Environnement & commandes

Windows 11. Deux shells : **PowerShell 5.1** (principal) et **Bash** (Git
Bash). Syntaxes différentes.

### 7.1 Pièges qui coûtent cher

- **PowerShell 5.1 n'a pas `&&`.** Deux lignes, ou Bash.
- **Ne JAMAIS utiliser `Get-Content`/`Set-Content` sur les sources** :
  réencodage UTF-16 BOM, accents corrompus. Éditer via l'outil `Edit`,
  Python, ou Bash. Tout est en **UTF-8**.
- Pour un affichage non-ASCII depuis Python : `PYTHONIOENCODING=utf-8`.
- **L'assistant ne voit PAS la vraie base.** L'application Claude est
  empaquetée MSIX : son shell lit un `%APPDATA%` **redirigé**, et
  `wind.db` y résout vers une copie privée périmée. **Les
  diagnostics du §9 sont lancés par l'utilisateur**, qui colle la sortie.
  Corollaire : annoncer d'abord ce qu'on s'attend à y lire, pour que
  l'aller-retour soit une mesure et non une collecte — et transmettre
  chaque chiffre **avec sa définition exacte** (un « ~1 650 restants » lu
  comme un reliquat alors que c'était un total a coûté une prédiction
  fausse).
- **Depuis l'ADR 0011, la base a deux compagnons** : `wind.db-wal`
  et `-shm`. Une copie à chaud doit prendre les trois.
- **Un commit ne peut pas être chaîné avec `git --no-pager …`** : le hook
  `block-no-verify` bloque le préfixe `--no-`. Séparer les commandes.
- **Une commande Tauri sans `async` s'exécute sur le THREAD PRINCIPAL**
  — celui de la pompe de messages : la fenêtre gèle pour toute sa durée
  (constat 2026-08-15, gels de 2 à 4,6 s au démarrage). Toute commande
  qui ouvre la base, touche un fichier ou le keyring est `async fn` ;
  la gate `e2e/garde-thread-principal.mjs` le tient (exemption nommée
  pour les pures d'état). Mesure du symptôme :
  `python e2e/sonde-gel.py <base.db>` (base HORS dépôt).

### 7.2 Les notifications exigent l'application INSTALLÉE

`tauri-winrt-notification` exige une identité applicative
(AppUserModelID), portée par un raccourci du menu Démarrer. Donc :
`cargo tauri build`, installer, lancer depuis le menu Démarrer ;
`GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET`, `MICROSOFT_CLIENT_ID` définis
au niveau utilisateur. ⚠️ Windows n'inscrit l'application dans
Paramètres → Notifications qu'APRÈS sa première notification réussie.

### 7.3 Commandes

```bash
cargo test --workspace --all-targets           # tout, EXEMPLES COMPRIS
cargo test --workspace --doc                   # les doc-tests, exclus ci-dessus
cargo build -p wind-desktop --release     # binaire
cargo run -p wind-desktop --release       # lancer (sans notifications)

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

Mesures : `node e2e/mesure.mjs` (démarrage, page, RAM — `MESURE_DB`,
`MESURE_COMPTES`, `MESURE_REUTILISER`), `e2e/mesure-ram.ps1`, et
`python e2e/sonde-gel.py <base.db>` (gel de la pompe de messages,
budget « aucun gel > 150 ms », PLAN-GELS — exige Python 3, seul outil
du dépôt à le demander).

⚠️ **La base de mesure se place HORS du dépôt** (OneDrive perturberait la
mesure). Les trois bases du gate 3 (`gate3.db`, `gate3-corps.db`,
`gate3-envoyes.db`) sont **conservées** dans un scratchpad temporaire de
session et ont été **migrées au schéma ADR 0010** le 2026-07-26 — elles
restent valides et comparables. C'est un dossier Temp : vérifier leur
existence avant usage, regénérer par `seed_inbox` sinon (plusieurs
minutes ; ne pas le faire « pour être sûr »).

### 7.4 Le gate pré-push

`.githooks/pre-push` rejoue : `fmt` → `clippy -D warnings` →
`cargo test --workspace --all-targets` → `--doc` → `npm test` (e2e).

**`--all-targets` n'est pas décoratif** : sans lui, cargo ignore les tests
des EXEMPLES — les diagnostics du terrain vivent là et portent leurs tests.
`--no-verify` existe ; s'en servir est une décision, pas un raccourci.

**Le gate ne reflète la CI que sur la MÊME toolchain.** La version de Rust
est **épinglée** dans [`rust-toolchain.toml`](../rust-toolchain.toml)
(source unique : local + hook + CI). Le job CI, lui, ne lit pas ce
fichier : sa ref d'action est épinglée à la main dans
[`ci.yml`](../.github/workflows/ci.yml) — **monter de version se fait aux
DEUX endroits**, puis on rejoue clippy (un lint neuf peut apparaître).
Enseignement payé : la CI suivait « le dernier stable » et le hook
tournait sur une toolchain locale en retard (1.94 vs 1.97) ; un lint
clippy neuf a cassé la CI sans que le gate local le voie.

### 7.5 Déterminisme des E2E

Étanches par construction : base jetable (`WIND_DB_PATH`), comptes
factices (`WIND_E2E_ACCOUNT`), `GOOGLE_CLIENT_ID`/`SECRET` retirés,
profil WebView2 dédié. **Les E2E ne parlent à aucun serveur** : tout le
chemin réseau réel (OAuth, dossiers, passes de fond, STATUS) n'est couvert
que par des tests unitaires sur la partie pure et ne se prouve que sur le
terrain.

---

## 8. Ce qui reste

### Le chantier fait : migration visible et interruptible (ADR 0012)

Terminé et **validé au terrain** le 2026-07-26, sur copies. L'adoption
est une unité transactionnelle unique (du DROP conditionnel des tables
de fils jusqu'à `user_version`) : annuler rembobine tout, la passe se
rejoue entière au prochain lancement. Écran modal au démarrage — chaque
commande ouvre sa propre connexion, sans porte la première venue
paierait la passe en silence. Preuves : test de rembobinage sur une
vraie base de fichier, banc (3,66 s, pas de régression), annulation
exercée en pleine passe à l'échelle du gate 3.

### Le chantier fait : installeur NSIS + mise à jour signée (ADR 0013)

Terminé et **validé au terrain** le 2026-07-26 : la boucle 0.1.1 → 0.1.2
s'applique sur l'app installée, base intacte. NSIS (**pas MSIX** — il
virtualiserait `%APPDATA%` et orphelinerait la base) ; updater Tauri
signé minisign, piloté depuis Rust (capabilities au minimum) ; signature
de code Windows reportée à la bêta. Publication d'une version :
`scripts/faire-release.ps1 <version>` prépare le `latest.json`, la
Release GitHub reste manuelle (tag = version nue).

### Le chantier fait : télémétrie de crash locale et opt-in (ADR 0014)

Terminé et **validé au terrain** le 2026-07-26. Fichier local seul
(aucun réseau, aucun tiers), panics backend seuls, opt-in off par
défaut ; le **message du panic est supprimé** (seul vecteur de donnée
personnelle), prouvé à deux niveaux (mémoire et fichier écrit). Le hook
ne touche jamais la base (consentement en fichier + `AtomicBool`).
Trouvaille terrain corrigée : un crash sur le thread principal produit un
**double panic** à la frontière FFI de WebView2 — compteur `SEQ` (noms
uniques) + filtre du secondaire `cannot unwind`.

### Le chantier fait : plus aucune commande sur le thread principal (ADR 0019)

Terminé et **validé au terrain** le 2026-08-15 (PLAN-GELS, `e32280b`,
A39/A40). Le freeze du démarrage (25,2 s de gels cumulés sur 40 s,
mesurés) est mort à la racine : toute commande bloquante passe par
`hors_pompe()` — spawn_blocking + verrou global, la sérialisation
d'avant conservée — tenu par la gate `garde-thread-principal.mjs` et le
budget « aucun gel de pompe > 150 ms » (`sonde-gel.py`). Au passage, le
terrain a livré et fait corriger le jour même : l'avancement figé à
99 % par les départs en attente de rejeu (le dénominateur s'ajuste), et
la boucle du trait hitofude morte-née (animation CSS dans un `<mask>`
non rendu → SMIL). Dette ouverte : D-8 (sondes chères, hors pompe).

### Le chantier suivant : bêta fermée 20-50 utilisateurs

Dernière étape avant le gate 5 ([PLAN.md](PLAN.md) §4). Kaizen
hebdomadaire sur les frictions **observées**. Rien n'est engagé.

### La longue traîne en cours

Le rattrapage intégral des corps (~250 000 messages restants) avance à
200 par lot au fil de l'usage. Rien à coder ; surveiller le disque et le
bandeau. La recherche gagne en profondeur à mesure.

### Reports assumés

- **Requêtes chères des sondes périodiques** (PLAN-GELS D4) : hors de
  la pompe elles ne gèlent plus rien, mais leur coût CPU reste réel —
  registre **D-8** de [DETTE.md](DETTE.md), chiffres et pistes dedans.
- **Doublons multi-boîtes dans la recherche** (nouveau, ADR 0010) : le
  même message copié dans plusieurs boîtes remonte plusieurs fois dans
  les résultats. À observer en bêta avant de décider d'un dédoublonnage.
- **Tri par date de la recherche** — tranché en bêta (levier ×1,8–2,9).
- **Défilement profond** : `OFFSET` coûte ~230 ms à 150 000 conversations ;
  seule une pagination par curseur l'effacerait.
- **Envoi de pièces jointes** (lecture seule en v1) ; **filtre « a une
  pièce jointe »** ; **`to:` dans la recherche**.
- **CONDSTORE réel, IDLE/push** — reports de Phase 1 inchangés.
- **Dossier CASA Google** — chemin critique du lancement public, côté
  produit-owner.

### Dette connue, non corrigée

`apps/desktop/ui/style.css` : la règle d'élément `header { display: flex }`
s'applique aussi à `#detail-header`. Tout enfant pleine largeur ajouté là
devient un item flex écrasé à 0 px. (Le bandeau d'avancement de
l'ADR 0010 et l'écran de migration de l'ADR 0012 ont été placés **hors**
de tout `<header>` pour cette raison.)

Cousin de cette dette, désormais **tenu par une règle** : toute règle
d'ID qui pose un `display` écrase le `[hidden]` du navigateur et exige
son garde-fou `#id[hidden] { display: none }`. Huit occurrences à ce
jour ; la dernière (`#detail`) laissait l'iframe sandboxée capter le
premier clic et tuer les raccourcis clavier (§9). Un E2E tient le cas.

### La Phase 5

Installeur MSIX/NSIS + mise à jour signée, télémétrie de crash opt-in,
bêta fermée 20-50 utilisateurs, kaizen hebdomadaire sur les frictions
**observées**. Gate 5 : deux semaines sans défaut critique.

---

## 9. Enseignements — à lire avant de reprendre

Ils ont coûté cher. Les ignorer les fera repayer.

### Les défauts se trouvent sur le terrain, pas dans les tests

Jamais des erreurs de logique : toujours des **hypothèses fausses sur
l'environnement ou l'usage**. Une suite de tests partage l'hypothèse.
Dernier exemple : « database is locked » au premier essai de la
synchronisation intégrale (ADR 0011).

### Un lecteur périodique à côté d'écritures longues exige le WAL

Le mode rollback a tenu tant que les écritures duraient des secondes. La
synchronisation intégrale les a étirées en minutes, et le sondage
d'avancement (800 ms) a fait expirer le `busy_timeout` des écrivains dès
le premier essai. **Le risque avait été nommé en revue ; il fallait le
traiter à ce moment-là.** Quand on ajoute un lecteur périodique, vérifier
le mode de journal.

### Une borne héritée n'est pas une borne décidée

La passe d'en-têtes empruntait l'horizon de 12 mois du rattrapage des
corps — une borne qui existait pour le **budget disque**, alors qu'un
bloc d'en-têtes pèse ~3 ko et ne se range pas sur le disque. Reprise
parce que la fonction avait la même *forme*, pas la même *raison*. Le
diagnostic l'a montrée convergée à 1 656/1 656 avec 78 % de la base
définitivement hors de portée. **Quand on hérite d'un paramètre,
réexaminer sa raison d'être.**

### Une portée déclarée avant la création de son objet se mémorise sur le parent

La boucle de synchronisation **crée** la boîte « Envoyés » : au moment de
déclarer la portée du regroupement, il n'y a aucune ligne à mettre à
jour, et la boîte naîtrait hors portée — messages sans fil jusqu'au
prochain démarrage, sans signal. D'où `accounts.sent_mailbox`, consulté
par `create_mailbox`. **Une déclaration qui précède son objet se porte
par le parent, pas par l'ordre des appels.**

### Un diagnostic écrit pour un décor se relit dans le décor suivant

Sur la base intégrale, « jamais lus : 250 864 » mélangeait l'attente
réelle et le hors-portée délibérément ignoré — un chiffre qui ne désigne
plus rien fait relancer le diagnostic pour rien. Ventilé par portée le
jour même. **Quand le décor change (ADR), relire chaque diagnostic avec
les yeux du nouveau décor.**

### Une fonctionnalité neuve doit ADOPTER les données anciennes

Le piège s'est présenté **quatre fois** : pièces jointes, conversations,
en-têtes de fil, schéma. `CREATE TABLE IF NOT EXISTS` ne touche pas une
table existante, mais un index partiel neuf échoue sur une colonne
absente — et l'application ne démarrait plus. **Écrire la migration avec
la fonctionnalité, la prouver par un test qui rembobine une vraie base de
fichier.** (Les migrations de l'ADR 0010 — trois colonnes — ont suivi
cette règle et sont passées sans bruit sur la base réelle ET sur les
bases du gate 3.)

### Mesurer avant de corriger — y compris ses propres hypothèses

Sur le faux regroupement, trois hypothèses étaient fausses ; le
diagnostic a désigné la cause en une commande. Sur l'adoption, la cause
« dominante » annoncée ne valait qu'un quart du coût. Sept outils
existent, même modèle — lecture seule, **aucun contenu divulgué** :

| Outil | Répond à |
|---|---|
| `diagnostic_index` | les messages sont-ils dans l'index de recherche ? |
| `diagnostic_fils` | quel identifiant réunit un fil ? (ventilé par portée depuis l'ADR 0010) |
| `diagnostic_brouillons` | le tirage des brouillons fait-il son travail ? |
| `banc_page_liste` | le coût d'une page dépend-il de la taille de la boîte ? |
| `banc_migration_fils` | que coûte l'adoption d'une base héritée ? (copie `VACUUM INTO`, ne mute pas la base visée) |
| `banc_recherche` | recherche et ouverture tiennent-elles leurs budgets ? |
| `seed_inbox` | fabriquer un décor (les 500 plus récents reçoivent un corps) |

En écrire un nouveau coûte 40 lignes et fait gagner un aller-retour.

### Un test vert peut encoder un modèle FAUX de l'autre écrivain

La détection de conflit des brouillons simulait le tirage par une
réécriture en place ; le vrai tirage **remplace**. **Simuler l'autre
écrivain en appelant SON VRAI CHEMIN.** Même famille : un faux serveur
doit annoncer exactement ce qu'il sert (`FakeServer::exists` et
`message_count` renvoient le décor réel, jamais une constante).

### Une promesse d'index ne vaut que pour la requête qu'on avait en tête

L'ADR 0008 §4 raisonnait sur une boîte ; le produit interroge la boîte
unifiée — 987 ms de tri matérialisé, invisibles à l'échelle du terrain.
**Un test de PLAN D'EXÉCUTION attrape cette classe de régression.**

### Un décor de mesure peut ne jamais exercer ce qu'on croit valider

L'index partiel a vécu plusieurs jours sans qu'un fil soit jamais écarté :
le décor n'avait qu'une boîte par compte. **Vérifier que le décor produit
la condition que le code prétend traiter.** Corollaire vécu à l'ADR 0011 :
tester le WAL sur une base MÉMOIRE aurait validé un modèle faux — elle
répond « memory ».

### Un test qui ne tourne pas n'est pas un test

`cargo test --workspace` ignore les tests des exemples — d'où
`--all-targets` dans le gate (§7.4).

### Le compilateur ne protège pas une identité faite de chaînes

`account_id` et `mailbox_id` sont des `i64`, une boîte est une `String`.
Après un changement de signature, le code compilait en visant le mauvais
message. Tenir l'invariant par un test.

### Un signal demandé doit être OBSERVABLE

Vérifier dans le code que chaque signal demandé en validation est
réellement affiché — et pas écrasé une ligne plus loin. Cas d'école : une
barre d'avancement ne doit jamais dire « 0 % » quand elle ne sait pas, ni
« 100 % » tant que ce n'est pas fini (`sync_percent`, cas dégénérés
testés).

### Un statut posé sans regarder en efface un autre

Trois fois. Quand une fonction pose un message d'état, l'appelant décide
du sien à partir de son bilan. (C'est pourquoi l'avancement de la synchro
a son bandeau, séparé de la ligne de statut.)

### Ne jamais avaler une erreur

`let _ = …show()` a détruit la preuve d'un défaut de notifications. Les
échecs non bloquants remontent dans le bilan de synchro — la
synchronisation intégrale consigne l'échec de CHAQUE dossier sans jamais
bloquer les autres, et la garde d'espace disque **dit combien** il manque
plutôt que « espace insuffisant ».

### Un outil de mesure se vérifie comme le reste

`mesure-ram.ps1` sommait toutes les instances ; `mesure.mjs` n'isolait pas
son profil ; un diagnostic divulguait des identifiants en découpant un
en-tête entier sur son premier `@`. Corrigés — le réflexe reste.

### Un élément « caché » peut rester rendu — et voler le focus

`#detail { display: flex }` écrasait le `[hidden]` du navigateur
(spécificité d'un ID contre la feuille par défaut) : le panneau de
lecture était rendu en permanence, son iframe sandboxée couvrait la
moitié de la fenêtre, et le premier clic y perdait le clavier — les
raccourcis morts tant qu'on ne cliquait pas ailleurs. **Invisible aux
E2E**, qui injectent leurs touches par CDP sans passer par le focus de
la fenêtre Windows ; trouvé par le Chef Ingénieur pendant la validation
terrain d'un AUTRE chantier (ADR 0012). Deux leçons : toute règle d'ID
posant un `display` exige son garde-fou `#id[hidden]` (la classe entière
a été passée au crible) ; et les premiers gestes d'une session — cliquer
n'importe où, `/` d'emblée — sont un parcours terrain à part entière.

### Valider un écran rapide exige le décor qui le ralentit

Sur la boîte réelle, l'écran de migration vit moins d'une seconde : la
portée à adopter (~7 500 messages) est 30× plus petite que le décor du
gate 3. L'annulation en pleine passe ne s'exerce que sur `gate3.db`
rembobinée (`user_version = 0`), où la barre monte ~4 s. **Choisir le
décor pour la propriété qu'on valide, pas pour son réalisme.**

### La chaîne de publication a ses propres hypothèses fausses

La validation de l'updater (ADR 0013) a payé deux pièges, aucun dans le
code Rust — tous dans l'**outillage de publication**. Un `latest.json`
écrit à la main s'est corrompu (collage PowerShell multi-ligne, puis
risque de BOM que `serde_json` refuse). Et l'URL du paquet pointait
`releases/download/v0.1.2/…` alors que le tag GitHub est la **version
nue** (`0.1.2`) : le bandeau apparaissait — la détection marchait — mais
l'installation renvoyait 404. **Le chemin entre `cargo tauri build` et
l'app de l'utilisateur est du terrain lui aussi ; il se diagnostique en
regardant les vrais assets publiés (API GitHub), pas en supposant.** Les
deux sont désormais tenus par `scripts/faire-release.ps1`.

### Le thread d'une commande est une décision, pas un détail

Dans Tauri 2, une commande sans `async` s'exécute sur le thread
principal — la pompe de messages. Trente-quatre commandes ouvraient la
base depuis ce thread ; tout allait bien tant qu'elles restaient sous
~100 ms, puis un lot de rattrapage de 130 Mo a gelé la fenêtre 4,6 s
d'un tenant (constat CE du 2026-08-15 : « la fenêtre ne peut pas être
déplacée »). Le coût des requêtes n'était pas la racine — leur PLACE
l'était : 865 ms sont acceptables sur un thread de fond, inacceptables
sur la pompe. Remède à la racine : toute commande bloquante est
`async`, une gate le tient (exemption nommée et justifiée pour les
pures d'état), et le symptôme a son instrument — `sonde-gel.py` mesure
la pompe comme Windows la juge (`SendMessageTimeout`). Avant/après sur
le même décor : 25,2 s de gels cumulés → zéro.

### Un panic sur le thread principal fait DEUX panics

La capture de crash (ADR 0014) s'est prouvée juste en test, mais le
terrain a montré un comportement qu'aucun test unitaire ne voyait : un
panic sur le thread principal tente de se dérouler, traverse la frontière
FFI de WebView2 (nounwind), et déclenche un SECOND panic `cannot unwind`
qui aborte. Le hook s'exécute pour les deux, dans la même seconde — le
second écrasait le premier (le seul utile). Corrigé par un compteur dans
le nom de fichier et un filtre du panic secondaire. **Le comportement de
l'environnement au moment d'un crash ne se voit qu'en crashant pour de
vrai.**

---

## 10. Carte des fichiers

| Fichier | Rôle |
|---|---|
| [`docs/PLAN.md`](PLAN.md) | Concept paper — source de vérité produit |
| [`docs/adr/`](adr/) | Les 15 décisions gelées |
| [`docs/PHASE0.md`](PHASE0.md) → [`PHASE3.md`](PHASE3.md) | Revues de clôture |
| [`crates/mail-core/src/store.rs`](../crates/mail-core/src/store.rs) | Stockage SQLite (WAL), schéma, migrations, boîte unifiée, portée du regroupement |
| [`crates/mail-core/src/sync.rs`](../crates/mail-core/src/sync.rs) | Moteur de synchro + `sync_order`, `sync_percent`, `disk_shortfall` |
| [`crates/mail-core/src/thread.rs`](../crates/mail-core/src/thread.rs) | Conversations : union-find pur + persistance, portée compte |
| [`crates/mail-core/src/drafts.rs`](../crates/mail-core/src/drafts.rs) | Brouillons : poussée, tirage, conflit d'édition |
| [`crates/mail-core/src/outbox.rs`](../crates/mail-core/src/outbox.rs) | Boîte d'envoi + règles d'or |
| [`crates/mail-core/src/search.rs`](../crates/mail-core/src/search.rs) | Index FTS5 contentless, transactionnel |
| [`crates/mail-core/src/backfill.rs`](../crates/mail-core/src/backfill.rs) | Rattrapage des corps ET passe d'en-têtes — `NO_HORIZON` depuis l'ADR 0010 |
| [`crates/mail-core/src/test_support.rs`](../crates/mail-core/src/test_support.rs) | `FakeServer` — rejoue les bizarreries du terrain |
| [`crates/mail-core/examples/`](../crates/mail-core/examples/) | 3 diagnostics + 3 bancs + `seed_inbox` |
| [`crates/mail-imap/src/convert.rs`](../crates/mail-imap/src/convert.rs) | Traduction IMAP → domaine ; découverte archive et envois |
| [`crates/mail-auth/src/provider.rs`](../crates/mail-auth/src/provider.rs) | Fournisseurs OAuth décrits **en données** |
| [`apps/desktop/src/commands.rs`](../apps/desktop/src/commands.rs) | Commandes Tauri (IPC), boucle toutes-boîtes, garde disque, avancement |
| [`apps/desktop/ui-v2/src/App.svelte`](../apps/desktop/ui-v2/src/App.svelte) | L'UI (Svelte 5, seule depuis B2/PLAN-RETRAIT-V1) : écrans 01-04, fente d'avis, cycle de synchro automatique |
| [`e2e/README.md`](../e2e/README.md) | Harnais E2E déterministe (CDP) |
| [`scripts/faire-release.ps1`](../scripts/faire-release.ps1) | Prépare le `latest.json` signé d'une version (ADR 0013) — sans BOM, URL au tag nu |
| [`crates/mail-core/src/crash.rs`](../crates/mail-core/src/crash.rs) | Rédaction PURE d'un rapport de crash — écarte le message (PII) (ADR 0014) |
| [`apps/desktop/src/telemetry.rs`](../apps/desktop/src/telemetry.rs) | Panic hook, consentement en fichier, écriture locale du rapport (ADR 0014) |
| [`spikes/ui-socle-v2/`](../spikes/ui-socle-v2/RAPPORT.md) | Spike de départage du socle UI v2 — preuve de l'ADR 0015, **jetable** |

---

*Vos mails, instantanément. La performance et la fiabilité ne sont pas des
options — ce sont les fonctionnalités.*
