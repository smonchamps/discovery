// Banc E4 (PLAN-RETOURS-V3 R4) : mesure l'épaisseur RÉSERVÉE par les
// barres de défilement dans l'app réelle, lancée par le harnais e2e
// avec les arguments de production (args-navigateur.mjs).
//
// Deux sondes sur un div hors écran :
// - `native` : `scrollbar-color` NON-DÉFAUT posée — Chromium bascule
//   l'élément sur le chemin standard, la barre est celle du navigateur
//   (`auto` ne suffirait pas : c'est la valeur par défaut, une règle
//   webkit éventuelle tiendrait). Overlay => 0 ; classique => ~15 px.
// - `custom` : propriété retirée — le chemin que l'app utilise
//   vraiment. DOIT rendre 0 depuis A44 : une valeur non nulle signe le
//   retour d'une règle `::-webkit-scrollbar` ou d'une barre classique,
//   la régression exacte que l'amendement interdit.
//
// Verdicts consignés (2026-08-16, PLAN-RETOURS-V3 § 2 bis) : webkit A7
// 10 px ; classique 15 px ; msOverlayScrollbarWinStyle seul 15 px (non
// honoré) ; OverlayScrollbar 0 px — adopté.
//
// Usage : node mesure-scrollbar.mjs [fluent]
//   `fluent` réessaie le style Fluent Windows PAR-DESSUS l'overlay —
//   la liste de features se réécrit ENTIÈRE : un `--enable-features`
//   répété n'est pas fusionné par Chromium, le dernier gagne.
import { launchAppV2, closeApp } from './launch.mjs';

if (process.argv.includes('fluent')) {
  process.env.WIND_E2E_ARGS_EXTRA =
    '--enable-features=OverlayScrollbar,msOverlayScrollbarWinStyle';
}

const { app, browser, page } = await launchAppV2();
try {
  const mesure = await page.evaluate(() => {
    const d = document.createElement('div');
    d.style.cssText =
      'position:fixed;left:-500px;top:0;width:100px;height:100px;' +
      'overflow:scroll;scrollbar-color:rgb(1,2,3) rgb(4,5,6);';
    document.body.appendChild(d);
    const native = d.offsetWidth - d.clientWidth;
    d.style.scrollbarColor = '';
    const custom = d.offsetWidth - d.clientWidth;
    d.remove();
    return { native, custom };
  });
  console.log(JSON.stringify(mesure));
} finally {
  await closeApp({ app, browser });
}
