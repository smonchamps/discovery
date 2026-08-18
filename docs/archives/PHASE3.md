# Revue de clôture — Phase 3 « recherche, multi-comptes, échelle » (2026-07-25)

La recherche, le multi-comptes et le passage à l'échelle du plan
([PLAN.md](../PLAN.md) §4) sont livrés, validés sur trois comptes réels
(Gmail, Microsoft 365, IMAP générique) et re-mesurés au gate 3 —
**3 comptes, 200 000 messages cumulés**.

Le gate a trouvé un défaut que ni les tests ni le terrain n'auraient pu
trouver, parce qu'il n'apparaît qu'à l'échelle. C'est exactement ce pour
quoi il existe.

## 1. Livré, contre le plan

| Exigence du plan | État | Preuve |
|---|---|---|
| Recherche plein-texte FTS5 (< 100 ms sur 100 000 messages) | ⚠️ | Tenue jusqu'à ~35 000 correspondances ; au-delà, le classement BM25 domine — voir §2 et §4 |
| Filtres `from:` / `date:` | ✅ | `parse_query`, repli sur la date quand la requête n'a pas de terme |
| Filtres `to:` / « a une pièce jointe » | ❌ | Reportés (§4) |
| Multi-comptes Gmail + Microsoft + IMAP générique | ✅ | Trois comptes réels ; fournisseurs décrits **en données** ([ADR 0006](../adr/0006-microsoft-imap-oauth2.md)) |
| Boîte unifiée | ✅ | `unified_recent` ; identité `(account_id, uid)` jusque dans la sélection |
| Pièces jointes | ✅ (lecture) | Trombone en liste, liste au détail ; envoi reporté (§4) |
| Notifications Windows | ✅ | Validé **application installée** — l'identité applicative exige un raccourci du menu Démarrer |
| Threading des conversations | ✅ | Union-find pur sur en-têtes RFC 5322 ([ADR 0008](../adr/0008-regroupement-en-conversations.md)) |
| Dossiers et déplacement | ✅ | Report de Phase 2 rattrapé ; décodage UTF-7 modifié (`mutf7`) |
| Rattrapage des corps | ✅ | Non prévu au plan — imposé par la mesure ([ADR 0007](../adr/0007-rattrapage-des-corps.md)) |
| Brouillons : tirage et conflit d'édition | ✅ | Report de Phase 2 rattrapé ; validé terrain |
| **Gate : budgets tenus avec 3 comptes et 200 000 messages** | ⚠️ | Six postes sur huit ✅ — la recherche et l'adoption d'une base héritée font exception |

294 tests Rust · 19 parcours E2E · clippy `-D warnings`.

## 2. Budgets re-mesurés — gate 3 (release, 3 comptes, 200 000 messages)

| Métrique | Phase 2 (50 000) | **Gate 3 (200 000)** | Budget | |
|---|---|---|---|---|
| Démarrage → fenêtre utilisable | 350 ms | **360–389 ms** | < 1 s | ✅ |
| RAM résidente (working set privé) | 89,6 Mo | **92,2 Mo** | < 200 Mo | ✅ |
| Page de liste (application complète) | 3,82 ms | **12,4 ms** | < 100 ms | ✅ |
| Ouverture d'un message | — | **0,09–0,16 ms** | < 50 ms | ✅ |
| Taille de la base | 97 Mo / 2 800 msg | **778,4 Mo** / 200 000 msg + 16 002 corps | < 1 Go | ✅ |
| Recherche | — | **118–208 ms** | < 100 ms | ❌ |
| Ouverture d'une base **héritée** (adoption des fils) | — | **4,22 s** | < 1 s | ❌ |
| Perte de données | 0 | 0 | 0 | ✅ |

**Le volume ne coûte presque rien en RAM** : +2,6 Mo pour ×4 le nombre de
messages. La virtualisation de la liste isole la mémoire du volume, comme
au gate 1.

**Le budget disque est confortable**, et testé plus durement qu'il ne le
sera : 16 002 corps stockés, soit **2,7× ce que modélise l'horizon 12 mois**
de l'ADR 0007. À 778 Mo, la marge reste de 22 % dans ce cas défavorable.

### Le défaut trouvé par le gate

La page de liste coûtait **87 ms**, et jusqu'à **987 ms** en défilement
profond. L'[ADR 0008](../adr/0008-regroupement-en-conversations.md) §4
promettait pourtant que « le coût d'une page ne dépend plus de la taille
de la boîte ».

La promesse valait pour **une** boîte. La **boîte unifiée** — la vue par
défaut du produit — couvre la même boîte de tous les comptes : elle filtre
sur le nom de la boîte, pas sur un `mailbox_id`. Un index préfixé par
cette colonne ne peut donc plus porter l'ordre global, et SQLite retombait
sur un tri matérialisé de 160 000 conversations **à chaque page**.

