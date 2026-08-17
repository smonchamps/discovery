# PLAN-RETOURS-2 — synchro Gmail, trait de chargement, composition (indépendante, Cc/Cci)

> **CHANTIER SOLDÉ le 2026-08-17 — terrain complet.** Commit unique
> `dfa6224` (feat), CI verte (run 32063007854). GO CE du plan le
> 2026-08-17 (D1-D4), décisions #1 le même jour (D5-D8). Terrain validé
> par le CE sur ses vrais comptes : #2 (animation vue sur un envoi), #3,
> #4 (Cc/Cci, Cci non fuité) ; #1 confirmé (le courrier arrive tout de
> suite, IDLE tient à 30 min). Journal Système A52-A54, ADR 0021 (S-D4
> tranché). Reports : DETTE D-19 (Cc/Cci cross-appareil), D-20 (coût par
> cycle Gmail / vues virtuelles). Pas de publication : vit sur `main`.
>
> Deuxième salve de retours terrain du Chef Ingénieur (2026-08-17), sur
> la 0.1.8. Quatre retours **hétérogènes** en taille et en nature : une
> régression de performance à mesurer (§1), un défaut d'affichage à
> simplifier (§2), et deux placeholders du prototype jamais câblés à
> transformer en fonctionnalités (§3, §4). Le plan les porte ensemble
> mais les tranche séparément — le § Décisions CE arbitre le périmètre
> réel avant tout code.

## Constat (les faits, ce qui est prouvé au code)

### #1 — La synchronisation Gmail est trop longue (smonchamps@gmail.com)

> « on dirait que chaque dossier est parcouru individuellement à chaque synchro »

