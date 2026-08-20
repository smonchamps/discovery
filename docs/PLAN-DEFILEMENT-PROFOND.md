# PLAN-DEFILEMENT-PROFOND — le drag de la barre ne doit plus affamer l'application

> **CHANTIER SOLDÉ le 2026-08-20 — terrain complet.** Commit `70e44e3`
> (main), CI verte (run 32382945877). GO CE du plan (STOP 1) :
> 2026-08-20 (D1 : coût O(offset) assumé ; D2 : banc versé au dépôt).
> Terrain (STOP 2) : TROIS passes le même jour — la panne du drag morte
> à la première, puis deux constats de premier affichage corrigés dans
> la session (2-3 s à l'arrêt du geste, 3-5 s au démarrage/premier
> clic Archives → racine : les comptages sur le chemin d'affichage) ;
> troisième passe « OK au terrain ». Journal Système **A64**. Revue à
> regard neuf : 10 trouvailles confirmées, corrigées. Reports : DETTE
> **D-26**. e2e : 94 → **97**. Version : CORRECTIF (0.2.1 au moment de
> publier).
>
> Chantier ouvert le 2026-08-20 (`/chantier`), sur constat terrain du
> CE : dans Archives, un défilement rapide à la barre (clic tenu)
> jusqu'à ~1/3 de la liste laisse des blocs « .. », puis la liste se
> vide ; ensuite TOUS les dossiers disent « Aucun message ici. »
> pendant plusieurs minutes, avant un retour spontané à la normale.
>
> Comportement attendu (énoncé CE) : les messages s'affichent ou sont
> **marqués en chargement** ; la vue ne se vide jamais intégralement ;
> un changement de dossier montre les messages du dossier ouvert.

---

## Constat — faits mesurés (2026-08-20, banc `e2e/mesure-defilement.mjs`)

Décor : base seedée 120 000 messages en Archives + 2 000 INBOX, un
compte, app **release**, hors ligne. Reproduction fidèle du symptôme.

1. **La rafale** : un drag tenu de 2 s (~60 évts/s, jusqu'à 1/3 de la
   liste) déclenche **~161 appels `list_category`** (bruit de fond :
   3,3 appels/s). Mécanisme lu dans `Liste.svelte` : l'`$effect` sur
   `debut`/`fin` sert chaque page traversée par chaque position
   intermédiaire du drag ; rien n'annule ni ne coalesce — les pages
   devenues invisibles partent quand même au cœur.
2. **Le coût suit l'offset** : la page de 200 de `category_page`
   (SQL brut, forme exacte, index `idx_envelopes_date`) coûte
   **10 ms à l'offset 0, 66 ms à 10 k, 157 ms à 40 k, 247 ms à 80 k**.
   Le budget « page de liste < 100 ms » (STANDARD §3) est crevé dès
   ~20 k — la promesse O(1) de l'ADR 0008 ne vaut que pour la
   réception (`threads` + `idx_threads_date_globale`) ; les catégories
   par messages paient `LIMIT offset+200` par boîte + tri fusionné.
   (La clause d'exclusion de l'intégrale Gmail, mesurée, coûte peu :
   les sondes `NOT EXISTS` passent par l'index partiel
   `idx_envelopes_message`.)
3. **La sérialisation transforme la rafale en panne** : `hors_pompe`
   (ADR 0019, verrou global voulu) draine la file à ~20 appels/s sur
   cette base chaude — attente bout-en-bout **p50 2,4 s, max 6,2 s**,
   rétablissement à T+9 s. Au terrain (256 k messages, 4 comptes,
   cache froid, drag plus long et plus profond), la même mécanique à
   ~250 ms-1 s la page × 200-400 pages = **les minutes constatées**.
   Pendant ce temps, TOUTES les commandes attendent derrière (pages,
   nav, sondes, gestes).
4. **« Aucun message ici. » est un mensonge d'état** : reproduit — la
   bascule de dossier pendant la saturation montre l'écran vide sur
   une boîte de 2 000 messages. Mécanisme lu : le changement de source
   remet `total = 0` mais ne remet PAS `premierePageMs` (posé une fois
   pour toutes à la première page servie de la SESSION) ; la garde
   `total === 0 && premierePageMs !== null` affiche donc « Aucun
   message ici. » pendant que la page 0 du nouveau dossier attend en
   file. Le « retour à la normale après quelques minutes » du terrain
   est le drainage de la file, pas une réparation.
5. Pendant le drainage, l'écran montre les blocs « .. »
   (`ligne-attente`) — le premier volet du récit terrain. Le « vide
   total » en Archives même est le même mensonge après une bascule
   (retour en haut de liste, `total` remis à 0) ; hors bascule, les
   placeholders sans avatar ni heure en tiennent lieu visuellement.

## Périmètre

**Dans ce chantier** : le front de la liste (`Liste.svelte`) — borner
les requêtes en vol et faire dire la vérité à l'écran vide ; le banc
`mesure-defilement.mjs` versé au dépôt comme gate re-jouable ; e2e des
deux comportements ; Système amendé (DC-D2) ; terrain sur la vraie base.

**Refus de périmètre explicites (STANDARD §2.6) :**
- **Pas de refonte de la pagination du cœur** (`category_page`
  O(offset)) : le patron deux-temps (clés puis hydratation) y est déjà ;
  le mur résiduel est le parcours d'index `LIMIT offset+limit`, sans
  remède simple pour un saut arbitraire multi-boîtes. Une page profonde
  UNIQUE à ~250 ms est vivable quand une seule vole à la fois et que
  l'écran dit « chargement » — voir D1. Consigné en dette si D1 retenu.
- **Pas de touche à `hors_pompe`** (priorités, annulation côté cœur) :
  la sérialisation est un choix mesuré de l'ADR 0019 — on ne rouvre pas
  sans mesure nouvelle ; c'est la rafale qui est le défaut, pas le
  verrou.
- **Pas de redessin des placeholders** (« .. ») : l'énoncé accepte
  « marqués en chargement » ; leur forme actuelle reste.
- **Pas d'optimisation des sondes périodiques** (D-8 existante).

## Options — set-based sur le point dur (borner la rafale)

- **(a) File bornée, dernière fenêtre gagne — recommandée et retenue.**
  Au plus **2** requêtes de pages en vol par liste ; à chaque
  libération, on lance la page la plus utile de la fenêtre **courante**
  (celle de `premier`, puis sa voisine visible), jamais les pages d'une
  position dépassée. Un drag tenu continue d'alimenter l'écran au fil
  du geste (les pages sous le pouce arrivent), la file du cœur reste
  vide, l'arrêt du geste se sert en ~1 aller. Rafale attendue au banc :
  161 → **≤ ~8** ; rétablissement : la latence d'UNE page.
- (b) Debounce d'immobilité (~100 ms) : simple, mais pendant un drag
  lent CONTINU plus rien ne se charge (l'écran reste en « .. » tant
  que le geste dure) — régression vécue par rapport à (a), pour une
  économie identique. Ne bat pas (a).
- (c) Annulation côté cœur (commande d'abandon) : exigerait un canal
  d'annulation à travers `hors_pompe` et l'IPC — complexité cœur pour
  un résultat que (a) obtient en n'ENVOYANT pas les requêtes inutiles.
  Écartée (§2.6).

Le second défaut (écran vide menteur) n'a qu'une forme raisonnable :
l'état « vide » ne peut s'affirmer qu'après une page 0 de la
**génération courante** — `premierePageMs` redevient nul au changement
de source (ou équivalent par génération).

## Étapes

- **E1 — la file bornée** (`Liste.svelte`). RED : e2e qui journalise
  les appels au cœur par la couture existante `__e2eRetenue` (aucun
  code de prod pour compter), joue un drag dense sur un décor seedé et
  constate la rafale actuelle ; GREEN : ≤ 8 appels pour le même geste,
  lignes servies à l'arrêt. La mécanique `recharger`/`allerEtServir`
  (banc P1, stale-while-revalidate de PLAN-REACTIVITE) reste intacte —
  ses e2e existants font foi.
- **E2 — l'écran vide honnête**. RED : e2e — transport retenu
  (`__e2eRetenue` pendante), bascule de dossier : l'écran affiche
  aujourd'hui « Aucun message ici. » sur une boîte pleine ; GREEN :
  jamais ce message tant que la page 0 de la génération courante n'a
  pas répondu — l'attente se montre (squelette), le vide ne s'affirme
  que prouvé.
- **E3 — gate et banc**. `mesure-defilement.mjs` versé au dépôt (si D2
  retenu), rejoué : rafale ≤ 8, rétablissement local en secondes,
  bascule de dossier toujours servie. Gate complète (`/gate`), Système
  amendé (journal A-n : la liste ne demande que ce qu'elle montre ;
  l'état vide ne s'affirme qu'après preuve), ETAT/DETTE mis à jour.
- **E4 — terrain (STOP 2)**. Sur la vraie base : le geste exact du
  constat (drag tenu à 1/3 d'Archives), chiffres attendus : lignes ou
  « .. » pendant le geste, service en ~1 s à l'arrêt (une page
  profonde ~250 ms-1 s), bascule de dossier immédiate et servie, plus
  jamais « Aucun message ici. » sur boîte pleine, plus de minutes de
  panne. Version : **CORRECTIF** (aucune capacité nouvelle,
  STANDARD §2.9) → 0.2.1 au moment de publier.

## § Décisions CE

- **D1 — Le budget de la page profonde hors réception.** La page à
  offset 80 k coûte ~247 ms (cœur seul, base 120 k) : le budget
  « < 100 ms » n'est pas tenable sans refonte de la pagination des
  catégories. Proposition : **assumer** ~250 ms-1 s pour UNE page
  profonde isolée (une seule en vol, écran honnête qui dit le
  chargement), consigner le report « pagination profonde des
  catégories » en DETTE avec ses chiffres, re-mesurer au terrain en
  E4 ; l'alternative est un chantier cœur dédié avant celui-ci.
  → **Réponse CE (2026-08-20) : « Assumer ~250 ms-1 s »** — une seule
  page en vol, écran qui dit le chargement, report en DETTE avec les
  chiffres, re-mesure au terrain en E4.
- **D2 — Le banc au dépôt.** `mesure-defilement.mjs` (155 lignes,
  patron de `mesure-scrollbar`/`mesure-v2`) entre au dépôt comme banc
  rejouable de la gate E3, ou reste un outil jetable de ce chantier ?
  Proposition : **le verser** — c'est lui qui détectera la prochaine
  régression de rafale.
  → **Réponse CE (2026-08-20) : « Le verser au dépôt »** — banc
  rejouable de la gate E3.

**GO CE du plan (STOP 1) : 2026-08-20** — « GO. Le plan est validé tel
quel — l'implémentation TDD commence (aucun code de production n'a été
écrit à ce stade). »

## Verdicts d'étapes

- **E1 — livrée (2026-08-20).** RED montré : cœur muet (transport
  retenu), le drag demandait **10 pages** (attendu ≤ 2) ; GREEN : la
  pompe bornée (`VOL_MAX = 2`, `pomper`/`lancer`/`pageUtile` dans
  `Liste.svelte`) sert la page la plus utile de la fenêtre courante à
  chaque vol libre. Couture `__e2eJournal` posée dans transport.js
  (relevé {commande, départ, arrivée} par appel — miroir de
  `__e2eRetenue`, rien hors e2e). `pending` survit aux changements de
  génération : un vol ouvert garde sa place et repompe en se réglant.
- **E2 — livrée (2026-08-20).** RED montré : transport retenu, la
  bascule de dossier affichait « Aucun message ici. » sur une boîte
  pleine ; GREEN : `totalConnu` (remis à faux au changement de source)
  garde l'affichage du vide — tant que la source n'a pas répondu,
  squelette d'attente (6 rangées « … », `attente-source`).
- **E3 — banc rejoué (2026-08-20, même décor 120 k, release, code
  final après revue)** : rafale du drag **161 → 41** appels (et
  l'écran s'alimente PENDANT le geste — voulu), vols simultanés max
  **2**, attente bout-en-bout p50 **2 408 → 104 ms** (max 6 169 →
  129 ms), lignes servies **0,5 s** après l'arrêt (avant : 9 s
  localement, minutes au terrain), bascule de dossier immédiate et
  servie. Système amendé (**A64**), DETTE **D-26** (D1 : coût
  O(offset) assumé). **Gate complète verte (2026-08-20)** : fmt, build
  ui-v2 sans avertissement, contrastes (700 paires), cohérence
  (476 valeurs), garde thread principal (66 commandes), clippy, tests
  Rust (482), doc, e2e **96 passés** (94 → 96).

### Revue à regard neuf (2026-08-20) — 10 trouvailles, corrigées

Cinq recenseurs indépendants (8 angles), verdicts convergents ;
corrections dans la foulée, spec et banc rejoués verts :
1. **Tempête de relances sur échec** (3 angles) : le `finally`
   repompait la page en boucle microtâche sur toute erreur persistante
   — un échec ne repompe plus, l'essai suivant attend un geste/effet.
2. **Promesse d'une autre source** : `pending` clé par (source, page) —
   un vol étranger n'occulte plus la page de la source neuve
   (`allerEtServir` ne peut plus se régler sans rien servir).
3. **Recharges plus vite que le règlement** : séparation
   `source`/`generation` — un vol de la MÊME source à génération
   antérieure AFFICHE ses lignes (stale-while-revalidate), sa page
   reste dépareillée donc resservie (sinon : squelette à demeure
   pendant le rattrapage des corps, qui recharge par lot).
4. **Bascule derrière deux pages profondes** : la page 0 d'une source
   sans réponse passe devant la jauge (débord borné à un vol).
5. **Statut menteur** : `ontotal(null)` tant que la source n'a pas
   répondu — jamais « Archives · 0 éléments » d'attente.
6. **Décor au bon niveau** : `seed_inbox` inscrit la boîte seedée au
   cache `folders` — le SQL dupliqué (launch.mjs + banc), qui écrasait
   au passage le décor Archivés/Factures de TOUS les comptes, supprimé.
7. **`enVol` doublon** de `pending.size` : supprimé (une seule jauge).
8. **`premierePageMs` figé** à la première source : remis à null par
   source (le statut de démarrage, capturé une fois, ne bouge pas).
9. **Spec durcie** : quiescence prouvée avant la retenue, `≥ 1` contre
   l'assertion creuse, journal nettoyé dans le `finally`.
10. **Doublons** : squelette en snippet partagé, geste du drag partagé
    spec/banc (`geste-defilement.mjs`).
Écarts assumés : le débord de `allerEtServir` (saut délibéré du banc,
une fenêtre au plus) ; la couture `__e2eJournal` qui rend les rejets
« traités » en e2e seulement ; `chipsAvant` au fil du défilement
(préexistant, coût mesurable un jour via D-22 famille bancs).
- **E4 — terrain, première passe (2026-08-20)** : points 1, 2, 5 OK ;
  le mensonge « Aucun message ici. » est mort au terrain. **Trois
  constats** : (3) service à l'arrêt du geste en 2-3 s (attendu ~1 s) ;
  (4) squelette 1-4 s à la bascule ; (6) messages du démarrage à
  > 6 s. **Corrigés le jour même (3, 4)** — racine mesurée sur décor
  intégrale (200 k « Tous les messages » + 20 k INBOX) :
  `category_totals` (COUNT + sonde NOT EXISTS par ligne) coûte
  **~240 ms PAR APPEL**, payé à chaque page servie — la page elle-même
  ne coûte que 14 ms (offset 0) / 129 ms (80 k). Remèdes : **le total
  ne voyage qu'avec la page 0** (`MessagePage.total: Option<u64>`,
  None en profondeur — le front garde le total connu de sa source) ;
  **`VOL_MAX` 2 → 1** (le cœur sérialise : deux vols ne parallélisent
  rien, ils allongent d'une page dépassée l'attente de la page utile).
  Banc rejoué : vols max **1**, bout-en-bout p50 **17 ms** (était
  104 ms), lignes à T+0,5 s ; sur décor intégrale la page profonde
  passe de 368 à **129 ms**. (6) n'est PAS une régression du
  chantier : démarrage au code final **890 ms** (spawn → première
  ligne, banc P1 sur 122 k, première page 50 ms) — c'est le « corps à
  la demande bridé à ~7 s au lancement » du terrain 2026-08-19, déjà
  à l'ETAT, chantier **perf-lecture** dédié. **Re-gate complète verte**
  après remèdes (96 e2e, 2,7 min).
