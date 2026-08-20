// Lanceur E2E : construit l'application, seed une base ISOLÉE, démarre la
// fenêtre Tauri avec les crochets de test, s'y attache via CDP.
//
// Déterminisme par construction :
// - base de test jetable (WIND_DB_PATH) — jamais la vraie ;
// - compte factice au jeton invalide (WIND_E2E_ACCOUNT) — hors ligne
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
import { construireV2, purgerCacheHttp } from './rebuild-v2.mjs';
import { purgerOAuth } from './isolation.mjs';
import { allouerPortCdp } from './port-cdp.mjs';
import { argsNavigateur } from './args-navigateur.mjs';

const root = path.resolve(import.meta.dirname, '..');
// Un premier démarrage WebView2 sur machine froide (CI : pas de cache,
// antivirus actif) dépasse largement les 30 s d'origine.
const READY_TIMEOUT_MS = 90_000;
const POLL_MS = 500;
// Mémo du port de la suite (alloué au premier lancement, voir attacher).
let portSuite = null;

// L'app embarque ui-v2 (la seule interface depuis B2, PLAN-RETRAIT-V1) ;
// le décor par défaut est le jeu d'essai Clarity (seed_clarity). Les
// pièges du rebuild (dist périmé, zombie, cache) vivent dans
// `rebuild-v2.mjs`, une fois.
// `vierge: true` : base NEUVE et aucun compte factice — l'état « zéro
// compte » qui doit montrer l'écran 01 (onboarding).
// `comptes: [{email, messages}]` : le décor seed_inbox — les parcours
// portés de v1 (R2) rejouent les graines EXACTES des specs d'origine.
export async function launchAppV2({ vierge = false, comptes = null } = {}) {
  construireV2(root, { release: false });

  const db = path.join(
    root,
    'target',
    'e2e',
    vierge ? 'parcours-v2-vierge.db' : comptes ? 'parcours-v2-inbox.db' : 'parcours-v2.db',
  );
  rmSync(db, { force: true });
  mkdirSync(path.dirname(db), { recursive: true });
  if (vierge) {
    return attacher(db, []);
  }
  if (comptes) {
    for (const compte of comptes) {
      execSync(
        `cargo run -p mail-core --example seed_inbox -- "${db}" ${compte.messages} ${compte.email}`,
        { cwd: root, stdio: 'inherit' },
      );
    }
    // `deconnecte: true` : le compte vit au REGISTRE (seedé ci-dessus)
    // mais ne reçoit pas de session — l'état « jeton mort » du réel,
    // celui que Réglages sait désormais réparer (terrain 2026-08-20).
    return attacher(
      db,
      comptes.filter((compte) => !compte.deconnecte).map((compte) => compte.email),
    );
  }
  execSync(`cargo run -p mail-core --example seed_clarity -- "${db}"`, {
    cwd: root,
    stdio: 'inherit',
  });
  return attacher(db, ['paul.merand@atelier-nord.fr', 'paul@merand.fr']);
}

async function attacher(db, emails) {
  // Profil WebView2 explicite et inscriptible : sur un runner CI,
  // l'emplacement par défaut peut être refusé. Stable d'un lancement à
  // l'autre — un profil neuf à chaque fois rendrait chaque démarrage
  // froid, donc lent, pour rien.
  const profile = path.join(root, 'target', 'e2e', 'webview2');
  mkdirSync(profile, { recursive: true });
  purgerCacheHttp(profile);

  // Port CDP libre, choisi par l'OS — un port par SUITE, pas par
  // lancement : WebView2 partage son processus navigateur par profil, et
  // deux lancements de la même gate (même profil) doivent porter des
  // arguments navigateur IDENTIQUES — un processus attardé aux options
  // différentes ferait échouer la création d'environnement. Entre
  // worktrees, chaque suite est un processus Node distinct : ports
  // distincts, plus aucun état partagé (constat 2026-08-15, port-cdp.mjs).
  const port = (portSuite ??= await allouerPortCdp());

  const env = {
    ...process.env,
    WIND_DB_PATH: db,
    // `--lang=fr` : la détection de langue au premier lancement
    // (navigator.language, PLAN-LANGUES) lit la locale du WebView — sans
    // cette épingle, la suite dépendrait de la langue de la machine.
    // Le français reste la langue canonique des parcours (L-6).
    // Les arguments de PRODUCTION (tauri.conf.json) + le port CDP + la
    // langue épinglée, composés par args-navigateur.mjs — la variable
    // ÉCRASE la conf au niveau du loader WebView2, elle doit donc la
    // reprendre pour que l'e2e voie le navigateur livré (revue
    // 2026-08-16). WIND_E2E_ARGS_EXTRA : passe-plat des bancs de mesure
    // (E4, mesure-scrollbar.mjs) — un flag posé dans l'environnement du
    // parent n'atteindrait jamais le WebView2 sans lui.
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: argsNavigateur(
      root,
      port,
      process.env.WIND_E2E_ARGS_EXTRA ?? '',
    ),
    WEBVIEW2_USER_DATA_FOLDER: profile,
  };
  if (emails.length > 0) env.WIND_E2E_ACCOUNT = emails.join(',');
  else delete env.WIND_E2E_ACCOUNT;
  purgerOAuth(env);

  const app = spawn(path.join(root, 'target', 'debug', 'wind-desktop.exe'), [], {
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
      browser ??= await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
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
    throw new Error(startupFailure(exited, browser !== null, log, port));
  }
  return { app, browser, page };
}

/// Message d'échec qui DIT pourquoi : processus mort (avec son code), port
/// muet, ou page jamais créée — et dans tous les cas la sortie réelle de
/// l'application.
function startupFailure(exited, connected, log, port) {
  let cause;
  if (exited) {
    cause = `l'application s'est arrêtée au démarrage (code ${exited.code}, signal ${exited.signal})`;
  } else if (connected) {
    cause = `CDP joignable sur le port ${port}, mais aucune page « tauri.localhost » après ${READY_TIMEOUT_MS / 1000} s`;
  } else {
    cause = `CDP injoignable sur le port ${port} après ${READY_TIMEOUT_MS / 1000} s`;
  }
  const output = log.trim();
  return output
    ? `${cause}\n--- sortie de l'application ---\n${output}\n--- fin ---`
    : `${cause}\n(l'application n'a rien écrit sur sa sortie)`;
}

export async function closeApp({ app, browser }) {
  if (browser) await browser.close().catch(() => {});
  if (app) {
    // Attendre la sortie RÉELLE : un second lancement dans la même gate
    // (écran 01 sur base vierge) réutilise le port de la suite et le
    // profil WebView2 — les reprendre à un processus encore vivant est
    // une course.
    const fini = new Promise((resolve) => {
      if (app.exitCode !== null) resolve();
      else app.once('exit', resolve);
    });
    app.kill();
    await fini;
  }
}
