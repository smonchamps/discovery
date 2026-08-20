# ADR 0022 — Corps riche : body_html à côté du texte, frontière unique, images par geste

Date : 2026-08-20 · Statut : accepté

## Contexte

R4 (PLAN-COMPOSITION-HTML) apporte la mise en forme au composeur. L'ADR
0003 avait explicitement refusé le HTML sortant (« texte brut seul ») ;
le lever oblige à décider où vit le HTML, qui fait autorité sur le
texte du repli, et ce que deviennent les images distantes d'un contenu
cité — trois décisions qui traversent brouillons, journal d'envoi,
SMTP, tirage et éditeur.

## Décision

**1. `body_html` en colonne NULLable À CÔTÉ du texte** (`drafts`,
`outbox`) — jamais à sa place. `body`/`body_text` restent TOUJOURS
peuplés : aperçus, recherche, repli `text/plain`. `NULL` = chemin texte
historique, octet pour octet (les bases héritées ne bougent pas —
migration `add_missing_columns`, rembobinable).

**2. UNE frontière d'entrée** (`frontiere_corps`, commands.rs) pour
TOUT corps qui entre en base — composeur, envoi, tirage : assainit par
ammonia, DÉRIVE le texte du repli du même HTML (une seule autorité,
jamais deux vérités), retombe en chemin texte si le rendu est vide (le
`<br>` résiduel d'un éditeur vidé ne fait pas un envoi au `text/plain`
vide). L'éditeur `contenteditable` n'accepte QUE du HTML passé par
cette frontière — c'est l'exception bornée à « jamais `innerHTML` »
(STANDARD §6.4).

**3. L'envoi part en `multipart/alternative`** (texte d'abord — RFC
2046, du plus simple au plus fidèle), emboîté dans `mixed` avec les
pièces ; le reflet Brouillons pareil (`draft_bytes`). Jamais de HTML
seul.

**4. Les images distantes se décident AU GESTE** (verdict terrain D5) :
une **réponse** cite au pixel neutre — une citation `AllowRemote`
reposée dans l'éditeur (document principal, CSP `img-src https:`)
chargeait les pixels espions du message cité au simple clic
« Répondre » (revue du 2026-08-20) ; un **transfert** conserve les
images — le destinataire reçoit le message entier, composer le
transfert vaut « afficher les images » implicite. La frontière est
`AllowRemote` (idempotence : elle ne re-neutralise pas ce que l'amont
a décidé).

**5. Le vocabulaire HTML est l'allowlist ammonia de la LECTURE** —
l'éditeur émet via `execCommand` legacy (`styleWithCSS` éteint), dont
la sortie (`b/i/u/strike`, `font color/face/size`, `align`, listes,
`blockquote`) est exactement ce que `sanitize.rs` conserve : rien à
élargir, aucune couche de traduction.

## Écarté

- Remplacer `body` par le HTML (une colonne) : casse aperçus, recherche
  et repli — migration lourde pour rien.
- Éditeur tiers (Quill, ProseMirror) : dépendance lourde, modèle de
  document propre à re-mapper vers ammonia — contraire à l'ADR 0015.
- `BlockRemote` au transfert (première forme) : renversé au terrain
  (D5) — un transfert amputé de ses images ne transmet pas le message.

## Conséquences et vigilances

- Un brouillon réouvert puis fermé SANS frappe doit ré-émettre les
  valeurs stockées à l'octet près : la re-sérialisation `innerHTML` du
  navigateur n'est jamais fidèle, et la détection « contenu identique »
  du cœur re-pousserait une copie Gmail à chaque ouverture (STANDARD
  §9, pièges du contenteditable).
- Le tirage (`import_remote_draft`) porte le HTML : sans lui, un
  brouillon riche poussé puis re-rapatrié perdait sa mise en forme en
  silence.
- Tests des règles : `save_draft_roundtrips_body_html`,
  `body_html_change_marks_the_draft_dirty`,
  `enqueue_roundtrips_body_html`,
  `html_body_travels_as_multipart_alternative_with_plain_fallback`,
  `html_body_with_pieces_nests_alternative_inside_mixed`,
  `l_echo_d_un_envoi_riche_porte_le_html_compose`,
  `import_remote_draft_keeps_the_rich_body`, e2e « mise en forme ».
