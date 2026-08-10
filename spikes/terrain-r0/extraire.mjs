// Extraction TERRAIN (R0-S2 + du S1) : lit la VRAIE base en LECTURE SEULE
// et produit donnees.gen.js (gitignore — donnees reelles, jamais commitees).
//
// Zero dependance : node:sqlite (Node >= 22.5 ; la machine est en 24).
// La base n'est jamais ecrite, jamais copiee ailleurs que dans ce dossier.
import { DatabaseSync } from 'node:sqlite';
import { existsSync, writeFileSync } from 'node:fs';
import path from 'node:path';

const ICI = import.meta.dirname;
const chemin = process.env.DISCOVERY_DB_PATH
  ?? path.join(process.env.APPDATA ?? '', 'dev.discovery.app', 'discovery.db');

if (!existsSync(chemin)) {
  console.error(
    `Pas de base a ${chemin}\n` +
    `Lance d'abord l'application avec ton compte reel (voir README.md),\n` +
    `laisse la synchronisation et le rattrapage des corps tourner, puis\n` +
    `relance ce script. (Autre base : variable DISCOVERY_DB_PATH.)`);
  process.exit(1);
}

const db = new DatabaseSync(chemin, { readOnly: true });

// ---- S2 : les enveloppes recentes, une ligne par fil (comme la liste) ----
const enveloppes = db.prepare(`
  SELECT e.mailbox_id, e.uid, e.subject, e.sender, e.sender_address,
         e.thread_id, e.date_epoch, e.seen, e.flagged,
         m.name AS boite, a.email AS compte,
         (SELECT COUNT(*) FROM attachments p
           WHERE p.mailbox_id = e.mailbox_id AND p.uid = e.uid) AS pieces,
         b.html
  FROM envelopes e
  JOIN mailboxes m ON m.id = e.mailbox_id
  JOIN accounts  a ON a.id = m.account_id
  LEFT JOIN bodies b ON b.mailbox_id = e.mailbox_id AND b.uid = e.uid
  WHERE m.threaded = 1
  ORDER BY e.date_epoch DESC
  LIMIT 400`).all();

const parFil = new Map();
for (const r of db.prepare(`
  SELECT e.thread_id AS t, COUNT(*) AS n
  FROM envelopes e JOIN mailboxes m ON m.id = e.mailbox_id
  WHERE m.threaded = 1 AND e.thread_id IS NOT NULL
  GROUP BY e.thread_id`).all()) parFil.set(r.t, r.n);

// Apercu : le texte du corps, debarrasse de ses balises. Grossier mais
// suffisant pour juger la troncature — le vrai apercu viendra du coeur.
function apercu(html) {
  if (!html) return '';
  return html
    .replace(/<(style|script|head)[\s\S]*?<\/\1>/gi, ' ')
    .replace(/<!--[\s\S]*?-->/g, ' ')
    .replace(/<[^>]+>/g, ' ')
    .replace(/&nbsp;/gi, ' ').replace(/&amp;/gi, '&')
    .replace(/&lt;/gi, '<').replace(/&gt;/gi, '>').replace(/&#\d+;/g, ' ')
    .replace(/\s+/g, ' ').trim().slice(0, 220);
}

const vues = new Set();
const lignes = [];
for (const e of enveloppes) {
  const cle = e.thread_id ?? `solo-${e.mailbox_id}-${e.uid}`;
  if (vues.has(cle)) continue;
  vues.add(cle);
  lignes.push({
    sender: e.sender || e.sender_address || '(sans expediteur)',
    subject: e.subject || '(sans objet)',
    apercu: apercu(e.html),
    date_epoch: e.date_epoch, seen: !!e.seen, flagged: !!e.flagged,
    fil: e.thread_id ? (parFil.get(e.thread_id) ?? 1) : 1,
    pieces: e.pieces, boite: e.boite, compte: e.compte,
  });
  if (lignes.length >= 120) break;
}

// ---- S1 (du) : des corps reels varies pour le volet de lecture ----------
const avecCorps = enveloppes.filter((e) => e.html && e.html.trim().length > 0);
const parTaille = [...avecCorps].sort((a, b) => b.html.length - a.html.length);
const distantes = avecCorps.filter((e) => /<img[^>]+src\s*=\s*["']?https?:/i.test(e.html));
const courts = [...avecCorps].sort((a, b) => a.html.length - b.html.length);

const choisis = new Map();
const prendre = (liste, n, motif) => {
  for (const e of liste.slice(0, n)) {
    const cle = `${e.mailbox_id}-${e.uid}`;
    if (!choisis.has(cle)) choisis.set(cle, { e, motif });
  }
};
prendre(parTaille, 2, 'riche (newsletter probable)');
prendre(distantes, 2, 'images distantes');
prendre(courts, 2, 'simple');
prendre(avecCorps, 6, 'recent');

const corps = [...choisis.values()].slice(0, 12).map(({ e, motif }) => ({
  subject: e.subject || '(sans objet)', sender: e.sender || e.sender_address,
  motif, taille: e.html.length, html: e.html,
}));

const sortie = {
  extrait_le: new Date().toISOString(), base: chemin,
  total_enveloppes: enveloppes.length, lignes, corps,
};
writeFileSync(path.join(ICI, 'donnees.gen.js'),
  'const TERRAIN = ' + JSON.stringify(sortie) + ';\n');
console.log(`donnees.gen.js : ${lignes.length} lignes (S2), ` +
  `${corps.length} corps (S1) — dont ${distantes.length ? 'des' : 'AUCUN'} ` +
  `mails a images distantes dans la base.`);
