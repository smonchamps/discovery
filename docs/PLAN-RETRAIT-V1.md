# Plan de retrait de l'interface v1

Le dernier acte de la refonte (PLAN-UI-V2 §P5, « gate de bascule »).
Méthode shusa : chaque étape a sa gate chiffrée, la ligne s'arrête quand
une gate casse, et le retour arrière reste UNE ligne jusqu'au retrait
final.

## 0. Le fait qui commande tout le plan

**v1 n'est pas qu'un écran : c'est la colonne vertébrale d'exécution.**
Relevé du 2026-08-11 (`apps/desktop/ui/app.js`) — cinq flux que SEULE
v1 déclenche aujourd'hui :

| Flux | Déclencheur v1 | v2 aujourd'hui |
|---|---|---|
| Reconnexion des comptes | `connect_accounts` à l'init | **jamais appelé** |
| Synchronisation | `sync_inbox` à l'init (`onConnected` → `refresh`) + bouton | **jamais appelé** (v2 ne fait que lire `sync_progress`) |
| Brouillons distants (tirer/pousser) | `sync_drafts` après synchro et à la fermeture d'édition | **jamais appelé** |
| Boîte d'envoi (retenter au retour du réseau) | `flush_outbox` après chaque synchro | seulement après un envoi utilisateur |
| Rattrapage des corps | auto-démarré après la première synchro | porté en P5 (ligne de progression) ✓ |

Supprimer `apps/desktop/ui/` sans porter ces flux livrerait un client
qui ne relève plus le courrier. D'où l'ordre : **R1 (autonomie de v2) →
R2 (parcours portés) → B1 (bascule de défaut, réversible) → observation
→ B2 (retrait du code)**.

## Préalables (avant R1 — ce sont les gates déjà écrites au plan v2)

- [ ] Terrain P4 + P5 passé par le Chef Ingénieur sur sa base réelle.
- [ ] **Annexe A signée** ligne à ligne (PLAN-UI-V2) — notamment D2
      coupée en connaissance.
- [ ] Chantier charset U+FFFD soldé ou explicitement découplé (il touche
      le cœur, pas les UI ; il ne bloque pas la bascule sauf verdict
      contraire).

## R1 — v2 devient autonome (le vrai chantier)

**Objectif :** tout ce que v1 déclenche, v2 le déclenche — selon D5 :
synchro AUTOMATIQUE, sans bouton.

