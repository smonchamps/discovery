// Mesure des budgets du plan (PLAN.md §1) sur build RELEASE, 50 000
// messages seedés — l'outil des revues de phase.
//
//   node mesure.mjs
//
// Méthodologie RAM (ADR 0002) : somme des working sets PRIVÉS du
// processus principal et de ses processus WebView2, après 30 s de
// stabilisation — c'est ce que l'utilisateur voit dans le Gestionnaire
// des tâches, sans les réservations jamais résidentes de Chromium.
import { spawn, execSync } from 'node:child_process';
import { mkdirSync, rmSync } from 'node:fs';
import path from 'node:path';
import { chromium } from '@playwright/test';

const root = path.resolve(import.meta.dirname, '..');
execSync('cargo build -p discovery-desktop --release', { cwd: root, stdio: 'inherit' });

const db = path.join(root, 'target', 'e2e', 'mesure.db');
rmSync(db, { force: true });
mkdirSync(path.dirname(db), { recursive: true });
execSync(`cargo run -p mail-core --example seed_inbox --release -- "${db}" 50000`, {
  cwd: root,
  stdio: 'inherit',
});

const env = {
  ...process.env,
  DISCOVERY_DB_PATH: db,
  DISCOVERY_E2E_ACCOUNT: 'mesure@exemple.fr',
  WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=9222',
};
delete env.GOOGLE_CLIENT_ID;
delete env.GOOGLE_CLIENT_SECRET;

const app = spawn(path.join(root, 'target', 'release', 'discovery-desktop.exe'), [], {
  env,
  stdio: 'ignore',
});

let browser = null;
for (let attempt = 0; attempt < 60 && !browser; attempt++) {
  try {
    browser = await chromium.connectOverCDP('http://127.0.0.1:9222');
  } catch {
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
}

try {
  const page = browser
    .contexts()
    .flatMap((context) => context.pages())
    .find((candidate) => candidate.url().includes('tauri.localhost'));
  await page.locator('.row').first().waitFor({ timeout: 15000 });
  await page.waitForFunction(() => document.getElementById('perf').dataset.startup);
  console.log('démarrage  :', await page.evaluate(() => document.getElementById('perf').dataset.startup));
  console.log('liste      :', await page.locator('#perf').textContent());

  console.log('stabilisation 30 s avant la mesure RAM…');
  await new Promise((resolve) => setTimeout(resolve, 30000));
  const ram = execSync(
    `powershell -NoProfile -ExecutionPolicy Bypass -File "${path.join(import.meta.dirname, 'mesure-ram.ps1')}"`,
  ).toString();
  console.log('RAM        :', ram.trim());
} finally {
  if (browser) await browser.close();
  app.kill();
}
