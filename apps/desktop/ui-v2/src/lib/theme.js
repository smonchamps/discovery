// Thèmes Clarity — comportement du prototype, exactement : défaut
// `nature`, choix persisté sous localStorage['discovery-theme'],
// restauré au montage. D6 (livré à E2 des Réglages) : le suivi de l'OS
// sombre est un second booléen — quand il est actif et que l'OS est en
// sombre, « La nuit » s'AFFICHE sans toucher au thème choisi, qui
// revient tel quel dès que l'OS repasse en clair.

export const THEMES = ['air', 'feu', 'eau', 'astres', 'terre', 'nature', 'nuit'];
const CLE = 'discovery-theme';
const CLE_AUTO = 'discovery-theme-auto';

// Les fiches du dialogue Réglages — libellés, descriptions et pastilles
// VERBATIM du prototype (objet `themes` du template ; pastilles dans
// l'ordre accent, fond, panneau, surface, encre). Les mêmes valeurs
// vivent en jetons dans systeme.css : les pastilles doivent montrer
// chaque thème SANS l'appliquer, d'où les hex répétés ici.
export const FICHES = [
  { id: 'air', label: "L'air", desc: 'Ciels pâles et bleu clair, beaucoup de respiration.',
    pastilles: ['#3a7aa1', '#eef2f5', '#e3e9ee', '#ffffff', '#22303a'] },
  { id: 'feu', label: 'Le feu', desc: 'Sables chauds et braise, un accent orangé profond.',
    pastilles: ['#c0492b', '#f5efe8', '#eee3d9', '#fffaf5', '#2d201b'] },
  { id: 'eau', label: "L'eau", desc: "Verts d'eau et sarcelle, fond frais et minéral.",
    pastilles: ['#1f7d6d', '#e9f1f0', '#dce9e7', '#ffffff', '#16302d'] },
  { id: 'astres', label: 'Les astres', desc: 'Gris bleutés et indigo, une lumière nocturne claire.',
    pastilles: ['#5b53b5', '#eef0f6', '#e4e6f0', '#ffffff', '#22243a'] },
  { id: 'terre', label: 'La terre', desc: 'Argile, ocre et lin, un accent terracotta.',
    pastilles: ['#9c5a30', '#f1eee6', '#e8e2d6', '#fbf9f3', '#2b271e'] },
  { id: 'nature', label: 'La nature', desc: 'Vert pin sur ivoire — la direction actuelle.',
    pastilles: ['#2f6e5b', '#f0f2ef', '#eaece9', '#ffffff', '#232725'] },
  { id: 'nuit', label: 'La nuit', desc: 'Surfaces sombres, encre claire, pin adouci. Thème sombre.',
    pastilles: ['#57a88f', '#1b1e21', '#23272b', '#2b3034', '#edefed'] },
];

export function themeActuel() {
  let nom = 'nature';
  try { nom = localStorage.getItem(CLE) || 'nature'; } catch { /* stockage indisponible : défaut */ }
  return THEMES.includes(nom) ? nom : 'nature';
}

function osSombre() {
  return globalThis.matchMedia?.('(prefers-color-scheme: dark)').matches ?? false;
}

function poser(nom) {
  if (nom === 'nature') delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = nom;
}

// L'unique endroit qui décide du thème AFFICHÉ : le suivi de l'OS gagne
// quand il est actif et que l'OS est sombre, le choix persiste sinon.
function refleter() {
  poser(suiviOs() && osSombre() ? 'nuit' : themeActuel());
}

export function suiviOs() {
  try { return localStorage.getItem(CLE_AUTO) === '1'; } catch { return false; }
}

export function appliquerSuiviOs(actif) {
  try { localStorage.setItem(CLE_AUTO, actif ? '1' : '0'); } catch { /* le choix ne survivra pas, rien d'autre à faire */ }
  refleter();
}

export function appliquerTheme(nom) {
  if (!THEMES.includes(nom)) return;
  try { localStorage.setItem(CLE, nom); } catch { /* stockage indisponible : le choix ne survivra pas, rien d'autre à faire */ }
  refleter();
}

export function restaurerTheme() {
  refleter();
  // L'OS peut basculer en cours de session (mode nuit planifié) : le
  // reflet suit sans redémarrage.
  globalThis.matchMedia?.('(prefers-color-scheme: dark)')
    .addEventListener?.('change', refleter);
  return themeActuel();
}