| offset | avant | après |
|---|---|---|
| 0 | 75,4 ms | **0,60 ms** |
| 20 000 | 648 ms | 31 ms |
| 150 000 | 867 ms | 228 ms |

Correctif : un second index portant le même tri sans préfixe de boîte,
créé en 62,8 ms sur la base de 160 000 fils. Gardé par un test qui
interroge le **plan d'exécution** et non un chronomètre — une durée dépend
de la machine, un plan non.

### L'adoption d'une base héritée — le risque nommé, et réel

Le §8 de la passation désignait `migrate_threads` comme « principal risque
pour le budget de démarrage » : il rattache à un fil **tous** les messages
qui n'en ont pas, à l'ouverture. Instantané sur 2 800 messages, il n'avait
jamais été mesuré sur 200 000.

Il l'est maintenant, et le risque était fondé. Deux régimes, qui ne se
confondent pas :

| Régime | Quand | Mesure |
|---|---|---|
| **à jour** | à chaque démarrage | **2,5 ms** ✅ |
| **adoption** | une fois, sur une base jamais regroupée | **4,22 s** ❌ |

Le cas courant — celui que l'utilisateur paie tous les jours — est
excellent. C'est la migration qui coûte, et elle bloque la fenêtre sans
rien dire.

Deux correctifs mesurés l'ont ramenée de **11,1 s à 4,22 s** :

1. `Vec::contains` sur l'ensemble des fils touchés était **quadratique** —
   160 000 fils font ~1,3×10¹⁰ comparaisons. Un `BTreeSet` : 11,1 → 8,5 s.
2. **Aucune requête du chemin chaud n'était mise en cache.** Chaque message
   provoquait ~5 `prepare`, soit ~1 million d'analyses SQL. `prepare_cached` :
   8,5 → 4,22 s. Ce gain profite aussi à **chaque synchronisation**, qui
   emprunte le même chemin.

Le reste — ~1 million d'exécutions de requêtes — est le coût intrinsèque
d'un union-find message par message. Le réduire encore demanderait une
adoption *ensembliste*, que l'algorithme n'accepte pas naturellement.

**Honnêteté sur la démarche :** j'avais annoncé le quadratique comme cause
dominante avant de le mesurer. Il ne valait qu'un quart du coût. La cause
principale était ailleurs, et seule la mesure l'a désignée — le §3.2 des
enseignements, appliqué à moi-même.

### La recherche, l'autre poste non tenu

Le coût suit le nombre de correspondances : `ORDER BY rank` calcule BM25
sur **toutes**, pas sur les 50 rendues. Deux mesures le décomposent :

- à correspondances égales, `"fac"*` coûte **73 ms de plus** que
  `"facture"*` — c'est l'expansion de préfixe ;
- les 134 ms de `"facture"*` restent hors budget — c'est le classement.

Le chiffre transférable n'est donc pas la durée mais le **coût unitaire :
~2,9 µs par correspondance**, ~4,4 µs dès trois lettres. Le budget de
100 ms est atteint vers **35 000 correspondances**.

Deux réserves d'honnêteté :

1. le corpus du banc n'a que **six modèles de sujet**, donc un mot y touche
   16,7 % des messages *par construction*. Une vraie boîte est bien plus
   diverse. L'ADR 0004 avait fait la même réserve sur son propre banc ;
2. le mécanisme, lui, est réel et indépendant du corpus.

## 2 bis. Re-mesure après l'ADR 0009 (2026-07-25, même jour)

Le chantier de la [portée des fils au compte](../adr/0009-portee-des-fils-au-compte.md)
— « Envoyés » synchronisé, identité portant la boîte, index **partiel** —
a été livré après cette revue. Les budgets ont donc été re-mesurés.

| Métrique | Gate 3 | **Après ADR 0009** | |
|---|---|---|---|
| Page de liste (offset 0) | 0,71 ms | **0,71 ms** | ✅ |
| Adoption d'une base héritée | 4,22 s / 200 000 | **3,72 s** / 199 200 | ❌ inchangé |
| Ouverture d'un message | 0,09–0,16 ms | **0,09–0,15 ms** | ✅ |
| Recherche, coût unitaire | 2,43–4,43 µs/corr. | **2,47–4,58 µs/corr.** | ❌ inchangé |

**Aucune régression.** Les deux postes déjà hors budget le restent, dans
les mêmes proportions et pour les mêmes raisons.

### L'index partiel, éprouvé dans la condition qui le justifie

