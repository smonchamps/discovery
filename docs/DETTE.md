# Registre de dette

La dette assumée EN CONNAISSANCE : chaque entrée dit le fait mesuré, le
choix, et ce qui la rouvrirait. Une entrée se solde par un commit qui la
raye — jamais par l'oubli. (STANDARD §2.6 : un report = une ligne
motivée.)

## Ouverte

### D-1 · Ouverture p95 au-dessus du budget sur les très gros corps

- **Fait (2026-08-11, gate R1)** : ouverture p95 mesurée à 52–55 ms
  (budget 50 ms) sur la base réelle — p50 ~14 ms. Le dépassement est
  porté par UN corps > 1 Mo de l'échantillon déterministe ; la base en
  compte 207 (jusqu'à 28 Mo). Identique avec et sans le cycle de
  synchro : coût d'assainissement du HTML, pré-existant à la refonte.
- **Décision CE (2026-08-11)** : acté en dette, à traiter plus tard.
- **Piste** : assainissement paresseux ou streaming des très gros
  corps ; à instruire comme chantier cœur séparé.
- **Rouvre si** : le terrain rapporte des ouvertures perceptiblement
  lentes, ou si le p50 dérive.

### D-2 · Éviction LRU des pages de liste

- **Fait (P1-P2)** : la liste fenêtrée garde toutes les pages servies en
  mémoire ; sur une session très longue à grands défilements, la RAM
  monte sans redescendre.
- **Piste** : éviction LRU des pages hors fenêtre.
- **Rouvre si** : la RAM dépasse le budget (200 Mo) en usage réel.

### D-4 · Piège de focus des surimpressions

- **Fait (A8)** : Tab peut sortir d'une surimpression (composition,
  réglages) vers le fond ; Échap et le focus visible couvrent
  l'essentiel.
- **Rouvre si** : le terrain au lecteur d'écran le réclame.

### D-8 · Requêtes chères des sondes périodiques (hors pompe, coût CPU réel)

- **Fait (2026-08-15, PLAN-GELS)** : `nav_snapshot` **865 ms** par
  compte Gmail (compteur Archives d'une intégrale, exclusion par
  `message_id`, 87 k lignes — toutes les 10 s) ; `pending_total`
  **575 ms** (COUNT par boîte, NOT EXISTS sur `bodies` — à chaque
  génération de courrier). Mesurés en SQL direct sur base réelle.
- **Décision CE (2026-08-15, D4 du plan)** : depuis `hors_pompe()`
  elles ne gèlent plus rien ni personne — les optimiser sans constat
  serait du travail sans mesure. Famille D-7 (chronos de réactivité).
- **Piste** : cache des compteurs de nav invalidé par génération ;
  `pending_total` en une requête agrégée.
