# PLAN-RETOURS-4 — quatre retours terrain (2026-08-18)

Quatrième lot de retours du Chef Ingénieur, dans la foulée de la 0.1.10.
Quatre demandes : (R1) le téléchargement d'une pièce jointe, (R2) la
fusion nom + poids d'une pièce en une puce, (R3) la lisibilité du corps
sur thème sombre, (R4) une barre de mise en forme réelle dans le
composeur.

> **État : R1-R2-R3 implémentés, `/code-review high` passé (2 correctifs
> appliqués, 1 écart assumé), `/gate` complète VERTE (91 e2e). R4 en
> chantier dédié. En attente de STOP 2 — terrain du correctif R3.**
> Décisions consignées en bas.
>
> Gate 2026-08-18 : fmt OK · ui-v2 build 0 avert. · contraste 28/700 ·
> coherence 28/476 · garde-thread 65 · clippy muet · tests Rust 0 échec ·
> doc OK · **e2e 91 passed**.

---

## Constat — genchi genbutsu, faits vérifiés dans le code

### R1 — « Cliquer une pièce ne fait rien ; ça devrait ouvrir un choix de dossier. »

- La puce d'une pièce en lecture est bien câblée : `Fil.svelte:139-156`
  (`enregistrer`) → commande `save_attachment`
  (`commands.rs:1911-1949`).
- **`save_attachment` enregistre en SILENCE dans le dossier
  Téléchargements** (`app.path().download_dir()`), nom rendu unique
  (`unique_path`), puis retourne le chemin ; l'UI lève un toast
  « Pièce enregistrée : {chemin} ». **Aucun dialogue** ne s'ouvre.
- L'infobulle le dit déjà : `lecture.enregistrer` =
  « Enregistrer dans Téléchargements » (`catalogue.fr.js:147`).
- Donc du point de vue du CE « il ne se passe rien » : pas de dialogue,
  le fichier atterrit dans Téléchargements sans confirmation visible.
