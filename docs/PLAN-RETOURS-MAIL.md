# PLAN-RETOURS-MAIL — quatre retours CE sur le courrier réel

**CHANTIER SOLDÉ le 2026-08-16 — terrain complet.** Commit `19ea16a`,
CI verte ([run 31972909869](https://github.com/smonchamps/wind/actions/runs/31972909869)).
Ouvert le 2026-08-16 à la suite de quatre retours terrain du Chef
Ingénieur sur ses vrais comptes. GO CE au STOP 1 le 2026-08-16
(verdicts §6). Deux passes terrain le même jour : 1re — **R4 OK**,
R2/R1 corrigés le jour même, R3 remonté à sa racine par capture
Gmail-vs-Wind ; 2e — **R1 OK, R2 OK** ; R3 corrigé (fuite du `<title>`)
puis **R3 OK** confirmé au terrain. Journal du Système : **A48**.
Reports assumés au §7.

Les quatre retours, dans l'ordre du CE :

- **R1** — « Répondre à tous » n'inscrit personne dans « À » sur les
  mails « à un seul expéditeur ».
- **R2** — des objets d'email portent des caractères parasites
  (ex. `Test \"Envoyés\"`, `Expédié : \"OHTO GS02 GS02 Stylo à...`).
- **R3** — l'objet du message apparaît en double, en haut du corps, dans
  le volet de lecture (ex. « Je vous présente ma nouvelle stratégie
  Linkedin 2026 lors d'une masterclass (+25 millions de vues) »).
- **R4** — dans « Envoyés », l'expéditeur/destinataire affiché est faux
  (ex. « Test PJ 3 » marqué *de* smonchamps@gmail.com *à*
  smonchamps@gmail.com alors qu'il partait à sebastien.monchamps@gmail.com).

---

## 1. Constat (instruction sur pièces)

Deux retours (**R2, R4**) ont une **cause tenue par le code**, prouvable
et corrigeable hors terrain. Deux (**R1, R3**) exigent une **mesure sur
la vraie boîte** avant qu'on en tienne la racine (§7.1 : l'assistant ne
lit pas la base du CE ; §2.5 genchi genbutsu).

### R2 — objets parasités : escapes IMAP non retirés — CAUSE TENUE

Le `\"` du retour n'est pas un artefact de copie : c'est le
**backslash-escape des chaînes entre guillemets d'IMAP** (RFC 3501
`quoted-string`) laissé intact.

- Un objet réel `Test "Envoyés"` circule sur le fil ENVELOPE sous la
  forme `"Test \"Envoyés\""` : les `"` internes sont échappés.
- `imap-proto` 0.16.7 **retire les guillemets externes mais NE
  DÉ-ÉCHAPPE PAS** le contenu. Prouvé par ses propres tests
  (`core.rs:255-262`) : `quoted("Hello \" ")` rend `Hello \" ` (le
  backslash reste), `quoted("Hello \\ ")` rend `Hello \\ `.
- `decode_header` (`crates/mail-imap/src/convert.rs:550`) prend ces
  octets tels quels, les présente à `mail-parser` comme
  `Subject: Test \"Envoyés\"` — aucun encoded-word RFC 2047 à décoder,
  donc rendus **verbatim, backslashes compris**.
- Même chemin pour le **nom d'affichage** de l'expéditeur
  (`sender_display` → `decode_header`, convert.rs:332) et pour les
  adresses (`address_literal`, convert.rs:339) : un nom
  `"Société ""ACME"""` ressortirait échappé lui aussi.

Le second exemple (`Expédié : "OHTO...`) est le même défaut : un objet
contenant un `"` littéral.

**Racine** : `decode_header` (et le décodage d'adresse) doivent
**dé-échapper les séquences `quoted-string` d'IMAP** (`\"` → `"`,
`\\` → `\`, seules séquences valides RFC 3501) avant tout autre
traitement.

**Point dur assumé** : IMAP transmet aussi les chaînes en *littéral*
(`{n}\r\n…`), où les octets sont bruts, sans escape — et `imap-proto`
ne nous dit pas laquelle des deux formes il a lue. Dé-échapper `\"`/`\\`
inconditionnellement corromprait un objet-littéral contenant réellement
la séquence backslash+guillemet — cas rarissime, contre le cas courant
(tout objet à guillemets aujourd'hui cassé). Le compromis est le même
que celui de tous les clients mûrs : on dé-échappe. Documenté au code.

### R4 — « Envoyés » : le destinataire n'est jamais stocké — CAUSE TENUE

L'enveloppe stockée **ne porte que l'expéditeur** : la table `envelopes`
a `sender`/`sender_address`, **aucune colonne destinataire**
(`store.rs:73-74`, `SELECT_UNIFIED` store.rs:313). C'est un choix de
la synchro « enveloppes d'abord » (le commentaire de `fetch_recipients`,
mail-imap/src/lib.rs:695, et de `reply_all_context`, commands.rs:2265,
l'assument : « l'enveloppe stockée ne porte que l'expéditeur »).

Conséquence dans le volet de lecture (`Fil.svelte:82-88`) :

```js
const propre = (m) => fil.ligne && m.sender_address === fil.ligne.account_email;
function destinataire(m) {
  const vise = propre(m)
    ? fil.messages.find((x) => x.sender_address && x.sender_address !== m.sender_address)
    : fil.messages.find((x) => propre(x));
  return vise ? vise.sender : (fil.ligne?.account_email ?? '');   // ← repli sur SOI
}
```

Pour un message **envoyé** isolé (aucune réponse dans le fil — le cas
« Test PJ 3 »), `propre(m)` est vrai mais `fil.messages` ne contient que
ce seul message : `vise` est indéfini, le repli renvoie **la propre
adresse du compte**. D'où « à smonchamps@gmail.com » au lieu de
sebastien.monchamps@gmail.com. Le commentaire du code l'assume déjà
comme « le fait honnête » — le CE tranche que ce n'est plus acceptable.

La **liste** d'« Envoyés » a le même manque : `Liste.svelte:413-415`
affiche toujours `ligne.sender` (donc **soi**) ; dans un dossier
d'envois, la colonne devrait montrer le **destinataire**.

**Racine** : le destinataire n'existe nulle part en base. Le corriger
exige de **stocker le(s) destinataire(s)** de chaque message et
d'afficher, dans un dossier d'envois, le destinataire à la place de
l'expéditeur.

### R1 — « Répondre à tous » sans « À » — MESURE REQUISE

Le chemin lu de bout en bout **paraît correct** :
`reply_all_context` (commands.rs:2272) relit À/Cc en direct sur le
serveur, appelle `reply_all_recipients` (pur, compose.rs:108, testé) et
**garantit un « À » non vide OU lève une erreur** (repli sur
l'expéditeur, commands.rs:2298-2305). Côté UI, une erreur de reply-all
**referme** la fenêtre et affiche un avis (Composition.svelte:153-165).
Le symptôme décrit — *fenêtre ouverte, « À » vide, sans erreur* — n'est
reproductible par aucun chemin du code actuel.

Deux hypothèses, qu'une seule mesure départage :

- **H-a** : l'expéditeur de ces messages n'a **pas d'adresse
  analysable** (`sender_address = None` : `From:` sans `mailbox@host` —
  display-name seul, groupe RFC 5322, `<>` vide, fréquents sur les
  notifications). Alors *« Répondre »* (simple) **échoue aussi** : son
  chemin d'erreur (Composition.svelte:167-170) laisse « À » **vide** en
  gardant la fenêtre ouverte — exactement le symptôme, mais pour la
  réponse simple, pas « à tous ».
- **H-b** : le défaut est propre au chemin « à tous » (relève serveur,
  compte/own résolu de travers) — alors « Répondre » simple, lui,
  **remplit** « À ».

**Mesure discriminante** (une manip du CE, §3) : sur le message fautif,
*« Répondre »* simple remplit-il « À » ? + quel est le `From:` brut de
ce message ? Réponse ⇒ racine.

### R3 — objet en double dans le corps — MESURE REQUISE

Wind **n'injecte jamais** l'objet dans le corps (vérifié : aucune
couture sujet→corps dans l'UI ni dans `mail-render`). Le doublon vient
donc du **corps de l'email lui-même** : le *préheader* (la ligne de
pré-en-tête que les outils d'emailing posent en tête de `<body>`,
souvent une **copie de l'objet**), **normalement masqué**, redevient
**visible** parce que Wind ne peut pas honorer son masquage.

Deux techniques de masquage, deux issues :

- **H-a (forte)** : masquage par **classe CSS dans un `<style>`**
  (`.preheader{display:none}` — le défaut Mailchimp/Sendinblue…).
  `ammonia` **retire les blocs `<style>`** (hors de ses tags par
  défaut, `sanitize.rs:43`) : la classe ne masque plus rien, le
  préheader s'affiche. **Duplicat exact de l'objet en tête de corps.**
- **H-b** : masquage **inline** (`style="display:none"`) : `ammonia`
  **conserve** l'attribut `style` et `clean_style` garde `display:none`
  (sanitize.rs:103-133) — le préheader reste masqué. Alors le doublon a
  une autre cause (couleur du texte égalée par la palette bakée du
  thème, mail-render/src/lib.rs:126 ; ou l'email affiche vraiment son
  objet — hors de notre responsabilité).

**Mesure discriminante** (§3) : le message brut du cas cité — sa tête de
`<body>` dit la technique de masquage, donc la racine.

---

## 2. Périmètre

**Dans le périmètre :**

- **R2** : dé-échappement des `quoted-string` IMAP (objets, noms,
  adresses). Correctif pur, testable, sans schéma.
- **R4** : stockage du destinataire + affichage « au destinataire »
  dans un dossier d'envois (liste et volet de lecture). Migration de
  schéma (invariant §7 : adopter l'ancien) + question de rattrapage.
- **R1, R3** : **instruction d'abord** — la mesure du §3, puis la
  racine, puis le correctif du jour même (méthode /terrain).

**Hors périmètre (refus explicites, §2.6) :**

- Réécrire un vrai parseur CSS pour honorer TOUS les masquages d'email
  (R3) : la couche 3 (CSP de l'iframe) reste le filet ; on traite la
  cause désignée par la mesure, pas la classe entière.
- Un champ « Cc » à la composition (R4) : le manque signalé est
  l'affichage du destinataire, pas la composition multi-champs.
- `to:` dans la recherche : report assumé de PASSATION inchangé, même
  si R4 rend le destinataire indexable — à rouvrir séparément.

---

## 3. Mesures à jouer par le CE (genchi genbutsu, §7.1)

Commandes PowerShell fournies au STOP correspondant. Les deux mesures
R1/R3 ne divulguent rien de sensible au dépôt — elles restent chez le CE.

- **R1** : sur un message où « Répondre à tous » laisse « À » vide,
  cliquer *« Répondre »* (simple) et noter si « À » se remplit ; relever
  l'en-tête `From:` brut du message (menu de débogage / source).
- **R3** : récupérer la **source brute** du message cité et regarder les
  premières lignes de `<body>` (bloc `<style>` + `class` de préheader,
  ou `style="display:none"` inline).

---

## 4. Étapes

Ordre : les causes tenues d'abord (R2 puis R4), R1/R3 après mesure.

- **E1 — R2 : dé-échappement quoted-string.** TDD : test RED sur
  `envelope_from_parts` avec un objet `Test \"Envoyés\"` et un nom
  d'expéditeur échappé ⇒ attendu `Test "Envoyés"`. Helper
  `unescape_imap_quoted` appliqué dans `decode_header` (avant la passe
  RFC 2047) et à `address_literal`. Gate : `cargo test -p mail-imap`.
- **E2 — R4 : stocker le destinataire.** Migration (colonne(s) sur
  `envelopes`), écriture au moment de la synchro d'un dossier d'envois,
  test de rembobinage sur base de fichier (invariant §7). Portée selon
  D1.
- **E3 — R4 : afficher « au destinataire ».** Dans un dossier d'envois,
  liste et volet de lecture montrent le destinataire ; `destinataire()`
  s'appuie sur la donnée stockée, plus sur le repli. Système amendé
  (DC-D2). Gate e2e.
- **E4 — R4 : rattrapage** (selon D2) des envois déjà synchronisés.
- **E5 — R1 (livré)** : `reply_all_context` lit les À/Cc stockés
  (instantané) ; rattrapage étendu à INBOX + Envoyés. Racine mesurée :
  connexion IMAP par clic (>10 s).
- **E6 — R3 (livré)** : `title` ajouté aux `clean_content_tags` de
  `mail-render` (le `<title>` de l'email ne fuit plus). Test RED→GREEN.

Chaque étape : `/code-review high` sur le diff avant le commit final,
puis `/gate` complète (jamais les tests seuls), puis STOP 2 terrain.

---

## 5. Décisions CE (à poser au STOP 1)

- **D1 — R4, portée du stockage du destinataire.** Le minimum affiche
  « à X » ; le complet (À + Cc) débloque en prime le « Répondre à tous »
  **hors ligne** (plus de relève serveur au clic) et pourrait recouper
  R1. Recommandation : **À + Cc**.
- **D2 — R4, rattrapage des envois existants.** Les ~256 k messages déjà
  synchronisés n'ont aucun destinataire en base. Le corriger sur
  l'existant exige une passe de relève ciblée sur le dossier d'envois.
  Recommandation à arbitrer : rattraper les envois, ou n'appliquer que
  sur les nouveaux ?
- **D3 — R1 et R3, mesure d'abord.** Reproduire la racine par la mesure
  du §3 avant d'écrire le correctif (recommandé), plutôt que coder sur
  hypothèse.
- **D4 — séquence.** Traiter R2 + R4 maintenant (causes tenues), R1 + R3
  après réception des mesures — ou tout regrouper.

---

## 5bis. Terrain (1re passe, 2026-08-16) et corrections du jour

Verdict CE au 1er terrain : **R4 OK**. **R2 KO** (antislashs toujours là).
**R1 OK mais > 10 s** pour remplir « À ». **R3** : message brut fourni.

Racines mesurées et corrigées le jour même :

- **R2 — données héritées.** Le correctif ne nettoyait que les
  enveloppes NEUVES ; les objets déjà en base gardaient leurs escapes
  (la synchro incrémentale ne relit pas l'existant). Ajout d'une
  **migration unique** (marqueur `objets-escapes`) qui dé-échappe les
  objets/noms/adresses stockés contenant un `\`. Le contenu stocké est
  déjà RFC 2047-décodé — dé-échapper la valeur équivaut au nouveau
  décodage. `unescape_imap_quoted` déplacé en `mail-core` (partagé
  synchro + migration). FTS non touché (le tokeniseur écarte déjà le `\`).
- **R1 — connexion IMAP par clic.** From mesuré : `Amazon
  <pickup-point@amazon.fr>` (expéditeur unique, adresse valide). Le
  « À » vide venait de la fenêtre de composition ouverte AVANT la fin de
  `reply_all_context`, qui **ouvrait une connexion IMAP authentifiée à
  chaque clic** (~10 s). Correctif : `reply_all_context` lit les À/Cc
  **stockés** (R4) — instantané, hors ligne — et ne retombe sur la
  relève serveur que si le message n'a pas encore ses destinataires.
  Le rattrapage (§E4) est étendu à **INBOX + Envoyés** (budget partagé,
  même portée que la passe de fils) pour que le courrier reçu déjà
  synchronisé gagne aussi le « à tous » instantané.
- **R3 — le `<title>` de l'email fuyait (corrigé).** Captures Gmail vs
  Wind à l'appui : la ligne en trop dans Wind n'était NI le `<h1>` du
  corps (les deux clients l'affichent) NI le préheader (resté masqué),
  mais le texte de `<head><title>…</title>` (ligne 158 du brut,
  identique à l'objet). `ammonia` retire la balise interdite mais
  **déballe son texte** par défaut ; Gmail jette tout le `<head>`.
  Correctif : `mail-render` ajoute `title` aux `clean_content_tags`
  (contenu retiré, tag ET texte, comme `script`/`style`). Le corps
  garde son propre `<h1>`. L'hypothèse « préheader démasqué » du plan
  initial était fausse — le `display:none` inline est bien conservé ;
  seule la capture terrain a désigné la vraie cause (genchi genbutsu).

## 7. Reports assumés

- **Reply-all hors ligne sur l'existant** : le champ « À » est instantané
  dès que le message a ses destinataires en base — immédiat pour le
  courrier neuf, et au fil du rattrapage (INBOX + Envoyés, borné par
  cycle) pour l'ancien. Avant que le rattrapage l'ait couvert, un ancien
  message retombe UNE fois sur la relève serveur (lente). Transitoire,
  converge — pas une dette permanente.
- **D-15** (DETTE.md) : l'affichage « À : X » de la liste est cadré sur
  la catégorie Envoyés ; un envoi vu par navigation de dossier montre
  encore l'expéditeur. Le volet de lecture est correct partout.
- **D-16** (DETTE.md) : la sonde de reliquat du rattrapage n'est pas
  indexée (famille D-8, sondes périodiques hors pompe).
- **Marqueur de convergence `''` ≠ NULL** (documenté au code
  `set_recipients`) : le rattrapage écrit `''` pour « lu, aucun
  destinataire », NULL restant « pas encore lu ». Intentionnel, pas une
  dette — mais à connaître pour toute requête future sur `to_addrs`.

## 6. Verdicts CE

STOP 1 tranché le **2026-08-16**, mot pour mot :

- **D1 — portée du stockage du destinataire** → **« À + Cc (recommandé) »**.
  On stocke À et Cc : corrige R4 et débloque « Répondre à tous » hors
  ligne (recoupe peut-être R1).
- **D2 — rattrapage des envois existants** → **« Rattraper les envois »**.
  Passe de relève ciblée sur le(s) dossier(s) d'envois pour peupler les
  destinataires manquants sur l'existant.
- **D3 — R1/R3, méthode** → **« Mesurer d'abord (recommandé) »**. Le CE
  joue les manips du §3 ; racine prouvée puis correctif le jour même.
- **D4 — séquence** → **« R2+R4 maintenant, R1/R3 après mesure »**. Je
  démarre R2 puis R4 ; le CE rassemble les mesures R1/R3 en parallèle.
