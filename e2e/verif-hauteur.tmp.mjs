// Mesure DOM : hauteur réelle d'une ligne, ses enfants, et les gabarits
// sondés — pour voir d'où vient l'espacement fautif. Compteurs et
// dimensions seulement.
import { spawn } from 'node:child_process';
import { mkdirSync } from 'node:fs';
import path from 'node:path';
import { chromium } from '@playwright/test';

const root = path.resolve(import.meta.dirname, '..');
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
try {
  let page = null;
  for (let n = 0; n < 300 && !page; n++) {
    page = browser.contexts().flatMap((c) => c.pages()).find((p) => p.url().includes('tauri.localhost'));
    if (!page) await new Promise((r) => setTimeout(r, 100));
  }
  await page.locator('[data-testid="ligne"]').first().waitFor({ timeout: 60000 });
  const mesures = await page.evaluate(() => {
    const lignes = [...document.querySelectorAll('[data-testid="ligne"]')].slice(0, 3);
    const decrire = (el) => ({
      hauteur: el.offsetHeight,
      enfants: [...el.children].map((c) => `${c.className.split(' ')[0]}:${c.offsetHeight}`),
      style: (({ paddingTop, paddingBottom, rowGap, display }) =>
        ({ paddingTop, paddingBottom, rowGap, display }))(getComputedStyle(el)),
    });
    const fenetre = document.querySelector('[data-testid="liste"] .cadre > .espace > div');
    return {
      etat: window.__mesure.etat(),
      lignes: lignes.map(decrire),
      positions: lignes.map((l) => Math.round(l.getBoundingClientRect().top)),
      fenetreStyle: fenetre ? (({ rowGap, display }) => ({ rowGap, display }))(getComputedStyle(fenetre)) : null,
    };
  });
  console.log(JSON.stringify(mesures, null, 1));
} finally {
  if (browser) await browser.close();
  app.kill();
}
