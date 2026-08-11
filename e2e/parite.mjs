// Banc de parité visuelle Clarity (PLAN-UI-V2 §4) : v2 sur le décor du
// prototype (seed_clarity) d'un côté, le prototype lui-même
// (docs/design/ui_prototype.html, joué dans Edge) de l'autre — mêmes
// états, même viewport 1440×900, même densité de pixels. Les paires de
// captures atterrissent dans target/parite/ ; la parité se JUGE au
// terrain, ce banc l'outille.
//
//   node parite.mjs
//
// États capturés : onboarding (écran 01), réception avec le fil Vantis
// ouvert, onglet Non lus, conversation plein écran, composition en mode
// réponse, surimpression Réglages, thème « La nuit ».
import { spawn, execSync } from 'node:child_process';
import { mkdirSync, rmSync } from 'node:fs';
import path from 'node:path';
import { chromium } from '@playwright/test';
import { construireV2, purgerCacheHttp } from './rebuild-v2.mjs';

const root = path.resolve(import.meta.dirname, '..');
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
  await page.waitForTimeout(300);
  await page.screenshot({ path: path.join(sortie, 'proto-onboarding.png') });
  await page.locator('button', { hasText: 'Continuer' }).click();
  await page.locator('text=Boîte de réception').first().waitFor();
  await page.waitForTimeout(400);
  await page.screenshot({ path: path.join(sortie, 'proto-reception.png') });
  await page.locator('span', { hasText: 'Non lus' }).last().click();
  await page.waitForTimeout(300);
  await page.screenshot({ path: path.join(sortie, 'proto-nonlus.png') });
  await page.locator('span', { hasText: /^Tous$/ }).last().click();
  await page.locator('span', { hasText: 'Voir la conversation' }).click();
  await page.waitForTimeout(300);
  await page.screenshot({ path: path.join(sortie, 'proto-conversation.png') });
  await page.locator('button', { hasText: 'Boîte de réception' }).click();
  await page.waitForTimeout(200);
  // Pas d'ancrage exact : le texte du bouton contient AUSSI la ligature
  // de son icône (« reply »).
  await page.locator('button', { hasText: 'Répondre' }).first().click();
  await page.waitForTimeout(300);
  await page.screenshot({ path: path.join(sortie, 'proto-composition.png') });
  await page.locator('span', { hasText: /^Annuler$/ }).click();
  await page.waitForTimeout(200);
  await page.locator('button', { hasText: 'Réglages' }).click();
  await page.waitForTimeout(300);
  await page.screenshot({ path: path.join(sortie, 'proto-reglages.png') });
  await page.locator('span', { hasText: 'La nuit' }).first().click();
  await page.locator('button', { hasText: 'Terminé' }).click();
  await page.waitForTimeout(300);
  await page.screenshot({ path: path.join(sortie, 'proto-nuit.png') });
  await navigateur.close();
  console.log('prototype capturé (6 états)');
}

// --- 2. v2 sur le décor Clarity, fenêtre aux mêmes dimensions -------
construireV2(root, { fenetre: { width: 1440, height: 900 } });

const db = path.join(root, 'target', 'e2e', 'clarity.db');
rmSync(db, { force: true });
execSync(`cargo run -p mail-core --example seed_clarity --release -- "${db}"`, {
  cwd: root,
  stdio: 'inherit',
});

const profile = path.join(root, 'target', 'e2e', 'webview2-parite');
mkdirSync(profile, { recursive: true });
purgerCacheHttp(profile);
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
  await page.locator('[data-testid="ligne"]').first().click();
  await page.locator('[data-testid="voir-conversation"]').click();
  await page.locator('[data-testid="message-deplie"]').first().waitFor();
  await page.waitForTimeout(500);
  await page.screenshot({ path: path.join(sortie, 'v2-conversation.png') });
  await page.locator('[data-testid="retour-boite"]').click();
  await page.waitForTimeout(200);
  await page.locator('[data-testid="repondre"]').click();
  await page.locator('[data-testid="composition"]').waitFor();
  await page.waitForTimeout(400);
  await page.screenshot({ path: path.join(sortie, 'v2-composition.png') });
  // Vider AVANT d'annuler : fermer conserve (brouillon + toast), et le
  // banc ne doit ni dériver le décor ni photographier un toast.
  await page.locator('[data-testid="composition-a"]').fill('');
  await page.locator('[data-testid="composition-objet"]').fill('');
  await page.locator('[data-testid="composition-corps"]').fill('');
  await page.locator('[data-testid="composition-annuler"]').click();
  await page.waitForTimeout(300);
  // Réglages puis « La nuit » par le VRAI parcours — plus de crochet.
  await page.locator('[data-testid="reglages"]').click();
  await page.waitForTimeout(300);
  await page.screenshot({ path: path.join(sortie, 'v2-reglages.png') });
  await page.locator('[data-theme-id="nuit"]').click();
  await page.locator('[data-testid="reglages-termine"]').click();
  await page.waitForTimeout(300);
  await page.screenshot({ path: path.join(sortie, 'v2-nuit.png') });
  await page.evaluate(() => window.__mesure.theme('nature'));
  console.log('v2 capturée (5 états)');
} finally {
  if (browser) await browser.close();
  app.kill();
  await new Promise((r) => setTimeout(r, 1500));
}

// --- 3. v2 à zéro compte : l'écran 01 sur base VIERGE ----------------
{
  const dbVierge = path.join(root, 'target', 'e2e', 'clarity-vierge.db');
  rmSync(dbVierge, { force: true });
  const envVierge = { ...env, DISCOVERY_DB_PATH: dbVierge };
  delete envVierge.DISCOVERY_E2E_ACCOUNT;
  purgerCacheHttp(profile);
  const appVierge = spawn(path.join(root, 'target', 'release', 'discovery-desktop.exe'), [], {
    env: envVierge,
    stdio: 'ignore',
  });
  let nav = null;
  for (let n = 0; n < 300 && !nav; n++) {
    try { nav = await chromium.connectOverCDP('http://127.0.0.1:9222'); }
    catch { await new Promise((r) => setTimeout(r, 100)); }
  }
  if (!nav) {
    appVierge.kill();
    throw new Error('CDP injoignable pour le lancement vierge.');
  }
  try {
    let page = null;
    for (let n = 0; n < 300 && !page; n++) {
      page = nav.contexts().flatMap((c) => c.pages()).find((p) => p.url().includes('tauri.localhost'));
      if (!page) await new Promise((r) => setTimeout(r, 100));
    }
    if (!page) throw new Error('fenêtre Tauri (vierge) introuvable après 30 s.');
    await page.locator('[data-testid="onboarding"]').waitFor({ timeout: 60000 });
    await page.waitForTimeout(400);
    await page.screenshot({ path: path.join(sortie, 'v2-onboarding.png') });
    console.log('v2 vierge capturée (écran 01)');
    console.log(`paires dans ${sortie}`);
  } finally {
    await nav.close();
    appVierge.kill();
  }
}
