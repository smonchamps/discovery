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
// base d'abord — `lang_get` est une sonde en lecture seule, seule
// commande permise avant `migration_check` (ADR 0012) ; au premier
// lancement, la langue du système si elle est couverte, sinon `fr`.
// La clé détectée ne se pose PAS ici : `lang_set` ouvre la base et
// paierait l'adoption d'une base héritée en silence, sans modale
// (terrain 2026-08-15) — elle se pose par `poserLangueDetectee()`,
// que l'App appelle une fois la migration assurée.
let aPoser = null;

export async function restaurerLangue() {
  let code = null;
  let repondu = false;
  try {
    code = await appel('lang_get');
    repondu = true;
  } catch { /* hors Tauri ou base illisible : repli de session ci-dessous */ }
  if (!code) {
    const systeme = (globalThis.navigator?.language ?? 'fr').toLowerCase();
    code = systeme.startsWith('en') ? 'en' : 'fr';
    // La détection ne s'arme À POSER que si la base a VRAIMENT répondu
    // « aucune préférence ». Un échec de lecture n'est pas une absence
    // (revue 2026-08-15) : poser après coup écraserait une préférence
    // existante que la sonde n'a simplement pas pu lire.
    if (repondu) aPoser = code;
  }
  appliquerLangue(code);
}

// La pose différée du premier lancement : APRÈS la modale de migration,
// pour que le shell voie la clé sans attendre un passage par les
// Réglages — et sans jamais toucher une base pas encore adoptée.
// Rend la promesse : l'App l'attend pour que la création de schéma du
// premier lancement reste SÉRIALISÉE avant la flotte des sondes.
export function poserLangueDetectee() {
  if (!aPoser) return Promise.resolve();
  return appel('lang_set', { lang: aPoser })
    .then(() => { aPoser = null; })
    .catch(() => { /* hors Tauri ou échec d'écriture : la clé restera
      absente, la détection se rejouera au prochain lancement */ });
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
