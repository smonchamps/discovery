// Contrat IPC du formulaire IMAP/SMTP générique.
//
// Ce parcours existe à cause d'un défaut réel : la commande Rust attend un
// argument `input` (une struct), l'UI envoyait les champs à plat. Résultat,
// « ajout IMAP impossible : invalid args `input` … missing required key
// input » — la fonctionnalité n'a jamais pu créer un seul compte, et
// personne ne s'en était aperçu car rien ne l'exerçait.
//
// On ne peut pas joindre un vrai serveur IMAP ici (E2E hors ligne par
// construction). Mais on peut verrouiller ce qui avait cassé : viser un
// hôte volontairement injoignable et exiger que l'échec vienne de la
// CONNEXION, jamais de la désérialisation des arguments.
import { test, expect } from '@playwright/test';
import { launchApp, closeApp } from '../launch.mjs';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });

test.beforeAll(async () => {
  ({ app, browser, page } = await launchApp({ messages: 20 }));
});

test.afterAll(async () => {
  await closeApp({ app, browser });
});

test('compte générique : le formulaire atteint la connexion (contrat IPC)', async () => {
  await page.locator('#connect').click();
  await expect(page.locator('#add-menu')).toBeVisible();

  await page.locator('#add-imap').click();
  await expect(page.locator('#imap-dialog')).toBeVisible();

  // `.test` est un TLD réservé : il ne résout jamais, l'échec est rapide.
  await page.locator('#imap-email').fill('cobaye@exemple.fr');
  await page.locator('#imap-password').fill('mot-de-passe-factice');
  await page.locator('#imap-host').fill('imap.invalide.test');
  await page.locator('#smtp-host').fill('smtp.invalide.test');
  await page.locator('#imap-form button[type="submit"]').click();

  const status = page.locator('#status');
  // Le message prouve que les arguments ont été désérialisés et que la
  // commande est allée jusqu'à ouvrir une connexion.
  await expect(status).toContainText('connexion IMAP impossible', { timeout: 30_000 });
  // La régression d'origine, nommée : elle ne doit jamais revenir.
  await expect(status).not.toContainText('invalid args');
  await expect(status).not.toContainText('missing required key');
});
