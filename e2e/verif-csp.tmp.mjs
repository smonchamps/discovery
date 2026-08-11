// Sonde CSP : la balise réellement servie, et le sort de deux commandes
// dans la même fenêtre (binaire déjà compilé — pas de rebuild).
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
  await new Promise((r) => setTimeout(r, 2500));
  const sonde = await page.evaluate(async () => {
    const meta = document.querySelector('meta[http-equiv="Content-Security-Policy"]');
    const invoke = window.__TAURI__.core.invoke;
    const essaye = async (cmd, args) => {
      try {
        const r = await invoke(cmd, args);
        return `OK (${JSON.stringify(r).length} octets)`;
      } catch (e) {
        return `ECHEC ${String(e).slice(0, 90)}`;
      }
    };
    return {
      csp: meta ? meta.content : '(pas de meta CSP)',
      list_messages: await essaye('list_messages', { offset: 0, limit: 5 }),
      list_category: await essaye('list_category', { category: 'reception', accountId: null, nonLus: false, offset: 0, limit: 5 }),
      nav_snapshot: await essaye('nav_snapshot'),
      sync_progress: await essaye('sync_progress'),
    };
  });
  console.log(JSON.stringify(sonde, null, 1));
} finally {
  if (browser) await browser.close();
  app.kill();
}
