// Les parcours critiques du gate 2 (PLAN.md §4) : lire, trier, répondre —
// plus le filet des brouillons. Tout se joue HORS LIGNE sur une base
// seedée : déterministe, zéro credential, zéro réseau requis.
//
// Les tests partagent une même fenêtre et s'enchaînent : chacun laisse
// l'application dans l'état attendu par le suivant (mode `serial`).
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

test("lire : la liste s'affiche, le plus récent d'abord, et le corps s'ouvre", async () => {
  await expect(page.locator('.row').first()).toBeVisible();
  await expect(page.locator('.row').first()).toContainText('n°200');
  await expect(page.locator('#perf')).toContainText('200 messages');

  await page.locator('.row').first().click();

  await expect(page.locator('#detail-subject')).toContainText('n°200');
  await expect(page.locator('#detail-frame')).toHaveAttribute(
    'srcdoc',
    /Corps du message n°200/,
  );
});

test('trier : « e » archive le message ouvert, la liste et le compte suivent', async () => {
  await page.keyboard.press('e');

  await expect(page.locator('#perf')).toContainText('199 messages');
  await expect(page.locator('.row').first()).toContainText('n°199');
  // L'auto-avance ouvre le message suivant : le triage ne casse pas le flux.
  await expect(page.locator('#detail-subject')).toContainText('n°199');
});

test('répondre : destinataire, « Re: » et citation pré-remplis — envoi hors ligne journalisé, jamais perdu', async () => {
  await page.keyboard.press('r');

  await expect(page.locator('#compose')).toBeVisible();
  await expect(page.locator('#compose-title')).toHaveText('Répondre');
  await expect(page.locator('#compose-to')).toHaveValue(/@exemple\.fr$/);
  await expect(page.locator('#compose-subject')).toHaveValue(/^Re: /);
  await expect(page.locator('#compose-body')).toHaveValue(/a écrit :/);
  await expect(page.locator('#compose-body')).toHaveValue(/> Corps du message n°199/);

  const quoted = await page.locator('#compose-body').inputValue();
  await page.locator('#compose-body').fill(`Réponse E2E.\n${quoted}`);
  await page.locator('#compose-send').click();

  // Hors ligne par construction : l'envoi est JOURNALISÉ, pas perdu —
  // la règle d'or de la boîte d'envoi, visible à l'écran.
  await expect(page.locator('#compose')).toBeHidden();
  await expect(page.locator('#outbox-bar')).toBeVisible();
  await expect(page.locator('#outbox-summary')).toContainText('1 en attente');
});

test('brouillon : Échap conserve le texte, Reprendre le restitue intact', async () => {
  await page.keyboard.press('c');
  await expect(page.locator('#compose')).toBeVisible();

  await page.locator('#compose-subject').fill('Brouillon E2E');
  await page.locator('#compose-body').fill('Texte précieux.');
  await page.keyboard.press('Escape'); // sortir du champ…
  await page.keyboard.press('Escape'); // …fermer : conserver, jamais jeter

  await expect(page.locator('#compose')).toBeHidden();
  await expect(page.locator('#drafts-bar')).toBeVisible();
  await expect(page.locator('#drafts-summary')).toContainText('Brouillon(s) : 1');

  await page.getByRole('button', { name: 'Reprendre' }).click();

  await expect(page.locator('#compose')).toBeVisible();
  await expect(page.locator('#compose-subject')).toHaveValue('Brouillon E2E');
  await expect(page.locator('#compose-body')).toHaveValue('Texte précieux.');
});
