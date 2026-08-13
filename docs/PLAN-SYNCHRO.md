# Plan — Synchronisation : sincérité, geste manuel, temps réel

Commande (2026-08-13) : les retours bêta sur la synchronisation ne sont
pas bons. Trois plaintes : (1) pas de vrai temps réel — la bulle arrive
sur le téléphone, le message n'apparaît dans Discovery que quelques
minutes plus tard ; (2) pas de bouton de synchronisation manuelle ;
(3) la barre d'état du pied de page manque de clarté et n'a aucune
barre de progression.

**La plainte n°1 attaque la promesse elle-même** — *« Vos mails,
instantanément »* (PLAN.md). Ce n'est pas un chantier de confort.

## 0. L'analyse — cinq pourquoi, trois causes racines

**Plainte 1 — le retard.** Le téléphone reçoit en push (serveur → APNs/FCM) ;
Discovery **sonde toutes les 5 minutes** (`App.svelte` :
`setInterval(synchroniser, 300000)`), et le cycle est **complet et
séquentiel** — INBOX du compte 1, puis TOUS ses dossiers, puis le
compte 2… La liste ne se recharge qu'en **fin de cycle**. Latence
perçue : jusqu'à 5 min de sondage + la durée du cycle. Cause racine :
**IDLE/push est un report assumé de Phase 1** (PASSATION §8), jamais
rouvert depuis. Les bulles de Discovery elles-mêmes partent en fin de
passe — le téléphone gagnera toujours.

**Plainte 2 — le bouton absent est une décision, pas un oubli.** D5
(PLAN-UI-V2 §7) : « Coupé — synchro automatique + ligne de progression
+ barre de statut suffisent ». **Le terrain vient d'invalider cette
hypothèse.** Mais le bouton réclamé est aussi un symptôme : on réclame
un levier quand on ne fait pas confiance à la machine — et la confiance
est détruite par la plainte 1 (c'est lent) et la plainte 3 (on ne voit
rien). Corriger la plainte 2 seule traiterait le symptôme.

**Plainte 3 — la barre d'état ment par omission.** Quatre défauts relevés :
- le prototype normatif affiche « Tous les messages sont à jour ·
  **dernière synchronisation il y a 2 minutes** » — la seconde moitié
  n'a **jamais été implémentée** (aucun horodatage en base : écart
  d'implémentation au prototype signé, pas une capacité neuve) ;
- « Tous les messages sont à jour » reste affiché **pendant qu'un
  cycle tourne** — `sync_percent` ne bouge qu'en rattrapage intégral,
  le cycle courant est invisible ;
- l'échec de synchro passe APRÈS l'attente d'envoi dans les priorités
  du statut, et rien ne dit depuis quand on est à sec ;
- aucune barre visuelle — le Système (A4) ne prévoit que du texte
  atténué ; le prototype est muet ; le terrain réclame la barre.

## 1. État des lieux