- **Rouvre si** : le terrain désigne le coût (ventilateur, batterie,
  contention d'écriture, latence perçue des sondes).

### D-9 · L'invariant A41 n'a pas de garde structurelle

- **Fait (revue 2026-08-15, A41)** : « rien ne touche la base avant
  `migration_check` » vit dans des commentaires et un test de sonde
  (`la_langue_se_lit_sans_adopter_la_base`) — rien n'empêche une
  future commande pré-modale d'ouvrir la base en plein
  (`Store::open`) : toute la suite resterait verte, le bug se
  redécouvrirait au terrain.
- **Piste** : un drapeau `adopted` sur `MigrationShared` (ou un
  helper d'ouverture partagé des 30+ `Store::open` de `commands.rs`)
  qui fait échouer BRUYAMMENT toute ouverture pleine avant la sonde ;
  à instruire en chantier, pas en voie rapide.
- **Rouvre si** : une commande de démarrage s'ajoute avant la modale.

### D-10 · La pose différée de la langue n'a pas de test UI

- **Fait (revue 2026-08-15)** : la moitié Rust d'A41 est tenue par un
  test qui rembobine une vraie base ; l'ordre côté UI (`assurer()`
  avant `poserLangueDetectee()`, pose seulement si la sonde a
  répondu) n'est asserté par aucun e2e — `refonte-langue.spec.js` ne
  joue jamais le premier lancement sur base vierge.
- **Piste** : spec e2e `vierge: true` qui asserte `prefs.lang` posée
  après démarrage, et absente si `migration_check` échoue.
- **Rouvre si** : un refactor d'`onMount` touche à l'ordre de
  démarrage.

### D-11 · Le banc de bascule de thème est resté calibré à 7 thèmes

- **Fait (revue 2026-08-16, PLAN-WADA-ELARGI)** : `e2e/mesure-v2.mjs`
  garde 60 itérations et des commentaires « les 7 thèmes » alors
  qu'A42 en livre 28 — l'échantillon par thème tombe de ~8 à ~2, le
  chiffre « coût de bascule par thème » n'est plus comparable à la
  ligne de base historique.
- **Raison du report** : hors périmètre du chantier (fichier non
  touché par le diff), et recalibrer sans re-mesurer une ligne de base
  serait du travail sans mesure. Famille D-7 (chronos de réactivité).
- **Piste** : à la prochaine passe de mesure, recalibrer (28 × N
  itérations) et re-poser la ligne de base dans le même relevé.
- **Rouvre si** : une passe de mesure compare la bascule de thème à
  l'historique.

### D-15 · Affichage « À : destinataire » cadré sur la catégorie Envoyés

- **Fait (2026-08-16, PLAN-RETOURS-MAIL R4)** : dans la liste, la bascule
  vers « À : X » (au lieu de l'expéditeur = SOI) est gardée par
  `categorie === 'envoyes'` (`Liste.svelte`). Un envoi consulté par une
  autre voie que la catégorie Envoyés — navigation par dossier — montre
  encore l'expéditeur. Le volet de lecture, lui, est correct partout (il
  se cale sur `propre(m)`, pas sur la catégorie).
- **Raison du report** : le retour terrain visait le dossier Envoyés ;
  une détection par ligne (« ce message est de moi ») basculerait aussi
  des lignes de la Réception où NOTRE dernière réponse est en tête, un
  changement de comportement plus large que le retour.
- **Rouvre si** : le terrain consulte des envois hors de la catégorie
  Envoyés et l'expéditeur affiché gêne.

### D-16 · Rattrapage des destinataires : sonde de reliquat non indexée

- **Fait (2026-08-16, PLAN-RETOURS-MAIL)** : `backfill_recipients`
  appelle `recipients_pending_count` (scan `to_addrs IS NULL`, sans
  index) à chaque cycle, INBOX + Envoyés — même après convergence à
  zéro. Même classe que D-8 (sondes périodiques hors pompe : coût CPU
  réel, aucun gel).
- **Raison du report** : aligné sur le motif existant de la passe
  d'en-têtes (`thread_headers_pending_count`, même coût, accepté) ;
  l'optimiser sans constat serait du travail sans mesure. Famille D-8.
- **Piste** : sauter la sonde quand le passage n'a rien rapporté, ou un
  index partiel `WHERE to_addrs IS NULL`.
- **Rouvre si** : le terrain désigne le coût (avec D-8).

### D-17 · Rattrapage des corps aveugle à un bridage Gmail

- **Fait (analyse 2026-08-17)** : sur le chemin du rattrapage des corps,
  une erreur serveur Gmail (bridage : `[OVERQUOTA]`, « bandwidth
  exceeded », rejet de login / « web login required ») est **capturée
  puis jetée**. `run_backfill_all` la pousse bien dans
  `BackfillSummary.errors` (`commands.rs:4135` au connect, `:4158` en
  plein FETCH), mais la boucle UI ne lit JAMAIS `bilan.errors`
  (`App.svelte:391-417`) : elle ne regarde que `remaining` et `fetched`,
  et casse en silence sur `fetched === 0`. Symptôme au terrain :
  « Rattrapage des messages · N restants » se fige sans un mot — le pire
  cas pour diagnostiquer un bridage. Le rattrapage est le seul des trois
  à ignorer son canal d'erreurs (la synchro, elle, remonte via
  `synchroEchec`, `App.svelte:181`).
- **Second défaut couplé** : `is_connection_error` (`mail-imap`,
  `lib.rs:897`) ne reconnaît QUE nos erreurs préfixées `"connexion "`.
  Une réponse serveur Gmail tombe donc dans le `Err(_)` de
  `connect_imap` (`commands.rs:3666`) et est traitée comme « jeton
  mort » → `authenticate_silent` + reconnexion. Le garde anti-martèlement
  du commentaire (`commands.rs:3660-3664`) ne protège que le cas panne
  réseau : un bridage déclenche justement le refresh + reconnexion qu'il
  cherche à éviter.
- **Raison du report** : aucun bridage observé au terrain à ce jour — la
  méthode interdit d'optimiser contre un problème non mesuré. Mais la
  cécité, elle, est structurelle : le jour où ça mord, rien ne le dira.