Le décor du gate 3 n'avait qu'une boîte par compte : **tous** les fils
portaient un message reçu, donc la clause `WHERE inbox_size > 0` de
`idx_threads_date_globale` n'écartait jamais rien. Elle n'avait, en
pratique, jamais servi.

Un décor à deux boîtes par compte a été fabriqué pour l'éprouver —
159 360 fils dont **seulement 79 200 avec un message reçu**, l'autre
moitié étant purement sortante :

| offset | INBOX seul (160 000 fils, tous visibles) | + « Envoyés » (79 200 visibles sur 159 360) |
|---|---|---|
| 0 | 0,71 ms | **0,71 ms** |
| 20 000 | 32,8 ms | 29,9 ms |

**Les ~80 000 fils invisibles coûtent exactement zéro.** La promesse de
l'ADR 0009 §4 est vérifiée, et non plus seulement raisonnée.

### Ce que la recherche donne sur une VRAIE boîte

Le coût unitaire est le seul chiffre transférable, et il se reporte :

- boîte réelle après « Envoyés » : **7 539 messages** ;
- même une requête matchant **tout** le corpus coûterait
  7 539 × 4,6 µs ≈ **35 ms**.

**Aucune requête ne peut donc dépasser le budget sur cette boîte.** Le
plafond des ~35 000 correspondances n'est atteignable qu'à l'échelle du
gate 3 — le poste reste hors budget en synthétique, et confortable en
usage réel. C'est exactement ce que la réserve d'honnêteté du §2 annonçait.

### Ce que le regroupement rapporte, mesuré sur le terrain

| | avant « Envoyés » | après | après la passe d'en-têtes |
|---|---|---|---|
| conversations de 2 messages ou plus | **15** | 234 | **248** |
| dont 6 à 20 messages | 0 | 4 | **6** |

De 15 à 248. Le plus gros fil réunit **14 messages** grâce aux
`References` rapatriées — c'est précisément le mécanisme que l'ADR 0008
(mesure 2) déclarait obligatoire.

La progression n'est pas finie : la passe est bornée par cycle, et
1 656 messages restent dans l'horizon de 12 mois.

**Limite nommée, non résolue :** les ancres « FANTÔME » des plus gros fils
montrent que des messages d'origine restent hors de la base — archivés
hors d'INBOX. Seule la synchronisation de l'archive les ramènerait, au
prix du disque et du plafond de recherche. Non tranché.

## 3. Enseignements consignés

1. **Un test vert peut encoder un modèle faux de l'autre écrivain.** La
   détection de conflit des brouillons était éprouvée en simulant le
   tirage par une *réécriture en place*. Le vrai tirage **remplace** :
   il retire la ligne et en importe une neuve. La ligne visée ayant
   disparu, la détection ne comparait plus qu'un horodatage, et se
   taisait. Règle : simuler l'autre écrivain en appelant **son vrai
   chemin**, jamais par une approximation qui lui ressemble.
2. **Une promesse d'index ne vaut que pour la requête qu'on avait en
   tête.** L'ADR 0008 §4 raisonnait sur une boîte ; le produit interroge
   la boîte unifiée. Un test de **plan d'exécution** attrape cette classe
   de régression, qu'aucun test fonctionnel ne voit et qu'aucun
   chronomètre ne rend reproductible.
3. **Les outils de mesure portent eux aussi des hypothèses
   d'environnement.** `mesure-ram.ps1` sommait toutes les instances de
   l'application : 202 Mo et 14 processus, soit deux applications
   additionnées, annonçant un budget dépassé qui tient largement.
   `mesure.mjs` n'isolait pas son profil WebView2, alors que le harnais
   E2E le fait — une fenêtre déjà ouverte faisait ignorer le port de
   débogage. Un outil de mesure se vérifie comme le reste.
4. **La recherche à la frappe émet TOUJOURS un préfixe** : le dernier
   terme devient `"terme"*`. Mesurer un mot entier ne mesure donc pas ce
   que le produit exécute, et la requête à préfixe n'est pas un cas
   limite — c'est le chemin normal, et le plus cher.
5. **Un défaut que le hasard masque est un défaut qu'on croit corrigé.**
   SQLite attribue `max(rowid) + 1` : quand le brouillon édité était le
   dernier, l'import reprenait l'identifiant libéré et la détection
   retombait sur ses pieds **par accident**. Le symptôme n'apparaissait
   qu'une fois sur deux.
6. **Un signal demandé doit être observable.** Deux versions d'un même
   brouillon — même sujet, même destinataire — étaient rigoureusement
   indiscernables dans le bandeau, qui n'affichait pas le corps. Le
   tirage fonctionnait ; rien à l'écran ne permettait de le constater, et
   la consigne de validation envoyée à l'utilisateur était donc
   invérifiable.
