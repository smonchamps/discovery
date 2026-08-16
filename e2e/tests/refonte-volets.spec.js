// Les volets d'affichage (PLAN-VOLETS E1) : la Disposition se choisit
// dans Réglages > Affichage — trois volets (défaut, la grille de
// l'écran 02) ou deux volets (liste pleine largeur, ouverture en plein
// écran par l'écran 03, message seul compris). Joué sur le décor
// Clarity.
//
// Hygiène : le profil WebView2 est PARTAGÉ entre les suites et les
// gates — la préférence `wind-volets` est retirée avant ET après, pour
// que ni un run interrompu ni cette suite ne poussent les autres
// écrans hors du défaut.
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });

test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2());
  // Un run précédent interrompu a pu laisser un mode : repartir du
  // défaut AVANT toute assertion.
  await page.evaluate(() => {
    localStorage.removeItem('wind-volets');
    localStorage.removeItem('wind-largeurs');
  });
  await page.reload();
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
});

test.afterAll(async () => {
  await page
    .evaluate(() => {
      localStorage.removeItem('wind-volets');
      localStorage.removeItem('wind-largeurs');
    })
    .catch(() => { /* la fenêtre est peut-être déjà morte : le beforeAll des autres suites ne lit pas ce profil-là */ });
  await closeApp({ app, browser });
});

const dossier = (categorie) =>
  page.locator(`[data-testid="nav-dossier"][data-categorie="${categorie}"]`);

test('le défaut est trois volets — le volet de lecture est là, rien à régler', async () => {
  await expect(page.locator('[data-testid="volet-lecture"]')).toBeVisible();
  await page.locator('[data-testid="reglages"]').click();
  await page
    .locator('[data-testid="reglages-groupe"][data-groupe="affichage"]')
    .click();
  await expect(page.locator('[data-testid="affichage-volets"]')).toHaveValue('3');
  await page.locator('[data-testid="reglages-termine"]').click();
});

test('la bascule en deux volets est immédiate — la lecture quitte la grille', async () => {
  await page.locator('[data-testid="reglages"]').click();
  await page
    .locator('[data-testid="reglages-groupe"][data-groupe="affichage"]')
    .click();
  await page.locator('[data-testid="affichage-volets"]').selectOption('2');
  // Application immédiate, sans confirmation (le geste du thème) : le
  // volet disparaît PENDANT que la surimpression est encore ouverte.
  await expect(page.locator('[data-testid="volet-lecture"]')).toHaveCount(0);
  await page.locator('[data-testid="reglages-termine"]').click();
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
});

test("en deux volets, l'ouverture est l'écran 03 — Échap rend la liste intacte", async () => {
  const lignes = page.locator('[data-testid="ligne"]');
  const avant = await lignes.count();
  await lignes.filter({ hasText: 'Relecture du contrat Vantis' }).first().click();
  // Plein écran : la conversation, PAS le volet.
  await expect(page.locator('[data-testid="fil-sujet"]')).toContainText(
    'Relecture du contrat Vantis',
  );
  await expect(page.locator('[data-testid="volet-lecture"]')).toHaveCount(0);
  await page.keyboard.press('Escape');
  await expect(page.locator('[data-testid="conversation"]')).toHaveCount(0);
  // La liste est INTACTE : mêmes lignes, la sélection tient.
  await expect(lignes).toHaveCount(avant);
  await expect(
    lignes.filter({ hasText: 'Relecture du contrat Vantis' }).first(),
  ).toHaveClass(/choisie/);
});

