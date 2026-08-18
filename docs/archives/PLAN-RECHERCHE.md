# PLAN-RECHERCHE — pertinence et performance de la barre de recherche

**CHANTIER SOLDÉ le 2026-08-17 — terrain complet.** Deux commits :
`6f44566` (destinataires `to:`/`à:`, `prefix='2 3'`, reconstruction d'index
visible et interruptible, A49 ; CI 32011441351) et `d82e0dc` (plafond 100,
« N sur M » avec total exact, soupape tri-date armée au-delà de 10 k corr.,
A50 ; CI 32020891088). GO CE des décisions D1-D4 le 2026-08-17 (+ arbitrages
plafond et budget). Terrain validé sur la vraie base (251 k messages / 7 Go)
le jour même : geste `to:`/nom nu, modale de reconstruction visible, budget
recherche **~66 ms** au pire (contre 113-210 ms annoncés). Revue à regard
neuf à chaque étape ; le cache chaud avait menti sur le coût de
reconstruction (×340), rattrapé au terrain.

## Constat (vérifié, pas supposé)

La recherche est bâtie sur **SQLite FTS5, index « sans contenu »,
transactionnel** — décision gelée de l'[ADR 0004](../adr/0004-moteur-de-recherche-fts5.md)
après un set-based mesuré contre Tantivy. Lecture faite du cœur
([`crates/mail-core/src/search.rs`](../../crates/mail-core/src/search.rs)),
du câblage UI ([`Liste.svelte:213`](../../apps/desktop/ui-v2/src/Liste.svelte),
[`App.svelte:1069`](../../apps/desktop/ui-v2/src/App.svelte)), de la commande
([`commands.rs:1728`](../../apps/desktop/src/commands.rs)), du spike
([`spikes/search-engine`](../../spikes/search-engine/README.md)) et des
observations ouvertes (PASSATION §1.2, DETTE D-16).

État des lieux :

- **Champs indexés** : `subject`, `sender` (nom + adresse), `body`. Les
  **destinataires `to`/`cc` ne sont PAS indexés** — pourtant stockés
  (`envelopes.to_addrs/cc_addrs`, rattrapés par `backfill_recipients`,
  DETTE D-16).
- **Requête** : dernier terme en préfixe (`"budg"*`), filtres `from:`/`de:`,
  `date:AAAA[-MM[-JJ]]`. Saisie jamais interprétée comme syntaxe FTS5.
- **Pertinence** : `bm25(search_fts, 10.0, 5.0, 1.0)` (sujet ×10, exp. ×5,
  corps ×1), puis date. Accents repliés natifs (`unicode61 remove_diacritics 2`).
- **UI** : seuil 3 car., débounce 150 ms, jeton anti-course, top-50, hors
  thread principal.

## Comparaison open-source (registre embarqué en-processus)

| Solution | Gain propre | Coût ici | Verdict |
|---|---|---|---|
| **FTS5** (actuel) | BM25, préfixe, accents FR natifs, zéro dép., transactionnel | BM25 sur *tous* les matchs | En place |
| **FTS5 + `prefix=`/`trigram`** | Préfixe court accéléré ; sous-chaîne + flou | Index plus gros | Levier gratuit, même moteur |
| **Tantivy** | Élagage top-k (WAND) sous-ms ; index 8× plus petit | Second magasin, commits lents | Plan B (ADR 0004) — ne déloge pas |
| **Xapian** (notmuch) | Stemming FR, la réf. mail local | FFI C++ + second magasin | Non — coût du plan B sans son gain |
| **Meilisearch / Typesense** | Tolérance aux fautes | Serveur/démon à superviser | Non — casse l'offline-first |
| **Fuse/MiniSearch/FlexSearch** | Simplicité JS | Index entier en webview | Non — 250 k+ msgs intenable |

**Verdict** : les moteurs qui battent FTS5 (vitesse brute, tolérance aux
fautes) exigent tous un second magasin ou un serveur — ce que l'ADR 0004
a écarté sur mesure. On n'échange pas le moteur : on exploite les leviers
inutilisés de FTS5 et on ferme les trous de pertinence.

## Périmètre

**Dans** : indexer les destinataires + filtre `to:` (A1) ; `prefix='2 3'`
sur l'index (B). Les deux partagent **une seule migration** de schéma FTS
(le schéma FTS5 ne supporte pas `ALTER ADD COLUMN` — drop + rebuild).

