# Wind

**Wind** — le client courrier de la suite **Elements**
(« ce que le vent porte, le rythme des jours »).

Application de bureau : cœur Rust (IMAP, SMTP, OAuth, rendu des messages)
et interface Svelte, empaquetée par Tauri. Cible Windows **arm64 et
x64** (release bi-arch, ADR 0023), installeur NSIS, mise à jour
automatique et signée (minisign, ADR 0013).

La dernière version livrée et l'état courant vivent dans
[docs/ETAT.md](docs/ETAT.md). En préparation de bêta fermée.

## Journal des versions

Les versions et leurs changements sont consignés dans
[CHANGELOG.md](CHANGELOG.md). Les paquets signés vivent dans les
[Releases GitHub](https://github.com/smonchamps/wind/releases).

## Documentation

- [docs/STANDARD.md](docs/STANDARD.md) — la méthode (instruction
  permanente), les décisions gelées et les invariants.
- [docs/ETAT.md](docs/ETAT.md) — l'état courant : version livrée,
  prochain chantier, chiffres du terrain.
- [docs/WORKFLOW.md](docs/WORKFLOW.md) — les workflows standardisés
  (`/chantier`, `/terrain`, `/gate`, `/solde`).
- [CHANGELOG.md](CHANGELOG.md) — le journal des versions.

## Construire et vérifier

Les commandes (jeu d'essai, build de l'installeur, e2e) sont décrites au
§7.3 de [docs/STANDARD.md](docs/STANDARD.md). La gate complète — format,
build UI, contrastes, cohérence du Système, clippy, tests Rust et e2e
réels — est rejouée au pré-push (`.githooks/pre-push`) : rien ne quitte la
machine sans elle.
