# Journal des versions

Toutes les modifications notables de Wind sont consignées ici.

Le format s'inspire de [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/),
et le projet suit le [versionnage sémantique](https://semver.org/lang/fr/) :
la règle d'incrémentation `MAJEUR.MINEUR.CORRECTIF` propre à Wind (Wind
n'exposant aucune API publique) est fixée dans
[`docs/PASSATION.md`](docs/PASSATION.md) §2.9.

Les paquets signés et leurs notes vivent dans les
[Releases GitHub](https://github.com/smonchamps/wind/releases) ; la mise à
jour est automatique et signée (minisign, ADR 0013).

## [0.2.1] - 2026-08-20

La liste ne se fige plus, et tout s'affiche plus vite.

### Corrigé

- **Défiler vite à la barre ne bloque plus l'application** : tirer
  rapidement la barre de défilement d'un grand dossier (Archives, par
  exemple) laissait la liste en blocs « … », puis affichait à tort
  « Aucun message ici. » dans tous les dossiers pendant plusieurs
  minutes. La liste ne demande plus que ce qu'elle montre : à l'arrêt
  du geste, les messages apparaissent aussitôt, et changer de dossier
  répond immédiatement.
- **L'écran vide ne ment plus** : « Aucun message ici. » ne s'affiche
  qu'une fois la boîte réellement consultée — pendant le chargement,
  des lignes d'attente le disent honnêtement.

### Modifié

- **Le démarrage et le premier affichage d'un dossier sont
  immédiats** : les comptages internes (dont le plus coûteux, celui des
  Archives Gmail) ne retardent plus l'affichage des messages — le
  nombre d'éléments et la barre de défilement s'ajustent juste après,
  d'eux-mêmes.

## [0.2.0] - 2026-08-20

La mise en forme arrive dans le composeur.

### Ajouté

- **Une vraie barre de mise en forme** dans la fenêtre de composition :
  police (sans serif, serif, monospace), taille (quatre crans), gras,
  italique, souligné, barré, couleur du texte (nuancier de douze
  teintes), alignement gauche/centre/droite, listes à puces et
  numérotée, retrait, et « Effacer la mise en forme ». Les raccourcis
  Ctrl+B/I/U fonctionnent aussi.
- Vos messages partent désormais **en HTML avec un repli texte
  automatique** : les destinataires voient votre mise en forme, et les
  clients texte reçoivent toujours une version lisible.
- **La citation d'une réponse** apparaît dans un bloc au filet gauche,
  comme dans les clients mûrs — votre réponse s'écrit au-dessus.
- **Les brouillons conservent la mise en forme**, y compris après un
  aller-retour avec le dossier Brouillons de votre messagerie.
- **Reconnecter un compte** : quand la connexion d'un compte expire ou
  est révoquée, Réglages > Comptes le signale (« Déconnecté ») et un
  bouton « Reconnecter » relance l'autorisation dans le navigateur —
  sans rien perdre ni re-synchroniser. L'avis d'accueil mène
  directement à cette page.

### Modifié

- **Les images distantes, selon le geste** : la citation d'une réponse
  les remplace par un pixel neutre (aucun traceur du message cité ne se
  charge à votre insu) ; un transfert, lui, transmet le message entier,
  images comprises.

## [0.1.11] - 2026-08-19

Trois retours du terrain.

### Modifié

- **Enregistrer une pièce jointe** : cliquer une pièce ouvre désormais une
  fenêtre « Enregistrer sous » où vous choisissez le dossier et le nom du
  fichier, au lieu d'un enregistrement silencieux dans Téléchargements.
- Le **nom et le poids d'une pièce jointe** sont réunis dans une seule
  puce, plus lisible d'un coup d'œil.

### Corrigé

- Sur les **thèmes sombres**, le corps des messages s'affiche désormais sur
  fond clair : le texte des courriels — souvent composé pour un fond blanc,
  comme les infolettres — redevient lisible, au lieu d'apparaître parfois en
  noir sur fond sombre.

## [0.1.10] - 2026-08-18

Quatre retours du terrain.

### Ajouté

- **Signaler un courrier comme indésirable**, et l'inverse : un bouton
  « Signaler comme spam » déplace la conversation vers le dossier
  indésirable de votre messagerie — c'est elle qui apprend. Depuis le
  dossier Indésirables, « Ce n'est pas un spam » la ramène en Réception.
- **Supprimer un brouillon** directement depuis la fenêtre de composition,
  d'un seul geste et après confirmation — distinct de « Annuler », qui,
  lui, conserve le brouillon.
- **Répondre message par message** : les boutons Répondre, Répondre à tous
  et Transférer sont désormais au bas de chaque message d'une conversation,
  pour répondre précisément à celui que vous lisez — vos propres messages
  compris, auquel cas la réponse repart vers les destinataires d'origine.

### Modifié

- Le **rattrapage des messages** affiche un pourcentage de progression dans
  la barre d'état, à côté du nombre de messages restants.

## [0.1.9] - 2026-08-17

Quatre retours du terrain.

### Ajouté

- **Cc et Cci** dans le composeur : ajoutez des destinataires en copie et
  en copie cachée. La copie cachée reste cachée (elle ne paraît jamais
  dans le message reçu par les autres) ; « Répondre à tous » replace les
  Cc d'origine en Cc.

### Modifié

- La **synchronisation Gmail est bien plus légère** : le balayage complet
  des dossiers, qui pouvait durer et se répétait toutes les 5 minutes,
  passe à toutes les 30 minutes. L'arrivée du nouveau courrier, elle,
  reste **instantanée** — rien ne change à ce que vous recevez, seulement
  au poids de fond.
- L'**animation de chargement** (le trait) est simplifiée : une animation
  complète et fluide dès qu'une action est en cours, au lieu d'un trait
  qui pouvait rester figé.

### Retiré

- Le bouton « Rendre indépendante » du composeur, qui ne faisait rien, est
  retiré (la fenêtre de composition détachée reviendra plus tard).

## [0.1.8] - 2026-08-16

Quatre correctifs sur le courrier réel, remontés au terrain.

### Corrigé

- Les objets et les noms d'expéditeur ne montrent plus les antislashs
  parasites des chaînes IMAP entre guillemets (ex. `Test \"Envoyés\"`) ;
  les messages déjà synchronisés sont réparés au premier lancement.
- Le dossier « Envoyés » affiche enfin le vrai destinataire (« À : … »),
  dans la liste comme à la lecture, au lieu de répéter votre propre
  adresse ; l'information est rattrapée sur les envois déjà synchronisés.
- « Répondre à tous » pré-remplit « À » instantanément, à partir des
  destinataires stockés, sans plus attendre une relève du serveur à
  chaque clic.
- L'objet ne s'affiche plus en double en tête du corps de certaines
  infolettres (le titre de leur en-tête HTML ne fuit plus dans le
  message).

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

[0.1.11]: https://github.com/smonchamps/wind/releases/tag/0.1.11
[0.1.10]: https://github.com/smonchamps/wind/releases/tag/0.1.10
[0.1.9]: https://github.com/smonchamps/wind/releases/tag/0.1.9
[0.1.8]: https://github.com/smonchamps/wind/releases/tag/0.1.8
[0.1.7]: https://github.com/smonchamps/wind/releases/tag/0.1.7
[0.1.6]: https://github.com/smonchamps/wind/releases/tag/0.1.6
[0.1.5]: https://github.com/smonchamps/wind/releases/tag/0.1.5
[0.1.4]: https://github.com/smonchamps/wind/releases/tag/0.1.4
[0.1.3]: https://github.com/smonchamps/wind/releases/tag/0.1.3
[0.1.2]: https://github.com/smonchamps/wind/releases/tag/0.1.2
[0.1.1]: https://github.com/smonchamps/wind/releases/tag/0.1.1
