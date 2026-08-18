# PLAN-RETOURS-3 — quatre retours terrain (rattrapage %, spam, brouillon, réponse par message)

> **CHANTIER SOLDÉ le 2026-08-18 — terrain complet.** Commit `8819090`
> (`feat: retours 3 …`), CI verte (run 32113377204). GO CE du plan le
> 2026-08-17 (STOP 1, D1-D5 tranchées ci-dessous) ; passe terrain du
> 2026-08-18 : R1-R3 validés d'emblée, **R4 corrigé le jour même** (constat
> terrain : les 3 gestes sur nos propres messages, réponse aux
> destinataires d'origine — A58 amendé, `reply_to` pur). Système amendé
> **A55-A58**. Fait suite à PLAN-RETOURS-2 (soldé, livré en 0.1.9). Prêt
> pour une release **0.1.10** (nouveauté visible ⇒ MINEUR, §2.9) — la
> release reste une décision CE (`scripts/faire-release.ps1`).
>
> Reports assumés à la clôture (§2.6) : **D-21** (double COUNT du corpus
> par lot du rattrapage — famille D-8, budget tenu au terrain) et **D-22**
> (report_spam « déjà spam » atteint via la recherche — cosmétique).

## Constat — instruction sur pièces (genchi genbutsu)

Quatre retours de l'utilisateur, quatre zones distinctes. Le socle
backend est **déjà en place pour trois d'entre eux** ; l'essentiel est du
câblage UI et deux commandes minces. Vérifié dans le code :

- **R1 — % de rattrapage.** La barre d'état affiche
  `Rattrapage des messages · {n} restants`
  (`statut.rattrapageCorps`, `App.svelte:267`). Le backend ne connaît
  **que le reste** : `BackfillStatus { remaining }`
  (`commands.rs:3995`), alimenté par `pending_total` qui somme
  `bodies_pending_count` sur toutes les boîtes en portée `NO_HORIZON`
  (`commands.rs:3858`). **Aucun total, aucun « fait » n'est stocké** :
  un pourcentage exige un dénominateur qui n'existe pas encore.

- **R2 — spam / non-spam.** Tout le mécanisme de déplacement existe :
  `Action::MoveTo(folder)` (`action.rs:28`, journalisée et rejouée),
  `move_message` (`commands.rs:2068`) via `queue_removal`, et surtout la
  **résolution canonique du dossier indésirables par compte** —
  `canonical_folders(compte).boite("indesirables")` (nav.rs, constante
  `INDESIRABLES = [spam, junk, junk e-mail, …]`, déjà testée). Il manque
  deux commandes qui résolvent le dossier Junk et un geste dans l'UI.

- **R3 — supprimer un brouillon.** `delete_draft(id)` existe
  (`commands.rs:3163`). Le pied de la composition porte Envoyer /
  Joindre / Enregistrer le brouillon / **Annuler**, et `fermer()`
  **conserve** le brouillon (ne le jette que s'il est vide,
  `Composition.svelte:343`). Il n'existe **aucun geste destructif**
  volontaire dans la fenêtre de composition.

- **R4 — répondre par message.** Le fil (`Fil.svelte`) porte **une seule**
  barre d'actions, en bas du fil entier (`.actions`, lignes 267-281) :
  Répondre / Répondre à tous / Transférer / Archiver / Supprimer.
  Répondre et Répondre-à-tous visent `cible()` — *le dernier message
  d'autrui du fil* (ligne 48), jamais un message précis choisi. Les
  annotations du Système anticipaient déjà le manque : « le “⋯” par
  message de la maquette attend ses actions » (Fil.svelte:8).

## Périmètre

**Ce qu'on fait.**

1. **R1** — un pourcentage dans le texte de la ligne « Rattrapage des
   messages », cohérent avec la décision A52/D1 (« le % vit dans le
   TEXTE », le trait ne fait qu'une boucle).
2. **R2** — signaler un courrier comme spam (déplacement vers le Junk du
   serveur) et un spam comme légitime (retour en Réception).
3. **R3** — supprimer volontairement le brouillon en cours depuis la
   fenêtre de composition.
4. **R4** — porter les gestes de réponse au niveau de **chaque message**
   du fil, positionnement à trancher.

**Ce qu'on ne fait PAS (refus de périmètre, §2.6).**

- **R1 : pas de barre graphique ni d'animation au pourcentage.** Le mode
  « barre au % » est mort chez Chromium (A40) ; le % reste dans le texte
  (A52). Pas de ventilation par compte : un seul chiffre agrégé.
- **R2 : aucun apprentissage anti-spam local.** On déplace vers le
  dossier Junk du fournisseur ; c'est LUI qui apprend (Gmail entraîne son
  filtre sur le déplacement). Un compte **sans dossier Junk détectable**
  ne reçoit pas le geste (indisponible, honnête — pas de dossier inventé).
- **R3 : pas de corbeille à brouillons ni d'annulation.** Un brouillon
  supprimé l'est ; « Annuler » conserve déjà pour qui hésite.
