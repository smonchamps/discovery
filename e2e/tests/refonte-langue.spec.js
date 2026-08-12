// La langue de l'interface (PLAN-LANGUES, E1) : audit des catalogues
// (mêmes clés en fr et en — le repli ne doit jamais servir en
// production), bascule immédiate depuis Réglages > Affichage, balayage
// des écrans majeurs en anglais, aller-retour RÉEL (recharger relit la
// préférence en base, pas un état de composant), puis retour au
// français — la langue canonique des autres parcours (L-6).
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';
import { FR } from '../../apps/desktop/ui-v2/src/lib/catalogue.fr.js';
import { EN } from '../../apps/desktop/ui-v2/src/lib/catalogue.en.js';

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

test('les catalogues fr et en portent exactement les mêmes clés', () => {
  expect(Object.keys(EN).sort()).toEqual(Object.keys(FR).sort());
});

test('le français du prototype est la langue par défaut', async () => {
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
  await expect(page.locator('[data-testid="ecrire"]')).toContainText('Écrire');
  await expect(page.locator('html')).toHaveAttribute('lang', 'fr');
});

test('la bascule en anglais est immédiate, sans redémarrage', async () => {
  await page.locator('[data-testid="reglages"]').click();
  await page.locator('[data-testid="reglages-groupe"][data-groupe="affichage"]').click();
  const choix = page.locator('[data-testid="affichage-langue"]');
  await expect(choix).toHaveValue('fr');
  await choix.selectOption('en');
  // La surimpression elle-même a déjà basculé…
  await expect(page.locator('[data-testid="reglages-termine"]')).toHaveText('Done');
  await page.locator('[data-testid="reglages-termine"]').click();
  // …et les écrans majeurs aussi : entête, nav, onglets, volet, langue
  // du document (lecteurs d'écran).
  await expect(page.locator('[data-testid="ecrire"]')).toContainText('Compose');
  await expect(page.locator('[data-testid="nav-dossier"][data-categorie="reception"]')).toContainText('Inbox');
  await expect(page.locator('[data-testid="nav-dossier"][data-categorie="corbeille"]')).toContainText('Trash');
  await expect(page.locator('[data-testid="onglet"][data-onglet="nonlus"]')).toContainText('Unread');
  await expect(page.locator('[data-testid="volet-lecture"]')).toContainText('Select a message to read it.');
  await expect(page.locator('html')).toHaveAttribute('lang', 'en');
});

test("l'aller-retour est réel : recharger relit la préférence en base", async () => {
  await page.reload();
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
  await expect(page.locator('[data-testid="ecrire"]')).toContainText('Compose');
  await expect(page.locator('html')).toHaveAttribute('lang', 'en');
});

test('le retour au français rétablit les formes exactes du prototype', async () => {
  await page.locator('[data-testid="reglages"]').click();
  await page.locator('[data-testid="reglages-groupe"][data-groupe="affichage"]').click();
  await page.locator('[data-testid="affichage-langue"]').selectOption('fr');
  await expect(page.locator('[data-testid="reglages-termine"]')).toHaveText('Terminé');
  await page.locator('[data-testid="reglages-termine"]').click();
  await expect(page.locator('[data-testid="ecrire"]')).toContainText('Écrire');
  await expect(page.locator('[data-testid="volet-lecture"]')).toContainText('Sélectionnez un message pour le lire.');
  await expect(page.locator('html')).toHaveAttribute('lang', 'fr');
});
