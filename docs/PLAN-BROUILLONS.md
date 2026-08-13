# Plan — Brouillons : le dossier et la mention en liste, plus de bandeau

Commande du Chef Ingénieur (2026-08-13) : l'interface porte aujourd'hui
DEUX surfaces pour les brouillons — le dossier « Brouillons » et la
fente d'avis (« Un brouillon en cours : … », Reprendre / Plus tard).
On ne conserve que deux usages :

1. **L'accès direct au dossier Brouillons** — la liste des brouillons,
   et la reprise au clic.
2. **Le brouillon-réponse visible dans la conversation** : quand un
   brouillon répond à un message, la ligne de sa conversation dans le
   volet central le montre **comme s'il était le dernier email du fil**,
   avec une mention « Brouillon ».

La fente d'avis cesse de porter les brouillons (sa source n° 5
disparaît ; échec d'envoi, mise à jour, crash, télémétrie restent).

Maquettes : `docs/design/maquette-brouillons.html` — **validées le
2026-08-13** (§1 variante B retenue ; §2, §3, §4 tels quels) ; le
contrat visuel sera inscrit au journal du Système (amendement à
prendre à la suite).

## 1. L'existant, et pourquoi il ne suffit pas

- **La fente d'avis** (App.svelte : `verifierBrouillons`,
  `avisBrouillons`, sondée toutes les 10 s) est aujourd'hui la SEULE
  voie de reprise d'un brouillon. La retirer sans rien d'autre
  casserait le filet — d'où les deux chantiers ci-dessous.
- **Le dossier Brouillons** (catégorie `brouillons` de `list_category`)
  montre le dossier IMAP canonique — les copies *serveur*. Cliquer une
  ligne ouvre la **lecture**, pas la reprise : ce n'est pas l'usage 1.
  Et un brouillon jamais poussé (hors ligne) n'y apparaît pas.
- **Les brouillons locaux** (table `drafts`, `list_drafts`) portent
  `reply_to_uid` mais **pas la boîte** du message visé : impossible
  aujourd'hui de relier un brouillon à son fil (les UID repartent de 1
  à chaque boîte — ADR 0009).
- **La liste Réception** est servie par l'agrégat matérialisé `threads`
  (une ligne par conversation, gate P1) : toute jointure ajoutée à la
  requête chaude se paie au défilement.

## 2. Les décisions (B-D1 à B-D5)

- **B-D1 — la source du dossier Brouillons : la table locale `drafts`**
  (plus le dossier IMAP). C'est elle qui permet la reprise au clic ;
  le tirage (Phase 3) y rapatrie déjà les brouillons nés ailleurs ;
  les brouillons hors ligne y sont ; et le compteur de nav s'aligne
  sur ce que le dossier montre. Le dossier IMAP reste ce qu'il est :
  le miroir de synchro, plus une surface d'affichage.
- **B-D2 — le lien brouillon → fil : colonne neuve `reply_to_mailbox`**
  posée par le composeur à la sauvegarde. Le `thread_id` se résout à la
  lecture (`list_drafts`), par jointure `mailboxes` + `envelopes` — les
  brouillons sont peu nombreux, c'est gratuit ; la requête chaude
  `SELECT_UNIFIED` n'est **pas touchée** (gate P1 sanctuarisée). Les
  brouillons antérieurs à la migration restent sans mention : le
  dossier est leur filet — honnête, et sans risque.
- **B-D3 — pas de re-tri de la Réception (v1)** : la ligne garde la
  place de son dernier *vrai* message ; seul son **aperçu** change
  (variante B validée — première ligne intacte). Re-trier exigerait d'injecter
  les brouillons dans l'agrégat `threads` — coût et risque P1 pour un
  gain incertain. Extension possible plus tard si le terrain la
  réclame.
