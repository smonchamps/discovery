// Retrait d'un compte depuis les Réglages : le geste est destructeur
// localement (courrier local effacé, connexion oubliée — jamais le
// serveur), donc il se CONFIRME sur place, et tout ce qui montrait le
// compte se replie : rangée des Réglages, boîte de la nav, liste.
//
// Suite à part : elle MUTILE son décor (un compte disparaît pour de
// bon) — la partager avec une autre suite sérialisée ferait dépendre
// leurs assertions de l'ordre d'exécution.
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });

test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2({
    comptes: [
      { email: 'un@exemple.fr', messages: 6 },
      { email: 'deux@exemple.fr', messages: 4 },
    ],
  }));
});

test.afterAll(async () => {
  await closeApp({ app, browser });
});

test("le retrait se confirme — et l'annulation ne touche à rien", async () => {
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
  // 6 + 4 messages, un fil sur cinq (uid 5 répond au 4) : 5 + 4 conversations.
  await expect(
    page.locator('[data-testid="nav-dossier"][data-categorie="reception"]'),
  ).toContainText('9');
  await page.locator('[data-testid="reglages"]').click();
  await expect(page.locator('[data-testid="compte-retirer"]')).toHaveCount(2);

  // Premier clic : la carte de confirmation, pas le retrait.
  await page.locator('[data-testid="compte-retirer"]').first().click();
  await expect(page.locator('[data-testid="reglages-retrait"]')).toBeVisible();
  await expect(page.locator('[data-testid="reglages-retrait"]')).toContainText(
    'un@exemple.fr',
  );
  await expect(page.locator('[data-testid="reglages-retrait"]')).toContainText(
    'Rien n’est supprimé sur le serveur',
  );

  // Annuler : la carte se replie, les deux comptes sont toujours là.
  await page.locator('[data-testid="retrait-annuler"]').click();
  await expect(page.locator('[data-testid="reglages-retrait"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="compte-retirer"]')).toHaveCount(2);
  await page.locator('[data-testid="reglages-termine"]').click();
});

test('confirmé : le compte quitte les Réglages, la nav et la liste', async () => {
  await page.locator('[data-testid="reglages"]').click();

  // Le second compte (deux@exemple.fr) part ; le premier reste.
  await page.locator('[data-testid="compte-retirer"]').nth(1).click();
  await expect(page.locator('[data-testid="reglages-retrait"]')).toContainText(
    'deux@exemple.fr',
  );
  await page.locator('[data-testid="retrait-confirmer"]').click();

  await expect(page.locator('[data-testid="toast"]')).toContainText('Compte retiré.');
  await expect(page.locator('[data-testid="compte-retirer"]')).toHaveCount(1);
  await expect(page.locator('[data-testid="reglages-comptes"]')).not.toContainText(
    'deux@exemple.fr',
  );
  await page.locator('[data-testid="reglages-termine"]').click();

  // La nav : « Toutes les boîtes » + le seul compte restant.
  await expect(page.locator('[data-testid="nav-boite"]')).toHaveCount(2);
  await expect(page.locator('[data-testid="nav"]')).not.toContainText('deux@exemple.fr');

  // La liste unifiée ne montre plus que le courrier du compte restant :
  // les 5 conversations d'un@exemple.fr, plus les 4 disparues.
  await expect(
    page.locator('[data-testid="nav-dossier"][data-categorie="reception"]'),
  ).toContainText('5');
  await expect(page.locator('[data-testid="ligne"]')).toHaveCount(5);
});