- **Piste** : (1) dans la boucle UI, si `fetched === 0` et
  `bilan.errors.length > 0`, poser un avis dans la fente (mécanisme déjà
  là, `App.svelte:552-558`) au lieu de casser muet ; (2) reconnaître le
  bridage (élargir `is_connection_error` ou garde dédiée) pour qu'un
  `[OVERQUOTA]` / rejet de login ne déclenche PAS de refresh —
  « laisser respirer » le compte, pas « jeton mort ».
- **Rouvre si** : le terrain rapporte un rattrapage qui se fige sans
  explication, ou un compte Gmail bridé/verrouillé après une grosse
  synchro initiale. Commencer par (1) — rendre l'erreur visible est le
  préalable à tout diagnostic.

### D-18 · « Charger plus » : le bouton et l'append ne sont pas testés en e2e

- **Fait (2026-08-17, PLAN-CHARGER-PLUS)** : le bouton « Afficher les N
  suivants », l'append, la borne douce à 1000 lignes et l'anti-course ne
  sont pas couverts en e2e. Le bouton n'apparaît qu'au-delà de 100 résultats
  (`resultats.length < total`), or les décors du gate (Clarity, inbox à 6-10
  messages) sont bien en dessous. Seul le CŒUR est testé
  (`search_capped_pages_without_gap_or_overlap` : pages sans trou ni
  doublon) ; l'UI a été validée AU TERRAIN (base 251 k) et par revue à
  regard neuf (qui a d'ailleurs attrapé un bug d'anti-course).
- **Raison du report** : un décor e2e à >100 messages avec un terme commun
  (`seed_inbox` à grand `nombre` + `ko_par_corps ≥ 1`, dont les corps
  portent les mots de `MOTS`) est une infrastructure à part, disproportionnée
  pour ce chantier — le terrain a couvert le geste.
- **Piste** : un spec dédié sur une base `seed_inbox` large (≈ 250 messages) ;
  assertions : bouton présent, clic → la liste grandit et « N sur M » monte,
  ~1000 lignes → invite « Affinez votre recherche », terme rare → pas de
  bouton.
- **Rouvre si** : le comportement du bouton régresse au terrain, ou avant de
  retoucher à `chargerPlus`/la borne.

### D-19 · Reprise cross-appareil d'un brouillon ne restitue pas Cc/Cci

- **Fait (2026-08-17, PLAN-RETOURS-2 #4)** : Cc et Cci sont persistés
  **localement** (colonnes `drafts.cc_raw`/`bcc_raw`) — autosave, fermeture
  et reprise sur la MÊME machine les gardent, et le miroir poussé au dossier
  Brouillons Gmail (`draft_bytes`) porte les en-têtes Cc et Bcc. Mais le
  **tirage** d'un brouillon écrit sur un AUTRE appareil (`import_remote_draft`,
  `commands.rs:1449`) ne lit que `to_raw`/`subject`/`body` du message
  distant : les Cc/Cci d'un brouillon rapatrié repartent **vides**.
- **Raison du report (§2.6)** : le chemin d'analyse distant (parser l'en-tête
  Cc, décider du sort du Bcc rapatrié) est une tranche à part ; la perte ne
  touche que la reprise cross-appareil, jamais l'envoi ni la reprise locale.
- **Piste** : étendre `RemoteDraft` + `convert.rs` pour extraire Cc (et,
  décision à prendre, Bcc) du message distant, puis `import_remote_draft`
  peuple `cc_raw`/`bcc_raw`.
- **Rouvre si** : un utilisateur signale des Cc perdus en reprenant un
  brouillon commencé ailleurs.

### D-20 · Cycle Gmail : coût par cycle encore élevé quand beaucoup de vues bougent

- **Fait (2026-08-17, PLAN-RETOURS-2 #1, ADR 0021)** : la cadence à 30 min
  a réglé la FRÉQUENCE (de ~45 % du temps en synchro à ~7 %). Mais un cycle
  complet Gmail coûte encore **jusqu'à ~135 s** quand beaucoup de vues ont
  changé (mesure release : 22 dossiers relevés, ~5 s/dossier changé — bridage
  Gmail probable, cf. **D-17**). L'exclusion des vues virtuelles (Important,
  Suivis) a été **écartée** : marginale après la cadence, et coûteuse (champ
  neuf au type `Folder` du cœur, détection des drapeaux `\Important`/`\Flagged`
  dans l'adaptateur, logique voisine de l'ADR 0010). « Tous les messages »
  est délibérément CONSERVÉ (Archives, mail archivé ailleurs — ADR 0010).