- **B-D4 — le clic sur une ligne « Brouillon » en Réception est
  inchangé** (sélection + volet de lecture). La reprise vit : (a) au
  dossier Brouillons, (b) dans la conversation (écran 03), où le
  brouillon apparaît en dernière position avec la même mention — le
  clic dessus rouvre le composeur. Sans (b), la liste promettrait un
  « dernier email » que la conversation n'aurait pas.
- **B-D5 — plusieurs brouillons sur un même fil** : le plus récent
  porte la mention en liste (aperçu et heure sont les siens) ; le
  dossier les montre tous. La suppression depuis le dossier reste hors
  périmètre — le chemin existant demeure (ouvrir, vider, fermer = jeter ;
  `delete_draft` du composeur).

## 3. Le contrat visuel (validé le 2026-08-13)

- **Ligne de conversation avec brouillon** (volet central) — **variante
  B retenue** : la première ligne ne bouge pas (expéditeur du fil,
  heure du dernier *vrai* message) ; l'aperçu devient le préfixe
  « Brouillon — » (13 px, 600, `--alert`) suivi du **corps du
  brouillon**. L'état non-lu reste celui du fil. La variante A
  (mention en première ligne, heure du brouillon) est écartée —
  conservée en maquette pour mémoire.
- **Ligne du dossier Brouillons** : même gabarit ; l'expéditeur devient
  le destinataire (« À : marie@… », ou « (sans destinataire) » en
  atténué), objet « (sans objet) » si vide, aperçu = corps, heure =
  dernière édition. Clic = reprise.
- **Conversation (écran 03)** : bloc replié final au trait accent
  **pointillé** — mention `✎ Brouillon`, aperçu, heure, « Reprendre »
  au survol. Clic = composeur par-dessus (la conversation reste montée).
- `--alert` en texte 13 px sur les TROIS fonds de rangée (repos,
  survol, choisie) : mesuré à E2 — seul le thème nuit échouait
  (3,58:1 sur `--sel`) ; son `--alert` est éclairci `#d9776b` →
  `#ea9a90` (même teinte, remède A8), et les deux paires neuves sont
  au banc `contraste.mjs`. Les 7 thèmes passent.
- Glyphe `edit_note` déjà dans la police (nav) : pas de régénération.

## 4. Travaux — cœur (`mail-core`)

1. **Migration** : `add_missing_columns(conn, "drafts",
   [("reply_to_mailbox", "TEXT")])` dans `migrate()` (même mécanique que
   `account_id`). `DraftContent` et `SavedDraft` gagnent le champ ;
   `DRAFT_SELECT`, insert/update et le WHERE anti-churn l'incluent.
2. **`Store::drafts()` résout le fil** : LEFT JOIN
   `mailboxes (account_id, name)` puis `envelopes (mailbox_id, uid)` →
   `thread_id` (`None` si non-réponse, boîte inconnue ou message
   disparu). Tests : réponse résolue, non-réponse, boîte disparue,
   brouillon d'avant migration.
3. **`nav_counts`** : le compteur `brouillons` compte la table `drafts`
   du compte (B-D1), plus le dossier IMAP. *Déplacé en E2 à
   l'implémentation (2026-08-13)* : tant que le dossier affiche encore
   la boîte IMAP et que la fente vit, un compteur local dirait N quand
   le dossier montre M — et le seed e2e (2 messages IMAP, aucun
   brouillon local) casserait la gate pour une incohérence qu'E2
   dissout de toute façon. Compteur, dossier et seed basculent
   ENSEMBLE.

## 5. Travaux — shell (`commands.rs`)

- `save_draft` : `content.replyToMailbox` transmis au cœur.
- `list_drafts` : `DraftRow` + `reply_to_mailbox` + `thread_id`.
- `list_category` ne sert plus la catégorie `brouillons` (la page vient
  de `list_drafts` côté UI) — la résolution du dossier IMAP canonique
  reste pour la synchro (poussée/tirage, STATUS).

## 6. Travaux — UI (`ui-v2`)

