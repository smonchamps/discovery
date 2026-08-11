// L'écran 02 de la refonte (PLAN-UI-V2 §P2), joué sur le décor Clarity :
// nav réelle, onglets filtrés côté coeur, volet de lecture, action
// réelle. Le fichier est nommé pour passer APRÈS les parcours v1
// (ordre alphabétique) : une seule reconstruction d'assets par gate.
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

test('la nav porte les compteurs du décor Clarity', async () => {
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
  await expect(dossier('reception')).toContainText('4');
  await expect(dossier('reception')).toContainText('/ 18');
  await expect(dossier('envoyes')).toContainText('12');
  await expect(dossier('brouillons')).toContainText('2');
  await expect(dossier('indesirables')).toContainText('/ 3');
  await expect(dossier('archives')).toContainText('64');
  await expect(dossier('corbeille')).toContainText('3');
  // Boîtes : l'agrégée + un rang par compte RÉEL.
  await expect(page.locator('[data-testid="nav-boite"]')).toHaveCount(3);
});

test('sélectionner ouvre le volet, lit le corps, et le non-lu tombe', async () => {
  await page.locator('[data-testid="ligne"]').first().click();
  await expect(page.locator('[data-testid="lecture-sujet"]')).toHaveText(
    'Relecture du contrat Vantis',
  );
  // Le corps vit dans l'iframe sandbox — invariant S1.
  await expect(
    page.frameLocator('[data-testid="volet-lecture"] iframe').locator('body'),
  ).toContainText('Bonjour Paul');
  // mark_seen est RÉEL : le héros de la réception retombe.
  await expect(dossier('reception')).toContainText('3');
});

test("l'onglet Non lus filtre côté coeur", async () => {
  await page.locator('[data-onglet="nonlus"]').click();
  await expect(page.locator('[data-testid="ligne"]')).toHaveCount(3);
  await page.locator('[data-onglet="tous"]').click();
  await expect(page.locator('[data-testid="ligne"]').nth(4)).toBeVisible();
});

test('les dossiers canoniques servent leurs listes', async () => {
  await dossier('archives').click();
  await expect(page.locator('[data-testid="statut"]')).toContainText(
    'Archives · 64 éléments',
  );
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
  await dossier('corbeille').click();
  await expect(page.locator('[data-testid="statut"]')).toContainText(
    'Corbeille · 3 éléments',
  );
  await dossier('reception').click();
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
});

test("la Boîte d'un compte borne la liste", async () => {
  await page.locator('[data-testid="nav-boite"]').nth(2).click();
  await expect(page.locator('[data-testid="ligne"]')).toHaveCount(2);
  await page.locator('[data-testid="nav-boite"]').first().click();
  await expect(page.locator('[data-testid="ligne"]').nth(4)).toBeVisible();
});

test('archiver agit sur le coeur et confirme par le toast', async () => {
  await page.locator('[data-testid="ligne"]').nth(1).click();
  await page.locator('[data-testid="archiver"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText(
    'Conversation archivée.',
  );
  await expect(dossier('reception')).toContainText('/ 17');
});

// ——— Écran 03 : la conversation plein écran (P3) ————————————————————

test('voir la conversation ouvre le fil plein écran, dernier message déplié', async () => {
  await page.locator('[data-testid="ligne"]').first().click();
  await page.locator('[data-testid="voir-conversation"]').click();
  await expect(page.locator('[data-testid="conversation-sujet"]')).toHaveText(
    'Relecture du contrat Vantis',
  );
  await expect(page.locator('[data-testid="message-replie"]')).toHaveCount(2);
  await expect(page.locator('[data-testid="message-deplie"]')).toHaveCount(1);
  // Le corps du déplié vit dans SA propre iframe sandbox (S1).
  await expect(
    page.frameLocator('[data-testid="message-deplie"] iframe').locator('body'),
  ).toContainText('Bonjour Paul');
  // Les fichiers joints réels du message.
  await expect(page.locator('[data-testid="message-deplie"]')).toContainText(
    'Contrat_Vantis_v4.pdf',
  );
});

test("tout déplier déplie le fil, l'entête d'un message le replie", async () => {
  await page.locator('[data-testid="tout-deplier"]').click();
  await expect(page.locator('[data-testid="message-deplie"]')).toHaveCount(3);
  await page.locator('[data-testid="message-deplie"]').first().locator('.tete-message').click();
  await expect(page.locator('[data-testid="message-replie"]')).toHaveCount(1);
});

