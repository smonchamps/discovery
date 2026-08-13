# Plan — Réglages en deux volets

Commande du Chef Ingénieur (2026-08-12) : restructurer la surimpression
Réglages en deux volets — à gauche la liste des GROUPES (Comptes,
Thèmes, Affichage, Notifications, Raccourcis, À propos), à droite les
réglages du groupe choisi.

Le prototype est muet sur cette surface (il ne montre que la liste des
thèmes) : le Système complète (A6), l'amendement **A13** portera la
spécification. Aucun budget n'est en jeu — c'est une surimpression.

## 1. Le contrat visuel (A13, à inscrire au journal à la livraison)

- **Surimpression élargie : 800 px** (contre 560), même carte signature
  (bordure gauche accent, rayon 10, ombre), même en-tête 48 px
  (« Réglages », fermer) et même pied (« Terminé »).
- **Volet gauche 220 px** : fond `--panel`, bordure droite `--border` —
  la grammaire de la nav de l'écran 02, réutilisée à l'identique :
  rangées 36 px, icône + libellé, état actif = surface + bordure accent
  gauche + ombre. Clavier : rangées `role="button"`, Entrée/Espace,
  focus visible (A8).
- **Volet droit** : défilant, kicker de section (12 px, capitales,
  `--muted`) + le contenu du groupe. Les blocs existants déménagent
  TELS QUELS (fiches de thèmes, rangées de comptes, carte du guichet).
- Icônes (inventaire A3, tous déjà présents sauf mention) : Comptes
  `person`, Thèmes `bookmark`, Affichage `visibility_off` → **à
  trancher** (candidat propre : `settings` déjà pris par le bouton ;
  voir décision R-D4), Notifications — glyphe absent de l'inventaire
  (`notifications`, à ajouter par régénération), Raccourcis
  `edit_square` → à trancher aussi (candidat `keyboard`, absent).
  Une régénération de police groupée réglera les manquants (et purgera
  `arrow_forward`, dû A12).

## 2. Les groupes et leur contenu — RÉEL seulement

Règle : un groupe ne s'expédie qu'avec du contenu réel. Aucun réglage
inventé pour meubler.

| Groupe | Contenu à la livraison | Source |
|---|---|---|
| **Comptes** | rangées des comptes réels + « Ajouter un compte » (guichet A11) | existant, déménagé |
| **Thèmes** | les 7 fiches, coche, application immédiate | existant, déménagé |
| **Affichage** | **décision R-D1** — candidats réels : OS sombre automatique (D6), formes de dates en jour de semaine (dette D-3) | à câbler si tranché |
| **Notifications** | **décision R-D2** — bulles d'arrivée on/off : le réglage n'existe pas au cœur (préférence à persister en base, lue par le shell qui émet) | capacité neuve |
| **Raccourcis** | la table D3 en RÉFÉRENCE (c/r/f/e/Suppr/«/»/Échap) — lecture seule, pas de re-mappage | contenu statique |
| **À propos** | version de l'application, « Vérifier les mises à jour » (`update_check`/`update_install`, le même flux que la fente d'avis), licence de la police d'icônes | commandes existantes |

## 3. Livraison en deux temps

### E1 — la structure et les groupes déjà pleins

Comptes, Thèmes, Raccourcis, À propos. Affichage et Notifications
n'apparaissent PAS tant que leur décision n'est pas tranchée — un
groupe vide serait une promesse creuse.

Travaux : `Reglages.svelte` restructuré (rail + volet, groupe actif en
`$state`, Échap/Terminé inchangés) ; icônes manquantes régénérées en
une passe ; e2e : navigation entre groupes au clic ET au clavier,
Comptes et Thèmes rejouent leurs parcours existants dans le nouveau
volet, À propos affiche la version.

**Gate E1 :** e2e verts (les parcours Réglages existants passent sans
réécriture de fond), contraste A8 inchangé (aucun jeton neuf), terrain
CE.

### E2 — les groupes à décision

Selon R-D1/R-D2 : Affichage (D6 : `prefers-color-scheme` → `nuit`
automatique, un booléen localStorage comme le thème ; dates D-3 : une
extension de `quand()`) ; Notifications (préférence en base + garde
dans l'émission des bulles — PETITE surface cœur, à spécifier avant de
coder).