1. **App.svelte** : `avisBrouillons`, `verifierBrouillons`,
   `brouillonsIgnores` supprimés. À la place, la même sonde de 10 s
   alimente un état `brouillons` (rows de `list_drafts`) passé à
   `Liste` et `Conversation` ; re-sondé après `sync_drafts` et au
   retour du composeur (callback `onbrouillon` de Composition :
   sauvegarde, suppression, envoi — la liste ne traîne pas 10 s en
   retard sur un geste local).
2. **Liste.svelte** :
   - *Décor Réception* : index `thread_id → brouillon le plus récent`
     dérivé des rows ; quand le fil en porte un, l'aperçu de la ligne
     rend le préfixe « Brouillon — » suivi du corps du brouillon
     (variante B — première ligne et heure intactes). Jamais sur les
     résultats de recherche (un résultat est un message, pas une
     conversation).
   - *Catégorie `brouillons`* : rendue par le chemin **non fenêtré**
     (celui des résultats de recherche) depuis l'état `brouillons`,
     filtré par compte si la nav borne ; total rapporté à la barre de
     statut. Clé de ligne = id du brouillon ; clic → `onreprendre`
     (JAMAIS `mark_seen`).
3. **Conversation.svelte** : si le fil ouvert porte un brouillon, bloc
   final « Brouillon » ; clic → `onreprendre`. Le composeur se
   superpose, la conversation reste montée dessous.
4. **Composition.svelte** : `ouvrirBrouillon` restaure aussi
   `replyToMailbox` (aujourd'hui perdu à la reprise — la chaîne
   réponse → brouillon → reprise → sauvegarde ne doit pas perdre le
   lien au fil).
5. **Catalogues fr/en** : neuves — `liste.brouillon` (« Brouillon »),
   `brouillons.a` (« À : {a}»), `brouillons.sansDestinataire` ;
   mortes — `avis.brouillon`, `avis.brouillons`. `compo.sansObjet` et
   `action.reprendre` restent (réutilisées). `statut.phase.brouillons`
   reste (phase de synchro, sans rapport).

## 7. e2e et gates

- `refonte-ecran02` : le parcours P11 (« la fente d'avis porte le
  brouillon… ») est remplacé par : brouillon-réponse enregistré → la
  ligne du fil porte la mention en Réception ; le dossier Brouillons le
  montre ; le clic y restitue objet/corps intacts ; la conversation
  montre le bloc final. Le compteur de nav (ligne 30) suit la nouvelle
  source — seed à ajuster si besoin.
- `refonte-parcours-v1` : « Échap conserve, Reprendre restitue intact »
  rejoué par le dossier Brouillons (plus de `fente-avis`).
- Gates : `cargo test` (workspace), suite e2e complète, banc P1
  inchangé (la requête chaude n'a pas bougé — à re-mesurer quand même),
  `contraste.mjs` pour la mention `--alert` sur les 7 thèmes.

## 8. Ordre de livraison

- **E1 — le socle** *(livré le 2026-08-13)* : migration +
  `reply_to_mailbox` bout en bout (cœur, shell, composeur), `drafts()`
  résout le fil. Rien ne change à l'écran, tout est testé — gates :
  379 Rust (5 neufs, `tests_fil`), clippy muet, 73 e2e.
- **E2 — les surfaces** *(livré le 2026-08-13)* : dossier Brouillons
  servi en local + compteur de nav aligné (§4.3) + reprise au clic +
  mention en Réception + suppression de la source de la fente +
  catalogues + seed et e2e réécrits + `--alert` nuit éclairci (§3).
  C'est le commit qui tient la commande : la fente n'est tombée
  qu'avec la reprise au dossier vivante. Gates : 380 Rust, clippy
  muet, 73 e2e, contraste 7 thèmes.
- **E3 — la conversation** *(livré le 2026-08-13)* : le bloc final en
  écran 03 (B-D4-b) — pointillé accent, mention ✎, corps, heure,
  « Reprendre » ; clic = composeur par-dessus, conversation montée
  dessous. Gates : 380 Rust, 74 e2e. **Le plan est soldé.**
