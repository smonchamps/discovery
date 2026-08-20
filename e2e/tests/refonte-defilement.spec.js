// Le défilement profond (PLAN-DEFILEMENT-PROFOND) : un drag tenu de la
// barre ne doit ni saturer le coeur ni faire mentir l'écran vide.
//
// Constat terrain du 2026-08-20, mesuré au banc mesure-defilement.mjs :
// un drag de 2 s déclenchait ~161 `list_category` (une page par
// position traversée, jamais annulée), la file sérialisée de
// `hors_pompe` se drainait en minutes sur la vraie base, et pendant ce
// temps TOUS les dossiers disaient « Aucun message ici. » — le
// changement de source remettait `total = 0` sans invalider la garde
// d'affichage du vide.
//
// Décor dédié : 6 000 messages en Archives — assez de pages pour qu'un
// drag rapide en traverse des dizaines.
import { test, expect } from '@playwright/test';
import { launchAppV2, closeApp } from '../launch.mjs';
import { tenirBarre } from '../geste-defilement.mjs';

let app;
let browser;
let page;

test.describe.configure({ mode: 'serial' });

test.beforeAll(async () => {
  ({ app, browser, page } = await launchAppV2({
    comptes: [{ email: 'defil@exemple.fr', messages: 300, archives: 6000 }],
  }));
});

test.afterAll(async () => {
  await closeApp({ app, browser });
});

const dossier = (categorie) =>
  page.locator(`[data-testid="nav-dossier"][data-categorie="${categorie}"]`);

test("les lignes ne suivent jamais le comptage — page d'abord, total au repos (terrain 2026-08-20)", async () => {
  // Le comptage d'une catégorie (sonde NOT EXISTS par ligne sur une
  // intégrale, ~240 ms sur 200 k — bien plus à froid) retardait chaque
  // PREMIER affichage : il vit désormais dans `category_total`, demandé
  // quand la pompe de pages est au repos — jamais devant les lignes.
  // Démarrage d'abord retombé (réception, sondes) : le journal ne doit
  // porter que le geste observé.
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
  await new Promise((resolve) => setTimeout(resolve, 1500));
  await page.evaluate(() => {
    window.__e2eJournal = [];
  });
  await dossier('archives').click();
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
  // 6 000 messages : la page 0 est PLEINE, le vrai total ne peut venir
  // que du comptage — et il finit au statut, après les lignes.
  await expect(page.locator('[data-testid="statut"]')).toContainText('Archives · 6000');
  const ordre = await page.evaluate(() => {
    const journal = window.__e2eJournal;
    const page0 = journal.find((a) => a.commande === 'list_category');
    const compte = journal.find((a) => a.commande === 'category_total');
    delete window.__e2eJournal;
    return {
      page0Arrivee: page0?.arrivee ?? null,
      compteDepart: compte?.depart ?? null,
    };
  });
  expect(ordre.page0Arrivee).not.toBeNull();
  expect(ordre.compteDepart).not.toBeNull();
  expect(ordre.compteDepart).toBeGreaterThan(ordre.page0Arrivee);
});

test('un drag tenu ne garde jamais plus de deux pages en vol (E1)', async () => {
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
  await dossier('archives').click();
  await expect(page.locator('[data-testid="liste-titre"]')).toHaveText('Archives');
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
  // Quiescence prouvée AVANT la retenue : plus une attente à l'écran,
  // donc plus un vol ouvert — sans quoi l'assertion de rafale pourrait
  // passer à vide (deux vols résiduels occuperaient déjà la jauge).
  await expect(page.locator('[data-testid="ligne-attente"]')).toHaveCount(0);

  // Transport RETENU pendant tout le geste : le coeur ne répond pas —
  // exactement la saturation du terrain, rendue DÉTERMINISTE (sur un
  // petit décor rapide, la file ne se formerait pas ; sur la vraie
  // base, elle durait des minutes). Le journal (couture __e2eJournal)
  // compte ce que la liste DEMANDE pendant ce silence.
  try {
    await page.evaluate(() => {
      window.__e2eJournal = [];
      window.__e2eRetenue = new Promise((liberer) => {
        window.__e2eLiberer = liberer;
      });
    });
    // La barre tenue au clic jusqu'à 1/3 de la liste (une dizaine de
    // pages traversées) — le geste partagé avec le banc.
    await tenirBarre(page, { pas: 60 });
    // L'invariant qui tue la panne du terrain : coeur muet, la liste
    // demande AU MOINS une page (le geste a bougé la fenêtre) et JAMAIS
    // plus de 2 — pas une par position traversée. Les pages dépassées
    // ne partent pas ; les suivantes attendront un vol libre, la file
    // du coeur ne grandit pas.
    const demandees = await page.evaluate(
      () => window.__e2eJournal.filter((a) => a.commande === 'list_category').length,
    );
    expect(demandees).toBeGreaterThanOrEqual(1);
    expect(demandees).toBeLessThanOrEqual(2);
  } finally {
    // Libérer et nettoyer QUOI QU'IL ARRIVE : la suite est sérielle —
    // une retenue qui survivrait gèlerait tous les tests suivants, un
    // journal qui survivrait enregistrerait chaque appel du reste de
    // la suite.
    await page.evaluate(() => {
      window.__e2eLiberer?.();
      delete window.__e2eRetenue;
      delete window.__e2eLiberer;
      delete window.__e2eJournal;
    });
  }

  // Le coeur répond : la fenêtre COURANTE se sert en une paire
  // d'allers — lignes visibles, plus d'attente, sans drainer d'abord
  // une file de pages devenues invisibles.
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible({ timeout: 5000 });
  await expect(page.locator('[data-testid="ligne-attente"]')).toHaveCount(0, { timeout: 5000 });
});

test("l'écran vide ne s'affirme qu'après preuve — jamais « Aucun message ici. » sur une boîte pleine (E2)", async () => {
  // Transport RETENU : la page 0 du dossier qu'on ouvre ne répond pas.
  // L'écran doit MONTRER l'attente — pas affirmer un vide qu'il n'a pas
  // prouvé (le mensonge du constat terrain : « Aucun message ici. »
  // dans tous les dossiers pendant que la file se drainait).
  const liste = page.locator('[data-testid="liste"]');
  try {
    await page.evaluate(() => {
      window.__e2eRetenue = new Promise((liberer) => {
        window.__e2eLiberer = liberer;
      });
    });
    await dossier('reception').click();
    await expect(page.locator('[data-testid="liste-titre"]')).toHaveText('Boîte de réception');
    // Pendant le vol : jamais le message de vide, l'attente se montre.
    await expect(page.locator('[data-testid="ligne-attente"]').first()).toBeVisible();
    await expect(liste).not.toContainText('Aucun message ici.');
  } finally {
    // Libérer QUOI QU'IL ARRIVE : la suite est sérielle — une retenue
    // qui survivrait au test gèlerait tous les suivants.
    await page.evaluate(() => {
      window.__e2eLiberer?.();
      delete window.__e2eRetenue;
      delete window.__e2eLiberer;
    });
  }
  // La page 0 arrive : les lignes prennent la place de l'attente.
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
  await expect(page.locator('[data-testid="ligne-attente"]')).toHaveCount(0);
});
