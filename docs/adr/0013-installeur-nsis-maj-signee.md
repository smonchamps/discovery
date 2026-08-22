# ADR 0013 — Installeur NSIS et mise à jour automatique signée

Date : 2026-07-26 · Statut : accepté — **boucle validée au terrain le
jour même** (0.1.1 → 0.1.2 appliqué sur l'app installée, base intacte)
· **Amendé par [ADR 0023](0023-retour-canal-x64.md)** (2026-08-22) :
release **bi-arch** (arm64 + x64, 5 assets, `latest.json` à deux clés)
et publication **entièrement scriptée** par `faire-release.ps1` depuis
la 0.1.10 — les mentions « trois assets », « publication manuelle » et
le nom d'asset `discovery_<v>_x64-setup.exe` ci-dessous sont d'époque
(le préfixe est `Wind_`, les arch `arm64`/`x64`).

## Contexte

La Phase 5 ([PLAN.md](../PLAN.md) §5) demande « Installeur **MSIX** +
mise à jour automatique signée ». Deux points à trancher avant de coder.

### Le format d'installeur — « MSIX » est une borne héritée

Le plan a nommé MSIX **avant** qu'un piège ne soit découvert au terrain :
**MSIX virtualise `%APPDATA%`**. Une application empaquetée MSIX n'écrit
pas dans `%APPDATA%\Roaming\<id>` réel, mais dans un conteneur privé sous
`%LOCALAPPDATA%\Packages\<PackageFamilyName>\…`. C'est exactement le
mécanisme qui empêche de lire la vraie base depuis l'assistant (l'app
Claude est MSIX — voir passation §7.1).

Or **tout le modèle de données de Discovery est un fichier SQLite** à
`%APPDATA%\dev.discovery.app\discovery.db`, résolu par
`AppHandle::app_data_dir()`. Sur la machine du Chef Ingénieur, ce fichier
pèse **~715 Mo** (256 312 messages). Passer Discovery **lui-même** en
MSIX redirigerait ce chemin dans le conteneur du paquet : **la base
existante deviendrait orpheline**, et la migration de l'[ADR 0012] — qui
adopte précisément la base trouvée à ce chemin — ne la verrait plus.

C'est l'enseignement « une borne héritée n'est pas une borne décidée »
(passation §9) : « MSIX » était une hypothèse du plan, pas une mesure.

### Ce que « signée » recouvre — deux signatures distinctes

- **Signature de l'updater** (minisign, intégrée à Tauri) : garantit
  qu'une mise à jour ne peut pas être falsifiée entre la publication et
  l'installation. **Gratuite, obligatoire** — sans elle, l'updater
  refuse d'appliquer un paquet. Ce n'est pas une décision.
