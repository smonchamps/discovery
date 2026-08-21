# PLAN-RETOURS-7 — survol des pièces, pièces en tête, écran 03 à plat, épingles

> Chantier ouvert le 2026-08-21 (`/chantier`), sur quatre retours CE :
> (1) amélioration — au survol d'une pièce jointe, icône et texte
> « télécharger » pour que l'UX dise l'action à venir ; (2)
> amélioration — les pièces jointes en haut du mail plutôt qu'en bas ;
> (3) amélioration — la conversation ouverte (« Ouvrir ») à la même
> forme que le volet de visualisation : chaque email dans son
> élévation, la conversation elle-même sans élévation ; (4) feature —
> épingler un email pour qu'il apparaisse toujours en haut de la liste
> du volet central.

---

## Constat — faits vérifiés sur pièces (2026-08-21)

### 1. Le survol d'une pièce jointe (R1)

- Les pièces d'un message en lecture sont rendues à UN seul endroit :
  `Fil.svelte` (bloc `.fichiers`, `data-testid="lecture-fichiers"`),
  composant partagé par le volet ET l'écran 03 — une correction couvre
  les deux cadres.
- Le survol actuel se limite à un fond `--sel`
  (`.puce.bouton:hover`). L'infobulle `title` dit « Enregistrer… »
  (`lecture.enregistrer`), le toast « Pièce enregistrée » ; le clic
  ouvre le dialogue natif « Enregistrer sous » (PLAN-RETOURS-4).
  **Trois vocabulaires coexisteraient** si « télécharger » entre sans
  arbitrage — c'est la décision D1.
