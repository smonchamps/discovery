# Icônes vendorisées — sous-ensemble Material Symbols Rounded (R0-S3)

**32 glyphes**, **15 984 octets** (15,6 Kio) woff2, servis depuis le dépôt.
Hors ligne et CSP (`font-src 'self'`) par construction — **aucun CDN**,
jamais.

## Inventaire (la source : le Système « Clarity »)

Relevé du handoff (classe `ms` et `icon:'…'`), amendé par le journal :
A11 ajoute `person_add` (section Comptes des Réglages) ; A12 retire
`forward` (verdict terrain du 2026-08-12 — « Transférer » porte
désormais `reply` en symétrie verticale, `.ms.miroir`, aucun glyphe
neuf requis). `arrow_forward` est présent mais INUTILISÉ (première
lecture du verdict) : à purger à la prochaine régénération.

`all_inbox` `archive` `arrow_back` `arrow_forward` `attach_file`
`bookmark` `check_circle` `close` `delete` `description` `drafts`
`edit_note` `edit_square` `format_list_bulleted` `format_quote` `forum`
`group_add` `inbox` `link` `mark_email_unread` `open_in_new` `person`
`person_add` `reply` `report` `search` `send` `settings` `storage`
`unfold_more` `visibility_off` `work`

Ajouter un glyphe = régénérer le fichier (ci-dessous) **et** tenir cette
liste à jour — l'inventaire est le contrat.

## Axes retenus — le besoin réel, rien de plus

| Axe | Plage | Pourquoi |
|---|---|---|
| `opsz` | **20** (figé) | icônes à 16 px, toujours |
| `wght` | **300–600** | 300 partout ; 600 pour le dossier ouvert |
| `FILL` | **0–1** | contour partout ; rempli pour le dossier ouvert |
| `GRAD` | — (retiré) | jamais prescrit par le Système |

Axes pleins : 48,6 Kio. Axes resserrés : **15,1 Kio** (−69 %).

## Provenance et régénération

Police [Material Symbols](https://github.com/google/material-design-icons)
(Google), **Apache 2.0** — texte dans [LICENSE](LICENSE). Le découpage par
ligatures ne se fait pas proprement en local (la fermeture GSUB retiendrait
tout) : on passe par le subsetteur de Google Fonts, **une fois**, puis le
fichier vit ici.

```bash
NOMS="all_inbox,archive,...(la liste ci-dessus, triée, séparée par des virgules)"
curl -s -A "Mozilla/5.0 ... Chrome/126.0" "https://fonts.googleapis.com/css2?family=Material+Symbols+Rounded:opsz,wght,FILL@20,300..600,0..1&icon_names=$NOMS&display=block"
# puis télécharger l'URL fonts.gstatic.com/l/font?... du @font-face renvoyé
# dans material-symbols-rounded.subset.woff2
```

## Usage (la classe du Système)

```css
@font-face {
  font-family: 'Material Symbols Rounded';
  font-style: normal;
  font-weight: 300 600;
  font-display: block;
  src: url('material-symbols-rounded.subset.woff2') format('woff2');
}
.ms {
  font-family: 'Material Symbols Rounded';
  font-weight: 300; font-size: 16px; line-height: 1; flex: none;
  font-variation-settings: 'opsz' 20, 'FILL' 0;
}
/* Dossier ouvert : rempli, graisse 600, dans l'accent. */
.ms.ouvert { font-variation-settings: 'opsz' 20, 'FILL' 1; font-weight: 600; }
```

## Preuve ([apercu.html](apercu.html))

Page servie localement sous CSP `default-src 'none'; font-src 'self'` :
**PASS — police locale chargée, 32/32 ligatures repliées** (les 31 glyphes
+ le témoin FILL 1/600). Vérification objective : une ligature résolue se
replie sur ~1 em ; un nom resté en toutes lettres est bien plus large.
