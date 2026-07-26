# ADR 0012 — Migration visible et interruptible : l'adoption se rembobine

Date : 2026-07-26 · Statut : accepté (validé au terrain le jour même)

## Contexte

L'adoption d'une base héritée — rattacher chaque message à son fil —
coûte **3,69 s à 200 000 messages** (gate 3), payés en silence par la
première commande qui ouvre la base, dans un gel d'interface. Déjà
ramenée de 11,1 s par deux correctifs mesurés ; le reste est le coût
intrinsèque de l'union-find. L'[ADR 0010](0010-synchronisation-integrale.md)
en a fait un **prérequis de la Phase 5** : la boîte réelle (256 312
messages) dépasse l'échelle du gate 3, et chaque future évolution de la
règle de regroupement rejouera cette passe.

Le précédent est l'[ADR 0007](0007-rattrapage-des-corps.md) : un travail
long doit être **visible et interruptible**. Mais l'asymétrie est
entière, et c'est elle qui décide de la forme :

| | Rattrapage des corps (0007) | Adoption des fils (ici) |
|---|---|---|
| La liste en dépend ? | non — elle vit sans corps | **oui — elle part de `threads`** |
| Fractionnable ? | oui, par lots de 200 | **non** : une adoption partielle persistée est une boîte à moitié vide |
| « Interrompre » | cesser de rappeler la commande | **tout défaire**, et se rejouer entière plus tard |

## Décision

**L'adoption devient une unité transactionnelle unique, rapportée et
annulable.**

1. **Une seule transaction** dans `Store::init`, du `DROP` conditionnel
   des tables de fils jusqu'à `PRAGMA user_version` : suppression,
   recréation du schéma, détachement des enveloppes, adoption, version.
   Avant, `user_version` avançait dans sa propre transaction *avant*
   l'adoption — c'est précisément ce qui interdisait le rembobinage.
   Annuler = `ROLLBACK` : la base revient à l'état d'avant l'ouverture,
   `user_version` inchangé, et la passe entière se rejoue au prochain
   lancement. Le `BEGIN` est *deferred* : sur une base à jour rien
   n'écrit, la transaction reste lectrice et ne rencontre jamais
   l'écrivain d'une synchro longue (leçon de l'[ADR 0011](0011-journal-wal.md)).

2. **`Store::open_with_progress(path, on_progress)`** : le rappel reçoit
   `(fait, total)` tous les 1 000 pas et répond `ControlFlow` —
   `Break` = annuler. Le total est un **majorant déclaré d'emblée**
   (rattacher chaque orphelin + consolider au plus autant de fils) : il
   ne bouge jamais en route, une barre qui recule étant pire qu'une
   barre imprécise. `(total, total)` n'est annoncé qu'après `COMMIT` —
   jamais « 100 % » avant que ce soit vrai. L'affichage réutilise
   `sync_percent`, qui porte déjà les cas dégénérés.

3. **`Store::pending_adoption(path)`** : une sonde en **lecture seule**
   (ni fichier créé, ni migration déclenchée) pour que le desktop sache
   s'il doit afficher l'écran AVANT la première vraie ouverture.

4. **Côté desktop**, quatre commandes (`migration_check`, `migration_run`,
   `migration_progress`, `migration_cancel`) et un écran modal qui bloque
   tout le démarrage tant que la base n'est pas adoptée. Nécessaire parce
   que **chaque commande ouvre sa propre connexion** : sans porte, la
   première venue paierait la passe. Après « Annuler », l'écran propose
   « Reprendre » — montrer la boîte sans la passe est impossible, par
   construction.

## Preuves

- **Rembobinage** (`annuler_l_adoption_defait_tout_et_laisse_user_version_inchangee`) :
  annulation au 1 000ᵉ message d'une vraie base de fichier → `user_version`
  intact, tables v1 intactes (le `DROP` aussi est défait), zéro message
  perdu ; la réouverture rejoue la passe entière et la liste est complète.
- **Observabilité** (`l_adoption_annonce_son_avancement_du_depart_a_la_fin`) :
  total annoncé d'emblée, avancement jamais décroissant, « fini » dit une
  seule fois, à la fin.
- **Silence du cas courant** (`une_base_a_jour_s_ouvre_sans_annoncer_de_migration`,
  `la_sonde_dit_quand_une_adoption_attend_sans_la_declencher`) : aucun
  faux bandeau, aucune trace laissée par la sonde.
- **Banc** (`banc_migration_fils`, gate3.db, 200 000 messages) :

  | | avant (référence) | après |
  |---|---|---|
  | adoption (base héritée) | 3,69 s | **3,66 s** |
  | à jour (cas courant) | ~2,5 ms | **2,5 ms** |
  | regroupement | 160 000 fils, 0 orphelin | identique |

  Le coût des paliers de rapport est invisible ; la transaction unique ne
  change pas le prix de la passe.

## Validation terrain (2026-07-26, sur copies — jamais la vraie base)

- **Copie de la base réelle** (256 312 messages) rembobinée à
  `user_version = 0` : écran affiché **moins d'une seconde** — la portée
  à adopter (~7 500 messages, INBOX + Envoyés) est petite devant la
  base ; liste complète à l'arrivée. À cette échelle, la migration est
  quasi imperceptible : c'est le décor du gate 3 qui justifie l'écran.
- **Copie de gate3.db** (200 000 messages, tous en portée) : écran avec
  barre qui monte ~4 s ; **« Annuler » en pleine passe** → message
  d'annulation, pas de liste ; application fermée puis relancée →
  **l'écran se représente du début**, preuve du rembobinage à l'échelle ;
  passe laissée finir → liste complète.

**Trouvaille collatérale de cette validation** (défaut préexistant,
corrigé le jour même) : `#detail { display: flex }` écrasait `[hidden]`
(spécificité d'ID contre la feuille du navigateur) — le panneau de
lecture restait rendu en permanence et son iframe sandboxée captait le
premier clic : le focus clavier partait dans l'iframe et les raccourcis
mouraient tant qu'on ne cliquait pas ailleurs. Trois garde-fous ajoutés
(`#detail`, `#detail-note`, `#compose-from-row`), un E2E le tient.

## Conséquences et limites assumées

- **Une deuxième instance** de l'application lancée PENDANT une migration
  verrait ses commandes échouer après le `busy_timeout` (5 s). Risque
  préexistant et inchangé — l'unité ne l'aggrave pas, elle le borne dans
  le temps.
- L'annulation a une **latence d'un palier** (1 000 pas, ~20 ms au rythme
  du banc) : imperceptible, et le bouton se désarme au premier clic.
- Le total affiché est un majorant : la barre saute en avant à la fin
  (consolidation plus courte que l'estimation), jamais en arrière.
- La passe reste **bloquante pour l'usage** pendant quelques secondes à
  l'échelle réelle — c'est le choix : la fractionner est impossible sans
  mentir à la liste (§ Contexte).
