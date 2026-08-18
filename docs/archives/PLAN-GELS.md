# PLAN-GELS — la fenêtre ne gèle plus : aucune commande bloquante sur le thread principal

**CHANTIER SOLDÉ le 2026-08-15 — terrain complet le jour même.** GO CE
sur les quatre décisions (D1-D4, toutes les recommandations, §5) ;
livraison intégrale en un commit (`e32280b`, A39/A40) après revue à
regard neuf (§4bis) et un constat terrain de passe 1 — le trait
hitofude figé — instruit et corrigé le jour même, deux racines (§6).
Passe terrain 2 validée par le CE (« Ok pour les 3 »), CI verte.
Preuves : 25,2 s de gels cumulés → zéro (décors A/B), zéro gel sur
60 s sur copie de la vraie base (4,75 Go), dashoffset du trait en
mouvement pendant un cycle réel. Décision gelée par l'ADR 0019.

> Bug rapporté le 2026-08-15 : « Freeze de plusieurs secondes de Wind au
> démarrage en ligne de commande. L'application ne répond pas au clic et
> la fenêtre ne peut pas être agrandie ou déplacée. »

## 1. Constat — mesuré le 2026-08-15, application release réelle

Le symptôme « fenêtre indéplaçable, clics ignorés » est la définition
Windows de *Ne répond pas* : la **pompe de messages du thread principal
est bloquée**. Or dans Tauri 2, **toute commande déclarée sans `async`
s'exécute sur le thread principal** — celui de la pompe. Chaque commande
synchrone qui ouvre la base gèle donc la fenêtre pour toute sa durée.

Mesure : sonde `SendMessageTimeout(WM_NULL)` à ~100 ms sur la fenêtre
(`sonde-fenetre.py`, scratchpad), application release lancée en ligne de
commande — le geste exact du constat — sur une base réelle de 251 062
enveloppes / 17 761 corps (~69 ko/corps), compte factice hors ligne
(crochet e2e). Deux décors :

