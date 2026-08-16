# Wind

**Wind** — le client courrier de la suite **Elements**
(« ce que le vent porte, le rythme des jours »).

Application de bureau : cœur Rust (IMAP, SMTP, OAuth, rendu des messages)
et interface Svelte, empaquetée par Tauri. Cible Windows arm64 natif,
installeur NSIS, mise à jour automatique et signée (minisign, ADR 0013).

Dernière version livrée : **0.1.7**. En préparation de bêta fermée.

## Journal des versions

Les versions et leurs changements sont consignés dans
[CHANGELOG.md](CHANGELOG.md). Les paquets signés vivent dans les
[Releases GitHub](https://github.com/smonchamps/wind/releases).

## Documentation

- [docs/PASSATION.md](docs/PASSATION.md) — comment reprendre le projet, la
  méthode (instruction permanente), l'état du terrain, les décisions
  gelées et les invariants.
- [docs/WORKFLOW.md](docs/WORKFLOW.md) — les workflows standardisés
  (`/chantier`, `/terrain`, `/gate`, `/solde`).
- [CHANGELOG.md](CHANGELOG.md) — le journal des versions.

## Construire et vérifier

Les commandes (jeu d'essai, build de l'installeur, e2e) sont décrites au
§7.3 de [docs/PASSATION.md](docs/PASSATION.md). La gate complète — format,
build UI, contrastes, cohérence du Système, clippy, tests Rust et e2e
réels — est rejouée au pré-push (`.githooks/pre-push`) : rien ne quitte la
machine sans elle.
