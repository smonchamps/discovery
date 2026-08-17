# Icônes vendorisées — sous-ensemble Material Symbols Rounded (R0-S3)

**46 glyphes**, **21 500 octets** (21,0 Kio) woff2, servis depuis le dépôt.
Hors ligne et CSP (`font-src 'self'`) par construction — **aucun CDN**,
jamais.

## Inventaire (la source : le Système « Clarity »)

Relevé du handoff (classe `ms` et `icon:'…'`), amendé par le journal :
A11 ajoute `person_add` (section Comptes des Réglages) ; A12 retire
`forward` (verdict terrain du 2026-08-12 — « Transférer » porte
désormais `reply` en symétrie verticale, `.ms.miroir`, aucun glyphe
neuf requis). A13 (Réglages en deux volets) purge `arrow_forward`
(inutilisé depuis la précision d'A12) et ajoute les glyphes des groupes :
`display_settings` (Affichage), `keyboard` (Raccourcis), `notifications`
(Notifications), `info` (À propos) — « une icône, un sens » (A3), aucun
réemploi. A14 ajoute `reply_all` (« Répondre à tous », barres d'actions
de la lecture et de la conversation). A16 (PLAN-SYNCHRO E3) ajoute
`sync` (bouton de relève manuelle, barre d'état — D5 rouverte). A17
(PLAN-PIECES-JOINTES E3) ajoute `hourglass_empty` (puce d'une pièce en
rapatriement, transfert) et `warning` (refus au plafond du composeur).
Terrain 0.1.4 (2026-08-14) : les icônes des avis RARES de la fente
n'étaient JAMAIS entrées dans la police — `error` (échec d'envoi),
`link_off` (reconnexion), `system_update_alt` (mise à jour, vu au
premier auto-update), `volunteer_activism` (télémétrie) ; ajoutées
toutes les quatre, et l'inventaire se vérifie désormais par BALAYAGE
des sources (`grep` des noms utilisés), pas de mémoire. PLAN-VOLETS E2
(2026-08-15) ajoute `menu` (bouton du tiroir de navigation, mode un
volet). PLAN-WADA E1 (2026-08-15, A32) ajoute `inventory_2` (dossier
Archives de la nav au dessin des pistes, W2-D3) et retire `work` au
balayage (patron A13 : plus employé nulle part — la tuile de la boîte
en cours porte `person`, W2-D5/D7) ; le compte reste à 44. Terrain A46
(2026-08-16) ajoute `open_in_full` (« Ouvrir » — le volet vers l'écran
03 ; `unfold_more` servait DEUX sens, contraire à A3) et `unfold_less`
(« Replier », la bascule du fil) ; le compte passe à 46. A53
(PLAN-RETOURS-2, D2) retire le bouton « Rendre indépendante » du
composeur : `open_in_new` n'est plus employé, mais il est **conservé au
sous-ensemble** (réservé) — le multi-fenêtre revient en chantier dédié,
régénérer la police pour un seul glyphe serait disproportionné ; le
compte reste à 46.

`all_inbox` `archive` `arrow_back` `attach_file` `bookmark`
`check_circle` `close` `delete` `description` `display_settings`
`drafts` `edit_note` `edit_square` `error` `format_list_bulleted`
`format_quote` `forum` `group_add` `hourglass_empty` `inbox` `info`
`inventory_2` `keyboard` `link` `link_off` `mark_email_unread` `menu`
`notifications` `open_in_full` `open_in_new` `person` `person_add`
`reply` `reply_all` `report` `search` `send` `settings` `storage`
`sync` `system_update_alt` `unfold_less` `unfold_more` `visibility_off`
`volunteer_activism` `warning`

Ajouter un glyphe = régénérer le fichier (ci-dessous), tenir cette
liste à jour — l'inventaire est le contrat — **et** incrémenter le
`?v=` de l'URL `@font-face` (`systeme.css`) : le nom du fichier ne
change pas et le cache HTTP de WebView2 survit aux mises à jour — sans
cache-buster, un poste peut servir l'ancien sous-ensemble à la version
neuve.

**La police vit en DEUX exemplaires** : ici (la source vendorisée) et
[`apps/desktop/ui-v2/public/icones/`](../../apps/desktop/ui-v2/public/icones/)
— la copie que Vite sert réellement (`/icones/…` dans `systeme.css`).
Régénérer sans recopier laisse l'app sur l'ancien sous-ensemble : la
ligature nouvelle reste en toutes lettres à l'écran (vécu à l'ajout de
`reply_all`). Toute régénération recopie donc le fichier dans `public/`.

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
**PASS — police locale chargée, 47/47 ligatures repliées** (les 46 glyphes
+ le témoin FILL 1/600 ; rejouée le 2026-08-16, terrain A46).
Vérification objective : une ligature résolue se replie sur ~1 em ; un
nom resté en toutes lettres est bien plus large.