- **À mesurer au terrain (Phase 0 à compléter) :** un fichier apparaît-il
  *effectivement* dans Téléchargements au clic, et un toast (vert
  « Pièce enregistrée : … » ou rouge d'erreur) s'affiche-t-il ? Cela
  tranche entre « enregistrement muet mais réussi » (UX à corriger) et
  « échec réel du rapatriement IMAP » (bug de fond distinct). La demande
  du CE — ouvrir un dialogue de choix — reste valable dans les deux cas,
  mais si le *fetch* échoue, le dialogue seul ne suffira pas.
- Écart d'architecture noté (hors bug) : `save_attachment` ouvre le
  `Store` hors `hors_pompe` (contrairement à `message_attachments`) —
  sans conséquence de gel (async, hors thread principal), à ranger.

### R2 — « Regrouper poids et nom d'une pièce dans la même puce. »

- En lecture, chaque pièce rend **DEUX puces** (`Fil.svelte:234-241`) :
  un `<button class="puce bouton">` (icône `description` + nom) puis une
  `<span class="puce">` séparée (icône `storage` + taille).
- Le composeur, lui, fait DÉJÀ l'inverse : nom + taille dans la MÊME
  puce (`Composition.svelte:704-712`, commentaire l.855-858 :
  « nom + taille + retrait dans la MÊME puce — un objet manipulable,
  pas deux lectures »). R2 aligne la lecture sur le composeur.
- Le CE pose explicitement l'**exception à la règle « 1 puce = 1
  information »** (Système). L'amendement du Système (DC-D2) doit
  consigner l'exception et le contrat d'icône (DC-D3).

### R3 — « Sur thèmes sombres, le texte standard des messages est noir sur fond sombre, illisible. »

- Le corps est rendu dans une iframe `sandbox` avec `srcdoc` produit par
  `mail_render::email_document` (`Fil.svelte:227`), qui **bake** dans le
  `<style>` du document : `body{color:{encre};background:{fond}}`
  (`mail-render/src/lib.rs:126-146`).
- La palette vient du front : `paletteLecture()` lit `--ink` (encre) et
  `--surface` (fond) aux jetons calculés (`theme.js:121-127`), passée à
  `message_body`/`echo_body` (`fil.svelte.js:110-117`,
  `commands.rs:1793-1824`, `2187-2211`).
- Sur les thèmes `-nuit`, `--surface` est sombre et `--ink` clair — le
  **texte SANS couleur propre hérite donc du clair** : le mécanisme A42
  est correct pour ce cas.
- **La racine probable :** `mail-render/src/sanitize.rs` **CONSERVE
  volontairement les couleurs de l'expéditeur** — `style="color:…"`,
  `<font color>` (test `keeps_table_layout_used_by_newsletters`,
  l.251-258 : `color: #333` gardé). Or l'immense majorité des courriels
  (infolettres, transactionnels, courriers pro rédigés dans Gmail/Outlook)
  fixent un texte **sombre** pensé pour un fond blanc. Sur la dalle
  sombre A42, ce texte sombre devient illisible.
- Tension de conception : **A42 a rendu la dalle du corps sombre**
  (pour éviter le « pavé blanc » sur les 14 thèmes sombres) et c'est
  précisément ce choix qui expose le texte sombre d'expéditeur. R3
  rouvre donc A42. Décision CE (D3).
- **À mesurer au terrain :** le noir apparaît-il sur TOUS les courriels
  (y compris un courriel personnel sans couleur → alors bug de palette)
  ou seulement sur ceux qui portent leurs couleurs (infolettres →
  alors tension de conception A42) ? Ouvrir un courriel personnel simple
  et une infolettre, en thème `-nuit`, tranche.

### R4 — « Une vraie barre de mise en forme dans le composeur. »

Boutons demandés : Police · Taille · Gras/Italique/Souligné/Barré ·
Couleur du texte · Alignement (G/C/D) · Liste à puces · Liste numérotée ·
Diminuer le retrait · Augmenter le retrait · Effacer la mise en forme.

- Le composeur est aujourd'hui un **`<textarea>` texte brut**
  (`Composition.svelte:698-701`) ; la « barre de format » est **inerte** :
  six `<span>` décoratifs G/I/S/Liste/Lien/Citation
  (`Composition.svelte:743-750`, commentaire l.22 : « Inertes comme au
  prototype »).
- Le modèle est **texte brut de bout en bout** :
  - `mail-core` : `Draft.body_text`, `OutboxMessage.body_text`
    (`compose.rs:31`), `DraftContent.body` / `SavedDraft.body`
    (colonne DB, `drafts.rs:47,80`).
  - citation : `quote_reply` / `quote_forward` produisent du **texte** à
    préfixes `>` et séparateurs `----` (`compose.rs:191-229`).
  - `mail-smtp` : `SinglePart::plain(body_text)` /
    `MultiPart::mixed` (`lib.rs:170-195`, `draft_bytes` l.220-265).
- **R4 est une tranche verticale majeure** : un corps HTML doit
  traverser mail-core (modèle + **migration de colonne** brouillons,
  invariant #7), mail-smtp (**multipart/alternative** : text/html +
  repli text/plain, puis mixed avec les pièces), la citation (blockquote
  HTML), commands.rs (queue_send / save_draft / contextes), et l'UI
  (éditeur `contenteditable` + barre d'outils, HTML compatible ammonia).
- La lecture est déjà prête : nos propres envois (Envoyés) repassent par
  `mail_render::sanitize` — le HTML produit doit n'employer que ce
  qu'ammonia garde (`font`/`color`/`face`/`size`, `style`, `b/i/u/s`,
  `ul/ol/li`, `align`, `blockquote`). Tout est déjà autorisé
  (`sanitize.rs:49-62`).
- Cette tranche touche les **règles d'or de la boîte d'envoi** (ADR 0003)
  et exige une migration rembobinable (ADR 0012, invariant #7) : elle
  mérite son propre chantier et sa propre passe terrain. **Recommandation
  D1 : R4 en chantier dédié (PLAN-COMPOSITION-HTML), après R1-R2-R3.**

---

## Périmètre

**Dans ce plan (si D1a) :** R1, R2, R3. Chacun petit, cerné, validable au
terrain le jour même.

**Reporté en chantier dédié (si D1a) :** R4 — composeur enrichi HTML
(esquisse ci-dessous), pour ne pas mêler trois correctifs simples à une
tranche verticale qui touche l'envoi.

**Refus de périmètre explicites (§2.6) :**
- R3 option « dalle sombre + neutralisation heuristique des couleurs
  sombres d'expéditeur » : fragile (casse liens, titres colorés, CTA,
  images à transparence) — écartée sauf demande CE.
- R4 : pas de collage d'images inline dans le corps, pas de tableaux à la
  main, pas de polices web distantes (CSP) — hors de la demande, à
  reporter si besoin.

---

## Options et verdicts

### R1 — comportement du téléchargement
- **(a) Dialogue « Enregistrer sous » natif** (choix dossier + nom
  pré-rempli au nom de la pièce, dossier initial = Téléchargements).
  C'est la demande du CE. Coût : `plugin:dialog|save` +
  capability `dialog:allow-save` + `save_attachment` prend le chemin
  cible. **Recommandé.**
- (b) Garder Téléchargements, renforcer le retour visible seulement.
  Ne répond pas à la demande.

### R3 — lisibilité du corps sur thème sombre
- **(a) Dalle claire du corps, toujours** (encre sombre / fond clair,
  quel que soit le thème de l'app) — comme Gmail/Outlook/Apple Mail. Le
  courriel est rédigé pour un fond clair : lisible par construction,
  simple, robuste. **Annule le noircissement A42.** Perte : le corps
  n'est plus « immersif sombre ». **Recommandé (robuste).**
- (b) Dalle sombre conservée, encre « standard » forcée claire. Répond à
  la lettre (« passer le texte en blanc ») MAIS le texte à couleur
  d'expéditeur (fréquent) restera sombre : ne règle qu'une partie.
- (c) Mesurer d'abord, puis trancher (a) ou (b) selon que le noir touche
  tous les courriels ou seulement ceux à couleurs.

### R4 — approche du composeur enrichi (si engagé)
- `contenteditable` + Selection/Range API, sérialisation HTML
  compatible ammonia, envoi en **multipart/alternative**. Approche
  standard, unique option sérieuse — pas de départage set-based à faire.
  Le point dur n'est pas l'algorithme mais le **périmètre** (D1) et la
  **migration des brouillons** (invariant #7).

---

## Étapes

### E1 — R1 : dialogue d'enregistrement des pièces — **LIVRÉ (code)**
1. Rust : nouvelle commande `chemin_enregistrement_suggere` (Téléchargements
   + `safe_file_name`, `unique_path` — l'autorité de désinfection reste au
   cœur) ; `save_attachment(…, dest)` écrit au chemin **choisi**, sans plus
   toucher `download_dir` (`commands.rs`). Registrée dans `main.rs`.
2. UI : `enregistrer` (`Fil.svelte`) → chemin suggéré → `choisirDestination`
   (`plugin:dialog|save`, `transport.js`) → annuler = rien (ni toast ni
   fetch) → `save_attachment(dest)` + toast.
3. Capability `dialog:allow-save` ajoutée (`capabilities/default.json`).
4. Catalogue : infobulle `lecture.enregistrer` → « Enregistrer… » / « Save… »
   (fr + en) — elle ne ment plus « dans Téléchargements ».
5. Couture e2e : `__e2eDestination` (symétrique de `__e2ePieces`).
6. Écrit compile (`cargo check`) + front build OK. Gate complète : à la fin
   du chantier, avec E2/E3.

### E2 — R2 : fusion nom + poids en une puce (lecture) — **LIVRÉ (code)**
1. `Fil.svelte` : une seule `<button class="puce bouton">` par pièce =
   `[description] {nom} {poids}` (nom encre pleine, poids atténué) ; la
   `<span class="puce"> storage` séparée retirée ; CSS `.nom`/`.taille`.
2. Système amendé (A59/A60, DC-D2/DC-D3) : règle « 1 puce = 1 info »
   porte l'exception ; les trois maquettes de pièces fusionnées ; icône
   `storage` retirée de l'usage, réservée au sous-ensemble
   (`assets/icones/README.md`, précédent A53).
3. e2e : `piece-jointe` une par pièce, portant nom ET poids (« 220 Ko »)
   — `refonte-ecran02` et `refonte-volets`.
4. Front build OK. Gate complète à la fin.

### E3 — R3 : dalle claire du corps, toujours — **LIVRÉ (code)**
1. Mesure terrain faite (2026-08-18) : seul le texte à couleurs
   d'expéditeur est noir sur sombre → (a) dalle claire.
2. `message_body` / `echo_body` (`commands.rs`) : param `palette` retiré,
   `email_document(&…, policy, &Palette::default())` — dalle claire
   toujours. Struct `PaletteLecture` supprimée.
3. Front : `fil.svelte.js` ne passe plus `palette` ; `paletteLecture()`
   retirée de `theme.js` (import retiré). Les tests A42 de mail-render
   (`email_document_bake_la_palette…`, `…color_scheme…`) restent verts —
   ils testent la FONCTION `email_document`, toujours capable de baker une
   palette ; l'app n'en passe simplement plus qu'une claire.
4. Système : paragraphe du corps réécrit + journal A61 (DC-D2).
5. Gate complète : à la fin, avec E1/E2.

### R4 — composeur enrichi HTML (esquisse, chantier dédié si D1a)
Sous-tranches, dans l'ordre, chacune TDD :
- **E4.1 mail-core** : `body_html` porté par Draft/OutboxMessage/
  DraftContent/SavedDraft ; **migration** colonne `body_html`
  (rembobinable, test sur base de fichier — invariant #7).
- **E4.2 mail-smtp** : `multipart/alternative` (text/plain dérivé +
  text/html) ; avec pièces, `mixed(alternative, pièces…)` ; idem
  `draft_bytes`.
- **E4.3 citation HTML** : variantes `quote_reply`/`quote_forward` en
  `<blockquote>` ; l'amorce et le top-posting conservés.
- **E4.4 commands.rs** : queue_send / save_draft / reply|forward_context
  portent le HTML (+ le plain dérivé).
- **E4.5 UI** : `contenteditable` + barre d'outils réelle (Police,
  Taille, G/I/S/Barré, Couleur, Alignement, Listes à puces/numérotée,
  Retrait −/+, Effacer la mise en forme) ; sérialisation HTML restreinte
  au vocabulaire ammonia ; catalogue fr/en ; Système (DC-D2).
- Passe terrain dédiée (envoi réel + relecture dans Envoyés + auto-update).

---

## § Décisions CE (2026-08-18)

- **D1 — Séquencement de R4.** → **R4 en chantier dédié après R1-R3.**
  Ce plan livre R1-R2-R3 ; R4 (composeur enrichi HTML) part dans
  PLAN-COMPOSITION-HTML, avec sa propre passe terrain et sa release.
- **D2 — R1, comportement.** → **Dialogue « Enregistrer sous ».** Choix
  du dossier + nom pré-rempli, dossier initial = Téléchargements.
- **D3 — R3, dalle du corps.** → **Mesuré (2026-08-18), puis tranché :
  dalle claire (blanc pur), toujours.** Mesure terrain : le texte SANS
  couleur propre est déjà clair/lisible (palette A42 OK pour lui) ; seul
  le texte À couleurs d'expéditeur (infolettres) est noir sur sombre.
  Cela écarte (b) — ces couleurs priment. Le corps s'affiche donc
  toujours sur dalle claire (`Palette::default`, blanc pur, tous thèmes),
  renversant la dalle sombre d'A42. R1 et R2 validés au terrain sur la
  build source le 2026-08-18 (« R1 et R2 OK »).
- **D4 — R2, fusion de la puce.** → **Oui, fusionner, icône unique
  `description`.** Une puce cliquable = `[description] nom · poids` ;
  exception « 1 puce = 1 information » consignée au Système (DC-D2/DC-D3).
