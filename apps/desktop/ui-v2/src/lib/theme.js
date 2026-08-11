// Thèmes Clarity — comportement du prototype, exactement : défaut
// `nature`, choix persisté sous localStorage['discovery-theme'],
// restauré au montage. (L'OS sombre automatique est en D6, après
// bascule.)

export const THEMES = ['air', 'feu', 'eau', 'astres', 'terre', 'nature', 'nuit'];
const CLE = 'discovery-theme';

// Les fiches du dialogue Réglages — libellés, descriptions et pastilles
// VERBATIM du prototype (objet `themes` du template ; pastilles dans
// l'ordre accent, fond, panneau, surface, encre). Les mêmes valeurs
// vivent en jetons dans systeme.css : les pastilles doivent montrer
// chaque thème SANS l'appliquer, d'où les hex répétés ici.
export const FICHES = [
  { id: 'air', label: "L'air", desc: 'Ciels pâles et bleu clair, beaucoup de respiration.',
    pastilles: ['#3f86b0', '#eef2f5', '#e3e9ee', '#ffffff', '#22303a'] },
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
