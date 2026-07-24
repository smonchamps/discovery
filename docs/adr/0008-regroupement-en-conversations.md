# ADR 0008 — Regroupement en conversations : en-têtes seuls, acquisition à deux vitesses

Date : 2026-07-24 · Statut : accepté

## Contexte

Le regroupement des messages en conversations est le dernier grand poste
fonctionnel de la Phase 3. Il touche trois choses à la fois : le modèle
(quels en-têtes rattachent un message à un fil), le stockage (comment
paginer une liste groupée sans agréger toute la boîte), et **le rendu de
la liste**, qui est le chemin chaud du produit — 60 fps, virtualisation
([PLAN.md](../PLAN.md) §1).

Deux mesures ont été prises avant d'écrire une ligne, et la seconde a
renversé le plan initial.

### Mesure 1 — `In-Reply-To` est déjà dans les octets qu'on télécharge

L'ENVELOPE IMAP (RFC 3501 §7.4.2) porte `In-Reply-To`. La synchronisation
le recevait déjà et le jetait. Le premier niveau de regroupement coûte
donc **zéro octet de réseau**.

### Mesure 2 — `In-Reply-To` seul ne regroupe presque rien dans une boîte de réception

Le cas normal d'un échange, vu depuis INBOX :

1. je reçois **A** ;
2. je réponds **B** — qui part dans « Envoyés », **pas** dans INBOX ;
3. je reçois **C**, qui porte `In-Reply-To: B`.

INBOX contient A et C. C pointe sur un message absent. Sans `References`
— qui porte aussi la **racine** A — les deux restent deux fils séparés.

**Le « chaînon manquant » n'est donc pas un cas limite : c'est le cas
majoritaire d'une vraie correspondance.** `References` est obligatoire,
pas un raffinement.

### Mesure 3 — `References` ne peut pas voyager avec l'ENVELOPE

`References` n'est pas dans l'ENVELOPE. La crate `imap` n'expose
`Fetch::header()` que pour `BODY[HEADER]` — le bloc **entier**, ~3 ko —
et pas pour `HEADER.FIELDS (REFERENCES)`, qui pèserait ~150 o. Mettre le
bloc entier dans la synchronisation décuplerait le coût de « enveloppes
d'abord » : ~150 Mo pour les 50 000 messages de la Phase 1.

## Décision

### 1. Regroupement par union-find sur les identifiants RFC 5322 — et rien d'autre

Chaque `Message-ID` rencontré — celui du message **et ceux de ses
ancêtres, même absents de la boîte** — est inscrit dans un annuaire qui
pointe vers un fil. Un message citant deux identifiants rattachés à deux
fils différents les **fusionne**.

L'algorithme est **pur et sans I/O**
([`thread.rs`](../../crates/mail-core/src/thread.rs)), testé contre les
cas du terrain : arrivée dans le désordre, ancêtre absent, message qui
relie deux fils, auto-référence, `References` de plusieurs milliers
d'entrées, `Message-ID: <>` malformé.

### 2. Refus explicite : jamais de regroupement par sujet

L'algorithme JWZ propose, en repli, de regrouper les messages de même
sujet une fois « Re: » retiré. **Nous le refusons.**

Un tel repli fusionne des messages sans aucun lien réel dès que le sujet
est banal — « Bonjour », « Facture », « Question ». Dans un client mail,
c'est une faute de **correction**, pas d'ergonomie : l'utilisateur voit
une conversation qui n'a jamais existé, et rien dans l'interface ne lui
permet de la défaire. Un fil coupé en deux est réparable et honnête ; deux
messages étrangers réunis ne le sont pas.

Conséquence assumée : les correspondants dont le logiciel n'émet ni
`In-Reply-To` ni `References` ne seront pas regroupés. C'est le bon côté
de l'échange.

### 3. Un fil ne regroupe que ce que la boîte contient

Les fils sont **relatifs à une boîte** (`mailbox_id`). Le compteur affiché
est donc celui des messages **reçus** : nos propres réponses vivent dans
« Envoyés », que la v1 ne synchronise pas. C'est cohérent avec ce que la
liste montre — elle n'affiche pas non plus nos envois.

### 4. Agrégat matérialisé, jamais incrémenté

Une table `threads` porte, par fil, son dernier message, sa date, sa
taille et son nombre de non-lus. La liste part de **cette** table :

```sql
FROM threads t JOIN envelopes e ON e.uid = t.last_uid ...
ORDER BY t.last_epoch DESC LIMIT ? OFFSET ?
```

