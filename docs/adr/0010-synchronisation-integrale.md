# ADR 0010 — Synchronisation intégrale de la boîte

Date : 2026-07-25 · Statut : accepté
· Révise [ADR 0007](0007-rattrapage-des-corps.md) (horizon)
· Précise [ADR 0009](0009-portee-des-fils-au-compte.md) (portée du regroupement)

## Contexte

Trois mesures du terrain, prises le même jour, disent la même chose.

**1. La passe d'en-têtes a convergé, et son résultat est plafonné.**
`diagnostic_fils` sur la boîte réelle : `lus, sans References` (1 399) +
`lus, avec References` (257) = **1 656**, exactement le nombre de messages
`dans l'horizon (12 mois)`. La passe a lu 1 656 messages sur 1 656
éligibles. Elle n'a plus rien à lire.

**2. Les 5 883 messages hors horizon — 78 % de la base — ne seront jamais
lus.** Ils restent regroupés par le seul `In-Reply-To`, dont la mesure 2 de
l'[ADR 0008](0008-regroupement-en-conversations.md) établit qu'il « ne
regroupe presque rien » dans une boîte de réception.

**3. L'horizon de la passe d'en-têtes était un héritage, pas une
décision.** [`commands.rs`](../../apps/desktop/src/commands.rs) lui passe
`backfill_horizon()` — la borne de l'ADR 0007, qui existe pour tenir le
budget disque (< 1 Go, [PLAN.md](../PLAN.md) §1). Or un bloc d'en-têtes
pèse ~3 ko contre ~50 ko pour un message entier, et il ne se range pas sur
le disque comme un corps : il tient dans une colonne. La borne a été
reprise parce que la fonction avait la même *forme*, pas la même *raison*.

### Le chiffrage, contre les options déjà écartées

| Option | Réseau | Verdict |
|---|---|---|
| `References` dans la synchronisation | 150 Mo / 50 000 msg | écartée (ADR 0008) |
| En-têtes portés par le rattrapage des corps | 137 Mo mesurés | écartée (ADR 0008) |
| **Étendre l'horizon des en-têtes seuls** | ~17,6 Mo | jamais examinée |

Deux ordres de grandeur sous les deux options rejetées.

## Décision

Le Chef Ingénieur tranche plus largement que le constat ne l'exigeait :
**à l'ajout d'un compte, Discovery télécharge l'intégralité de la boîte**,
tous dossiers confondus, sans horizon et sans quota de volume.

### 1. Toutes les boîtes, sans exception

INBOX, Envoyés, Archive, Corbeille, Spam, et tout dossier utilisateur.
**Aucune exception**, y compris le Spam et la Corbeille : le logiciel ne
juge pas ce qui mérite d'être conservé. Chercher un message qu'on a jeté
par erreur est un besoin réel ; un dossier exclu est un trou que
l'utilisateur ne peut pas combler.

### 2. Le gate « base < 1 Go » est LEVÉ — explicitement

C'est une décision de Chef Ingénieur, pas une dérive : elle est inscrite
ici pour qu'aucune revue future ne la prenne pour un oubli. Le budget du
[PLAN.md](../PLAN.md) §1 devient sans objet, remplacé par une **garde
d'espace disque** (§4).

Un gate levé n'est pas un gate oublié. Les autres — démarrage, ouverture,
page de liste, RAM, zéro perte — **restent bloquants**, et cette décision
les met sous tension : ils seront re-mesurés (§6).

### 3. Stocker et indexer ≠ regrouper

**C'est le cœur de cet ADR**, et il protège une décision gelée.

- **Stockage et recherche** : tous les messages, de toutes les boîtes.
  Tout devient lisible et cherchable.
- **Regroupement en fils** : la portée reste **INBOX + Envoyés**
  ([ADR 0009](0009-portee-des-fils-au-compte.md)). Un message hors de cette
  portée garde `thread_id = NULL`.

Sans cette séparation, l'ADR 0009 tombe par un chemin qu'aucun test actuel
ne surveille. `thread::adopt` travaille par **compte** : dès qu'Archive et
Spam entrent dans le compte, leurs messages rejoignent les fils, et trois
agrégats se corrompent en silence —

| Agrégat | Ce qui casse |
|---|---|
| `size` | « 12 messages » sur un fil qui en montre 3 |
| `unseen` | un fil éternellement non lu à cause d'un spam jamais ouvert |
| `last_epoch`, `last_mailbox_id` | **une conversation remonte en tête de liste parce qu'un spam s'y est accroché** |

Le troisième est un défaut de **correction**, pas d'ergonomie : la liste
mentirait sur l'ordre des échanges, et l'utilisateur n'aurait aucun moyen
de le défaire. C'est le même motif de refus que le regroupement par sujet
(ADR 0008 §2).

L'alternative écartée par l'ADR 0009 — « portée = toutes les boîtes, y
compris Archive et Corbeille » — reste donc écartée, **pour sa raison
d'origine** : elle ressusciterait des conversations que l'utilisateur a
rangées ou jetées. Télécharger l'archive ne la rouvre pas.

### 3 bis. `inbox_size` survit, et l'index partiel avec lui