| Surface | Constat | Sort |
|---|---|---|
| `App.svelte` — cycle | sondage 300 s, réentrance gardée, recharge en fin de cycle seule, pas de relève au réveil de veille | **P1** (recharge par compte), **E3** (réveil), E4 |
| `App.svelte` — `statut` dérivé | 8 priorités, pas d'horodatage, cycle courant invisible ; échec partiel muet | **E1**, **E3** (échec partiel) |
| `store.rs` | aucun horodatage de dernière synchro | **E1** (prefs, patron `lang`) |
| `commands.rs` — `sync_inbox` | cycle complet séquentiel, aucun état observable pendant | **E1** (activité), **P1** (courrier par compte), E3 (passe légère) |
| `sync_percent` | ne raconte que le rattrapage intégral | conservé tel quel |
| Police maison (32 glyphes) | pas de glyphe `sync` | **E3** (régénération, patron A12/A14) |
| `mail-imap` | crate `imap` (IDLE supporté), connexions éphémères, aucun timeout de lecture | **P0** (timeout + watchdog), **E4** (spike d'abord) |
| Bulles (`notify.rs`) | émises en fin de cycle, agrégat tous comptes → toujours après le téléphone | **P1** (par compte), **E4** (parité téléphone) |
| Catalogues fr/en (A15) | textes de statut existants seulement | **E1** (clés neuves, deux langues) |

## 2. Le contrat — ce que l'utilisateur doit pouvoir dire

1. « **Je vois quand** Discovery a relevé le courrier pour la dernière
   fois » — horodatage persistant, au mot près du prototype.
2. « **Je vois quand il travaille**, et sur quoi » — état sincère +
   progression visible pendant le cycle.
3. « **Je peux forcer** la relève, et il se passe quelque chose tout de
   suite » — le geste manuel (D5 rouverte).
4. « C'est arrivé sur le téléphone, **c'est dans Discovery** » — IDLE.

L'ordre de livraison suit la confiance : la **visibilité** (E1) et le
**geste** (E3) se livrent vite — c'est la correction « du jour » due au
terrain ; le **temps réel** (E4) passe par un spike mesuré
(front-loading §2.2 : moteur de synchro = point dur). L'audit du
2026-08-13 insère deux chantiers courts avant E2b : le **filet de
sécurité du cycle** (P0) et la **visibilité par compte** (P1).

## 3. Les étapes

### E1 — La barre d'état sincère

**État : livrée le 2026-08-13 (gate locale verte), en attente du
terrain CE.** Amendement A16 inscrit au journal du Système ; e2e :
horodatage au repos affirmé sur le décor Clarity (qui pose désormais
`derniere_synchro`), audit des clés fr/en inchangé et vert.

**Premier terrain (2026-08-13) : « Synchronisation · 2/2 · compte »
figé 7 minutes, sans aucune information — corrigé le jour même.** Deux
causes : (1) régression de la re-priorisation — le % de l'intégrale
était masqué PENDANT le cycle, alors que l'intégrale est le cycle ;
(2) manque réel — rien ne bougeait à l'intérieur d'un compte pendant
le balayage des dossiers. Livré : la boîte courante dans l'activité
(`SyncShared.boite`, posée dossier par dossier), et le cycle affiche
tout ce qu'il sait — « Synchronisation · 2/2 · compte · boîte… ·
37 % », barre déterminée dès qu'un % existe.

**Second terrain (2026-08-13, même jour) : « INBOX… » figé ≥ 2 min 15,
sans %.** Le % absent dit que la base se déclare complète (`percent =
100`) : ces minutes ne téléchargent pas d'enveloppes neuves. Or
l'étiquette « INBOX » couvrait QUATRE phases (relève INBOX, inventaire
des dossiers + garde STATUS, puis plus tard fils et brouillons) —
l'observation était aveugle. Livré, le jour même : **étapes nommées**
dans l'activité (`inventaire des dossiers`, `fils de discussion`,
`brouillons` — clés traduites par l'UI) et **chrono par phase** imprimé
en console (`relève compte N : INBOX x s · inventaire x s ·
n dossiers x s · fils x s · brouillons x s` — durées et décomptes
seuls, §6.8). Suspects à départager par cette mesure : `list_uids`
(inventaire complet des UID à chaque différentiel) et `changes_since`
(CONDSTORE — un modseq vieilli d'une journée de labels Gmail peut
renvoyer des milliers d'enveloppes).

**Troisième terrain (2026-08-13, soir) — la mesure qui réordonne le
plan.** Traces par phase sur la boîte réelle du Chef Ingénieur
(2 comptes, redirection stderr) :

| Phase (compte Gmail) | Cycle 1 | Cycle 2 (récurrent) |
|---|---|---|
| INBOX | 447,7 s | 33,9 s |
| inventaire | 1 361,5 s | 659,9 s |
| 50 dossiers | 2 800,5 s | 1 540,3 s |
| fils + brouillons | 69,8 s | 39,2 s |
| **total** | **~78 min** | **~38 min** |

Le cycle 1 portait les premières intégrales (unique, normal). Le
cycle 2 est la **facture récurrente : ~38 minutes toutes les
5 minutes** — le cycle ne se repose jamais, INBOX n'est revisitée
qu'une fois par cycle, et la latence réelle vécue est de l'ordre de la
durée du cycle. **C'est la cause racine de la plainte bêta n°1**, plus
profonde que l'absence d'IDLE.

Autopsie (audit `mail-imap` + `sync.rs` sur ces chiffres) :
- **`changes_since` n'est pas implémenté** (`Ok(None)`, « optimisation
  à venir » — report Phase 1) : chaque dossier paye à chaque cycle le
  repli intégral `SELECT` + `UID SEARCH ALL`, même les archives que
  rien ne touche. Corollaire latent : **les drapeaux ne se resynchronisent
  jamais** (un mail lu au téléphone reste non-lu ici).