- **E4 — terrain, deuxième passe (2026-08-20)** : le drag et les
  bascules après scroll brutal sont **acceptables au terrain** — le
  bug de l'énoncé est mort. Deux constats de PREMIER affichage
  restants : démarrage ~3 s, premier clic Archives ~5 s (« chargement
  imperceptible » exigé). **Racine mesurée, corrigée le jour même** :
  les comptages vivaient sur le chemin d'affichage — (a)
  `nav_snapshot` recalculait toutes les 10 s HUIT compteurs par compte
  (dont le total d'intégrale, ~240 ms warm la sonde, bien plus à
  froid) pour n'en AFFICHER que deux (A29 : la nav ne dit que le
  non-lu) → `nav_unread_counts`, les deux compteurs affichés seuls,
  parité verrouillée par test ; (b) la page portait son comptage,
  page 0 comprise → la page ne compte plus jamais : une page courte
  dit la fin exacte d'elle-même (les petits dossiers ne paient jamais
  de comptage), `category_total` (commande neuve, hors_pompe) n'est
  demandé que la pompe au repos, la barre de défilement suit le
  plancher des lignes puis s'ajuste au vrai total ; le statut ne dit
  un nombre que prouvé exact. SQL mesuré (décor intégrale 200 k +
  20 k INBOX) : premier affichage d'Archives 253 → **14 ms** de cœur ;
  page profonde 368 → 129 ms. e2e neuf : « les lignes ne suivent
  jamais le comptage » (ordre tenu au journal de transport). Gate
  complète verte : **97 e2e** (2,2 min), 20 cibles Rust dont la
  parité nav.
- **E4 — terrain, troisième passe (2026-08-20) : « OK au terrain. »**
  Démarrage et premiers affichages imperceptibles, drag et bascules
  sans régression. **Terrain complet** — reste commit, push, CI verte.
