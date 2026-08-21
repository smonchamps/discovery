# PLAN-RETOURS-5 — l'écho d'envoi dit vrai, l'adresse se complète

> **CHANTIER SOLDÉ le 2026-08-21 — terrain complet.** Chantier ouvert,
> GO CE du plan (STOP 1, D1-D5), implémenté, terrain validé (STOP 2,
> cinq points — le point 2 instruit puis rejoué en régime établi :
> « l'entrée temporaire s'affiche correctement puis disparaît »),
> **livré en 0.3.0** (publiée et vérifiée §2.10 le même jour,
> auto-update 0.2.1 → 0.3.0 **confirmé au terrain** — chaîne signée
> ADR 0013 prouvée vivante), le tout le MÊME jour. Commits : `6f94922`
> (feat, CI verte run 32475242855), `5951925` (notes CHANGELOG),
> `5a017d2` (release). Journal Système **A65**. Revue à regard neuf :
> 2 trouvailles confirmées, corrigées ; 1 assumée. Chiffres :
> `completer_adresses` pire cas (préfixe 1 lettre, 50 000
> correspondants) **22 ms** (banc `banc_completer_50k`, release) ;
> rattrapage unique de l'existant **142 ms** sur 200 000 enveloppes
> (banc `banc_rattrapage_200k`) ; tests Rust 347 → **348**, e2e
> 97 → **99**. Reports : **D-27** (la boîte d'envoi ne retente qu'en
> fin de cycle ou au geste — né du terrain, envois jamais perdus).
> Enseignement gravé (STANDARD §9) : lancer l'exe release NU avec
> `2> fichier` ne trace RIEN — le lanceur doit ATTENDRE le processus
> (`cargo run … --release 2> fichier`).
>
> Chantier ouvert le 2026-08-21 (`/chantier`), sur trois retours CE :
> (1) ETAT périmé — les chantiers perf-lecture et envoi de pièces
> jointes sont réalisés ; (2) bug — une entrée anormale apparaît dans
> « Envoyés » à l'envoi d'une pièce jointe (capture fournie), la vraie
> ligne n'arrivant que quelques minutes plus tard ; (3) demande —
> autocomplétion des adresses connues avec gestion du nom d'affichage
> pour À, Cc et Cci.

---

## Constat — faits vérifiés sur pièces (2026-08-21)

### 1. ETAT porte deux lignes périmées

- **« Envoi de pièces jointes (lecture seule en v1) »** figure encore
  aux reports assumés d'[ETAT.md](ETAT.md). Faux depuis longtemps :
  PLAN-PIECES-JOINTES est **soldé et archivé** (E1 `38cd812` : octets
  au brouillon, journal d'envoi, multipart/mixed SMTP ; E3 `27ed056` :
  transfert réel, plan soldé). La capture du CE montre d'ailleurs un
  envoi avec pièce qui fonctionne.
- **« Prochain chantier : perf-lecture »** (corps à la demande ~7 s au
  lancement, terrain du 2026-08-19). Aucun commit dédié n'existe au
  dépôt ; le CE constate le chantier réalisé. Lecture des faits : la
  0.2.1 (PLAN-DEFILEMENT-PROFOND, A64) a sorti les comptages du chemin
  d'affichage et allégé `nav_snapshot` — « démarrage et premiers
  affichages immédiats » confirmés au terrain. Le fondement exact du
  retrait est la décision **D1**.

### 2. Le bug : l'entrée anormale d'« Envoyés » est l'écho local, mal affiché

Le mécanisme lui-même est **voulu** (PLAN-REACTIVITE E3, R-D1
« < 1 s ») : à l'acceptation SMTP, un écho local naît dans « Envoyés »
et meurt à la réconciliation quand la copie serveur entre (même
`message_id`) — d'où le « quelques minutes plus tard, le vrai message
apparaît ». Ce qui est anormal, c'est **l'affichage de l'écho**, sur
deux points, tous deux lus dans le code et concordants avec la capture :

1. **« À : envoyes »** — la tranche des échos de
   [nav.rs:501](../crates/mail-core/src/nav.rs) sert `ec.destination`
   dans la **colonne des destinataires** (commentaire : « sa
   destination EST le destinataire à afficher » — faux : `destination`
   est le slug de catégorie `envoyes`, pas une adresse). La liste
   d'Envoyés affiche « À : `to_addrs` » ([Liste.svelte:46](../apps/desktop/ui-v2/src/Liste.svelte)),
   la tête de message « adresse · à `to_addrs` »
   ([Fil.svelte:93](../apps/desktop/ui-v2/src/Fil.svelte)) → pendant la
   fenêtre de réconciliation, l'écran dit « à envoyes ». La table
   `echos` ne stocke pas les destinataires ; **l'outbox les porte**
   (`outbox.recipients`, lisible via `origin_outbox_id` ; l'enveloppe
   source porte `to_addrs` pour les échos de geste).
