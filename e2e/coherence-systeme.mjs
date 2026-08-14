// Gate de cohérence du Système (PLAN-DC E3, décision DC-D6) : le
// document normatif (docs/design/systeme.dc.html) ne doit jamais
// dériver des valeurs livrées (apps/desktop/ui-v2/src/systeme.css).
//
//   node coherence-systeme.mjs   -> écarts nommés + verdict
//
// Trois vérifications :
//   1. La table du contrat des jetons (cellules data-theme/data-jeton)
//      égale les :root de systeme.css, VALEUR POUR VALEUR, dans les
//      deux sens — un jeton du CSS absent du doc est un échec autant
//      qu'une valeur fausse, et une cellule orpheline (jeton mort au
//      CSS) autant qu'une cellule manquante.
//   2. Aucun compteur de glyphes (« N glyphes ») dans le corps du doc —
//      le contrat est assets/icones/README.md, seul compteur qui fait
//      foi (DC-D3). Le journal des amendements, archive de faits datés,
//      est seul exempté.
//   3. Le renvoi au contrat des icônes est bien présent.
//
// Le remède est toujours le même : amender le Système dans le commit
// fautif (DC-D2) — jamais tordre la gate.
import { readFileSync } from 'node:fs';
import path from 'node:path';

const root = path.resolve(import.meta.dirname, '..');
const css = readFileSync(
  path.join(root, 'apps', 'desktop', 'ui-v2', 'src', 'systeme.css'),
  'utf8',
);
const doc = readFileSync(
  path.join(root, 'docs', 'design', 'systeme.dc.html'),
  'utf8',
);

let echecs = 0;
const echec = (message) => {
  echecs += 1;
  console.log(`ECHEC ${message}`);
};

// --- 1. Les jetons : systeme.css d'un côté… --------------------------
// La classe [a-zA-Z][a-zA-Z0-9]* : « ink2 » porte un chiffre (le bogue
// du banc de contraste, corrigé au même commit que cette gate).
const themesCss = {};
for (const [, nom, corps] of css.matchAll(
  /:root(?:\[data-theme="([a-z]+)"\])?\s*\{([^}]+)\}/g,
)) {
  const jetons = {};
  for (const [, cle, valeur] of corps.matchAll(
    /--([a-zA-Z][a-zA-Z0-9]*)\s*:\s*([^;]+);/g,
  )) {
    jetons[cle] = valeur.replace(/\s+/g, ' ').trim();
  }
  if (Object.keys(jetons).length > 0) themesCss[nom ?? 'nature'] = jetons;
}

// --- …la table du contrat du doc de l'autre --------------------------
const themesDoc = {};
for (const [, theme, jeton, contenu] of doc.matchAll(
  /<td data-theme="([a-z]+)" data-jeton="([a-zA-Z0-9]+)"[^>]*>([^<]*)<\/td>/g,
)) {
  (themesDoc[theme] ??= {})[jeton] = contenu.replace(/\s+/g, ' ').trim();
}

if (Object.keys(themesDoc).length === 0) {
  echec('la table du contrat des jetons est introuvable dans le doc (cellules data-theme/data-jeton)');
}

for (const [nom, jetonsCss] of Object.entries(themesCss)) {
  const jetonsDoc = themesDoc[nom] ?? {};
  for (const [jeton, valeurCss] of Object.entries(jetonsCss)) {
    if (!(jeton in jetonsDoc)) {
      echec(`${nom} · --${jeton} : livré « ${valeurCss} », absent du doc`);
    } else if (jetonsDoc[jeton] !== valeurCss) {
      echec(`${nom} · --${jeton} : doc « ${jetonsDoc[jeton]} », livré « ${valeurCss} »`);
    }
  }
  for (const jeton of Object.keys(jetonsDoc)) {
    if (!(jeton in jetonsCss)) {
      echec(`${nom} · --${jeton} : au doc mais mort dans systeme.css`);
    }
  }
}
for (const nom of Object.keys(themesDoc)) {
  if (!(nom in themesCss)) {
    echec(`thème « ${nom} » : au doc mais absent de systeme.css (thème fantôme — DC-D5)`);
  }
}

// --- 2. Aucun compteur de glyphes hors du journal --------------------
const debutJournal = doc.indexOf('Journal des amendements');
const corpsDoc = debutJournal === -1 ? doc : doc.slice(0, debutJournal);
if (debutJournal === -1) {
  echec('la section « Journal des amendements » est introuvable');
}
for (const [motif] of corpsDoc.matchAll(/\b\d+\s*(?:<[^>]+>\s*)*glyphes/g)) {
  echec(`compteur de glyphes dans le corps du doc (« ${motif.replace(/\s+/g, ' ')} ») — le contrat est assets/icones/README.md (DC-D3)`);
}

// --- 3. Le renvoi au contrat des icônes ------------------------------
if (!corpsDoc.includes('assets/icones/README.md')) {
  echec('le renvoi au contrat assets/icones/README.md manque dans le corps du doc (DC-D3)');
}

const nbThemes = Object.keys(themesCss).length;
const nbJetons = Object.values(themesCss).reduce((n, t) => n + Object.keys(t).length, 0);
console.log(
  echecs === 0
    ? `Tout concorde — ${nbThemes} thèmes, ${nbJetons} valeurs de jetons, le doc dit le livré.`
    : `${echecs} écart(s) entre le Système et le livré. Le remède : amender le Système dans le commit fautif (DC-D2).`,
);
process.exitCode = echecs === 0 ? 0 : 1;
