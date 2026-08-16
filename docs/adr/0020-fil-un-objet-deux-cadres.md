# ADR 0020 — Le fil de lecture : un objet, deux cadres, l'exclusivité au store

**Date** : 2026-08-16 · **Statut** : adopté (UI v3, A43, commit 16f06e6)

## Contexte

Le verdict CE de la séance d'annotation du 2026-08-16 met le fil en
cartes DANS le volet de lecture, et l'écran 03 reste le plein écran du
même contenu. La question D4 (« que devient l'écran 03 ? ») a été
tranchée par le CE : « une coexistence qui n'est qu'un changement de
taille des mêmes objets ».

## Décision

1. **Un composant** (`Fil.svelte`) porte tout le dessin du fil ; **un
   état module** (`lib/fil.svelte.js`, runes) porte messages, dépliage,
   corps, pièces, images — partagé entre les cadres.
2. **L'exclusivité des cadres est structurelle** : `fil.cadre`
   (`null | 'volet' | 'plein'`) est le SEUL interrupteur ; chaque cadre
   rend `{#if fil.cadre === le-sien}`. Aucun booléen local de
   visibilité — la première version en avait trois, réconciliés à la
   main, désynchronisés au premier chemin non couvert (archivage au
   raccourci depuis l'écran 03, bascule de disposition).
3. **Agrandir/réduire ne rechargent rien** (`agrandirFil`/`reduireFil`) ;
   **ouvrir recharge toujours** (`ouvrirFil` — la mémoïsation a caché
   la propre réponse de l'utilisateur) ; **fermer purge** (`fermerFil`,
   importable de partout, tous modes de disposition).

## Conséquences

- Le chrono P1 « ouverture » mesure sélection → fil affiché
  (`thread_messages` compris, pièces exclues) — série re-basée (D-12).
- Le changement de cadre remonte les iframes (D-13, assumé).
- Les specs e2e s'ancrent au cadre (`volet-lecture`/`conversation`) et
  assertent l'unicité de l'objet (`fil-sujet` → count 1).
