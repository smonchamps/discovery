# Spike Phase 3 — moteur de recherche : FTS5 contre Tantivy

Départager l'hypothèse gelée du plan (SQLite + FTS5, [PLAN.md](../../docs/PLAN.md)
§2.4) et son alternative (Tantivy) par des chiffres, sur le critère du plan :
**recherche < 100 ms sur 100 000 messages** (§1), projetée à l'échelle du
gate 3 (**200 000 messages**, 3 comptes cumulés).

```powershell
cargo run --release -- [nombre_de_docs]   # défaut : 100 000
```

Corpus déterministe (LCG, distribution zipf-ienne, vocabulaire français
accentué) avec **termes marqueurs à fréquence exacte** — les deux moteurs
retournent les mêmes comptes de hits, ce qui valide le banc. Protocole
identique : 3 échauffements, 50 itérations, top-50 avec ranking BM25,
médiane et p95.

## Mesures (2026-07-18, machine de dev, build release)

### 100 000 documents

| Mesure | FTS5 | Tantivy 0.22 |
|---|---|---|
| Construction | 46,9 s (à froid¹) | 1,8 s |
| Incrémental 500 docs (scénario synchro) | **36,3 ms** | 345,6 ms |
| Disque | 300,3 Mo² | 35,0 Mo |
| Rare (121 hits) — p95 | 0,19 ms | 0,14 ms |
| Commun 16,7 % (16 750) — p95 | 15,6 ms | 0,36 ms |
| ET (479) — p95 | 0,87 ms | 0,36 ms |
| Phrase (1 005) — p95 | 1,28 ms | 0,26 ms |
| Préfixe `budg*` (69 114) — p95 | 73,1 ms | 0,37 ms³ |
| Accents `reunion`→`réunion` (89 780) — p95 | 90,3 ms | **0 hit⁴** |

### 200 000 documents — l'échelle du gate 3

| Mesure | FTS5 | Tantivy 0.22 |
|---|---|---|
| Construction | 16,4 s (cache chaud¹) | 3,2 s |
| Incrémental 500 docs | **25,3 ms** | 350,8 ms |
| Disque | 595,5 Mo² | 70,9 Mo |
| Rare (241) — p95 | 0,33 ms | 0,12 ms |
| Commun 16,7 % (33 417) — p95 | **37,4 ms ✅** | 0,68 ms |
| ET (955) — p95 | 4,1 ms | 0,77 ms |
| Phrase (2 005) — p95 | 4,7 ms | 0,45 ms |
| Préfixe (137 891 = 69 % du corpus) — p95 | **155,1 ms ❌** | 0,75 ms |
| Accents (179 117 = 90 % du corpus) — p95 | **188,0 ms ❌** | 0 hit⁴ |

¹ La construction FTS5 est sensible à l'état du cache disque (47 s à froid,
16 s à chaud pour 2× plus de docs) — coût unique, en tâche de fond de la
synchro initiale, non bloquant pour la décision.
² La table FTS5 du spike STOCKE le contenu ; en production, une table
*external content* (`content=bodies`) n'écrit que l'index — la taille
réelle ajoutée serait très inférieure.
³ Préfixe Tantivy via `RegexQuery` (FST) — pas de préfixe natif dans le
QueryParser ; en production il faudrait des edge-ngrams à l'indexation.
⁴ Le tokenizer par défaut de Tantivy ne replie pas les diacritiques :
« reunion » ne trouve pas « réunion ». Corrigeable par configuration
(`AsciiFoldingFilter`), mais rien n'est gratuit.

## Enseignements

1. **Le coût FTS5 suit le nombre de documents qui matchent** : `ORDER BY
   rank` calcule BM25 sur *tous* les matchs. Tantivy élague le top-k
   (block-max WAND) : sub-milliseconde même quand 90 % du corpus matche.
2. **Les deux ❌ de FTS5 sont des requêtes matchant 69-90 % du corpus** —
   artefact d'un vocabulaire synthétique de ~150 mots où tout est
   ultra-fréquent. Sur une vraie boîte (vocabulaire de dizaines de
   milliers de mots), le cas réaliste est la ligne « commun 16,7 % » :
   37 ms à 200 000 messages, budget tenu avec marge ×2,7.
3. **L'incrémental est le chemin récurrent** (chaque synchro insère) :
   FTS5 y est 10-14× plus rapide — la transaction SQLite absorbe 500 docs
   en 25-36 ms là où chaque commit Tantivy coûte ~350 ms.
4. Verdict et conséquences : [ADR 0004](../../docs/adr/0004-moteur-de-recherche-fts5.md).
