// La boîte unifiée à DEUX comptes — le cœur produit de la Phase 3 :
// fusion par date, chaque ligne porte la pastille de son compte, et
// répondre émet depuis le compte du message d'origine.
import { test, expect } from '@playwright/test';
import { launchApp, closeApp } from '../launch.mjs';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });

test.beforeAll(async () => {
  ({ app, browser, page } = await launchApp({
    accounts: [
      { email: 'un@exemple.fr', messages: 30 },
      { email: 'deux@exemple.fr', messages: 20 },
    ],
  }));
});

test.afterAll(async () => {
  await closeApp({ app, browser });
});

test('boîte unifiée : les deux comptes fusionnés, pastilles visibles', async () => {
  await expect(page.locator('.account-chip')).toHaveCount(2);
  await expect(page.locator('#perf')).toContainText('50 messages');
  await expect(page.locator('.row').first()).toBeVisible();
  await expect(page.locator('.row .account-dot').first()).toBeVisible();
});

test("répondre depuis la boîte unifiée : le compte du message est l'émetteur", async () => {
  // Le plus récent (n°30) appartient au premier compte seedé.
  await page.locator('.row').first().click();
  await page.keyboard.press('r');

  await expect(page.locator('#compose')).toBeVisible();
  await expect(page.locator('#compose-from-row')).toBeVisible();
  const from = await page
    .locator('#compose-from')
    .evaluate((select) => select.selectedOptions[0].textContent);
  expect(from).toBe('un@exemple.fr');
});
