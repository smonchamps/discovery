// R1 (PLAN-RETOURS-8, A74) — le repère de compte : le jeu d'icônes
// DÉDIÉ (D2, réservé aux comptes — A3 « une icône, un sens » tenu par
// réservation) et les familles du nuancier mesuré (D1). Les hex vivent
// dans systeme.css (.repere[data-teinte]) et sont mesurés par
// e2e/contraste.mjs ; ici, seulement les NOMS — l'allowlist jumelle vit
// côté Rust (commands.rs, repere_normalise) et fait foi à l'écriture.
export const REPERE_ICONES = [
  'home',
  'work',
  'school',
  'star',
  'favorite',
  'flight',
  'shopping_bag',
  'account_balance',
  'sports_esports',
  'eco',
  'pets',
  'music_note',
];

export const REPERE_TEINTES = [
  'rouge',
  'orange',
  'ocre',
  'olive',
  'vert',
  'sapin',
  'bleu',
  'indigo',
  'violet',
  'magenta',
  'rose',
  'brun',
];