| Décor | Gels > 150 ms sur 40 s | Cumul | Pire gel |
|---|---|---|---|
| **A** — 17 761 aperçus NULL (l'état que produit la réparation `apercus-entites`) | 16 | **25,2 s** | **4,6 s** |
| **B** — témoin, aperçus intacts | 7 | 5,2 s | 1,8 s |

Attribution, chronométrée en SQL direct sur le décor B :

| Commande (synchrone → thread principal) | Coût mesuré | Quand elle frappe |
|---|---|---|
| `preview_catchup(2000)` | 1,26 s de lecture seule (130 Mo) + parse Rust + 2 000 UPDATE + COUNT → **2 à 4,6 s par lot**, en boucle jusqu'à épuisement du stock | t+1,5 s puis tous les 250 ms — c'est le gel « plusieurs secondes » du constat |
| `nav_snapshot` (compteur Archives d'une intégrale Gmail, exclusion par `message_id`) | **865 ms** par compte à 87 k lignes | démarrage, puis toutes les 10 s |
| `backfill_status` → `pending_total` (boucle COUNT par boîte, NOT EXISTS sur `bodies`) | **575 ms** (233 301 en attente) | t+3 s, puis à chaque génération de courrier |
| `sync_progress`, compteurs simples | 14–73 ms | toutes les 5–10 s |

Les trois premiers dépassent à eux seuls le budget « chaque action
répond en moins de 100 ms » — non pas en répondant lentement, mais en
gelant TOUTE la fenêtre pendant qu'ils travaillent. Le décor A explique
le constat du jour : la réparation `apercus-entites` (commit récent) a
remis à NULL un stock d'aperçus sur la base réelle, redonnant à
`preview_catchup` des lots pleins à mâcher sur le thread principal.

**La racine n'est pas le coût des requêtes : c'est leur PLACE.** Un
travail de 865 ms est acceptable sur un thread de fond ; il est
inacceptable sur la pompe de messages. (Modèle A38 : la ceinture — des
requêtes moins chères — n'aurait pas fermé la classe ; la racine — plus
aucune commande bloquante sur le thread principal — la ferme.)

## 2. Périmètre

**Fait** : toute commande Tauri qui ouvre la base, touche un fichier ou
le keyring passe hors du thread principal (attribut `async`). Une garde
de gate empêche la régression. Le lot de `preview_catchup` est
redimensionné sur mesure.

**Refusé, et pourquoi** :
- **Optimiser `nav_snapshot` (865 ms) et `pending_total` (575 ms)** :
  une fois hors de la pompe, ils ne gèlent plus rien ; leur coût CPU
  périodique est réel mais n'a plus d'effet visible. Les optimiser
  maintenant serait du travail sans constat — consigné en dette (famille
  D-7, chronos joints), à rouvrir si une mesure terrain le désigne
  (ventilateur, batterie, contention d'écriture).
- **Un cache des compteurs de nav** : même raison — pas de constat une
  fois la pompe libre.
- **Paralléliser ou accélérer le rattrapage des aperçus** : sa lenteur
  est sans importance une fois invisible ; il converge puis se tait.

## 3. Options et verdict

| Option | Verdict |
|---|---|
| **O1 — `#[tauri::command(async)]` sur toutes les commandes bloquantes** | **Retenue.** Une ligne d'attribut par commande, aucune signature ni UI changée, ferme la classe entière. Tauri exécute ces commandes sur le pool de l'async runtime, la pompe reste libre. |
| O2 — Ne basculer que les 3 coupables mesurés | Écartée : la classe reste ouverte — la prochaine commande synchrone un peu lourde regèlera la fenêtre. Le §9 de PASSATION est plein de classes fermées trop tard. |
| O3 — Réduire le coût des requêtes sans les déplacer | Écartée : ceinture sans racine. 100 ms de budget ne se tiendront jamais sur la pompe pour des COUNT à l'échelle 256 k. |

O1 se prouve par la mesure de gate (sonde de pompe, décor A) — pas
d'avis, un chiffre avant/après.

## 4. Étapes

- **E1 — La garde d'abord (RED)** : un script de gate
  (`e2e/garde-thread-principal.mjs`, même famille que
  `coherence-systeme.mjs`) balaie `apps/desktop/src/*.rs` : toute
  fonction `#[tauri::command]` non-async dont le corps atteint
  `Store::open` / fichier / keyring est un rouge. Il échoue sur le code
  actuel — c'est le RED honnête de ce chantier (un test unitaire Rust ne
  peut pas observer le placement de thread du runtime Tauri).
- **E2 — La bascule (GREEN)** : attribut `async` sur toutes les
  commandes désignées par la garde ; les commandes pures d'état
  (`sync_activity`, `migration_progress`, `app_version`, `open_link`,
  `reseau_etat`…) restent synchrones — des atomiques, rien à gagner.
  Gate : garde verte, suite e2e complète verte.
- **E3 — Le lot d'aperçus sur mesure** : 2 000 corps = 130 Mo par lot ;
  redimensionné (500, à confirmer par mesure de durée de lot) pour que
  chaque transaction d'écriture reste courte — le verrou d'écriture
  protège les gestes UI concurrents (leçon du BUSY de `delete_draft`,
  terrain 2026-08-15).
- **E4 — La preuve (gate chiffrée)** : re-mesure de la sonde sur le
  décor A : le cumul de gels doit passer de 25,2 s à ~0 (aucun gel
  > 150 ms après l'apparition de la fenêtre). L'outil de sonde entre au
  dépôt selon D3.
- **Documentation** : PASSATION §7.1 (piège : commande Tauri non-async
  = thread principal) et §9 (enseignement) ; dette D-7 enrichie des
  chronos `nav_snapshot`/`pending_total` ; mémoire persistante.

## 4bis. Livraison — 2026-08-15

- **E1 ✓** : `e2e/garde-thread-principal.mjs`, règle inversée (toute
  commande est `async` sauf exemption nommée de pures d'état — une
  liste de marqueurs raterait la commande bloquante au travers d'une
  aide, `queue_removal`). RED montré : 34 commandes fautives.
  `telemetry_selftest_panic` exemptée : elle ne bloque pas, elle
  panique — et l'ADR 0014 a validé le double-panic du thread principal.
  Câblée : pre-push [4/7], CI, /gate.
- **E2 ✓** : 34 commandes basculées en `async fn` (30 `commands.rs`,
  4 `telemetry.rs`), garde verte, compilation propre. Aucune signature
  ni ligne d'UI changée.
- **E2-bis ✓ (revue à regard neuf, même jour)** : la revue
  `/code-review high` a montré que `async` seul ne suffisait pas — le
  corps bloquant épinglait un worker tokio (le runtime Tauri n'a pas de
  pool bloquant : workers = cœurs), et la SÉRIALISATION qu'offrait le
  thread principal disparaissait (paires état-local/file-d'actions de
  `mark_flagged`, TOCTOU `save_draft`/`delete_draft`,
  `SQLITE_BUSY_SNAPSHOT` hors `busy_timeout`). Remède au même niveau
  que le constat : `hors_pompe()` — `spawn_blocking` + verrou global
  des commandes (`AppState.commandes`) — enveloppe les 34 corps : la
  pompe reste libre, les commandes restent une à la fois, comme avant.
  Corrections associées : garde de génération sur `chargerNav`
  (App.svelte, motif Liste) ; garde durcie (attributs paramétrés,
  `pub(crate)`, chiffres, comptage croisé attributs/prises, zéro prise
  = rouge, `process.exitCode`) ; sonde durcie (sortie drainée et
  recrachée à l'échec, processus mort ≠ gels, argtypes/restype user32,
  durée nulle refusée, kill + wait) ; `coherence-systeme.mjs` entre au
  pre-push ([4/8] — l'écart préexistant est clos au passage).
- **E3 ✓** : lot `preview_catchup` 2 000 → 500 (App.svelte).
- **E4 ✓** : sonde versionnée (`e2e/sonde-gel.py`, budget « aucun gel
  > 150 ms », sortie non nulle sinon). **Preuve sur le décor A
  restauré : 25,2 s de gels cumulés → zéro gel > 150 ms sur 40 s**,
  et le rattrapage a continué de travailler hors de la pompe
  (12 000 aperçus recalculés pendant la mesure, 24 lots de 500).
- Documentation : Système A39 (DC-D2, même commit), PASSATION §3
  (budget pompe), §7.1 (piège), §7.3 (sonde aux mesures, prérequis
  Python), §8 (dette D4), §9 (enseignement).
- **Gate complète du 2026-08-15, verte** : fmt ; build ui-v2 zéro
  avertissement ; contrastes ; cohérence (119 valeurs) ; garde du
  thread principal (62 commandes vérifiées) ; clippy muet ; 421 tests
  Rust ; **72/72 e2e, pleine suite, sans flake**. Re-preuve sonde
  après `hors_pompe` : zéro gel > 150 ms sur 40 s, décors A et B, et
  le rattrapage avance toujours (15 000 aperçus recalculés pendant la
  mesure A).

## 5. Décisions CE

- **D1 — Étendue de la bascule** : toutes les commandes bloquantes
  (recommandé, ferme la classe) ou seulement les trois mesurées ?
- **D2 — Taille du lot `preview_catchup`** : 500 (recommandé, mesure de
  confirmation en E3) ou garder 2 000 une fois hors de la pompe ?
- **D3 — L'outil de sonde au dépôt** : versionner la sonde de pompe
  (`e2e/sonde-gel.py`) avec un budget PASSATION « aucun gel de pompe
  > 150 ms » (recommandé — un gel redevient mesurable au premier
  aller-retour), ou la laisser en outil de session ?
- **D4 — La dette des requêtes chères** : consigner `nav_snapshot`
  865 ms et `pending_total` 575 ms en dette D-7 sans les optimiser
  (recommandé), ou ouvrir un chantier d'optimisation maintenant ?

*Réponses CE, consignées le 2026-08-15 :*
- **D1** : « Toutes les bloquantes » — la bascule couvre toute commande
  qui ouvre la base, touche un fichier ou le keyring.
- **D2** : « 500, confirmé par mesure » — le lot de `preview_catchup`
  passe à 500, durée de lot mesurée en E3.
- **D3** : « Versionner + budget » — la sonde entre au dépôt
  (`e2e/sonde-gel.py`), budget PASSATION « aucun gel de pompe
  > 150 ms ».
- **D4** : « Dette D-7, chronos joints » — `nav_snapshot` 865 ms et
  `pending_total` 575 ms consignés sans optimisation.

## 6. Terrain — passe 1 du 2026-08-15

Verdict CE : points 1 (réactivité au lancement) et 2 OK ; **constat** :
« Lors d'une synchronisation, le trait hitofude reste fixe alors qu'il
devrait être animé. » Instruit le jour même sur copie de la vraie base
(4,75 Go, C:\Temp) — deux racines (A40) :

1. **L'avancement figé à 99 % par les départs en attente** : les 6
   manquants du dénominateur = exactement les 5 `archive` + 1 `delete`
   de `pending_actions` (les gestes de triage du CE pendant sa passe !).
   Le geste retire la ligne locale (écho E3), `remote_total` date du
   dernier SELECT → `sync_percent` < 100 → trait « plein », immobile.
   Remède cœur : `sync_progress` ajuste le dénominateur des retraits en
   instance (archive, delete, move_to ; jamais les marquages ; borne à
   zéro par boîte). RED→GREEN :
   `un_depart_en_attente_ne_compte_plus_dans_le_denominateur`, par le
   VRAI chemin du geste (`geste_avec_echo`). L'écart résiduel de 1
   (compte 2, sans action en attente) est du transit en vol —
   `faut_relever` force la relève sur tout écart de compte, il ne peut
   pas devenir permanent.
2. **La boucle `vague` morte-née** : animation CSS d'un chemin DANS le
   `<mask>` — sous-arbre non rendu, Chromium n'y fait pas tourner les
   animations CSS. Prouvé au CDP sur la vraie fenêtre : `playState:
   idle`. Remède : tracé en SMIL (`<animate>`), fondu inchangé (chemin
   rendu), `prefers-reduced-motion` tenu à la main (SMIL ignore le bloc
   CSS A8). Preuve après remède : dashoffset **en mouvement** (16,3 →
   29,7 px pendant la fenêtre d'un cycle réel), horloge SMIL en marche,
   fondu `running`. Un e2e neuf tient la présence de l'`<animate>` dans
   le trait de la barre (spec ecran02).

Gate rejouée après les deux remèdes : fmt, build ui-v2 zéro
avertissement, contrastes, cohérence, garde (62 commandes), clippy
muet, 422 tests Rust (le test du dénominateur en plus), **73/73 e2e
pleine suite** (le test hitofude en plus — après neutralisation des
runs concurrents de la session sœur sur le port CDP ; les échecs
intermédiaires étaient le flake brouillon fantôme documenté 0956c85,
52/52 en isolation). **Point 2 du CE : sonde de 60 s sur COPIE DE LA
VRAIE BASE (4,75 Go, C:\Temp) — zéro gel > 150 ms.**
