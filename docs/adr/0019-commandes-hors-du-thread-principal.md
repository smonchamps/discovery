# ADR 0019 — Les commandes bloquantes vivent hors du thread principal, une à la fois

**Date** : 2026-08-15 · **Statut** : accepté, livré (`e32280b`,
PLAN-GELS), tenu par une gate.

## Contexte

Terrain du 2026-08-15 : freeze de plusieurs secondes au démarrage — la
fenêtre ne répond ni au clic ni au déplacement. Mesure (sonde
`SendMessageTimeout`, base réelle 251 062 enveloppes) : **25,2 s de
gels cumulés sur 40 s**, pire gel 4,6 s. Racine : dans Tauri 2, une
commande déclarée sans `async` s'exécute sur le **thread principal** —
la pompe de messages Windows. Trente-quatre commandes ouvraient la base
depuis là ; tout tenait tant qu'elles restaient sous ~100 ms, puis un
lot de rattrapage de 130 Mo a dépassé.

Deux faits d'architecture contraignent le remède :

- le runtime async de Tauri n'a **pas de pool bloquant** : `async` seul
  déplace le blocage sur un worker tokio (workers = cœurs) — sur deux
  cœurs, deux commandes lentes affament toute la file IPC ;
- le thread principal offrait gratuitement la **sérialisation** des
  commandes : une à la fois, dans l'ordre. La perdre ouvre des courses
  réelles (paires état-local/file-d'actions de `mark_flagged`, TOCTOU
  `save_draft`/`delete_draft`, `SQLITE_BUSY_SNAPSHOT` que le
  `busy_timeout` ne couvre pas).

## Décision

1. **Toute commande qui ouvre la base, touche un fichier ou le coffre
   est `async fn` et son corps passe par `hors_pompe()`** :
   `spawn_blocking` (la pompe ne fait que pomper) **+ verrou global des
   commandes** (`AppState.commandes` — la sérialisation d'avant,
   conservée). Les deux moitiés sont indissociables.
2. **Les exemptions sont nommées et justifiées une à une** (pures
   d'état : atomiques, détaché, panic d'auto-test ADR 0014) dans la
   gate `e2e/garde-thread-principal.mjs` — jouée au pre-push, en CI et
   dans `/gate`. Comptage croisé attributs/prises : zéro prise = rouge.
3. **Le symptôme a son budget et son instrument** : aucun gel de pompe
   > 150 ms (PASSATION §3), mesuré par `python e2e/sonde-gel.py
   <base.db>` sur base hors dépôt.

## Conséquences

- Preuve à la livraison : zéro gel > 150 ms sur 40 s (décors 251 k
  enveloppes, avec et sans stock d'aperçus) et sur 60 s sur copie de la
  vraie base (4,75 Go) ; le travail de fond continue (15 000 aperçus
  recalculés pendant la mesure).
- Les commandes restent une à la fois : un lot d'écriture long fait
  attendre un geste — comme avant, mais fenêtre libre. D'où les lots
  courts (`preview_catchup` à 500, D2).
- Le coût CPU des sondes chères demeure (D-8) : hors pompe, il ne gèle
  plus — il se rouvrira sur constat, pas sur intuition.
- Un panic dans `hors_pompe` ne condamne pas les commandes suivantes
  (verrou empoisonné récupéré, même choix que `verrou_compte`).
