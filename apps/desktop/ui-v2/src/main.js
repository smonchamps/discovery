// Montage v2 — thème ET langue restaurés AVANT le premier rendu (pas
// de flash ; la langue est une lecture de base locale, sub-milliseconde),
// crochets de mesure exposés pour le banc P1 (mesure-v2.mjs) et les e2e.
import './systeme.css';
import { mount } from 'svelte';
import { appliquerTheme, restaurerTheme, THEMES } from './lib/theme.js';
import { restaurerLangue } from './lib/texte.svelte.js';
import App from './App.svelte';

restaurerTheme();
await restaurerLangue();

const app = mount(App, { target: document.getElementById('app') });

// Démarrage : première page visible -> #perf porte data-startup, comme
// en v1 — le banc attend ce signal.
const attente = setInterval(() => {
  const { liste } = app.api();
  if (liste && liste.etat().premierePageMs !== null) {
    clearInterval(attente);
    app.marquerDemarrage();
  }
}, 16);

// Banc de mesure : page (saut + service + rendu, reflow forcé), thème
// (bascule à chaud), ouverture (message_body -> iframe).
window.__mesure = {
  themes: THEMES,
  etat() {
    const { liste, lecture } = app.api();
    return { ...liste.etat(), ...lecture.etat() };
  },
  async page(index) {
    const { liste } = app.api();
    return liste.allerEtServir(index);
  },
  theme(nom) {
    const t0 = performance.now();
    appliquerTheme(nom);
    void document.body.offsetHeight;
    return performance.now() - t0;
  },
  async ouvrir(index) {
    const { liste, lecture } = app.api();
    await liste.allerEtServir(index);
    const ligne = liste.ligneA(index);
    if (!ligne) throw new Error(`aucune ligne servie à l'index ${index}`);
    return lecture.ouvrir(ligne);
  },
};
