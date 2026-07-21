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

## Mesures — compte Gmail réel, 200 corps, 2026-07-21

| Mesure | Valeur |
|---|---|
| Corps téléchargés | 200 (0 échec) |
| HTML transféré | 11,6 Mo — **59,2 Ko par corps** |
| Durée | 38,3 s — **192 ms par corps** |
| Croissance de la base | +12,1 Mo — **61,9 Ko par corps** |
| Extrapolation (1992 restants) | ~6,4 min · ~120 Mo · base finale ~137 Mo |

### La surprise : l'index est presque gratuit

Croissance disque **61,9 Ko** par corps, pour **59,2 Ko** de HTML. L'écart —
**~2,5 Ko par message** — est tout ce que coûte l'index FTS5. La vigilance
« taille » de l'[ADR 0004](../../docs/adr/0004-moteur-de-recherche-fts5.md)
est levée : la table *contentless* tient sa promesse.

**Ce n'est donc pas l'index qui coûte, ce sont les corps stockés** — 25×
plus. Cela ouvre une option que les chiffres seuls révèlent : indexer sans
stocker.

### Mise à l'échelle du gate 3 (200 000 messages cumulés)

| Politique | Disque | Temps (séquentiel) |
|---|---|---|
| Stocker tous les corps | **12,4 Go** | 10,7 h |
| Indexer sans stocker | **500 Mo** | 10,7 h |
| Plafond de récence (12 mois, 3 comptes) | ~370 Mo | ~20 min |

### Nuance sur le temps

Les 192 ms/corps mesurent le chemin actuel de `load_body` : **un aller-retour
IMAP par message**. Un rattrapage réel grouperait les `FETCH` (50 par
commande), ce qui réduirait fortement ce chiffre. **192 ms est un majorant,
pas le coût incompressible.**

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