[`thread.rs`](../../crates/mail-core/src/thread.rs) prévenait qu'il
faudrait « repenser plutôt que paramétrer » `RECEIVED_MAILBOX` le jour où
la liste montrerait plusieurs boîtes. **Ce jour n'est pas arrivé** : la
liste montre toujours les fils ayant au moins un message dans INBOX.

`inbox_size > 0` garde donc son sens, l'index partiel de l'ADR 0009 §4
reste valide, et le test de plan d'exécution
(`la_boite_unifiee_ne_materialise_pas_son_tri`) continue de le garder. Le
gate 3 n'est pas menacé par cette porte.

### 4. Vérification d'espace AVANT, message clair si insuffisant

Pas de quota, mais pas de disque plein non plus. Avant d'engager la
synchronisation d'un compte, on estime le volume et on le compare à
l'espace libre. S'il manque, **on refuse et on le dit** — on ne commence
pas pour s'arrêter au milieu.

L'estimation repose sur deux mesures du projet, pas sur un chiffre
inventé :

- **~49 ko par message**, pièces jointes comprises — le rattrapage complet
  de la boîte réelle a été mesuré à 137 Mo pour 2 801 messages ;
- **~1,2 ko d'enveloppe et d'index** — déduit de `gate3-corps.db`
  (778,9 Mo pour 200 000 enveloppes + 16 002 corps).

Soit **~50 ko par message**, multiplié par la somme des `EXISTS` que le
serveur annonce par dossier (gratuit, déjà dans le protocole). Ordres de
grandeur : 50 000 messages → 2,5 Go ; 100 000 → 5 Go.

L'estimation est **délibérément haute** : annoncer trop et tenir vaut
mieux que commencer et échouer à mi-chemin.

### 5. En tâche de fond, avec un avancement en pourcentage

Même forme que le rattrapage des corps (ADR 0007) : bornée par cycle,
reprenable — l'état, c'est la base — et groupée. Elle ne bloque jamais la
lecture : la liste est utilisable dès les premières enveloppes.

L'avancement s'affiche en **pourcentage** : dénominateur = somme des
`EXISTS`, numérateur = messages en base. Un travail long qui ne dit pas où
il en est est indistinguable d'un travail bloqué — c'est l'enseignement
« ne jamais avaler une erreur » appliqué à la durée.

### 6. Ce qui doit être re-mesuré, et ce qui va faire mal

**La recherche est déjà hors budget** (118–208 ms, cible < 100 ms) et se
paie **au nombre de correspondances** (~2,9 µs l'unité, plafond vers
35 000 — ADR 0009). Multiplier le corpus par l'archive entière rapproche
mécaniquement ce plafond.

Les deux leviers sont chiffrés et disponibles : **tri par date** (×2,
quatre requêtes sur six repassent sous le budget) et l'option **`prefix=`**
de FTS5 (−73 ms d'expansion). Le Chef Ingénieur avait choisi de trancher le
premier en bêta, sur de vraies boîtes ; **cette décision le remet sur le
chemin critique**.

À re-mesurer sur `gate3-corps.db` : recherche, démarrage à froid, adoption
d'une base héritée.

## Conséquences

**Positives**

- Les 5 883 messages hors horizon deviennent cherchables — la recherche
  « plein texte » porte enfin sur toute la correspondance.
- Le regroupement se déplafonne : les `References` de toute la boîte
  entrent dans l'annuaire, et la convergence de l'ADR 0008 §5 fait le
  reste sans qu'aucune information acquise ne soit perdue.
- Un message archivé, supprimé ou tombé en spam se retrouve. C'est un
  besoin réel qu'aucun repli ne couvrait.

**Négatives, assumées**

- **Le disque n'est plus borné.** Décision explicite (§2).
- **La recherche s'éloigne de son budget** avant que ses leviers ne soient
  appliqués (§6).
- **L'adoption d'une base héritée grandit avec elle** : 3,7 s à 200 000
  messages, sur un chemin déjà hors budget. Le chantier « migration
  visible et interruptible » retenu pour la Phase 5 en devient un
  **prérequis**, plus une amélioration.
- **Le Spam entre dans les résultats de recherche.** Conséquence directe et
  voulue du §1.
- La première synchronisation d'un compte existant est longue et
  volumineuse. Elle s'annonce (§5) plutôt que de se subir.

## Alternatives écartées

| Option | Pourquoi non |
|---|---|
| Étendre le seul horizon des en-têtes (~17,6 Mo) | Suffisait à déplafonner le **regroupement**, mais laissait 78 % de la boîte hors de la **recherche**. Écarté par le Chef Ingénieur au profit du périmètre complet. |
| Exclure le Spam | Le plus gros dossier d'une boîte ancienne, et jamais cherché — mais c'est un jugement du logiciel sur ce qui mérite d'être gardé. Refusé. |
| Exclure la Corbeille | Chercher un message supprimé par erreur est précisément le cas où l'utilisateur a besoin du logiciel. |
| Quota de volume avec purge des plus anciens | Rend le résultat d'une recherche dépendant de la date à laquelle on la fait. Un trou silencieux est pire qu'un disque plein annoncé. |
| Porter la portée des fils à toutes les boîtes | Ressusciterait des conversations rangées ou jetées, et corromprait trois agrégats (§3). Déjà écarté par l'ADR 0009. |
