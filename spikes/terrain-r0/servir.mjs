// Sert la racine du depot en local (les pages terrain referencent la police
// vendorisee ../../assets/icones). Aucune ecriture, aucun acces distant.
import { createServer } from 'node:http';
import { readFileSync } from 'node:fs';
import path from 'node:path';

const RACINE = path.resolve(import.meta.dirname, '..', '..');
const PORT = 8797;
const types = {
  '.html': 'text/html; charset=utf-8', '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8', '.css': 'text/css; charset=utf-8',
  '.woff2': 'font/woff2', '.png': 'image/png',
};

createServer((q, s) => {
  try {
    const propre = path.normalize(decodeURIComponent(q.url.split('?')[0]));
    const f = path.join(RACINE, propre);
    if (!f.startsWith(RACINE)) throw new Error('hors racine');
    s.writeHead(200, { 'content-type': types[path.extname(f)] || 'application/octet-stream' });
    s.end(readFileSync(f));
  } catch { if (!s.headersSent) s.writeHead(404); s.end(); }
}).listen(PORT, '127.0.0.1', () => {
  console.log('Kit terrain R0 :');
  console.log(`  S2      http://127.0.0.1:${PORT}/spikes/terrain-r0/ligne-104.html`);
  console.log(`  S1 (du) http://127.0.0.1:${PORT}/spikes/terrain-r0/volet-lecture.html`);
  console.log('Ctrl+C pour arreter.');
});
