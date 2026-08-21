// PLAN-RETOURS-6 : signatures, envoi différé, « important », entête du
// composeur — les quatre retours CE du 2026-08-21, joués sur le décor
// Clarity. Les comptes du décor sont au jeton invalide : la boîte
// d'envoi journalise sans jamais rien envoyer — l'échéance et le
// journal se lisent par `outbox_status`, comme au terrain.
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

const invoke = (cmd, args) =>
  page.evaluate(([c, a]) => window.__TAURI__.core.invoke(c, a), [cmd, args ?? {}]);

const fond = (loc) => loc.evaluate((el) => getComputedStyle(el).backgroundColor);

test("R4 : l'entête du composeur porte le fond du pied de page de Wind (A66)", async () => {
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
  await page.locator('[data-testid="ecrire"]').click();
  await expect(page.locator('[data-testid="composition-kicker"]')).toBeVisible();
  // Le même jeton (--panel) des deux côtés : la comparaison se fait sur
  // la couleur CALCULÉE — un thème qui bouge ne casse pas le test.
  const tete = await fond(page.locator('[data-testid="composition"] .tete'));
  const pied = await fond(page.locator('[data-testid="statut"]'));
  expect(tete).toBe(pied);
  await page.locator('[data-testid="composition-annuler"]').click();
  await expect(page.locator('[data-testid="composition"]')).toHaveCount(0);
});

test('R1 : la signature se pose aux Réglages, paraît au nouveau message — et fermer sans frappe ne sème rien', async () => {
  // Poser la signature du premier compte, par la surface réelle.
  await page.locator('[data-testid="reglages"]').click();
  await page.locator('[data-testid="reglages-groupe"][data-groupe="signature"]').click();
  const editeur = page.locator('[data-testid="signature-editeur"]').first();
  await expect(editeur).toBeVisible();
  await editeur.click();
  await editeur.pressSequentially('Cordialement, Léa');
  await page.locator('[data-testid="signature-enregistrer"]').first().click();
  await expect(page.locator('[data-testid="signature-etat"]').first()).toContainText(
    'Signature enregistrée.',
  );
  // Terrain 2026-08-21 : « Appliquer à tous les comptes » copie la
  // signature ET la portée — et ça se VOIT sur le bloc du 2e compte.
  await page.locator('[data-testid="signature-tous"]').first().click();
  await expect(page.locator('[data-testid="signature-etat"]').first()).toContainText(
    'appliqués à tous les comptes',
  );
  await expect(page.locator('[data-testid="signature-editeur"]').nth(1)).toContainText(
    'Cordialement, Léa',
  );
  // Puis on EFFACE celle du 2e compte : le rechargement au changement
  // de compte émetteur, plus bas, doit se voir.
  await page.locator('[data-testid="signature-effacer"]').nth(1).click();
  await expect(page.locator('[data-testid="signature-editeur"]').nth(1)).toHaveText('');
  await page.locator('[data-testid="reglages-termine"]').click();

  // Un nouveau message la porte, sous deux lignes vides.
  const avant = (await invoke('list_drafts')).length;
  await page.locator('[data-testid="ecrire"]').click();
  await expect(page.locator('[data-testid="composition-corps"]')).toContainText(
    'Cordialement, Léa',
  );
  // Terrain 2026-08-21 : la signature SUIT le compte émetteur — le
  // 2e compte (signature effacée) vide le corps posé.
  const de = page.locator('select[data-testid="composition-de"]');
  const emails = await de.locator('option').allTextContents();
  await de.selectOption(emails[1]);
  await expect(page.locator('[data-testid="composition-corps"]')).not.toContainText(
    'Cordialement, Léa',
  );
  await de.selectOption(emails[0]);
  await expect(page.locator('[data-testid="composition-corps"]')).toContainText(
    'Cordialement, Léa',
  );
  // Fermer SANS FRAPPE : la signature seule ne fait pas un brouillon —
  // aucun fantôme semé à chaque ouverture (garde anti-churn).
  await page.locator('[data-testid="composition-annuler"]').click();
  await expect(page.locator('[data-testid="composition"]')).toHaveCount(0);
  expect((await invoke('list_drafts')).length).toBe(avant);

  // D4, portée par défaut « nouveaux messages seuls » : une réponse ne
  // porte PAS la signature tant que le compte ne l'a pas choisi.
  await page
    .locator('[data-testid="ligne"]', { hasText: 'Relecture du contrat Vantis' })
    .click();
  await page.locator('[data-testid="repondre"]').first().click();
  await expect(page.locator('[data-testid="composition-kicker"]')).toHaveText('Répondre');
  await expect(page.locator('[data-testid="composition-corps"]')).not.toContainText(
    'Cordialement, Léa',
  );
  await page.locator('[data-testid="composition-annuler"]').click();
  await expect(page.locator('[data-testid="composition"]')).toHaveCount(0);

  // Portée activée sur le compte 1 : la réponse porte la signature
  // entre l'amorce et la citation — et le changement de compte la
  // recompose SANS perdre la citation (terrain 2026-08-21, 2e passe).
  await page.locator('[data-testid="reglages"]').click();
  await page.locator('[data-testid="reglages-groupe"][data-groupe="signature"]').click();
  await page.locator('[data-testid="signature-repliques"]').first().click();
  await expect(
    page.locator('[data-testid="signature-repliques"]').first(),
  ).toHaveAttribute('aria-checked', 'true');
  await page.locator('[data-testid="reglages-termine"]').click();
  // Le dernier message du fil Vantis porte un corps (contrat du décor,
  // même cible que le parcours réponse d'ecran02).
  await page.locator('[data-testid="repondre"]').last().click();
  const corps = page.locator('[data-testid="composition-corps"]');
  await expect(corps).toContainText('Cordialement, Léa');
  await expect(corps).toContainText('a écrit :');
  const de2 = page.locator('select[data-testid="composition-de"]');
  const emails2 = await de2.locator('option').allTextContents();
  await de2.selectOption(emails2[1]);
  // Compte 2 : pas de signature (effacée) — la citation, elle, reste.
  await expect(corps).not.toContainText('Cordialement, Léa');
  await expect(corps).toContainText('a écrit :');
  await page.locator('[data-testid="composition-annuler"]').click();
  await expect(page.locator('[data-testid="composition"]')).toHaveCount(0);
});

