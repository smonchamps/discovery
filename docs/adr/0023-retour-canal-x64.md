# ADR 0023 — Retour du canal x64 : release bi-arch (arm64 + x64)

**Date** : 2026-08-22 · **Statut** : accepté (décisions CE D5-D8 du
2026-08-22, PLAN-RETOURS-8 § Décisions)

## Contexte

Le canal x64 a été **retiré en 0.1.3** (PLAN-WIND E4) : le seul poste
utilisateur était ARM64 et faisait tourner l'app x64 en émulation.
Depuis, `faire-release.ps1` ne bâtit que l'hôte (arm64), la Release
porte 3 assets et `latest.json` une seule clé `windows-aarch64`. La
directive CE du 2026-08-22 (PLAN-RETOURS-8 R3) rouvre le canal :
**chaque release livre x64 ET arm64**.

Faits d'instruction :

- **L'updater Tauri choisit sa plateforme seul** : la clé `{os}-{arch}`
  vient des constantes de compilation du binaire installé. Un seul
  `latest.json` sert donc les deux canaux ; il suffit d'y ajouter
  `windows-x86_64`. **Rien à changer côté Rust.**
- **La version est globale au manifeste** : les deux architectures
  sortent à la même version, ou pas du tout.
- **Le cross-build x64 depuis le poste ARM64 est prouvé** (E1,
  2026-08-22) : toolset MSVC 14.50 avec libs x64 déjà posé, cible
  rustup ajoutée, override `lld-link` étendu au triple x64 (le piège
  `link.exe` de Git Bash, déjà payé sur arm64, se rejouait tel quel) ;
  `cargo tauri build --target x86_64-pc-windows-msvc` lie et bundle en
  1 min 45 s → `target/x86_64-pc-windows-msvc/release/bundle/nsis/
  Wind_<v>_x64-setup.exe`. La CI `quality` (windows-latest, x64)
  prouvait déjà compile + tests en continu.
- **Panne silencieuse propre au bi-arch** : une clé de plateforme
  manquante ou des signatures croisées ne produisent AUCUNE erreur —
  l'updater du canal muet conclut « pas de mise à jour ». Troisième
  membre de la famille des pièges de l'ADR 0013 (BOM, tag `v`).

## Décision

1. **Cross-build local sur le poste ARM64** (D6) — deux
   `cargo tauri build --target <triple>` dans `faire-release.ps1`, la
   clé de signature ne quitte jamais le poste (une même clé signe les
   deux canaux), mot de passe demandé une fois. La CI reste une gate,
   jamais un builder de release.
2. **Tout-ou-rien** (D7) — un build en échec bloque toute la release :
   jamais un canal décalé, jamais un manifeste partiel.
3. **`latest.json` à deux clés**, construit par plateforme depuis le
   répertoire de SA cible ; **garde anti-croisement encodée** (les
   deux signatures doivent être distinctes) — jamais laissée à la
   vigilance.
4. **`verifier-release.ps1`** scripte la vérification §2.10 (5 assets
   nommés, BOM, deux clés, signatures == `.sig` et distinctes, URL qui
   résolvent) — avec deux plateformes les contrôles manuels doublaient.
5. **Preuve terrain par canal** (D5) : arm64 sur ce poste ; x64 sur un
   **second poste x64** — jamais en émulation (le motif du retrait de
   0.1.3). Le premier auto-update x64 n'est constatable qu'à la release
   suivant la première bi-arch ; l'install x64 se constate dès elle.
6. **Le critère MAJEUR de §2.9 s'évalue PAR CANAL** : une rupture
   d'auto-update sur un seul canal suffit à déclencher MAJEUR.
   L'ajout du canal x64 n'en est pas une (les postes arm64 continuent
   de lire leur clé) → la première release bi-arch est MINEURE (D8).

## Conséquences

- Temps de release ~doublé (deux builds, ~4 min chacun) — assumé, la
  confirmation `OUI` reste après les builds.
- `installer-poste.ps1` (préparation d'un poste x64) décrivait le
  bi-arch comme « chantier à part » — retourné.
- Les six mentions « 3 assets au tag nu » de l'historique d'ETAT
  restent vraies pour LEURS versions ; la norme courante est « 5
  assets » (§2.10 amendé).

## Écartée — build x64 en CI GitHub

Le runner `windows-latest` est x64 natif (pas de cross), mais la clé de
signature deviendrait un secret GitHub Actions et la release un
processus en deux lieux. Écartée (D6) tant que le cross-build local
tient — à rouvrir si un build x64 local échoue durablement.
