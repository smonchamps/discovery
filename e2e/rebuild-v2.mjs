// Reconstruire l'application — le point UNIQUE qui connaît les trois
// pièges MESURÉS du banc de la refonte. Chaque piège a coûté une
// session de fantômes ; ils vivent ici, une fois.
//
// Depuis B2 (PLAN-RETRAIT-V1), ui-v2 est la SEULE interface : plus
// aucun échange de dist — il ne reste que la taille de fenêtre du banc
// de parité.
import { execSync } from 'node:child_process';
import { readFileSync, rmSync, utimesSync, writeFileSync } from 'node:fs';
import path from 'node:path';

function construire(root, { release, fenetre }) {
  // 1. `generate_context!` n'embarque le dist qu'à la COMPILATION de
  //    main.rs : un changement d'assets SEULS ne recompile rien, et le
  //    binaire garderait un dist périmé (constaté : règle CSS présente
  //    sur disque, absente des feuilles chargées). Le bump de mtime
  //    force la ré-expansion.
  utimesSync(path.join(root, 'apps', 'desktop', 'src', 'main.rs'), new Date(), new Date());
  // 2. Un zombie de banc verrouille l'exe : le LINK échoue (« accès
  //    refusé ») et le vieux binaire serait rejoué en silence
  //    (constaté). On ne tue QUE les instances issues de CE target/ —
  //    jamais l'application installée de l'utilisateur, et jamais la
  //    suite d'un AUTRE worktree : le motif '*\target\*' d'origine
  //    abattait l'application de l'autre suite en plein vol
  //    (Stop-Process -Force = code 0xFFFFFFFF sans sortie — le constat
  //    du 2026-08-15, PLAN-ISOLATION-E2E).
  const balayage =
    'Get-Process wind-desktop -ErrorAction SilentlyContinue | '
    + `Where-Object { $_.Path -like '${path.join(root, 'target')}\\*' } | Stop-Process -Force`;
  try {
    execSync(`powershell -NoProfile -Command "${balayage}"`, { stdio: 'ignore' });
  } catch {
    /* rien à tuer */
  }
  // 3. Échange de conf éventuel (taille de fenêtre du banc de parité) :
  //    RESTAURÉ même sur échec — le dépôt ne reste jamais sale.
  const conf = path.join(root, 'apps', 'desktop', 'tauri.conf.json');
  const commande = `cargo build -p wind-desktop${release ? ' --release' : ''}`;
  if (!fenetre) {
    execSync(commande, { cwd: root, stdio: 'inherit' });
    return;
  }
  const origine = readFileSync(conf, 'utf8');
  const modifiee = JSON.parse(origine);
  Object.assign(modifiee.app.windows[0], fenetre);
  try {
    writeFileSync(conf, JSON.stringify(modifiee, null, 2));
    execSync(commande, { cwd: root, stdio: 'inherit' });
  } finally {
    writeFileSync(conf, origine);
  }
}

export function construireV2(root, { release = true, fenetre = null } = {}) {
  execSync('npm run build', {
    cwd: path.join(root, 'apps', 'desktop', 'ui-v2'),
    stdio: 'inherit',
  });
  construire(root, { release, fenetre });
}

// Le cache HTTP du profil WebView2 survit aux rebuilds et peut servir un
// index.html périmé avec ses vieux assets hashés — styles fantômes, CSP
// d'une autre époque (constaté). On purge le CACHE, pas le profil : le
// cache GPU qui rend les démarrages tièdes reste.
export function purgerCacheHttp(profile) {
  for (const dossier of ['Cache', 'Code Cache']) {
    rmSync(path.join(profile, 'EBWebView', 'Default', dossier), {
      recursive: true,
      force: true,
    });
  }
}