2. **Section « FICHIERS JOINTS » vide** —
   [Fil.svelte:239](../apps/desktop/ui-v2/src/Fil.svelte) rend la
   section dès `attachment_count > 0`, mais
   [fil.svelte.js:133](../apps/desktop/ui-v2/src/lib/fil.svelte.js)
   ne rapatrie jamais les métadonnées de pièces d'un écho (elles n'ont
   pas de `(mailbox, uid)`) : un titre sans rien dessous. **Les
   métadonnées existent** : `outbox_attachments` garde nom/mime/taille
   après la purge des octets (PJ-D7).

Non reproduit en local sur la vraie base (STANDARD §7.1 — la base du
CE ne se lit pas d'ici) ; la racine est déterminée par lecture du code
et la capture la confirme point par point. Le RED e2e la prouvera sur
décor.

### 3. L'autocomplétion : ce que le produit sait déjà

- Les champs À/Cc/Cci sont des `input` texte nus
  ([Composition.svelte:878](../apps/desktop/ui-v2/src/Composition.svelte)) ;
  `EmailAddress::parse` **refuse chevrons, virgules et blancs**
  (anti-injection d'en-têtes, [address.rs](../crates/mail-core/src/address.rs))
  — le chemin d'envoi ne connaît que l'adresse nue.
- La base connaît les correspondants : chaque enveloppe porte
  `(sender, sender_address)` — **nom d'affichage compris** — et,
  depuis PLAN-RETOURS-MAIL, `to_addrs`/`cc_addrs` (adresses nues,
  jointes par `\n`). Aucune table de contacts n'existe, aucun index
  dédié aux préfixes d'adresse.

## Périmètre

**Dans ce chantier** : ETAT remis d'équerre ; l'écho d'envoi qui dit
ses vrais destinataires et ses pièces (cœur + nav + front, migration
additive) ; l'autocomplétion À/Cc/Cci (requête cœur + menu clavier au
composeur) selon les décisions D3-D5 ; e2e des deux comportements ;
Système amendé (DC-D2) ; gate complète ; terrain.

**Refus de périmètre explicites (STANDARD §2.6) :**
- **Pas de refonte de l'écho ni de sa réconciliation** : la fenêtre de
  quelques minutes est le fonctionnement voulu (la copie serveur
  arrive par la passe d'après-geste puis le cycle) ; on corrige ce que
  l'écho *dit*, pas quand il meurt.
- **Pas de carnet d'adresses** (création/édition de contacts) :
  l'autocomplétion tire du courrier vu, rien de plus. Un carnet serait
  un chantier produit à part entière.
- **Pas d'envoi name-addr SMTP** si D3 tranche l'insertion en adresse
  nue : toucher `EmailAddress`/compose/SMTP pour porter des noms
  d'affichage dans l'enveloppe est un chantier de validation à part
  (risque d'injection d'en-têtes — la raison d'être du refus des
  chevrons).
- **Pas de nom d'affichage de l'expéditeur sur l'écho** (il montre
  l'adresse du compte, pas son nom) : cosmétique, fenêtre de quelques
  minutes, mort à la réconciliation.

## Options et verdicts

### O1 — Où l'écho prend-il ses destinataires ?

| Option | Mécanisme | Verdict |
|---|---|---|
| **A. Colonne `to_addrs` sur `echos`**, remplie à la création (envoi : `outbox.recipients` reformaté `\n` en Rust ; geste : `envelopes.to_addrs` copié tel quel) | Migration additive (patron existant des colonnes d'enveloppes) ; `nav.rs` sert `ec.to_addrs` comme les vraies lignes | **Retenue.** Un seul format (`\n`), aucune conversion de séparateur en SQL, la tranche des échos reste sans jointure nouvelle ; couvre AUSSI les échos de geste |
| B. `LEFT JOIN outbox` dans la tranche des échos de `nav.rs` | Pas de migration | Rejetée : `outbox.recipients` a SON séparateur (`TO_SEPARATOR`) ≠ `\n` des enveloppes — conversion en SQL fragile ; ne couvre pas les échos de geste (pas d'outbox) |

### O2 — Les pièces de l'écho (décision CE, D2)

- **a. Servir les métadonnées** : commande `echo_attachments`
  (nom + taille depuis `outbox_attachments` via `origin_outbox_id`),
  puces **inertes** (les octets sont purgés à `sent` — rien à
  enregistrer pendant la fenêtre). Échos de geste : section absente
  (leurs métadonnées meurent avec la source). *Recommandée : l'écran
  dit ce qui est parti, sans mentir sur ce qu'il peut faire.*
- **b. Masquer la section** pour tout écho (la puce « N fichiers » de
  l'inventaire du fil reste, elle est vraie). Moins de code, moins
  d'information pendant la fenêtre.

### O3 — L'autocomplétion, forme retenue (détails aux décisions D3-D5)

Cœur : commande `completer_adresses(prefixe, limite)` — requête sur
les enveloppes en portée, appariement préfixe sur l'adresse ET le nom,
**classement récence + fréquence** (le correspondant récent et
fréquent d'abord), indésirables et corbeille exclus, dédoublonnage par
adresse (le nom le plus récent gagne). Front : menu sous le champ
actif (À, Cc, Cci — le segment après la dernière virgule), clavier
↓ ↑ Entrée Échap, clic ; suggestion affichée « Nom — adresse ».
Mesure avant livraison : la requête sous ~50 ms sur décor 200 k
(STANDARD §3, budget de frappe), sinon index dédié.

## Étapes

- **E1 — ETAT d'équerre** (docs seuls) : report « envoi de pièces
  jointes » retiré (renvoi à PLAN-PIECES-JOINTES archivé) ;
  perf-lecture retiré selon D1 ; « prochain chantier » remis sur la
  bêta fermée (ou ce que D1 en dit). Gate : relecture, cohérence avec
  archives.
- **E2 — l'écho dit ses destinataires** (TDD) : RED sur `nav.rs` (la
  ligne d'écho d'envoi porte les destinataires de l'outbox, jamais
  `envoyes`) et sur `echo_envoi`/`geste_avec_echo` (la colonne se
  remplit) ; migration additive `echos.to_addrs` ; nav sert la colonne
  (NULL → liste vide, repli d'avant R4). e2e : un envoi avec pièce
  montre « À : adresse » dans Envoyés pendant la fenêtre d'écho.
- **E3 — les pièces de l'écho** (selon D2) : commande
  `echo_attachments` + rendu des puces inertes, OU masquage de la
  section pour un écho. e2e : plus jamais un titre « Fichiers joints »
  sans rien dessous.
- **E4 — l'autocomplétion, le cœur** (si D5 la garde ici) : fonction
  pure de classement (décision testable), requête mesurée sur décor
  200 k, commande exposée. RED d'abord.
- **E5 — l'autocomplétion, le composeur** : menu, clavier, insertion
  selon D3, Système amendé au même commit (DC-D2), e2e du parcours
  (taper 3 lettres → choisir → l'adresse au champ, Cc et Cci compris).
- **E6 — qualité et sortie** : revue à regard neuf (`/code-review
  high`), gate complète, **terrain (STOP 2)**, docs (A-n, ETAT,
  DETTE si report), version selon §2.9 : correctif seul → **0.2.2**
  (CORRECTIF) ; avec l'autocomplétion → **0.3.0** (MINEUR, capacité
  nouvelle).

## § Réalisation (2026-08-21)

- **E1** : ETAT amendé — perf-lecture éteint (D1), report « envoi de
  pièces jointes » retiré (livré depuis PLAN-PIECES-JOINTES), chantier
  en cours consigné.
- **E2/E3 (l'écho dit vrai)** : colonne `echos.to_addrs` (migration
  additive), remplie à la naissance de l'écho — envoi : copie de
  `outbox.recipients` (déjà au format `\n` des enveloppes) ; geste :
  copie de `envelopes.to_addrs`. `nav.rs` sert `ec.to_addrs` (le slug
  de destination ne fuit plus). `echo_attachments` (cœur + commande)
  sert nom/mime/taille depuis `outbox_attachments` ; le front rend les
  puces **inertes** et ne montre la section que si des puces existent.
  RED montrés avant l'implémentation (4 tests cœur + nav). Décor e2e :
  un écho d'envoi avec pièce vit dans le seed (`TransportDecor`, vrai
  chemin `flush_outbox`). La preuve de réconciliation (D2) reste tenue
  par `la_vraie_ligne_tue_l_echo` (cœur) + vérification terrain.
- **E4 (annuaire des correspondants)** : table `correspondants`
  (adresse PK minuscule, nom, récence, fréquence) — **jamais un
  parcours d'`envelopes` par frappe** (leçon A64). Alimentée : synchro
  (messages NEUFS seuls — pas de double compte au re-sync ; expéditeurs
  hors indésirables/corbeille, destinataires du dossier d'envois),
  mise en file d'un envoi, rattrapage des destinataires
  (`set_recipients`, ajout de revue) ; rattrapage UNIQUE de l'existant
  à l'ouverture (set-based, marque `prefs`
  `annuaire_correspondants_v1`, 142 ms/200 k). `completer_adresses` :
  préfixe sur adresse/nom/mot du nom (LIKE échappé), classement
  `score()` pur (paliers récence × fréquence), 22 ms pire cas.
  Écrit sans RED préalable (tests et implémentation dans la même
  passe — dit ici, pas simulé) ; 9 tests dont 2 bancs `#[ignore]`.
- **E5 (composeur)** : menu sous À/Cc/Cci, segment après la dernière
  virgule, dès 2 caractères, débobiné 150 ms + jeton dernier-gagne ;
  ↓ ↑ Entrée/Tab Échap, clic ; nom montré, **adresse nue insérée**
  (D3). Système amendé **A65** (DC-D2). e2e : 2 parcours neufs
  (99 au total, 58/58 verts sur le spec en local).
- **Revue à regard neuf** (`/code-review high`) : (1) l'écho de GESTE
  montrait aussi un titre « Fichiers joints » vide → section
  conditionnée aux puces réelles, corrigé ; (2) les destinataires
  rattrapés par `backfill_recipients` n'entraient pas à l'annuaire →
  `set_recipients` note désormais (test dédié), corrigé ; (3) assumé :
  l'intégrale Gmail double la fréquence uniformément (le classement
  relatif ne bouge pas).

## § Terrain (STOP 2, 2026-08-21) — validé

Checklist en cinq points, verdicts CE :

1. **Premier lancement** (rattrapage de l'annuaire sur la vraie base) :
   OK, rien d'anormal.
2. **L'écho d'envoi** : d'abord NON REPRODUCTIBLE — les entrées
   temporaires n'apparaissaient pas, « Boîte d'envoi · 2 envois en
   attente » à demeure. Instruction : l'écho ne naît qu'à `sent` ; les
   envois n'étaient jamais partis. Racine : au premier lancement, la
   vidange d'après-envoi passe avant que les sessions soient prêtes,
   et la boîte d'envoi n'a pas de retentative propre (→ **D-27**,
   assumé — les messages sont partis à la première vidange déclenchée,
   jamais perdus ni doublés). Deux passes de trace brûlées sur le
   piège `2> fichier` + exe fenêtré (STANDARD §9 affiné, une trace de
   vidange ajoutée à `run_flush_all` au passage). **Rejoué en régime
   établi : OK** — l'entrée temporaire s'affiche correctement (vrais
   destinataires, pièce nom + poids inerte) puis disparaît à l'arrivée
   de la copie serveur.
3. **Écho de geste** (pas de section « Fichiers joints » vide) : OK.
4. **Autocomplétion** (À/Cc/Cci, nom montré, adresse nue, clavier et
   clic, indésirables exclus) : OK.
5. **Budget de frappe** (fluide pendant la synchro) : OK.

## § Décisions CE — tranchées le 2026-08-21

- **D1 — perf-lecture à ETAT** : sur quel fondement le marquer
  réalisé ? — *Réponse CE (2026-08-21) : « Éteint par la 0.2.1 »* —
  le symptôme (~7 s au lancement) est mort au terrain depuis la
  0.2.1 ; le chantier est retiré d'ETAT, à rouvrir si le terrain le
  redit.
- **D2 — pièces de l'écho** : servir nom + taille (puces inertes) ou
  masquer la section ? Le CE a d'abord proposé un dossier « Boîte
  d'envoi » local portant les entrées temporaires ; confronté à deux
  faits (l'écho naît APRÈS l'acceptation SMTP — le message est déjà
  parti ; et « la copie dans Envoyés en < 1 s » est un comportement
  qu'il a validé au terrain, PLAN-REACTIVITE), il a tranché — *Réponse
  CE (2026-08-21), mot pour mot : « Je n'avais pas bien compris les
  implications de ma décision sur PLAN REACTIVITE. Corrige l'écho dans
  Envoyés comme tu le recommandes, et assure toi bien que quand le
  message envoyé est réellement synchronisé du serveur, l'écho
  disparaisse. »* — donc : nom + poids en puces inertes dans Envoyés,
  pas de nouveau dossier, et la **preuve exigée** que la
  réconciliation retire l'écho à l'arrivée de la copie serveur
  (test + vérification terrain, ajoutés à E2/E6).
- **D3 — nom d'affichage à l'insertion** : — *Réponse CE
  (2026-08-21) : « L'adresse nue »* — le menu montre le nom, insère
  l'adresse ; le chemin d'envoi ne change pas.
- **D4 — sources des suggestions** : — *Réponse CE (2026-08-21) :
  « Expéditeurs + nos destinataires »* — indésirables et corbeille
  exclus, dédoublonnage par adresse, le nom le plus récent gagne.
- **D5 — découpage** : — *Réponse CE (2026-08-21) : « Tout ce
  chantier, une 0.3.0 »* — une seule publication MINEUR, qui emporte
  le correctif.