test("R2 : un envoi programmé attend son heure, se dit, et s'annule en brouillon (D1/D2)", async () => {
  await page.locator('[data-testid="ecrire"]').click();
  await page.locator('[data-testid="composition-a"]').fill('dest@exemple.fr');
  await page.locator('[data-testid="composition-objet"]').fill('Départ programmé');
  await page.locator('[data-testid="composition-plus-tard"]').click();
  // La carte dit la sémantique locale (D1) et prérègle +1 h.
  await expect(page.locator('[data-testid="composition-differe"]')).toContainText(
    'si Wind est ouvert',
  );
  await page.locator('[data-testid="composition-differe-confirmer"]').click();
  await expect(page.locator('[data-testid="composition"]')).toHaveCount(0);
  // Le toast dit l'ÉCHÉANCE, jamais « envoyé » — rien n'est parti.
  await expect(page.locator('[data-testid="toast"]')).toContainText('Envoi programmé');

  // Le journal porte l'échéance, à part des « en attente ».
  const statut = await invoke('outbox_status');
  expect(statut.scheduled).toBe(1);
  expect(statut.queued).toBe(0);
  const entree = statut.entries.find((e) => e.subject === 'Départ programmé');
  expect(entree.send_at_epoch).toBeGreaterThan(Math.floor(Date.now() / 1000));

  // La barre d'état le dit (sonde 10 s), et la fente offre l'annulation.
  await expect(page.locator('[data-testid="progression"]')).toContainText('programmé');
  const fente = page.locator('[data-testid="fente-avis"]');
  await expect(fente).toContainText('Départ programmé');
  await fente.getByRole('button', { name: "Annuler l'envoi" }).click();
  await expect(page.locator('[data-testid="toast"]')).toContainText('Envoi annulé');

  // D2 : le brouillon est DE RETOUR, le journal est vide.
  const apres = await invoke('outbox_status');
  expect(apres.scheduled).toBe(0);
  expect(apres.entries.length).toBe(0);
  const brouillons = await invoke('list_drafts');
  expect(brouillons.some((b) => b.subject === 'Départ programmé')).toBe(true);
});

test("R3 : « Important » se marque, suit le brouillon repris, et part au journal", async () => {
  await page.locator('[data-testid="ecrire"]').click();
  const marque = page.locator('[data-testid="composition-important"]');
  await expect(marque).toHaveAttribute('aria-pressed', 'false');
  await marque.click();
  await expect(marque).toHaveAttribute('aria-pressed', 'true');
  await page.locator('[data-testid="composition-a"]').fill('dest@exemple.fr');
  await page.locator('[data-testid="composition-objet"]').fill('Marqué important');
  await page.locator('[data-testid="composition-brouillon"]').click();
  await expect(page.locator('[data-testid="composition"]')).toHaveCount(0);

  // La reprise restitue le marquage — l'état vit au brouillon. (Le
  // dossier Brouillons a SON testid : `ligne-brouillon`, pas `ligne`.)
  await page
    .locator('[data-testid="nav-dossier"][data-categorie="brouillons"]')
    .click();
  await page
    .locator('[data-testid="ligne-brouillon"]', { hasText: 'Marqué important' })
    .click();
  await expect(page.locator('[data-testid="composition-important"]')).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  // L'envoi journalise (compte hors ligne : il reste en file) — le
  // marquage du journal et les en-têtes SMTP sont prouvés côté Rust.
  await page.locator('[data-testid="composition-envoyer"]').click();
  await expect(page.locator('[data-testid="composition"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="toast"]')).toContainText('Message envoyé.');
});
