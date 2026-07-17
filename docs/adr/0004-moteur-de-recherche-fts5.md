# ADR 0004 — Recherche : SQLite FTS5 confirmé, Tantivy en plan B chiffré

Date : 2026-07-18 · Statut : accepté

## Contexte

La Phase 3 ouvre sur la recherche plein-texte : < 100 ms sur 100 000
messages ([PLAN.md](../PLAN.md) §1), budgets tenus à 200 000 messages au
gate 3. La grille set-based de Phase 0 (§2.4) posait SQLite + FTS5 en
hypothèse gelée, Tantivy en alternative. Le spike
[`spikes/search-engine`](../../spikes/search-engine/README.md) les a
départagés sur corpus déterministe de 100 000 puis 200 000 documents,
protocole identique, comptes de hits vérifiés croisés.

## Mesures décisives (p95, top-50 avec ranking, 200 000 docs)

| | FTS5 | Tantivy |
|---|---|---|
| Requête réaliste la plus large (16,7 % de hits) | **37,4 ms ✅** | 0,68 ms |
| Requêtes rare / ET / phrase | 0,3–4,7 ms ✅ | < 1 ms |
| Requête dégénérée (90 % du corpus matche) | 188 ms ❌ | 0,15 ms |
| Incrémental 500 docs (chemin récurrent de la synchro) | **25 ms** | 350 ms |
| `reunion` trouve `réunion` (français) | **oui, natif** | non (config à faire) |
| Second magasin à réconcilier | **non** | oui |

## Décision

**FTS5 est confirmé** pour la recherche v1. La règle du set-based était :
Tantivy devait battre l'hypothèse *nettement* — il est plus rapide sur
toutes les requêtes, mais il perd sur ce qui structure un client
offline-first :

1. **Transactionnalité** : l'index vit DANS la base — suppression du
   message et de son entrée d'index dans la même transaction. Tantivy est
   un second magasin : tombstones, gardes, réconciliation après crash —
   la Phase 2 vient de payer ce prix pour les brouillons, on ne le paie
   pas une seconde fois pour un index reconstructible.
2. **Le chemin récurrent** : chaque synchro insère des messages. FTS5
   absorbe 500 documents en 25-36 ms ; chaque commit Tantivy en coûte
   ~350 — et il faudrait une politique de commits différés.
3. **Le français est natif** (`unicode61 remove_diacritics 2`) ; le
   tokenizer Tantivy par défaut retourne **zéro** résultat sur
   « reunion » → « réunion ».
4. **Zéro dépendance nouvelle** : FTS5 est déjà dans le SQLite bundled.

Sur le critère du plan, FTS5 tient le gate 3 avec une marge ×2,7 sur la
requête réaliste la plus défavorable (37 ms pour 100 ms de budget).

## Vigilances et garde-fous (mesurés, pas supposés)

- **Le coût FTS5 suit le nombre de matchs** (`ORDER BY rank` = BM25 sur
  tous) : une requête matchant 69-90 % du corpus dépasse le budget à
  200 000 messages. Ce cas est un artefact du vocabulaire synthétique du
  spike (~150 mots), mais le mécanisme est réel. Garde-fous produit :
  search-as-you-type déclenché à partir de 3 caractères + debounce ;
  option `prefix=` de FTS5 à évaluer à l'implémentation.
- **Table *external content*** (`content=`) en production : le spike
  stockait le contenu dans la table FTS (595 Mo) ; l'index seul doit
  être bien plus petit — à mesurer à l'implémentation.
- **Plan B documenté et chiffré** : si le mur des requêtes larges se
  matérialise sur corpus réel (mesure en bêta, Phase 5), Tantivy est
  sub-milliseconde partout (élagage block-max WAND), index 8× plus
  petit — la bascule serait une décision informée, pas une réécriture
  en panique. Ses coûts sont connus : second magasin, commits lents,
  folding des diacritiques à configurer.

## Conséquences

- La recherche production s'implémente dans `mail-core` sur FTS5, après
  la fondation multi-comptes (l'index portera `account_id` dès le début).
- Le spike reste dans `spikes/search-engine` (hors workspace — Tantivy
  ne doit pas entrer dans le lock de production), relançable pour
  re-mesurer sur d'autres volumes.
