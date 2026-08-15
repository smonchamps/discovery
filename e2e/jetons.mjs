// Lecture des jetons de systeme.css — le parseur UNIQUE des deux gates
// (contraste.mjs, coherence-systeme.mjs). Né à la revue d'A42 : chacune
// portait sa copie de la même regex, et le même bogue s'y est corrigé
// deux fois de suite (la classe [a-zA-Z] qui perdait « ink2 » à
// PLAN-DC E3, puis [a-z]+ qui laissait les 14 thèmes -nuit sans AUCUNE
// mesure). Une seule implémentation, un seul endroit à corriger.
//
// NOMBRE_ATTENDU est le plancher de complétude : un bloc que le motif
// ne reconnaît pas n'est pas une paire à sauter, c'est la gate qui
// devient aveugle en silence. À mettre à jour avec la table, dans le
// même commit (DC-D2) — l'écart est bruyant, c'est le but.
export const NOMBRE_ATTENDU = 28;

// [a-z-]+ : les déclinaisons sombres portent un trait d'union
// (« nature-nuit », A42). [a-zA-Z][a-zA-Z0-9]* côté jeton : « ink2 »
// porte un chiffre. `motifValeur` : contraste.mjs n'extrait que les
// hex #RRGGBB (seuls mesurables), coherence-systeme.mjs prend toute
// valeur (ombres, scrims) — le défaut.
export function lireThemes(css, { motifValeur = /[^;]+/ } = {}) {
  const themes = {};
  for (const [, nom, corps] of css.matchAll(
    /:root(?:\[data-theme="([a-z-]+)"\])?\s*\{([^}]+)\}/g,
  )) {
    const jetons = {};
    for (const [, cle, valeur] of corps.matchAll(
      new RegExp(`--([a-zA-Z][a-zA-Z0-9]*)\\s*:\\s*(${motifValeur.source})`, 'g'),
    )) {
      jetons[cle] = valeur.replace(/\s+/g, ' ').trim();
    }
    if (Object.keys(jetons).length > 0) themes[nom ?? 'nature'] = jetons;
  }
  return themes;
}
