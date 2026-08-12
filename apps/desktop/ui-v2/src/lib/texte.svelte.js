// La langue de l'interface (PLAN-LANGUES, A15) : catalogues plats
// fr/en, bascule immédiate — la langue est un `$state`, tout gabarit
// qui passe par `t()` se re-rend au changement, comme le thème change
// à chaud. Préférence d'APPLICATION persistée en base (`prefs.lang`,
// le patron des bulles) : le shell Rust la lit pour composer les
// notifications — localStorage lui serait invisible.
//
// Repli : toute clé absente du catalogue actif retombe sur le français
// (la référence, mot pour mot du prototype) — jamais un trou à
// l'écran ; l'audit e2e garantit que les jeux de clés sont identiques.
import { appel } from './transport.js';
import { FR } from './catalogue.fr.js';
import { EN } from './catalogue.en.js';

export const CATALOGUES = { fr: FR, en: EN };
export const LANGUES = ['fr', 'en'];

// La règle du pluriel, par langue : « 0 élément » mais "0 items" — le
// strict besoin du dépôt, pas un moteur CLDR.
const PLURIEL = {
  fr: (n) => n > 1,
  en: (n) => n !== 1,
};

const etat = $state({ langue: 'fr' });

export function langueActuelle() {
  return etat.langue;
}

// Applique SANS persister — l'appelant pose la préférence (Réglages,
// via `lang_set`), comme l'interrupteur des bulles. `<html lang>` suit
// (lecteurs d'écran, correcteurs).
export function appliquerLangue(code) {
  const langue = CATALOGUES[code] ? code : 'fr';
  etat.langue = langue;
  document.documentElement.lang = langue;
}

// Restaure AVANT le premier rendu (pas de flash) : la préférence en
// base d'abord ; au premier lancement, la langue du système si elle
// est couverte, sinon `fr` — et la clé se pose aussitôt, pour que le
// shell la voie sans attendre un passage par les Réglages.
export async function restaurerLangue() {
  let code = null;
  try {
    code = await appel('lang_get');
  } catch { /* hors Tauri : la détection ci-dessous décide */ }
  if (!code) {
    const systeme = (globalThis.navigator?.language ?? 'fr').toLowerCase();
    code = systeme.startsWith('en') ? 'en' : 'fr';
    appel('lang_set', { lang: code }).catch(() => { /* hors Tauri : rien à poser */ });
  }
  appliquerLangue(code);
}

// `t(cle, params)` : gabarits `{nom}` ; une barre `|` sépare le
// singulier du pluriel, tranché par `params.n` selon la règle de la
// langue. Les valeurs non-chaîne (tables de dates) rendent telles
// quelles.
export function t(cle, params) {
  const catalogue = CATALOGUES[etat.langue] ?? FR;
  let valeur = catalogue[cle];
  if (valeur === undefined) {
    if (import.meta.env?.DEV) {
      console.warn(`clé absente du catalogue ${etat.langue} : ${cle}`);
    }
    valeur = FR[cle];
  }
  if (valeur === undefined) return cle;
  if (typeof valeur !== 'string') return valeur;
  if (valeur.includes('|') && params && typeof params.n === 'number') {
    const [singulier, pluriel] = valeur.split('|');
    valeur = PLURIEL[etat.langue]?.(params.n) ? pluriel : singulier;
  }
  if (!params) return valeur;
  return valeur.replace(/\{(\w+)\}/g, (_, nom) => String(params[nom] ?? ''));
}