**Hors, assumé** :

- **Dédoublonnage multi-boîtes** (A2) — le CE tranche « observer en bêta
  d'abord » : mesurer la gêne réelle avant de risquer de masquer une copie
  Spam/Corbeille. Reste report ouvert (PASSATION §149).
- **Tri par date des requêtes larges** — soupape documentée, décision
  finale en bêta sur vraies boîtes (levier ×1,8–2,9 re-validé).
- **Stemming FR / tolérance aux fautes** (C1) — set-based post-v1, spike
  jetable commandé (trigram vs Snowball-FR), hors de ce plan.
- **Changement de moteur** (Tantivy/Xapian/Meili) — refusé, cf. ADR 0004.

## Options et verdicts

**Performance — le mur a deux moitiés distinctes** (nommées ici, les docs
les confondaient) :

1. *Expansion du préfixe* (trouver les termes de `"fac"*`) : coût ∝ nombre
   de **termes** distincts. → **`prefix='2 3'`** précalcule les entrées
   2/3 lettres, aligné sur le seuil de 3 car. **Préserve BM25.**
2. *Score BM25 sur tous les matchs* : coût ∝ nombre de **documents**. →
   **tri par date** (×1,8–2,9) le supprime, mais **sacrifie le classement**.

Non concurrents mais complémentaires. Verdict : **`prefix='2 3'` d'abord**
(gain sans perte de pertinence), tri-par-date gardé en soupape, mesure
finale en bêta. À l'échelle réelle le budget est ~tenu (~37 ms) ; les ❌ de
l'ADR venaient d'un vocabulaire synthétique.

## Étapes

- **E1 — Destinataires indexés + filtre `to:`/`à:`.**
  Colonne `recipients` dans `search_fts` (nom + adresse `to`/`cc`,
  déjà stockés joints). Filtre `to:`/`à:` → `recipients:"…"*`, sur le
  modèle de `from:`. BM25 → 4 colonnes (`10, 5, 3, 1` — dest. entre exp.
  et corps, à ajuster). Gate : tests RED→GREEN (`to:` trouve un envoi par
  destinataire nom/adresse ; accents repliés ; `to:` seul liste par date ;
  saisie hostile reste littérale).

- **E2 — `prefix='2 3'` + migration de schéma unique.**
  `CREATE VIRTUAL TABLE search_fts USING fts5(subject, sender, recipients,
  body, prefix='2 3', content='', contentless_delete=1, tokenize=…)`.
  `migrate_search` détecte l'ancien schéma (absence de `recipients` dans
  `sqlite_master.sql`), drop `search_fts`+`search_docs`, recrée, rebuild.
  Gate : test de migration (base ancien schéma → cherchable après, index
  reconstruit, comptes justes) ; `banc_recherche` re-mesuré au terrain
  (STOP 2, sur la vraie base du CE).

## Décisions CE (STOP 1 — 2026-08-17, AskUserQuestion, mot pour mot)

- **D1 — Indexer les destinataires + filtre `to:`** : « **Oui, dans le
  plan v1 (Recommandé)** ». → E1.
- **D2 — Dédoublonner les copies multi-boîtes** : « **Observer en bêta
  d'abord (Recommandé)** ». → hors périmètre, reste report ouvert.
- **D3 — Performance** : « **`prefix='2 3'` proactif, mesurer le reste en
  bêta (Recommandé)** ». → E2 ; tri-par-date en soupape.
- **D4 — Stemming FR / tolérance aux fautes** : « **Spike jetable
  mesuré** ». → spike `search-fuzzy` (baseline vs trigram vs Snowball-FR),
  mesuré le 2026-08-17 (100 k docs, protocole `search-engine`).
  **Verdict : ni stemming ni trigram ne paient leur coût en v1.**
  - *baseline (actuel)* : P1,00 partout, préfixe SAYT natif, index/synchro
    les plus légers — **gardé pour la v1**, coût 0.
  - *trigram* : seul à tolérer les fautes (R1,00) mais précision 0,05–0,20,
    **+6,5× disque, +18–28× synchro** — jamais en index primaire ; au mieux
    un « vouliez-vous dire » secondaire.
  - *stemming Snowball* : gain **partiel** (pluriel + conjugaisons d'une
    même catégorie seulement — Snowball stemme, ne **lemmatise pas** : ne
    relie pas le nom `réunion` au verbe `réunir`), entame la précision sur
    collisions (« note »→P0,50), **sacrifie le préfixe SAYT**, ×4 CPU
    d'insertion. **Post-v1, conditionnel** à une douleur prouvée en bêta.
  - Piste retenue si le trou de rappel se confirme : expansion
    pluriel/synonymes **à la requête**, plus ciblée qu'un changement de
    tokenizer. Spike en worktree isolé (non commité — décision solde).

