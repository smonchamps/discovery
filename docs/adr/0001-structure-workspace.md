# ADR 0001 — Structure du workspace : deux membres seulement

Date : 2026-07-11 · Statut : accepté

## Contexte

Le plan ([PLAN.md](../PLAN.md), §3) prévoit à terme `mail-core`, `mail-protocols`,
`sync-server`, `apps/desktop` et `apps/web`. La simplicité est une caractéristique
qualité clé du produit — elle s'applique aussi au code.

## Décision

Le workspace ne contient que ce dont on a besoin aujourd'hui :

- `crates/mail-core` — le domaine, sans dépendance UI ni réseau ;
- `apps/desktop` — coquille binaire, future app Tauri (Phase 1).

`mail-protocols` ne sera extrait de `mail-core` que lorsque plusieurs
implémentations de protocoles existeront ; `sync-server` et `apps/web`
n'apparaîtront qu'en Phase 4. Créer ces crates maintenant serait du stock
mort (muda) : des interfaces figées avant d'avoir appris des spikes de Phase 0.

Le spike initial `src/main.rs` (IMAP avec mot de passe en dur) est supprimé,
conformément au plan (§9.1).

## Conséquences

- Moins de frontières à maintenir tant que le domaine est petit.
- L'extraction future de `mail-protocols` est un simple déplacement de modules,
  gardé honnête par la règle « `mail-core` ne dépend d'aucune UI ».
- Lints workspace partagés : `unsafe_code = "forbid"`, `unwrap`/`expect`
  interdits hors tests.
