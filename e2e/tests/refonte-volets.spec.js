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
  await page.evaluate(() => localStorage.removeItem('wind-volets'));
  await page.reload();
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
});

test.afterAll(async () => {
  await page
    .evaluate(() => localStorage.removeItem('wind-volets'))
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
  await expect(page.locator('[data-testid="conversation-sujet"]')).toContainText(
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
  await expect(page.locator('[data-testid="conversation-sujet"]')).toContainText(
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
  await page.locator('[data-testid="conv-supprimer"]').click();
  await expect(page.locator('[data-testid="conversation"]')).toHaveCount(0);
  await dossier('corbeille').click();
  const echo = page.locator('[data-testid="ligne"]', { hasText: 'Facture 2026-0841' });
  await echo.click();
  await expect(page.locator('[data-testid="conversation-sujet"]')).toContainText(
    'Facture 2026-0841',
  );
  await expect(page.locator('[data-testid="message-deplie"]')).toHaveCount(1);
  // Un geste sur l'écho attend la réconciliation — et le dit, ici
  // aussi ; le retour à la boîte est joué par le câblage existant.
  await page.locator('[data-testid="conv-supprimer"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText(
    'Copie en cours de synchronisation',
  );
  await expect(page.locator('[data-testid="conversation"]')).toHaveCount(0);
  await dossier('reception').click();
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
  await expect(page.locator('[data-testid="lecture-sujet"]')).toContainText(
    'Relecture du contrat Vantis',
  );
  await expect(page.locator('[data-testid="conversation"]')).toHaveCount(0);
});
