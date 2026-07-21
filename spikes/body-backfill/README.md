# Banc Phase 3 — que coûte le rattrapage des corps de messages ?

## Le constat qui l'a provoqué

La validation terrain du 2026-07-21 a montré que le corpus réel est
**quasi sans corps** :

| Compte | Enveloppes | Corps en cache |
|---|---|---|
| gmail #1 | 537 | 18 (3 %) |
| gmail #2 | 2193 | 1 (0,05 %) |
| zoho | 3 | 1 |

C'est la conséquence directe de la synchro **« enveloppes d'abord »**
([PLAN.md](../../docs/PLAN.md) §3) : un corps n'est téléchargé qu'au clic.
Excellent pour le démarrage en 350 ms — mais la recherche plein-texte ne
porte donc, en pratique, **que sur les sujets et les expéditeurs**, alors
que l'[ADR 0004](../../docs/adr/0004-moteur-de-recherche-fts5.md) a
tranché FTS5 sur un corpus *avec* corps.

Un des quatre verbes du produit (*lire, trier, **chercher**, écrire*) est
donc amputé. Avant de décider quoi que ce soit, il faut des chiffres.

## Ce que le banc mesure

```powershell
# fermez l'application d'abord
cargo run --release -- "$env:APPDATA\dev.discovery.app\discovery.db" vous@gmail.com 200
```

- octets de HTML par corps, sur du courrier **réel** ;
- durée par corps, donc durée pour un compte entier ;
- **croissance de la base** — corps stockés ET entrées d'index FTS ;
- extrapolation au reste du compte.

### Deux garde-fous

1. **Il travaille sur une COPIE** de la base (suffixée `-banc.db`) : une
   mesure ne mute jamais l'état de production.
2. **Il n'emprunte que l'API publique du noyau** (`load_body`) : il mesure
   le vrai chemin de téléchargement, de mise en cache et d'indexation du
   produit, pas une imitation.

## Mesures

> À remplir au premier passage.

| Mesure | Valeur |
|---|---|
| Taille moyenne d'un corps | — |
| Durée moyenne par corps | — |
| Croissance de la base par corps | — |
| Extrapolation (compte de 2193 messages) | — |

## La décision qui suivra

Quatre politiques possibles, à départager sur ces chiffres :

| Politique | Ce qu'elle coûte |
|---|---|
| Tout rapatrier | le plus simple ; disque et bande passante maximaux |
| Les N derniers mois | couvre l'usage réel de la recherche à coût borné |
| Au fil de l'eau (pompe de fond) | préserve « enveloppes d'abord », complexité d'une pompe reprenable |
| Statu quo | recherche sur sujets seulement, à documenter honnêtement |

Le plan n'a **aucun budget disque** : s'il faut en poser un, c'est ici que
le chiffre se décide.
