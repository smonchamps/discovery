# PLAN-DOCUMENTATION — trois gestes kaizen, et le standard prend son nom (2026-08-19)

> **CHANTIER OUVERT — décisions D1-D4 tranchées par le CE le
> 2026-08-19** (voir § Décisions CE, réponses consignées mot pour
> mot) ; exécution E1-E3 sur son GO.
> Chantier documentation pur : aucun code touché, aucune release.
> Origine : analyse de la structure documentaire (session du
> 2026-08-19), passée au challenge d'un regard sensei — le grand plan
> de restructuration (9 répertoires) est **rejeté**, faute de douleur
> mesurée qui le tire ; restent trois gestes kaizen appuyés sur des
> faits, plus le renommage de PASSATION.md, dont le nom induit le
> comportement qu'on corrige.

Trois gestes et un renommage. Chaque geste résout une douleur
constatée, tient en un commit, et porte sa mesure de réussite. Le
chantier se valide par un test de reprise à froid — la fonction que
cette documentation doit garantir.

---

## Constat — genchi genbutsu, faits mesurés

- **F1 — Le document d'instruction ne tient plus dans une lecture.**
  `PASSATION.md` : 1 052 lignes, ~28 000 tokens ; la lecture agent est
  tronquée au plafond de 25 000 (mesuré ce jour : `Read` s'arrête à la
  ligne 805 sur 1 053). Une session neuve ne peut plus charger son
  instruction d'un seul geste.
- **F2 — Le nom induit la réécriture.** Une quinzaine de commits
  « passation réécrite / remise à l'état réel » dans l'historique. Un
  nom de fichier est une instruction pour un agent : « passation » dit
  *instantané de relève, à réécrire à chaque fois* — et c'est
  exactement ce que chaque session a fait. Personne ne « réécrit »
  DETTE.md ni un ADR : leurs noms disent des registres, ils sont
  traités en registres.
- **F3 — Les morts et les vivants sont mélangés.** 23 `PLAN-*.md`
  **tous soldés** (vérifié ce jour, un par un) et 5 revues de phase
  closes vivent à plat dans `docs/`, à côté des documents vivants. Le
  statut ne se lit pas dans `ls` ; six plans (BROUILLONS, DC, LANGUES,
  REGLAGES, UI-V2, WIND) n'affichent pas leur solde en tête — le
  marqueur est enfoui au fil du texte.
- **F4 — Du normatif vit hors dépôt.** La procédure de vérification
  d'une release n'existe **que** dans une mémoire Claude
  (`verifier-release-wind`) ; la règle de numérotation existe en
  double (mémoire + §2.9) ; l'archive des chantiers soldés existe en
  double (mémoire + fichiers). Deux sources de vérité, divergence
  possible en silence — et une mémoire n'est pas versionnée, pas
  revue, pas poussée.
- **F5 — Les références vivantes sont comptées.** À amender si le nom
  change : `CLAUDE.md`, `README.md`, `docs/WORKFLOW.md`,
  `docs/DETTE.md`, les 4 skills (`chantier` ×5, `gate` ×1, `solde` ×2,
  `terrain` ×1), l'agent `.claude/agents/spike.md`, 4 mémoires. À ne
  **pas** toucher (historiques) : les plans soldés, ADR 0018/0019,
  `CHANGELOG.md` — ils citent le document tel qu'il s'appelait à leur
  époque.

## Refus de périmètre — ce que ce chantier ne fait PAS

Rejetés au challenge, faute de douleur observée qui les tire ; à
rouvrir seulement sur constat :

- la taxonomie `produit/` / `regles/` / `registres/` / `phases/`
  (9 répertoires) — optimisée pour un lecteur humain qui navigue, qui
  n'existe pas ici ; pour un agent, chaque fichier de plus est une
  occasion de chargement partiel ;
- `chantiers/INDEX.md` — stock neuf à maintenir, en doublon de `ls`
  et de l'historique git ;
- `ARCHITECTURE-FONCTIONNELLE.md` et l'éclatement de `PLAN.md` — le
  jour où une session paiera un défaut *causé* par leur absence, ce
  constat sera le `/terrain` qui les fera naître ;
- une règle méta `DOCUMENTATION.md` — si la structure a besoin d'un
  document pour s'expliquer, c'est qu'elle est trop complexe.

---

## Étapes

### E1 — L'état sort, le standard prend son nom (un commit)

Les deux mouvements sont indissociables : renommer sans sortir l'état
serait mentir dans l'autre sens (un « standard » qui porte du volatile).