- **`folders()` est rejoué DANS chaque `SyncEngine::sync`** : ~51 LIST
  par compte et par cycle pour la même arborescence.
- La garde d'espace paye déjà un STATUS par dossier — **jamais réutilisé**
  pour décider si le dossier a bougé.
- Les phases se dégonflent d'un cycle à l'autre (1 361 → 660 s à
  commandes identiques) : **bridage Gmail probable** après l'intégrale —
  le VOLUME de commandes est lui-même le problème.

**Défaut structurel consigné au passage : AUCUN timeout de lecture sur
la connexion IMAP** (`ClientBuilder::connect()` brut, `mail-imap`) — un
réseau qui cale en plein FETCH gèle la relève sans fin ni erreur, en
silence. Non corrigé à chaud : un timeout naïf casserait IDLE (lectures
longues par nature) — le **spike E3 doit traiter les deux ensemble**
(cycle de vie de connexion : timeout, keepalive, reprise après veille).

- **Horodatage** : à la fin de chaque cycle réussi, le shell écrit
  `derniere_synchro` (epoch) dans `prefs` (patron exact de `lang`) ;
  l'UI l'affiche au repos : « Tous les messages sont à jour · dernière
  synchronisation il y a N minutes » (« à l'instant » sous 60 s),
  re-rendu toutes les 30 s, `depuis()` dans `quand.js`.
- **Activité de cycle observable** : `SyncActivity` partagé dans
  `AppState` (compte courant, fait/total), mis à jour par
  `sync_inbox`, lu par une commande `sync_activity` ; l'UI sonde à 1 s
  **pendant le cycle seulement**. Statut : « Synchronisation · 2/4 ·
  marie@… ».
- **Priorités re-triées** (sincérité d'abord) : recherche > catégorie ≠
  réception > cycle en cours > intégrale (%) > rattrapages > attente
  d'envoi > échec > repos + horodatage.
- **Barre fine 2 px** au ras supérieur de la barre d'état : déterminée
  quand un % existe (intégrale, rattrapage des corps), indéterminée
  pendant le cycle courant. **Amendement A16** au Système (le prototype
  est muet sur la forme de la progression).
- Toutes les clés neuves aux catalogues **fr et en** (A15).

### E2 (ré-instruite au 3ᵉ terrain) — Le cycle sobre, AVANT le bouton

**GO du Chef Ingénieur le 2026-08-13 (soir). E2a livrée le même soir
(ADR 0017), gate locale verte — terrain CE dû sur la trace.** Le bouton
(ex-E2) glisse en E3, IDLE en E4 : brancher une « passe légère » sur un
différentiel qui coûte 34 s (INBOX seule) serait livrer un bouton cassé.

**E2a — la relève gardée par STATUS** (petit, gain massif) :
- `STATUS (MESSAGES UIDNEXT UIDVALIDITY)` une fois par dossier et par
  cycle — celui que la garde d'espace paye DÉJÀ, enrichi et réutilisé.
- Décision pure `faut_relever(status, état_local)` (TDD) : dossier
  inchangé (3 valeurs égales) → **sauté**. Une arrivée bouge UIDNEXT,
  une suppression bouge MESSAGES ; seuls les drapeaux glissent — ce
  qu'ils font déjà aujourd'hui, sans CONDSTORE.
- `folders()` sorti de `SyncEngine::sync`, rejoué UNE fois par compte.
- Gate chiffrée (trace existante, sa boîte) : **cycle au repos < 60 s
  sur le compte Gmail** (contre 38 min), INBOX inchangée < 5 s.

**E2b — CONDSTORE réel** (le report Phase 1 arrive à échéance) —
**État : livrée le 2026-08-13 (gate locale verte), terrain CE dû —
drapeau posé au téléphone, reflété au cycle suivant, cycle au repos
inchangé.** HIGHESTMODSEQ au SELECT et au STATUS, `faut_relever` étendu
(serveur muet = sobriété conservée ; base héritée = une relève de
convergence) ; l'inventaire d'UIDs ne se paye plus que si le décompte
l'exige — même quand le dossier bouge, un drapeau seul ne coûte que le
delta :
- `changes_since` implémenté (`UID FETCH … CHANGEDSINCE`), HIGHESTMODSEQ
  relevé au STATUS : les dossiers qui ont bougé payent le delta, plus
  l'inventaire ; **et les drapeaux se resynchronisent enfin** (mail lu
  au téléphone → lu ici).
- Gate : drapeau posé depuis un autre client, reflété au cycle suivant ;
  cycle au repos inchangé (< 60 s).

### P0 — Le filet de sécurité du cycle (audit 2026-08-13, AVANT E2b)

**État : livrée le 2026-08-13 (gate locale verte), terrain CE dû —
câble coupé en plein cycle.** Valeurs livrées : TCP 30 s,
lecture/écriture 120 s. La connexion est construite à la main dans
`mail-imap` (le `ClientBuilder` de la crate ne borne rien), STARTTLS
négocié sur la socket avant TLS — jamais de session en clair, tenu par
tests (serveur muet → erreur en 0,2 s ; STARTTLS refusé → erreur
franche). Watchdog UI avec jeton de cycle : la fin tardive d'un cycle
déclaré mort ne peut plus toucher l'état d'un cycle relancé.

**L'audit du 2026-08-13 a nommé la conséquence du défaut consigné au
1ᵉʳ terrain (« AUCUN timeout de lecture ») : l'arrêt permanent et
silencieux de la synchronisation.** Un FETCH qui cale — réseau tombé en
plein cycle, veille en pleine relève — gèle le thread bloquant sans
fin : la promesse `sync_inbox` ne se résout jamais, le `finally` de
`synchroniser` ne s'exécute pas, `enSynchro` reste vrai, et la garde de
réentrance saute TOUS les cycles suivants. Plus aucune relève jusqu'au
redémarrage, zéro erreur — pendant que la barre annonce un cycle
éternel (`FinDeCycle` ne droppe jamais : le thread n'avance plus). Sur
un portable refermé en plein cycle, c'est un scénario d'usage, pas un
cas d'école. Et le report du timeout au spike IDLE ne tient pas d'ici
là : il n'y a pas encore d'IDLE à casser.

- **Timeouts de lecture et d'écriture** (60–120 s) posés sur la socket
  à la connexion (`mail-imap`) — provisoire assumé, re-instruit au
  spike E4 (IDLE vit de lectures longues par nature). Un réseau qui
  cale devient une erreur ordinaire : rapportée au bilan, retentée au
  cycle suivant — jamais un gel.
- **Watchdog côté UI**, pour ce que le timeout ne voit pas : activité
  (`sync_activity`) et avancement (`sync_progress`) immobiles N minutes
  avec `enSynchro` vrai → cycle déclaré mort, garde réarmée, échec
  affiché. N au-dessus du plus long silence légitime mesuré
  (l'intégrale du terrain avance par lots de ~75 s) : 5 min tient. Le
  watchdog ne tue rien (un `spawn_blocking` ne s'annule pas) — il rend
  la main ; c'est le timeout qui achève le thread gelé.
- Gate : réseau coupé en plein cycle → échec visible en < 3 min, cycle
  suivant reparti seul, sans redémarrage.

### P1 — La visibilité par compte (avec ou avant E2b)

**État : livrée le 2026-08-13 (gate locale verte), terrain CE dû —
deux comptes, message neuf sur le premier visible avant la fin de la
relève du second.** Livré en compteur sondé (`SyncShared.courrier`),
pas en événement — R0-S5 tenu ; bulles émises par compte dans
`run_sync`, dès la relève INBOX, l'agrégat global de fin de cycle a
disparu.

**Second constat de l'audit : le courrier relevé n'est visible qu'en
FIN de cycle complet, tous comptes confondus.** `sync_order` met INBOX
en tête précisément pour servir l'écran — puis le résultat attend
l'inventaire, les dossiers, les fils, les brouillons, et les AUTRES
comptes : la liste ne se recharge qu'au retour de `sync_inbox`, et les
bulles partent en agrégat final. La latence perçue n'est donc pas la
cadence de sondage : c'est cadence + durée totale du cycle. E2a
raccourcit le cycle ; P1 fait qu'une seconde de cycle ne soit plus une
seconde de latence.

- Dès la relève INBOX d'un compte soldée, le shell cumule le courrier
  du cycle dans l'activité partagée (`SyncShared.courrier`) ; la sonde
  à 1 s le voit bouger et recharge liste et nav, sans attendre la fin
  du cycle. Un compteur SONDÉ, pas un canal : le port UI reste R0-S5
  (« la progression se lit par sondage ») — l'« événement Tauri » de
  l'audit aurait ouvert un second canal pour rien.
- Les **bulles du compte partent au même moment** (les arrivées ne
  viennent que d'INBOX) : une bulle par compte au plus — l'agrégat
  global de fin de cycle disparaît. Sa raison d'être (limiter la
  nuisance) s'inverse ici : attendre les autres comptes, c'est la
  course perdue contre le téléphone ; et E4 fera de toute façon partir
  les bulles à la passe légère, par compte.
- C'est la moitié du bénéfice d'IDLE, gratuite : la latence perçue
  tombe de « durée du cycle multi-comptes » à « position d'INBOX dans
  le cycle » — quelques secondes. E4 réveillera la passe légère, qui
  alimente le même compteur.
- Gate : deux comptes, message neuf sur le premier — à l'écran pendant
  que le second se relève encore (à l'œil et à la trace) ; bulle émise
  avant la fin du cycle.

### E3 — Le geste manuel (D5 rouverte, ex-E2)

**État : livrée le 2026-08-13 (gate locale verte : e2e bouton +
Réessayer sur le décor, clés fr/en, police 37 glyphes régénérée sans
régression), terrain CE dû — clic → nouveaux messages en secondes,
reprise de veille → relève seule.** D5 soldée à l'Annexe A ; A16 du
journal corrigé (câblé en E3). La passe légère pose l'horodatage
(chaque INBOX vient d'être vérifiée) ; l'échec partiel s'affiche en
alerte (« 1 compte sur 2 injoignable ») via `accounts_failed` au bilan.

- **Passe légère** `sync_inbox_light` : STATUS INBOX de chaque compte,
  relève seulement si ça a bougé (E2a), + `flush_outbox` + bulles —
  pas de balayage des dossiers. C'est elle que le bouton déclenche :
  réponse en secondes, tenue par la gate d'E2a.
- **Bouton** à l'emplacement validé sur maquette (S-D1) ; pendant un
  cycle : glyphe en rotation, bouton inhibé (réentrance déjà gardée) ;
  sur échec : le même bouton devient « Réessayer ».
- Police régénérée avec le glyphe `sync` (patron A12/A14).
- **Annexe A mise à jour** : la ligne D5 passe de « coupée » à
  « rouverte au terrain (2026-08-13), câblée en E3 ».
- **Réveil de veille** (audit 2026-08-13) : au resume, l'utilisateur
  regarde l'écran — c'est LE moment du frais — et attend aujourd'hui
  jusqu'à 5 min, avec le gel P0 en prime si une connexion dormait. La
  reprise déclenche la même passe légère : le geste a trois
  déclencheurs — le bouton, le réveil, puis IDLE (E4). Détection au
  plus simple : un tick de sonde en retard de plusieurs minutes signe
  la veille (saut d'horloge) — aucune API système à câbler.
- **L'échec partiel se dit** (audit 2026-08-13) : `synchroEchec` ne
  s'allume que si TOUS les comptes échouent — un compte mort sur deux
  est invisible, et l'horodatage « à jour » est rajeuni par le
  survivant. Mensonge par omission, exactement le genre qu'E1 corrige :
  la barre alerte dès qu'UN compte échoue (« 1 compte sur 2
  injoignable »), et le bouton devient « Réessayer » pour lui aussi.
  Clés neuves aux catalogues fr et en (A15).

### E4 — Le temps réel (IDLE, ex-E3)

- **Spike jetable d'abord** — `spikes/idle/`, hors workspace, sur les
  trois fournisseurs (Gmail, Microsoft, IMAP générique). Protocole :
  latence arrivée → événement (p50/p95), tenue de connexion 60 min,
  reconnexion après coupure réseau, **veille/reprise Windows**,
  comportement à l'expiration du jeton OAuth. **Gate chiffrée : p50
  ≤ 5 s, p95 ≤ 30 s, reconnexion automatique prouvée sur les trois.**
- **ADR 0017** sur mesures : un veilleur IDLE par compte (thread du
  shell) sur INBOX ; un événement réveille la **passe légère du compte
  concerné** puis un événement Tauri pousse l'UI à recharger liste et
  nav. Où vit `idle` (extension du trait `MailServer` ou capacité
  séparée) : tranché à l'ADR, sur ce que le spike apprend.
- Les bulles partent avec la passe légère → **parité avec le
  téléphone**, sans rien changer à `notify.rs`.
- Le cycle complet reste à 5 min (dossiers, brouillons, différentiel) —
  cadence re-discutée en bêta (S-D4).
- **Budgets re-mesurés** avec veilleurs actifs : RAM (184–187 Mo sur
  200 déjà), démarrage. Un budget cassé = andon.

## 4. Ce qu'on ne fait PAS (PASSATION §2.6)

- **Pas de QRESYNC** — absent chez Gmail, le différentiel d'UIDs reste
  la référence des suppressions. (CONDSTORE, d'abord écarté ici, a été
  ré-instruit en E2b par la mesure du 3ᵉ terrain — la ligne ne vaut
  plus que pour QRESYNC.)
- **Pas de push mobile** (APNs/FCM) — v1 est desktop ; l'ADR 0015 le
  note déjà pour plus tard.
- **Pas de réglage de cadence exposé** — une préférence de plus est une
  surface de confusion de plus ; la bonne cadence est une décision
  produit, mesurée en bêta.
- **Pas de synchro « du dossier ouvert »** — la passe légère + le cycle
  complet couvrent le besoin observé.

## 5. Décisions du Chef Ingénieur

Verdict du 2026-08-13 sur maquettes :

| # | Décision | Verdict |
|---|---|---|
| S-D1 | Emplacement du bouton : barre d'état (droite) ou entête | **Tranchée : barre d'état** (variante A) — le geste vit à côté de l'information qu'il rafraîchit |
| S-D2 | Portée du geste : passe légère ou cycle complet | **Tranchée : passe légère** — répondre en secondes ; le cycle complet a son minuteur |
| S-D3 | Barre visuelle 2 px (A16) ou texte seul (Système actuel) | **Tranchée : barre 2 px** — inscrite au journal du Système (A16) |
| S-D4 | Cadence du cycle complet une fois IDLE actif | Ouverte — **garder 5 min**, re-mesurer en bêta |
| S-D5 | Horodatage global ou par compte | Ouverte — **global** livré en E1 ; le détail par compte attendra un besoin observé |

## 6. Maquettes

`docs/design/maquette-synchro.html` — tous les états de la barre
(repos, cycle, intégrale, rattrapage, attente d'envoi, échec), les deux
variantes d'emplacement du bouton, thèmes nature et nuit. **Rien ne se
code avant le verdict du Chef Ingénieur sur S-D1/S-D2/S-D3.**

## 7. Gates

| Étape | Gate |
|---|---|
| E1 | e2e verts (statut + horodatage), clés fr/en complètes, terrain CE : la barre se lit et se comprend sur sa boîte réelle |
| E2a | terrain CE : cycle au repos < 60 s sur le compte Gmail du terrain, INBOX inchangée < 5 s — lisible à la trace (`n dossiers (k sautés)`) |
| P0 | réseau coupé en plein cycle → échec visible en < 3 min, cycle suivant reparti seul, sans redémarrage |
| P1 | deux comptes, message neuf sur le premier : à l'écran avant la fin de la relève du second ; bulle émise avant la fin du cycle |
| E2b | drapeau posé depuis un autre client reflété au cycle suivant ; cycle au repos inchangé (< 60 s) |
| E3 | terrain CE : clic → nouveaux messages visibles en secondes ; reprise de veille → relève partie seule ; un compte en échec sur deux → alerte visible ; D5 soldée à l'Annexe A ; police régénérée sans régression (32+1 glyphes) |
| E4 spike | p50 ≤ 5 s, p95 ≤ 30 s, reconnexion prouvée (coupure + veille/reprise), trois fournisseurs |
| E4 | terrain CE : bulle téléphone et apparition Discovery « en même temps » (< 30 s constaté) sur compte réel ; budgets re-mesurés (RAM, démarrage) ✅ |

La ligne s'arrête quand une gate casse — c'est elle qui commande.