- Les puces d'un **écho d'envoi** sont **inertes** (`disabled`,
  RETOURS-5 D2, e2e `toBeDisabled()` à l'appui) : elles ne doivent
  RIEN promettre au survol.
- **Aucun glyphe « télécharger » dans le sous-ensemble** (61 glyphes,
  inventaire contractuel `assets/icones/README.md`). L'ajouter =
  régénérer le woff2, recopier dans `public/icones/`, passer `?v=64`
  (convention : v = nombre de glyphes), amender l'inventaire, rejouer
  la preuve `apercu.html` — procédure rodée (58 → 61 au chantier
  précédent).
- Piège d'exécution : la puce est `white-space:nowrap` — un libellé
  qui APPARAÎT au survol élargirait la puce et ferait refluer la
  rangée. La forme retenue (O1) garde la largeur stable. Et tout état
  `:hover` doit avoir son jumeau `:focus-visible` (clavier, A8).

### 2. Les pièces jointes en haut du mail (R2)

- Ordre DOM actuel d'une carte dépliée : tête du message →
  `garde-images` (si images distantes bloquées) → `iframe` du corps →
  bloc « Fichiers joints » → barre de réponse. Le bloc des pièces est
  bien **après** le corps.
- Le déplacer avant est un déplacement dans le MÊME conteneur flex
  (`.contenu`, gap 12 px) — aucun CSS nouveau. Ordre cible : tête →
  **fichiers** → garde-images → corps (la garde d'images reste collée
  au corps qu'elle concerne).
- Aucun test ne verrouille la position actuelle — R2 passera sans
  casse, mais il n'existe AUCUN test d'ordre : il en faut un neuf
  (comparaison d'ordre DOM pièces/corps).

### 3. L'écran 03 et son élévation (R3)

- Le Système dit déjà la forme demandée… pour le volet : **« À PLAT »
  (A46)** — aucune élévation englobante, le panneau défile en un seul
  flot, seules les cartes de message s'élèvent. Et il acte l'inverse
  pour l'agrandissement : « l'écran 03 garde sa carte pleine » (A46).
  Le retour renverse cette demi-phrase — le reste d'A46 s'étend tel
  quel.
- Concrètement, l'écran 03 diffère du volet par DEUX choses :
  le wrapper `.carte` de `Conversation.svelte` (surface + bordure +
  `box-shadow:var(--shadow)`, défilement interne) et l'absence de la
  classe `volet` sur le composant `Fil` (qui réactive filet de tête,
  barre d'actions filetée et scroll interne). Volet et écran 03 sont
  « deux cadres du même objet » (composant Fil, état partagé) — la
  correction est locale à ces deux points.
- Garde existante : le volet a un e2e de « platitude »
  (`refonte-ecran02.spec.js`, terrain A46) ; l'écran 03 n'en a pas —
  il lui en faut un jumeau.

### 4. Épingler un email (R4)

- **Aucune épingle n'existe.** Le voisin `envelopes.flagged` (étoile
  IMAP) est complet côté cœur mais mort côté UI — et il est **écrasé
  par la vérité serveur** à chaque synchro (`upsert_envelopes`) : une
  épingle locale ne peut PAS le réutiliser. L'épingle sera une donnée
  **locale** (le serveur n'a pas ce concept ; détourner `\Flagged`
  serait mentir aux autres clients).
- Le tri de la liste vit côté SQL à **4 endroits** (boîte unifiée,
  page de catégorie sans/avec échos, re-tris externes), gardé par un
  test de plan (`la_boite_unifiee_ne_materialise_pas_son_tri`,
  interdit `TEMP B-TREE FOR ORDER BY`) et l'index partiel
  `idx_threads_date_globale`. Mettre l'épingle DANS l'ORDER BY
  paginé menacerait cette garde — l'option retenue (O2) sert les
  épingles **à part**, en tête de page 0, sans toucher l'ORDER BY.
- La table `threads` est **reconstruite à l'adoption** (DROP) : une
  épingle portée par `threads` mourrait à la prochaine migration de
  fils. La clé stable est l'enveloppe `(mailbox_id, uid)` — table
  dédiée `pins`, additive, au patron `add_missing_columns`/CREATE
  IF NOT EXISTS.
- Les lignes de la liste n'ont **aucun geste** (avatar « visuel
  seul », pas de menu contextuel dans tout le produit) ; les gestes
  d'un message vivent dans le FIL (barre de tri A58 : Archiver,
  Supprimer, Spam) — c'est la place naturelle d'« Épingler » (D3).
- La hauteur des lignes est **sondée en deux gabarits** (h1/h2, avec
  ou sans rang de puces) : la marque d'épingle doit passer par le rang
  de puces existant, sinon le fenêtrage dérive.
- Les **échos de nav** (mailbox `echo:<id>`) meurent à la
  réconciliation — épingler un écho créerait une orpheline : échos non
  épinglables (refus). Les brouillons vivent hors `envelopes`
  (`list_drafts`) : non épinglables (refus).
- Pas de glyphe « épingle » dans le sous-ensemble : `keep` (et son
  état `keep_off` pour « Désépingler ») rejoignent la régénération de
  R1 — une seule passe 61 → 64.

## Périmètre

**Dans ce chantier** : survol descriptif des puces de pièces en
lecture (R1, selon D1) ; pièces jointes remontées entre la tête du
message et le corps (R2) ; écran 03 à plat, au dessin exact du volet
(R3, selon D2) ; épingles locales de bout en bout (R4, selon D3-D5) :
table `pins`, commande de bascule, service en tête de liste, marque
sur la ligne, geste dans la barre du fil ; 2 à 3 glyphes neufs en une
régénération ; e2e de chaque parcours ; gate complète ; terrain.

**Refus de périmètre explicites (STANDARD §2.6) :**
- **Pas de menu contextuel** sur les lignes de la liste : surface
  neuve, grammaire absente du produit — l'épingle passe par la barre
  du fil.
- **Pas de synchro serveur des épingles** : donnée locale ; `\Flagged`
  (l'étoile) est une autre sémantique, on ne la détourne pas. L'étoile
  UI reste un chantier à part si le terrain la demande.
- **Échos d'envoi et brouillons non épinglables** (l'écho meurt à la
  réconciliation ; le brouillon vit hors enveloppes).
- **La recherche ignore les épingles** : ses résultats gardent leur
  tri (pertinence/date).
- **Les puces d'écho restent inertes** au survol (aucune promesse de
  téléchargement, RETOURS-5 D2).
- **Pas de maquette d'étude** : les quatre surfaces suivent la
  grammaire normée (puces, cartes, barre du fil, rang de puces de la
  ligne) — aucun écran neuf.

## Options et verdicts

