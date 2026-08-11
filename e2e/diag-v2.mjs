// Diagnostic andon P1 : décomposer le budget « page de liste » en ses
// trois étages — requête coeur (elapsed_us de MessagePage), transport
// IPC, rendu Svelte — pour que l'arbitrage se fasse sur les chiffres du
// bon étage. Réutilise le binaire et la base du banc (aucun build,
// aucun seed) : lancer APRÈS mesure-v2.mjs.
//
//   node diag-v2.mjs
import { spawn } from 'node:child_process';
import { mkdirSync, rmSync } from 'node:fs';
import path from 'node:path';
import { chromium } from '@playwright/test';

const root = path.resolve(import.meta.dirname, '..');
const db = process.env.MESURE_DB || path.join(root, 'target', 'e2e', 'mesure-v2.db');
const profile = path.join(root, 'target', 'e2e', 'webview2-mesure-v2');
mkdirSync(profile, { recursive: true });
for (const dossier of ['Cache', 'Code Cache']) {
  rmSync(path.join(profile, 'EBWebView', 'Default', dossier), { recursive: true, force: true });
}

const env = {
  ...process.env,
  DISCOVERY_DB_PATH: db,
  DISCOVERY_E2E_ACCOUNT: 'mesure@exemple.fr',
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
for (let attempt = 0; attempt < 60 && !browser; attempt++) {
  try {
    browser = await chromium.connectOverCDP('http://127.0.0.1:9222');
  } catch {
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
}
if (!browser) {
  app.kill();
  throw new Error('CDP injoignable sur le port 9222.');
}

try {
  // On attend la PAGE, pas le port (leçon de launch.mjs).
  let page = null;
  for (let attempt = 0; attempt < 60 && !page; attempt++) {
    page = browser
      .contexts()
      .flatMap((context) => context.pages())
      .find((candidate) => candidate.url().includes('tauri.localhost'));
    if (!page) await new Promise((resolve) => setTimeout(resolve, 500));
  }
  if (!page) throw new Error('fenêtre Tauri introuvable après 30 s — le processus a-t-il démarré ?');
  await page.locator('[data-testid="ligne"]').first().waitFor({ timeout: 60000 });

  // 1. Étage coeur + IPC : appel BRUT, sans aucun rendu. `elapsed_us`
  //    est mesuré DANS la commande Rust — la différence avec le mur,
  //    c'est l'IPC (sérialisation comprise).
  const brut = await page.evaluate(async () => {
    const appel = window.__TAURI__.core.invoke;
    const sortie = [];
    for (const offset of [0, 1000, 10000, 50000, 100000, 200000]) {
      const reps = [];
      for (let n = 0; n < 5; n++) {
        const t0 = performance.now();
        const p = await appel('list_messages', { offset, limit: 200 });
        reps.push({ mur: performance.now() - t0, coeur: p.elapsed_us / 1000 });
      }
      reps.sort((x, y) => x.mur - y.mur);
      const med = reps[2];
      sortie.push(`offset ${String(offset).padStart(6)} : coeur ${med.coeur.toFixed(1)} ms · mur ${med.mur.toFixed(1)} ms (méd. de 5)`);
    }
    return sortie;
  });
  for (const ligne of brut) console.log(ligne);

  // 2. Étage rendu : saut vers des pages DÉJÀ servies -> le service est
  //    un no-op, il ne reste que Svelte + reflow.
  const rendu = await page.evaluate(async () => {
    await window.__mesure.page(100000); // chauffe : pages servies
    const reps = [];
    for (let n = 0; n < 20; n++) {
      reps.push(await window.__mesure.page(100000 + (n % 2) * 5)); // même fenêtre
    }
    reps.sort((a, b) => a - b);
    return `rendu seul (pages servies) : méd. ${reps[10].toFixed(1)} ms · max ${reps[19].toFixed(1)} ms`;
  });
  console.log(rendu);

  // 3. Défilement de PROXIMITÉ (le geste réel) : sauts de ± une fenêtre
  //    autour d'une position, pages voisines fraîches.
  const proche = await page.evaluate(async () => {
    const base = 150000;
    await window.__mesure.page(base);
    const reps = [];
    for (let n = 1; n <= 20; n++) {
      reps.push(await window.__mesure.page(base + n * 12)); // ~une fenêtre
    }
    reps.sort((a, b) => a - b);
    return `défilement proche (fenêtre à fenêtre) : méd. ${reps[10].toFixed(1)} ms · p95 ${reps[18].toFixed(1)} ms`;
  });
  console.log(proche);
} finally {
  if (browser) await browser.close();
  app.kill();
}