Un `GROUP BY thread_id` avec `MAX(date)` obligerait SQLite à parcourir
puis trier les 200 000 enveloppes **à chaque page de défilement**. Ici
l'index porte le tri et la pagination : le coût d'une page ne dépend plus
de la taille de la boîte.

L'agrégat se **recalcule**, il ne s'incrémente pas. Un compteur entretenu
par additions dérive au premier chemin oublié (fusion, UIDVALIDITY,
action rejouée), et une dérive se voit à l'écran pour toujours : « 4
messages » sur un fil qui en montre 3. Le recalcul est borné par la
taille du fil.

Comme l'index de recherche ([ADR 0004](0004-moteur-de-recherche-fts5.md)),
l'agrégat s'entretient **dans la transaction** qui écrit le message.

### 5. Acquisition à deux vitesses, et convergence

| En-tête | Source | Coût | Quand |
|---|---|---|---|
| `In-Reply-To` | ENVELOPE | 0 o | à la synchronisation |
| `References` | `BODY.PEEK[HEADER]` | ~3 ko | passe de fond bornée |

La passe d'en-têtes réutilise la connexion **déjà ouverte** par la
synchronisation : elle ne coûte aucun aller-retour supplémentaire. Elle
est bornée (2 000 messages par compte et par synchronisation), reprenable
(son état, c'est la base) et groupée, comme le rattrapage des corps
([ADR 0007](0007-rattrapage-des-corps.md)).

Livrer l'acquisition en deux temps n'est possible que grâce à une
propriété de l'algorithme : la fusion le rend **convergent**. Un fil né en
deux morceaux se recolle dès que le lien manquant apparaît, sans qu'aucune
information acquise ne soit perdue. Les conversations se regroupent donc
progressivement, jamais à l'envers.

`refs = NULL` signifie « jamais lu », `refs = ''` signifie « lu, et il n'y
en a pas ». Confondre les deux ferait redemander éternellement les mêmes
messages.

### 6. Un fil vidé disparaît avec son annuaire

Archiver tous les messages d'une conversation supprime le fil et ses
liens. Une réponse ultérieure ouvrira un fil neuf — ce qui est honnête,
puisque la boîte ne contient plus rien de cet échange. Garder les fils
vides obligerait la liste à les filtrer, au prix de l'index qui la rend
rapide.

## Conséquences

**Positives**

- Une ligne par conversation, avec son compteur ; un fil reste non lu tant
  qu'il lui reste un message non lu, même si le dernier est lu.
- Le coût d'une page de liste reste indépendant de la taille de la boîte.
- Aucun aller-retour réseau ajouté ; ouvrir un fil est **purement local**,
  comme choisir un dossier de destination.
- Les bases existantes sont **adoptées** à l'ouverture : sans cette passe,
  chaque message hérité aurait gardé `thread_id` NULL et la liste — qui
  part de `threads` — aurait été vide. C'est le piège des pièces jointes,
  cette fois traité d'emblée et prouvé par test.

**Négatives, assumées**

- Les correspondants sans en-têtes de fil ne sont pas regroupés (§2).
- Le compteur ignore nos propres réponses (§3).
- Les messages hors de l'horizon de récence (12 mois, ADR 0007) ne voient
  pas leurs `References` rapatriées : ils restent regroupés par le seul
  `In-Reply-To`.
- Un message qui n'est pas en tête de son fil **n'a plus de ligne à lui**
  dans la liste. C'est l'objet même du regroupement, mais cela change la
  navigation : on l'atteint par le bandeau de conversation.

## Alternatives écartées

| Option | Pourquoi non |
|---|---|
| `X-GM-THRID` (fil natif Gmail) | Propre à Gmail. Le produit sert désormais Microsoft 365 et l'IMAP générique ; une voie par fournisseur est exactement ce que le trait `MailServer` existe pour éviter. |
| Repli sur le sujet | Fusionne des messages étrangers, sans recours pour l'utilisateur (§2). |
| `References` dans la synchronisation | ~150 Mo sur 50 000 messages : détruirait « enveloppes d'abord » (mesure 3). |
| Faire porter les en-têtes par le rattrapage des corps | Gratuit en octets — les corps contiennent les en-têtes — mais imposerait de **re-télécharger toute la boîte** (137 Mo mesurés chez l'utilisateur contre 8 Mo pour les seuls en-têtes), et ne couvrirait pas les messages hors horizon des corps. |
| `GROUP BY` à la volée | Parcours + tri de toute la boîte à chaque page (§4). |
