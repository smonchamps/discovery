// R2 (PLAN-RETRAIT-V1) : les parcours v1 portés sur v2, avec la graine
// EXACTE des specs d'origine (seed_inbox, 200 messages : un fil sur
// cinq, une pièce jointe sur dix). Ce fichier remplacera
// parcours-critiques / recherche / multi-comptes à B2.
//
// Abandons motivés (PASSATION §2.6) :
// - « étoiler (s) » et « déplacer (v) » tombent avec D2 — coupés à la
//   bascule, commandes cœur conservées, réversibles par spéc courte ;
// - l'auto-avance après archivage (v1 ouvrait le message suivant) ne se
//   porte pas : le prototype ferme le volet — écart assumé A6 ;
// - « deux brouillons de même sujet distincts au corps » : couvert par
//   nature depuis PLAN-BROUILLONS — le dossier Brouillons montre les
//   brouillons LOCAUX et leur aperçu distingue au corps, sans réseau.
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';

test.describe.configure({ mode: 'serial' });

test.describe('décor v1 : un compte, 200 messages', () => {
  let app;
  let browser;
  let page;

  test.beforeAll(async () => {
    ({ app, browser, page } = await launchAppV2({
      comptes: [{ email: 'e2e@exemple.fr', messages: 200 }],
    }));
  });

  test.afterAll(async () => {
    await closeApp({ app, browser });
  });

  test("lire : la liste s'affiche, le plus récent d'abord, le corps s'ouvre en iframe", async () => {
    await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
    await expect(page.locator('[data-testid="ligne"]').first()).toContainText('n°200');
    // 200 messages, un fil sur cinq : 160 conversations.
    await expect(
      page.locator('[data-testid="nav-dossier"][data-categorie="reception"]'),
    ).toContainText('160');
    // Aucun avis parasite au lancement (màj/télémétrie neutralisées §7.5).
    await expect(page.locator('[data-testid="fente-avis"]')).toHaveCount(0);

    await page.locator('[data-testid="ligne"]').first().click();
    await expect(page.locator('[data-testid="lecture-sujet"]')).toContainText('n°200');
    await expect(
      page.frameLocator('[data-testid="volet-lecture"] iframe').locator('body'),
    ).toContainText('Corps du message n°200');
  });

  test("trier : « e » archive la sélection, la liste suit", async () => {
    await page.keyboard.press('e');
    await expect(page.locator('[data-testid="toast"]')).toContainText('Conversation archivée.');
    // Le n°200 répondait au n°199 : la tête du fil devient le n°199.
    await expect(page.locator('[data-testid="ligne"]').first()).toContainText('n°199');
  });

  test('répondre : préremplissages réels, envoi hors ligne JOURNALISÉ, jamais perdu', async () => {
    await page.locator('[data-testid="ligne"]').first().click();
    await page.keyboard.press('r');
    await expect(page.locator('[data-testid="composition-kicker"]')).toHaveText('Répondre');
    await expect(page.locator('[data-testid="composition-a"]')).toHaveValue(/@exemple\.fr$/);
    // Forme du prototype (« Re : »), citation réelle du cœur.
    await expect(page.locator('[data-testid="composition-objet"]')).toHaveValue(/^Re : /);
    const corps = page.locator('[data-testid="composition-corps"]');
    await expect(corps).toHaveValue(/a écrit :/);
    await expect(corps).toHaveValue(/> Corps du message n°199/);

    const cite = await corps.inputValue();
    await corps.fill(`Réponse E2E.\n${cite}`);
    await page.locator('[data-testid="composition-envoyer"]').click();
    await expect(page.locator('[data-testid="composition"]')).toHaveCount(0);
    await expect(page.locator('[data-testid="toast"]')).toContainText('Message envoyé.');
    // Hors ligne par construction : la règle d'or, VISIBLE — l'attente
    // non fautive vit dans la ligne de progression (sonde 10 s).
    await expect(page.locator('[data-testid="progression"]')).toContainText(
      "Boîte d'envoi · 1 envoi en attente",
    );
  });

  test('brouillon : Échap conserve, le dossier Brouillons restitue intact', async () => {
    await page.keyboard.press('c');
    await page.locator('[data-testid="composition-objet"]').fill('Brouillon E2E');
    await page.locator('[data-testid="composition-corps"]').fill('Texte précieux.');
    await page.keyboard.press('Escape'); // sortir du champ…
    await page.keyboard.press('Escape'); // …fermer : conserver, jamais jeter
    await expect(page.locator('[data-testid="composition"]')).toHaveCount(0);
    await expect(page.locator('[data-testid="toast"]')).toContainText('Brouillon enregistré.');

    // Plus de fente (PLAN-BROUILLONS) : le brouillon vit AU DOSSIER —
    // sans destinataire, l'atténué le dit — et le clic le rouvre INTACT.
    await expect(page.locator('[data-testid="fente-avis"]')).toHaveCount(0);
    await page.locator('[data-testid="nav-dossier"][data-categorie="brouillons"]').click();
    const ligne = page.locator('[data-testid="ligne-brouillon"]', { hasText: 'Brouillon E2E' });
    await expect(ligne).toContainText('(sans destinataire)');
    await ligne.click();
    await expect(page.locator('[data-testid="composition-objet"]')).toHaveValue('Brouillon E2E');
    await expect(page.locator('[data-testid="composition-corps"]')).toHaveValue('Texte précieux.');
    // Vider puis fermer : le seul cas où fermer supprime — la ligne
    // quitte le dossier sans attendre la sonde.
    await page.locator('[data-testid="composition-objet"]').fill('');
    await page.locator('[data-testid="composition-corps"]').fill('');
    await page.locator('[data-testid="composition-annuler"]').click();
    await expect(page.locator('[data-testid="ligne-brouillon"]')).toHaveCount(0);
    // Retour en Réception : la suite de la chaîne sérielle joue sur la
    // boîte.
    await page.locator('[data-testid="nav-dossier"][data-categorie="reception"]').click();
    await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
  });

  test('pièces jointes : la puce marque les porteurs, et eux seuls', async () => {
    // n°190 (fil 189+190, pièce sur le 190) ; n°186 : ni fil ni pièce.
    // Témoins choisis DANS la fenêtre rendue — la liste v2 est fenêtrée,
    // une ligne plus profonde n'existe pas dans le DOM sans défilement.
    const porteur = page.locator('[data-testid="ligne"]', { hasText: 'n°190' }).first();
    await expect(porteur).toContainText('1 fichier');
    const nu = page.locator('[data-testid="ligne"]', { hasText: 'n°186' }).first();
    await expect(nu).not.toContainText('fichier');

    await porteur.click();
    await expect(page.locator('[data-testid="lecture-sujet"]')).toContainText('n°190');
    await expect(page.locator('[data-testid="volet-lecture"]')).toContainText(
      '2 messages · 1 fichier',
    );
  });

  test('conversations : une ligne par fil, compteur, échange navigable plein écran', async () => {
    // Le n°189 n'a pas de ligne à lui : le n°190 représente le fil.
    await expect(
      page.locator('[data-testid="ligne"]', { hasText: 'n°189' }),
    ).toHaveCount(0);
    await page.locator('[data-testid="voir-conversation"]').click();
    await expect(page.locator('[data-testid="conversation-sujet"]')).toContainText('n°190');
    await expect(page.locator('[data-testid="message-deplie"]')).toHaveCount(1);
    await expect(page.locator('[data-testid="message-replie"]')).toHaveCount(1);
    await expect(page.locator('[data-testid="message-deplie"]')).toContainText('facture-190.pdf');

    // Déplier le plus ancien sans quitter le fil.
    await page.locator('[data-testid="message-replie"]').click();
    await expect(page.locator('[data-testid="message-deplie"]')).toHaveCount(2);
    await expect(page.locator('[data-testid="message-deplie"]').first()).toContainText('n°189');
    await page.locator('[data-testid="retour-boite"]').click();
  });

  test('recherche : « / » focalise, les résultats servent, archiver en retire (régression #4)', async () => {
    await page.keyboard.press('/');
    await expect(page.locator('[data-testid="champ-recherche"]')).toBeFocused();
    await page.locator('[data-testid="champ-recherche"]').fill('facture');
    const resultats = page.locator('[data-testid="resultats"] [data-testid="ligne"]');
    await expect(resultats.first()).toBeVisible();

    const avant = await resultats.count();
    expect(avant).toBeGreaterThan(0);
    const archive = await resultats.first().locator('.objet').textContent();

    // Archiver le premier résultat SANS quitter la recherche.
    await resultats.first().click();
    await page.keyboard.press('e');
    await expect(page.locator('[data-testid="toast"]')).toContainText('Conversation archivée.');
    await expect(resultats).toHaveCount(avant - 1);
    await expect(page.locator('[data-testid="resultats"]')).not.toContainText(archive);

    // Échap rend la boîte telle quelle.
    await page.locator('[data-testid="champ-recherche"]').press('Escape');
    await expect(page.locator('[data-testid="resultats"]')).toHaveCount(0);
    await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
  });
});

test.describe('décor v1 : deux comptes fusionnés', () => {
  let app;
  let browser;
  let page;

  test.beforeAll(async () => {
    ({ app, browser, page } = await launchAppV2({
      comptes: [
        { email: 'un@exemple.fr', messages: 30 },
        { email: 'deux@exemple.fr', messages: 20 },
      ],
    }));
  });

  test.afterAll(async () => {
    await closeApp({ app, browser });
  });

  test('boîte unifiée : fusion par date, un rang de nav par compte réel', async () => {
    await expect(page.locator('[data-testid="nav-boite"]')).toHaveCount(3);
    await expect(page.locator('[data-testid="ligne"]').first()).toContainText('n°30');
  });

  test("répondre depuis l'unifiée : le compte du message est l'émetteur — et se choisit (A10)", async () => {
    await page.locator('[data-testid="ligne"]').first().click();
    await page.keyboard.press('r');
    const de = page.locator('[data-testid="composition-de"]');
    await expect(de).toHaveValue('un@exemple.fr');
    await expect(de.locator('option')).toHaveCount(2);
    await page.locator('[data-testid="composition-annuler"]').click();
  });
});
