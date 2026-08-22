// Banc de contraste WCAG des jetons Clarity (systeme.css) : chaque
// paire (encre, fond) réellement posée par ui-v2, dans les 28 thèmes
// (A42). Seuils : 4,5:1 pour le texte courant, 3:1 pour le grand texte
// et les composants d'interface (icônes, bordures porteuses de sens).
//
//   node contraste.mjs        -> tableau complet + verdict
//
// La source est systeme.css (les jetons expédiés), pas le prototype :
// c'est ce que l'utilisateur voit qui se mesure.
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { lireThemes, NOMBRE_ATTENDU } from './jetons.mjs';

const root = path.resolve(import.meta.dirname, '..');
const css = readFileSync(
  path.join(root, 'apps', 'desktop', 'ui-v2', 'src', 'systeme.css'),
  'utf8',
);

// Le parseur partagé des deux gates (jetons.mjs) ; seuls les hex sont
// mesurables ici (les ombres et scrims rgba() ne portent pas de paire).
const themes = lireThemes(css, { motifValeur: /#[0-9a-fA-F]{6}/ });

// --- Luminance et rapport WCAG ---------------------------------------
function lum(hex) {
  const c = [1, 3, 5].map((i) => {
    const v = parseInt(hex.slice(i, i + 2), 16) / 255;
    return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
}
const rapport = (a, b) => {
  const [l1, l2] = [lum(a), lum(b)].sort((x, y) => y - x);
  return (l1 + 0.05) / (l2 + 0.05);
};

// --- Les paires posées par ui-v2 (relevé des composants) -------------
// [encre, fond, seuil, où]
const PAIRES = [
  ['ink', 'bg', 4.5, 'titres, objets de liste'],
  ['ink', 'surface', 4.5, 'cartes, boutons, entête'],
  ['ink', 'panel', 4.5, 'nav, barre de format'],
  ['ink', 'sel', 4.5, 'rangée sélectionnée'],
  ['ink', 'hover', 4.5, 'rangée survolée (A29/A35)'],
  ['ink2', 'bg', 4.5, 'expéditeurs, corps de texte'],
  ['ink2', 'surface', 4.5, 'puces, boutons secondaires'],
  ['ink2', 'panel', 4.5, 'nav (libellés)'],
  ['ink2', 'sel', 4.5, 'aperçus (rangée sélectionnée)'],
  ['ink2', 'hover', 4.5, 'aperçus (rangée survolée)'],
  ['muted', 'bg', 4.5, 'heures, aperçus, statut'],
  ['muted', 'surface', 4.5, 'kickers, descriptions, placeholder'],
  ['muted', 'panel', 4.5, 'statut, sections nav'],
  ['muted', 'sel', 4.5, 'heures (rangée sélectionnée) — le remède A35'],
  ['muted', 'hover', 4.5, 'heures (rangée survolée)'],
  ['onAccent', 'accent', 4.5, 'boutons principaux, pastille de non-lus (nav)'],
  ['alert', 'bg', 4.5, "texte d'erreur (onboarding), mention Brouillon (rangée)"],
  ['alert', 'sel', 4.5, 'mention Brouillon (rangée choisie)'],
  ['alert', 'hover', 4.5, 'mention Brouillon (rangée survolée)'],
  ['alert', 'surface', 4.5, 'mention Brouillon (cartes)'],
  ['alert', 'surface', 3, "icône/bordure d'alerte (fente)"],
  ['accent', 'surface', 3, 'icônes signature, coche, héros non-lu'],
  ['accent', 'panel', 3, 'trait hitofude (entête, barre d’état)'],
  ['accent', 'sel', 3, 'liseré de la ligne choisie (A29)'],
  ['tuileInk', 'tuile', 4.5, 'la tuile de la boîte en cours (nav, A35)'],
];

let echecs = 0;
// Plancher de complétude : un thème que le motif ne reconnaît pas n'est
// pas un thème « en moins », c'est un thème JAMAIS mesuré qui part en
// production — le trou exact des 14 -nuit invisibles à [a-z]+ (A42).
if (Object.keys(themes).length !== NOMBRE_ATTENDU) {
  echecs += 1;
  console.log(`ECHEC ${Object.keys(themes).length} thème(s) extraits de systeme.css — ${NOMBRE_ATTENDU} attendus (jetons.mjs) : un bloc échappe au motif, ou la table a changé sans amender le plancher`);
}
// Seuls les ÉCARTS s'impriment (revue A42) : 28 × 25 lignes « ok »
// noyaient les échecs — le verdict final porte les comptes, un rouge
// reste bruyant et localisé.
let mesures = 0;
for (const [nom, t] of Object.entries(themes)) {
  for (const [encre, fond, seuil, ou] of PAIRES) {
    if (!t[encre] || !t[fond]) {
      // Un jeton introuvable n'est pas une paire à sauter : c'est le
      // banc qui ment. Bruyant, comme tout échec.
      echecs += 1;
      console.log(`ECHEC ${nom} · ${encre} sur ${fond} : jeton introuvable dans systeme.css  ${ou}`);
      continue;
    }
    mesures += 1;
    const r = rapport(t[encre], t[fond]);
    if (r < seuil) {
      echecs += 1;
      console.log(
        `ECHEC ${nom} · ${encre.padEnd(9)} sur ${fond.padEnd(8)} `
        + `${r.toFixed(2).padStart(5)}:1  (seuil ${seuil}:1)  ${ou}`,
      );
    }
  }
}
// --- REPERES (A74, PLAN-RETOURS-8 R1) --------------------------------
// Le nuancier des repères de compte : 12 familles × 2 déclinaisons
// (sombre pour les 14 thèmes clairs, claire pour les 14 -nuit). On lit
// les hex EXPÉDIÉS (les règles .repere[data-teinte] de systeme.css),
// jamais une copie : chaque déclinaison doit tenir 3:1 (composant) sur
// les fonds où le repère se pose, et porter son glyphe à 4,5:1.
const REPERE_FAMILLES = 12;
// `tuile` en fait partie (revue 2026-08-22) : la pastille du compte EN
// COURS se pose sur la tuile de nav — l'oublier laissait ce fond-là,
// précisément, sans mesure.
const FONDS_REPERE = ['panel', 'bg', 'sel', 'hover', 'surface', 'tuile'];
function lireReperes(prefixe) {
  const reperes = {};
  for (const [, teinte, hex] of css.matchAll(new RegExp(
    `${prefixe}\\.repere\\[data-teinte="([a-z]+)"\\]\\s*\\{\\s*background:(#[0-9a-fA-F]{6})`,
    'g',
  ))) {
    reperes[teinte] = hex;
  }
  return reperes;
}
const sombres = lireReperes('(?<!-nuit"\\] )');
const claires = lireReperes('\\[data-theme\\$="-nuit"\\] ');
// Les encres des glyphes se LISENT du CSS expédié, comme les fonds —
// une copie locale mentirait dès que systeme.css bouge (revue).
const encreSombre = css.match(/\.repere\s*\{[^}]*color:(#[0-9a-fA-F]{6})/)?.[1];
const encreClaire = css.match(
  /\[data-theme\$="-nuit"\] \.repere\s*\{[^}]*color:(#[0-9a-fA-F]{6})/,
)?.[1];
if (!encreSombre || !encreClaire) {
  echecs += 1;
  console.log('ECHEC nuancier des repères : encre de glyphe introuvable dans systeme.css (.repere { color:… })');
}
for (const [nom, groupe] of [['sombre', sombres], ['claire', claires]]) {
  if (Object.keys(groupe).length !== REPERE_FAMILLES) {
    echecs += 1;
    console.log(`ECHEC nuancier des repères : ${Object.keys(groupe).length} déclinaison(s) ${nom}(s) extraite(s) de systeme.css — ${REPERE_FAMILLES} attendues`);
  }
}
for (const [nom, t] of Object.entries(themes)) {
  const nuit = nom.endsWith('-nuit');
  const groupe = nuit ? claires : sombres;
  const glyphe = nuit ? encreClaire : encreSombre;
  if (!glyphe) continue;
  for (const [teinte, hex] of Object.entries(groupe)) {
    for (const fond of FONDS_REPERE) {
      if (!t[fond]) continue;
      mesures += 1;
      const r = rapport(hex, t[fond]);
      if (r < 3) {
        echecs += 1;
        console.log(`ECHEC ${nom} · repère ${teinte.padEnd(8)} sur ${fond.padEnd(8)} ${r.toFixed(2).padStart(5)}:1  (seuil 3:1)  nav, badge de liste`);
      }
    }
    mesures += 1;
    const g = rapport(glyphe, hex);
    if (g < 4.5) {
      echecs += 1;
      console.log(`ECHEC ${nom} · glyphe sur repère ${teinte.padEnd(8)} ${g.toFixed(2).padStart(5)}:1  (seuil 4.5:1)`);
    }
  }
}

console.log(echecs === 0
  ? `Tout passe — ${Object.keys(themes).length} thèmes, ${mesures} paires mesurées.`
  : `${echecs} paire(s) sous le seuil.`);
process.exitCode = echecs === 0 ? 0 : 1;