### O1 — La forme du survol (R1)

| Option | Mécanisme | Verdict |
|---|---|---|
| **A. Voile de puce** | Au `:hover`/`:focus-visible`, un voile couvre la puce (position absolute, même géométrie) : glyphe `download` + libellé D1, centrés ; la largeur ne bouge pas | **Retenue.** Largeur stable (pas de reflux de la rangée), l'action dite en toutes lettres, le nom reste lisible hors survol |
| B. Libellé apposé au survol | La puce s'élargit pour montrer icône + texte | Rejetée : reflux de toute la rangée au survol — la surface fuit sous le curseur |
| C. Infobulle seule | `title` natif enrichi | Rejetée : c'est l'existant (« Enregistrer… »), le retour dit justement qu'il ne suffit pas |

### O2 — Où vivent et comment se servent les épingles (R4)

| Option | Mécanisme | Verdict |
|---|---|---|
| **A. Table `pins(mailbox_id, uid, epoch)` + service à part** | Additive, clé enveloppe (survit à la reconstruction des fils) ; les épingles de la vue sont servies par une requête dédiée (jointure `envelopes`, quelques lignes) et PRÉPOSÉES à la page 0 ; le flot paginé les exclut par anti-jointure `NOT EXISTS` sur une table minuscule indexée | **Retenue.** L'ORDER BY paginé ne bouge pas (garde de plan intacte), l'index `idx_threads_date_globale` ne bouge pas, le coût est celui d'une anti-jointure sur ~quelques épingles |
| B. Colonne `threads.pinned` dans l'ORDER BY | `ORDER BY pinned DESC, last_epoch DESC…` | Rejetée : perdue au DROP d'adoption des fils, index partiel à doubler, test de plan menacé, et ne couvre pas les catégories hors Réception |
| C. Réutiliser `envelopes.flagged` | Rien à créer | Rejetée : écrasée par la vérité serveur à chaque synchro, et sémantique étoile ≠ épingle |

### O3 — L'ordre entre épingles

Date du message décroissante — le même ordre que la liste, pas
« dernier épinglé premier » : la section épinglée se lit comme la
liste. (Choix d'exécution, pas d'arbitrage : aucun coût de
réversibilité.)

## Étapes

- **E1 — glyphes** : `download`, `keep`, `keep_off` au sous-ensemble
  (61 → 64), `?v=64`, README amendé, preuve `apercu.html` rejouée.
- **E2 — R2, pièces en tête** : déplacement du bloc `.fichiers` avant
  la garde d'images et le corps ; e2e neuf d'ordre DOM
  (pièces AVANT corps) ; Système amendé (A-n, même commit).
