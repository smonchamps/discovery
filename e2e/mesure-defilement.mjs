// Banc du défilement profond (chantier defilement-archives, Phase 0) :
// reproduit le constat terrain du 2026-08-20 — un drag rapide de la
// barre de défilement dans Archives laisse des blocs « .. », puis la
// bascule vers n'importe quel dossier dit « Aucun message ici. » sur
// des boîtes pleines, plusieurs minutes au terrain.
//
// Hypothèse à éprouver (jamais de supposition sans mesure) : chaque
// position intermédiaire du drag déclenche ses pages `list_category`
// (O(offset) hors réception), rien n'annule les pages devenues
// invisibles, et le verrou global de `hors_pompe` sérialise le tout —
// la file se draine en minutes, TOUTES les commandes attendent derrière.
//
// Comptage : la couture `window.__e2eJournal` de transport.js (posée
// par PLAN-DEFILEMENT-PROFOND E1) — un relevé {commande, depart,
// arrivee} par appel au coeur. (Deux voies écartées, constatées :
// enrober `__TAURI__.core.invoke` perd la course contre l'injection
// Tauri — defineProperty tardif la remplace sans setter ; enrober
// `__TAURI_INTERNALS__.invoke` après coup ne compte rien, transport
// détient la référence d'origine.)
//
// Protocole :
// - base seedée : 2 000 INBOX + 120 000 Archives (un compte) ;
// - 3 s de bruit de fond (sondes périodiques) pour étalonner ;
// - drag simulé : scrollTop poussé en 120 pas sur ~2 s (≈60 évts/s,
//   la densité d'une barre tenue au clic) jusqu'à 1/3 de la liste ;
// - échantillonnage 500 ms : appels partis/réglés, lignes, placeholders,
//   « Aucun message ici. » ;
// - à T+5 s : bascule Réception puis retour Archives (le second volet du
//   constat) ; on échantillonne jusqu'au rétablissement.
//
//   node mesure-defilement.mjs
//   MESURE_REUTILISER=1 pour garder la base en place.
import { spawn, execSync } from 'node:child_process';
import { existsSync, mkdirSync, rmSync } from 'node:fs';
import path from 'node:path';
import { chromium } from '@playwright/test';
import { construireV2, purgerCacheHttp } from './rebuild-v2.mjs';
import { purgerOAuth } from './isolation.mjs';
import { allouerPortCdp } from './port-cdp.mjs';
import { argsNavigateur } from './args-navigateur.mjs';
import { tenirBarre } from './geste-defilement.mjs';

const root = path.resolve(import.meta.dirname, '..');
const ARCHIVES = 120_000;
const INBOX = 2_000;

construireV2(root);

// --- Base seedée -----------------------------------------------------
const db = path.join(root, 'target', 'e2e', 'mesure-defilement.db');
if (process.env.MESURE_REUTILISER && existsSync(db)) {
  console.log(`base réutilisée : ${db}`);
} else {
  rmSync(db, { force: true });
  mkdirSync(path.dirname(db), { recursive: true });
  execSync(
    `cargo run -p mail-core --example seed_inbox --release -- "${db}" ${INBOX} mesure@exemple.fr`,
    { cwd: root, stdio: 'inherit' },
  );
  // Le seeder inscrit lui-même la boîte « Archives » au cache
  // `folders` : la canonique archives résout sans retouche SQL.
  execSync(
    `cargo run -p mail-core --example seed_inbox --release -- "${db}" ${ARCHIVES} mesure@exemple.fr 500 0 Archives`,
    { cwd: root, stdio: 'inherit' },
  );
}

// --- Lancement (miroir de mesure-v2.mjs) -----------------------------
const profile = path.join(root, 'target', 'e2e', 'webview2-mesure-defilement');
mkdirSync(profile, { recursive: true });
purgerCacheHttp(profile);
const port = await allouerPortCdp();
const env = {
  ...process.env,
  WIND_DB_PATH: db,
  WIND_E2E_ACCOUNT: 'mesure@exemple.fr',
  WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: argsNavigateur(root, port),
  WEBVIEW2_USER_DATA_FOLDER: profile,
};
purgerOAuth(env);

const app = spawn(path.join(root, 'target', 'release', 'wind-desktop.exe'), [], {
  env,
  stdio: 'ignore',
});