**État : livré (GO du 2026-08-11), gate terrain aux deux tiers** — le
témoin (envoi réel resté en file) est **parti seul** à la première
passe (`sent` en base) et sa copie a été **tirée dans Envoyés** par le
cycle automatique, constaté par le Chef Ingénieur le jour même. Deux
incidents de mise en route consignés : la configuration OAuth
(`GOOGLE_CLIENT_ID`/`SECRET`) n'était posée nulle part — posée au
niveau utilisateur conformément à PASSATION §7.2, en se souvenant
qu'un terminal déjà ouvert ne voit jamais les variables nouvelles ; et
un gel de ~10 s au premier lancement d'un binaire fraîchement
recompilé (analyse Defender probable — à confirmer : il ne doit PAS se
reproduire au second lancement du même binaire). **Gate close le 2026-08-11** : mail entrant arrivé sans action ✓
(terrain CE) ; brouillon reflété ✓ (poussée immédiate à la fermeture
ajoutée — elle n'était que dans le cycle) ; budgets re-mesurés avec le
cycle actif sur copie de la base réelle (2 942 conversations,
2 passes à chaud) : démarrage 578-649 ms ✓, première page 94-99 ms ✓,
page p95 9,2-9,7 ms ✓, thème p95 0,4 ms ✓, RAM 184-187 Mo ✓,
ouverture p50 ~14 ms ✓ et **p95 52-55 ms** — 2 à 5 ms au-dessus du
budget, porté par UN corps > 1 Mo de l'échantillon déterministe (la
base en compte 207, jusqu'à 28 Mo) : coût d'assainissement du courrier
réel, pré-existant à R1, mesuré identique avec et sans le cycle. Écart
soumis à l'arbitrage du Chef Ingénieur ; piste si refusé :
assainissement paresseux des très gros corps, chantier séparé. Choix
d'implémentation actés : l'échec TOTAL de synchro vit dans la ligne de
progression (« Synchronisation impossible — nouvelle tentative
automatique »), pas dans la fente — §6 n'y met pas la synchro, et
« hors ligne » n'est pas un incident ; l'échec de RECONNEXION, lui, est
un avis (« Compte non reconnecté — raison », Réessayer/Ignorer), inséré
sous l'échec d'envoi dans la priorité.

**Livré :**
1. **Démarrage** : après la modale de migration, `connect_accounts`
   (reconnexion silencieuse par le coffre). Échecs partiels → fente
   d'avis (« compte non reconnecté — raison »), pas un statut muet.
   L'onboarding reste la porte à zéro compte.
2. **Cycle de synchronisation automatique** : `sync_inbox` au démarrage
   (après reconnexion), puis périodique — proposition : **toutes les
   5 min**, plus une passe immédiate au retour d'un ajout de compte.
   La ligne de progression existante montre l'avancement ; le bilan
   d'erreurs de synchro alimente la fente d'avis s'il est total
   (aucun compte joignable = information, pas silence).
3. **Après chaque synchro réussie**, la séquence v1 conservée à
   l'identique : `sync_drafts` (tirer), `flush_outbox` (le réseau est
   peut-être revenu — règle d'or : la file retente sa chance),
   `sync_drafts` (pousser), rattrapage des corps si nécessaire.
4. **Brouillons** : à la fermeture d'une composition qui a conservé un
   brouillon, `sync_drafts` en poussée discrète (le reflet Gmail de v1,
   sans lequel le dossier Brouillons de la nav reste vide).
5. Aucune commande cœur nouvelle : tout existe déjà.

**Gate R1 :** sur la base réelle du Chef Ingénieur, v2 SEULE (v1 jamais
ouverte) : un mail reçu sur le serveur apparaît sans action manuelle en
≤ 5 min ; un envoi hors ligne part au retour du réseau sans action ;
un brouillon poussé est visible dans Gmail. Budgets P1 re-mesurés
inchangés (la synchro périodique ne doit pas dégrader page p95 ni RAM).

## R2 — les 21 parcours v1 portés ou soldés

Inventaire e2e du 2026-08-11 : `parcours-critiques` (12),
`recherche` (6), `multi-comptes` (2), `compte-generique` (1) = 21,
joués sur l'UI v1 par `launchApp`. Destins :

| Parcours v1 | Destin |
|---|---|
| Envoi, brouillon, conflit d'édition, boîte d'envoi (requeue/abandon), pièces jointes, migration | **à porter** sur v2 (les surfaces existent : composition, fente d'avis, modale) — mêmes seeds, testids v2 |
| Recherche (6 : accents, sujets, bornes) | **à porter** sur la recherche D1 (le décor `seed_inbox` reste) |
| Multi-comptes (pastilles, sélecteur De) | **remplacés/câblés** : la nav Boîtes v2 couvre les pastilles ; le sélecteur De est câblé en v2 (A10, verdict terrain) — porter le parcours « choisir le compte émetteur » |
| Étoile / Déplacer | **soldés par D2** (coupés) — les parcours tombent avec, ligne motivée |
| Compte générique (ajout IMAP réel contre serveur de test) | **à porter** sur l'écran 01 (le guichet générique existe) |

**Trou découvert au portage (2026-08-11) — soldé le jour même** : v1
offrait « ajouter un compte » à tout moment ; v2 n'avait aucune porte
d'ajout dès qu'un compte existait. Arbitrage CE : section « Comptes »
dans Réglages (rangées des comptes réels + « Ajouter un compte » qui
déplie LE guichet de l'écran 01, implémentation partagée). Amendement
A11 au journal du Système.

**Abandons de portage motivés** (consignés en tête de
`refonte-parcours-v1.spec.js`) : étoiler/déplacer (D2) ; auto-avance
après archivage (le prototype ferme le volet, écart A6) ; distinction
au corps de deux brouillons locaux (le bandeau-liste v1 n'existe plus —
l'équivalent v2 est le dossier Brouillons après reflet, parcours
terrain).

**Gate R2 :** la suite e2e v2 couvre chaque parcours « à porter » ;
`npm test` complet vert AVEC les specs v1 encore en place (les deux
suites coexistent jusqu'à B2).

## B1 — bascule de défaut (réversible en UNE ligne)

**État : basculé le 2026-08-12** (GO du Chef Ingénieur, Annexe A signée
le même jour — D2 « OK = accord » sur la coupe). `frontendDist` →
`ui-v2/dist` ; l'échange de conf change de main (`construireV1` pour
les parcours d'observation v1, `construireV2` ne garde que la taille de
fenêtre du banc) ; le job CI `quality` construit ui-v2 avant tout cargo
(`generate_context!` exige le dossier). Budgets de clôture : ceux de la
gate R1 (même binaire, cycle actif — voir R1). Le banc v1 `mesure.mjs`
est gelé (il embarquerait v2) : `mesure-v2.mjs` est l'outil, `mesure.mjs`
tombe à B2. **Le compteur des deux semaines sans défaut critique court
depuis le 2026-08-12.**

1. `tauri.conf.json` : `frontendDist` → `../ui-v2/dist` définitivement ;
   le build embarque le `npm run build` de ui-v2 (étape déjà dans le
   hook [2/6] et la CI).
2. `e2e/rebuild-v2.mjs` : l'échange de conf disparaît (il ne reste que
   la taille de fenêtre pour le banc de parité et la purge de cache).
3. Mesures de clôture : `mesure-v2.mjs` sur base réelle (démarrage,
   ouverture, page, RAM) consignées dans la revue.
4. **v1 reste dans le dépôt, dormante** — le retour arrière est la
   ligne de conf, rien d'autre.

**Gate B1 :** budgets verts consignés + le Chef Ingénieur utilise v2 au
quotidien. **Le compteur des deux semaines sans défaut critique
commence ICI** (un défaut critique = perte ou fantôme de courrier, gel,
sécurité — pas un pixel).

## Observation — deux semaines

- Andon ouvert : tout défaut critique arrête le compteur, se corrige,
  et le compteur repart de zéro (PASSATION §2.6).
- Aucun retrait de code pendant la fenêtre.

## B2 — retrait du code v1

Dans CET ordre (chaque pas compile et passe la gate avant le suivant) :

1. **`apps/desktop/ui/` supprimé** (index.html, app.js, styles).
2. **e2e** : specs v1 supprimées (`parcours-critiques`, `recherche`,
   `multi-comptes`, `compte-generique`), `launchApp` v1 retiré de
   `launch.mjs` (`seed_inbox` RESTE : les parcours portés R2 l'utilisent),
   `mesure.mjs` v1 retiré au profit de `mesure-v2.mjs`.
3. **Commandes orphelines** — retirées SEULEMENT si plus aucun appelant
   (UI, e2e, bancs, exemples) : `list_messages`, `startup_report`,
   `sync_inbox` en revanche RESTE (R1 l'appelle), et les commandes D2
   (`mark_flagged`, `move_message`, `list_folders`) **restent au cœur**
   comme signé à l'Annexe A — on retire leurs seuls câbles v1.
   `main.rs` : registrations ajustées.
4. **Hook/CI** : rien à retirer ([2/6] et [3/6] restent) ; la CI
   n'exécutait pas les e2e (ADR 0005).
5. **Docs** : PLAN-UI-V2 §P5 clos ; **`docs/PHASE-REFONTE.md`** — la
   revue de clôture exigée par la gate (livré vs plan, budgets mesurés,
   écarts assumés A6-A9, enseignements, reports, GO/NO-GO signé) ;
   PASSATION mise à jour (v2 = l'UI, v1 n'existe plus) ; ce plan marqué
   soldé.

**Gate B2 :** gate pré-push complète verte (fmt, build ui-v2,
contrastes, clippy, tests Rust dont exemples, e2e entièrement v2) ;
`grep -r "apps/desktop/ui/"` ne trouve plus que l'historique des docs.

## Refus explicites (PASSATION §2.6)

- **Pas de retrait des commandes cœur D2** : coupées à l'écran, pas du
  noyau — réversibles par spéc courte.
- **Pas de réécriture d'historique git** ; v1 reste lisible dans
  l'historique.
- **Pas de retrait du prototype ni du Système** (`docs/design/`) : ce
  sont les références normatives de v2, pas des restes de v1.
- **Pas de multi-fenêtre, Cc/Cci, pièces jointes à l'envoi** dans ce
  plan : reports déjà actés à l'Annexe A, inchangés.

## Estimation et ordre de grandeur

R1 est le seul vrai chantier (une séquence de démarrage et un cycle
périodique dans `App.svelte`, zéro commande neuve) ; R2 est du portage
mécanique de specs ; B1 est une ligne de conf plus des mesures ; B2 est
de la suppression gardée par des gates. Le chemin critique est le
calendrier (deux semaines d'observation), pas le code.
