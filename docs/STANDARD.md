# Standard — le standard de travail de Wind

> **Ce document est l'instruction permanente du projet** : méthode
> (§2), produit (§3), architecture (§4), décisions gelées (§5),
> invariants (§6), environnement (§7), enseignements (§9). Il
> s'**amende par kaizen** — un constat, un amendement — et ne se
> réécrit pas. **L'état courant** (version livrée, prochain chantier,
> chiffres du terrain) vit dans [ETAT.md](ETAT.md), l'instantané de
> relève.
>
> Né de la scission de PASSATION.md le 2026-08-19
> (PLAN-DOCUMENTATION, décisions CE D1-D2). **La numérotation §2-§10
> est figée** : toute référence externe (« §2.9 », « §7.1 ») reste
> vraie.
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
> projet et tu appliques la méthode décrite dans `docs/STANDARD.md` §2 —
> c'est une instruction permanente, elle prime sur tout. Lis d'abord ce
> document en entier, puis lis `docs/ETAT.md`.

Ordre de lecture, une fois :

1. **ce document** — méthode, invariants, pièges ;
2. [`docs/ETAT.md`](ETAT.md) — où on en est, quoi faire en premier ;
3. [`docs/PLAN.md`](PLAN.md) — le concept paper, source de vérité produit ;
4. les ADRs dans [`docs/adr/`](adr/) — **décisions gelées**, à ne pas
   rouvrir sans mesure contraire.

Ne lis pas le code avant. Il est volumineux et abondamment commenté ; les
commentaires expliquent *pourquoi*, et supposent le contexte ci-dessous.

---

## 1. Où on en est → [ETAT.md](ETAT.md)

L'état courant — version livrée, prochain chantier, chiffres du
terrain, arbitrages ouverts — vit dans [ETAT.md](ETAT.md),
l'instantané de relève, réécrit à chaque chantier : c'est sa
fonction.

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

### 2.9 Numérotation des versions

**Wind suit un format `x.y.z`, où `x` = MAJEUR, `y` = MINEUR, `z` =
CORRECTIF.** Wind n'expose aucune API publique : le « contrat » dont la
rupture vaut MAJEUR est redéfini sur les **deux seules choses que
l'utilisateur ne peut pas réparer seul** — la chaîne d'auto-update et la
survie de sa boîte.

On descend, on s'arrête au premier « oui » :

