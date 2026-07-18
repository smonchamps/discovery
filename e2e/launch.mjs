// Lanceur E2E : construit l'application, seed une base ISOLÉE, démarre la
// fenêtre Tauri avec les crochets de test, s'y attache via CDP.
//
// Déterminisme par construction :
// - base de test jetable (DISCOVERY_DB_PATH) — jamais la vraie ;
// - compte factice au jeton invalide (DISCOVERY_E2E_ACCOUNT) — hors ligne
//   garanti, la boîte d'envoi journalise sans jamais rien envoyer ;
// - configuration OAuth retirée de l'environnement — aucun test ne peut
//   toucher au vrai compte, même par accident.
import { spawn, execSync } from 'node:child_process';
import { mkdirSync, rmSync } from 'node:fs';
import path from 'node:path';
import { chromium } from '@playwright/test';

const root = path.resolve(import.meta.dirname, '..');
const CDP_PORT = 9222;

export async function launchApp({
  accounts = [{ email: 'e2e@exemple.fr', messages: 200 }],
} = {}) {
  execSync('cargo build -p discovery-desktop', { cwd: root, stdio: 'inherit' });

  const db = path.join(root, 'target', 'e2e', 'parcours.db');
  rmSync(db, { force: true });
  mkdirSync(path.dirname(db), { recursive: true });
  for (const account of accounts) {
    execSync(
      `cargo run -p mail-core --example seed_inbox -- "${db}" ${account.messages} ${account.email}`,
      { cwd: root, stdio: 'inherit' },
    );
  }

  const env = {
    ...process.env,
    DISCOVERY_DB_PATH: db,
    DISCOVERY_E2E_ACCOUNT: accounts.map((account) => account.email).join(','),
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${CDP_PORT}`,
  };
  delete env.GOOGLE_CLIENT_ID;
  delete env.GOOGLE_CLIENT_SECRET;

  const app = spawn(path.join(root, 'target', 'debug', 'discovery-desktop.exe'), [], {
    env,
    stdio: 'ignore',
  });

  let browser = null;
  for (let attempt = 0; attempt < 60 && !browser; attempt++) {
    try {
      browser = await chromium.connectOverCDP(`http://127.0.0.1:${CDP_PORT}`);
    } catch {
      await new Promise((resolve) => setTimeout(resolve, 500));
    }
  }
  if (!browser) {
    app.kill();
    throw new Error(`CDP inaccessible sur le port ${CDP_PORT} après 30 s`);
  }

  const pages = browser.contexts().flatMap((context) => context.pages());
  const page = pages.find((candidate) => candidate.url().includes('tauri.localhost'));
  if (!page) {
    await browser.close();
    app.kill();
    throw new Error("page de l'application introuvable via CDP");
  }
  return { app, browser, page };
}

export async function closeApp({ app, browser }) {
  if (browser) await browser.close().catch(() => {});
  if (app) app.kill();
}