## Solde (STOP 2 — terrain validé le 2026-08-17)

**E1 + E2 livrés.** `to:`/`à:` et la recherche par nom nu de destinataire
répondent au terrain ; `prefix='2 3'` en place. `banc_recherche` sur la
vraie base (251 256 messages, 7 Go) : recherche **< 100 ms partout**, pire
cas « fac » (3 car.) **82 ms** — budget tenu (contre 113–210 ms annoncés
avant). Ouverture < 3 ms.

**Point dur remonté par la revue, tranché au terrain — E3.** La colonne
`recipients` force une reconstruction unique de l'index (FTS5 ne sait pas
ajouter de colonne). Ma mesure préalable (0,7 s) était **faussée par un
cache chaud** ; au terrain, à froid sur 7 Go, la reconstruction a **gelé le
démarrage ~4 min en silence** (liste vide, « démarrage… », hors de tout
écran). Correctif **le jour même** (CE : « visible et interruptible ») :

- `pending_adoption` détecte l'index à reconstruire **même sur une base aux
  fils déjà adoptés** → la modale de migration (ADR 0012) s'affiche au lieu
  de geler ;
- reconstruction **interruptible et reprenable** (progression relevée,
  `Break` → `Error::Interrupted` → rembobinage) ;
- reconstruction **en flux** (fin du `Vec` de tous les corps en mémoire) ;
- libellé de la modale rendu **générique** (« Mise à jour de votre boîte »),
  vrai de l'adoption comme de la reconstruction — Système amendé (**A49**),
  DC-D2.

Tests neufs : détection (`pending_adoption_sees_an_old_search_index`),
progression + flux (`migration_reports_progress_and_indexes_every_message`),
annulation + rembobinage (`migration_cancel_rolls_back_and_reruns`).
Gate complète verte, e2e 86/86.

## Plafond de la recherche — soldé (2026-08-17, décisions CE)

Micro-chantier ouvert après le solde principal, sur constat du CE (« 50
résultats » se lit comme « exactement 50 »).

- **Total exact + « N sur M »** : la barre affiche « Recherche · N sur M
  résultats » quand le rendu est plafonné (M = vrai total, via un `COUNT`
  sur l'index calculé **seulement** si plafonné — 3,5 ms au terrain,
  négligeable). Sinon « N résultats ». `search_total` partagé avec `search`
  par `build_match`/`date_clauses`.
- **Plafond `SEARCH_LIMIT` = 100** (était 50). Deux tours de terrain : 200
  dépassait le budget (124 ms sur « fac »), et **100 aussi** (104 ms) — parce
  que le mur n'est ni le COUNT (3,5 ms) ni l'hydratation, mais le **plancher
  BM25** (~80 ms sur les 36 k correspondances de « fac », et il monte avec le
  corpus). Le plafond ne gratte que l'hydratation (~0,2 ms/ligne) ; il ne
  pouvait pas régler ça seul.
- **Soupape tri-date ARMÉE** (D-plafond, révision terrain) : au-delà de
  `WIDE_QUERY_THRESHOLD` (10 000 correspondances), `search_capped` classe par
  **date** au lieu de BM25. Le budget tient (**~66 ms sur « fac »**), et pour
  un préfixe de 3 car. la date est un meilleur ordre que le « classement de
  pertinence » d'un moignon. En deçà du seuil, BM25 inchangé. Le COUNT du
  total (déjà nécessaire à « N sur M ») informe la bascule sans requête de
  plus. `search`/`search_recent`/`search_total` publics ; `search_capped` est
  le point d'entrée UI ; corps partagé `run_search(force_date)`.
- Système A50, DC-D2. Banc `banc_recherche` aligné sur le plafond de prod
  (100) + le COUNT.

### Report ouvert (au CE)

- **Recherche sans limite pratique** = liste de résultats **virtualisée +
  pagination par curseur** (le mur : hydratation `SELECT_UNIFIED` par ligne
  + liste non fenêtrée). Chantier à part entière, non ouvert.
