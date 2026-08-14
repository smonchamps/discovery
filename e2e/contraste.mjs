// Banc de contraste WCAG des jetons Clarity (systeme.css) : chaque
// paire (encre, fond) réellement posée par ui-v2, dans les 7 thèmes.
// Seuils : 4,5:1 pour le texte courant, 3:1 pour le grand texte et les
// composants d'interface (icônes, bordures porteuses de sens).
//
//   node contraste.mjs        -> tableau complet + verdict
//
// La source est systeme.css (les jetons expédiés), pas le prototype :
// c'est ce que l'utilisateur voit qui se mesure.
import { readFileSync } from 'node:fs';
import path from 'node:path';

const root = path.resolve(import.meta.dirname, '..');
const css = readFileSync(
  path.join(root, 'apps', 'desktop', 'ui-v2', 'src', 'systeme.css'),
  'utf8',
);

// --- Extraction des jetons par thème ---------------------------------
const themes = {};
const blocs = [...css.matchAll(/:root(?:\[data-theme="([a-z]+)"\])?\s*\{([^}]+)\}/g)];
for (const [, nom, corps] of blocs) {
  const jetons = {};
  // [a-zA-Z0-9] : « ink2 » porte un chiffre — l'ancienne classe [a-zA-Z]
  // le laissait tomber, et le `continue` d'en bas taisait les 21 paires
  // ink2 jamais mesurées (bogue trouvé à PLAN-DC E3).
  for (const [, cle, valeur] of corps.matchAll(/--([a-zA-Z][a-zA-Z0-9]*)\s*:\s*(#[0-9a-fA-F]{6})/g)) {
    jetons[cle] = valeur;
  }
  if (Object.keys(jetons).length > 0) themes[nom ?? 'nature'] = jetons;
}

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
  ['ink', 'sel', 4.5, 'rangée survolée/sélectionnée'],
  ['ink2', 'bg', 4.5, 'expéditeurs, corps de texte'],
  ['ink2', 'surface', 4.5, 'puces, boutons secondaires'],
  ['ink2', 'panel', 4.5, 'nav (libellés)'],
  ['muted', 'bg', 4.5, 'heures, aperçus, statut'],
  ['muted', 'surface', 4.5, 'kickers, descriptions, placeholder'],
  ['muted', 'panel', 4.5, 'statut, sections nav'],
  ['onAccent', 'accent', 4.5, 'boutons principaux'],
  ['alert', 'bg', 4.5, "texte d'erreur (onboarding), mention Brouillon (rangée)"],
  ['alert', 'sel', 4.5, 'mention Brouillon (rangée survolée)'],
  ['alert', 'surface', 4.5, 'mention Brouillon (rangée choisie)'],
  ['alert', 'surface', 3, "icône/bordure d'alerte (fente)"],
  ['accent', 'surface', 3, 'icônes signature, coche, héros non-lu'],
  ['accent', 'panel', 3, 'compteur héros (nav)'],
];

let echecs = 0;
for (const [nom, t] of Object.entries(themes)) {
  console.log(`\n=== ${nom} ===`);
  for (const [encre, fond, seuil, ou] of PAIRES) {
    if (!t[encre] || !t[fond]) {
      // Un jeton introuvable n'est pas une paire à sauter : c'est le
      // banc qui ment. Bruyant, comme tout échec.
      echecs += 1;
      console.log(`ECHEC ${encre.padEnd(9)} sur ${fond.padEnd(8)} jeton introuvable dans systeme.css  ${ou}`);
      continue;
    }
    const r = rapport(t[encre], t[fond]);
    const ok = r >= seuil;
    if (!ok) echecs += 1;
    console.log(
      `${ok ? '  ok  ' : 'ECHEC '}${encre.padEnd(9)} sur ${fond.padEnd(8)} `
      + `${r.toFixed(2).padStart(5)}:1  (seuil ${seuil}:1)  ${ou}`,
    );
  }
}
console.log(`\n${echecs === 0 ? 'Tout passe.' : `${echecs} paire(s) sous le seuil.`}`);
process.exitCode = echecs === 0 ? 0 : 1;
