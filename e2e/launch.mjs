// Lanceur E2E : construit l'application, seed une base ISOLÉE, démarre la
// fenêtre Tauri avec les crochets de test, s'y attache via CDP.
//
// Déterminisme par construction :
// - base de test jetable (DISCOVERY_DB_PATH) — jamais la vraie ;
// - compte factice au jeton invalide (DISCOVERY_E2E_ACCOUNT) — hors ligne
//   garanti, la boîte d'envoi journalise sans jamais rien envoyer ;
// - configuration OAuth retirée de l'environnement — aucun test ne peut
//   toucher au vrai compte, même par accident.
//
// Deux leçons du premier passage en CI :
// - **diagnosticabilité** : la sortie de l'application est CAPTURÉE et
//   recrachée en cas d'échec. Sans cela, une panique au démarrage ou un
//   WebView2 absent se présentent comme un timeout muet, indiagnosticable
//   à distance ;
// - **on attend la PAGE, pas le port** : le CDP répond avant que la fenêtre
//   n'ait créé son document. Se contenter du port ouvert crée une course
//   qui se voit dès que le démarrage est froid.
import { spawn, execSync } from 'node:child_process';
import { mkdirSync, rmSync } from 'node:fs';
import path from 'node:path';
import { chromium } from '@playwright/test';

const root = path.resolve(import.meta.dirname, '..');
const CDP_PORT = 9222;
// Un premier démarrage WebView2 sur machine froide (CI : pas de cache,
// antivirus actif) dépasse largement les 30 s d'origine.
const READY_TIMEOUT_MS = 90_000;
const POLL_MS = 500;

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

  // Profil WebView2 explicite et inscriptible : sur un runner CI,
  // l'emplacement par défaut peut être refusé. Stable d'un lancement à
  // l'autre — un profil neuf à chaque fois rendrait chaque démarrage
  // froid, donc lent, pour rien.
  const profile = path.join(root, 'target', 'e2e', 'webview2');
  mkdirSync(profile, { recursive: true });

  const env = {
    ...process.env,
    DISCOVERY_DB_PATH: db,
    DISCOVERY_E2E_ACCOUNT: accounts.map((account) => account.email).join(','),
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${CDP_PORT}`,
    WEBVIEW2_USER_DATA_FOLDER: profile,
  };
  delete env.GOOGLE_CLIENT_ID;
  delete env.GOOGLE_CLIENT_SECRET;

  const app = spawn(path.join(root, 'target', 'debug', 'discovery-desktop.exe'), [], {
    env,
    stdio: ['ignore', 'pipe', 'pipe'],
  });

  // Le journal de l'application est notre seule fenêtre sur un échec de
  // démarrage : on le collecte dès la première ligne.
  let log = '';
  app.stdout.on('data', (chunk) => {
    log += chunk;
  });
  app.stderr.on('data', (chunk) => {
    log += chunk;
  });
  let exited = null;
  app.on('exit', (code, signal) => {
    exited = { code, signal };
  });

  // On attend que la PAGE de l'application soit là. On s'arrête net si le
  // processus meurt : inutile d'attendre 90 s un CDP qui ne viendra jamais.
  let browser = null;
  let page = null;
  const deadline = Date.now() + READY_TIMEOUT_MS;
  while (!page && !exited && Date.now() < deadline) {
    try {
      browser ??= await chromium.connectOverCDP(`http://127.0.0.1:${CDP_PORT}`);
      page =
        browser
          .contexts()
          .flatMap((context) => context.pages())
          .find((candidate) => candidate.url().includes('tauri.localhost')) ?? null;
    } catch {
      // Ni le port ni la page ne sont prêts : on repasse.
    }
    if (!page) await new Promise((resolve) => setTimeout(resolve, POLL_MS));
  }

  if (!page) {
    if (browser) await browser.close().catch(() => {});
    app.kill();
    throw new Error(startupFailure(exited, browser !== null, log));
  }
  return { app, browser, page };
}

/// Message d'échec qui DIT pourquoi : processus mort (avec son code), port
/// muet, ou page jamais créée — et dans tous les cas la sortie réelle de
/// l'application.
function startupFailure(exited, connected, log) {
  let cause;
  if (exited) {
    cause = `l'application s'est arrêtée au démarrage (code ${exited.code}, signal ${exited.signal})`;
  } else if (connected) {
    cause = `CDP joignable sur le port ${CDP_PORT}, mais aucune page « tauri.localhost » après ${READY_TIMEOUT_MS / 1000} s`;
  } else {
    cause = `CDP injoignable sur le port ${CDP_PORT} après ${READY_TIMEOUT_MS / 1000} s`;
  }
  const output = log.trim();
  return output
    ? `${cause}\n--- sortie de l'application ---\n${output}\n--- fin ---`
    : `${cause}\n(l'application n'a rien écrit sur sa sortie)`;
}

export async function closeApp({ app, browser }) {
  if (browser) await browser.close().catch(() => {});
  if (app) app.kill();
}
