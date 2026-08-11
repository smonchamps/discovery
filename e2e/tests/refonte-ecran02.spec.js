// L'écran 02 de la refonte (PLAN-UI-V2 §P2), joué sur le décor Clarity :
// nav réelle, onglets filtrés côté coeur, volet de lecture, action
// réelle. Le fichier est nommé pour passer APRÈS les parcours v1
// (ordre alphabétique) : une seule reconstruction d'assets par gate.
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });

test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2());
});

test.afterAll(async () => {
  await closeApp({ app, browser });
});

const dossier = (categorie) =>
  page.locator(`[data-testid="nav-dossier"][data-categorie="${categorie}"]`);

test('la nav porte les compteurs du décor Clarity', async () => {
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
  await expect(dossier('reception')).toContainText('4');
  await expect(dossier('reception')).toContainText('/ 18');
  await expect(dossier('envoyes')).toContainText('12');
  await expect(dossier('brouillons')).toContainText('2');
  await expect(dossier('indesirables')).toContainText('/ 3');
  await expect(dossier('archives')).toContainText('64');
  await expect(dossier('corbeille')).toContainText('3');
  // Boîtes : l'agrégée + un rang par compte RÉEL.
  await expect(page.locator('[data-testid="nav-boite"]')).toHaveCount(3);
});

test('sélectionner ouvre le volet, lit le corps, et le non-lu tombe', async () => {
  await page.locator('[data-testid="ligne"]').first().click();
  await expect(page.locator('[data-testid="lecture-sujet"]')).toHaveText(
    'Relecture du contrat Vantis',
  );
  // Le corps vit dans l'iframe sandbox — invariant S1.
  await expect(
    page.frameLocator('[data-testid="volet-lecture"] iframe').locator('body'),
  ).toContainText('Bonjour Paul');
  // mark_seen est RÉEL : le héros de la réception retombe.
  await expect(dossier('reception')).toContainText('3');
});

test("l'onglet Non lus filtre côté coeur", async () => {
  await page.locator('[data-onglet="nonlus"]').click();
  await expect(page.locator('[data-testid="ligne"]')).toHaveCount(3);
  await page.locator('[data-onglet="tous"]').click();
  await expect(page.locator('[data-testid="ligne"]').nth(4)).toBeVisible();
});

test('les dossiers canoniques servent leurs listes', async () => {
  await dossier('archives').click();
  await expect(page.locator('[data-testid="statut"]')).toContainText(
    'Archives · 64 éléments',
  );
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
  await dossier('corbeille').click();
  await expect(page.locator('[data-testid="statut"]')).toContainText(
    'Corbeille · 3 éléments',
  );
  await dossier('reception').click();
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
});

test("la Boîte d'un compte borne la liste", async () => {
  await page.locator('[data-testid="nav-boite"]').nth(2).click();
  await expect(page.locator('[data-testid="ligne"]')).toHaveCount(2);
  await page.locator('[data-testid="nav-boite"]').first().click();
  await expect(page.locator('[data-testid="ligne"]').nth(4)).toBeVisible();
});

test('archiver agit sur le coeur et confirme par le toast', async () => {
  await page.locator('[data-testid="ligne"]').nth(1).click();
  await page.locator('[data-testid="archiver"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText(
    'Conversation archivée.',
  );
  await expect(dossier('reception')).toContainText('/ 17');
});
