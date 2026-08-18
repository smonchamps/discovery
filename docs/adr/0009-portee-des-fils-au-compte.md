# ADR 0009 — La portée d'un fil est le COMPTE, pas la boîte

Date : 2026-07-25 · Statut : accepté
· Révise [ADR 0008](0008-regroupement-en-conversations.md) §3 et §4

## Contexte

L'[ADR 0008](0008-regroupement-en-conversations.md) §3 a gelé une règle :
*« les fils sont **relatifs à une boîte** (`mailbox_id`) […] nos propres
réponses vivent dans "Envoyés", que la v1 ne synchronise pas »*.

La règle était **conditionnée à cette prémisse**, et la prémisse vient de
tomber : le Chef Ingénieur a décidé de synchroniser « Envoyés ». Cette
décision était elle-même reportée après le gate 3, pour connaître le coût à
l'échelle avant d'engager un second dossier
([PHASE3.md](../archives/PHASE3.md) §5) — il est désormais connu.

Ce n'est donc pas rouvrir une décision gelée contre une mesure : c'est en
retirer le socle. La règle du §5 de la passation est respectée.

### Le constat qui rend le chantier nécessaire

La synchronisation seule n'apporterait **rien**. Le moteur de synchro est
déjà paramétré par nom de boîte (`commands.rs` fixe simplement
`MAILBOX = "INBOX"`) : ajouter « Envoyés » est de la plomberie. Mais le
regroupement, lui, est cloisonné :

```sql
threads.mailbox_id  NOT NULL
thread_links        PRIMARY KEY (mailbox_id, message_id)
```

Une réponse rangée dans « Envoyés » formerait **son propre fil, dans son
propre espace d'identifiants**, sans jamais rejoindre celui d'INBOX. On
paierait la synchronisation, le disque et l'index de recherche pour zéro
regroupement supplémentaire.

C'est ce cloisonnement, et lui seul, qui empêchait le regroupement de
rapporter : 40 messages regroupés en 15 conversations sur 2 813 messages
réels (ADR 0008, constat de terrain).

## Décision

### 1. Un fil appartient à un COMPTE

`threads.account_id` remplace `mailbox_id` ; `thread_links` est re-clé sur
`(account_id, message_id)`. Un message reçu et la réponse qu'on lui a faite
appartiennent au même fil, puisqu'ils appartiennent au même échange.

La portée s'arrête au compte : deux comptes ne fusionnent jamais leurs
fils, même si le même message y figure. L'invariant *« identité =
`(account_id, uid)` »* (passation §6.2) l'exige, et un fil qui
traverserait les comptes rendrait la boîte unifiée impossible à expliquer.

### 2. Ce que la liste montre

Un fil a une ligne dans la liste **dès qu'il contient au moins un message
reçu**. Il est représenté par son message le **plus récent, d'où qu'il
vienne** — y compris nos propres réponses.

Répondre fait donc remonter la conversation, et l'extrait affiché devient
notre réponse. C'est ce que fait Gmail, et c'est cohérent avec la question
que la liste répond : *« où en est cet échange ? »*, pas *« quand
m'a-t-on écrit ? »*.

**Un fil purement sortant n'a pas de ligne** : écrire à quelqu'un qui ne
répond jamais ne crée pas une conversation dans la boîte de réception.
C'est le pendant exact de la règle « un fil ne montre que ce que la boîte
contient » — la boîte de réception reste ce qu'on a reçu.

### 3. Le compteur couvre tout l'échange

« 3 » sur une ligne signifie trois messages dans la conversation, reçus et
envoyés confondus.

Ce n'est pas un raffinement, c'est une **cohérence obligatoire** : le
bandeau de conversation montre l'échange complet. Un compteur qui
n'annoncerait que les reçus contredirait à l'écran ce que l'ouverture du
fil affiche — exactement le défaut « deux chiffres qui se contredisent »
que l'ADR 0008 §4 cherchait à éviter en recalculant l'agrégat plutôt qu'en
l'incrémentant.

### 4. Un index PARTIEL, sans quoi le gate 3 est perdu

Le gate 3 vient de corriger un tri matérialisé qui coûtait jusqu'à 987 ms
par page ([PHASE3.md](../archives/PHASE3.md) §2). La règle §2 ci-dessus le
ramènerait par une autre porte : filtrer « les fils ayant au moins un
message reçu » **tout en triant par date** oblige SQLite à parcourir puis
jeter tous les fils purement sortants.

