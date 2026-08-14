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

### D-6 · Flake e2e v1 : « étoiler » (parcours-critiques)

- **Fait (2026-08-13, gate E1 de PLAN-SYNCHRO)** : le test v1
  « étoiler : “s” pose l'étoile » a échoué UNE passe sur trois de la
  suite complète (la classe `flagged` n'apparaît pas dans les 15 s),
  puis passé 12/12 en isolation et à la passe suivante. Chemin
  (`mark_flagged`, UI v1) étranger au chantier synchro.
- **Décision** : consigné, non instruit — v1 est dormante depuis B1 et
  le test disparaît avec elle à B2 (PLAN-RETRAIT-V1).
- **Rouvre si** : il retombe sur un parcours v2, ou si B2 est reporté.

### D-7 · Les gestes interactifs se paient au débit du fond pendant une synchro

- **Fait (terrain 0.1.4, 2026-08-14)** : pendant une passe de
  synchronisation (session d'après-mise-à-jour, rattrapage en cours),
  l'ouverture d'un message fraîchement reçu a mis **plus d'une minute**
  à afficher son corps, et le rapatriement d'une pièce en transfert
  **plus de 30 secondes** — les deux sont des allers-retours serveur
  interactifs (`message_body`, `fetch_source_attachment`) sur leur
  propre connexion, mais le MÊME compte Gmail servait en parallèle la
  passe de fond à plein débit. Écarté à l'instruction : le verrou
  d'écriture SQLite (busy_timeout 5 s — il aurait erré, pas attendu),
  un sommeil ou martèlement dans le chemin de connexion (aucun).
- **Hypothèse** : bridage par compte côté Gmail (bande passante et
  commandes IMAP partagées entre la passe de fond et le geste).
- **À instruire** : mesurer au terrain (chronologie `sync_activity`
  contre l'horodatage du clic), puis un mécanisme de PRIORITÉ AU
  GESTE — suspendre ou céder la passe de fond pendant un
  aller-retour interactif (le rattrapage des corps sait déjà se
  borner ; la passe d'en-têtes, moins). Chantier de synchronisation,
  pas de pièces jointes — le composeur et la Lecture n'ont pas à
  connaître la charge du compte.
- **Corollaire du même terrain** : la puce 📎 de la ligne du volet
  central n'apparaît qu'à la re-servie de la liste après le scan (~2
  minutes observées PENDANT la contention) — se dissout avec la
  cause ; à re-mesurer après D-7.

## Soldée

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