- **R4 : pas de menu « ⋯ Plus » par message** (exception b des
  annotations : jamais de menu Plus). Gestes directs, nus, comme le reste
  du fil.

## Options et verdicts

Aucun **point dur mesurable** ici : les quatre retours s'appuient sur des
mécanismes existants, il n'y a pas d'options concurrentes à départager sur
des chiffres (pas de spike). Le seul vrai arbitrage est **l'ergonomie du
positionnement R4**, qui relève du Chef Ingénieur (D4) et se prépare par
une **maquette d'étude** (jetable, DC-D4), pas par une mesure.

Un seul poste demande une **mesure au terrain** (pas un spike) : le coût
du COUNT du dénominateur R1 sur la vraie base (256 k / 7 Go). La sonde
`backfill_status` est déjà `hors_pompe` (ne gèle pas la fenêtre), mais un
COUNT du corpus complet répété au sondage doit rester **bon marché**
(indexé) ou **caché** — à vérifier au STOP 2, budget « pas de sonde chère »
(DETTE D-8). Repli déjà prévu : cacher le total, ne le rafraîchir qu'au
changement franc.

## Étapes

> **État au 2026-08-17 (implémentation close, gate en cours) :** E1-E4
> livrées. Amendements du Système **A55** (R1), **A56** (R2), **A57** (R3),
> **A58** (R4). Revue `/code-review high` passée : 2 constats corrigés
> (réponse-à-soi masquée sur nos propres messages ; toast « supprimé » seul
> si un brouillon existait), 3 reportés (double COUNT → mesure terrain D-8 ;
> report_spam déjà-spam et double connexion → cas mineurs).
>
> **Passe terrain du 2026-08-18 (STOP 2) :** R1 OK (app fluide → budget D-8
> tenu, pas d'optimisation COUNT), R2 OK, R3 OK. **R4 — constat terrain
> corrigé le jour même :** le CE répond parfois sur ses propres messages →
> rétablir les 3 gestes sur TOUS les messages (garde de revue retirée), et
> la réponse sur un message propre vise les destinataires d'origine (le À
> pour Répondre, À+Cc pour Répondre à tous). Décision **pure** `reply_to`
> (mail-core, TDD) ; `reply_context` branché (avec repli relève serveur si
> l'envoi n'a pas ses destinataires en base) ; `reply_all_split` gérait
> déjà le cas. Décor Clarity doté des destinataires de m1 ; e2e + Système
> (A58) mis à jour. Re-gate complète, puis commit + CI.

Ordre : du moins risqué au plus structurant. Chaque étape close par sa
gate partielle (`cargo test` ciblé + `npm test` de la suite touchée) ; la
**gate complète `/gate` avant le commit final** (jamais les tests seuls).

