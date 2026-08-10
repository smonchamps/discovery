// Harnais mobile — test en webview mobile RÉELLE côté moteur : Android
// System WebView est du Blink/V8, exactement le moteur d'Edge/Chromium.
// On pilote Edge headless par CDP (WebSocket natif de Node, zéro
// dépendance) pour imposer les conditions d'un téléphone :
//
//   - viewport 390 x 844, deviceScaleFactor 3, mobile:true, tactile ;
//   - UA Android ;
//   - RALENTISSEMENT CPU ×1 / ×4 / ×6 (convention DevTools : ×4 ≈ milieu
//     de gamme, ×6 ≈ entrée de gamme, RELATIF à cette machine).
//
// Honnêteté de mesure (leçon PASSATION) :
//   ✓ moteur RÉEL d'Android (Blink/V8), viewport/DPR/tactile réels,
//     CPU ralenti — la vraie inconnue, le rendu étant 40–60× sous budget
//     sur CPU desktop.
//   ✗ PAS le matériel réel (GPU/RAM/thermique d'un vrai téléphone).
//   ✗ PAS iOS/WKWebView (WebKit, simulateur macOS uniquement) — hors de
//     portée de cette machine, à valider plus tard sur appareil/ferme.
//
//   node mesure-mobile.mjs                 # ① et ② aux 3 régimes CPU
import { createServer } from 'node:http';
import { spawn } from 'node:child_process';
import { readFile } from 'node:fs/promises';
import { existsSync, mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';

const RACINE = import.meta.dirname;
const EDGE = 'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe';
const PORT = 8792;
const CDP = 9223;
const REGIMES = [1, 4, 6];               // ralentissements CPU à balayer
const UA_ANDROID = 'Mozilla/5.0 (Linux; Android 14; Pixel 7) AppleWebKit/537.36 '
  + '(KHTML, like Gecko) Chrome/151.0.0.0 Mobile Safari/537.36';

const OPTIONS = {
  option1: '/option1-web-components/index.html',
  option2: '/option2-svelte/dist/index.html',
};
const MIME = { '.html': 'text/html; charset=utf-8', '.js': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8', '.json': 'application/json; charset=utf-8' };

// --- serveur statique + collecteur de résultats ---
let resoudreResultat = null;
const serveur = createServer(async (req, res) => {
  if (req.method === 'POST' && req.url === '/resultat') {
    let corps = '';
    req.on('data', (c) => (corps += c));
    req.on('end', () => { res.writeHead(204).end(); resoudreResultat?.(JSON.parse(corps)); });
    return;
  }
  try {
    const fichier = path.join(RACINE, decodeURIComponent(req.url.split('?')[0]));
    if (!fichier.startsWith(RACINE)) return void res.writeHead(403).end();
    const d = await readFile(fichier);
    res.writeHead(200, { 'content-type': MIME[path.extname(fichier)] || 'application/octet-stream' });
    res.end(d);
  } catch { res.writeHead(404).end('introuvable'); }
});

// --- client CDP minimal sur le WebSocket natif de Node ---
function cdp(wsUrl) {
  const ws = new WebSocket(wsUrl);
  let id = 0;
  const attente = new Map();
  ws.addEventListener('message', (ev) => {
    const m = JSON.parse(ev.data);
    if (m.id && attente.has(m.id)) { attente.get(m.id)(m); attente.delete(m.id); }
  });
  return {
    ouvert: new Promise((r) => ws.addEventListener('open', r)),
    envoyer(method, params = {}) {
      const monId = ++id;
      return new Promise((resolve, reject) => {
        attente.set(monId, (m) => (m.error ? reject(new Error(method + ': ' + JSON.stringify(m.error))) : resolve(m.result)));
        ws.send(JSON.stringify({ id: monId, method, params }));
      });
    },
    fermer: () => ws.close(),
  };
}

async function ciblePage() {
  for (let essai = 0; essai < 40; essai++) {
    try {
      const cibles = await (await fetch(`http://127.0.0.1:${CDP}/json`)).json();
      const page = cibles.find((c) => c.type === 'page');
      if (page?.webSocketDebuggerUrl) return page.webSocketDebuggerUrl;
    } catch { /* Edge pas encore prêt */ }
    await new Promise((r) => setTimeout(r, 250));
  }
  throw new Error('aucune cible page CDP après 10 s');
}

// --- exécution ---
await new Promise((r) => serveur.listen(PORT, '127.0.0.1', r));
const profil = mkdtempSync(path.join(tmpdir(), 'spike-mobile-'));
const edge = spawn(EDGE, [
  '--headless=new', '--disable-gpu', '--no-first-run', '--no-default-browser-check',
  '--enable-precise-memory-info', `--user-data-dir=${profil}`, `--remote-debugging-port=${CDP}`,
  'about:blank',
], { stdio: 'ignore' });

const bilans = [];
try {
  const cli = cdp(await ciblePage());
  await cli.ouvert;
  await cli.envoyer('Page.enable');
  await cli.envoyer('Emulation.setDeviceMetricsOverride',
    { width: 390, height: 844, deviceScaleFactor: 3, mobile: true });
  await cli.envoyer('Emulation.setTouchEmulationEnabled', { enabled: true, maxTouchPoints: 5 });
  await cli.envoyer('Emulation.setUserAgentOverride', { userAgent: UA_ANDROID, platform: 'Android' });
  try { await cli.envoyer('Emulation.setFocusEmulationEnabled', { enabled: true }); } catch { /* optionnel */ }

  for (const [nom, url] of Object.entries(OPTIONS)) {
    if (!existsSync(path.join(RACINE, url.replace(/^\//, '').split('/').slice(0, 2).join('/')))) {
      console.log(`⏭  ${nom} : non construit`); continue;
    }
    for (const rate of REGIMES) {
      await cli.envoyer('Emulation.setCPUThrottlingRate', { rate });
      const p = new Promise((r) => (resoudreResultat = r));
      await cli.envoyer('Page.navigate', { url: `http://127.0.0.1:${PORT}${url}` });
      const r = await Promise.race([
        p, new Promise((_, rej) => setTimeout(() => rej(new Error('délai')), 120000)),
      ]);
      r.rate = rate;
      bilans.push(r);
      console.log(`▶  ${nom}  CPU ×${rate}  →  1er ${r.premierRenduMs} ms · page p95 ${r.pageP95Ms} ms · thème p95 ${r.themeP95Ms} ms · tas ${r.tasJsMo} Mo`);
    }
  }
  cli.fermer();
} finally {
  edge.kill();
  serveur.close();
}

// --- tableau ---
const col = (s, n) => String(s).padEnd(n);
const num = (s, n) => String(s).padStart(n);
console.log('\n' + col('option', 10) + num('CPU', 5) + num('1er rendu', 11)
  + num('page p50', 10) + num('page p95', 10) + num('page max', 10)
  + num('thème p95', 11) + num('tas JS Mo', 11));
for (const b of bilans) {
  console.log(col(b.id, 10) + num('×' + b.rate, 5) + num(b.premierRenduMs, 11)
    + num(b.pageP50Ms, 10) + num(b.pageP95Ms, 10) + num(b.pageMaxMs, 10)
    + num(b.themeP95Ms, 11) + num(b.tasJsMo, 11));
}
console.log('\nJSON :', JSON.stringify(bilans));
