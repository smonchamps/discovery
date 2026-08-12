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

test("au repos, la ligne de progression dit que tout est à jour", async () => {
  // La base vierge est le SEUL décor au repos réel : zéro compte, donc
  // ni synchro en échec (comptes factices des autres décors), ni envoi
  // en attente, ni rattrapage. C'est l'état que gardait le test v1
  // « aucun bandeau quand tous les corps sont là ».
  await expect(page.locator('[data-testid="progression"]')).toHaveText(
    'Tous les messages sont à jour',
  );
});

test('une saisie invalide est refusée sur place', async () => {
  await page.locator('[data-testid="onboarding-adresse"]').fill('pas-une-adresse');
  await page.locator('[data-testid="onboarding-continuer"]').click();
  await expect(page.locator('[data-testid="onboarding-erreur"]')).toContainText(
    'adresse e-mail complète',
  );
});

// AVANT le parcours « domaine inconnu » : le guichet est stateful en
// mode serial — une fois les champs IMAP révélés, ils le restent.
test('une adresse Microsoft prend la route OAuth, jamais le guichet IMAP (D4)', async () => {
  await page.locator('[data-testid="onboarding-adresse"]').fill('paul@outlook.com');
  await page.locator('[data-testid="onboarding-continuer"]').click();
  // La route est le test : l'échec vient de la configuration OAuth
  // (MICROSOFT_CLIENT_ID retiré par le harnais — échec rapide, sans
  // navigateur), PAS d'un guichet générique qui se serait révélé.
  await expect(page.locator('[data-testid="onboarding-erreur"]')).toContainText(
    'Connexion impossible',
  );
  await expect(page.locator('#ob-imap')).toHaveCount(0);
  await page.locator('[data-testid="onboarding-adresse"]').fill('');
});

test('un domaine inconnu révèle le guichet IMAP/SMTP, serveurs proposés', async () => {
  await page.locator('[data-testid="onboarding-adresse"]').fill('paul@exemple.fr');
  await page.locator('[data-testid="onboarding-continuer"]').click();
  await expect(page.locator('#ob-imap')).toHaveValue('imap.exemple.fr');
  await expect(page.locator('#ob-smtp')).toHaveValue('smtp.exemple.fr');
  // Rien n'est parti : pas d'erreur de connexion, le formulaire attend.
  await expect(page.locator('[data-testid="onboarding-erreur"]')).toHaveCount(0);
});

// Porté de compte-generique.spec.js (R2) : le contrat IPC du formulaire
// générique. Le défaut d'origine — champs envoyés à plat au lieu de la
// struct `input` — rendait l'ajout IMAP impossible SANS qu'aucun test ne
// le voie. On vise un hôte qui ne résout jamais (`.test`, TLD réservé) :
// l'échec DOIT venir de la connexion, jamais de la désérialisation.
test("compte générique : le formulaire atteint la connexion (contrat IPC)", async () => {
  await page.locator('#ob-mdp').fill('mot-de-passe-factice');
  await page.locator('#ob-imap').fill('imap.invalide.test');
  await page.locator('#ob-smtp').fill('smtp.invalide.test');
  await page.locator('[data-testid="onboarding-continuer"]').click();

  const erreur = page.locator('[data-testid="onboarding-erreur"]');
  await expect(erreur).toContainText('connexion IMAP impossible', { timeout: 30_000 });
  // La régression d'origine, nommée : elle ne doit jamais revenir.
  await expect(erreur).not.toContainText('invalid args');
  await expect(erreur).not.toContainText('missing required key');
});