- **Raison du report (§2.6)** : la cadence capte l'essentiel du gain ; le
  reste ne vaut pas la surface de code tant qu'un terrain ne le redemande pas.
- **Piste** : (1) exclure Important/Suivis par drapeau IMAP (non fragile) ;
  (2) attaquer le ~5 s/dossier (bridage — croise D-17) ; (3) LIST-STATUS
  n'aide pas Gmail (non annoncé), l'inventaire reste à ~52 STATUS.
- **Rouvre si** : le cycle à 30 min redevient gênant au terrain, ou si le
  bridage est confirmé (croise D-17).

### D-21 · Pourcentage de rattrapage : double COUNT du corpus par lot

- **Fait (2026-08-18, PLAN-RETOURS-3 R1)** : le dénominateur du `%` de
  rattrapage (A55) ajoute `corpus_total` (un COUNT du corpus par boîte) à
  côté de `pending_total`, recalculés **à chaque lot** de `backfill_bodies`
  (50 corps) pendant la boucle de rattrapage — soit deux+ balayages complets
  par lot sur ~256 k messages × ~40 boîtes. Détecté à la revue
  `/code-review high`.
- **Raison du report (§2.6)** : la sonde est **hors pompe** (ne gèle pas la
  fenêtre) et le terrain a jugé l'app **parfaitement fluide** au rattrapage
  (2026-08-18) — le budget est tenu. Optimiser sans constat serait du travail
  sans mesure. **Famille D-8** (sondes périodiques hors pompe, coût CPU réel).
- **Piste** : une seule requête par boîte rendant `(total, manquants)`
  ensemble ; ou cacher le total (quasi stable dans une boucle) et ne le
  rafraîchir qu'au changement franc.
- **Rouvre si** : le terrain désigne le coût CPU du rattrapage (avec D-8, D-16).

### D-22 · « Signaler comme spam » sur un spam atteint par la recherche

- **Fait (2026-08-18, PLAN-RETOURS-3 R2)** : `report_spam` rend `Ok(())`
  sans rien déplacer quand le message est **déjà** dans le dossier indésirable
  (`spam == mailbox`), mais l'UI flashe « signalé comme indésirable » et ferme
  le fil. Atteignable seulement via la **recherche** (un spam ouvert avec
  `categorie != 'indesirables'` montre encore le bouton « Signaler »).
- **Raison du report (§2.6)** : cas de bord (chemin recherche→spam),
  cosmétique (aucune donnée corrompue) ; le corriger proprement exigerait que
  l'UI connaisse le dossier Junk de chaque compte (le `Fil` ne l'a pas) —
  disproportionné pour la fréquence.
- **Rouvre si** : l'usage réel montre le faux succès gênant.

## Soldée

### ~~D-6 · Flake e2e v1 : « étoiler » (parcours-critiques)~~ — soldée le 2026-08-15

- **Fait (2026-08-13)** : flake une passe sur trois du test v1
  « étoiler », chemin étranger aux chantiers en cours ; consigné, non
  instruit — v1 dormante.
- **Soldée à B2** (PLAN-RETRAIT-V1) : la spec `parcours-critiques` est
  retirée avec l'interface v1, le flake meurt avec elle. Rouvrirait si
  un symptôme équivalent touchait un parcours v2.

### ~~D-3 · Dates en jour de semaine (2 à 6 jours)~~ — soldée le 2026-08-12

- **Fait (P3)** : le prototype affiche « Lundi, 18:20 » pour les
  messages de la semaine ; `quand()` affichait « 8 août ». Écart visuel
  mineur, dit à la livraison P3.
- **Soldée à E2 des Réglages** (R-D1, PLAN-REGLAGES) : `quand()` étendu
  — 2 à 6 jours → jour de semaine, `quandLong()` compose « Lundi,
  18:20 » sans retouche. Sans réglage : la forme du prototype ne
  s'opte pas.

### ~~D-5 · Charset amont (U+FFFD dans les corps stockés)~~ — soldée le 2026-08-11

- **Fait** : des corps de la base réelle portaient U+FFFD dès le HTML
  stocké — décodage du charset MIME à la synchronisation.
