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

test("étoiler : « s » pose l'étoile — visible en liste — puis la retire", async () => {
  await page.keyboard.press('s');
  await expect(page.locator('#star')).toHaveText('★');
  await expect(page.locator('.row').first()).toHaveClass(/flagged/);

  await page.keyboard.press('s');
  await expect(page.locator('#star')).toHaveText('☆');
  await expect(page.locator('.row').first()).not.toHaveClass(/flagged/);
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

/// Ajout de compte : les trois voies doivent être offertes, et le
/// dialogue Microsoft rester invisible tant qu'on ne l'ouvre pas.
///
/// Ce dernier point n'est pas une formalité : le menu d'ajout est
/// précisément resté affiché en permanence pendant une livraison, la
/// spécificité d'ID écrasant `[hidden]`. Le même piège guette chaque
/// nouvel élément masqué — d'où l'assertion.
test('ajout de compte : trois voies, et le dialogue Microsoft ne fuit pas', async () => {
  await expect(page.locator('#add-menu')).toBeHidden();
  await expect(page.locator('#ms-dialog')).toBeHidden();

  await page.locator('#connect').click();

  await expect(page.locator('#add-gmail')).toBeVisible();
  await expect(page.locator('#add-microsoft')).toBeVisible();
  await expect(page.locator('#add-imap')).toBeVisible();

  // Microsoft ne livre pas l'adresse du compte : elle est saisie avant
  // que le navigateur ne prenne la main (ADR 0006).
  await page.locator('#add-microsoft').click();
  await expect(page.locator('#add-menu')).toBeHidden();
  await expect(page.locator('#ms-email')).toBeFocused();

  // Échap doit rendre la main — un dialogue qui piège l'utilisateur est
  // pire que pas de dialogue du tout.
  await page.keyboard.press('Escape');
  await expect(page.locator('#ms-dialog')).toBeHidden();
});

/// Pièces jointes : le décor en sème une un message sur dix. Le
/// trombone doit apparaître là où il y en a — et surtout PAS ailleurs.
///
/// Ce second point est le vrai test : une image inline prise pour une
/// pièce jointe ferait apparaître un trombone sur presque tous les
/// messages, et le signal deviendrait du bruit.
test('pièces jointes : listées quand il y en a, absentes sinon', async () => {
  await page.keyboard.press('Escape');
  await expect(page.locator('#scroll-space')).toBeVisible();

  // n°190 porte une pièce jointe (multiple de dix), n°189 non.
  await page.locator('.row', { hasText: 'n°190' }).first().click();
  await expect(page.locator('#detail-subject')).toContainText('n°190');
  await expect(page.locator('#attachments')).toBeVisible();
  await expect(page.locator('#attachments .attachment')).toHaveCount(1);
  await expect(page.locator('#attachments .attachment')).toContainText('facture-190.pdf');
  await expect(page.locator('#attachments .attachment')).toContainText('20 Ko');

  await page.locator('.row', { hasText: 'n°189' }).first().click();
  await expect(page.locator('#detail-subject')).toContainText('n°189');
  await expect(page.locator('#attachments')).toBeHidden();
});

/// Le trombone doit aussi se voir dans la LISTE, sans avoir a ouvrir le
/// message : c'est la que l'utilisateur trie. Un message sur dix en
/// porte un dans le decor — le voisin immediat doit rester nu.
test('liste : le trombone marque les messages porteurs, et eux seuls', async () => {
  const withClip = page.locator('.row', { hasText: 'n°180' }).first();
  const without = page.locator('.row', { hasText: 'n°179' }).first();

  await expect(withClip.locator('.clip')).toBeVisible();
  await expect(without.locator('.clip')).toHaveCount(0);
});