test("le retour rend la boîte intacte, sélection comprise", async () => {
  await page.locator('[data-testid="retour-boite"]').click();
  await expect(page.locator('[data-testid="conversation"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
  await expect(page.locator('[data-testid="lecture-sujet"]')).toHaveText(
    'Relecture du contrat Vantis',
  );
});

// ——— Écran 04 + Réglages : composition et thèmes (P4) ————————————————

test("écrire ouvre la composition ; l'annuler vide ne laisse rien", async () => {
  await page.locator('[data-testid="ecrire"]').click();
  await expect(page.locator('[data-testid="composition-kicker"]')).toHaveText(
    'Nouveau message',
  );
  // Le compte émetteur réel, adresse seule (écart dit : pas de nom).
  await expect(page.locator('[data-testid="composition-de"]')).toHaveText(
    'paul.merand@atelier-nord.fr',
  );
  await page.locator('[data-testid="composition-annuler"]').click();
  await expect(page.locator('[data-testid="composition"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="toast"]')).toHaveCount(0);
});

test('répondre préremplit depuis le coeur : adresse, Re :, amorce, citation, fichiers', async () => {
  await page.locator('[data-testid="repondre"]').click();
  await expect(page.locator('[data-testid="composition-kicker"]')).toHaveText('Répondre');
  await expect(page.locator('[data-testid="composition-a"]')).toHaveValue(
    'c.rousseau@atelier-nord.fr',
  );
  await expect(page.locator('[data-testid="composition-objet"]')).toHaveValue(
    'Re : Relecture du contrat Vantis',
  );
  const corps = await page.locator('[data-testid="composition-corps"]').inputValue();
  expect(corps.startsWith('Bonjour Camille,\n\n')).toBe(true);
  expect(corps).toContain('a écrit :');
  // Les fichiers du message répondu, en puces (nom + taille).
  await expect(page.locator('[data-testid="composition"]')).toContainText(
    'Contrat_Vantis_v4.pdf',
  );
});

test('enregistrer le brouillon conserve et confirme', async () => {
  await page.locator('[data-testid="composition-brouillon"]').click();
  await expect(page.locator('[data-testid="composition"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="toast"]')).toContainText(
    'Brouillon enregistré.',
  );
});

test("envoyer journalise dans la boîte d'envoi et confirme", async () => {
  await page.locator('[data-testid="repondre"]').click();
  await expect(page.locator('[data-testid="composition-a"]')).toHaveValue(
    'c.rousseau@atelier-nord.fr',
  );
  await page.locator('[data-testid="composition-envoyer"]').click();
  await expect(page.locator('[data-testid="composition"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="toast"]')).toContainText('Message envoyé.');
});

// ——— P5 : recherche, garde d'images, fente d'avis, progression ———————

test('la recherche sert ses résultats aux lignes du prototype (D1)', async () => {
  await page.locator('[data-testid="champ-recherche"]').fill('Vantis');
  await expect(page.locator('[data-testid="resultats"]')).toBeVisible();
  // La recherche traverse les boîtes : le fil Vantis sort en plusieurs
  // messages (réception, envoyés…) — on exige sa présence, pas son rang.
  await expect(
    page.locator('[data-testid="resultats"] [data-testid="ligne"]',
      { hasText: 'Relecture du contrat Vantis' }).first(),
  ).toBeVisible();
  await expect(page.locator('[data-testid="progression"]')).toContainText('Recherche ·');
  // Échap dans le champ : la boîte revient telle quelle.
  await page.locator('[data-testid="champ-recherche"]').press('Escape');
  await expect(page.locator('[data-testid="resultats"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
});

test("les images distantes restent bloquées, l'opt-in est par message", async () => {
  await page.locator('[data-testid="ligne"]', { hasText: 'renouvellement du domaine' }).click();
  await expect(page.locator('[data-testid="garde-images"]')).toContainText(
    '1 image distante bloquée',
  );
  await page.locator('[data-testid="afficher-images"]').click();
  await expect(page.locator('[data-testid="garde-images"]')).toHaveCount(0);
  // Revenir sur le message : la garde est DE RETOUR — l'opt-in ne
  // survit pas à la sélection.
  await page.locator('[data-testid="ligne"]').first().click();
  await page.locator('[data-testid="ligne"]', { hasText: 'renouvellement du domaine' }).click();
  await expect(page.locator('[data-testid="garde-images"]')).toBeVisible();
});

test("la fente d'avis porte le brouillon en cours, Reprendre le rouvre", async () => {
  // Le brouillon vient du parcours P4 « Enregistrer le brouillon » ;
  // la sonde passe toutes les 10 s.
  await expect(page.locator('[data-testid="fente-avis"]')).toContainText('brouillon');
  await page.locator('[data-testid="fente-avis"] button', { hasText: 'Reprendre' }).click();
  await expect(page.locator('[data-testid="composition-objet"]')).toHaveValue(
    'Re : Relecture du contrat Vantis',
  );
  // Vider puis fermer : le seul cas où fermer supprime — le brouillon
  // est réglé, l'avis s'éteint à la sonde suivante.
  await page.locator('[data-testid="composition-a"]').fill('');
  await page.locator('[data-testid="composition-objet"]').fill('');
  await page.locator('[data-testid="composition-corps"]').fill('');
  await page.locator('[data-testid="composition-annuler"]').click();
  await expect(page.locator('[data-testid="composition"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="fente-avis"]')).toHaveCount(0);
});

test("la ligne de progression porte l'attente non fautive de la boîte d'envoi", async () => {
  // L'envoi du parcours P4 attend toujours (compte hors ligne par
  // construction) : attente NON fautive — la ligne, pas la fente.
  await expect(page.locator('[data-testid="progression"]')).toContainText(
    "Boîte d'envoi · 1 envoi en attente",
  );
});

test('les raccourcis servent le clavier (D3)', async () => {
  // c : écrire ; Échap sort d'abord du champ (les lettres y redeviennent
  // des lettres), le second ferme — vide, rien n'est conservé.
  await page.keyboard.press('c');
  await expect(page.locator('[data-testid="composition-kicker"]')).toHaveText(
    'Nouveau message',
  );
  await page.keyboard.press('Escape');
  await page.keyboard.press('Escape');
  await expect(page.locator('[data-testid="composition"]')).toHaveCount(0);
  // e : archiver la sélection.
  await page.locator('[data-testid="ligne"]').first().click();
  await page.keyboard.press('e');
  await expect(page.locator('[data-testid="toast"]')).toContainText(
    'Conversation archivée.',
  );
});

test("le clavier active ce que le clic active (A8) : nav, rangée, onglet", async () => {
  // Une rangée de nav n'est pas un <button> (géométrie du prototype) :
  // elle doit répondre à Entrée quand même.
  await page.locator('[data-testid="nav-dossier"][data-categorie="archives"]').focus();
  await page.keyboard.press('Enter');
  await expect(page.locator('[data-testid="statut"]')).toContainText('Archives ·');
  // Une ligne de liste, à Espace.
  await page.locator('[data-testid="ligne"]').first().focus();
  await page.keyboard.press(' ');
  await expect(page.locator('[data-testid="lecture-sujet"]')).not.toBeEmpty();
  // Retour réception par le clavier.
  await page.locator('[data-testid="nav-dossier"][data-categorie="reception"]').focus();
  await page.keyboard.press('Enter');
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
});

test('les réglages appliquent et persistent le thème', async () => {
  await page.locator('[data-testid="reglages"]').click();
  await expect(page.locator('[data-testid="theme"]')).toHaveCount(7);
  await page.locator('[data-theme-id="nuit"]').click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'nuit');
  await page.locator('[data-testid="reglages-termine"]').click();
  await expect(page.locator('[data-testid="reglages-modal"]')).toHaveCount(0);
  // Persistance : le choix survit dans localStorage (rechargé au montage).
  expect(await page.evaluate(() => localStorage.getItem('discovery-theme'))).toBe('nuit');
  // La coche suit le choix à la réouverture ; retour à `nature` pour ne
  // pas teinter d'autres parcours.
  await page.locator('[data-testid="reglages"]').click();
  await expect(page.locator('[data-theme-id="nuit"] .coche')).toBeVisible();
  await page.locator('[data-theme-id="nature"]').click();
  await page.locator('[data-testid="reglages-termine"]').click();
});