- **Soldée par `0f7f059`** (session séparée, PR #1 fusionnée) : feature
  `full_encoding` de mail-parser (gb2312…), repli windows-1252 quand
  les octets ne sont pas de l'UTF-8 valide, et réparation une-fois des
  corps mutilés (purge marquée, retéléchargement au rattrapage, aperçu
  et index refaits au passage).

### D-12 · Ouverture du fil : cascade `thread_messages` → corps, série P1 à re-baser

- **Fait (UI v3, revue du 2026-08-16)** : la sélection ouvre désormais
  le FIL — `thread_messages` puis le corps du dernier, en série (deux
  allers-retours là où v2 n'en faisait qu'un), et le chrono P1
  « ouverture » a changé de définition (sélection → fil affiché,
  pièces exclues). La série historique (< 50 ms, ADR 0015) n'est plus
  comparable.
- **Report assumé** : paralléliser le corps de la ligne de tête avec la
  liste du fil (un fetch perdu dans le cas rare d'une tête plus
  fraîche), et re-baser le banc `mesure-v2` sur la nouvelle définition.
  Reporté pour garder le commit v3 sur les verdicts CE ; à instruire
  avec D-7/D-11 (famille bancs).

### D-13 · Agrandir/réduire remonte les iframes du fil

- **Fait (revue v3)** : le changement de cadre démonte puis remonte les
  iframes `srcdoc` des messages dépliés — le réseau ne rejoue rien
  (état partagé), mais le rendu re-parse chaque document et perd le
  défilement interne. Sensible sur un long fil « tout déplié ».
- **Report assumé** : garder les deux cadres montés (`display:none` +
  `inert`) coûterait des testids dupliqués — exactement ce que la
  revue v3 a corrigé ; le remède demande de re-scoper la suite e2e
  d'abord. À instruire si le terrain le sent.

### D-14 · Re-baser le banc P1 sur la géométrie A44

- **Fait (PLAN-RETOURS-V3, 2026-08-16)** : deux changements de
  géométrie dans le même chantier — les barres overlay rendent ~10 px
  de largeur à chaque volet défilant (0 px réservé contre 10 px
  webkit), et la liste est à deux gabarits (h1 nue / h2 porteuse,
  ~+27 px par ligne à puces) : `visibles`, le nombre de lignes rendues
  par saut et le coût d'un reflow ont bougé. Les percentiles « page »
  du banc `mesure-v2` ne sont plus comparables à la série d'avant A44.
- **Report assumé** : re-mesurer et re-baser les budgets P1 sur la
  géométrie livrée, en une passe dédiée — à instruire avec la famille
  des bancs (D-7/D-11/D-12), pour ne pas mélanger une re-base de
  budgets avec un chantier de features. Les bancs mesurent DÉJÀ la
  bonne géométrie (args-navigateur.mjs) ; seule la série de référence
  date.

### D-23 · Téléchargement d'une pièce : chemin réseau non couvert en e2e

- **Fait (PLAN-RETOURS-4, R1, 2026-08-18)** : le nouveau geste
  « Enregistrer sous » (clic puce → `chemin_enregistrement_suggere` →
  dialogue `plugin:dialog|save` → `save_attachment(dest)`) a sa couture
  e2e (`__e2eDestination`) mais aucun test ne l'exerce. Le succès exige
  le rapatriement IMAP des octets — terrain seul par construction
  (§7.5) ; seul le chemin d'annulation (`!dest → return`, ni toast ni
  fetch) est jouable hors ligne.
- **Report assumé** (code-review high, écart assumé) : R1 est validé au
  terrain (CE, 2026-08-18). Ajouter un e2e du seul chemin d'annulation
  apporte peu ; le chemin de succès restera terrain. À instruire si un
  banc de composition/pièces se monte.

### D-24 · Stub PASSATION.md à retirer

