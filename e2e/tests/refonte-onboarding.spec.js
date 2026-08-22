// L'écran 01 de la refonte (PLAN-UI-V2 §P4, D4) : à ZÉRO compte,
// l'application accueille — depuis PLAN-RETOURS-8 (A75), c'est le
// PARCOURS en quatre étapes qui s'ouvre sur une base vierge (la clé
// `wind-accueil-fait` absente) ; l'étape 1 porte le guichet d'A11
// inchangé — les parcours de porte (Microsoft, IMAP générique, contrat
// IPC) se jouent dedans tels quels. Lancement séparé : l'état zéro
// compte ne peut pas se jouer sur le décor Clarity.
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });

test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2({ vierge: true }));
  // Le profil WebView2 est partagé : une suite précédente a pu poser
  // les marques d'accueil — les retirer pour jouer le VRAI premier
  // lancement.
  await page.evaluate(() => {
    localStorage.removeItem('wind-accueil-fait');
    localStorage.removeItem('wind-accueil-commence');
  });
  await page.reload();
});

test.afterAll(async () => {
  await page
    .evaluate(() => {
      localStorage.removeItem('wind-accueil-fait');
      localStorage.removeItem('wind-accueil-commence');
    })
    .catch(() => { /* fenêtre déjà morte */ });
  await closeApp({ app, browser });
});

test("à zéro compte, le parcours accueille — étape 1, le guichet", async () => {
  await expect(page.locator('[data-testid="onboarding"]')).toBeVisible();
  // Terrain 2026-08-22 (constat 1) : « Bienvenue dans Wind », puis
  // « Étape 1/4 », puis l'invite d'ajout.
  await expect(page.locator('[data-testid="onboarding"]')).toContainText(
    'Bienvenue dans Wind',
  );
  await expect(page.locator('[data-testid="accueil-progression"]')).toHaveText(
    'Étape 1/4',
  );
  await expect(page.locator('[data-testid="onboarding"]')).toContainText(
    'Pour commencer, ajoutez une adresse email.',
  );
  // 2e passe terrain (constat 2) : la note « serveur détecté » a
  // quitté l'accueil ; (constat 1) : sans compte, « Ajouter » est LE
  // geste — primaire.
  await expect(page.locator('[data-testid="onboarding"]')).not.toContainText(
    'Le serveur est détecté automatiquement.',
  );
});

test('sans compte ajouté, Continuer est ABSENT (D4, 3e passe terrain)', async () => {
  // Jamais un bouton grisé : tant qu'aucun compte n'existe, la marche
  // ne montre pas Continuer — « Ajouter » est le geste primaire.
  await expect(page.locator('[data-testid="accueil-continuer"]')).toHaveCount(0);
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

// Terrain 2026-08-22 (constat 3) : le guichet générique révélé porte un
// « Retour » qui REPLIE les champs serveur — rien ne part, l'adresse
// reste.
test('le guichet générique se replie par « Retour »', async () => {
  await expect(page.locator('#ob-imap')).toHaveCount(1);
  await page.locator('[data-testid="guichet-retour"]').click();
  await expect(page.locator('#ob-imap')).toHaveCount(0);
  await expect(page.locator('[data-testid="onboarding-adresse"]')).toHaveValue(
    'paul@exemple.fr',
  );
});

// EN DERNIER (le reload remet le guichet à zéro, les tests d'avant sont
// statefuls) : le second régime de l'écran 01 — un poste déjà accueilli
// revenu à zéro compte retrouve le guichet SEUL, sans étapes (A75).
test('déjà accueilli, zéro compte : le guichet seul, sans parcours', async () => {
  await page.evaluate(() => localStorage.setItem('wind-accueil-fait', '1'));
  await page.reload();
  await expect(page.locator('[data-testid="onboarding"]')).toBeVisible();
  await expect(page.locator('[data-testid="onboarding"]')).toContainText(
    'Bienvenue dans Wind',
  );
  await expect(page.locator('[data-testid="accueil-progression"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="accueil-continuer"]')).toHaveCount(0);
});
