# ADR 0007 — Rattrapage des corps : plafond de récence, corps stockés

Date : 2026-07-21 · Statut : accepté

## Contexte

La validation terrain du compte IMAP générique a révélé, incidemment, un
défaut de conception bien plus large que celui qu'elle cherchait.

L'utilisateur a cherché un mot présent dans le **corps** d'un message
jamais ouvert. Aucun résultat. Le diagnostic
([`diagnostic_index`](../../crates/mail-core/examples/diagnostic_index.rs))
a disculpé l'index — complet à 100 % sur les trois comptes — et désigné la
vraie cause :

| Compte | Enveloppes | Corps en cache |
|---|---|---|
| gmail #1 | 537 | 18 (3 %) |
| gmail #2 | 2193 | 1 (0,05 %) |
| zoho | 3 | 1 |

La synchro **« enveloppes d'abord »** ([PLAN.md](../PLAN.md) §3) ne
télécharge un corps qu'au clic. Le corpus réel est donc **quasi sans
corps** : la recherche « plein-texte » ne porte, en pratique, que sur les
**sujets et les expéditeurs**.

Deux conséquences inacceptables en l'état :

1. l'un des quatre verbes du produit (*lire, trier, **chercher**, écrire*,
   §1) est amputé sans que rien ne le dise ;
2. l'[ADR 0004](0004-moteur-de-recherche-fts5.md) a tranché FTS5 sur un
   corpus **avec corps** — une prémisse que la production n'atteindrait
   jamais.

## Mesures ([`spikes/body-backfill`](../../spikes/body-backfill/README.md))

200 corps rapatriés depuis un compte Gmail réel, via l'API publique du
noyau, sur une **copie** de la base :

| Mesure | Valeur |
|---|---|
| HTML transféré | 59,2 Ko par corps |
| Croissance de la base | **61,9 Ko par corps** |
| Durée | 192 ms par corps |

### Le fait structurant : l'index est presque gratuit

61,9 Ko de disque pour 59,2 Ko de HTML. L'écart — **~2,5 Ko par
message** — est tout ce que coûte l'index FTS5. La vigilance « taille »
de l'ADR 0004 est **levée par la mesure** : la table *contentless* tient
sa promesse, 25× plus légère que le contenu qu'elle indexe.

**Ce ne sont donc pas les index qui coûtent, ce sont les corps stockés.**

### Mise à l'échelle du gate 3 (200 000 messages cumulés)

| Politique | Disque |
|---|---|
| Stocker tous les corps | 12,4 Go |
| Indexer sans stocker | 500 Mo |
| **Plafond 12 mois, 3 comptes** | **~370 Mo** |

Sur la boîte réelle mesurée (2 730 messages), tout stocker coûterait
~170 Mo et ~9 min : parfaitement supportable. **C'est le gate 3 qui casse,
pas l'usage courant.**

## Décision

**Rattraper et STOCKER les corps des 12 derniers mois**, en tâche de fond,
après la synchro des enveloppes.

Ce n'est **pas** un renoncement à « enveloppes d'abord » : la liste reste
utilisable immédiatement, le rattrapage vient après et ne bloque rien. Les
deux décisions se complètent.

Pourquoi ce choix contre les alternatives :

- **contre « indexer sans stocker »** (25× moins de disque) : cela ferait
  disparaître la lecture hors-ligne des messages anciens, alors que le
  produit se promet *offline-first* (§1) — et exigerait de découpler
  l'indexation du cache, l'index se reconstruisant aujourd'hui depuis la
  table `bodies` à chaque `upsert_envelopes`. Beaucoup de complexité pour
  un problème que le plafond de récence résout déjà ;
- **contre le statu quo** : la mesure a rendu l'amputation indéfendable ;
- **bénéfice non recherché** : le rattrapage répare aussi la **lecture
  hors-ligne**, aujourd'hui quasi inexistante (18 corps sur 537).

### Budget disque — le plan n'en avait aucun

Posé ici, dérivé de la mesure : **base locale < 1 Go en usage courant
(3 comptes)**. À 62 Ko par corps, cela autorise ~16 000 corps stockés,
cohérent avec 12 mois sur trois comptes actifs.

L'horizon de 12 mois est un **réglage**, pas un dogme : le coût par
message étant désormais connu, tout N se convertit directement en
mégaoctets.

## Validation terrain (2026-07-21, trois comptes réels)

Le rattrapage complet a tourné sur la boîte réelle : reprise après arrêt,
interruption propre, et — le test qui a déclenché tout ce chantier — **le
mot cherché dans le corps d'un message jamais ouvert remonte enfin**.

| | Prédit par le banc | Mesuré en production |
|---|---|---|
| Base finale (~2 730 messages) | ~170 Mo | **97 Mo** |
| Coût par corps stocké | 61,9 Ko | **~34 Ko** |

**Le banc surestimait de 45 %, et l'écart s'explique.** Il échantillonnait
les 200 messages *les plus récents* — c'est-à-dire la couche la plus
chargée en infolettres HTML lourdes. Le corpus complet, lui, contient
aussi les échanges personnels, bien plus légers. Le chiffre du banc était
un **majorant**, comme celui de la durée ; les deux se sont confirmés
majorants.

Conséquences sur les décisions prises plus haut :

- le budget **< 1 Go** n'est pas serré, il est confortable : la boîte
  réelle en occupe **10 %** ;
- l'extrapolation du gate 3 (~370 Mo à 12 mois sur 3 comptes) est elle
  aussi un majorant — à ~34 Ko/corps elle retombe vers ~200 Mo ;
- le levier « indexer sans stocker » **s'éloigne** d'autant. Il reste
  documenté, il n'est plus à l'ordre du jour.

Reste ouvert : le **débit réel groupé** face au majorant de 192 ms/corps.
Non mesuré ici, la durée n'ayant pas été relevée. Sans conséquence sur une
décision en cours — à instrumenter le jour où le rattrapage gênera.

## Conséquences

- **Implémentation** : pompe de fond, reprenable après coupure, qui
  n'entre jamais en concurrence avec la synchro ni avec la vidange de la
  boîte d'envoi (mêmes verrous que `outbox_flush` / `drafts_push`).
- **Grouper les `FETCH`** : les 192 ms/corps mesurent le chemin actuel de
  `load_body`, soit **un aller-retour IMAP par message**. Un rattrapage
  réel doit grouper (50 par commande). Le chiffre mesuré est un
  **majorant**, et l'écart entre les deux est à re-mesurer une fois la
  pompe écrite.
- **Visibilité** : l'avancement doit être visible et interruptible — un
  téléchargement de fond invisible est une mauvaise surprise réseau.
- **Levier connu si le gate 3 se tend** : « indexer sans stocker » au-delà
  de l'horizon, chiffré ici à 500 Mo pour 200 000 messages. Décision
  informée, pas réécriture en panique.
- **Re-mesurer au gate 3** : budgets démarrage/RAM avec une base d'~1 Go —
  la lecture SQLite est insensible au volume (Phase 1), mais cela se
  vérifie plutôt que se suppose.
