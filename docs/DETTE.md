# Registre de dette

La dette assumée EN CONNAISSANCE : chaque entrée dit le fait mesuré, le
choix, et ce qui la rouvrirait. Une entrée se solde par un commit qui la
raye — jamais par l'oubli. (PASSATION §2.6 : un report = une ligne
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
