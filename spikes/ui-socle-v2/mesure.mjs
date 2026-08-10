// Harnais de mesure — sans dépendance. Un serveur Node sert l'arbre du
// spike et collecte les résultats ; chaque page s'auto-mesure et POST son
// bilan. On lance Edge en headless (Chromium = moteur de WebView2) avec
// `--enable-precise-memory-info` pour un tas JS exact.
//
//   node mesure.mjs                 # mesure toutes les options construites
//   node mesure.mjs option1         # une seule
//
// Méthodologie assumée (leçon PASSATION : un outil de mesure se vérifie) :
//   - même corpus déterministe, même scénario, même moteur pour les 3 ;
//   - profil Edge jetable et isolé (sinon une fenêtre déjà ouverte fausse) ;
//   - poids = octets GZIPPÉS réellement expédiés (proxy transfert/démarrage).
import { createServer } from 'node:http';
import { spawn } from 'node:child_process';
import { readFile, stat, readdir } from 'node:fs/promises';
import { existsSync, mkdtempSync } from 'node:fs';
import { gzipSync } from 'node:zlib';
import { tmpdir } from 'node:os';
import path from 'node:path';

const RACINE = import.meta.dirname;
const EDGE = 'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe';
const PORT = 8791;

const OPTIONS = {
  option1: { url: '/option1-web-components/index.html', dir: 'option1-web-components' },
  option2: { url: '/option2-svelte/dist/index.html', dir: 'option2-svelte/dist' },
};

const MIME = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
};

let resoudreResultat = null;

const serveur = createServer(async (req, res) => {
  if (req.method === 'POST' && req.url === '/resultat') {
    let corps = '';
    req.on('data', (c) => (corps += c));
    req.on('end', () => {
      res.writeHead(204).end();
      if (resoudreResultat) resoudreResultat(JSON.parse(corps));
    });
    return;
  }
  try {
    const rel = decodeURIComponent(req.url.split('?')[0]);
    const fichier = path.join(RACINE, rel);
    if (!fichier.startsWith(RACINE)) { res.writeHead(403).end(); return; }
    const donnees = await readFile(fichier);
    res.writeHead(200, { 'content-type': MIME[path.extname(fichier)] || 'application/octet-stream' });
    res.end(donnees);
  } catch {
    res.writeHead(404).end('introuvable');
  }
});

// Octets gzippés expédiés par une option : tous les .js/.css/.html du
// dossier servi (+ les modules communs importés, pour ①).
async function poidsGz(dir, communs = []) {
  const base = path.join(RACINE, dir);
  const fichiers = [];
  async function marcher(d) {
    for (const e of await readdir(d, { withFileTypes: true })) {
      const p = path.join(d, e.name);
      if (e.isDirectory()) await marcher(p);
      else if (/\.(js|mjs|css|html)$/.test(e.name)) fichiers.push(p);
    }
  }
  if (existsSync(base)) await marcher(base);
  for (const c of communs) if (existsSync(path.join(RACINE, c))) fichiers.push(path.join(RACINE, c));
  let brut = 0, gz = 0;
  for (const f of fichiers) {
    const d = await readFile(f);
    brut += d.length;
    gz += gzipSync(d).length;
  }
  return { brut, gz, nb: fichiers.length };
}

async function mesurerOption(nom, cfg) {
  const profil = mkdtempSync(path.join(tmpdir(), 'spike-edge-'));
  const args = [
    '--headless=new', '--disable-gpu', '--no-first-run',
    '--no-default-browser-check', '--enable-precise-memory-info',
    '--window-size=1280,1000', `--user-data-dir=${profil}`,
    `http://127.0.0.1:${PORT}${cfg.url}`,
  ];
  const edge = spawn(EDGE, args, { stdio: 'ignore' });

  const resultat = await Promise.race([
    new Promise((r) => (resoudreResultat = r)),
    new Promise((_, rej) => setTimeout(() => rej(new Error(`${nom} : pas de résultat après 60 s`)), 60000)),
  ]).finally(() => edge.kill());

  const communs = nom === 'option1'
    ? ['commun/systeme.css', 'commun/donnees.js', 'commun/scenario.js', 'commun/rapporter.js']
    : [];
  resultat.poids = await poidsGz(cfg.dir, communs);
  return resultat;
}

const demandees = process.argv.slice(2).length ? process.argv.slice(2) : Object.keys(OPTIONS);

await new Promise((r) => serveur.listen(PORT, '127.0.0.1', r));

const bilans = [];
for (const nom of demandees) {
  const cfg = OPTIONS[nom];
  if (!cfg || !existsSync(path.join(RACINE, cfg.dir))) {
    console.log(`⏭  ${nom} : non construit (ignoré)`);
    continue;
  }
  process.stdout.write(`▶  ${nom} … `);
  try {
    const r = await mesurerOption(nom, cfg);
    bilans.push(r);
    console.log('ok');
  } catch (e) {
    console.log('ÉCHEC — ' + e.message);
  }
}
serveur.close();

if (bilans.length) {
  const col = (s, n) => String(s).padEnd(n);
  const num = (s, n) => String(s).padStart(n);
  console.log('\n' + col('option', 10) + num('1er rendu', 11) + num('page p50', 10)
    + num('page p95', 10) + num('page max', 10) + num('thème p50', 11)
    + num('thème p95', 11) + num('tas JS Mo', 11) + num('poids gz Ko', 13));
  for (const b of bilans) {
    console.log(col(b.id, 10) + num(b.premierRenduMs, 11) + num(b.pageP50Ms, 10)
      + num(b.pageP95Ms, 10) + num(b.pageMaxMs, 10) + num(b.themeP50Ms, 11)
      + num(b.themeP95Ms, 11) + num(b.tasJsMo, 11)
      + num((b.poids.gz / 1024).toFixed(1), 13));
  }
  console.log('\nJSON :', JSON.stringify(bilans));
}
