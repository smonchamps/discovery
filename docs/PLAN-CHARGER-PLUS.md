# PLAN-CHARGER-PLUS — pagination « charger plus » des résultats

**CHANTIER SOLDÉ le 2026-08-17 — terrain complet.** Commit `99c9707`
(A51 ; CI 32025943490). GO CE du plan le 2026-08-17 (D1 borne douce ~1000
lignes ; D2 mécanisme tranché à la mesure → **pagination en deux temps**
après que l'OFFSET nu s'est révélé O(offset) au terrain). Terrain validé le
jour même : banc « OFFSET en profondeur » **plat** (p1 ≈ p10, < 100 ms), geste
« charger plus » sur la vraie base. Revue à regard neuf : un bug d'anti-course
corrigé (drapeau `chargementPlus` qui fuyait à `true`).

## Constat (vérifié)

La recherche rend au plus `SEARCH_LIMIT = 100` lignes, non fenêtrées
(`Liste.svelte`, `{#each resultats}`), et la barre dit « N sur M » (M =
total exact). `search_capped(input, limit) -> (rows, total)` porte déjà le
total ; il ne manque que **la suite** : au-delà des 100 premiers, rien.

Demande CE (2026-08-17) : un bouton **sous la liste** qui **ajoute** le lot
suivant (100, ou le reste quand il en reste moins), façon « charger plus » —
la liste grandit, la barre passe « 200 sur M », « 300 sur M »… jusqu'à tout
afficher.

## Périmètre

**Dans** : bouton « Afficher les N suivants » (N = min(100, reste)) quand
`résultats < total` ; clic → append du lot suivant ; disparaît quand tout
est affiché.

**Hors, assumé** :

- **Virtualisation de la liste** — la demande est « charger plus » (append),
  pas un défilement virtualisé infini. Refusé (chantier bien plus lourd,
  non demandé).
- **Navigation page par page** (précédent/suivant) — c'est de l'append, pas
  de la pagination classique. Refusé (CE : « la liste grandit »).
- **Curseur (keyset)** sauf si l'OFFSET échoue le budget (voir Options).

## Options — le mécanisme de la page suivante (point dur, mesuré)

Deux façons de servir la tranche `[offset, offset+100)` :

- **OFFSET** (`LIMIT 100 OFFSET k*100`) : **uniforme** (marche pour le tri
  BM25 comme pour le tri date). Rejoue la requête et saute `k*100` lignes.
  Raisonnement : le coût dominant est l'**énumération/classement des
  correspondances** (déjà payé à la page 1 — ~83 ms sur « fac » en tri
  date), le saut de quelques centaines de lignes est marginal devant. Donc
  une page k peu profonde coûte ≈ la page 1. Ne se dégrade qu'aux offsets
  profonds (milliers) — mais le DOM meurt avant.
- **Curseur** (`WHERE (date, uid) < (dernier) LIMIT 100`) : O(page), stable,
  mais **ne marche que pour le tri date** (le rang BM25 ne fait pas une clé
  de curseur). Deux mécanismes à maintenir.