1. **Créer `docs/ETAT.md`** — l'instantané de relève. Reprend le §1
   (où on en est : version livrée, prochain chantier, chiffres
   terrain, arbitrages ouverts) et le §8 (ce qui reste : reports,
   longue traîne). En-tête qui assume son rôle : *« Ce document est
   réécrit à chaque chantier — c'est sa fonction. »*
2. **`git mv PASSATION.md STANDARD.md`** (l'historique suit). Dans
   STANDARD : le §1 devient un renvoi d'une ligne vers ETAT.md, le §8
   idem. **La numérotation §2-§10 est conservée telle quelle** — toute
   référence existante « §2.9 », « §7.1 » reste vraie, aucune
   renumérotation à répercuter.
3. **Stub `docs/PASSATION.md`** (3 lignes) : « Scindé le 2026-08-19 —
   le standard vit dans STANDARD.md, l'état de reprise dans ETAT.md. »
   Poka-yoke temporaire pour les vieilles mémoires et l'ancien rituel
   de reprise ; sa condition de retrait est consignée en DETTE
   (D-24 proposé : retirer quand deux reprises à froid consécutives
   n'y auront pas trébuché).
4. **Amender au même commit** (esprit DC-D2) : `CLAUDE.md`,
   `README.md`, `WORKFLOW.md`, `DETTE.md`, les 4 skills, `spike.md` —
   « PASSATION §2 » devient « STANDARD.md §2 », « PASSATION §1 »
   devient « ETAT.md ».
5. **Mémoires** (hors dépôt, même moment) : les 4 mémoires citant
   PASSATION pointent vers STANDARD/ETAT.

*Mesures de réussite :* STANDARD.md ≈ 750 lignes (~20 k tokens),
lisible en **une** lecture ; ETAT.md < 200 lignes ; à partir de là,
un chantier ordinaire ne touche plus que ETAT.md (et DETTE, et son
plan).

### E2 — Archiver les soldés (un commit)

1. Créer `docs/archives/` — **un seul** répertoire, pas quatre.
2. **Garde avant tout déplacement** : vérifier le marqueur de solde de
   *chaque* plan ; pour les six où il est enfoui, le normaliser en
   tête (blockquote datée, une ligne, comme PLAN-RETOURS-4) — le
   statut d'un document se lit dans ses 5 premières lignes.
3. `git mv` des 28 fichiers : les 23 `PLAN-*.md` soldés + `PHASE0.md`
   à `PHASE3.md` + `PHASE-REFONTE.md`. **`PLAN.md` reste** à la
   racine : c'est le concept paper, source de vérité produit, vivant.
4. **Garde des liens relatifs** : les fichiers déplacés qui pointent
   `adr/…` ou `PASSATION.md` voient leurs liens réécrits en
   `../adr/…` etc. (le *texte* historique, lui, ne change pas — seuls
   les hrefs suivent le déplacement).

*Mesure de réussite :* la racine de `docs/` ne contient plus que des
vivants — STANDARD, ETAT, PASSATION (stub), PLAN.md, DETTE, WORKFLOW,
ce plan (jusqu'à son solde), `adr/`, `design/`, `archives/`.

### E3 — Rapatrier le normatif orphelin (un commit + mémoires)

1. **La vérification de release entre au dépôt** : nouvelle
   sous-section §2.10 « Vérifier une release » dans STANDARD.md,
   reprenant les faits de la mémoire `verifier-release-wind` (Release
   « Latest » via l'endpoint `/releases/latest/`, `latest.json` sans
   BOM + URL au tag nu, `sig` == `.sig`, la preuve vivante =
   l'auto-update au terrain, le commit de release ne bumpe que
   tauri.conf.json, le pre-push rejoue la gate).
2. **L'enseignement « le cache chaud ment »** (mémoire
   `mesure-reconstruction-cache-chaud`, ×340 mesuré) entre au §9 —
   c'est un enseignement payé, sa place est avec les autres.
3. **Triage des mémoires** — la règle : *le dépôt est la seule source
   de vérité du normatif ; la mémoire ne garde que le local-machine et
   les pointeurs.*

   | Mémoire | Devient |
   |---|---|
   | `numerotation-versions-semver` | pointeur vers STANDARD §2.9 |
   | `verifier-release-wind` | pointeur vers STANDARD §2.10 |
   | `gate-complete-avant-commit` | pointeur vers le skill `/gate` (+ garde les réflexes purs) |
   | `chantiers-soldes-2026-08` | pointeur vers `docs/archives/` |
   | `placement-barre-tri-fil` | pointeur vers le journal Système (A58 le porte déjà) |
   | `e2e-flaky-local-ci-reference` | **reste** — fait machine |
   | `identite-git-smonchamps` | **reste** — fait machine/compte |
   | `workflow-skills-agents` | pointeur vers WORKFLOW.md |

   `MEMORY.md` (l'index) est remis à jour en conséquence.

*Mesure de réussite :* plus aucune règle du projet qui n'existe **que**
hors dépôt.

### E4 — Test de reprise à froid (le terrain de ce chantier)

Le protocole, joué par le CE dans une **session neuve**, sans aider :

1. Ouvrir une session et lancer une reprise ordinaire. Attendu : la
   session trouve STANDARD.md et ETAT.md seule (via CLAUDE.md), et
   énonce sans erreur : version livrée (0.1.10), prochaine release
   (0.1.11, CORRECTIF), prochain chantier (composeur enrichi → 0.2.0,
   MINEUR), et la règle de versionnage qui justifie les deux.
2. **Test du stub** : coller l'ancien rituel du §0 (qui cite
   `docs/PASSATION.md`) dans une session neuve. Attendu : la session
   rebondit via le stub vers STANDARD/ETAT sans se perdre.
3. Un accroc = correction le jour même, re-test.

---

## Gardes du chantier

- **Les historiques sont intouchés** : plans archivés, ADR, CHANGELOG
  gardent leurs citations « PASSATION » — un document historique cite
  le monde de sa date.
- **La numérotation § de STANDARD.md est figée** (§2-§10) — aucune
  référence externe à renuméroter, ni aujourd'hui ni demain.
- **Un geste = un commit** (`docs:`), sans accents dans le message,
  gate complète avant chaque commit (le pre-push la rejoue — chantier
  docs ou pas, la règle est la même).
- Ce plan est soldé par `/solde` comme les autres, puis rejoint
  `archives/` lui-même.

## Mesures du chantier — bilan attendu au solde

| Mesure | Avant | Attendu |
|---|---|---|
| Instruction lisible en une lecture agent | non (tronquée à 805/1053) | oui (~750 lignes) |
| Commits d'un chantier ordinaire touchant le standard | oui (réécriture) | non (ETAT.md seul) |
| Fichiers vivants à la racine de `docs/` | 28 | 7 + 3 dossiers |
| Règles projet existant hors dépôt seulement | 1 (vérif. release) | 0 |

---

## § Décisions CE

- **D1 — Le nom du standard.** Recommandation : **`STANDARD.md`** —
  vocabulaire maison (WORKFLOW.md dit déjà « standard work »), et le
  nom porte le bon réflexe : un standard ne se réécrit pas, il
  s'amende par kaizen. Alternatives : `REFERENTIEL.md` (exact mais ne
  charge aucun comportement), garder `PASSATION.md` (statu quo,
  F2 continue).
  **Réponse CE (2026-08-19) : « STANDARD.md »**
- **D2 — Périmètre d'ETAT.md.** Recommandation : **§1 + §8** (les deux
  sont du volatile ; ne sortir que le §1 laisserait « ce qui reste »
  se périmer dans le standard). Alternative : §1 seul.
  **Réponse CE (2026-08-19) : « §1 + §8 »**
- **D3 — Le stub PASSATION.md.** Recommandation : **oui, temporaire**,
  avec sa condition de retrait consignée en DETTE (D-24 : retirer
  après deux reprises à froid propres). Alternative : pas de stub,
  mise à jour des mémoires jugée suffisante.
  **Réponse CE (2026-08-19) : « oui, avec condition de retrait en
  DETTE (deux reprises à froid propres) »**
- **D4 — `docs/architecture/index.html`** (hors des trois gestes, mais
  le fichier est **non suivi** et peut se perdre en silence — il faut
  trancher son existence). Options : (a) le commiter tel quel comme
  illustration non normative (`docs:`, un commit) ; (b) le déclarer
  maquette jetable (esprit DC-D4) et le supprimer ; (c) le garder pour
  un futur chantier architecture — mais alors le commiter quand même,
  un fichier non versionné n'est pas « gardé ». Recommandation :
  **(a)** — trois lignes d'en-tête préciseront qu'il illustre sans
  faire foi.
  **Réponse CE (2026-08-19) : « commiter tel quel en illustration non
  normative »**
