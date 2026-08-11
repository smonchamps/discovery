// Vérification ponctuelle de l'écran 02 : captures + erreurs console.
import { spawn, execSync } from 'node:child_process';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { chromium } from '@playwright/test';

const root = path.resolve(import.meta.dirname, '..');
const conf = path.join(root, 'apps', 'desktop', 'tauri.conf.json');
const sortie = process.env.CAPTURES || path.join(root, 'target', 'e2e');

execSync('npm run build', { cwd: path.join(root, 'apps', 'desktop', 'ui-v2'), stdio: 'inherit' });
const confOrigine = readFileSync(conf, 'utf8');
const confV2 = JSON.parse(confOrigine);
confV2.build.frontendDist = 'ui-v2/dist';
try {
  writeFileSync(conf, JSON.stringify(confV2, null, 2));
  execSync('cargo build -p discovery-desktop --release', { cwd: root, stdio: 'inherit' });
} finally {
  writeFileSync(conf, confOrigine);
}

const profile = path.join(root, 'target', 'e2e', 'webview2-mesure-v2');
mkdirSync(profile, { recursive: true });
const env = {
  ...process.env,
  DISCOVERY_DB_PATH: process.env.LOCALAPPDATA + '\\discovery-mesure.db',
  DISCOVERY_E2E_ACCOUNT: 'mesure@exemple.fr',
  WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=9222',
  WEBVIEW2_USER_DATA_FOLDER: profile,
};
delete env.GOOGLE_CLIENT_ID;
delete env.GOOGLE_CLIENT_SECRET;

const app = spawn(path.join(root, 'target', 'release', 'discovery-desktop.exe'), [], { env, stdio: 'ignore' });
let browser = null;
for (let n = 0; n < 300 && !browser; n++) {
  try { browser = await chromium.connectOverCDP('http://127.0.0.1:9222'); }
  catch { await new Promise((r) => setTimeout(r, 100)); }
}
const erreurs = [];
try {
  let page = null;
  for (let n = 0; n < 300 && !page; n++) {
    page = browser.contexts().flatMap((c) => c.pages()).find((p) => p.url().includes('tauri.localhost'));
    if (!page) await new Promise((r) => setTimeout(r, 100));
  }
  page.on('console', (msg) => { if (msg.type() === 'error') erreurs.push(msg.text()); });
  page.on('pageerror', (err) => erreurs.push(String(err)));
  await new Promise((r) => setTimeout(r, 4000));
  console.log('sondage DOM :', await page.evaluate(async () => ({
    corps: document.body.innerHTML.length,
    lignes: document.querySelectorAll('[data-testid="ligne"]').length,
    attentes: document.querySelectorAll('[data-testid="ligne-attente"]').length,
    nav: document.querySelectorAll('[data-testid="nav-dossier"]').length,
    etat: window.__mesure ? window.__mesure.etat() : null,
    manuel: await window.__TAURI__.core.invoke('list_category', {
      category: 'reception', accountId: null, nonLus: false, offset: 0, limit: 3,
    }).then((p) => `total ${p.total}, ${p.rows.length} lignes`, (e) => `ECHEC ${e}`),
  })));
  if (erreurs.length) {
    console.log('ERREURS :', erreurs.slice(0, 8));
  }
  await page.locator('[data-testid="ligne"]').first().waitFor({ timeout: 30000 });
  await new Promise((r) => setTimeout(r, 1200));
  await page.screenshot({ path: path.join(sortie, 'ecran02-reception.png') });

  await page.locator('[data-testid="ligne"]').first().click();
  await new Promise((r) => setTimeout(r, 800));
  await page.screenshot({ path: path.join(sortie, 'ecran02-lecture.png') });

  // Dossier Archives (le gros), puis onglet Non lus en réception.
  await page.locator('[data-categorie="archives"]').click();
  await new Promise((r) => setTimeout(r, 900));
  await page.screenshot({ path: path.join(sortie, 'ecran02-archives.png') });
  await page.locator('[data-categorie="reception"]').click();
  await page.locator('[data-onglet="nonlus"]').click();
  await new Promise((r) => setTimeout(r, 900));
  await page.screenshot({ path: path.join(sortie, 'ecran02-nonlus.png') });

  console.log('erreurs console :', erreurs.length === 0 ? 'aucune' : erreurs.slice(0, 5));
} finally {
  if (browser) await browser.close();
  app.kill();
}
