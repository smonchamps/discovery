# État — l'instantané de relève de Wind

> **Ce document est réécrit à chaque chantier — c'est sa fonction.**
> Version livrée, prochain chantier, chiffres du terrain, arbitrages
> ouverts, reports : tout le volatile vit ici. La méthode, les
> invariants et les pièges vivent dans [STANDARD.md](STANDARD.md) —
> eux ne se réécrivent pas, ils s'amendent.
>
> Extrait de PASSATION.md (§1 + §8) le 2026-08-19 — PLAN-DOCUMENTATION.

---

## Où on en est, et quoi faire en premier

**Rien n'est cassé, rien n'est à moitié écrit, rien n'est en vol.**

**Premier geste : publier la release 0.2.0** — le composeur enrichi
HTML est **livré, terrain complet, CI verte** (`537a1e4`,
PLAN-COMPOSITION-HTML soldé le 2026-08-20) mais **pas encore publié**.
C'est une capacité nouvelle → **MINEUR 0.2.0** (STANDARD.md §2.9,
première bascule `y`+1 du 0.x) :
`scripts/faire-release.ps1 0.2.0` (clé `C:\Keys\wind.key`, mot de passe
à la main, confirmation `OUI`), puis vérification de release
(STANDARD.md §2.10) et auto-update 0.1.11 → 0.2.0 confirmé au terrain.

**Chantier à reprendre de zéro : perf-lecture** (corps à la demande
bridé à ~7 s au lancement, terrain du 2026-08-19). Son WIP non commité
a été **retiré sur décision CE du 2026-08-20** (« qualité trop
aléatoire ») ; le sujet repart par `/chantier` dans une session dédiée.
Matière disponible : les six constats de la revue du 2026-08-20 sur ce
WIP, consignés au § revue de
[PLAN-COMPOSITION-HTML](PLAN-COMPOSITION-HTML.md) (le `await` du
prefetch qui bloquait le lancement, le verrou tenu pendant l'I/O
réseau, `boites.first()` sensible à la casse, erreurs avalées, premier
cycle complet non garanti, `backfill_status` sorti de `hors_pompe` sans
exemption ADR 0019).

**Dernière version livrée : 0.1.11** (publiée 2026-08-19, `6977778`,
auto-update **0.1.10 → 0.1.11 confirmé au terrain le 2026-08-19** — mise à
jour depuis l'app, la chaîne signée ADR 0013 reste prouvée vivante ;
release **vérifiée a posteriori** : Latest, 3 assets, `latest.json` sans
BOM, URL au tag nu, signature == `.sig`). La 0.1.11 porte les **trois
retours** de PLAN-RETOURS-4 (R1-R3, un **CORRECTIF** — aucune capacité
nouvelle, STANDARD.md §2.9) : téléchargement d'une pièce par dialogue
« Enregistrer sous » ; nom + poids d'une pièce dans une seule puce ; corps
des messages toujours sur dalle claire (thèmes sombres redevenus lisibles).

**La 0.1.10** (2026-08-18, `a25c566`, auto-update 0.1.9 → 0.1.10 confirmé)
portait les quatre retours de PLAN-RETOURS-3 (% de rattrapage ; spam /
non-spam ; supprimer un brouillon ; réponse par message). **La publication
est pilotée de bout en bout par `scripts/faire-release.ps1`** : bump de
tauri.conf.json + build signé (clé `C:\Keys\wind.key`, mot de passe à la
main) + manifeste + — après confirmation `OUI` — commit de release, push
(gate rejouée), tag nu + Release GitHub marquée Latest, notes tirées du
CHANGELOG. Fait prouvé : `TAURI_SIGNING_PRIVATE_KEY` accepte le **chemin**
du fichier de clé (pas seulement son contenu) ; la publication n'est donc
**plus manuelle** (l'ADR 0013 la décrivait ainsi ; le script la fait,
derrière une confirmation).

**La 0.1.9** (2026-08-17) portait les quatre retours de PLAN-RETOURS-2
(Cc/Cci ; cadence de synchro Gmail 5→30 min ; trait de chargement ; retrait
« Rendre indépendante »). **La 0.1.8** (2026-08-16) portait les quatre
correctifs courrier de PLAN-RETOURS-MAIL.

