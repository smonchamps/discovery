# Journal des versions

Toutes les modifications notables de Wind sont consignées ici.

Le format s'inspire de [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/),
et le projet suit le [versionnage sémantique](https://semver.org/lang/fr/).

Les paquets signés et leurs notes vivent dans les
[Releases GitHub](https://github.com/smonchamps/wind/releases) ; la mise à
jour est automatique et signée (minisign, ADR 0013).

## [0.1.7] - 2026-08-16

La refonte entière au poste : le Système v2 « Wada » et son élargissement,
l'UI v3 et ses retours CE, sur une fenêtre qui ne gèle plus.

### Ajouté

- Trois modes d'affichage au choix — trois volets (défaut inchangé), deux
  volets, ou un volet avec tiroir de navigation (PLAN-VOLETS).
- Système visuel v2 « Wada » : palette remappée à teinte d'usage
  constante, le trait hitofude comme signature et seul indicateur de
  progression, nav et liste aux dessins des pistes, 119 jetons
  (PLAN-WADA).
- 28 thèmes et sombre automatique par déclinaison `-nuit`
  (PLAN-WADA-ELARGI).
- UI v3 : bandeau de liste, avatars, le volet de lecture devient le fil ;
  volets réglables à la souris, barres natives (PLAN-UI-V3,
  PLAN-RETOURS-V3).

### Modifié

- Volet de lecture au dessin exact de la maquette Classique ; bascule
  Déplier/Replier dérivée de l'état, hauteur du corps au contenu, entête
  de composition allégé, libellés « Tout » (retours CE A44-A47).

### Retiré

- L'interface v1 : la refonte est terminée (PLAN-RETRAIT-V1).

### Corrigé

- La fenêtre ne gèle plus : aucune commande bloquante sur le thread
  principal, jamais de CPU dans la fenêtre du verrou d'écriture,
  `busy_timeout` porté à 30 s (PLAN-GELS, ADR 0019).
- Un lien du corps ouvre le navigateur système et le corps ne bouge
  jamais ; l'iframe reste inerte (A37, invariant S1).
- La langue se lit sans adopter la base ; la modale de migration reste la
  première surface à payer l'adoption (ADR 0012).
- Deux suites e2e simultanées ne se marchent plus dessus : port CDP libre
  par suite, balayage borné au worktree (PLAN-ISOLATION-E2E).

## [0.1.6] - 2026-08-14

### Corrigé

- Réactivité de l'affichage (PLAN-REACTIVITE), validée au terrain : plus
  de lignes d'attente pendant une synchronisation ; suppression,
  archivage et envoi visibles dans leur dossier en moins d'une seconde,
  hors ligne compris (écho local) ; l'aperçu arrive avec la ligne, en un
  seul affichage.

## [0.1.5] - 2026-08-14

### Corrigé

- Icônes des avis rares (dont le bandeau de mise à jour) : police portée à
  43 glyphes.
- La copie Envoyés se relève sitôt l'envoi accepté (`sync_sent`).

## [0.1.4] - 2026-08-14

### Ajouté

- Pièces jointes : envoi et transfert réel.

### Corrigé

- Affichage des pièces jointes à la première ouverture (constat terrain du
  2026-08-14).

### Sécurité

- Première mise à jour signée sous la nouvelle clé (rotation de la clé de
  signature du 2026-08-14).

## [0.1.3] - 2026-08-14

### Modifié

- Discovery devient **Wind** (PLAN-WIND) — la base se déménage
  automatiquement au premier lancement.
- Canal arm64 natif.

### Sécurité

- Rotation de la clé de signature : installation manuelle requise depuis
  discovery 0.1.2 ; la chaîne d'auto-update reprend ensuite.

## [0.1.2] - 2026-07-26

### Corrigé

- `latest.json` corrigé : BOM retiré et URL au tag nu — l'auto-update
  aboutit (ADR 0013).

## [0.1.1] - 2026-07-26

### Ajouté

- Première version publiée (discovery) : installeur NSIS et mise à jour
  signée minisign, pilotée depuis Rust (ADR 0013).

[0.1.7]: https://github.com/smonchamps/wind/releases/tag/0.1.7
[0.1.6]: https://github.com/smonchamps/wind/releases/tag/0.1.6
[0.1.5]: https://github.com/smonchamps/wind/releases/tag/0.1.5
[0.1.4]: https://github.com/smonchamps/wind/releases/tag/0.1.4
[0.1.3]: https://github.com/smonchamps/wind/releases/tag/0.1.3
[0.1.2]: https://github.com/smonchamps/wind/releases/tag/0.1.2
[0.1.1]: https://github.com/smonchamps/wind/releases/tag/0.1.1