L'agrégat `threads` porte donc un compteur de messages reçus, et l'index
qui sert la liste est **partiel** :

```sql
CREATE INDEX idx_threads_date_globale
    ON threads(last_epoch DESC, last_uid DESC, account_id)
    WHERE inbox_size > 0;
```

Le filtre entre ainsi dans l'index au lieu d'être évalué après lui. La
promesse de l'ADR 0008 §4 — *le coût d'une page ne dépend pas de la taille
de la boîte* — est maintenue **par construction**, et le test de plan
d'exécution (`la_boite_unifiee_ne_materialise_pas_son_tri`) la garde.

### 5. L'agrégat doit désigner sa boîte

`threads.last_uid` ne suffit plus : le dernier message d'un fil peut être
dans INBOX ou dans « Envoyés », et *« un UID seul n'identifie rien »*
(passation §6.2). L'agrégat porte donc `last_mailbox_id` en plus de
`last_uid`.

### 6. La migration passe par le marqueur de version

Les tables changent de clé : SQLite impose une reconstruction. Le mécanisme
existe déjà — `PRAGMA user_version` inférieur à `THREADING_VERSION`
déclenche l'effacement des fils et leur recalcul complet à l'ouverture
(ADR 0008 §1 bis). Il suffit donc de **supprimer les deux tables** dans ce
chemin et d'incrémenter la version.

Coût mesuré de la reconstruction : **4,22 s pour 200 000 messages**, une
seule fois. Il est déjà hors budget et déjà consigné comme report
([PHASE3.md](../archives/PHASE3.md) §4) ; ce chantier ne l'aggrave pas d'un ordre de
grandeur, mais il **rend son traitement plus urgent** — la migration
touchera cette fois toutes les bases existantes, pas seulement les
héritées.

### 7. Découverte du dossier « Envoyés »

Attribut `\Sent` annoncé par le serveur, puis **repli par nom** (`sent`,
`envoyés`, `éléments envoyés`) — même ordre de priorité et même exception
délibérée à la règle « jamais de nom en dur » que l'archivage
([ADR 0006](0006-microsoft-imap-oauth2.md)), et pour la même raison : un
serveur réel n'annonce pas toujours ce qu'il possède. Le décodage UTF-7
modifié (`mutf7`) sert ici aussi.

Si aucun dossier n'est trouvé, le compte fonctionne comme avant : les fils
ne regroupent que les reçus. Une dégradation locale et silencieuse, jamais
une erreur.

## Conséquences

**Positives**

- Le regroupement rapporte enfin ce pour quoi il a été écrit : un échange
  complet dans une ligne, dans l'ordre où il s'est déroulé.
- La lecture d'un fil ne demande aucun aller-retour réseau de plus.
- « Envoyés » devient synchronisé, ce qui solde un report de Phase 3.

**Négatives, assumées**

- **Le corpus de recherche grandit**, et la recherche se paie au nombre de
  correspondances (~2,9 µs l'unité, plafond vers 35 000). Ajouter
  « Envoyés » rapproche donc ce plafond. À re-mesurer.
- **Le disque grandit** : enveloppes, index FTS, et corps rattrapés du
  dossier « Envoyés ».
- **Toutes les bases existantes reconstruisent leurs fils** au premier
  lancement — 4,22 s à 200 000 messages, sur un chemin déjà identifié comme
  hors budget.
- La liste **change d'ordre** sous les yeux de l'utilisateur au premier
  lancement : des conversations remontent parce qu'il y avait répondu.
  Attendu, mais à ne pas découvrir sans prévenir.

## Alternatives écartées

| Option | Pourquoi non |
|---|---|
| Synchroniser « Envoyés » sans changer la portée | Coût payé, gain nul : les fils resteraient cloisonnés par boîte. |
| Portée = toutes les boîtes d'un compte, y compris Archive et Corbeille | Ressusciterait des conversations que l'utilisateur a rangées ou jetées. INBOX + Envoyés est le périmètre de l'échange **vivant** ; élargir se décidera sur un besoin observé. |
| Portée = le compte, mais la liste triée sur le dernier message REÇU | Une conversation où l'on vient de répondre resterait figée à sa date d'avant — l'inverse de « où en est cet échange ». Écarté par le Chef Ingénieur. |
| Filtrer les fils sortants après l'index (sans index partiel) | Réintroduit le parcours que le gate 3 vient de supprimer (§4). |
| Compteur limité aux messages reçus | Contredirait à l'écran le bandeau de conversation, qui montre l'échange entier (§3). |
