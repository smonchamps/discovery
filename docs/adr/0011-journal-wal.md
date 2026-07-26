# ADR 0011 — Journal SQLite en WAL

Date : 2026-07-26 · Statut : accepté
· Conséquence directe de [ADR 0010](0010-synchronisation-integrale.md)

## Contexte

Premier essai terrain de la synchronisation intégrale : une synchro de
77 s s'est conclue par **« database is locked »** sur la passe
d'en-têtes du compte Microsoft.

Le mécanisme est structurel, pas accidentel. En mode rollback, un
lecteur bloque l'écrivain le temps de sa lecture. Chaque commande ouvre
sa propre connexion, et l'ADR 0010 a ajouté un lecteur **périodique** —
le sondage d'avancement, toutes les 800 ms — précisément pendant que les
écritures s'allongent de quelques secondes à plusieurs minutes. Le
`busy_timeout` de 5 s de l'écrivain finit par expirer : l'erreur vient
du produit lui-même, pas d'un usage exotique.

Le risque avait été nommé lors de la revue du bandeau d'avancement ; le
terrain l'a transformé en défaut le jour même de la première synchro
intégrale. Décision arbitrée par le Chef Ingénieur sur ce constat.

## Décision

`PRAGMA journal_mode = WAL`, posé à l'ouverture de la base
(`Store::init`).

- **Les lecteurs ne bloquent plus jamais l'écrivain, ni l'inverse** —
  c'est la propriété qui manquait, et toute la raison du choix.
- **Persistant** : écrit dans l'en-tête du fichier, relu à chaque
  ouverture. Les bases héritées en rollback sont converties à leur
  première ouverture — prouvé par un test sur base fichier
  (`une_base_fichier_s_ouvre_en_wal`), pas en mémoire : une base mémoire
  répond « memory » à ce PRAGMA et aurait validé un modèle faux.
- **Le `busy_timeout` reste** : deux écrivains se sérialisent toujours,
  lui seul les fait patienter.
- Les tests en mémoire ne changent pas : le PRAGMA y répond « memory »,
  sans erreur.

## Conséquences

**Positives** — plus aucun verrou lecture/écriture ; la jauge, la liste
et la recherche restent servies pendant une synchronisation longue.

**Négatives, assumées**

- Deux fichiers compagnons (`-wal`, `-shm`) à côté de `discovery.db`.
  Sans effet sur les sauvegardes à froid ; une copie à chaud doit
  prendre les trois (ce qui était déjà vrai du journal rollback).
- Le `-wal` grandit pendant une longue rafale d'écritures et se résorbe
  aux points de contrôle automatiques. Aucun réglage manuel tant que la
  mesure n'en montre pas le besoin.

## Alternatives écartées

| Option | Pourquoi non |
|---|---|
| Allonger le `busy_timeout` | Déplace l'expiration, ne supprime pas le blocage — et fige l'interface d'autant. |
| Espacer le sondage de la jauge | Traite UN lecteur ; la liste et la recherche bloquent pareil. Et un avancement qui se rafraîchit à la minute ne rassure personne. |
| Une connexion partagée unique sérialisée | Reconstruire un ordonnanceur devant SQLite, qui en possède déjà un — complexité maximale pour le même résultat. |