7. **Un statut posé sans regarder en efface un autre** — troisième
   occurrence. L'avertissement de conflit était recouvert une seconde plus
   tard par le bilan de la poussée, et la collision était *certaine* : le
   brouillon conservé à part est neuf, donc toujours à pousser.

## 4. Reporté, volontairement

- **Défilement profond** : `OFFSET` fait parcourir puis jeter *n* lignes,
  donc 228 ms à 150 000 conversations. Seule une pagination **par
  curseur** l'effacerait — elle change la signature du store et la liste
  virtualisée de l'UI. Un utilisateur qui fait défiler 60 000
  conversations est hors du client cible (1 à 4 comptes).
- **Recherche au-delà de ~35 000 correspondances.** Deux leviers mesurés
  et chiffrés : le **tri par date** au lieu de BM25 (×2, quatre requêtes
  sur six repassent sous le budget) et l'option **`prefix=`** de FTS5
  (−73 ms d'expansion). Le premier est un arbitrage produit — récence
  contre pertinence — que le corpus synthétique ne peut pas trancher.
  Décision : **on tranche en bêta sur de vraies boîtes**, ce que l'ADR
  0004 prévoyait déjà.
- **Adoption d'une base héritée de 200 000 messages** : 4,22 s, budget
  dépassé. Le précédent existe et il est bon — le rattrapage des corps
  ([ADR 0007](../adr/0007-rattrapage-des-corps.md)) a rendu un travail de fond
  **visible et interruptible** plutôt que de le cacher. Une migration qui
  s'annonce vaut mieux qu'une fenêtre figée quatre secondes sans
  explication. Ce qu'il ne faut PAS faire : adopter par tranches à chaque
  démarrage — la liste part de `threads`, donc une adoption partielle
  afficherait une boîte à moitié vide, et ce serait le piège du §3.6, la
  fonctionnalité fausse dès la première ouverture.
- **Envoi de pièces jointes** — lecture seule en v1.
- **Filtre « a une pièce jointe »**, **`to:` dans la recherche**.
- **Synchronisation du dossier « Envoyés »** — voir §5.
- **CONDSTORE réel, IDLE/push** — reports de Phase 1 inchangés.
- **Dossier CASA Google** — toujours côté produit-owner, chemin critique
  du lancement public.

## 5. La décision produit qui attendait le gate

Le regroupement en conversations est correct mais **rapporte peu** sur la
boîte réelle : 40 messages regroupés en 15 conversations sur 2 813. La
cause est une décision assumée (ADR 0008 §3) — *on ne regroupe que ce que
la boîte contient* — et les réponses de l'utilisateur vivent dans
« Envoyés », que la v1 ne synchronise pas.

L'arbitrage avait été explicitement reporté après le gate 3, pour
connaître le coût à l'échelle avant d'engager un second dossier.
**Ce coût est maintenant connu :**

- la RAM ne dépend pas du volume (+2,6 Mo pour ×4 de messages) ;
- le coût d'une page ne dépend plus de la taille de la boîte ;
- le disque tient avec 2,7× la charge modélisée ;
- en revanche, **la recherche se paie au nombre de correspondances** :
  ajouter « Envoyés » augmente le corpus, donc rapproche le plafond des
  35 000 correspondances.

La décision appartient au Chef Ingénieur et n'est pas prise ici.

## 6. Décision

**Gate 3 tenu sur six postes sur huit.** Les deux exceptions sont
documentées, mesurées, et chacune dispose d'un remède connu :

- **la recherche**, hors budget sur un corpus synthétique dont la
  sélectivité est reconnue extrême, avec deux leviers chiffrés en réserve ;
- **l'adoption d'une base héritée**, ramenée de 11,1 s à 4,22 s et qui ne
  se paie qu'une fois — le démarrage courant reste à 2,5 ms.

Aucun poste n'est cassé sans remède connu, et aucun défaut de perte de
données ni de sécurité n'a été trouvé.

**Le gate a fait son travail.** Les deux défauts qu'il a trouvés — le tri
matérialisé de la boîte unifiée et le coût de l'adoption — étaient
invisibles à l'échelle du terrain, et aucun test fonctionnel ne pouvait
les voir. C'est l'argument même du gate : il ne vérifie pas que le produit
marche, il vérifie qu'il marche **à la taille où il servira**.

**Phase 3 close.** Restent à arbitrer avant d'ouvrir la Phase 4 : la
synchronisation d'« Envoyés » (§5), et l'ordre entre la Phase 4 (web) et
la Phase 5 (durcissement et bêta) — la bêta étant précisément ce qui
permettrait de trancher la recherche sur de vraies boîtes.
