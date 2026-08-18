// PLAN-RETOURS-3 — les retours terrain joués sur le décor Clarity, en
// ISOLATION (leur propre instance) : R4 (répondre PAR message, à un
// message précis du fil) et R2 (signaler indésirable / le contraire).
// Nommé pour passer après ecran02 (ordre alphabétique) — une seule
// reconstruction d'assets par gate.
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

test('R4/D4 : répondre vise le message CHOISI du fil, pas le dernier', async () => {
  // Le fil Vantis : m1 (notre envoi, Paul) <- m2 (Sofia Nardi) <- m3
  // (Camille Rousseau). Le geste par message doit viser CE message —
  // répondre au message de Sofia compose vers Sofia, pas vers Camille
  // (le dernier). C'est la précision que le retour demande.
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
  await page
    .locator('[data-testid="ligne"]', { hasText: 'Relecture du contrat Vantis' })
    .first()
    .click();
  await expect(
    page.locator('[data-testid="volet-lecture"] [data-testid="fil-sujet"]'),
  ).toHaveText('Relecture du contrat Vantis');

  // Tout déplier : les trois messages, chacun sa barre de réponse EN BAS.
  await page.locator('[data-testid="volet-lecture"] [data-testid="tout-deplier"]').click();
  const messages = page.locator('[data-testid="volet-lecture"] [data-testid="message-deplie"]');
  await expect(messages).toHaveCount(3);
  // Ordre chronologique (le plus ancien en tête) : m2 (Sofia) au milieu.
  await expect(messages.nth(1)).toContainText('Sofia Nardi');

  // Répondre AU message de Sofia (le milieu) — sa barre, pas celle du dernier.
  await messages.nth(1).locator('[data-testid="repondre"]').click();
  await expect(page.locator('[data-testid="composition-kicker"]')).toHaveText('Répondre');
  await expect(page.locator('[data-testid="composition-a"]')).toHaveValue(
    's.nardi@atelier-nord.fr',
  );
  await page.locator('[data-testid="composition-annuler"]').click();
  await expect(page.locator('[data-testid="composition"]')).toHaveCount(0);

  // Constat terrain (2026-08-18) : NOTRE propre message (m1, Paul, en
  // tête) porte AUSSI les trois gestes, et répondre y vise les
  // destinataires d'origine (Camille en À), jamais soi-même (Paul).
  await expect(messages.nth(0)).toContainText('Paul Mérand');
  await expect(
    messages.nth(0).locator('[data-testid="repondre"]'),
  ).toBeVisible();
  await expect(
    messages.nth(0).locator('[data-testid="repondre-tous"]'),
  ).toBeVisible();
  await messages.nth(0).locator('[data-testid="repondre"]').click();
  await expect(page.locator('[data-testid="composition-kicker"]')).toHaveText('Répondre');
  await expect(page.locator('[data-testid="composition-a"]')).toHaveValue(
    'c.rousseau@atelier-nord.fr',
  );
  await page.locator('[data-testid="composition-annuler"]').click();
  await expect(page.locator('[data-testid="composition"]')).toHaveCount(0);

  // Répondre à tous sur notre propre message : le À d'origine (Camille),
  // le Cc d'origine EN Cc (Sofia) — jamais soi-même.
  await messages.nth(0).locator('[data-testid="repondre-tous"]').click();
  await expect(page.locator('[data-testid="composition-a"]')).toHaveValue(
    'c.rousseau@atelier-nord.fr',
  );
  await expect(page.locator('[data-testid="composition-cc"]')).toHaveValue(
    's.nardi@atelier-nord.fr',
  );
  await page.locator('[data-testid="composition-annuler"]').click();
  await expect(page.locator('[data-testid="composition"]')).toHaveCount(0);
});

test('R2/D2 : signaler un courrier comme indésirable le sort de la Réception', async () => {
  await dossier('reception').click();
  // « Atelier de septembre » (Sofia, compte travail — qui a un dossier
  // Spam) : un message autonome, cas simple.
  const ligne = page.locator('[data-testid="ligne"]', { hasText: 'Atelier de septembre' });
  await expect(ligne.first()).toBeVisible();
  await ligne.first().click();
  await expect(
    page.locator('[data-testid="volet-lecture"] [data-testid="fil-sujet"]'),
  ).toHaveText('Atelier de septembre');

  // La barre du fil porte « Signaler comme spam » (vue Réception).
  await page.locator('[data-testid="volet-lecture"] [data-testid="signaler-spam"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText(
    'signalé comme indésirable',
  );
  // Disparition optimiste : la Réception n'en porte plus la trace.
  await expect(
    page.locator('[data-testid="ligne"]', { hasText: 'Atelier de septembre' }),
  ).toHaveCount(0);
});

test('R2/D2 : « Ce n’est pas un spam » ramène un message en Réception', async () => {
  await dossier('indesirables').click();
  const ligne = page.locator('[data-testid="ligne"]', { hasText: 'Vous avez gagné' });
  await expect(ligne.first()).toBeVisible();
  await ligne.first().click();
  await expect(
    page.locator('[data-testid="volet-lecture"] [data-testid="fil-sujet"]'),
  ).toHaveText('Vous avez gagné');

  // En vue Indésirables, la barre bascule sur « Ce n'est pas un spam » —
  // et « Signaler comme spam » n'y est plus.
  await expect(
    page.locator('[data-testid="volet-lecture"] [data-testid="signaler-spam"]'),
  ).toHaveCount(0);
  await page.locator('[data-testid="volet-lecture"] [data-testid="pas-spam"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText('remis en réception');
  await expect(
    page.locator('[data-testid="ligne"]', { hasText: 'Vous avez gagné' }),
  ).toHaveCount(0);
});
