// Captures d'écran RÉELLES des trois dispositions (terrain 2026-08-22,
// constat 5 de PLAN-RETOURS-8 R2) : l'étape « disposition » du parcours
// d'accueil montre l'application elle-même (décor Clarity, données
// factices), jamais un dessin approché. À REJOUER quand l'écran 02
// change de visage, puis committer les PNG régénérés :
//
//   node capture-accueil.mjs        (depuis e2e/)
//
// Écrit apps/desktop/ui-v2/src/assets/accueil/disposition-{3,2,1}.png
// (importés par Onboarding.svelte, embarqués par Vite).
import { mkdirSync } from 'node:fs';
import path from 'node:path';
import { launchAppV2, closeApp } from './launch.mjs';

const dossier = path.resolve(
  import.meta.dirname,
  '..',
  'apps',
  'desktop',
  'ui-v2',
  'src',
  'assets',
  'accueil',
);
mkdirSync(dossier, { recursive: true });

const { app, browser, page } = await launchAppV2();
try {
  for (const volets of [3, 2, 1]) {
    await page.evaluate((v) => {
      localStorage.setItem('wind-volets', String(v));
      localStorage.setItem('wind-accueil-fait', '1');
    }, volets);
    await page.reload();
    await page.locator('[data-testid="ligne"]').first().waitFor();
    if (volets === 3) {
      // Le volet de lecture montre un message ouvert — l'aperçu dit la
      // vraie fenêtre de travail, pas une colonne vide.
      await page.locator('[data-testid="ligne"]').first().click();
      await page.locator('[data-testid="volet-lecture"]').waitFor();
    }
    // Peinture posée (polices, hitofude) avant la photo.
    await page.waitForTimeout(500);
    // Recadrée AU-DESSUS de la barre d'état : les comptes factices ne
    // synchronisent pas, et « Synchronisation impossible » n'a rien à
    // faire dans un aperçu d'accueil.
    const statut = await page.locator('[data-testid="statut"]').boundingBox();
    const vue = await page.evaluate(() => ({
      largeur: window.innerWidth,
      hauteur: window.innerHeight,
    }));
    await page.screenshot({
      path: path.join(dossier, `disposition-${volets}.png`),
      clip: { x: 0, y: 0, width: vue.largeur, height: statut?.y ?? vue.hauteur },
    });
    console.log(`disposition-${volets}.png capturée`);
  }
} finally {
  await page
    .evaluate(() => {
      localStorage.removeItem('wind-volets');
      localStorage.removeItem('wind-accueil-fait');
    })
    .catch(() => {});
  await closeApp({ app, browser });
}
