// R2 (PLAN-RETOURS-8, A75) — le parcours de premier démarrage ne se
// joue qu'UNE fois. Les marques vivent en localStorage (V-D4 :
// préférence pure UI, le shell n'a rien à en lire — le patron de
// wind-theme / wind-volets). Deux clés : `fait` (Terminer cliqué, ou
// installation existante réputée accueillie) et `commence` (le
// parcours s'est affiché) — c'est elle qui distingue une mise à jour
// (comptes présents, jamais de parcours → réputée faite) d'un
// parcours ABANDONNÉ à mi-course (compte ajouté à l'étape 1, app
// quittée avant Terminer → il reprend au prochain lancement).
//
// Couture e2e `__e2eAccueil` (lecture défensive, patron __e2eLiens) :
// elle vit ICI, à la frontière de la persistance — jamais dans la
// décision produit d'App.svelte. Sous la couture, rien n'est « fait »
// et rien ne s'écrit : un décor semé rejoue le parcours entier sans
// polluer le profil.
const CLE_FAIT = 'wind-accueil-fait';
const CLE_COMMENCE = 'wind-accueil-commence';

const forceE2e = () => globalThis.window?.__e2eAccueil === true;

export function accueilFait() {
  if (forceE2e()) return false;
  try {
    return localStorage.getItem(CLE_FAIT) === '1';
  } catch {
    // Stockage indisponible : réputé fait — un parcours qui
    // reviendrait à CHAQUE lancement serait pire qu'un parcours
    // manqué (et `marquerAccueilFait` ne pourrait pas l'éteindre).
    return true;
  }
}

export function marquerAccueilFait() {
  if (forceE2e()) return;
  try {
    localStorage.setItem(CLE_FAIT, '1');
  } catch { /* stockage indisponible : accueilFait() répond déjà « fait » */ }
}

export function accueilCommence() {
  if (forceE2e()) return false;
  try {
    return localStorage.getItem(CLE_COMMENCE) === '1';
  } catch {
    return false;
  }
}

export function marquerAccueilCommence() {
  if (forceE2e()) return;
  try {
    localStorage.setItem(CLE_COMMENCE, '1');
  } catch { /* stockage indisponible : la reprise ne survivra pas, sans casse */ }
}
