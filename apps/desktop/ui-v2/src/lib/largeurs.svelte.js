// Largeurs des volets de l'écran 02 (PLAN-RETOURS-V3 R3, verdict CE
// D3) : la nav et la liste se règlent à la souris, bornées — nav
// 180-400 px, liste 300-640 px —, persistées ensemble sous une seule
// clé. Préférence pure UI, le patron de volets.svelte.js : localStorage,
// restauration AVANT le premier rendu, valeur inconnue → défaut.
// `$state` partagé : la grille qui lit `largeurActuelle()` se re-rend
// au glissement, comme le nombre de volets.
//
// Revue (2026-08-16) : régler et persister sont DEUX gestes — le
// glissement règle à chaque pointermove (état seul, aucune écriture),
// le relâchement persiste une fois. Un `plafond` optionnel s'ajoute
// aux bornes : les bornes maximales cumulées (400 + 640) dépassent la
// fenêtre par défaut (1000 px) — sans plafond, le volet fil tombe à 0
// et la poignée sort de l'écran, état persisté irrécupérable. Le
// plafond vient de l'appelant : la fenêtre est une connaissance d'UI,
// pas de ce module.
const CLE = 'wind-largeurs';
export const DEFAUTS = { nav: 248, liste: 400 };
export const BORNES = { nav: [180, 400], liste: [300, 640] };

const etat = $state({ ...DEFAUTS });

function bornee(volet, px, plafond) {
  const [min, max] = BORNES[volet];
  return Math.min(Math.min(max, plafond), Math.max(min, Math.round(px)));
}

export function largeurActuelle(volet) {
  return etat[volet];
}

// Règle SANS persister — le pas du glissement.
export function reglerLargeur(volet, px, plafond = Infinity) {
  if (!(volet in DEFAUTS) || !Number.isFinite(px)) return;
  etat[volet] = bornee(volet, px, plafond);
}

// Écrit l'état courant — le relâchement, le clavier, le double-clic.
export function persisterLargeurs() {
  try {
    localStorage.setItem(CLE, JSON.stringify(etat));
  } catch { /* stockage indisponible : le réglage ne survivra pas, rien d'autre à faire */ }
}

// Règle ET persiste — le geste ponctuel (clavier, programmatique).
export function appliquerLargeur(volet, px, plafond = Infinity) {
  reglerLargeur(volet, px, plafond);
  persisterLargeurs();
}

// Le double-clic de la poignée (D3) : la frontière rend son défaut.
export function defautLargeur(volet) {
  if (!(volet in DEFAUTS)) return;
  etat[volet] = DEFAUTS[volet];
  persisterLargeurs();
}

// Restaure AVANT le premier rendu (pas de flash de grille) ; toute
// valeur absente, non numérique ou hors bornes retombe sur le défaut.
export function restaurerLargeurs() {
  let lu = {};
  try {
    lu = JSON.parse(localStorage.getItem(CLE) ?? '{}') ?? {};
  } catch { /* stockage ou JSON illisible : défauts */ }
  for (const volet of Object.keys(DEFAUTS)) {
    const px = lu[volet];
    etat[volet] =
      Number.isFinite(px) && px === bornee(volet, px, Infinity)
        ? px
        : DEFAUTS[volet];
  }
}