**La 0.1.7** (2026-08-16) reste la ligne de la refonte entière — le
Système v2 « Wada » et son élargissement (28 thèmes, PLAN-WADA /
PLAN-WADA-ELARGI), l'UI v3 et les retours CE (A44-A47, PLAN-UI-V3 /
PLAN-RETOURS-V3), les trois modes d'affichage (PLAN-VOLETS), l'interface
v1 retirée (PLAN-RETRAIT-V1), sur une fenêtre qui ne gèle plus
(PLAN-GELS, ADR 0019) ; auto-update 0.1.6 → 0.1.7 confirmé au terrain.
Tous les plans de cette ligne sont soldés.

**Dernier chantier soldé : PLAN-COMPOSITION-HTML** (2026-08-20,
`537a1e4`, A62-A63 + ADR 0022, terrain complet, CI verte — **à publier
en 0.2.0**). Le composeur passe au **corps riche de bout en bout** :
colonne `body_html` à côté du texte (drafts + outbox, migration
rembobinable, NULL sur l'existant), envoi `multipart/alternative`
(texte dérivé du même HTML par LA frontière unique `frontiere_corps`),
reflet Brouillons et tirage pareil (un brouillon riche re-rapatrié
garde sa mise en forme), écho Envoyés en HTML, citation `blockquote`,
éditeur `contenteditable` + `execCommand` legacy (sortie = allowlist
ammonia exacte), barre R4 stricte (D1-D3 : sans Lien/Citation, familles
génériques + 4 crans, nuancier 12 teintes), icônes 46 → 58. **Images
distantes par geste (D5 terrain)** : réponse au pixel neutre (une
citation `AllowRemote` chargeait les pixels espions au clic Répondre —
revue), transfert conserve les images. **Deux constats terrain corrigés
le jour même (A63)** : reconnexion d'un compte au jeton mort
(`invalid_grant`) depuis Réglages > Comptes (`reconnect_account`, garde
d'identité, e2e dédié) ; l'avis de déconnexion mène aux Réglages.
Revue à regard neuf : 10 trouvailles confirmées, corrigées (dont trois
pièges du contenteditable gravés au STANDARD §9). Reports : DETTE D-25.
e2e : 92 → **94**.

**Chantier soldé précédent : PLAN-DOCUMENTATION** (2026-08-19, `78a2a91`
→ `8cf8ac3`, CI verte, terrain E4 : reprise à froid et test du stub
propres). La documentation est restructurée en trois gestes kaizen :
**la méthode vit dans [STANDARD.md](STANDARD.md)** (numérotation
§2-§10 figée, s'amende par kaizen, ne se réécrit pas), **l'état dans
ce document** (réécrit à chaque chantier), les 24 plans soldés et
5 revues de phase dans [archives/](archives/), le normatif orphelin
rapatrié au dépôt (vérification de release → STANDARD §2.10, piège du
cache chaud → §9) ; les mémoires Claude ne portent plus que des faits
machine et des pointeurs. Stub PASSATION.md temporaire (D-24 : une
reprise propre sur les deux requises comptée).

**Chantier soldé précédent : PLAN-RETOURS-4** (2026-08-18, `52aec3e`, A59-A61,
terrain complet, CI verte — **R1-R3 livrés en 0.1.11** (`6977778`, auto-update
confirmé le 2026-08-19) ; **R4 reporté en chantier dédié**, décision CE D1).
Trois retours, tous **corrections / ajustements de l'existant** (aucune
capacité nouvelle → **CORRECTIF**, STANDARD.md §2.9). (1) **Téléchargement d'une pièce
par dialogue** : le clic enregistrait en SILENCE dans Téléchargements (« il ne
se passe rien ») ; il ouvre désormais « Enregistrer sous » natif — dossier ET
nom au choix, défaut Téléchargements + nom assaini ; nouvelle commande
`chemin_enregistrement_suggere` (le nom vient de l'UI ; `safe_file_name` reste
l'autorité de désinfection), `save_attachment(dest)` écrit au chemin choisi,
capability `dialog:allow-save`, couture e2e `__e2eDestination` (A59). (2) **Nom
+ poids d'une pièce dans la MÊME puce** : la lecture s'aligne sur le composeur,
exception assumée à « 1 puce = 1 information » ; glyphe `storage` retiré de
l'usage, conservé réservé au sous-ensemble (précédent A53, A60). (3) **Corps
toujours sur dalle claire** : mesuré au terrain que seul le texte à COULEURS
d'expéditeur (infolettres pensées pour fond blanc) était noir sur sombre — le
texte sans couleur propre était déjà lisible ; le corps bake désormais
`mail_render::Palette::default` (fond blanc) quel que soit le thème, comme les
clients mûrs, **renversant la dalle sombre d'A42** ; le front ne transmet plus
de palette (`paletteLecture` retirée, params `palette` de
`message_body`/`echo_body` retirés — A61). Report : DETTE D-23. **Piège gravé
(A61)** : ne JAMAIS re-transmettre une palette de thème au corps d'un message —
le corps est volontairement clair partout (le texte d'expéditeur est composé
pour fond blanc) ; garde e2e « le corps reste sur dalle claire même sous un
thème sombre ».

**Chantier soldé précédent : PLAN-RETOURS-3** (2026-08-18, `8819090`, A55-A58,
terrain complet, CI verte — **livré en 0.1.10**, auto-update confirmé au
terrain le 2026-08-18). Quatre retours terrain. (1) **Pourcentage de
rattrapage** : la barre d'état passe à « N restants · P % » ; `P` = corps
présents / corpus en portée, fonction pure `backfill_percent` (sœur de
`sync_percent`, plafonnée à 99 tant qu'un corps manque), dénominateur
`bodies_total_count` ; le % vit dans le TEXTE (A55). (2) **Spam / non-spam** :
`report_spam`/`mark_not_spam` réutilisent `MoveTo` — le dossier indésirable
est résolu par compte (`canonical_folders`), c'est le fournisseur qui apprend ;
geste par fil, indisponible si pas de dossier Junk (A56). (3) **Supprimer un
brouillon** depuis la composition — geste destructif avec confirmation inline,
distinct d'« Annuler » qui conserve (A57). (4) **Réponse par message** :
Répondre/Répondre-tous/Transférer passent en bas de chaque message ; la barre
du fil ne garde que le tri + spam. **Constat terrain corrigé le jour même** :
les 3 gestes sur nos PROPRES messages aussi — répondre y vise les
destinataires d'origine (À pour Répondre, À+Cc pour Répondre-tous ; fonction
pure `reply_to`), jamais soi-même (A58). Reports : **D-21** (double COUNT du
rattrapage, famille D-8, budget tenu au terrain), **D-22** (report_spam
déjà-spam via recherche). Piège confirmé : la trace terrain (`… 2> fichier`)
échoue si le chemin n'existe pas — le « Bureau » est redirigé sous OneDrive
(`C:\Users\<u>\Desktop` absent) ; écrire à la racine du dépôt.

**Chantier soldé précédent : PLAN-RETOURS-2** (2026-08-17, `dfa6224`, A52-A54
+ ADR 0021, terrain complet, **livré en 0.1.9**). (1) **Synchro Gmail « trop
longue »** : mesurée au terrain (trace `run_sync`, ~135 s en release quand
22 vues Gmail ont bougé — ~5 s par dossier changé, bridage probable). La
sobriété (ADR 0017) tient ; c'était la **cadence** qui coûtait. Le
veilleur IDLE (ADR 0018) tenant INBOX en temps réel, le **cycle complet
passe de 5 à 30 min** (+ passe légère INBOX à 5 min en filet) — S-D4
tranché, **ADR 0021**. All Mail RESTE synchronisé (Archives intacte, ADR
0010 préservé) ; l'exclusion des vues virtuelles est reportée (STANDARD.md §2.6). (2)
**Trait de chargement** : le mode « au pourcentage » (figé chez Chromium,
A40) meurt ; le trait fait sa boucle complète dès qu'une action tourne, le
% reste dans le TEXTE (A52). (3) **« Rendre indépendante » retirée** —
placeholder inerte, multi-fenêtre reporté en chantier dédié (A53). (4)
**Cc/Cci fonctionnels** — tranche compose→Draft→outbox→SMTP→UI ; **Cci
dans l'enveloppe SMTP SEULE** (`send_raw`, jamais un en-tête Bcc servi),
« Répondre à tous » remet les Cc d'origine en Cc (`reply_all_split`) ;
brouillons locaux portent cc/bcc (A54). Piège payé : l'app **release** est
sous-système *windows* → `eprintln` **muet** en console (mesurer en
débogage ou `2> fichier`).

**Chantier soldé précédent : PLAN-RETOURS-MAIL** (2026-08-16, `19ea16a`,
A48, terrain complet, livré en 0.1.8). Quatre retours du CE sur le
courrier réel : objets/noms débarrassés des escapes `quoted-string`
d'IMAP que `imap-proto` laisse (correctif + migration de l'existant),
dossier « Envoyés » qui dit enfin le vrai destinataire, « Répondre à
tous » instantané, et le `<head><title>` de certaines infolettres qui ne
fuit plus en tête de corps. **Fait d'état à retenir : l'enveloppe stockée
porte désormais les destinataires** (`envelopes.to_addrs`/`cc_addrs`,
tirés de la même ENVELOPE que l'expéditeur) — le « l'enveloppe ne porte
que l'expéditeur » d'avant est renversé ; `reply_all_context` les lit
d'abord (hors ligne), la relève serveur n'est qu'un repli. Reports :
DETTE D-15/D-16.

### L'état du terrain — chiffres du 2026-07-26, boîte réelle

La synchronisation intégrale (ADR 0010) a tout ramené : **256 312
messages** (7 539 avant), 4 comptes, tous dossiers — spam et corbeille
compris, décision explicite du Chef Ingénieur.

**La passe d'en-têtes a convergé à zéro** : `diagnostic_fils` affiche
`jamais lus : 0` dans la portée du regroupement. Ce chiffre est final —
plus rien n'est en train de bouger côté fils. Résultat du regroupement :

| | avant ADR 0009 | avant ADR 0010 | **final** |
|---|---|---|---|
| fils de 2 à 5 | 15 (tous confondus) | 242 | **577** |
| fils de 6 à 20 | — | 6 | **35** |
| fils de plus de 20 | — | 0 | **1** |

**La portée tient à l'échelle** : 248 771 messages hors portée n'ont créé
aucun fil et n'ont fait remonter aucune conversation — c'est l'invariant
STANDARD.md §6.9, tenu par test.

**Ce qui bouge encore : le rattrapage des corps.** ~250 000 messages
attendent leur corps, à 200 par lot, au fil de l'usage — une longue
traîne de plusieurs jours ou semaines, reprenable, visible dans le
bandeau ocre de l'application. **La base grandira vers ~13 Go**
(256 312 × ~50 ko) ; le budget « < 1 Go » est levé (ADR 0010 §2) et la
garde d'espace disque veille avant chaque engagement.

**Premier réflexe d'une nouvelle session :** demander à l'utilisateur où
en est le bandeau de rattrapage et ce que pèse
`%APPDATA%\dev.elements.wind\wind.db` (avec ses compagnons `-wal` et
`-shm`). Rappel du STANDARD.md §7.1 : tu ne peux pas lire sa base toi-même.
(Avant PLAN-WIND E3 : `dev.discovery.app\discovery.db` — le déménagement
est automatique au premier lancement Wind.)

### Les budgets non tenus, avec leur remède

| Poste | Mesure (2026-07-26) | Levier |
|---|---|---|
| Adoption d'une base héritée | 3,66 s à 200 000 messages, une seule fois | **réglé en forme par l'ADR 0012** : visible, annulable, rembobinable — la durée est assumée, la passe est unique |
| Recherche | ~~113–210 ms~~ → **~66 ms ✅** (2026-08-17) | **réglé** : `prefix='2 3'` + destinataires indexés + **soupape tri-date armée** au-delà de 10 k corr. (`WIDE_QUERY_THRESHOLD`) ; mesuré sur la VRAIE base (251 k / 7 Go), pire cas préfixe 3 car. (36 k corr.) à ~66 ms (PLAN-RECHERCHE, A50) |

Le budget recherche est **tenu au terrain**. Enseignement du terrain : le
mur n'est pas le plafond de rendu (l'hydratation ne coûte que ~0,2 ms/ligne)
mais le **plancher BM25** — classer 36 k correspondances d'un préfixe 3 car.
prend ~80 ms, quel que soit le plafond, et ce plancher monte avec le corpus.
La soupape tri-date de l'ADR 0004 le résout : au-delà de 10 k correspondances,
`search_capped` classe par date (meilleur ordre pour une requête aussi large,
de toute façon), ~66 ms. Reste indexé désormais : destinataires (`to:`/`à:`),
le trou de pertinence le plus courant.

### Arbitrages — tranchés et ouverts

**Tranchés** (ne pas rouvrir sans mesure) :
- ~~Synchroniser l'archive ?~~ → **Tout est synchronisé** (ADR 0010),
  spam et corbeille compris, sans quota. La question est soldée.
- ~~Périmètre de la Phase 5 ?~~ → La migration visible et interruptible
  d'abord — **faite** (ADR 0012). Suivent, dans l'ordre : installeur,
  télémétrie, bêta.

**Ouverts** (au Chef Ingénieur) :
- **Recherche sans limite pratique** (2026-08-17) — le plafond lui-même est
  soldé (`SEARCH_LIMIT = 100`, barre « N sur M » avec le vrai total ;
  A50/PLAN-RECHERCHE). Reste ouvert le seul vrai « tout voir » : liste de
  résultats **virtualisée + pagination par curseur** (le mur : hydratation
  `SELECT_UNIFIED` par ligne + liste non fenêtrée). Chantier à part.
- **Tri par date de la recherche** — **armé** au terrain (2026-08-17) : le
  plancher BM25 d'un préfixe 3 car. très large (36 k corr.) dépasse le budget
  quel que soit le plafond. `search_capped` bascule sur la date au-delà de
  `WIDE_QUERY_THRESHOLD` (10 k corr.) ; en deçà, BM25. Seuil calé sur cette
  machine — à re-mesurer si le budget se tend en bêta.
- **Doublons multi-boîtes dans la recherche** — observé au terrain : le
  même message vit copié dans plusieurs boîtes (« 19 messages partagent
  un Message-ID »), et la recherche renverra chaque copie. Dédoublonner à
  l'affichage ? À observer en usage réel avant de décider (D2, gardé ouvert).

### Ensuite — la Phase 5

Durcissement et bêta ([PLAN.md](PLAN.md) §4). Ordre arbitré : migration
visible et interruptible **✓ faite (ADR 0012)** → installeur + mise à
jour signée **✓ faite (ADR 0013)** → télémétrie de crash opt-in **✓
faite (ADR 0014)** → **bêta fermée 20-50 utilisateurs (prochain)**.
Gate 5 : deux semaines sans défaut critique.


---

## Ce qui reste

### Le chantier fait : migration visible et interruptible (ADR 0012)

Terminé et **validé au terrain** le 2026-07-26, sur copies. L'adoption
est une unité transactionnelle unique (du DROP conditionnel des tables
de fils jusqu'à `user_version`) : annuler rembobine tout, la passe se
rejoue entière au prochain lancement. Écran modal au démarrage — chaque
commande ouvre sa propre connexion, sans porte la première venue
paierait la passe en silence. Preuves : test de rembobinage sur une
vraie base de fichier, banc (3,66 s, pas de régression), annulation
exercée en pleine passe à l'échelle du gate 3.

### Le chantier fait : installeur NSIS + mise à jour signée (ADR 0013)

Terminé et **validé au terrain** le 2026-07-26 : la boucle 0.1.1 → 0.1.2
s'applique sur l'app installée, base intacte. **Re-validé le 2026-08-16 à
l'échelle de la refonte** : l'auto-update 0.1.6 → 0.1.7 (la refonte
entière) s'applique sur l'app installée, chaîne signée prouvée vivante.
NSIS (**pas MSIX** — il
virtualiserait `%APPDATA%` et orphelinerait la base) ; updater Tauri
signé minisign, piloté depuis Rust (capabilities au minimum) ; signature
de code Windows reportée à la bêta. Publication d'une version :
`scripts/faire-release.ps1 <version>` prépare le `latest.json`, la
Release GitHub reste manuelle (tag = version nue).

### Le chantier fait : télémétrie de crash locale et opt-in (ADR 0014)

Terminé et **validé au terrain** le 2026-07-26. Fichier local seul
(aucun réseau, aucun tiers), panics backend seuls, opt-in off par
défaut ; le **message du panic est supprimé** (seul vecteur de donnée
personnelle), prouvé à deux niveaux (mémoire et fichier écrit). Le hook
ne touche jamais la base (consentement en fichier + `AtomicBool`).
Trouvaille terrain corrigée : un crash sur le thread principal produit un
**double panic** à la frontière FFI de WebView2 — compteur `SEQ` (noms
uniques) + filtre du secondaire `cannot unwind`.

### Le chantier fait : plus aucune commande sur le thread principal (ADR 0019)

Terminé et **validé au terrain** le 2026-08-15 (PLAN-GELS, `e32280b`,
A39/A40). Le freeze du démarrage (25,2 s de gels cumulés sur 40 s,
mesurés) est mort à la racine : toute commande bloquante passe par
`hors_pompe()` — spawn_blocking + verrou global, la sérialisation
d'avant conservée — tenu par la gate `garde-thread-principal.mjs` et le
budget « aucun gel de pompe > 150 ms » (`sonde-gel.py`). Au passage, le
terrain a livré et fait corriger le jour même : l'avancement figé à
99 % par les départs en attente de rejeu (le dénominateur s'ajuste), et
la boucle du trait hitofude morte-née (animation CSS dans un `<mask>`
non rendu → SMIL). Dette ouverte : D-8 (sondes chères, hors pompe).

### Le chantier suivant : bêta fermée 20-50 utilisateurs

Dernière étape avant le gate 5 ([PLAN.md](PLAN.md) §4). Kaizen
hebdomadaire sur les frictions **observées**. Rien n'est engagé.

### La longue traîne en cours

Le rattrapage intégral des corps (~250 000 messages restants) avance à
200 par lot au fil de l'usage. Rien à coder ; surveiller le disque et le
bandeau. La recherche gagne en profondeur à mesure.

### Reports assumés

- **Requêtes chères des sondes périodiques** (PLAN-GELS D4) : hors de
  la pompe elles ne gèlent plus rien, mais leur coût CPU reste réel —
  registre **D-8** de [DETTE.md](DETTE.md), chiffres et pistes dedans.
- **Doublons multi-boîtes dans la recherche** (nouveau, ADR 0010) : le
  même message copié dans plusieurs boîtes remonte plusieurs fois dans
  les résultats. À observer en bêta avant de décider d'un dédoublonnage.
- **Tri par date de la recherche** — **armé** (A50, PLAN-RECHERCHE) : au-delà
  de 10 k correspondances (`WIDE_QUERY_THRESHOLD`), le classement bascule sur
  la date, le plancher BM25 dépassant sinon le budget. Plus un report.
- **Défilement profond de la LISTE** : `OFFSET` coûte ~230 ms à 150 000
  conversations. La recherche a résolu le même mur (l'`OFFSET` **hydrate les
  lignes sautées**) par une **pagination en deux temps** — clés ordonnées
  puis hydratation de la seule page (A51, PLAN-CHARGER-PLUS) ; le même patron
  s'appliquerait à la liste.
- **Envoi de pièces jointes** (lecture seule en v1) ; **filtre « a une
  pièce jointe »**. (Le **`to:` dans la recherche** est LIVRÉ — A49.)
- **CONDSTORE réel, IDLE/push** — reports de Phase 1 inchangés.
- **Dossier CASA Google** — chemin critique du lancement public, côté
  produit-owner.

### Dette connue, non corrigée

`apps/desktop/ui/style.css` : la règle d'élément `header { display: flex }`
s'applique aussi à `#detail-header`. Tout enfant pleine largeur ajouté là
devient un item flex écrasé à 0 px. (Le bandeau d'avancement de
l'ADR 0010 et l'écran de migration de l'ADR 0012 ont été placés **hors**
de tout `<header>` pour cette raison.)

Cousin de cette dette, désormais **tenu par une règle** : toute règle
d'ID qui pose un `display` écrase le `[hidden]` du navigateur et exige
son garde-fou `#id[hidden] { display: none }`. Huit occurrences à ce
jour ; la dernière (`#detail`) laissait l'iframe sandboxée capter le
premier clic et tuer les raccourcis clavier (STANDARD.md §9). Un E2E tient le cas.

### La Phase 5

Installeur MSIX/NSIS + mise à jour signée, télémétrie de crash opt-in,
bêta fermée 20-50 utilisateurs, kaizen hebdomadaire sur les frictions
**observées**. Gate 5 : deux semaines sans défaut critique.