**Gate E2 :** idem E1 + un aller-retour réel de chaque réglage neuf
(changer, relancer l'app, constater la persistance).

## 4. Décisions au Chef Ingénieur

| # | Décision | Recommandation |
|---|---|---|
| R-D1 | Contenu d'Affichage à E2 : D6 (OS sombre auto) et/ou dates en jour de semaine (D-3) | Les deux — petits, réels, et D-3 sort du registre de dette |
| R-D2 | Notifications on/off (capacité cœur neuve : préférence persistée + garde à l'émission) | Oui, mais à E2 avec sa spéc courte — c'est le seul morceau non-UI |
| R-D3 | Timing : pendant la fenêtre d'observation B1→B2 ? | Oui — travail additif d'UI, aucun retrait ; la fenêtre n'interdit que le retrait de v1 |
| R-D4 | Icônes des groupes Affichage/Raccourcis (`display_settings`/`keyboard` à ajouter, ou réemploi de l'inventaire) | Ajouter les glyphes justes à la régénération groupée — un réemploi violerait « une icône, un sens » (A3) |

## 5. Refus explicites

- Pas de re-mappage des raccourcis (référence seule) ; pas de réglage
  « signature », « format d'envoi », « polices » — capacités neuves non
  demandées.
- Pas de groupe affiché vide.
- Le comportement actuel (thème immédiat, guichet, Terminé, Échap) ne
  change pas — la structure change, pas les gestes.

---

Le GO du Chef Ingénieur sur ce plan (avec R-D1 à R-D4) ouvre E1.

## 6. Journal de livraison (2026-08-12)

GO du Chef Ingénieur, R-D1 à R-D4 tranchées selon les recommandations.
Amendement **A13** inscrit au journal du Système.

- **E1 livré** : rail 220 px + volet, carte 800 px (hauteur posée
  640 px, bornée à l'écran — le rail ne respire pas au gré du groupe),
  Comptes/Thèmes déménagés tels quels, Raccourcis (table D3 en
  référence), À propos (version réelle via la commande neuve
  `app_version`, `update_check`/`update_install`, licence des icônes).
  Gate : e2e 25/25, contraste A8 sans jeton neuf.
- **Police régénérée en une passe** (R-D4) : +`display_settings`,
  +`notifications`, +`keyboard`, +`info` (À propos, absent du plan,
  tranché au même critère « une icône, un sens »), −`arrow_forward`
  (dû A12) — 35 glyphes, 16 828 octets, inventaire et preuve à jour.
- **E2 livré** (R-D1 : les deux ; R-D2 : oui) : Affichage porte
  « Sombre automatique » (D6, booléen `discovery-theme-auto` ; la
  moitié « thèmes Le vent/Tournesol » de D6 reste après bascule) ; les
  dates D-3 sont soldées DANS `quand()`, sans réglage — la forme du
  prototype ne s'opte pas, le groupe n'affiche que le réglage réel.
  Notifications : « Bulles d'arrivée », spéc courte appliquée — table
  `prefs` (clé/valeur) au SCHEMA idempotent de mail-core,
  `notif_pref_get/set`, garde à l'ÉMISSION dans le shell ; défaut
  activées. Gate : e2e 61/61 (aller-retour réel des deux réglages,
  rechargement compris), contraste inchangé.
- **Refus tenus** : pas de re-mappage, pas de réglage inventé, pas de
  groupe vide ; thème immédiat, guichet, Terminé, Échap inchangés.
- **Terrain CE passé le 2026-08-12** : « E1 et E2 entièrement ok » —
  base réelle, rail au clic et au clavier, aller-retour des deux
  réglages neufs constaté (OS sombre, bulles coupées à la synchro
  réelle), hauteur 640 px non récusée. Les deux gates sont closes ;
  le plan est SOLDÉ.

## 7. Complément (2026-08-13) — retrait d'un compte

Le groupe Comptes gagne le geste inverse de l'A11 : chaque rangée porte
un bouton de retrait (glyphe `delete`, déjà dans l'inventaire — « une
icône, un sens » tenu : c'est bien une suppression). Le geste est
DESTRUCTEUR localement, donc il se confirme sur place, dans une carte
sous la rangée qui dit ce qu'il efface — courrier local et connexion —
et ce qu'il n'efface pas : le serveur, jamais touché.

Côté shell, la commande neuve `remove_account` : coffre de l'OS
D'ABORD (`mail_auth::forget_credentials`, sans exiger de configuration
OAuth — un CLIENT_ID absent ne doit pas retenir un compte), base
ensuite (`Store::delete_account`, une transaction : cascades du schéma
plus index de recherche, brouillons, pierres tombales et boîte d'envoi,
qui n'ont pas de clé étrangère), session en mémoire enfin. Les boucles
qui reposent une session rafraîchie ne ressuscitent plus un compte
retiré pendant leur cycle (`reposer_sessions`). Gate : 374 tests Rust
(le retrait ne laisse rien et épargne le voisin), e2e 73/73 dont la
suite neuve `refonte-retrait-compte` (confirmation, annulation, nav et
liste repliées).
