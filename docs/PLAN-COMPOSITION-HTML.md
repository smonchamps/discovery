# PLAN-COMPOSITION-HTML — composeur enrichi HTML (R4)

> Chantier ouvert le 2026-08-19 (`/chantier`), issu du report R4 de
> PLAN-RETOURS-4 (décision CE D1 du 2026-08-18). Release attendue :
> **0.2.0** — première capacité nouvelle du 0.x (MINEUR, STANDARD §2.9,
> déjà consigné à l'ETAT).
>
> **GO CE du plan (STOP 1) : 2026-08-19**, décisions D1-D3 consignées
> au § Décisions CE.

Demande du Chef Ingénieur (R4, 2026-08-18) : une vraie barre de mise en
forme dans le composeur — Police · Taille · Gras / Italique / Souligné /
Barré · Couleur du texte · Alignement (G/C/D) · Liste à puces · Liste
numérotée · Diminuer / Augmenter le retrait · Effacer la mise en forme.

---

## Constat — faits vérifiés dans le code (2026-08-19)

- **Le composeur est un `<textarea>` texte brut**, la barre de format est
  inerte : six `<span>` décoratifs G/I/S/Liste/Lien/Citation
  (`Composition.svelte`).
- **Le modèle est texte de bout en bout** : `Draft.body_text`
  (`compose.rs`), `OutboxMessage.body_text` (`outbox.rs`, colonne
  `outbox.body_text`), `DraftContent.body` / `SavedDraft.body` (colonne
  `drafts.body`). Citation en texte à préfixes `>` et séparateur `----`
  (`quote_reply` / `quote_forward`).
- **mail-smtp** : `SinglePart::plain`, ou `MultiPart::mixed(plain,
  pièces…)` ; le reflet Brouillons (`draft_bytes`) pareil. Aucun
  `multipart/alternative` nulle part.
- **La migration a son patron** : `add_missing_columns` (idempotent,
  `store.rs`), exécuté dans l'unité transactionnelle de l'adoption
  (ADR 0012) — un ajout de colonne NULLable est rembobinable par
  construction (ROLLBACK), les tests de migration sur base de fichier
  existent et servent de modèle (invariant #7).
- **La lecture est prête** : l'allowlist ammonia (`sanitize.rs`)
  conserve `font[color|face|size]`, `style` (filtré par `clean_style`),
  `align`, et les défauts ammonia portent `b/i/u/s/strike`, `ul/ol/li`,
  `blockquote`, `div`, `span`. C'est **exactement** le vocabulaire
  qu'émet `contenteditable`/`execCommand` en mode legacy
  (`styleWithCSS=false`). Rien à élargir côté sanitisation.
- **L'écho Envoyés** naît du journal en convertissant le texte :
  `texte_en_html(body_text)` (`echo.rs`) — il devra porter le HTML
  composé directement.
- **La dérivation HTML → texte existe déjà** : `mail_render::body_text`,
  utilisée par `forward_context`. `mail-core` ne dépend PAS de
  `mail-render` : la dérivation vit dans `commands.rs` (l'app dépend des
  deux) — une seule autorité de dérivation.
- **e2e** : le composeur est couvert par `refonte-parcours-portes` et
  `refonte-ecran02`, via `fill`/`toHaveValue` sur le `<textarea>` — à
  adapter au `contenteditable` (`innerText`/`toContainText`).
- **WebView2 = Chromium** : `contenteditable` + `document.execCommand`
  y sont pleinement fonctionnels (déprécié mais entretenu — c'est le
  moteur de Gmail même) ; la gate e2e le tient sous surveillance.
- **La barre exige 12 glyphes neufs** (format_bold, alignements,
  listes, retraits, format_clear…) : le sous-ensemble Material passe de
  46 à **58 glyphes** (23,1 Kio), régénéré par la procédure du README —
  cache-buster `?v=58`.

## Périmètre

**Dans ce chantier** — la tranche verticale complète, chaque couche TDD :
corps HTML porté par le modèle (brouillons + journal d'envoi, migration),
envoi `multipart/alternative` (+ `mixed` avec pièces), reflet Brouillons
pareil, citation HTML (`<blockquote>`), écho Envoyés en HTML, éditeur
`contenteditable` + barre demandée par R4, e2e adaptés et étendus,
Système amendé (DC-D2), terrain dédié, release 0.2.0.

**Refus de périmètre explicites (STANDARD §2.6) :**
- **Pas d'images inline** (collage ou insertion dans le corps) — hors
  demande ; les pièces jointes restent la voie.
- **Pas de tableaux** à la main, **pas de polices web distantes** (la
  CSP `default-src 'none'` de la lecture les bloquerait de toute façon).
- **Pas de signature HTML**, pas de bascule « envoyer en texte brut »
  par message : le repli `text/plain` est automatique et systématique
  (`multipart/alternative`), comme les clients mûrs.
- ~~Le tirage des brouillons distants reste texte~~ → **levé le
  2026-08-20 (revue)** : la revue a montré que ce refus détruisait la
  mise en forme de NOS brouillons riches re-rapatriés (UIDVALIDITY,
  édition webmail) — `import_remote_draft` porte désormais le HTML
  assaini par la même frontière.
- **Lien et Citation** : retirés (D1).

## Options et verdicts

### Moteur d'édition
- **(a) `contenteditable` + `execCommand` — recommandé et retenu.** La
  sortie legacy (`<b>`, `<i>`, `<font>`, `align`, listes, `blockquote`)
  est déjà, mot pour mot, l'allowlist ammonia : aucune couche de
  traduction, zéro dépendance, ~200 lignes d'UI. Déprécié mais stable
  dans Chromium (Gmail vit dessus) ; la gate e2e joue chaque bouton.
- (b) `contenteditable` + Selection/Range manuel : réécrire un moteur
  d'édition (fusion de runs, listes imbriquées, retraits) pour le même
  résultat — coût sans gain, ne bat pas (a).
- (c) Éditeur tiers (Quill, ProseMirror…) : dépendance lourde, modèle
  de document propre dont la sortie serait à re-mapper vers le
  vocabulaire ammonia — contraire à la sobriété du socle (ADR 0015,
  zéro dépendance UI). Ne bat pas (a) nettement : écarté.

Pas de spike : le point dur n'est ni algorithmique ni mesurable — c'est
le périmètre (décisions D1-D3) et la migration (patron éprouvé).

### Où vit le HTML
- `body_html` **en colonne NULLable à côté du texte** (drafts, outbox) —
  retenu. `body` / `body_text` restent peuplés (dérivés du HTML par
  `mail_render::body_text`) : aperçus du dossier Brouillons, repli
  `text/plain`, compat totale de l'existant. `NULL` = brouillon ou envoi
  d'avant la migration, chemin texte inchangé. À la reprise d'un vieux
  brouillon texte, conversion texte → HTML à l'ouverture de l'éditeur —
  mais un brouillon texte réouvert PUIS FERMÉ SANS FRAPPE repart tel
  quel (garde anti-churn : convertir sans frappe fabriquerait une
  modification et re-pousserait une copie vers Gmail à chaque ouverture).
- Remplacer `body` par du HTML (une seule colonne) : casserait aperçus
  et repli, migration lourde — écarté.

### Sûreté du HTML sortant
Le HTML passe par **LA frontière Rust unique** (`frontiere_corps` —
assainit, dérive le texte du repli, retombe en texte si le rendu est
vide), traversée par les TROIS écrivains : `save_draft`, `queue_send`,
et le tirage (`pull_drafts`). Les images distantes se décident **au
geste** (verdict terrain **D5**, 2026-08-20) : une **réponse** cite au
pixel neutre — en `AllowRemote`, cliquer « Répondre » chargeait les
pixels espions du message cité sans consentement (revue du 2026-08-20,
invariant §6.4) — ; un **transfert** conserve les images, le
destinataire reçoit le message entier (composer le transfert vaut
« afficher les images » implicite). La frontière elle-même est
`AllowRemote` : elle ne re-neutralise pas ce que l'amont a décidé,
l'assainissement étant idempotent.

## Faits notables de l'implémentation (2026-08-20)

- **E1-E5 livrées (code)**, suites vertes : mail-core 333, mail-smtp 22,
  ui-v2 build 0 avertissement, e2e `refonte-parcours-portes` 11/11 et
  `refonte-ecran02` 56/56 en isolation, clippy muet.
- **Deux défauts produit débusqués par la passe e2e**, corrigés le jour
  même :
  1. la **pré-mise au point volait le focus** — le `setTimeout` qui
     place le focus à l'ouverture du composeur pouvait arracher le
     focus à un champ où l'utilisateur tapait déjà (vu à l'e2e : le
     corps atterrissait dans le champ À). Garde : si le focus est déjà
     dans la carte, la pré-mise au point s'abstient ;
  2. le **routeur clavier ignorait l'éditeur riche** — `surTouche`
     (App.svelte) ne reconnaissait comme saisie que INPUT/TEXTAREA :
     taper « c », « e » ou Suppr dans le corps déclenchait les
     raccourcis globaux (Suppr supprimait la conversation sélectionnée
     pendant la frappe). Garde : `isContentEditable` rejoint la
     détection de saisie.
- **Piège e2e appris** : `fill('')` sur un contenteditable est un
  no-op Chromium (`insertText` vide ne supprime pas la sélection) — on
  vide comme l'utilisateur, Ctrl+A + Suppr. Et `fill(texte)` passe par
  l'élément FOCALISÉ (course possible avec tout focus programmatique),
  contrairement au fill atomique des input/textarea.
- Sous-ensemble d'icônes régénéré 46 → 58 (23,1 Kio), preuve apercu
  rejouée : PASS 59/59 ligatures ; cache-buster `?v=58`.
- Système amendé : maquette de la barre réelle + table d'icônes +
  journal **A62** (DC-D2, même commit).
- **Interférence perf-lecture : SOLDÉE (décision CE, 2026-08-20).** Le
  CE a jugé le WIP perf-lecture trop aléatoire : ses modifications non
  commitées (App.svelte, commands.rs `prefetch_bodies`/`run_prefetch`/
  `PREFETCH_PAR_COMPTE`/`backfill_status`, main.rs) ont été **retirées
  chirurgicalement** — le sujet repartira de zéro dans une autre
  session. Son PLAN-PERF-LECTURE.md a quitté le dépôt (copie archivée
  hors dépôt). Les six constats de la revue sur ce WIP restent au §
  revue ci-dessous, comme matière pour la reprise.

## Revue à regard neuf (2026-08-20) — verdicts

`/code-review high`, 8 angles indépendants. **10 trouvailles confirmées,
toutes corrigées** le jour même, suites re-jouées vertes. Les plus
notables : pixels espions chargés au clic Répondre (citation
`AllowRemote` + `innerHTML` document principal → politique `BlockRemote`
partout) ; frappe écrasée par la citation tardive ; churn Gmail du
brouillon riche réouvert (l'anti-churn ré-émet désormais les valeurs
stockées à l'octet près) ; `text/plain` vide (`<div><br></div>` passait
la frontière) ; tirage qui détruisait la mise en forme ;
`peutSupprimer`/placeholder/nuancier/défilement.

**Écarts assumés sans correctif** (consignés) : le miroir JS
`texteEnHtml` de `texte_en_html` (8 lignes, documenté — le supprimer
exigerait de servir la conversion par le Rust et re-créerait le churn
texte→riche) ; `draft_bytes` à 8 arguments positionnels (allow posé,
précédent `insert_draft`) ; pas de `Default` sur `DraftContent` (45
littéraux de test à retoucher au prochain champ) ; le triptyque de
vidage e2e recopié 3 fois.

**Hors périmètre — WIP perf-lecture, à remettre à son chantier** : le
`await prefetch_bodies` bloque la passe légère du lancement (compte
injoignable = INBOX gelée) ; le verrou `bodies_backfill` tenu pendant
toute l'I/O réseau ; `boites.first()` sensible à la casse pour désigner
la réception ; erreurs avalées (`let _`, `Err(_) => continue`) contre
STANDARD §9 ; premier cycle complet non garanti si `relever` est en vol
à T+20 s ; `backfill_status` sorti de `hors_pompe` sans exemption
consignée (ADR 0019).

## Étapes

### E1 — mail-core : le modèle porte le HTML (+ migration)
1. `Draft.body_html: Option<String>` (posé par l'appelant, `compose()`
   rend `None`), `OutboxMessage.body_html`, `DraftContent.body_html`,
   `SavedDraft.body_html`.
2. Colonnes `drafts.body_html TEXT` et `outbox.body_html TEXT` (schéma
   neuf + `add_missing_columns`) ; `enqueue_outbox`, `save_draft`,
   `DRAFT_SELECT`, `OUTBOX_SELECT` les portent ; le `WHERE` « contenu
   identique » de `save_draft` compare AUSSI `body_html` (une mise en
   forme seule re-pousse).
3. TDD : aller-retour `body_html` (brouillon et journal) ; migration sur
   base héritée DE FICHIER (colonnes ajoutées, lignes anciennes NULL,
   comportement texte intact — invariant #7).

### E2 — mail-core : citation HTML
1. `quote_reply_html(sender, date, html)` → attribution ÉCHAPPÉE +
   `<blockquote>` (filet gauche inline) ; `quote_forward_html(…)` →
   en-tête De/Date/Objet échappé + corps tel quel.
2. `texte_en_html` (déjà pub, `echo.rs`) est l'autorité d'échappement,
   ré-exportée au niveau crate. TDD : fonctions pures.

### E3 — mail-smtp : multipart/alternative
1. `build_message` : `body_html` présent → `MultiPart::alternative`
   (texte d'abord — RFC 2046, du plus simple au plus fidèle) ; avec
   pièces → `mixed(alternative, pièces…)`. Absent → chemin texte
   octet pour octet inchangé (tenu par les tests existants).
2. `draft_bytes` : même bascule pour le reflet Brouillons.
3. TDD sur `formatted()` : structure MIME, repli texte, HTML, pièces.

### E4 — commands.rs + écho : la tranche se soude
1. `queue_send` reçoit `bodyHtml` ; assainit (`corps_riche_assaini`),
   dérive le texte (`mail_render::body_text`), compose avec les deux.
2. `save_draft` pareil ; `DraftRow` sert `body_html` tel que stocké.
3. `reply_context` / `reply_all_context` / `forward_context` rendent
   AUSSI `body_html` (citation E2 sur le corps assaini) — `body` texte
   reste le repli.
4. L'écho Envoyés porte le HTML composé tel quel (TDD mail-core).
5. Le reflet Brouillons (`sync_drafts`) pousse le `body_html`.

### E5 — UI : l'éditeur et la barre
1. `<textarea>` → `<div contenteditable>` ; le DOM est la vérité
   (`poserCorps`/`chargeCorps`), `innerText` pour `vide()`.
2. Barre réelle (D1-D3) : Police (3 familles génériques) · Taille
   (4 crans) · G/I/S/Barré · Couleur (nuancier 12 teintes) · Alignement
   G/C/D · Listes · Retrait −/+ · Effacer (`execCommand`,
   `styleWithCSS=false`) ; états actifs (`queryCommandState`),
   sélection photographiée pour survivre aux `<select>`.
3. Reprise d'un brouillon : `body_html` → `innerHTML` ; brouillon texte
   → conversion locale (`texteEnHtml`, miroir du cœur), garde
   anti-churn si fermé sans frappe.
4. Catalogue fr/en (ADR 0016) ; sous-ensemble d'icônes 46 → 58.
5. e2e : specs adaptés (`toContainText`/`innerText`) + nouveau cas
   (gras appliqué → brouillon riche survit à la reprise ; « Effacer la
   mise en forme » nettoie).
6. **Système amendé dans le même commit** (DC-D2, journal A62 ;
   inventaire d'icônes DC-D3). La barre reprend le vocabulaire existant
   (puces 32 px) — pas de maquette d'étude séparée : aucun langage
   visuel nouveau.

### E6 — Qualité, terrain, release
1. `/code-review high` sur le diff complet, corrections confirmées.
2. `/gate` — un rouge = andon.
3. **⛔ STOP 2 — terrain** : checklist remise au CE (envoi réel mis en
   forme à soi-même, relecture dans Wind ET Gmail web, repli texte
   vérifié à la source du message, reprise d'un vieux brouillon texte,
   brouillon riche poussé/relu, thème sombre), commandes PowerShell
   fournies.
4. Après terrain : release **0.2.0** (`scripts/faire-release.ps1`),
   auto-update confirmé, CI verte, `/solde`.

## Terrain (STOP 2) — verdicts du 2026-08-20

- **Constat 1 (bloquant, corrigé le jour même)** : jetons OAuth des deux
  comptes Gmail révoqués (`invalid_grant`), aucun geste de réparation.
  → Réglages > Comptes dit l'état par compte (« Déconnecté ») et offre
  **« Reconnecter »** (consentement navigateur sur la ligne existante,
  garde d'identité) ; commande `reconnect_account`, spec e2e dédiée,
  journal **A63**. **« Reconnexion OK sur les deux comptes »** (CE).
- **Constat 2 (corrigé le jour même)** : l'avis « Compte non
  reconnecté » proposait « Réessayer » (connexion silencieuse,
  condamnée avec un jeton mort) → remplacé par **« Réglages »**, porte
  directe vers la page Comptes (consigné à A63).
- **Points 1 à 8 : OK au terrain** (CE, 2026-08-20) — barre complète,
  envoi riche relu dans Wind et Gmail web (multipart/alternative, repli
  texte non vide), citation/amorce/top-posting, brouillons riches sans
  churn, vieux brouillons texte intacts, frappe sûre, placeholder,
  thème sombre.
- **D5 — images distantes de la citation** : **pixel neutre validé pour
  la RÉPONSE seule** ; pour le **TRANSFERT, les images distantes sont
  transmises** (pas de pixel neutre) — appliqué le jour même
  (`forward_context` en `AllowRemote`, frontière idempotente), consigné
  à §6.4 du STANDARD et au Système (A62). Re-passe terrain ciblée : un
  transfert d'infolettre arrive avec ses images.

## § Décisions CE

- **D1 — Lien et Citation.** La barre inerte actuelle montre aussi
  « Lien » et « Citation », que R4 ne demande pas. Les retirer
  (périmètre strict, recommandé — dire non est le défaut §2.6), ou les
  câbler aussi (`createLink` sur URL saisie ; `blockquote`) ?
  → **Réponse CE (2026-08-19) : « Retirer »** — périmètre strict R4, la
  barre livre exactement les boutons demandés ; Lien et Citation
  reportables en retour terrain s'ils manquent à l'usage.
- **D2 — Polices et tailles proposées.** Un courriel n'emporte que les
  polices du destinataire : liste courte web-safe recommandée
  (Sans-serif / Serif / Monospace, servies par la pile système), ou
  liste nommée (Arial, Georgia, Times, Courier…) ?
  Tailles : quatre crans (Petit / Normal / Grand / Très grand) sur
  `font size` 1-7, recommandé.
  → **Réponse CE (2026-08-19) : « Génériques »** — Sans-serif / Serif /
  Monospace, servies par la pile système du destinataire ; tailles en
  4 crans (Petit / Normal / Grand / Très grand).
- **D3 — Couleur du texte.** Nuancier fixe (une douzaine de teintes
  sûres, recommandé — cohérent, lisible partout), ou sélecteur natif
  libre (16 M de couleurs) ?
  → **Réponse CE (2026-08-19) : « Nuancier fixe »** — une douzaine de
  teintes sûres (encre, gris, rouge, orange, vert, bleu, violet…),
  lisibles sur la dalle claire du corps, un clic.
