// Banc P1 de la refonte (PLAN-UI-V2 §5) : les budgets sur la coquille
// v2 (Svelte, ligne continue à UN gabarit depuis A29), base seedée à
// l'échelle réelle — 256 312 messages par défaut.
//
//   node mesure-v2.mjs
//
//   MESURE_DB          chemin de la base (défaut : target/e2e/mesure-v2.db)
//   MESURE_COMPTES     « email:nombre » séparés par des virgules
//                      (défaut : mesure@exemple.fr:256312)
//   MESURE_REUTILISER  =1 pour garder la base en place
//
// La config Tauri expédiée pointe sur `ui` (v1). Le banc échange
// TEMPORAIREMENT `frontendDist` vers `ui-v2/dist`, compile, puis RESTAURE
// la config avant toute mesure — le dépôt ne reste jamais sale, même sur
// échec (finally). C'est le prix accepté en P1 pour ne pas toucher à
// l'UI expédiée ; si la boucle devient pénible, P2 en décidera autrement.
//
// Protocole (miroir du spike ADR 0015, mais vrai coeur + vrai IPC) :
// - démarrage : horloge murale, spawn -> première ligne visible ;
// - page : 300 sauts répartis sur la profondeur (LCG déterministe),
//   chaque saut attend le SERVICE (IPC) + le rendu + un reflow forcé ;
// - thème : 60 bascules à chaud sur les 7 thèmes ;
// - ouverture : 20 messages parmi les 400 plus récents (les corps seedés
//   couvrent les 500 plus récents) ;
// - RAM : working sets privés après 30 s (mesure-ram.ps1, ADR 0002).
import { spawn, execSync } from 'node:child_process';
import { existsSync, mkdirSync, rmSync } from 'node:fs';
import path from 'node:path';
import { chromium } from '@playwright/test';
import { construireV2, purgerCacheHttp } from './rebuild-v2.mjs';
import { purgerOAuth } from './isolation.mjs';

const root = path.resolve(import.meta.dirname, '..');

// --- 1. Construire ui-v2 puis le shell qui l'embarque ----------------
construireV2(root);

// --- 2. Base seedée à l'échelle -------------------------------------
const db = process.env.MESURE_DB || path.join(root, 'target', 'e2e', 'mesure-v2.db');
const comptes = (process.env.MESURE_COMPTES || 'mesure@exemple.fr:256312')
  .split(',')
  .map((entree) => {
    const [email, nombre] = entree.split(':');
    return { email: email.trim(), nombre: Number(nombre) };
  });

if (process.env.MESURE_REUTILISER && existsSync(db)) {
  console.log(`base réutilisée : ${db}`);
} else {
  rmSync(db, { force: true });
  mkdirSync(path.dirname(db), { recursive: true });
  for (const { email, nombre } of comptes) {
    execSync(
      `cargo run -p mail-core --example seed_inbox --release -- "${db}" ${nombre} ${email}`,
      { cwd: root, stdio: 'inherit' },
    );
  }
}

// --- 3. Lancer la vraie fenêtre, s'attacher par CDP -----------------
const profile = path.join(root, 'target', 'e2e', 'webview2-mesure-v2');
mkdirSync(profile, { recursive: true });
purgerCacheHttp(profile);

const env = {
  ...process.env,
  WIND_DB_PATH: db,
  WIND_E2E_ACCOUNT: comptes[0].email,
  WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=9222',
  WEBVIEW2_USER_DATA_FOLDER: profile,
};
purgerOAuth(env);

const t0 = performance.now();
const app = spawn(path.join(root, 'target', 'release', 'wind-desktop.exe'), [], {
  env,
  stdio: 'ignore',
});

