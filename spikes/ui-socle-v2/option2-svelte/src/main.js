// Option ② — montage Svelte 5. Même contrat `window.__spike` que ① ;
// les mises à jour sont forcées synchrones par flushSync pour mesurer le
// travail réel de patch, pas une frame planifiée.
import '../../commun/systeme.css';
import { mount, flushSync } from 'svelte';
import { TOTAL } from '../../commun/donnees.js';
import { lancer } from '../../commun/rapporter.js';
import Liste from './Liste.svelte';

const app = document.getElementById('app');
app.className = 'liste-cadre';

const t0 = performance.now();
const comp = mount(Liste, { target: app });
flushSync();
const premierRendu = performance.now() - t0;

window.__spike = {
  id: 'option2',
  total: TOTAL,
  premierRendu,
  aller(index) { flushSync(() => comp.aller(index)); },
  theme(nom) { document.documentElement.dataset.theme = nom; },
  reflow() { return app.offsetHeight; },
};

lancer(window.__spike);