1. **MAJEUR** (`x`+1, puis `y` et `z` → 0) — si **l'un** est vrai :
   - la version **ne s'atteint pas par auto-update** depuis la précédente
     (réinstallation manuelle : rotation de clé de signature, changement
     d'installeur/format — **c'est arrivé en 0.1.3**). Depuis le retour
     du canal x64 (PLAN-RETOURS-8, ADR 0023), il y a **deux chaînes
     d'auto-update** (arm64 et x64) : le critère s'évalue **par
     canal**, et une rupture sur UN seul canal suffit à déclencher
     MAJEUR. Ajouter un canal ne casse rien (l'updater de chaque poste
     ne lit que sa clé `{os}-{arch}`) — retirer ou casser un canal, si ;
   - elle embarque une **migration de données non rembobinable** (contraire
     à l'[ADR 0012](adr/0012-migration-visible-interruptible.md)) ;
   - \+ le passage **unique** `0.x → 1.0.0` au jalon « hors développement
     initial » (sortie de bêta) — décision produit du shusa.
2. **MINEUR** (`y`+1, puis `z` → 0) — si la release **ajoute au moins une
   capacité nouvelle** visible par l'utilisateur.
3. **CORRECTIF** (`z`+1) — si la release n'inclut que des corrections,
   ajustements de l'existant, perf, allègements internes, nettoyages.

La release se publie par `scripts/faire-release.ps1 <version>`
([ADR 0013](adr/0013-installeur-nsis-maj-signee.md), bi-arch depuis
[ADR 0023](adr/0023-retour-canal-x64.md) : deux builds `--target`,
arm64 natif + x64 en cross-build local, **tout-ou-rien** — un build en
échec bloque toute la release, jamais un canal décalé) ; le tag GitHub
reste la **version nue**.

⚠️ **Les notes utilisateur D'ABORD, systématiquement** : écrire (et
committer) l'entrée `## [<version>]` de `CHANGELOG.md` **avant** de
lancer le script — il refuse net sans elle (« CHANGELOG.md n'a pas
d'entree… »), c'est son premier contrôle. Oubli commis **au moins trois
fois** en session (dernière : 0.2.1, 2026-08-20) : le réflexe fait
partie de la préparation de release, pas de l'après-coup.

### 2.10 Vérifier une release publiée

Depuis la 0.1.10 (2026-08-18), `scripts/faire-release.ps1 <v>` fait
**toute** la release (validé au terrain) — à condition que l'entrée
`## [<v>]` du CHANGELOG existe déjà (§2.9, son premier contrôle) :
bump de la seule ligne
`version` de `apps/desktop/tauri.conf.json`, **deux builds signés**
(arm64 natif + x64 cross, bi-arch depuis PLAN-RETOURS-8/ADR 0023 ;
clé au **chemin** `C:\Keys\wind.key` — `TAURI_SIGNING_PRIVATE_KEY`
accepte un chemin ; mot de passe saisi une fois), `latest.json` sans
BOM à **deux clés de plateforme**, puis — après confirmation `OUI` —
commit `release: version <v>`, push (gate rejouée), tag NU + Release
GitHub `--latest` à **cinq assets**, notes tirées du CHANGELOG.

Contrôle **a posteriori**, avant d'annoncer verte :
**`scripts/verifier-release.ps1 <v>` joue tous les contrôles de forme**
(la friction est encodée une fois — avec deux plateformes, les
contrôles manuels doublaient). Ce qu'il vérifie, et qui reste la norme
si on contrôle à la main :

- **La Release est « Latest »** — l'endpoint updater est
  `…/releases/latest/download/latest.json` :
  `gh api repos/smonchamps/wind/releases/latest --jq '.tag_name'`
  doit rendre la nouvelle version.
- **Cinq assets au tag NU** (jamais `v<x>`), nommés exactement :
  `Wind_<v>_arm64-setup.exe` + son `.sig`, `Wind_<v>_x64-setup.exe` +
  son `.sig`, `latest.json`. (« Cinq » ne suffit pas : deux exe de la
  même architecture passeraient un simple comptage.)
- **`latest.json` sans BOM** (premiers octets `7b` = `{`, pas
  `ef bb bf` — serde_json le refuse en silence).
- **Les DEUX clés de plateforme présentes** (`windows-aarch64` ET
  `windows-x86_64`) : une clé manquante est une **panne silencieuse**
  — l'updater du canal muet conclut « pas de mise à jour », sans
  erreur. Même famille que le BOM et le tag `v` (ADR 0013).
- **Par plateforme** : signature du manifeste == fichier `.sig` de la
  MÊME architecture ; URL au tag NU (`/releases/download/<v>/…` — le
  piège du 404) vers l'exe de la MÊME architecture ; l'URL résout
  (302 puis 200, `Content-Length` = taille de l'asset).
- **Signatures arm64 et x64 DISTINCTES** (garde anti-croisement) :
  une signature copiée sous la mauvaise clé passe tous les contrôles
  de forme et ne casse que chez l'utilisateur.
- **La crypto minisign n'est PAS vérifiable localement** (pas de
  `minisign` sur ce poste ; `tauri signer` n'a pas de `verify`). Ne
  jamais forger un PASS : la preuve définitive est l'**auto-update
  `<n-1> → <n>` constaté au terrain, PAR CANAL** — arm64 sur ce
  poste ; x64 sur le second poste x64 (décision CE D5,
  PLAN-RETOURS-8). Le premier auto-update x64 n'est constatable qu'à
  la release SUIVANT la première release bi-arch (aucun n-1 x64
  n'existe avant elle) ; l'install x64, elle, se constate dès la
  première.
- `CHANGELOG.md` (racine) porte l'entrée `## [<v>] - <date>` et le
  lien vers la Release en pied.

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
| **Recherche** | < 100 ms | **~66 ms ✅** (terrain, vraie base 251 k / 7 Go, pire cas préfixe 3 car. 36 k corr. ; tenu par la **soupape tri-date** au-delà de 10 k corr., le plancher BM25 dépassant sinon — `WIDE_QUERY_THRESHOLD`, A50/PLAN-RECHERCHE) |
| **Adoption d'une base héritée** | < 1 s | **3,66 s — assumé** (ADR 0012 : une seule fois, visible, annulable, rembobinable) |
| **Reconstruction de l'index de recherche** | pas de gel muet | **~4 min à froid sur 7 Go — assumé** (ADR 0012 : une seule fois à la MAJ, visible, annulable, rembobinable ; PLAN-RECHERCHE E3) |

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

Décisions Phase 0 ([PHASE0.md](archives/PHASE0.md) §2) : SQLite local ; CONDSTORE ;
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
   **Exception unique, bornée (A62)** : l'éditeur riche du composeur
   pose par `innerHTML` — c'est sa fonction — mais n'accepte QUE du
   HTML passé par la frontière ammonia côté Rust (`frontiere_corps`,
   citations comprises). Les images distantes s'y décident PAR GESTE
   (verdict terrain D5, 2026-08-20) : une RÉPONSE cite au pixel neutre —
   la revue du 2026-08-20 a montré le piège exact, une citation assainie
   en `AllowRemote` chargeait les pixels espions du message cité au
   simple clic « Répondre » (la CSP du document principal laisse
   `img-src https:`) ; un TRANSFERT, lui, CONSERVE les images — le
   destinataire reçoit le message entier, et composer le transfert vaut
   « afficher les images » implicite, c'est le geste qui le dit.
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
- **L'app en `--release` est MUETTE en console** (`main.rs` :
  `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]`) :
  sous-système *windows*, aucune console attachée, `eprintln` (la trace
  `run_sync`) n'a nulle part où s'écrire. Pour lire une trace au terrain :
  soit **débogage** (`cargo run -p wind-desktop`, console attachée, mais
  durées CPU gonflées), soit **rediriger via un lanceur qui ATTEND**
  (`cargo run … --release 2> fichier` — cargo, appli console, tient le
  handle jusqu'au bout ; timing release exact). ⚠️ **Lancer l'exe NU ne
  trace RIEN** (`& …\wind-desktop.exe 2> fichier` depuis PowerShell) :
  PowerShell n'attend pas un exécutable fenêtré et cesse de lire son
  stderr aussitôt l'invite rendue — fichier créé, vide À JAMAIS, même
  quand les traces partent bel et bien. Payé deux fois : PLAN-RETOURS-2
  (un « pas de trace » pris pour « pas de synchro »), PLAN-RETOURS-5
  (deux passes terrain brûlées sur un fichier vide, 2026-08-21).
- **Un commit ne peut pas être chaîné avec `git --no-pager …`** : le hook
  `block-no-verify` bloque le préfixe `--no-`. Séparer les commandes.
- **`prefers-color-scheme` est MORT dans le WebView2 de Tauri** : jamais
  sombre, zéro événement, même sous une vraie bascule Windows (mesuré
  aux sondes, terrain A42 du 2026-08-16). L'écoute du thème OS passe
  par l'API fenêtre Tauri (`theme()` + `onThemeChanged`) ; `matchMedia`
  n'est que le repli hors Tauri et la poignée du banc (emulateMedia).
  Corollaires : `Set-ItemProperty` sur `AppsUseLightTheme` ne prévient
  PERSONNE (pas de `WM_SETTINGCHANGE`) — une vérification terrain passe
  par les Paramètres Windows ou `e2e/bascule-sombre.ps1` ; et le profil
  WebView2 de la suite (`target/e2e/webview2`) PERSISTE entre les runs —
  un test mort après avoir armé un réglage localStorage empoisonne les
  relances locales (remède : purger le dossier ; la CI, elle, part
  toujours propre).
- **Le fil de lecture est UN objet, DEUX cadres** (UI v3, A43,
  2026-08-16) : `Fil.svelte` + état module `lib/fil.svelte.js`, et
  l'exclusivité des cadres vit au store (`fil.cadre` :
  null/volet/plein) — jamais de booléen local de visibilité dans un
  cadre (trois booléens réconciliés à la main se sont désynchronisés
  au premier chemin oublié, revue v3). Corollaires : toute purge passe
  par `fermerFil()` (importable partout — `lecture?.fermer()` était un
  no-op en 1-2 volets) ; chaque `ouvrirFil` RECHARGE (la mémoïsation
  cachait la propre réponse envoyée) ; le chrono P1 « ouverture »
  mesure désormais sélection → fil affiché (thread_messages compris,
  pièces exclues) — série d'avant v3 non comparable. Leçon de banc :
  un `sed` de testids peut DÉSARMER des assertions discriminantes —
  re-scoper au cadre (`[data-testid="volet-lecture"] …`) et asserter
  l'unicité (`toHaveCount(1)`).
- **Les barres de défilement sont NATIVES en surimpression** (A44,
  2026-08-16) : trait Chromium `OverlayScrollbar`, posé par
  `additionalBrowserArgs` de tauri.conf.json — ce champ s'épelle SANS
  « uments », et sa pose REMPLACE les `--disable-features` par défaut
  de wry (repris dans la valeur). Trois pièges mesurés : la variable
  `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` ÉCRASE la conf au niveau du
  loader — tout lanceur qui la pose doit reprendre les args de prod
  (`e2e/args-navigateur.mjs`, source unique : launch, mesure-v2,
  diag-v2) ; un `--enable-features` RÉPÉTÉ n'est pas fusionné, le
  dernier gagne ; `scrollbar-width:auto` ne désarme PAS des règles
  webkit (valeur par défaut — il faut une valeur non-défaut pour
  sonder le chemin natif). UNE règle `::-webkit-scrollbar` /
  `scrollbar-width` / `scrollbar-color` quelque part fait retomber
  l'élément au chemin classique (~15 px de gouttière) — la garde n°5
  de `coherence-systeme.mjs` le bloque ; le `color-scheme` (poignée
  claire en -nuit) vit en CSS à côté des jetons ET baké dans l'iframe
  du corps (mail-render, luminance du fond).
- **La liste est à DEUX gabarits depuis A44** (terrain 2026-08-16 :
  hauteur au contenu — le rang de puces n'existe que sur les lignes
  porteuses) : la mécanique de fenêtrage d'avant A29 est de retour
  (h1/h2 sondés, `chipsParPage`, `chipsAvant`, index itératif, ancrage
  au delta d'une page resservie). Toute variante de rangée neuve doit
  entrer dans les DEUX sondes, et le banc P1 se lit avec h1 ET h2
  (D-14 : re-base).
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

**Deux gates peuvent jouer en même temps** (deux worktrees, deux push) :
depuis PLAN-ISOLATION-E2E (2026-08-15), chaque suite e2e reçoit un port
CDP libre choisi par l'OS (`e2e/port-cdp.mjs`, un port par suite — les
arguments navigateur d'un même profil WebView2 doivent rester
identiques), et le balayage de zombies de `rebuild-v2.mjs` est borné au
`target/` du worktree courant. Avant : port 9222 partagé + balayage
global = applications abattues en `0xFFFFFFFF` sans sortie et suites qui
se pilotaient l'une l'autre (`connectOverCDP` reconnaît sa fenêtre au
seul critère `tauri.localhost`). Terrain : 73 + 73 verts simultanés.

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

## 8. Ce qui reste → [ETAT.md](ETAT.md)

Chantiers récents, longue traîne et reports assumés vivent dans
[ETAT.md](ETAT.md) ; la dette détaillée, dans [DETTE.md](DETTE.md).

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

Un troisième piège, même famille (constat CE du 2026-08-22) : les **notes
de release sont parties en mojibake** (« Ã© » pour « é ») sur neuf
versions (0.1.10 à 0.6.0). Racine : le script lisait le CHANGELOG UTF-8
par `Get-Content -Raw` **sans `-Encoding UTF8`** ; invoqué par
`powershell` (Windows PowerShell 5.1, encodage par défaut cp1252), il
décodait l'UTF-8 en Latin-1, puis `WriteAllText` le ré-encodait en
UTF-8 — double encodage. Le code Rust est hors de cause, encore : le
corps de l'app était propre, seules les notes de la Release GitHub
étaient touchées — invisibles à la gate, visibles au terrain (ici sur
la page des Releases). Remède à la racine : `-Encoding UTF8` sur les
trois lectures de fichiers UTF-8 du script (dont `tauri.conf.json`,
même piège latent dès qu'un accent y entrerait) ; les neuf Releases
réparées à la main depuis les sections propres du CHANGELOG, via
`gh release edit --notes-file` par un chemin qui ne ré-encode pas.
**Un script de publication qui lit de l'UTF-8 sous PowerShell 5.1 doit
le dire — le défaut de la coquille n'est pas l'UTF-8.**

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

### Une bibliothèque tierce livre ce qu'elle livre, pas ce qu'on suppose

PLAN-RETOURS-MAIL a payé deux hypothèses fausses sur des bibliothèques,
et une capture terrain a tranché la troisième. `imap-proto` retire les
guillemets d'une `quoted-string` IMAP mais **laisse les backslash-escapes
dans le contenu** (`\"`, `\\`) — prouvé par ses propres tests ; nos
objets à guillemets s'affichaient parasités. `ammonia`, lui, retire une
balise interdite mais **déballe son texte** par défaut (hors
`clean_content_tags`) : le `<head><title>` d'une infolettre fuyait en
tête de corps. Aucune des deux ne se devine — elles se **lisent dans la
source de la crate** (ou son comportement mesuré). Et sur le doublon
d'objet, mes deux premières hypothèses (le `<h1>` du corps, le préheader
démasqué) étaient fausses : c'est la **capture Gmail-vs-Wind du CE** qui
a désigné le vrai coupable, le `<title>`. **Quand un rendu diffère d'un
client de référence, la capture comparée vaut dix hypothèses.**

### Un correctif de décodage ne répare pas les données déjà décodées

Le dé-échappement neuf ne nettoyait que les enveloppes NEUVES ; les
objets déjà en base gardaient leurs escapes (la synchro incrémentale ne
relit pas l'existant). Comme pour les aperçus (D-5) et les fils, **un
changement de décodage exige une passe sur l'existant** — ici une
migration qui dé-échappe la valeur stockée (équivalente au nouveau
décodage : le contenu est déjà RFC 2047-décodé, seule reste la couche
d'escape IMAP). Le réflexe des quatre pièges d'adoption (§6.7), sous une
autre forme.


### Une mesure d'I/O disque ne vaut qu'à froid

Mesurer une reconstruction ou une migration **liée au disque** sur une
copie fraîchement écrite (`Copy-Item`) est un mensonge : la copie
laisse ses pages en cache RAM, la relecture est servie par la mémoire.
Fait mesuré (PLAN-RECHERCHE, 2026-08-17) : reconstruction FTS5 sur
7 Go / 130 k corps — **0,7 s** sur copie fraîche, **~4 min** à froid au
terrain, écart **×340** (annoncé « ×5-10 au pire »). Le coût dominant
n'est pas le calcul mais la **relecture des corps** depuis le disque —
invisible sur cache chaud. Corollaire produit : tout changement de
schéma FTS5 force une reconstruction qui relit les corps ; sur une
base fournie, la sortir du chemin de démarrage (modale ADR 0012,
`pending_adoption` la détecte). **Ne jamais conclure « budget tenu »
sur une mesure de labo quand le chemin réel est lié au disque.**

### Un contenteditable n'est ni un input ni un textarea — trois pièges payés

Payés le même jour (PLAN-COMPOSITION-HTML, e2e du 2026-08-20) :

1. **Playwright `fill('')` est un no-op** dessus — `insertText` d'une
   chaîne vide ne supprime pas la sélection dans Chromium. Vider comme
   l'utilisateur : Ctrl+A puis Suppr. Et `fill(texte)` écrit dans
   l'élément **focalisé** au moment de l'insertion (pas atomique comme
   sur un input) : toute pré-mise au point programmée du focus peut
   détourner la frappe vers un autre champ — la garde « un focus déjà
   posé prime » est produit, pas test.
2. **Les routeurs clavier ne le voient pas** : un garde
   `instanceof HTMLInputElement || HTMLTextAreaElement` laisse ses
   touches fuir vers les raccourcis globaux (Suppr supprimait la
   conversation pendant la frappe). Ajouter `isContentEditable` à toute
   détection de saisie.
3. **Sa re-sérialisation n'est jamais fidèle** : relire `innerHTML`
   d'un contenu qu'on vient d'y poser rend des styles et entités
   normalisés — toute détection « contenu identique » qui compare au
   stocké se déclenche à tort (churn). Sans frappe de l'utilisateur,
   ré-émettre les valeurs stockées, jamais le DOM.

---

## 10. Carte des fichiers

| Fichier | Rôle |
|---|---|
| [`docs/ETAT.md`](ETAT.md) | L'instantané de relève — état courant, réécrit à chaque chantier |
| [`docs/PLAN.md`](PLAN.md) | Concept paper — source de vérité produit |
| [`docs/adr/`](adr/) | Les 15 décisions gelées |
| [`docs/archives/`](archives/) | Plans soldés et revues de clôture des phases |
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
| [`scripts/faire-release.ps1`](../scripts/faire-release.ps1) | **Toute** la release (ADR 0013, bi-arch ADR 0023) : bump, deux builds signés arm64 + x64 (tout-ou-rien), `latest.json` deux plateformes sans BOM, commit + push + Release Latest au tag nu |
| [`scripts/verifier-release.ps1`](../scripts/verifier-release.ps1) | La vérification §2.10 scriptée — 5 assets nommés, BOM, deux clés de plateforme, signatures == `.sig` et distinctes, URL qui résolvent |
| [`crates/mail-core/src/crash.rs`](../crates/mail-core/src/crash.rs) | Rédaction PURE d'un rapport de crash — écarte le message (PII) (ADR 0014) |
| [`apps/desktop/src/telemetry.rs`](../apps/desktop/src/telemetry.rs) | Panic hook, consentement en fichier, écriture locale du rapport (ADR 0014) |
| [`spikes/ui-socle-v2/`](../spikes/ui-socle-v2/RAPPORT.md) | Spike de départage du socle UI v2 — preuve de l'ADR 0015, **jetable** |

---

*Vos mails, instantanément. La performance et la fiabilité ne sont pas des
options — ce sont les fonctionnalités.*
