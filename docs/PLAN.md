# Plan Chef Ingénieur — Client email simple et performant (Windows + Web)

> Rédigé dans l'esprit du *shusa* Toyota : un Chef Ingénieur qui porte la vision produit,
> fige tôt les points durs, explore les alternatives en parallèle (set-based engineering),
> et construit la qualité dans le processus (jidoka) plutôt qu'en fin de chaîne.

---

## 1. Le concept paper (la vision du CE)

**Promesse produit :** *« Vos mails, instantanément. »* Un client email qui démarre en
moins d'une seconde, où chaque action (ouvrir, archiver, chercher) répond en moins de
100 ms, et qui fonctionne hors-ligne comme en ligne.

**Client cible :** le professionnel ou particulier exigeant, 1 à 4 comptes (Gmail,
Outlook/Microsoft 365, IMAP générique), lassé de la lourdeur d'Outlook et du web-mail.

**Ce que le produit EST :**
- Rapide : la performance est LA fonctionnalité, pas une optimisation.
- Simple : lire, trier, chercher, écrire. Rien d'autre au lancement.
- Fiable : jamais de perte de mail, jamais d'envoi fantôme, offline-first.
- Sûr : credentials dans le coffre de l'OS, HTML assaini, images distantes bloquées par défaut.

**Ce que le produit N'EST PAS (v1) :** pas de calendrier, pas de chat, pas d'IA intégrée,
pas de plugins, pas de mobile. Le CE refuse toute dérive de périmètre : chaque ajout se
paie en vitesse et en fiabilité.

**Objectifs chiffrés (équivalent du « target costing » Toyota) :**

| Métrique | Cible | Mesurée dès |
|---|---|---|
| Démarrage à froid (fenêtre utilisable) | < 1 s | Phase 1 |
| Ouverture d'un message | < 50 ms | Phase 1 |
| Recherche sur 100 000 messages | < 100 ms | Phase 3 |
| RAM en usage courant | < 200 Mo | Phase 1 |
| Base locale (3 comptes, corps rattrapés) | < 1 Go | Phase 3 ([ADR 0007](adr/0007-rattrapage-des-corps.md)) |
| Défilement de la liste | 60 fps | Phase 1 |
| Taille de l'installeur Windows | < 15 Mo | **mesuré : 4,75 Mo** (NSIS, 2026-07-21) |
| Perte de données | 0, prouvé par tests de crash-récupération | Phase 2 |

Ces budgets sont des **gates bloquants** : une phase ne se termine pas si un budget est
dépassé (andon — on arrête la ligne).

---

## 2. Phase 0 — Kentou : étude et front-loading (2 à 3 semaines)

Chez Toyota, les problèmes durs se résolvent AVANT le développement, pas pendant.

### 2.1 Genchi genbutsu — aller voir sur le terrain
- Décortiquer 5 clients existants : Outlook (lourdeur), Thunderbird (architecture),
  Superhuman (vitesse perçue, raccourcis), Mailspring (moteur C++ mailsync + UI JS),
  Hey (opinions produit). Noter ce qui rend chacun lent ou rapide.
- Interroger 8 à 10 utilisateurs réels : quel est le moment le plus frustrant de leur
  journée email ? (Hypothèse à valider : le tri du matin et la recherche.)

### 2.2 Recherche & réutilisation (obligatoire avant d'écrire du code)
Ne pas réécrire ce qui existe et fonctionne :
- **Pimalaya `email-lib`** (Rust) : abstraction IMAP/SMTP/Maildir éprouvée (base du CLI Himalaya).
- **Crates Stalwart** : `mail-parser`, `mail-send`, `mail-builder`, `imap-codec` — parsing/envoi MIME de qualité production.
- **Delta Chat `core`** (Rust) : meilleur moteur de synchro IMAP open-source en Rust ; à étudier pour les patterns de sync, pas forcément à intégrer.
- Crates candidates : `async-imap`, `oauth2`, `keyring` (Windows Credential Manager), `rusqlite`/`sqlx` + FTS5, `tantivy` (alternative recherche), `ammonia` (sanitisation HTML), `lettre`.

