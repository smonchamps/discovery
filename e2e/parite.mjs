// Banc de parité visuelle Clarity (PLAN-UI-V2 §4) : v2 sur le décor du
// prototype (seed_clarity) d'un côté, le prototype lui-même
// (docs/design/ui_prototype.html, joué dans Edge) de l'autre — mêmes
// états, même viewport 1440×900, même densité de pixels. Les paires de
// captures atterrissent dans target/parite/ ; la parité se JUGE au
// terrain, ce banc l'outille.
//
//   node parite.mjs
//
// États capturés : réception avec le fil Vantis ouvert, onglet Non lus,
// thème « La nuit ». (Conversation, composition et réglages viendront
// avec P3/P4.)
import { spawn, execSync } from 'node:child_process';
import { mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { chromium } from '@playwright/test';

const root = path.resolve(import.meta.dirname, '..');
const conf = path.join(root, 'apps', 'desktop', 'tauri.conf.json');
const sortie = path.join(root, 'target', 'parite');
mkdirSync(sortie, { recursive: true });

// --- 1. Le prototype, joué dans Edge (aucun réseau : fichier local) --
{
  const navigateur = await chromium.launch({ channel: 'msedge', headless: true });
  const contexte = await navigateur.newContext({
    viewport: { width: 1440, height: 900 },
    deviceScaleFactor: 1.5,
  });
  const page = await contexte.newPage();
  await page.goto('file://' + path.join(root, 'docs', 'design', 'ui_prototype.html').replaceAll('\\', '/'));
  await page.locator('button', { hasText: 'Continuer' }).waitFor({ timeout: 60000 });
  await page.locator('button', { hasText: 'Continuer' }).click();
  await page.locator('text=Boîte de réception').first().waitFor();
  await page.waitForTimeout(400);
  await page.screenshot({ path: path.join(sortie, 'proto-reception.png') });
  await page.locator('span', { hasText: 'Non lus' }).last().click();
  await page.waitForTimeout(300);
  await page.screenshot({ path: path.join(sortie, 'proto-nonlus.png') });
  await page.locator('span', { hasText: /^Tous$/ }).last().click();
  await page.locator('button', { hasText: 'Réglages' }).click();
  await page.locator('span', { hasText: 'La nuit' }).first().click();
  await page.locator('button', { hasText: 'Terminé' }).click();
  await page.waitForTimeout(300);
  await page.screenshot({ path: path.join(sortie, 'proto-nuit.png') });
  await navigateur.close();
  console.log('prototype capturé (3 états)');
}

// --- 2. v2 sur le décor Clarity, fenêtre aux mêmes dimensions -------
execSync('npm run build', { cwd: path.join(root, 'apps', 'desktop', 'ui-v2'), stdio: 'inherit' });
const confOrigine = readFileSync(conf, 'utf8');
const confV2 = JSON.parse(confOrigine);
confV2.build.frontendDist = 'ui-v2/dist';
confV2.app.windows[0].width = 1440;
confV2.app.windows[0].height = 900;
try {
  writeFileSync(conf, JSON.stringify(confV2, null, 2));
  execSync('cargo build -p discovery-desktop --release', { cwd: root, stdio: 'inherit' });
} finally {
  writeFileSync(conf, confOrigine);
}

const db = path.join(root, 'target', 'e2e', 'clarity.db');
rmSync(db, { force: true });
execSync(`cargo run -p mail-core --example seed_clarity --release -- "${db}"`, {
  cwd: root,
  stdio: 'inherit',
});

const profile = path.join(root, 'target', 'e2e', 'webview2-parite');
mkdirSync(profile, { recursive: true });
const env = {
  ...process.env,
  DISCOVERY_DB_PATH: db,
  DISCOVERY_E2E_ACCOUNT: 'paul.merand@atelier-nord.fr,paul@merand.fr',
  WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=9222',
  WEBVIEW2_USER_DATA_FOLDER: profile,
};
delete env.GOOGLE_CLIENT_ID;
delete env.GOOGLE_CLIENT_SECRET;

const app = spawn(path.join(root, 'target', 'release', 'discovery-desktop.exe'), [], {
  env,
  stdio: 'ignore',
});
let browser = null;
for (let n = 0; n < 300 && !browser; n++) {
  try { browser = await chromium.connectOverCDP('http://127.0.0.1:9222'); }
  catch { await new Promise((r) => setTimeout(r, 100)); }
}
if (!browser) {
  app.kill();
  throw new Error('CDP injoignable sur le port 9222.');
}
try {
  let page = null;
  for (let n = 0; n < 300 && !page; n++) {
    page = browser.contexts().flatMap((c) => c.pages()).find((p) => p.url().includes('tauri.localhost'));
    if (!page) await new Promise((r) => setTimeout(r, 100));
  }
  if (!page) throw new Error('fenêtre Tauri introuvable après 30 s.');
  await page.locator('[data-testid="ligne"]').first().waitFor({ timeout: 60000 });
  await page.locator('[data-testid="ligne"]').first().click();
  await page.waitForTimeout(600);
  await page.screenshot({ path: path.join(sortie, 'v2-reception.png') });
  await page.locator('[data-onglet="nonlus"]').click();
  await page.waitForTimeout(400);
  await page.screenshot({ path: path.join(sortie, 'v2-nonlus.png') });
  await page.locator('[data-onglet="tous"]').click();
  await page.waitForTimeout(300);
  await page.evaluate(() => window.__mesure.theme('nuit'));
  await page.waitForTimeout(300);
  await page.screenshot({ path: path.join(sortie, 'v2-nuit.png') });
  await page.evaluate(() => window.__mesure.theme('nature'));
  console.log('v2 capturée (3 états)');
  console.log(`paires dans ${sortie}`);
} finally {
  if (browser) await browser.close();
  app.kill();
}