- **Signature de code Windows** (certificat d'éditeur) : enlève
  l'avertissement SmartScreen « éditeur inconnu ». Coûte de l'argent,
  engage une identité. Distincte de la précédente.

## Décisions

1. **Installeur : NSIS, pas MSIX.** Déjà bâti, mesuré à **4,75 Mo**
   ([PLAN.md](../PLAN.md) §3), `installMode: currentUser` — c'est ce
   mode qui pose le raccourci du menu Démarrer et l'AppUserModelID dont
   les notifications ont besoin (passation §7.2). NSIS **ne touche pas à
   `%APPDATA%`** : la base reste où elle est. L'updater Tauri vise NSIS
   nativement ; MSIX se met à jour par App Installer/Store, un autre
   mécanisme, plus lourd. **Le §188 du PLAN.md est corrigé par cet ADR.**

2. **Mise à jour signée par l'updater Tauri (minisign).** Une paire de
   clés est générée **par le Chef Ingénieur** (`tauri signer generate`) :
   la clé **publique** est inscrite dans `tauri.conf.json` (publique par
   nature, elle se commite) ; la clé **privée** est un **secret** qui
   signe chaque publication et ne touche **jamais** le dépôt (§2.4 :
   zéro secret en clair). Sans elle générée, `cargo tauri build` ne
   produit pas d'artefacts de mise à jour — mais le gate pré-push ne
   bundle pas (§7.4), donc le dépôt reste vert entre-temps.

3. **Signature de code Windows : reportée au lancement public.** La bêta
   fermée (20-50 personnes prévenues) tourne avec l'updater signé
   minisign — l'intégrité des mises à jour est assurée. SmartScreen
   affichera « éditeur inconnu » (un clic « Exécuter quand même ») ; le
   choix du certificat (Azure Trusted Signing, ~10 $/mois, ou OV
   classique) se tranche avant le public, pas maintenant. Report assumé.

4. **Canal : GitHub Releases.** Le dépôt y est déjà ; l'endpoint
   `…/releases/latest/download/latest.json` est gratuit et natif pour
   l'updater. Le manifeste et les paquets signés y sont publiés.

5. **Updater piloté depuis Rust**, comme les notifications : la webview
   n'appelle jamais l'API updater, seulement nos commandes. Les
   capabilities restent `core:default` — moindre privilège préservé.

## Ce qui est fait ici

- `tauri-plugin-updater` ajouté, enregistré dans `main.rs`.
- `tauri.conf.json` : bloc `plugins.updater` (endpoint GitHub, clé
  publique **en attente**), `bundle.createUpdaterArtifacts: true`.
- Commandes `update_check` (rend la version disponible, ou rien) et
  `update_install` (télécharge, applique, redémarre).
- Bandeau discret, **hors de tout `<header>`** (dette CSS, passation
  §8) : « Une mise à jour est disponible » + « Installer et redémarrer »
  / « Plus tard ». Vérification au démarrage, une fois, silencieuse hors
  ligne — un contrôle que l'utilisateur doit réclamer n'aurait pas lieu
  (leçon de l'[ADR 0007]).

**Surface de test honnête.** L'updater est presque entièrement de la
configuration éditeur et de l'I/O de plateforme (téléchargement,
remplacement du binaire en cours d'exécution) : il n'y a pas de décision
pure à extraire et à tester en RED, contrairement à la synchro ou aux
fils. Le dire plutôt que simuler un test qui n'apprendrait rien (§2.4).
La preuve est **au terrain**, comme pour les notifications.

## Validation terrain (2026-07-26 — faite, bout en bout)

La boucle complète touche la signature, le réseau et le remplacement du
binaire vivant — elle ne se prouve que sur l'application installée
(comme les notifications, §7.2). **Jouée et validée** : la paire de clés
minisign générée (privée hors dépôt, publique dans `tauri.conf.json`),
une `0.1.1` bâtie signée, installée, lancée ; une `0.1.2` publiée en
Release GitHub ; la `0.1.1` installée a **détecté, téléchargé et
appliqué** la `0.1.2`, base intacte. Bandeau, redémarrage, version neuve,
zéro perte.

### Deux pièges du terrain, et leur remède permanent

Aucun n'était un défaut de logique — les deux étaient des **hypothèses
fausses sur l'outillage** (l'enseignement de fond, passation §9) :

1. **Le `latest.json` écrit à la main s'est corrompu** : un collage
   PowerShell multi-ligne a fini par écrire le *texte des commandes*
   dans le fichier ; et `Set-Content -Encoding utf8` y poserait un BOM
   que l'updater (`serde_json`) refuse.
2. **Le `.exe` renvoyait 404** : le manifeste pointait
   `releases/download/v0.1.2/…` alors que le tag de la Release est la
   **version nue** (`0.1.2`). Le bandeau apparaissait — la détection
   marchait — mais l'installation échouait sur le téléchargement.

**Remède :** [`scripts/faire-release.ps1`](../../scripts/faire-release.ps1)
`<version>` — lit le `.sig` bâti, écrit le `latest.json` **sans BOM** et
avec l'**URL au tag nu**. La publication (attacher les trois fichiers au
tag) reste manuelle. La friction est encodée une fois, plus jamais
repayée.

### Convention de publication (figée)

- **Tag = version nue** : `0.1.2`, jamais `v0.1.2`.
- La Release doit être **publiée**, ni brouillon ni *pre-release*, et
  marquée *latest* — sinon `releases/latest/download/…` renvoie 404.
- Les trois assets : `discovery_<version>_x64-setup.exe`, son `.sig`, et
  `latest.json`.

## Conséquences et limites

- **Pas de rollback d'update** : si une `0.1.1` est mauvaise, on publie
  une `0.1.2`. L'updater ne redescend pas de version — cohérent avec
  « le doublon est pire que le retard » : on avance, on ne revient pas.
- **SmartScreen prévient** en bêta : à documenter dans l'invitation.
- **Le manifeste `latest.json` est public** : il ne contient qu'une
  version, une URL et une signature — aucune donnée utilisateur.
