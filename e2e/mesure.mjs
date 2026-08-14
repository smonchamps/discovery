// Mesure des budgets du plan (PLAN.md §1) sur build RELEASE, 50 000
// messages seedés — l'outil des revues de phase.
//
//   node mesure.mjs
//
// Deux variables d'environnement permettent de changer d'échelle sans
// toucher au script — le gate 3 exige 3 comptes et 200 000 messages
// cumulés (PLAN.md §4) :
//
//   MESURE_DB       chemin de la base (défaut : target/e2e/mesure.db)
//   MESURE_COMPTES  « email:nombre » séparés par des virgules
//                   (défaut : mesure@exemple.fr:50000)
//
// La base du gate 3 se place HORS du dépôt : celui-ci vit dans OneDrive,
// dont la synchronisation perturberait la mesure qu'on est en train de
// prendre.
//
// Méthodologie RAM (ADR 0002) : somme des working sets PRIVÉS du
// processus principal et de ses processus WebView2, après 30 s de
// stabilisation — c'est ce que l'utilisateur voit dans le Gestionnaire
// des tâches, sans les réservations jamais résidentes de Chromium.
import { spawn, execSync } from 'node:child_process';
import { existsSync, mkdirSync, rmSync } from 'node:fs';
import path from 'node:path';
import { chromium } from '@playwright/test';

const root = path.resolve(import.meta.dirname, '..');
execSync('cargo build -p wind-desktop --release', { cwd: root, stdio: 'inherit' });

const db = process.env.MESURE_DB || path.join(root, 'target', 'e2e', 'mesure.db');
const comptes = (process.env.MESURE_COMPTES || 'mesure@exemple.fr:50000')
  .split(',')
  .map((entree) => {
    const [email, nombre] = entree.split(':');
    return { email: email.trim(), nombre: Number(nombre) };
  });

// MESURE_REUTILISER=1 garde la base en place : re-seeder 200 000
// enveloppes coûte une demi-minute qu'on ne veut pas repayer quand on
// remesure plusieurs fois de suite le même décor.
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

// Un profil WebView2 dédié, comme le harnais E2E (`launch.mjs`). Sans
// lui la mesure n'est pas reproductible : WebView2 partage UN processus
// navigateur par dossier de données utilisateur, donc une instance de
// Wind déjà ouverte — celle de l'utilisateur — fait ignorer nos
// arguments, et le port CDP ne s'ouvre jamais. Symptôme observé :
// « Cannot read properties of null », 30 s plus tard, sans autre indice.
const profile = path.join(root, 'target', 'e2e', 'webview2-mesure');
mkdirSync(profile, { recursive: true });

const env = {
  ...process.env,
  WIND_DB_PATH: db,
  WIND_E2E_ACCOUNT: comptes[0].email,
  WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=9222',
  WEBVIEW2_USER_DATA_FOLDER: profile,
};
delete env.GOOGLE_CLIENT_ID;
delete env.GOOGLE_CLIENT_SECRET;

const app = spawn(path.join(root, 'target', 'release', 'wind-desktop.exe'), [], {
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
if (!browser) {
  // Dire pourquoi, plutôt que de laisser un TypeError trente lignes plus
  // bas. C'est la leçon de l'ADR 0005 : un échec muet coûte un passage.
  app.kill();
  throw new Error(
    'CDP injoignable sur le port 9222 après 30 s. '
    + `Le processus est-il mort ? Une autre instance de Wind tourne-t-elle avec le profil ${profile} ?`,
  );
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
  // Restreindre la mesure à NOTRE instance : une fenêtre ouverte par
  // ailleurs s'ajouterait au total sans que rien ne le signale.
  const ram = execSync(
    `powershell -NoProfile -ExecutionPolicy Bypass -File "${path.join(import.meta.dirname, 'mesure-ram.ps1')}"`
    + ` -AppPid ${app.pid} -Profil "${profile}"`,
  ).toString();
  console.log('RAM        :', ram.trim());
} finally {
  if (browser) await browser.close();
  app.kill();
}