- **Fait (PLAN-DOCUMENTATION, 2026-08-19, décision CE D3)** :
  PASSATION.md est scindée en STANDARD.md (le standard de travail) et
  ETAT.md (l'instantané de relève) ; un stub de quelques lignes reste
  au chemin historique — poka-yoke pour les vieilles mémoires et
  l'ancien rituel de reprise.
- **Condition de retrait** : deux reprises à froid consécutives sans
  que rien ne trébuche sur l'ancien chemin ; alors le stub se supprime
  (commit `docs:`) et cette entrée se raye.
- **Avancement** : première reprise comptée le 2026-08-19 (E4 du
  chantier — reprise ordinaire propre, et le stub a rattrapé l'ancien
  rituel collé volontairement). Une reprise propre de plus et il tombe.

### D-25 · Composeur riche : quatre écarts assumés de la revue

- **Fait (PLAN-COMPOSITION-HTML, revue du 2026-08-20)** : quatre écarts
  relevés par la revue à regard neuf, assumés sans correctif —
  1. le miroir JS `texteEnHtml` (Composition.svelte) duplique
     `mail_core::texte_en_html` (8 lignes, documenté des deux côtés) :
     le supprimer exigerait de servir la conversion par le Rust, ce qui
     re-créerait le churn texte→riche que la garde anti-churn vient de
     tuer ; une divergence d'échappement resterait invisible aux tests ;
  2. `mail_smtp::draft_bytes` porte 8 arguments positionnels dont six
     `&str` consécutifs (allow clippy posé) : une inversion Cc/Cci à
     l'appel compilerait — le chemin le moins testé (reflet Brouillons) ;
  3. `DraftContent` n'a pas de `Default` : le prochain champ retouche
     ~45 littéraux de test mécaniquement ;
  4. le triptyque e2e de vidage d'un contenteditable (clic + Ctrl+A +
     Suppr) est recopié trois fois dans les specs.
- **Condition de reprise** : au prochain chantier qui touche les
  brouillons ou l'envoi, régler 2 et 3 (grouper les paramètres, dériver
  `Default`) ; 1 se rouvre seulement si un troisième convertisseur
  apparaît ; 4 au prochain spec qui en aurait besoin (helper partagé).

### D-26 · Pagination profonde des catégories : coût O(offset) assumé

- **Fait (PLAN-DEFILEMENT-PROFOND, 2026-08-20, décision CE D1)** : hors
  réception, `category_page` paie `LIMIT offset+limit` par boîte + tri
  fusionné — la page de 200 coûte 10 ms à l'offset 0, 66 ms à 10 000,
  157 ms à 40 000, **247 ms à 80 000** (SQL brut, base seedée 120 000,
  release). Le budget « page de liste < 100 ms » (STANDARD §3) est
  crevé dès ~20 000 ; la réception, elle, tient (patron `threads` +
  `idx_threads_date_globale`, 14,6 ms à l'offset 200 000). La clause
  d'exclusion de l'intégrale Gmail coûte peu (index partiel
  `idx_envelopes_message`).
- **Pourquoi assumé** : depuis A64, une seule page profonde vole à la
  fois (file bornée, VOL_MAX = 1) et l'écran dit le chargement — la
  latence d'UNE page isolée est vivable ; c'était la rafale ×
  sérialisation qui faisait la panne de plusieurs minutes. Resserre du
  terrain (2026-08-20) : le comptage (`category_totals`, sonde
  NOT EXISTS par ligne d'intégrale, ~240 ms sur 200 k) ne se paie plus
  qu'à la page 0 — la page profonde nue passe de 368 à ~129 ms sur le
  décor intégrale.
- **Condition de reprise** : si le terrain mesure une page profonde
  au-delà de ~1 s sur la vraie base (256 k, 4 comptes), ou si un
  chantier « liste sans limite » s'ouvre (le report existant de la
  recherche virtualisée) — le patron deux-temps de la recherche (A51)
  est le point de départ.

### D-27 · La boîte d'envoi ne retente qu'en fin de cycle ou au geste

- **Fait (terrain PLAN-RETOURS-5, 2026-08-21)** : au premier lancement,
  deux envois cliqués juste après l'ouverture sont restés « en
  attente » pendant toute la session — la vidange déclenchée par le
  clic « Envoyer » est passée avant que les sessions des comptes ne
  soient prêtes, et la boîte d'envoi n'a AUCUNE retentative propre :
  elle ne repart qu'à la fin d'un cycle complet (long sur la vraie
  base), à la passe légère du clic « Synchroniser », ou au retour
  réseau. Les messages sont partis à la première vidange réellement
  déclenchée (clic Synchroniser de la session suivante) — jamais
  perdus, jamais doublés : les règles d'or ont tenu.
- **Pourquoi assumé** : le cas ne se présente qu'à l'envoi dans les
  toutes premières secondes d'un lancement ; en régime établi, la
  vidange d'après-envoi part et aboutit (vérifié au même terrain).
  La barre d'état dit honnêtement « N envois en attente ».
- **Condition de reprise** : si le terrain revoit un envoi en attente
  au-delà d'un cycle, ou au premier retour utilisateur en bêta.
  Piste : une retentative bornée déclenchée à l'établissement des
  sessions (le déclencheur du retour en ligne, R-D3, existe déjà —
  l'y raccorder), jamais un minuteur de martèlement.