let browser = null;
for (let attempt = 0; attempt < 300 && !browser; attempt++) {
  try {
    browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
  } catch {
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
}
if (!browser) {
  app.kill();
  throw new Error(`CDP injoignable sur le port ${port} après 30 s`);
}

try {
  let page = null;
  for (let attempt = 0; attempt < 300 && !page; attempt++) {
    page = browser
      .contexts()
      .flatMap((context) => context.pages())
      .find((candidate) => candidate.url().includes('tauri.localhost'));
    if (!page) await new Promise((resolve) => setTimeout(resolve, 100));
  }
  if (!page) throw new Error('fenêtre Tauri introuvable après 30 s');

  page.on('console', (message) => {
    if (message.type() === 'error') console.log(`[console] ${message.text()}`);
  });
  await page.locator('[data-testid="ligne"]').first().waitFor({ timeout: 60000 });

  await page.evaluate(() => {
    window.__e2eJournal = [];
  });

  const etatEcran = () => page.evaluate(() => {
    const cadre = document.querySelector('[data-testid="liste"] .cadre');
    const journal = window.__e2eJournal.filter((a) => a.commande === 'list_category');
    const regles = journal.filter((a) => a.arrivee !== null).length;
    return {
      t: Math.round(performance.now()),
      appels: journal.length,
      regles,
      enVol: journal.length - regles,
      lignes: document.querySelectorAll('[data-testid="ligne"]').length,
      attentes: document.querySelectorAll('[data-testid="ligne-attente"]').length,
      videTexte: document.querySelector('[data-testid="liste"] .vide')?.textContent?.trim() ?? null,
      scrollTop: cadre ? Math.round(cadre.scrollTop) : null,
    };
  });

  // --- Bruit de fond : 3 s de sondes périodiques, sans geste ----------
  const avantBruit = await etatEcran();
  await new Promise((resolve) => setTimeout(resolve, 3000));
  const apresBruit = await etatEcran();
  const bruitParSeconde = (apresBruit.appels - avantBruit.appels) / 3;
  console.log(`bruit de fond : ${bruitParSeconde.toFixed(1)} appel(s)/s hors défilement`);

  // --- Archives, puis le drag ----------------------------------------
  await page.locator('[data-testid="nav-dossier"][data-categorie="archives"]').click();
  await page.locator('[data-testid="ligne"]').first().waitFor({ timeout: 30000 });
  // Le total est asynchrone (les lignes d'abord, le comptage au repos) :
  // le drag vise 1/3 de la VRAIE hauteur, pas du plancher provisoire.
  await page.waitForFunction(() => window.__mesure.etat().totalPrecis, null, { timeout: 30000 });
  await new Promise((resolve) => setTimeout(resolve, 1000));
  const avantDrag = await etatEcran();
  console.log('avant drag :', JSON.stringify(avantDrag));

  // Un drag TENU au clic : ~60 événements/s pendant 2 s — le geste
  // partagé avec la spec (geste-defilement.mjs).
  await tenirBarre(page, { pas: 120 });
  const finDrag = Date.now();
  const apresDrag = await etatEcran();
  console.log('drag fini  :', JSON.stringify(apresDrag));
  console.log(`rafale du drag : ~${apresDrag.appels - avantDrag.appels} appels en ${((apresDrag.t - avantDrag.t) / 1000).toFixed(1)} s`);

  // --- Échantillonnage + bascule de dossier à T+5 s -------------------
  let bascule = false;
  let retabli = null;
  const departEchantillons = Date.now();
  while (Date.now() - departEchantillons < 180_000) {
    await new Promise((resolve) => setTimeout(resolve, 500));
    const instantane = await etatEcran();
    const depuisDrag = ((Date.now() - finDrag) / 1000).toFixed(1);
    console.log(`T+${depuisDrag}s :`, JSON.stringify(instantane));

    if (!bascule && Date.now() - finDrag > 5000) {
      bascule = true;
      console.log('--- bascule Réception ---');
      await page.locator('[data-testid="nav-dossier"][data-categorie="reception"]').click();
      await new Promise((resolve) => setTimeout(resolve, 1500));
      console.log('réception  :', JSON.stringify(await etatEcran()));
      console.log('--- retour Archives ---');
      await page.locator('[data-testid="nav-dossier"][data-categorie="archives"]').click();
      await new Promise((resolve) => setTimeout(resolve, 1500));
      console.log('archives   :', JSON.stringify(await etatEcran()));
    }

    // Rétabli : plus rien en vol, des lignes visibles, pas d'attente.
    if (bascule && instantane.enVol === 0 && instantane.lignes > 0
        && instantane.attentes === 0 && instantane.videTexte === null) {
      retabli = depuisDrag;
      break;
    }
  }

  // --- Dépouillement ---------------------------------------------------
  const journal = (await page.evaluate(() => window.__e2eJournal))
    .filter((a) => a.commande === 'list_category');
  const regles = journal.filter((a) => a.arrivee !== null);
  const durees = regles.map((a) => a.arrivee - a.depart);
  // Le plafond de vols simultanés — l'invariant E1 (≤ 2 après correction).
  const bornes = [];
  for (const a of journal) {
    bornes.push([a.depart, +1]);
    bornes.push([a.arrivee ?? Number.MAX_SAFE_INTEGER, -1]);
  }
  bornes.sort((x, y) => x[0] - y[0] || x[1] - y[1]);
  let courant = 0;
  let enVolMax = 0;
  for (const [, delta] of bornes) {
    courant += delta;
    if (courant > enVolMax) enVolMax = courant;
  }
  const stats = (valeurs) => {
    if (valeurs.length === 0) return 'aucune valeur';
    const tri = [...valeurs].sort((a, b) => a - b);
    const q = (p) => tri[Math.min(tri.length - 1, Math.floor(p * tri.length))];
    return `p50 ${q(0.5).toFixed(0)} ms · p95 ${q(0.95).toFixed(0)} ms · max ${tri[tri.length - 1].toFixed(0)} ms`;
  };
  console.log('\n--- bilan ---');
  console.log(`appels list_category : ${journal.length} au total, ${regles.length} réglés`);
  console.log(`vols simultanés (max) : ${enVolMax}`);
  console.log(`durée bout en bout (départ -> arrivée, attente de file comprise) : ${stats(durees)}`);
  console.log(retabli !== null
    ? `rétablissement : T+${retabli}s après la fin du drag`
    : 'PAS de rétablissement en 180 s');
} finally {
  await browser.close().catch(() => {});
  app.kill();
}