test("un message SANS fil s'ouvre en plein écran — le repli du message seul (V-D2)", async () => {
  // « Compte rendu du 4 août » : message seul du décor (aucune
  // conversation à ouvrir — le test Annexe A l'affirme au volet).
  await page
    .locator('[data-testid="ligne"]', { hasText: 'Compte rendu du 4 août' })
    .click();
  await expect(page.locator('[data-testid="fil-sujet"]')).toContainText(
    'Compte rendu du 4 août',
  );
  // Le fil servi est la ligne elle-même : UN message, déplié, avec ses
  // fichiers réels (message_attachments).
  await expect(page.locator('[data-testid="message-deplie"]')).toHaveCount(1);
  await expect(
    page.locator('[data-testid="conversation"] [data-testid="piece-jointe"]'),
  ).toContainText('CR_04-08.pdf');
  await page.locator('[data-testid="retour-boite"]').click();
  await expect(page.locator('[data-testid="conversation"]')).toHaveCount(0);
});

test("un écho local s'ouvre en plein écran — corps local, geste différé dit (V-D2)", async () => {
  // Le contrat hors ligne d'E3 (PLAN-REACTIVITE), rejoué en deux
  // volets : supprimer → écho en Corbeille → l'ouverture passe par
  // l'écran 03 et sert echo_body.
  await page
    .locator('[data-testid="ligne"]', { hasText: 'Facture 2026-0841' })
    .first()
    .click();
  await page.locator('[data-testid="supprimer"]').click();
  await expect(page.locator('[data-testid="conversation"]')).toHaveCount(0);
  await dossier('corbeille').click();
  const echo = page.locator('[data-testid="ligne"]', { hasText: 'Facture 2026-0841' });
  await echo.click();
  await expect(page.locator('[data-testid="fil-sujet"]')).toContainText(
    'Facture 2026-0841',
  );
  await expect(page.locator('[data-testid="message-deplie"]')).toHaveCount(1);
  // Un geste sur l'écho attend la réconciliation — et le dit, ici
  // aussi ; le retour à la boîte est joué par le câblage existant.
  await page.locator('[data-testid="supprimer"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText(
    'Copie en cours de synchronisation',
  );
  await expect(page.locator('[data-testid="conversation"]')).toHaveCount(0);
  await dossier('reception').click();
});

test('en un volet, la nav quitte la grille et vit en tiroir (E2)', async () => {
  await page.locator('[data-testid="reglages"]').click();
  await page
    .locator('[data-testid="reglages-groupe"][data-groupe="affichage"]')
    .click();
  await page.locator('[data-testid="affichage-volets"]').selectOption('1');
  await page.locator('[data-testid="reglages-termine"]').click();
  // La nav n'est plus dans la grille ; le bouton du tiroir est là.
  await expect(page.locator('[data-testid="nav"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="btn-tiroir"]')).toBeVisible();
  // Ouvrir : la Nav est LA MÊME — dossiers réels, compteurs réels.
  await page.locator('[data-testid="btn-tiroir"]').click();
  await expect(page.locator('[data-testid="tiroir"]')).toBeVisible();
  await expect(dossier('corbeille')).toBeVisible();
  // Choisir ferme ET filtre — le geste accompli n'a plus besoin du
  // panneau.
  await dossier('corbeille').click();
  await expect(page.locator('[data-testid="tiroir"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="statut"]')).toContainText('Corbeille');
});

test('Échap ferme le tiroir ; quitter le mode un volet l\'emporte', async () => {
  await page.locator('[data-testid="btn-tiroir"]').click();
  await expect(page.locator('[data-testid="tiroir"]')).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page.locator('[data-testid="tiroir"]')).toHaveCount(0);
  // Retour à la réception par le tiroir, puis au mode deux volets —
  // le bouton du tiroir s'efface avec le mode, la nav revient en
  // grille (la suite continue sur le mode 2, que le test de
  // persistance attend).
  await page.locator('[data-testid="btn-tiroir"]').click();
  await dossier('reception').click();
  await page.locator('[data-testid="reglages"]').click();
  await page
    .locator('[data-testid="reglages-groupe"][data-groupe="affichage"]')
    .click();
  await page.locator('[data-testid="affichage-volets"]').selectOption('2');
  await page.locator('[data-testid="reglages-termine"]').click();
  await expect(page.locator('[data-testid="btn-tiroir"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="nav"]')).toBeVisible();
});

