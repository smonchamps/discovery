// L'écran 01 de la refonte (PLAN-UI-V2 §P4, D4) : à ZÉRO compte,
// l'application accueille par « Votre adresse. C'est tout. » — base
// vierge, aucun compte factice. Lancement séparé : l'état zéro compte
// ne peut pas se jouer sur le décor Clarity.
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });

test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2({ vierge: true }));
});

test.afterAll(async () => {
  await closeApp({ app, browser });
});

test("à zéro compte, l'écran 01 accueille", async () => {
  await expect(page.locator('[data-testid="onboarding"]')).toBeVisible();
  await expect(page.locator('[data-testid="onboarding"]')).toContainText(
    'Votre adresse.',
  );
  await expect(page.locator('[data-testid="onboarding"]')).toContainText(
    'Le serveur est détecté automatiquement.',
  );
});

test('une saisie invalide est refusée sur place', async () => {
  await page.locator('[data-testid="onboarding-adresse"]').fill('pas-une-adresse');
  await page.locator('[data-testid="onboarding-continuer"]').click();
  await expect(page.locator('[data-testid="onboarding-erreur"]')).toContainText(
    'adresse e-mail complète',
  );
});

test('un domaine inconnu révèle le guichet IMAP/SMTP, serveurs proposés', async () => {
  await page.locator('[data-testid="onboarding-adresse"]').fill('paul@exemple.fr');
  await page.locator('[data-testid="onboarding-continuer"]').click();
  await expect(page.locator('#ob-imap')).toHaveValue('imap.exemple.fr');
  await expect(page.locator('#ob-smtp')).toHaveValue('smtp.exemple.fr');
  // Rien n'est parti : pas d'erreur de connexion, le formulaire attend.
  await expect(page.locator('[data-testid="onboarding-erreur"]')).toHaveCount(0);
});
