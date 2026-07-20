// Parcours de recherche plein-texte (tâche #31-#33) : raccourci `/`,
// saisie avec debounce, résultats de la boîte unifiée, ouverture d'un
// message, puis Échap pour revenir à la liste.
import { test, expect } from '@playwright/test';
import { launchApp, closeApp } from '../launch.mjs';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });

test.beforeAll(async () => {
  ({ app, browser, page } = await launchApp({ messages: 200 }));
});

test.afterAll(async () => {
  await closeApp({ app, browser });
});

test('recherche : / ouvre le champ, une saisie trouve des résultats', async () => {
  await expect(page.locator('#search')).toBeHidden();

  await page.keyboard.press('/');

  await expect(page.locator('#search')).toBeVisible();
  await page.locator('#search').fill('facture');

  // Le debounce côté UI déclenche la recherche après ~150 ms.
  await expect(page.locator('#search-results .row').first()).toBeVisible({ timeout: 5_000 });
  await expect(page.locator('#search-results .row').first()).toContainText('facture');
});

test("recherche : ouvrir un résultat affiche le message, Échap revient à l'unifiée", async () => {
  await page.locator('#search-results .row').first().click();

  await expect(page.locator('#detail')).toBeVisible();
  await expect(page.locator('#detail-subject')).toContainText('facture');

  await page.keyboard.press('Escape');

  await expect(page.locator('#search')).toBeHidden();
  await expect(page.locator('#search-results')).toBeHidden();
  await expect(page.locator('#scroll-space')).toBeVisible();
  await expect(page.locator('#perf')).toContainText('200 messages');
});

test('recherche : archiver un résultat le retire des résultats (régression #4)', async () => {
  await page.keyboard.press('/');
  await page.locator('#search').fill('facture');
  const results = page.locator('#search-results .row');
  await expect(results.first()).toBeVisible({ timeout: 5_000 });

  const before = await results.count();
  const archived = await results.first().locator('.subject').textContent();

  // Ouvrir le premier résultat, puis l'archiver (raccourci e).
  await results.first().click();
  await expect(page.locator('#detail')).toBeVisible();
  await page.keyboard.press('e');

  // Le message archivé disparaît des résultats, sans quitter la recherche.
  await expect(page.locator('#search')).toBeVisible();
  await expect(results).toHaveCount(before - 1);
  await expect(page.locator('#search-results')).not.toContainText(archived);
});
