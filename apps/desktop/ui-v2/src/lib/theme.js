// Thèmes Clarity « Wada élargi » (A42) : 14 thèmes clairs et leurs 14
// déclinaisons sombres (suffixe -nuit), défaut `nature`, choix persisté
// sous localStorage['wind-theme'], restauré au montage. D6 réécrit à
// A42 : quand le suivi de l'OS sombre est actif et que l'OS est en
// sombre, la déclinaison -nuit du thème CHOISI s'affiche — le choix
// persisté reste le thème de base, le suffixe est un état dérivé,
// jamais enregistré. Un thème -nuit choisi à la main reste en paix.

const CLE = 'wind-theme';
const CLE_AUTO = 'wind-theme-auto';

// Recopie des clés Discovery d'avant la bascule (PLAN-WIND E3) : le
// choix survit au renommage, les anciennes clés sont retirées. Le
// profil WebView2 est déménagé tel quel par l'application — les
// anciennes clés sont donc bien là au premier lancement Wind.
try {
  for (const [ancienne, neuve] of [['discovery-theme', CLE], ['discovery-theme-auto', CLE_AUTO]]) {
    const valeur = localStorage.getItem(ancienne);
    if (valeur !== null) {
      if (localStorage.getItem(neuve) === null) localStorage.setItem(neuve, valeur);
      localStorage.removeItem(ancienne);
    }
  }
  // A42 (D4 de PLAN-WADA-ELARGI) : « la nuit » est devenue
  // `nature-nuit` — le choix survit au renommage, comme ci-dessus ; il
  // se rejoue APRÈS la recopie, qui peut ramener un `nuit` Discovery.
  // Les cinq thèmes retirés (air, feu, eau, astres, terre) retombent
  // sur `nature` par le garde-fou de themeActuel(), silencieusement.
  if (localStorage.getItem(CLE) === 'nuit') localStorage.setItem(CLE, 'nature-nuit');
} catch { /* stockage indisponible : rien à migrer */ }

// Les fiches du dialogue Réglages — pastilles VERBATIM du paquet A42
// (dans l'ordre accent, fond, panneau, surface, encre). Les mêmes
// valeurs vivent en jetons dans systeme.css : les pastilles doivent
// montrer chaque thème SANS l'appliquer, d'où les hex répétés ici.
// Libellés et descriptions vivent au catalogue (`theme.<id>.nom` /
// `theme.<id>.desc` — PLAN-LANGUES, A15).
export const FICHES = [
  { id: 'nature', pastilles: ['#1e7566', '#f2f0ea', '#e9e6dd', '#ffffff', '#24272e'] },
  { id: 'hortensia', pastilles: ['#4d6fa8', '#eef1f6', '#e3e8f0', '#ffffff', '#232a38'] },
  { id: 'coquelicot', pastilles: ['#a8402e', '#f5f0ea', '#ece2d7', '#ffffff', '#2e211d'] },
  { id: 'chaume', pastilles: ['#6d5613', '#f5f2e3', '#ece7d2', '#ffffff', '#2a2410'] },
  { id: 'bruyere', pastilles: ['#8a4a63', '#f5f0f2', '#ece3e7', '#ffffff', '#2c2127'] },
  { id: 'source', pastilles: ['#2f6f7a', '#ecf2f3', '#dfe9ea', '#ffffff', '#1c2c30'] },
  { id: 'bleuet', pastilles: ['#3f5fae', '#eff1f7', '#e4e8f2', '#ffffff', '#222840'] },
  { id: 'lavande', pastilles: ['#58549c', '#f1f1f6', '#e7e6f0', '#ffffff', '#26243a'] },
  { id: 'rosee', pastilles: ['#2f7291', '#edf2f4', '#e0eaee', '#ffffff', '#1d2b33'] },
  { id: 'iris', pastilles: ['#4a4e8f', '#f5eee9', '#ebe0d6', '#ffffff', '#282536'] },
  { id: 'moisson', pastilles: ['#3e5f80', '#f5f0dd', '#ebe4cb', '#ffffff', '#282417'] },
  { id: 'grenade', pastilles: ['#963142', '#f6efe8', '#ece1d5', '#ffffff', '#2c2023'] },
  { id: 'safran', pastilles: ['#a55a12', '#f5f1e8', '#eae4d4', '#ffffff', '#2b2213'] },
  { id: 'estampe', pastilles: ['#b13a24', '#f1f1ef', '#e6e6e2', '#ffffff', '#28221f'] },
  { id: 'nature-nuit', pastilles: ['#57a88f', '#1b1e21', '#23272b', '#2b3034', '#edefed'] },
  { id: 'hortensia-nuit', pastilles: ['#869dc4', '#191e28', '#202632', '#2a303b', '#f1f3f7'] },
  { id: 'coquelicot-nuit', pastilles: ['#c47d71', '#211815', '#291e1a', '#332824', '#f7f2ed'] },
  { id: 'chaume-nuit', pastilles: ['#9c8c5f', '#1e1a0c', '#26200e', '#302a19', '#f7f4e7'] },
  { id: 'bruyere-nuit', pastilles: ['#af8495', '#20181c', '#281e23', '#32282d', '#f7f2f4'] },
  { id: 'source-nuit', pastilles: ['#729da5', '#142023', '#19282b', '#233235', '#eff4f5'] },
  { id: 'bleuet-nuit', pastilles: ['#7c92c8', '#181d2e', '#1f243a', '#292e43', '#f1f3f8'] },
  { id: 'lavande-nuit', pastilles: ['#8d8bbc', '#1b1a2a', '#222034', '#2c2a3d', '#f3f3f7'] },
  { id: 'rosee-nuit', pastilles: ['#729fb4', '#151f25', '#1a272e', '#243137', '#f0f4f6'] },
  { id: 'iris-nuit', pastilles: ['#8487b3', '#1d1b27', '#242131', '#2e2b3a', '#f7f1ec'] },
  { id: 'moisson-nuit', pastilles: ['#7c92a9', '#1d1a11', '#242015', '#2e2a20', '#f7f2e2'] },
  { id: 'grenade-nuit', pastilles: ['#b8737e', '#201719', '#281d20', '#32272a', '#f7f1eb'] },
  { id: 'safran-nuit', pastilles: ['#c28f5e', '#1f180e', '#271f11', '#31291c', '#f7f3eb'] },
  { id: 'estampe-nuit', pastilles: ['#ca796a', '#1d1816', '#241f1c', '#2e2926', '#f3f3f1'] },
];

