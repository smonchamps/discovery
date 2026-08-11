// Reconstruire l'application avec ui-v2 embarquée — le point UNIQUE qui
// connaît les trois pièges MESURÉS du banc de la refonte. Chaque piège a
// coûté une session de fantômes ; ils vivent ici, une fois.
import { execSync } from 'node:child_process';
import { readFileSync, rmSync, utimesSync, writeFileSync } from 'node:fs';
import path from 'node:path';

export function construireV2(root, { release = true, fenetre = null } = {}) {
  execSync('npm run build', {
    cwd: path.join(root, 'apps', 'desktop', 'ui-v2'),
    stdio: 'inherit',
  });
  // 1. `generate_context!` n'embarque le dist qu'à la COMPILATION de
  //    main.rs : un changement d'assets SEULS ne recompile rien, et le
  //    binaire garderait un dist périmé (constaté : règle CSS présente
  //    sur disque, absente des feuilles chargées). Le bump de mtime
  //    force la ré-expansion.
  utimesSync(path.join(root, 'apps', 'desktop', 'src', 'main.rs'), new Date(), new Date());
  // 2. Un zombie de banc verrouille l'exe : le LINK échoue (« accès
  //    refusé ») et le vieux binaire serait rejoué en silence
  //    (constaté). On ne tue QUE les instances issues de target/ —
  //    jamais l'application installée de l'utilisateur.
  const balayage =
    "Get-Process discovery-desktop -ErrorAction SilentlyContinue | Where-Object { $_.Path -like '*\\target\\*' } | Stop-Process -Force";
  try {
    execSync(`powershell -NoProfile -Command "${balayage}"`, { stdio: 'ignore' });
  } catch {
    /* rien à tuer */
  }
  // 3. La config expédiée pointe sur `ui` (v1) : échangée le temps du
  //    build, RESTAURÉE même sur échec — le dépôt ne reste jamais sale.
  const conf = path.join(root, 'apps', 'desktop', 'tauri.conf.json');
  const origine = readFileSync(conf, 'utf8');
  const v2 = JSON.parse(origine);
  v2.build.frontendDist = 'ui-v2/dist';
  if (fenetre) Object.assign(v2.app.windows[0], fenetre);
  try {
    writeFileSync(conf, JSON.stringify(v2, null, 2));
    execSync(`cargo build -p discovery-desktop${release ? ' --release' : ''}`, {
      cwd: root,
      stdio: 'inherit',
    });
  } finally {
    writeFileSync(conf, origine);
  }
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
