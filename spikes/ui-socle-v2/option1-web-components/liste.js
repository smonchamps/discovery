// Option ① — liste virtualisée en vanilla, zéro dépendance. Fenêtrage
// manuel : seules les lignes visibles (+ overscan) existent dans le DOM.
// Construction impérative par textContent (jamais innerHTML) — l'invariant
// de sécurité de l'app réelle.
import { TOTAL, enveloppe } from '../commun/donnees.js';
import { lancer } from '../commun/rapporter.js';

const ROW = 112;      // hauteur fixe (ligne 104 + gap 8)
const OVER = 8;       // overscan
const VISIBLE = Math.ceil((globalThis.innerHeight || 1000) / ROW);

const app = document.getElementById('app');
const espace = document.createElement('div');
espace.className = 'espace';
espace.style.height = TOTAL * ROW + 'px';
const rows = document.createElement('div');
rows.className = 'rows';
espace.appendChild(rows);
app.appendChild(espace);

function fabriquer(env) {
  const a = document.createElement('article');
  a.className = 'ligne' + (env.nonlu ? ' nonlu' : '');

  const re = document.createElement('div');
  re.className = 'rangee-exp';
  const ex = document.createElement('span');
  ex.className = 'exp';
  ex.textContent = env.exp;
  const he = document.createElement('span');
  he.className = 'heure';
  he.textContent = env.heure;
  re.append(ex, he);

  const ob = document.createElement('h3');
  ob.className = 'objet';
  ob.textContent = env.objet;

  const ap = document.createElement('p');
  ap.className = 'apercu';
  ap.textContent = env.apercu;

  a.append(re, ob, ap);

  if (env.messages || env.fichiers) {
    const pc = document.createElement('div');
    pc.className = 'puces';
    if (env.messages) {
      const s = document.createElement('span');
      s.className = 'puce';
      s.textContent = env.messages + ' messages';
      pc.appendChild(s);
    }
    if (env.fichiers) {
      const s = document.createElement('span');
      s.className = 'puce';
      s.textContent = env.fichiers + ' fichiers';
      pc.appendChild(s);
    }
    a.appendChild(pc);
  }
  return a;
}

function fenetre(premier) {
  const debut = Math.max(0, premier - OVER);
  const fin = Math.min(TOTAL, premier + VISIBLE + OVER);
  rows.style.transform = 'translateY(' + debut * ROW + 'px)';
  const frag = document.createDocumentFragment();
  for (let i = debut; i < fin; i++) frag.appendChild(fabriquer(enveloppe(i)));
  rows.replaceChildren(frag);
}

const t0 = performance.now();
fenetre(0);
const premierRendu = performance.now() - t0;

window.__spike = {
  id: 'option1',
  total: TOTAL,
  premierRendu,
  aller(index) { fenetre(index); },
  theme(nom) { document.documentElement.dataset.theme = nom; },
  reflow() { return app.offsetHeight; },
};

lancer(window.__spike);
