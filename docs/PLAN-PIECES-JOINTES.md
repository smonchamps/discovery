# Plan — Pièces jointes : l'envoi

Commande (2026-08-14) : le composeur sait joindre des fichiers et les
envoyer. Aujourd'hui « Joindre » répond par un toast d'excuse, et les
puces de fichiers affichées en transfert sont une **fiction du
prototype** : elles promettent des pièces qui ne partent jamais. Ce plan
rend le geste réel — et fait tomber la fiction.

Maquettes : `docs/design/maquette-pieces-jointes.html` — **validées le
2026-08-14** (quatre états : le composeur avec pièces, le refus au
plafond, le transfert réel, la reprise d'un brouillon avec pièces).

## 1. L'existant, et pourquoi il ne suffit pas

- **Le composeur** (Composition.svelte) : `joindre()` = toast
  `toast.joindre` (« Sélecteur de fichiers — à venir »). En transfert et
  en réponse, `fichiers` est rempli par `message_attachments` et rendu en
  puces décoratives — **rien de tout cela n'est transmis à l'envoi**. En
  réponse, c'est doublement faux : l'usage du courrier veut qu'une
  réponse ne porte PAS les pièces d'origine.
- **La boîte d'envoi** (outbox.rs, ADR 0003) : le journal ne porte que du
  texte (`body_text`). Les deux règles d'or — jamais d'envoi perdu,
  jamais d'envoi fantôme — devront couvrir les pièces aussi : un crash
  entre le geste et la vidange ne doit perdre aucun octet.
- **`mail-smtp`** : `build_message` produit un message mono-partie
  (`.body(text)`) ; `draft_bytes` (reflet IMAP des brouillons) pareil.
  Le crate `lettre` sait faire du `MultiPart::mixed` + `Attachment` —
  aucune dépendance neuve.
- **Les brouillons** (drafts.rs) : « fermer = conserver » est un contrat
  tenu au caractère près (PLAN-BROUILLONS). Un brouillon qui perdrait ses
  pièces à la fermeture le trahirait — les pièces doivent donc vivre AVEC
  le brouillon, pas avec la session du composeur.
- **La réception** sait déjà tout des pièces (table `attachments`,
  `fetch_attachment` à la demande, puces de Lecture, `human_size`) : le
  vocabulaire visuel et le modèle « métadonnées gratuites, octets à la
  demande » existent — on les réutilise, on ne les réinvente pas.
- **Aucun sélecteur de fichiers** dans le shell : ni
  `tauri-plugin-dialog`, ni capacité associée.

## 2. Les décisions (PJ-D1 à PJ-D7)

- **PJ-D1 — l'ancre des pièces : le brouillon local.** Joindre un
  fichier copie ses octets dans une table neuve `draft_attachments`
  (id, draft_id, name, mime, size, bytes BLOB), en créant le brouillon
  s'il n'existe pas encore. Jamais de chemin nu en base : un fichier
  déplacé ou supprimé après le geste ne peut plus rien casser — les
  octets sont à nous dès le clic. Corollaire : `vide()` compte les
  pièces (un brouillon sans texte mais avec pièce n'est pas vide), et la
  reprise restitue les puces.
- **PJ-D2 — « jamais d'envoi perdu » couvre les pièces** : `queue_send`
  copie `draft_attachments` → `outbox_attachments` dans la MÊME
  transaction que l'insertion outbox, avant toute tentative réseau. La
  vidange lit le journal, rien d'autre. La suppression du brouillon
  après envoi (chemin existant) emporte ses blobs par `ON DELETE
  CASCADE`.
- **PJ-D3 — plafond de 25 Mo par message** (total des tailles décodées —
  la limite Gmail, la plus répandue), refusé **au geste**, pas à
  l'envoi : la puce n'apparaît jamais, le composeur dit pourquoi.
  L'encodage base64 (+33 %) peut encore heurter un serveur plus strict :
  ce refus-là est un 5xx classé `Permanent` — la fente d'avis existante
  (échec d'envoi) le porte déjà, rien de neuf à construire.
- **PJ-D4 — le transfert devient réel, la réponse devient honnête.** En
  transfert, les pièces du message d'origine sont **rapatriées à
  l'ouverture** (`fetch_attachment`, comme la Lecture) et versées dans
  `draft_attachments` : puce en « rapatriement… » puis nom + taille ;
  un échec marque la puce (`--alert`) avec « Réessayer » — jamais de
  pièce silencieusement absente du message parti. En réponse, les puces
  du prototype **disparaissent** (la fiction tombe ; l'usage du courrier
  ne les a jamais transmises).
- **PJ-D5 — le sélecteur : `tauri-plugin-dialog`**, invoqué côté UI
  (API JS du plugin, sélection multiple), les chemins passés à une
  commande `attach_files` qui lit et copie côté Rust. Les octets ne
  traversent jamais l'IPC en base64 ; la boîte de dialogue native reste
  du ressort du shell. Capacité à déclarer dans `capabilities/`.
- **PJ-D6 — le reflet IMAP des brouillons suit** : `draft_bytes` devient
  multipart quand le brouillon porte des pièces (même constructeur MIME
  que l'envoi). Le WHERE anti-churn de `save_draft` n'est pas concerné
  (les pièces changent par `attach_files`/`detach_file`, qui marquent le
  brouillon modifié eux-mêmes). Le tirage des pièces de brouillons nés
  ailleurs reste HORS périmètre (v1 : le texte seul, comme aujourd'hui).
- **PJ-D7 — la purge** : au passage à `sent`, les blobs
  d'`outbox_attachments` sont vidés (métadonnées gardées — le journal
  reste lisible) ; `interrupted` et `rejected` les GARDENT : la reprise
  sur décision de l'utilisateur doit pouvoir renvoyer le message entier.

## 3. Le contrat visuel (maquette, à valider)

- **La puce de pièce du composeur** : gabarit `puce` existant (32 px,
  6 px de rayon), glyphe `description`, **nom + taille dans la même
  puce** (la Lecture les sépare — ici la puce est un objet manipulable,
  pas deux lectures), et un retrait `close` (13 px) au bord droit.
  Survol du retrait : fond `--sel`. Une puce par fichier, rangée
  `fichiers` existante (repli en lignes).
- **Le poids total** en fin de rangée, 12,5 px `--muted`
  (« 3,2 Mo / 25 Mo ») — visible dès la première pièce, jamais avant.
- **Le refus au plafond** : rien ne s'ajoute ; un message 13 px
  `--alert` sous la rangée dit le fichier refusé et la place restante.
  Il s'efface à la pièce suivante acceptée ou au retrait d'une puce.
- **Le transfert** : mêmes puces, trois états — rapatriement (glyphe
  `hourglass_empty` + nom, atténué), arrivée (puce pleine, retirable),
  échec (nom en `--alert` + « Réessayer »). `--alert` en texte 13 px sur
  `--surface` : paires déjà au banc `contraste.mjs` (E2
  PLAN-BROUILLONS) — pas de couleur neuve, pas de re-mesure.
- **La reprise d'un brouillon** : les puces reviennent avec le texte,
  identiques au moment de la fermeture. Le dossier Brouillons et la
  mention en Réception ne changent PAS en v1 (pas de trombone en
  liste — extension possible si le terrain la réclame).
- Glyphes `attach_file`, `description`, `close` déjà dans la police
  (pied du composeur, puces de Lecture) ; `hourglass_empty` à vérifier
  au build de la police — sinon régénération (mécanique existante).

## 4. Travaux — cœur (`mail-core`)

1. **Schéma** : `draft_attachments` (id, draft_id → drafts ON DELETE
   CASCADE, name, mime, size, bytes) et `outbox_attachments` (id,
   outbox_id → outbox ON DELETE CASCADE, name, mime, size, bytes) dans
   `SCHEMA` + `migrate()`. Tests : cascade des deux côtés.
2. **`Store`** : `add_draft_attachment` (refus au-delà du plafond —
   l'erreur dit le poids restant), `remove_draft_attachment`,
   `draft_attachments_meta` (métadonnées seules — la liste des puces ne
   charge jamais les blobs), lecture des blobs à la construction MIME
   seulement.
3. **`compose::Draft` + `enqueue_outbox`** : le geste `queue_send` copie
   les pièces du brouillon vers `outbox_attachments` dans la transaction
   d'insertion (PJ-D2). `OutboxMessage` gagne
   `attachments: Vec<OutboxAttachment>` (name, mime, bytes) chargées à
   la vidange. Tests : crash simulé entre geste et vidange — les pièces
   survivent ; purge à `sent` ; quarantaine les garde (PJ-D7).
4. **Plafond** : constante `MAX_ATTACHMENTS_BYTES = 25 * 1024 * 1024`,
   testée aux bornes (24,9 accepté ; le fichier qui franchit refusé —
   les précédents restent).

## 5. Travaux — shell (`apps/desktop`)

- **`tauri-plugin-dialog`** au Cargo.toml + enregistrement main.rs +
  capacité `dialog:allow-open` dans `capabilities/`.
- **Commandes neuves** : `attach_files(accountId, draftId|null, paths)`
  → lit chaque fichier, refuse au plafond, rend `{draft_id, pieces,
  refuses}` (le brouillon est créé au premier fichier si besoin, PJ-D1) ;
  `detach_file(draftId, attachmentId)` ; `draft_attachments(draftId)`
  (métadonnées) ; `fetch_source_attachment(accountId, mailbox, uid,
  index, draftId)` — le rapatriement du transfert (PJ-D4), qui verse
  directement dans `draft_attachments`.
- **`queue_send`** gagne `draftId` (nullable — un envoi sans jamais
  avoir sauvé reste possible tant qu'aucune pièce n'est jointe).
- **`sync_drafts`** : `draft_bytes` multipart quand il y a des pièces
  (PJ-D6), même constructeur que `build_message` (factorisé dans
  `mail-smtp`, testé sur les octets produits : frontières MIME,
  Content-Disposition, noms non-ASCII encodés RFC 2231).

## 6. Travaux — UI (`ui-v2`)

1. **Composition.svelte** : `joindre()` ouvre le sélecteur (multiple),
   passe les chemins à `attach_files`, adopte le `draft_id` rendu ;
   `fichiers` devient l'état réel (métadonnées de `draft_attachments`) ;
   retrait par puce (`detach_file`) ; `vide()` compte les pièces ;
   `ouvrirBrouillon` recharge les puces ; `envoyer` passe `draftId` et
   ne supprime plus le brouillon lui-même côté UI qu'après le geste
   (chemin existant — les blobs suivent par cascade). En transfert :
   rapatriement par pièce avec états (PJ-D4) ; en réponse : plus de
   puces. Chaque geste rapporte `onbrouillon()` (la liste se ressonde,
   mécanique PLAN-BROUILLONS).
2. **Catalogues fr/en** : neuves — `compo.retirerPiece` (« Retirer
   {nom} »), `compo.poidsTotal` (« {poids} / 25 Mo »),
   `compo.pieceRefusee` (« {nom} dépasse la place restante ({reste}) »),
   `compo.rapatriement` (« Rapatriement… »), `action.reessayer`
   (« Réessayer »), `erreur.rapatriement` ; morte — `toast.joindre`.
3. **Aucune géométrie neuve** : rangée `fichiers`, gabarit `puce`,
   `--alert` texte — tout existe.

## 7. e2e et gates

- La boîte de dialogue native n'est **pas pilotable** par la suite
  Playwright/Tauri : le parcours e2e passe par la couture `attach_files`
  (chemins de fixtures injectés par le transport, sélecteur non ouvert) —
  le plugin lui-même reste couvert par le terrain.
- Parcours neufs (`refonte-ecran02` ou suite dédiée) : joindre deux
  fixtures → deux puces + poids total ; retirer une → une ; fermer →
  reprise au dossier Brouillons avec sa puce ; envoyer → le message en
  boîte d'envoi porte la pièce (assertion sur le journal) ; fixture
  au-delà du plafond → refus dit, rien de joint.
- Rust : MIME multipart (frontières, noms RFC 2231), plafond aux bornes,
  transaction geste→journal, cascade, purge/quarantaine.
- Gates : `cargo test` (workspace), clippy muet, suite e2e complète,
  banc P1 non concerné (la requête chaude ne bouge pas) ;
  `contraste.mjs` sans paire neuve (§3).

## 8. Ordre de livraison

- **E1 — le cœur** *(livré le 2026-08-14)* : schéma, plafond
  (`MAX_ATTACHMENTS_BYTES`, refus au geste avec place restante), gestes
  `add/remove_draft_attachment` + `draft_attachments_meta` (blobs jamais
  chargés en liste), copie geste→journal (`enqueue_outbox_from_draft`,
  même transaction), purge des octets à `sent` (métadonnées gardées,
  quarantaine et refus entiers), MIME multipart (`mail-smtp` — base64,
  noms RFC 2231, repli octet-stream, refus franc sur journal purgé).
  Rien ne change à l'écran, tout est testé — gates : 400 Rust (20
  neufs : `tests_pieces` × 2 + fil SMTP), clippy muet.
- **E2 — le composeur** *(livré le 2026-08-14)* : plugin dialog +
  capacité `dialog:allow-open`, commandes `attach_files` (brouillon-ancre
  au premier fichier, repris si tout est refusé) / `detach_file` /
  `draft_attachments`, `queue_send` porte `draftId`, puces réelles
  (nom + taille + retrait), poids total, refus au plafond sous la rangée,
  `vide()` compte les pièces, reprise avec puces, epoch des gestes
  adopté (pas de fork fantôme), `OutboxEntry.pieces`, catalogues
  (`toast.joindre` morte), e2e par la couture `window.__e2ePieces`
  (4 parcours). Écart dit : le refus est sans glyphe (`warning` absent
  de la police — régénération groupée à E3 avec `hourglass_empty`).
  C'est le commit qui tient la commande : « Joindre » joint, l'envoi
  emporte. Au passage, régression rattrapée : la conf expédiée pointait
  de nouveau sur la v1 dormante (`frontendDist: "ui"`, échange de banc
  resté sale au commit 4dbf06f) — invisible de la CI, la gate e2e vivant
  hors CI hébergée (ADR 0005).
- **E3 — le transfert réel et la réponse honnête** *(livré le
  2026-08-14)* : rapatriement par pièce (`fetch_source_attachment`,
  séquentiel — la première pièce crée l'ancre) avec les trois états de
  la maquette ; l'envoi est GARDÉ tant que des pièces manquent, et la
  croix d'une puce en échec est le renoncement explicite qui le libère
  (jamais d'absence silencieuse) ; la fiction des puces en réponse est
  tombée (e2e ajusté : l'absence est affirmée) ; reflet IMAP multipart
  (`draft_bytes` + `sync_drafts`, constructeur de partie commun
  `file_part`, PJ-D6) ; police régénérée **37 → 39 glyphes**
  (`hourglass_empty`, `warning` — A17, inventaire tenu, preuve rejouée
  40/40, copie `public/` faite) — le refus au plafond retrouve son
  glyphe. Parcours e2e neuf : transfert hors ligne → échec dit,
  « Réessayer », envoi bloqué puis libéré au renoncement.
  Gates : 403 Rust, clippy muet, 79 e2e. **Le plan est soldé** — reste
  la validation terrain du CE (envoi réel avec pièces, transfert en
  ligne, reflet Gmail multipart).

Hors périmètre (dit pour ne pas y glisser) : le glisser-déposer sur le
composeur, le trombone en liste (dossier Brouillons, Réception), le
tirage des pièces de brouillons nés ailleurs, l'aperçu des pièces avant
envoi.