let browser = null;
for (let attempt = 0; attempt < 300 && !browser; attempt++) {
  try {
    browser = await chromium.connectOverCDP('http://127.0.0.1:9222');
  } catch {
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
}
if (!browser) {
  app.kill();
  throw new Error(
    'CDP injoignable sur le port 9222 après 30 s. '
    + `Le processus est-il mort ? Une autre instance tourne-t-elle avec le profil ${profile} ?`,
  );
}

const stats = (valeurs) => {
  const tri = [...valeurs].sort((a, b) => a - b);
  const q = (p) => tri[Math.min(tri.length - 1, Math.floor(p * tri.length))];
  return `p50 ${q(0.5).toFixed(1)} ms · p95 ${q(0.95).toFixed(1)} ms · max ${tri[tri.length - 1].toFixed(1)} ms`;
};

try {
  // On attend la PAGE, pas le port (leçon de launch.mjs) : le CDP répond
  // avant que la fenêtre n'ait créé son document — chercher la page une
  // seule fois est une course, perdue dès que le démarrage est froid.
  let page = null;
  for (let attempt = 0; attempt < 300 && !page; attempt++) {
    page = browser
      .contexts()
      .flatMap((context) => context.pages())
      .find((candidate) => candidate.url().includes('tauri.localhost'));
    if (!page) await new Promise((resolve) => setTimeout(resolve, 100));
  }
  if (!page) throw new Error('fenêtre Tauri introuvable après 30 s — le processus a-t-il démarré ?');
  await page.locator('[data-testid="ligne"]').first().waitFor({ timeout: 60000 });
  const demarrage = performance.now() - t0;
  await page.waitForFunction(() => document.getElementById('perf').dataset.startup);
  console.log(`démarrage  : ${demarrage.toFixed(0)} ms (spawn -> première ligne, horloge murale)`);
  console.log('interne    :', await page.locator('#perf').textContent());

  const etat = await page.evaluate(() => window.__mesure.etat());
  console.log(`décor      : ${etat.total} lignes · gabarit ${etat.h1} px`);

  // MESURE_SANS_ACTIVITE=1 : peser la RAM AU REPOS, méthodologie ADR
  // 0002 — la même posture que le banc v1, sans quoi la comparaison
  // pèse un marathonien contre un dormeur.
  const auRepos = process.env.MESURE_SANS_ACTIVITE === '1';

  if (!auRepos) {
  // Pages : 300 sauts répartis sur la profondeur, LCG déterministe.
  const pages_ms = await page.evaluate(async () => {
    let graine = 42;
    const alea = () => ((graine = (graine * 1103515245 + 12345) % 2147483648) / 2147483648);
    const total = window.__mesure.etat().total;
    const mesures = [];
    for (let n = 0; n < 300; n++) {
      const index = Math.floor(alea() * Math.max(1, total - 40));
      mesures.push(await window.__mesure.page(index));
    }
    return mesures;
  });
  console.log(`page       : ${stats(pages_ms)} (service IPC + rendu + reflow)`);

  // Thème : 60 bascules à chaud sur les 7 thèmes.
  const themes_ms = await page.evaluate(() => {
    const noms = window.__mesure.themes;
    const mesures = [];
    for (let n = 0; n < 60; n++) mesures.push(window.__mesure.theme(noms[n % noms.length]));
    window.__mesure.theme('nature');
    return mesures;
  });
  console.log(`thème      : ${stats(themes_ms)}`);

  // Ouverture : 20 messages parmi les 400 plus récents (corps seedés).
  const ouvertures_ms = await page.evaluate(async () => {
    const mesures = [];
    for (let n = 0; n < 20; n++) mesures.push(await window.__mesure.ouvrir(n * 20));
    return mesures;
  });
  console.log(`ouverture  : ${stats(ouvertures_ms)}`);
  }

  console.log(`stabilisation 30 s avant la mesure RAM${auRepos ? ' (repos, aucune activité)' : ''}…`);
  await new Promise((resolve) => setTimeout(resolve, 30000));
  const ram = execSync(
    `powershell -NoProfile -ExecutionPolicy Bypass -File "${path.join(import.meta.dirname, 'mesure-ram.ps1')}"`
    + ` -AppPid ${app.pid} -Profil "${profile}"`,
  ).toString();
  console.log('RAM        :', ram.trim());
} finally {
  if (browser) await browser.close();
  app.kill();
}