- **E3 — R1, survol descriptif** (selon D1) : voile O1 sur
  `.puce.bouton` seule (jamais sur une puce inerte d'écho — e2e),
  `:hover` + `:focus-visible`, infobulle et toast alignés sur D1 dans
  les DEUX catalogues ; e2e : le voile apparaît au survol, dit D1,
  absent sur l'écho ; Système amendé (A-n).
- **E4 — R3, écran 03 à plat** (selon D2) : retrait du wrapper
  `.carte` (`Conversation.svelte`), scène sur `--bg`, le Fil en forme
  volet (un seul flot qui défile, tête sans filet, cartes seules
  élevées), largeur selon D2 ; e2e « platitude » jumeau de celui du
  volet ; le commentaire de code et le Système renversent « l'écran 03
  garde sa carte pleine » (A-n).
- **E5 — R4, épingles, cœur (TDD)** : RED — table `pins`,
  `toggle_pin` ; `pinned_rows(category, account_id)` sert les épingles
  de la portée (jointure enveloppes vivantes, ordre O3) ; les pages
  excluent les épinglés selon D5 ; l'épingle survit à une
  reconstruction des fils ; une enveloppe expurgée n'est plus servie.
  Garde de plan rejouée (aucun `TEMP B-TREE` nouveau).
- **E6 — R4, épingles, UI** : bouton « Épingler »/« Désépingler »
  (`keep`/`keep_off`) dans la barre du fil selon D3 ; section épinglée
  en tête de liste (page 0, au-dessus du flot, mêmes gabarits de
  ligne, marque `keep` dans le rang de puces existant) ; bascule →
  `recharger()` (stale-while-revalidate existant) ; e2e : épingler →
  la ligne est en tête, l'état survit au redémarrage (couture),
  désépingler la rend à sa place ; Système amendé (A-n).
- **E7 — qualité et sortie** : revue à regard neuf
  (`/code-review high`), gate complète, **terrain (STOP 2)** avec
  commandes PowerShell prêtes, docs (journal A-n, ETAT, DETTE si
  report), CHANGELOG **avant** release (§2.9 ⚠️), version selon D6 —
  capacité nouvelle (épingles) → **0.5.0** (MINEUR).

## § Réalisation (2026-08-21)

- **E1 (glyphes)** : `download`, `keep`, `keep_off` — sous-ensemble
  61 → **64** (26 704 octets), cache-buster `?v=64` (convention
  v = nombre de glyphes), README amendé, les deux exemplaires
  recopiés, **preuve apercu.html rejouée : PASS 65/65 ligatures
  repliées** (serveur local, 2026-08-21).
- **E2 (pièces en tête, R2)** : le bloc « Fichiers joints » passe sous
  la tête du message, avant la garde d'images et le corps — même
  conteneur flex, aucun CSS neuf. e2e d'ordre DOM
  (`compareDocumentPosition`), le premier à verrouiller la position.
  Système A71 (maquette et prose amendées).
- **E3 (voile « Enregistrer », R1, D1)** : voile en recouvrement
  absolu sur `.puce.bouton` — même géométrie (la rangée ne reflue
  pas ; e2e mesure la largeur avant/après), `:hover` ET
  `:focus-visible` (A8), fond `--sel` opaque (paire encre/sel du
  survol existant), glyphe `download` + « Enregistrer » (clé dédiée
  aux deux catalogues). Jamais rendu sur un écho, jamais montré
  pendant un enregistrement en vol (`:disabled`). Système A70.
- **E4 (écran 03 à plat, R3, D2)** : le wrapper `.carte` de
  `Conversation.svelte` meurt — la scène sur `--bg` défile en un seul
  flot, colonne centrée bornée 960 px ; `Fil.svelte` perd la double
  forme (`class:volet` et ses surcharges) : la forme à plat d'A46 est
  désormais LA forme, unique aux deux cadres. e2e jumeau de la
  platitude du volet (aucune élévation englobante entre la racine et
  l'objet-fil, cartes élevées, scène qui défile, colonne 960 px).
  Système A72 (renverse « l'écran 03 garde sa carte pleine » d'A46).
- **E5 (épingles, cœur, TDD)** : RED montré (13 erreurs — méthodes
  absentes) puis GREEN. Table `pins` (clé d'enveloppe, survit au DROP
  des tables de fils ; JAMAIS `flagged`, écrasé par la synchro) ;
  `toggle_pin` (le fil résolu UNE fois via `thread::thread_of`,
  désépingler libère le fil entier), `pin_state` (l'état se lit par le
  fil), `pinned_unified_scoped` (mêmes colonnes et même queue de
  jointures que la page — `UNIFIED_JOIN_TAIL` partagé) ; exclusion D5
  dans `unified_page_sql`, `unified_count_scoped` ET `unified_count`
  (la paire page/total décrit le même ensemble). **Garde de plan
  étendue** : la sous-requête des épingles part de `pins`
  (`CROSS JOIN` directif — sans lui, SQLite sans ANALYZE scannait
  `envelopes` ENTIÈRE sur le chemin le plus chaud, ~24 ms mesurés à
  200 k ; revue). Tests : épingle à part et hors du flot (bornée au
  compte, onglet non-lus), épingle qui suit le fil et sa tête
  nouvelle. 357 tests mail-core.
- **E6 (épingles, UI)** : bouton « Épingler »/« Désépingler »
  (`aria-pressed`) dans la barre du fil, offert par
  `epinglable = Réception ET hors recherche` (dérivé UNE fois dans
  App — un résultat de recherche peut vivre hors Réception, l'épingle
  serait invisible) ; l'état est SEMÉ de la ligne servie
  (`MessageRow.pinned` — exact par construction : le flot n'est jamais
  épinglé D5, la section l'est toujours), AUCUN aller-retour à
  l'ouverture ; la réponse de bascule est gardée par la clé du fil
  (discipline de jeton). Liste : section épinglée préposée au flot
  dans le même défilement (hauteur mesurée, fenêtrage recalé par effet
  à chaque mouvement de la mesure), marque `keep` « Épinglé » au rang
  de puces, le vide ne s'affirme que quand les DEUX sources ont
  répondu (E2 tenu sur une boîte toute épinglée), « N éléments »
  compte les épinglées affichées. e2e : épingler → en tête, une seule
  ligne, réversible ; hors Réception pas de bouton. Système A73.
- **Revue à regard neuf** (`/code-review high`, 8 angles) :
  10 trouvailles, **9 corrigées** — dont le scan d'`envelopes` de la
  sous-requête des épingles (mesuré au banc, CROSS JOIN + garde de
  plan), le vide menteur d'une boîte toute épinglée, l'écriture de
  `fil.epingle` sans discipline de jeton, le `pin_state` par ouverture
  (commande supprimée — l'état vient de la ligne), les compteurs
  incohérents, `unified_count` désaccordé de sa paire, les écritures
  dans `nav.rs` « tout est lecture » (déménagées à `store.rs`), la
  duplication `thread_of`/squelette SQL, le fenêtrage non recalé —
  **1 assumée** : l'épingle orpheline si le message-clé quitte sa
  boîte (DETTE **D-28**).
- e2e : nouveau spec `refonte-retours-7.spec.js` (5 parcours),
  103 → **108** ; deux assertions de la barre du fil étendues
  (`epingler` en Réception).

## § Terrain (STOP 2, 2026-08-21) — première passe : 4 OK, 1 constat corrigé le jour même

Verdicts CE : 1 OK · 2 OK · 3 OK · 4 OK sur les comportements +
1 constat visuel · 5 OK.

4. *« Les messages épinglés devraient apparaître dans le volet central
   avec la même forme que la boîte email sélectionnée du volet de
   gauche, pour mieux les distinguer visuellement. »* — **Corrigé** :
   la ligne épinglée prend le dessin de la **tuile de la boîte en
   cours** (nav, W2-D5) — fond `--tuile`, encre `--tuileInk` (paire
   déjà mesurée par la gate des contrastes) ; la teinte tient au
   survol, la sélection garde son liseré d'accent. Système A73 amendé ;
   e2e étendu (le fond calculé de la ligne épinglée == celui de la
   tuile de nav).

## § Décisions CE — tranchées le 2026-08-21 (STOP 1, GO)

- **D1 — le mot du survol (R1)** : — *Réponse CE (2026-08-21) :
  « Enregistrer »* — le vocabulaire actuel du produit : cohérent avec
  le dialogue « Enregistrer sous » et le toast « Pièce enregistrée » ;
  le voile dit « Enregistrer » (glyphe `download`), rien d'autre ne
  bouge dans les catalogues.
- **D2 — la largeur de l'écran 03 à plat (R3)** : — *Réponse CE
  (2026-08-21) : « Colonne centrée ~960 px »* — une colonne de lecture
  bornée et centrée ; sur un écran large, les cartes restent à largeur
  de lecture confortable.
- **D3 — le geste d'épingler (R4)** : — *Réponse CE (2026-08-21) :
  « Barre du fil seule »* — « Épingler »/« Désépingler » rejoint
  Archiver/Supprimer/Spam dans la barre du fil (volet + écran 03),
  aucune surface neuve sur les lignes.
- **D4 — la portée des épingles (R4)** : — *Réponse CE (2026-08-21) :
  « Réception seule »* — boîte unifiée et boîte par compte ;
  extensible ensuite si le terrain le demande.
- **D5 — la place de la ligne épinglée (R4)** : — *Réponse CE
  (2026-08-21) : « En tête seulement »* — la liste ne montre jamais
  deux fois le même message ; le flot paginé exclut les épinglés.
- **D6 — publication** : — *Réponse CE (2026-08-21) : « Une 0.5.0 »* —
  une seule release MINEUR emportant les quatre retours, après terrain
  validé et CI verte.
