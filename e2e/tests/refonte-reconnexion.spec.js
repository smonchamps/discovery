// Reconnexion d'un compte au jeton mort (constat terrain 2026-08-20) :
// `invalid_grant` (jeton expiré ou révoqué) laissait l'utilisateur
// démuni — la fente disait « Compte non reconnecté » mais aucun geste ne
// relançait le consentement. Désormais l'état se VOIT aux Réglages
// (link_off + « Déconnecté ») et se répare sur place (« Reconnecter »).
//
// Le décor : deux comptes au registre, dont un SANS session
// (`deconnecte: true` du lanceur — seedé en base, absent de
// WIND_E2E_ACCOUNT). L'isolation e2e purge la configuration OAuth : le
// consentement ne peut pas partir, ce qui rend l'ÉCHEC déterministe —
// c'est lui qu'on vérifie (dit sur place, geste rejouable) ; le succès
// exige un vrai navigateur, il appartient au terrain.
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });

test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2({
    comptes: [
      { email: 'sain@exemple.fr', messages: 4 },
      { email: 'mort@exemple.fr', messages: 3, deconnecte: true },
    ],
  }));
});

test.afterAll(async () => {
  await closeApp({ app, browser });
});

test('un jeton mort se VOIT aux Réglages — le compte sain, lui, ne change pas', async () => {
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
  await page.locator('[data-testid="reglages"]').click();

  const rangees = page.locator('[data-testid="reglages-comptes"]');
  await expect(rangees).toContainText('sain@exemple.fr');
  await expect(rangees).toContainText('mort@exemple.fr');
  // UN seul état « Déconnecté », un seul geste « Reconnecter » — sur la
  // bonne rangée.
  await expect(page.locator('[data-testid="compte-deconnecte"]')).toHaveCount(1);
  await expect(page.locator('[data-testid="compte-reconnecter"]')).toHaveCount(1);
  const morte = rangees.locator('div.compte', { hasText: 'mort@exemple.fr' });
  await expect(morte.locator('[data-testid="compte-deconnecte"]')).toContainText('Déconnecté');
  const saine = rangees.locator('div.compte', { hasText: 'sain@exemple.fr' });
  await expect(saine.locator('[data-testid="compte-deconnecte"]')).toHaveCount(0);
});

test("l'échec de reconnexion se dit SUR PLACE, et le geste se rejoue", async () => {
  await page.locator('[data-testid="compte-reconnecter"]').click();
  await expect(page.locator('[data-testid="reconnexion-erreur"]')).toBeVisible();
  // Le bouton revient : l'échec n'est pas un cul-de-sac.
  await expect(page.locator('[data-testid="compte-reconnecter"]')).toBeEnabled();
});