- **E1 — R3, supprimer un brouillon.** Le plus petit, socle backend
  déjà là.
  - Backend : rien à écrire (`delete_draft` existe).
  - UI : bouton « Supprimer le brouillon » (glyphe `delete`) au pied de
    `Composition.svelte`, distinct d'Annuler ; route par le même garde
    que `fermer()` pour qu'une sauvegarde en vol ne ressuscite pas le
    brouillon supprimé. Comportement selon D3.
  - Gate : e2e composition (créer un brouillon, le supprimer, vérifier
    qu'il ne réapparaît ni en liste Brouillons ni dans le fil).

- **E2 — R1, pourcentage de rattrapage.** TDD sur la partie pure.
  - Backend : fonction **pure** `backfill_percent(fait, total) -> u8`
    (motif §4, sœur de `sync_percent`), testée RED→GREEN aux bornes
    (0/0, tout fait, arrondi). `BackfillStatus` gagne `total` (ou `done`) ;
    `pending_total` gagne un compagnon `corpus_total` **indexé**.
  - UI : `statut.rattrapageCorps` reçoit `{p}` selon D1 ; catalogue FR + EN.
  - Gate : `cargo test -p mail-core` (pur) + `cargo test` commands ;
    e2e statut inchangée (pas de régression d'affichage).
  - **Mesure terrain** (STOP 2) : coût du COUNT sur 256 k.

- **E3 — R2, spam / non-spam.** TDD sur la résolution + rejeu.
  - Backend : deux commandes `report_spam` / `mark_not_spam` réutilisant
    `queue_removal(MoveTo(...))`. `report_spam` résout
    `boite("indesirables")` ; erreur douce si absent. `mark_not_spam`
    vise `INBOX`. Test pur : résolution du dossier Junk + `MoveTo` bien
    journalisée et rejouée (aller-retour `action.rs` déjà couvert).
  - UI : geste dans la barre du fil selon D2 ; « Ce n'est pas un spam »
    en vue Indésirables (bascule sur `categorie === 'indesirables'`).
    Optimisme UI : disparition locale immédiate (comme archiver), toast,
    `liste.recharger()` + `chargerNav()` + `passeApresGeste`.
  - Gate : e2e (signaler depuis Réception → disparaît ; « pas spam »
    depuis Indésirables → disparaît).

- **E4 — R4, réponse par message + Système.** Le plus structurant, en
  dernier ; dépend de D4/D5.
  - **Maquette d'étude** d'abord (haut/bas), reversée au Système, jetée.
  - UI : chaque `<article class="deplie">` reçoit sa barre de réponse
    visant CE message (`m`), position selon D4. Réorganisation de la barre
    de fil selon D5 (les gestes de tri y restent ; R2 s'y range aussi).
  - **DC-D2** : amender `docs/design/systeme.dc.html` dans le MÊME commit
    (journal A-n : la barre par message, le contrat d'icônes inchangé).
  - Gate : e2e fil (une barre de réponse par message déplié, `toHaveCount`
    discriminant ; répondre depuis un message précis vise le bon
    correspondant).

## § Décisions CE — à trancher au STOP 1

Chaque décision appartient au Chef Ingénieur. Réponses consignées ici
mot pour mot avec la date.

- **D1 — dénominateur et forme du % (R1).** La ligne montre aujourd'hui
  « Rattrapage des messages · {n} restants ». Le % le plus honnête est
  *corps présents / total du corpus en portée* — sur la vraie base
  (~256 k) il démarre bas et monte sur des semaines (la traîne est
  longue, PASSATION §1). Option A : « … · {n} restants · {p} % » (garder
  le compte, ajouter le %). Option B : « … · {p} % » (le % seul).
  Option C : statu quo (refuser le retour). **Recommandation : A.**
  → **RÉPONSE CE (2026-08-17) : « Compte + pourcentage » (Option A)** —
  « … · {n} restants · {p} % », p = corps présents / total du corpus en
  portée.

- **D2 — emplacement et portée du geste spam (R2).** Recommandation :
  dans la barre d'actions du fil, à côté d'Archiver/Supprimer ; **par
  fil** (cohérent avec archiver/supprimer, qui visent `fil.ligne`) ; en
  vue Indésirables le bouton devient « Ce n'est pas un spam ». Alternative :
  geste **par message**. **Recommandation : par fil.**
  → **RÉPONSE CE (2026-08-17) : « Barre du fil, par fil » (Option A)** —
  à côté d'Archiver/Supprimer, vise `fil.ligne` ; « Ce n'est pas un
  spam » en vue Indésirables.

- **D3 — suppression de brouillon : confirmation ? (R3).** Un brouillon
  supprimé n'a pas d'annulation. Option A : suppression au clic (geste
  explicite ; « Annuler » conserve déjà pour l'hésitant). Option B :
  petite confirmation avant suppression. **Recommandation : A** (léger,
  cohérent avec un brouillon local non envoyé).
  → **RÉPONSE CE (2026-08-17) : « Avec confirmation » (Option B)** — une
  petite confirmation (« Supprimer ce brouillon ? ») avant la suppression
  définitive.

- **D4 — positionnement des boutons de réponse par message (R4).** Le
  point que l'utilisateur soulève explicitement. Option Haut : sous
  l'en-tête du message, avant le corps (visible sans dérouler un long
  message). Option Bas : après le corps (on répond quand on a fini de
  lire — convention Gmail/Outlook). Option Deux : haut ET bas.
  Maquette d'étude fournie au STOP 1. **Recommandation : à la lecture de
  la maquette.**
  → **RÉPONSE CE (2026-08-17) : « En bas du message » (Option Bas)** —
  après le corps, un filet de séparation, la barre des trois gestes ; on
  répond quand on a fini de lire (convention Gmail/Outlook).

- **D5 — sort de la barre de fil globale (R4).** Si la réponse passe par
  message : la barre du bas garde les gestes de **tri** (Archiver,
  Supprimer, + Spam de R2). Répondre / Répondre-tous / Transférer
  deviennent **par message**. Garde-t-on aussi un « Répondre » global (au
  dernier message d'autrui, l'actuel `cible()`) pour le geste rapide ?
  Option A : non, tout passe par message (plus simple, un seul modèle).
  Option B : oui, garder un Répondre global au fil en plus. **Recommandation :
  A.**
  → **RÉPONSE CE (2026-08-17) : « Tri seul » (Option A)** — la barre du bas
  garde Archiver, Supprimer et Spam ; Répondre / Répondre-tous /
  Transférer passent UNIQUEMENT par message. Un seul modèle.

## Traçabilité prévue

- Journal du Système (A-n) : au moins R4 (barre par message) et R2
  (geste spam) — DC-D2, même commit que le code UI.
- Pas d'ADR attendu (aucune décision structurante gelée ; réutilisation
  de mécanismes existants). Si D4 fige une convention d'ergonomie
  réutilisable, une note au Système suffit.
- PASSATION §1 amendée à la clôture (nouvel état, dernière version).
- Mémoire persistante : chantier soldé d'août 2026 (archive).
