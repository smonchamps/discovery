// Le nombre de volets de l'écran 02 (PLAN-VOLETS, V-D1/V-D4) : 3 —
// nav, liste, lecture (défaut) — ou 2 — nav + liste pleine largeur, la
// lecture s'ouvre en plein écran (écran 03). Le mode 1 (liste seule,
// nav en tiroir) arrive en E2 du plan. Préférence pure UI : le shell
// n'a rien à en lire — localStorage, le patron du thème (D6), jamais
// la base. `$state` partagé : tout gabarit qui lit `voletsActuels()`
// se re-rend à la bascule, comme la langue.
const CLE = 'wind-volets';
const VALEURS = [3, 2];

const etat = $state({ volets: 3 });

export function voletsActuels() {
  return etat.volets;
}

// Applique ET persiste — le geste du thème : immédiat, sans
// confirmation. Toute valeur inconnue est refusée (le patron
// `appliquerTheme`).
export function appliquerVolets(n) {
  if (!VALEURS.includes(n)) return;
  etat.volets = n;
  try {
    localStorage.setItem(CLE, String(n));
  } catch { /* stockage indisponible : le choix ne survivra pas, rien d'autre à faire */ }
}

// Restaure AVANT le premier rendu (pas de flash de grille) ; toute
// valeur inconnue retombe sur 3 (le patron `themeActuel`).
export function restaurerVolets() {
  let n = 3;
  try {
    n = Number(localStorage.getItem(CLE) ?? '3');
  } catch { /* stockage indisponible : défaut */ }
  etat.volets = VALEURS.includes(n) ? n : 3;
}