C'est le symptôme **exact** que l'**ADR 0017 (« cycle sobre »)** a déjà
traité : chaque dossier payait `SELECT` + `UID SEARCH ALL` à chaque
cycle, même immobile — **~38 min mesurées sur ce compte Gmail** le
2026-08-13, ramenées sous **60 s** par la relève gardée
(`faut_relever`, [sync.rs:391](../crates/mail-core/src/sync.rs#L391)).

Le cycle complet ([commands.rs `run_sync`](../apps/desktop/src/commands.rs#L1077))
applique aujourd'hui :

- **LIST-STATUS** en un aller-retour (`folders_with_status`), Gmail
  l'annonce ([lib.rs:524](../crates/mail-imap/src/lib.rs#L524)) — l'inventaire
  n'est plus censé être un goulot ;
- **`doit_relever`** ([commands.rs:497](../apps/desktop/src/commands.rs#L497)) :
  saute un dossier dont ni UIDNEXT, ni MESSAGES, ni HIGHESTMODSEQ n'ont
  bougé ; le modseq est persité au SELECT (`update_state`,
  [store.rs:1029](../crates/mail-core/src/store.rs#L1029)) ;
- la barre d'état ne nomme (`poser_boite`) que les dossiers **réellement
  relevés** — un dossier sauté ne s'affiche pas.

**Donc** : si l'utilisateur voit défiler les dossiers un à un, soit la
sobriété est redevenue inefficace sur ce compte (`n_sautés ≈ 0`), soit
Gmail expose beaucoup de dossiers qui **bougent réellement à chaque
cycle** — « [Gmail]/Tous les messages » (All Mail, qui contient TOUT le
courrier : son UIDNEXT/MODSEQ glisse au moindre mail), « Important », les
onglets de catégorie. **Je ne peux pas trancher sans mesure** (§7.1 : je
ne lis pas la vraie base). La trace par phase existe déjà
([commands.rs:1373](../apps/desktop/src/commands.rs#L1373)) et EST
l'instrument qui a servi à décider l'ADR 0017.

### #2 — Bug d'affichage du trait en mode chargement

Le trait hitofude a trois états ([App.svelte:204](../apps/desktop/ui-v2/src/App.svelte#L204),
[Hitofude.svelte](../apps/desktop/ui-v2/src/Hitofude.svelte)) : statique
(repos « À jour »), `anime` (boucle 4 s), et **`progression` au
pourcentage** (`mode:'plein'`). Le défaut vit dans ce dernier :
le tracé est masqué à une longueur partielle par `stroke-dashoffset`,
mais la transition CSS **ne tourne pas dans le `<mask>` chez Chromium**
(limite connue, A40, commentée dans le composant) — le trait **saute à
une longueur partielle et y reste figé, sans fondu** (`.fondu` n'est
appliqué qu'en mode `anime`). Le plus visible pendant l'intégrale et les
rattrapages (les seuls porteurs de pourcentage). Le retour demande de
**supprimer ce mode** et de ne garder qu'**une animation de boucle
complète dès qu'une action tourne**, quelle qu'elle soit.

### #3 — Le bouton « Rendre indépendante » ne fonctionne pas

C'est un `<span class="puce">` **inerte**, sans handler
([Composition.svelte:567](../apps/desktop/ui-v2/src/Composition.svelte#L567)) —
explicitement documenté « inerte comme au prototype » en tête du fichier
(lignes 22-25). Ce n'est **pas une régression** : la fonction n'a jamais
existé. Le faire fonctionner = **une vraie fenêtre de composition
détachée** (Tauri multi-fenêtre), qui n'existe nulle part dans le
produit aujourd'hui (Wind est mono-fenêtre). C'est le retour le plus
lourd, et de loin.

### #4 — Les boutons Cc et Cci ne fonctionnent pas

Mêmes `<span>` inertes
([Composition.svelte:596-597](../apps/desktop/ui-v2/src/Composition.svelte#L596)),
même statut « inerte comme au prototype ». Le back-end d'envoi ne connaît
que `to` : `mail_core::compose(from, to, subject, body, in_reply_to)`
([compose.rs:36](../crates/mail-core/src/compose.rs#L36)), le `Draft`
n'a ni `cc` ni `bcc`, l'outbox stocke un seul champ `recipients`
([outbox.rs](../crates/mail-core/src/outbox.rs#L99)), et le SMTP
([mail-smtp/lib.rs:123](../crates/mail-smtp/src/lib.rs#L123)) n'ajoute que
`.to()`. Le câbler est une **tranche verticale** : compose → Draft →
outbox (schéma) → SMTP → UI, sous les règles d'or (jamais de fantôme) et
la garde d'injection d'en-têtes déjà en place
([compose.rs:262](../crates/mail-core/src/compose.rs#L262)).

## Périmètre

**Dans** (sous réserve des Décisions CE) : #2 (trait), #4 (Cc/Cci),
et #1 **après mesure**. **#3** est présenté comme décision de périmètre
(sortir en chantier dédié, ou retirer le bouton) — pas implémenté dans
cette salve sauf arbitrage contraire.

**Refus de périmètre explicites (§2.6)** :

- **Éditeur riche (gras/italique/listes/liens/citation)** : les autres
  puces inertes de la barre de format restent inertes — hors sujet des
  quatre retours.
- **Dédoublonnage multi-boîtes de la recherche** (report ADR 0010) : non
  rouvert ici, même si #1 le frôle (All Mail).
- **CONDSTORE — reflet des drapeaux** (E2b/PLAN-SYNCHRO) : #1 ne rouvre
  pas le reflet des drapeaux ; il ne touche que la sobriété/coût du cycle.
- **Câblage production du veilleur IDLE** (ADR 0018, sous gate spike) :
  hors périmètre — #1 vise le cycle complet, pas le temps réel.

## §1 — Synchro Gmail : mesuré, puis cadence (ADR 0021)

**Mesure terrain (2026-08-17, compte Gmail réel, trace `run_sync`, débogage) :**

```
INBOX 3,4s · inventaire 16,4s · 52 dossiers (46 sautés) 31,2s · fils 7,8s · brouillons 8,9s
```

(Compte n°2, 14 dossiers : 8,4 s au total — le coût est propre à Gmail.)

**Re-mesuré en release** (`2> sync-trace.txt`, l'app release étant muette
en console) : `INBOX 5,0s · inventaire 12,6s · 52 dossiers (30 sautés)
109,8s · fils 0,0s · brouillons 7,6s` ≈ **135 s** — ce cycle-là avait 22
dossiers changés.

**Verdict :** la sobriété (ADR 0017) **tient** (la plupart des dossiers
sautés). Le coût est **~5 s par dossier CHANGÉ** (réseau/bridage Gmail) +
le STATUS des 52 dossiers à l'inventaire. Un cycle Gmail **oscille de ~8 s
à ~135 s** selon le nombre de vues qui ont bougé. À 135 s **toutes les
5 min**, l'app synchronisait **~45 % du temps** — c'est la **fréquence**
qui fait le « trop long », pas le parcours.

Fait décisif : le **veilleur IDLE (ADR 0018) tourne en production** —
INBOX est déjà en temps réel. Le cycle complet de 5 min (hérité d'avant
IDLE) n'a plus à courir si souvent. C'est S-D4 (ADR 0018 §7), resté
ouvert.

**Correctif (ADR 0021), cadence seule :**
- **E1.1** — `App.svelte` : cycle complet **5 → 30 min** ; **passe légère
  INBOX à 5 min** en filet (contre un veilleur IDLE tombé sans reconnexion).
  La passe légère se sabre pendant un cycle (`enSynchro`).
- **All Mail reste synchronisé** — l'exclure aurait cassé la vue Archives
  et fait disparaître le mail archivé ailleurs (ADR 0010 préservé).
- **Report §2.6** : l'exclusion des vues Important/Suivis, marginale après
  la cadence et coûteuse en surface (champ neuf au type `Folder`), est
  écartée (ADR 0021, « Écartée »).
- **RED** : la cadence est une paire de constantes de minuterie dans
  `onMount` — aucune logique pure à éprouver, un RED n'apprendrait rien
  (dit, pas simulé). La preuve est au **terrain** (re-mesure release).
- **Gate** : `/gate` (build ui-v2, e2e — pas de changement Système, la
  cadence n'est pas du dessin, donc pas de DC-D2).

## §2 — Trait de chargement : une seule animation de boucle

- **E2.1** — `App.svelte`, dérivation `ligne` : quand une action tourne
  (cycle, intégrale, rattrapage corps/aperçus, envois en attente), le
  trait est **toujours** `{ mode: 'vague' }` (boucle). Abandon total du
  `mode:'plein'` au pourcentage. Repos « À jour » : trait statique
  inchangé. *(RED : un test de la fonction pure de décision de ligne, si
  extractible ; sinon vérification e2e/preview — la logique est du JSX
  dérivé, RED unitaire pauvre, je le dirai.)*
- **E2.2** — `Hitofude.svelte` : nettoyer le composant à deux états
  (statique / `anime`), retirer la prop `progression` devenue morte.
  Régler l'animation sur la description du retour : **fondu-in +
  remplissage G→D simultanés**, brève tenue pleine, **fondu-out**, en
  boucle — resynchroniser `hitofudeFade` (systeme.css) avec le tracé
  SMIL (aujourd'hui le fondu-in finit en 0,4 s alors que le remplissage
  dure 2 s). `prefers-reduced-motion` : trait plein immobile (règle A8,
  inchangée).
- **Gate E2** : `/gate` complète (dont `coherence-systeme` — le trait est
  normé au Système, DC-D2 : amender `systeme.dc.html`, journal A-n).

## §3 — « Rendre indépendante » : décision de périmètre (D2)

Trois voies, à trancher :

- **(a) Retirer le bouton** — honnête (§2.6), aligne l'UI sur ce que le
  produit fait. Coût : minime (retrait UI + Système). **Recommandé pour
  cette salve.**
- **(b) Chantier dédié multi-fenêtre** — vraie fenêtre Tauri de
  composition, état partagé par le brouillon (qui persiste déjà en
  base). Feature réelle, point dur (cycle de vie fenêtre, focus, autosave
  inter-fenêtres) : mérite son propre `/chantier` et peut-être un spike.
- **(c) Statu quo inerte** — rejeté : c'est le fantôme que §2.6 interdit.

## §4 — Cc / Cci : la tranche verticale

- **E4.1** — `mail-core::compose` : accepter `cc_raw`, `bcc_raw` ;
  valider chaque adresse (même frontière stricte, même garde
  d'injection). `Draft` gagne `cc: Vec<String>`, `bcc: Vec<String>`.
  RED : test « Cc validé, Cci validé, injection refusée dans Cc/Cci ».
- **E4.2** — `OutboxMessage` + schéma outbox : persister cc et bcc
  (migration additive, prouvée sur base de fichier — §6.7). RED :
  round-trip enqueue → to_send conserve cc **et** bcc.
- **E4.3** — `mail-smtp::build_message` : `.cc()` par adresse ; **Bcc
  jamais dans les en-têtes du message servi aux autres**, seulement dans
  l'enveloppe SMTP (règle du courrier). RED : le fil de sortie contient
  `Cc:` et **jamais** `Bcc:`, l'enveloppe porte quand même les Cci.
- **E4.4** — `queue_send` (commande) : params `cc`, `bcc` ; `save_draft`
  et la reprise de brouillon les persistent (autosave/conflit couverts).
- **E4.5** — `Composition.svelte` : `Cc`/`Cci` deviennent des bascules
  qui ouvrent des rangs de saisie (bind `cc`/`cci`), câblés à l'envoi et
  à l'autosave. DC-D2 : Système + journal A-n.
- **(D3)** — « Répondre à tous » remplit-il **Cc** (garde les Cc
  d'origine en Cc) au lieu de tout aplatir dans À ? Plus correct
  (`reply_all_recipients` sépare déjà to/cc en entrée). Recommandé.
- **Gate E4** : `/gate` complète + e2e d'envoi.

## Ordre proposé

1. **#1 mesure** (bloquant sur son propre design ; lancée en parallèle
   dès le STOP 1).
2. **#2 trait** (isolé, rapide, bon échauffement).
3. **#4 Cc/Cci** (tranche verticale, TDD).
4. **#3** selon D2 (retrait immédiat, ou renvoi en chantier dédié).
5. `/code-review high` sur le diff complet, puis `/gate`, puis STOP 2.

## § Décisions CE

- **D1 — Trait (#2), texte de statut.** Le trait passe à l'animation de
  boucle unique. Le **texte** de la barre garde-t-il le « N % »
  d'avancement de l'intégrale/rattrapage, ou passe-t-il à un libellé sans
  chiffre ?
- **D2 — « Rendre indépendante » (#3).** (a) retirer le bouton ; (b)
  chantier dédié multi-fenêtre ; (c) laisser inerte.
- **D3 — Cc/Cci (#4), « Répondre à tous ».** Remplir Cc avec les Cc
  d'origine (recommandé), ou garder l'aplatissement actuel dans À ?
- **D4 — Synchro (#1), All Mail.** Si la mesure montre que « Tous les
  messages » (et Important/catégories) dominent le coût : autorise-t-on
  à **exclure « [Gmail]/Tous les messages » du balayage récurrent**
  (risque : un message archivé présent NULLE PART ailleurs qu'en All Mail
  ne serait plus rapatrié lors du cycle) — ou on cherche un autre levier ?

### Réponses CE (STOP 1 — 2026-08-17)

- **D1** → **« Garder le "N %" »**. Le trait passe à l'animation de boucle
  unique ; le texte de la barre continue d'afficher l'avancement chiffré.
- **D2** → **« Retirer le bouton »**. « Rendre indépendante » est retiré
  de l'UI (et du Système) ; le multi-fenêtre pourra revenir en chantier
  dédié plus tard.
- **D3** → **« Les mettre en Cc »**. « Répondre à tous » remplit le champ
  Cc avec les Cc d'origine.
- **D4** → **« Décider après la mesure »**. Pas de pré-arbitrage sur All
  Mail : je reviens avec les chiffres et l'option précise (avec son
  risque) avant de trancher #1.

**GO** implémentation sur #2, #3 (retrait), #4. **#1 en attente de la
mesure terrain** (`cargo run -p wind-desktop --release`, trace par phase).

### Réponses CE — suite (#1, après mesure, 2026-08-17)

Terrain intermédiaire : #2 confirmé (animation exacte, vue sur un envoi),
**#3 OK, #4 OK pour tous**. La mesure du #1 est arrivée après un détour
(app release = sous-système *windows*, `eprintln` muet ; trace obtenue en
débogage). Décisions :

- **D5 — Livraison.** → **« Attendre le #1 »**. Les quatre retours sont
  livrés ensemble, en un seul lot, une fois le #1 conçu et implémenté.
- **D6 — All Mail.** → **« Le garder »**. Ne pas exclure « Tous les
  messages » : la vue Archives et l'intégrité du mail archivé priment
  (ADR 0010 préservé).
- **D7 — Cadence.** → **« 30 min »** pour le cycle complet (INBOX en temps
  réel par IDLE ; filet passe légère à 5 min).
- **D8 — Portée du #1.** → **« Cadence seule »**. L'exclusion des vues
  Important/Suivis est reportée (marginale après cadence, coûteuse en
  surface — ADR 0021). #1 = cadence + filet, point.

**GO** #1 (cadence). Reste : gate complète (App.svelte a changé), terrain
de re-mesure release, puis commit unique des quatre, push, CI, `/solde`.
