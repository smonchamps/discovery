// Thèmes Clarity — comportement du prototype, exactement : défaut
// `nature`, choix persisté sous localStorage['discovery-theme'],
// restauré au montage. (L'OS sombre automatique est en D6, après
// bascule.)

export const THEMES = ['air', 'feu', 'eau', 'astres', 'terre', 'nature', 'nuit'];
const CLE = 'discovery-theme';

export function appliquerTheme(nom) {
  if (!THEMES.includes(nom)) return;
  if (nom === 'nature') delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = nom;
  try { localStorage.setItem(CLE, nom); } catch { /* stockage indisponible : le choix ne survivra pas, rien d'autre à faire */ }
}

export function restaurerTheme() {
  let nom = 'nature';
  try { nom = localStorage.getItem(CLE) || 'nature'; } catch { /* idem */ }
  if (!THEMES.includes(nom)) nom = 'nature';
  if (nom !== 'nature') document.documentElement.dataset.theme = nom;
  return nom;
}