test('la préférence survit au relancement — et le retour à trois volets restaure le volet', async () => {
  // Persistance : la page rechargée restaure le mode 2 AVANT le
  // premier rendu (pas de flash de grille).
  await page.reload();
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
  await expect(page.locator('[data-testid="volet-lecture"]')).toHaveCount(0);
  // Retour à trois volets : le volet revient, la valeur est relue.
  await page.locator('[data-testid="reglages"]').click();
  await page
    .locator('[data-testid="reglages-groupe"][data-groupe="affichage"]')
    .click();
  await expect(page.locator('[data-testid="affichage-volets"]')).toHaveValue('2');
  await page.locator('[data-testid="affichage-volets"]').selectOption('3');
  await expect(page.locator('[data-testid="volet-lecture"]')).toBeVisible();
  await page.locator('[data-testid="reglages-termine"]').click();
  // En trois volets, le clic ouvre DANS le volet — pas de plein écran.
  await page
    .locator('[data-testid="ligne"]', { hasText: 'Relecture du contrat Vantis' })
    .first()
    .click();
  await expect(page.locator('[data-testid="fil-sujet"]')).toContainText(
    'Relecture du contrat Vantis',
  );
  await expect(page.locator('[data-testid="conversation"]')).toHaveCount(0);
});

test('les volets se redimensionnent à la souris — bornes, persistance, double-clic (PLAN-RETOURS-V3 R3)', async () => {
  // Verdict CE 2026-08-16 (D3) : poignées sur les DEUX frontières en
  // trois volets ; bornes nav 180-400, liste 300-640 ; largeurs
  // persistées ; double-clic = retour au défaut.
  const largeur = (testid) =>
    page
      .locator(`[data-testid="${testid}"]`)
      .evaluate((el) => Math.round(el.getBoundingClientRect().width));
  const saisir = async (testid, dx) => {
    const boite = await page.locator(`[data-testid="${testid}"]`).boundingBox();
    const x = boite.x + boite.width / 2;
    const y = boite.y + boite.height / 2;
    await page.mouse.move(x, y);
    await page.mouse.down();
    await page.mouse.move(x + dx, y, { steps: 4 });
    await page.mouse.up();
  };

  expect(await largeur('liste')).toBe(400);
  // La nav d'abord, bornée en bas à 180 — elle libère le plafond de la
  // liste (fenêtre 1000 : 1000 - 180 - 120 de réserve du fil = 700).
  await saisir('poignee-nav', -500);
  expect(await largeur('nav')).toBe(180);
  await saisir('poignee-liste', 120);
  expect(await largeur('liste')).toBe(520);
  // La borne haute retient la poignée : 640, jamais au-delà.
  await saisir('poignee-liste', 500);
  expect(await largeur('liste')).toBe(640);
  // Le PLAFOND de la fenêtre retient l'autre frontière (revue
  // 2026-08-16) : nav max 400 écraserait le fil sous sa réserve —
  // 1000 - 640 - 120 = 240, jamais au-delà, la poignée liste reste
  // saisissable à l'écran.
  await saisir('poignee-nav', 500);
  expect(await largeur('nav')).toBe(240);

  // Persistance : la page rechargée restaure les largeurs AVANT le
  // premier rendu — écrites au RELÂCHEMENT, jamais par pointermove.
  await page.reload();
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
  expect(await largeur('liste')).toBe(640);
  expect(await largeur('nav')).toBe(240);

  // Double-clic : chaque frontière rend son défaut.
  await page.locator('[data-testid="poignee-liste"]').dblclick();
  expect(await largeur('liste')).toBe(400);
  await page.locator('[data-testid="poignee-nav"]').dblclick();
  expect(await largeur('nav')).toBe(248);
});