// La liste des identifiants se DÉRIVE des fiches : une seule table à
// maintenir — une fiche sans thème (ou l'inverse) est impossible par
// construction, et la gate coherence-systeme.mjs tient les pastilles
// égales aux jetons livrés.
export const THEMES = FICHES.map((f) => f.id);

export function themeActuel() {
  let nom = 'nature';
  try { nom = localStorage.getItem(CLE) || 'nature'; } catch { /* stockage indisponible : défaut */ }
  return THEMES.includes(nom) ? nom : 'nature';
}

// Constat terrain A42 (2026-08-16, sondes au banc) : dans le WebView2
// de Tauri/wry, prefers-color-scheme ne suit PAS l'OS — jamais sombre,
// zéro événement, même sous une vraie bascule (registre + diffusion
// WM_SETTINGCHANGE). La source est l'API fenêtre Tauri (theme() +
// onThemeChanged), qui a tiré dans les deux sens à chaque bascule ;
// matchMedia reste le repli hors Tauri et la poignée du banc e2e
// (emulateMedia). Les DEUX canaux écrivent le même état et le dernier
// signal gagne — jamais un OU permanent : machine hôte en sombre, un
// OU rendrait emulateMedia('light') à jamais perdant (constaté au banc,
// D6 rouge à la première version du remède).
let sombreSignale = null;

function osSombre() {
  return sombreSignale
    ?? (globalThis.matchMedia?.('(prefers-color-scheme: dark)').matches ?? false);
}

function poser(nom) {
  if (nom === 'nature') delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = nom;
  // A44 : le `color-scheme` des barres natives ne se pose PAS ici —
  // il vit en CSS, à côté des jetons (`:root[data-theme$="-nuit"]`,
  // systeme.css) : tout chemin qui pose data-theme l'obtient, sans JS.
  // Revue A42 : la coche de Réglages suit la fiche AFFICHÉE — le
  // signal dit « le thème posé vient de changer », quel qu'en soit le
  // chemin (choix, bascule du suivi, événement OS en cours de session).
  document.dispatchEvent(new CustomEvent('wind:theme-affiche'));
}

// Le thème POSÉ sur <html> — l'état dérivé, jamais persisté (A42).
// C'est lui que la coche de Réglages désigne (revue A42) : sous suivi
// OS + OS sombre, l'œil voit la déclinaison -nuit, la coche aussi.
export function themeAffiche() {
  return document.documentElement.dataset.theme ?? 'nature';
}

// L'encre et le fond du thème affiché, lus aux jetons calculés — bakés
// par le cœur dans le document du corps de message (revue A42 : plus de
// dalle blanche #ffffff sur les 14 thèmes sombres). L'iframe sandbox ne
// voit jamais les jetons de l'hôte : les valeurs voyagent par l'appel.
export function paletteLecture() {
  const jetons = getComputedStyle(document.documentElement);
  return {
    encre: jetons.getPropertyValue('--ink').trim(),
    fond: jetons.getPropertyValue('--surface').trim(),
  };
}

// L'unique endroit qui décide du thème AFFICHÉ : quand le suivi de
// l'OS est actif et que l'OS est sombre, la déclinaison -nuit du thème
// choisi (A42). L'appartenance à THEMES est la garde unique : elle
// laisse en paix un thème déjà -nuit (« estampe-nuit-nuit » n'existe
// pas) et refuse de poser un attribut orphelin qu'aucun bloc CSS ne
// servirait — le choix s'affiche tel quel plutôt qu'en palette défaut.
function refleter() {
  const choisi = themeActuel();
  const nuit = `${choisi}-nuit`;
  poser(suiviOs() && osSombre() && THEMES.includes(nuit) ? nuit : choisi);
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
  // L'OS peut basculer en cours de session (mode nuit planifié) : le
  // reflet suit sans redémarrage, par le canal Tauri (le seul vivant
  // en production — constat terrain A42) ET par matchMedia (repli hors
  // Tauri, poignée e2e). Les rejets se taisent : hors Tauri la fenêtre
  // n'existe pas, le repli fait foi.
  const fenetre = globalThis.window?.__TAURI__?.window?.getCurrentWindow?.();
  if (fenetre) {
    fenetre.theme()
      .then((t) => {
        // L'état initial ne bat jamais un signal déjà arrivé.
        if (sombreSignale === null) {
          sombreSignale = t === 'dark';
          refleter();
        }
      })
      .catch(() => { /* thème illisible : le repli matchMedia fait foi */ });
    fenetre.onThemeChanged(({ payload }) => {
      sombreSignale = payload === 'dark';
      refleter();
    }).catch(() => { /* écoute refusée : le repli matchMedia fait foi */ });
  }
  refleter();
  globalThis.matchMedia?.('(prefers-color-scheme: dark)')
    .addEventListener?.('change', (e) => {
      sombreSignale = e.matches;
      refleter();
    });
  return themeActuel();
}
