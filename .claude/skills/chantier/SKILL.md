---
name: chantier
description: Dérouler un chantier Wind de bout en bout à partir d'un énoncé « Bug : … » ou « Feature : … » — instruction sur pièces, conception set-based, plan avec décisions CE, deux validations manuelles, implémentation TDD, gate complète, terrain, documentation, commit et CI verte.
---

# /chantier — le workflow standard d'un chantier Wind

L'argument est l'énoncé : `Bug : …` ou `Feature : …`. La méthode de
STANDARD §2 s'applique intégralement ; ce skill en est le déroulé
opératoire. **Deux points d'arrêt sont obligatoires** — la validation du
plan et la validation terrain. Rien ne les contourne.

## Phase 0 — Instruction (genchi genbutsu avant tout)

- **Bug** : reproduire et **mesurer** avant toute hypothèse. Si la
  reproduction exige la machine ou les comptes du CE (rappel §7.1 : tu
  ne peux pas lire sa base), demander la mesure et attendre. Un
  symptôme n'est pas une cause : remonter à la racine (modèle : A38,
  commits 9ebd7b2 → 5698641 — la ceinture d'abord, la racine ensuite).
- **Feature** : lire le Système (`docs/design/systeme.dc.html`, seul
  normatif — A18), les ADRs concernés (décisions gelées, STANDARD §5),
  l'état du code. Sur une zone vaste, les agents `Explore`/`Plan`
  intégrés font la reconnaissance.

## Phase 1 — Conception

1. **Constat** écrit : les faits, les chiffres, ce qui est prouvé.
2. **Set-based si point dur** : plusieurs options, départagées sur des
   mesures — spikes jetables dans `spikes/`, via l'agent `spike` (un
   agent par option, en worktree isolé). Modèle : ADR 0004.
   L'alternative doit battre l'hypothèse *nettement* pour la déloger.
3. **Prototype si UI** : maquette d'étude (projet Claude Design,
   `.dc.html`), jamais normative — l'esprit DC-D4 : sa substance sera
   reversée au Système, le fichier d'étude ne rentre pas au dépôt.
4. **Refus de périmètre explicites** : ce qu'on ne fait pas, et
   pourquoi (§2.6). Dire non est le comportement par défaut.
5. **Rédiger `docs/PLAN-XXX.md`** dans la forme canonique des plans du
   dépôt : constat, périmètre, options et verdicts chiffrés, étapes
   E1-En avec leur gate, et un **§ Décisions CE** listant chaque
   arbitrage qui appartient au Chef Ingénieur (numérotés D1, D2, …).

## ⛔ STOP 1 — validation CE du plan

Présenter le plan, puis poser les décisions du § Décisions CE **une à
une** (AskUserQuestion). Consigner les réponses au PLAN, mot pour mot,
avec la date. **Aucun code de production avant le GO.**

## Phase 2 — Implémentation

- Étape par étape, dans l'ordre du plan.
- **TDD strict** : le test échoue (RED, montré) avant l'implémentation
  (GREEN). Si un RED ne peut rien apprendre (fonction pure triviale),
  le dire — jamais le simuler.
- **DC-D2** : tout commit qui touche l'UI amende
  `docs/design/systeme.dc.html` dans le **même commit** (journal A-n).
- Zéro `unwrap()`/`expect()` en prod ; `thiserror` dans les crates,
  `anyhow` dans les apps. Décision pure et testable, I/O ailleurs
  (motif STANDARD §4).

## Phase 3 — Qualité

1. **Revue à regard neuf** : `/code-review high` sur le diff, une fois
   l'implémentation complète, avant le commit final. Corriger ce qui
   est confirmé.
2. **Gate complète** : `/gate`. Un rouge = andon — on arrête, on
   corrige, on rejoue. Jamais de `--no-verify` sans décision explicite
   du CE.

## ⛔ STOP 2 — validation terrain

Remettre au CE une **checklist de terrain** : quoi regarder, gestes à
jouer, chiffres attendus, budgets à re-mesurer (STANDARD §3 — un
budget dépassé arrête la ligne). Fournir **systématiquement, à ce
moment, les commandes PowerShell nécessaires à la réalisation du test
terrain** (lancement de l'app, build, préparation des comptes,
mesures) — prêtes à copier, une par bloc. Attendre le verdict. Un
constat terrain → correction **le jour même**, dans la même session,
puis re-gate et nouvelle passe terrain.

## Phase 4 — Documentation

- Journal du Système : A-n pour chaque fait notable (DC-D2).
- PLAN-XXX mis à jour (étapes livrées, commits, verdicts).
- **ADR** si décision structurante (`docs/adr/`, court, modèle 0004).
- ETAT amendé (l'état du projet, budgets re-mesurés) ; STANDARD
  amendé si piège ou enseignement nouveau (§7, §9).
- Mémoire persistante mise à jour (état du chantier, dates absolues).

## Phase 5 — Commit, push, CI

- Message : `type: description`, **sans accents**, corps portant les
  chiffres et le raisonnement, **jamais de Co-Authored-By** (§2.8).
- Push (le hook pre-push rejoue la gate), puis **`gh run watch`
  jusqu'à CI verte** — les e2e locaux peuvent flaker, la CI est la
  référence. Le chantier n'est clos qu'à la CI verte ; ensuite,
  `/solde`.
