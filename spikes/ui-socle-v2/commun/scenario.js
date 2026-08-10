// Le scénario de mesure, identique pour ①, ② et ③. Il opère sur une
// « cible » que chaque option implémente — c'est la seule chose qui
// diffère d'une famille à l'autre :
//
//   cible.total          nombre total de lignes (256 312)
//   cible.premierRendu   ms pour peindre la première fenêtre (mesuré à l'init)
//   cible.aller(index)   rend SYNCHRONEMENT la fenêtre commençant à `index`
//                        (vanilla : direct ; Svelte : flushSync)
//   cible.theme(nom)     applique SYNCHRONEMENT 'nature' | 'nuit'
//   cible.reflow()       force une lecture de layout (inclut le restyle)
//
// On mesure le travail réel « produire la prochaine page de liste » et
// « basculer le thème », layout forcé compris — pas une frame planifiée.

function pct(xs, p) {
  const t = [...xs].sort((a, b) => a - b);
  const i = Math.min(t.length - 1, Math.floor((p / 100) * t.length));
  return Math.round(t[i] * 1000) / 1000;
}
const arrondi = (x, n = 3) => Math.round(x * 10 ** n) / 10 ** n;

export function mesurer(cible, opts = {}) {
  const nPages = opts.pages ?? 300;
  const nThemes = opts.themes ?? 60;
  const total = cible.total;

  // 300 profondeurs réparties sur TOUT le corpus : la profondeur fait
  // partie du test (OFFSET profond était un report assumé de l'app).
  const dPage = [];
  for (let k = 0; k < nPages; k++) {
    const index = Math.floor((k / nPages) * (total - 40));
    const t0 = performance.now();
    cible.aller(index);
    cible.reflow();
    dPage.push(performance.now() - t0);
  }

  const dTheme = [];
  for (let k = 0; k < nThemes; k++) {
    const nom = k % 2 ? 'nuit' : 'nature';
    const t0 = performance.now();
    cible.theme(nom);
    cible.reflow();
    dTheme.push(performance.now() - t0);
  }
  cible.theme('nature');

  const heap = (performance.memory && performance.memory.usedJSHeapSize) || 0;
  return {
    premierRenduMs: arrondi(cible.premierRendu),
    pageP50Ms: pct(dPage, 50),
    pageP95Ms: pct(dPage, 95),
    pageMaxMs: pct(dPage, 100),
    themeP50Ms: pct(dTheme, 50),
    themeP95Ms: pct(dTheme, 95),
    tasJsMo: arrondi(heap / 1048576, 2),
  };
}