**Verdict terrain (2026-08-17) : ni OFFSET nu, ni curseur — DEUX TEMPS.**
La mesure a démenti le pari « OFFSET ≈ plat » : `LIMIT ? OFFSET ?` sur la
requête HYDRATÉE dégrade en O(offset) (« fac » : p1 72 ms → p10 **259 ms**),
car SQLite hydrate les lignes sautées (`SELECT_UNIFIED` par ligne) avant de
les jeter. Le curseur seul ne sauverait que le tri date, or les requêtes
BM25 étroites (« factu » : p10 165 ms) dégradent aussi. Remède retenu
(D2) : **pagination en deux temps** — phase 1 les CLÉS ordonnées
(`(mailbox_id, uid)`, l'OFFSET ne saute que des clés légères), phase 2
hydrater UNIQUEMENT les 100 de la page, réordonnées comme la phase 1. Coût
**plat** à toute profondeur, uniforme BM25/date, un seul mécanisme.

## Étapes

- **CP-E1 — pagination cœur.** `run_search(input, limit, offset, force_date)`
  ajoute `OFFSET ?` ; `search_capped(input, limit, offset)` le transmet
  (même `total` → même décision de tri d'une page à l'autre). Gate : tests
  (page 2 = la bonne tranche ; tri stable entre pages ; offset au-delà du
  total → vide).

- **CP-E2 — commande.** `search_messages(query, offset)` (offset défaut 0)
  rend `{rows, total}`. Le total informe l'UI du reste.

- **CP-E3 — bouton « charger plus ».** Sous `{#each resultats}`, quand
  `resultats.length < total` : bouton « Afficher les N suivants »
  (N = min(100, total − affichés)). Clic → `search_messages(q, affichés)`,
  **append** à `resultats`, maj « N sur M ». Réinitialisé à chaque nouvelle
  frappe (nouvelle recherche = page 1). Système amendé (DC-D2). Gate : e2e.

- **Gate + terrain.** `/code-review high`, `/gate` ; banc étendu mesure
  l'OFFSET aux pages 1/5/10 (budget < 100 ms) ; geste dans l'app.

## Décisions CE (STOP 1 — 2026-08-17, AskUserQuestion, mot pour mot)

- **D1 — profondeur / DOM** : « **Borne douce ~1000 lignes (Recommandé)** ».
  → le bouton « charger plus » disparaît au-delà de ~1000 lignes affichées,
  remplacé par une invite « affinez votre recherche (from:/to:/date:) ».
- **D2 — mécanisme** : « **Tu tranches à la mesure (Recommandé)** ».
  → mesuré au terrain : OFFSET nu dégrade (p10 = 259 ms) → **pagination en
  deux temps** (clés puis hydratation) validée en re-décision CE le
  2026-08-17. Coût plat, uniforme BM25/date.

**Constante de borne** : `MAX_RESULTATS` côté UI (lignes affichées au-delà
desquelles le bouton s'efface). 1000 = 10 lots de 100 (`LOT`).

## Solde (STOP 2 — terrain validé le 2026-08-17)

Bouton « Afficher les N suivants » sous la liste, **append** ; borne douce à
1000 lignes puis invite « Affinez votre recherche » ; anti-course
(`chargementPlus` + jeton). Cœur : `search_capped(input, limit, offset)`.

**Point dur mesuré, tranché au terrain — la pagination en deux temps.**
Le pari « OFFSET ≈ plat » était FAUX : `LIMIT ? OFFSET ?` sur la requête
hydratée dégrade en O(offset) (« fac » p10 = **259 ms**), car SQLite hydrate
les lignes sautées (`SELECT_UNIFIED`) avant de les jeter. Le curseur seul ne
sauvait que le tri date (les requêtes BM25 étroites dégradaient aussi).
Remède (re-décision CE) : **deux temps** — `page_keys` rend les
`(mailbox_id, uid)` ordonnés (l'OFFSET ne saute que des clés),
`hydrate_in_order` n'hydrate que les 100 de la page et les réordonne (IN
`(VALUES …)`, `mailbox_id` relu par nom). Ordre TOTAL (`… , e.uid DESC`) pour
des tranches sans trou ni doublon. Re-mesuré **plat** : p1 ≈ p10, tous
< 100 ms (« fac » 50→70 ms ; « réunion » 29→30 ms).

**Revue (regard neuf)** : un vrai bug d'anti-course attrapé — le drapeau
`chargementPlus` fuyait à `true` si une frappe survenait pendant un
chargement (reset gardé par le jeton), condamnant le bouton ; corrigé en
reset inconditionnel + garde synchrone.

Tests neufs : `search_capped_pages_without_gap_or_overlap`. e2e existant vert
(le bouton conditionnel n'apparaît pas sous 100 résultats). Système A51,
DC-D2. Banc `banc_recherche` : section « pagination OFFSET en profondeur »
(pages 1/5/10).
