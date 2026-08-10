// Bootstrap partagé : chaque page, une fois sa cible prête, laisse la
// première fenêtre se peindre (double rAF), lance le scénario, puis POST
// le résultat au harnais. Aucune dépendance : la page se mesure elle-même.
import { mesurer } from './scenario.js';

export async function lancer(cible) {
  await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
  const res = mesurer(cible, { pages: 300, themes: 60 });
  res.id = cible.id;
  try {
    await fetch('/resultat', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(res),
    });
  } catch (e) {
    // Ne rien avaler : l'exposer dans le titre pour que le harnais le voie.
    document.title = 'SPIKE_ERREUR:' + e;
    return;
  }
  window.__spikeResultat = res;
  document.title = 'SPIKE_DONE';
}