### 2.3 Les 4 problèmes durs à résoudre par spike (prototype jetable, 2-4 jours chacun)
1. **Moteur de synchro** : IMAP incrémental (CONDSTORE/QRESYNC quand dispo), réconciliation
   locale/serveur, file d'actions offline rejouable. C'est le cœur du produit.
2. **Le pont web** : un navigateur ne peut PAS parler IMAP (pas de socket TCP brut).
   Le client web exige donc un service backend de synchro. Décision structurante à figer ici.
3. **Rendu HTML des emails** : assainissement (XSS), blocage des images distantes,
   isolation (iframe sandbox / webview CSP), sans casser la mise en page des newsletters.
4. **OAuth2 Gmail/Microsoft** : flux desktop (loopback PKCE), stockage des tokens,
   et surtout le processus de **vérification Google des scopes restreints (audit CASA)** —
   long, coûteux, à démarrer très tôt.

### 2.4 Set-based concurrent engineering — explorer, puis éliminer
Explorer en parallèle, converger par élimination sur critères mesurés (pas d'avis, des chiffres) :

| Décision | Option A | Option B | Option C | Critère d'élimination |
|---|---|---|---|---|
| Shell Windows | **Tauri 2 (WebView2)** | Slint/egui natif | Electron | RAM, taille, vitesse de dev, réutilisation web |
| UI partagée | **TS/React partagé desktop+web** | UI natives séparées | — | coût de double maintenance |
| Stockage local | **SQLite + FTS5** | SQLite + Tantivy | fichiers Maildir | perf recherche 100k msgs |
| Accès Microsoft | **IMAP+OAuth** ✅ tranché | ~~Graph API~~ (plan B) | les deux | fiabilité, quotas, effort |
| Web | Backend de synchro mutualisé | Cœur Rust en WASM + proxy WebSocket | — | coût d'infra, confidentialité |

Les options en gras sont les hypothèses de départ du CE ; les spikes les
confirment ou les tuent. **L'accès Microsoft a été tranché contre
l'hypothèse initiale** : le spike a réfuté l'argument décisif de Graph
(« IMAP est condamné ») et mesuré une asymétrie d'effort écrasante —
voir [ADR 0006](adr/0006-microsoft-imap-oauth2.md). Graph reste le plan B,
chiffré, avec ses trois signaux de bascule.

**Livrable de Phase 0 :** concept paper finalisé + décisions gelées + budgets de perf
validés sur prototypes. **Gate :** revue de conception ; on ne code la v1 qu'après.

---

## 3. Architecture cible (hypothèse à valider en Phase 0)

```
┌────────────────────────┐   ┌────────────────────────┐
│   Desktop Windows      │   │        Web             │
│   Tauri 2 + WebView2   │   │   SPA (même UI TS)     │
│   UI TypeScript        │   │                        │
└───────────┬────────────┘   └───────────┬────────────┘
            │ IPC Tauri                  │ HTTPS/WebSocket
┌───────────▼────────────┐   ┌───────────▼────────────┐
│      mail-core (Rust)  │   │  sync-server (Rust)    │
│  ─ domaine (Message,   │   │  = même mail-core,     │
│    Thread, Folder…)    │   │  hébergé, multi-tenant │
│  ─ moteur de synchro   │   └───────────┬────────────┘
│  ─ file d'actions      │               │
│  ─ SQLite + FTS5       │        IMAP / SMTP / Graph
│  ─ IMAP/SMTP/OAuth     │
└───────────┬────────────┘
     IMAP / SMTP / Graph
```

**Principe clé : un seul cerveau.** `mail-core` (crate Rust) contient 100 % de la logique
métier, de la synchro et du stockage. Le desktop l'embarque en local ; le web l'exécute
côté serveur. L'UI (TypeScript) est partagée entre les deux cibles et reste « bête » :
elle affiche l'état et émet des intentions.

**Organisation du workspace Cargo :**

```
discovery/
├── crates/
│   ├── mail-core/        # domaine + synchro + stockage (zéro dépendance UI)
│   ├── mail-protocols/   # IMAP, SMTP, Graph, OAuth (derrière des traits)
│   └── sync-server/      # exposition HTTP/WS de mail-core (phase 4)
├── apps/
│   ├── desktop/          # Tauri 2
│   └── web/              # SPA (phase 4)
└── docs/                 # ce plan, ADRs, budgets de perf
```

**Modèle de données (inspiré du modèle JMAP, plus sain que le modèle IMAP) :**
`Account`, `Mailbox`, `Email` (enveloppe séparée du corps), `Thread`, `PendingAction`.
Synchro « enveloppes d'abord » : la liste est utilisable immédiatement, les corps sont
chargés à la demande et mis en cache.

**Sécurité (points non négociables) :**
- Jamais de credential en dur ni en clair : tokens OAuth dans le Credential Manager
  Windows via `keyring` ; mots de passe IMAP chiffrés au repos.
- TLS partout (`rustls`), pas de fallback non chiffré.
- HTML assaini par `ammonia` + iframe sandboxée + CSP stricte ; images distantes
  bloquées par défaut ; pas d'exécution de JS des mails, jamais.
- `cargo audit` + `cargo deny` en CI.

---

## 4. Plan de développement par phases (flux tiré, gates qualité)

Chaque phase livre un produit **utilisable et testé**, pas un empilement de couches.
Règle jidoka : tout défaut de perte de données ou de sécurité arrête la ligne.

### Phase 1 — Squelette marchant : « je lis mes mails » (4-5 semaines)
- Un compte Gmail via OAuth PKCE ; synchro des enveloppes INBOX vers SQLite.
- Shell Tauri : liste virtualisée + lecture d'un message (HTML assaini).
- CI complète dès le jour 1 : fmt, clippy `-D warnings`, tests, couverture ≥ 80 %,
  `cargo audit`, benchmarks de budgets de perf automatisés.
- **Gate 1 :** démarrage < 1 s, RAM < 200 Mo, liste 60 fps sur 50 000 messages réels.

### Phase 2 — Triage et écriture : « je travaille dans mes mails » (4-5 semaines)
- Actions : lu/non-lu, archiver, supprimer, déplacer, marquer — optimistes en UI,
  file d'actions offline rejouable avec réconciliation.
- Composer, répondre, transférer ; envoi SMTP avec file « boîte d'envoi » (jamais
  d'envoi perdu) ; brouillons synchronisés.
- Raccourcis clavier complets (l'arme de Superhuman).
- **Gate 2 :** zéro perte d'action prouvée par tests de coupure réseau/crash ; E2E des
  parcours critiques (lire, trier, répondre) verts.

### Phase 3 — Recherche, multi-comptes, échelle (4 semaines)
- Recherche plein-texte FTS5 (< 100 ms sur 100k messages), filtres from/to/date/pièce jointe.
- Multi-comptes (Gmail + Microsoft + IMAP générique), boîte unifiée.
- Pièces jointes, notifications Windows, threading des conversations.
- **Gate 3 :** budgets tenus avec 3 comptes / 200 000 messages cumulés.

### Phase 4 — Web (5-6 semaines)
- `sync-server` : le même `mail-core` exposé en HTTP/WebSocket, multi-tenant,
  chiffrement au repos, sessions.
- La même UI TypeScript déployée en SPA ; parité fonctionnelle lecture/triage/écriture.
- **Gate 4 :** revue de sécurité complète du serveur (c'est lui qui détient les tokens
  des utilisateurs — surface critique) ; pentest avant toute ouverture.

### Phase 5 — Durcissement et bêta (3-4 semaines)
- Installeur MSIX + mise à jour automatique signée ; télémétrie de crash opt-in.
- Bêta fermée 20-50 utilisateurs ; le CE dépouille chaque retour (genchi genbutsu).
- Kaizen : une itération hebdomadaire sur les frictions observées, pas imaginées.
- **Gate 5 :** 2 semaines sans défaut critique → lancement.

**Jalon de démarrage anticipé (dès Phase 0) :** dossier de vérification Google
(scopes restreints Gmail) et enregistrement app Azure AD — les délais d'audit
(plusieurs mois pour Google/CASA) sont sur le chemin critique du lancement public.

---

## 5. Qualité intégrée (jidoka) — règles permanentes

1. TDD systématique : le moteur de synchro se développe contre un **serveur IMAP simulé**
   (fixtures des bizarreries réelles : Gmail, Outlook, Dovecot, OVH…).
2. Couverture ≥ 80 % sur `mail-core` ; les E2E (Playwright sur l'UI) couvrent les
   parcours critiques ; tests de propriété (`proptest`) sur le parsing et la réconciliation.
3. Budgets de perf en CI : un benchmark qui régresse au-delà du budget = build rouge.
4. Zéro `unwrap()` en production, erreurs typées (`thiserror`) dans les crates,
   contexte (`anyhow`) dans les apps.
5. Revue de code sur tout, revue sécurité sur : auth, parsing de contenu externe,
   stockage, rendu HTML.

---

## 6. Organisation et cadence (obeya)

- **Le Chef Ingénieur** possède le concept paper, arbitre chaque compromis contre le
  client cible, et a le dernier mot sur le périmètre. Réflexe par défaut : dire non.
- Équipe cible : 1 CE, 2 dev Rust (core/protocoles), 1-2 dev TypeScript (UI),
  soutien ponctuel design + sécurité. (Solo ? Le plan tient, les phases s'allongent ~×2.)
- **Obeya hebdomadaire** : budgets de perf affichés, avancement par phase, top 3 risques,
  décisions à prendre. Tout écart au budget se traite la semaine même.
- Toute décision structurante = un ADR court dans `docs/adr/`.

---

## 7. Risques majeurs et contre-mesures

| Risque | Impact | Contre-mesure |
|---|---|---|
| Audit Google scopes restreints (CASA) : long, coûteux | Bloque le lancement public Gmail | Démarrer le dossier en Phase 0 ; bêta limitée à 100 utilisateurs en attendant |
| Bizarreries des serveurs IMAP réels | Bugs de synchro sans fin | Suite de fixtures par fournisseur ; matrice de serveurs testés ; QRESYNC optionnel |
| Rendu HTML : sécurité vs fidélité | XSS ou newsletters cassées | Spike Phase 0 ; corpus de 500 vrais emails comme jeu de test de rendu |
| Le web double la surface (infra, sécu, coût) | Retard, risque sécurité | Web en Phase 4 seulement, après un desktop solide ; revue sécu dédiée |
| Dérive de périmètre | Produit lent et tardif | Le concept paper liste les NON explicites ; le CE arbitre |
| WebView2 absent/cassé sur certaines machines | Crash au démarrage | Runtime evergreen + détection/installation au premier lancement |

---

## 8. Mesure du succès

- **Produit :** budgets de perf tenus en continu (§1) ; crash-free sessions > 99,5 %.
- **Usage bêta :** ≥ 60 % des testeurs l'utilisent encore comme client principal après
  30 jours ; temps de triage matinal réduit (mesuré, pas déclaré).
- **Ingénierie :** lead time d'une correction < 48 h ; zéro défaut critique ouvert > 7 jours.

---

## 9. Prochaines actions immédiates

1. Restructurer le dépôt en workspace Cargo (`crates/mail-core`, `apps/desktop`) —
   l'actuel `src/main.rs` (mot de passe en dur) est remplacé par le spike OAuth.
2. Lancer les 4 spikes de Phase 0 (§2.3) et la grille set-based (§2.4).
3. Créer le projet Google Cloud + app Azure AD ; ouvrir le dossier de vérification Google.
4. Planifier les entretiens utilisateurs (genchi genbutsu).
5. Mettre en place la CI (fmt, clippy, tests, couverture, audit, bench).
