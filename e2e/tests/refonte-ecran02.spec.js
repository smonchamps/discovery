// L'écran 02 de la refonte (PLAN-UI-V2 §P2), joué sur le décor Clarity :
// nav réelle, onglets filtrés côté coeur, volet de lecture, action
// réelle. Le fichier est nommé pour passer APRÈS les parcours v1
// (ordre alphabétique) : une seule reconstruction d'assets par gate.
import { mkdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';
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

test('la nav porte les pastilles de non-lus du décor Clarity (A29, W2-D4)', async () => {
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
  // Depuis A29 la nav ne dit QUE le non-lu, en pastille pleine — les
  // totaux (« 4 / 18 ») ont quitté la nav, la barre de statut les dit.
  // On vise l'élément pastille : le texte brut de la rangée porte le
  // nom de ligature de l'icône (« inventory_2 » a un chiffre).
  const pastille = (categorie) => dossier(categorie).locator('.pastille');
  await expect(pastille('reception')).toHaveText('4');
  await expect(dossier('reception')).not.toContainText('/');
  await expect(pastille('envoyes')).toHaveCount(0);
  await expect(pastille('brouillons')).toHaveCount(0);
  await expect(pastille('indesirables')).toHaveText('2');
  await expect(pastille('archives')).toHaveCount(0);
  await expect(pastille('corbeille')).toHaveCount(0);
  // Boîtes : l'agrégée + un rang par compte RÉEL ; la boîte en cours
  // (Toutes, au démarrage) est la tuile — l'identité seule, sans
  // compteur (A36, terrain E3).
  await expect(page.locator('[data-testid="nav-boite"]')).toHaveCount(3);
  await expect(page.locator('[data-testid="nav-boite"]').first()).toContainText('Toutes les boîtes');
  await expect(page.locator('[data-testid="nav-boite"]').first()).not.toContainText('non lus');
});

test('recharger garde les lignes servies — jamais de squelette (PLAN-REACTIVITE E1)', async () => {
  // La recharge que le cycle et les gestes déclenchent en rafale ne
  // doit JAMAIS repasser par les lignes d'attente : le transport est
  // RETENU (couture __e2eRetenue), la recharge part, et l'écran doit
  // montrer les MÊMES lignes — zéro « … » — jusqu'à l'arrivée de la
  // version fraîche. Avant E1, `recharger()` jetait les pages : ce
  // test montrait N squelettes, déterministe.
  const lignes = page.locator('[data-testid="ligne"]');
  const avant = await lignes.count();
  expect(avant).toBeGreaterThan(0);
  try {
    await page.evaluate(() => {
      window.__e2eRetenue = new Promise((liberer) => {
        window.__e2eLiberer = liberer;
      });
      window.__mesure.recharger();
    });
    // Le vol est ouvert (transport retenu), le DOM a re-rendu : les
    // lignes tiennent, aucune attente.
    await expect(page.locator('[data-testid="ligne-attente"]')).toHaveCount(0);
    await expect(lignes).toHaveCount(avant);
  } finally {
    // Libérer QUOI QU'IL ARRIVE : la suite est sérielle — une retenue
    // qui survivrait au test gèlerait tous les suivants.
    await page.evaluate(() => {
      window.__e2eLiberer?.();
      delete window.__e2eRetenue;
      delete window.__e2eLiberer;
    });
  }
  // La version fraîche a remplacé sans clignoter.
  await expect(lignes.first()).toBeVisible();
  await expect(page.locator('[data-testid="ligne-attente"]')).toHaveCount(0);
});

test("la barre d'état date la dernière relève — même sur échec", async () => {
  // Les comptes du décor n'ont pas de serveur : l'état STABLE ici est
  // l'échec de relève, et c'est justement lui qui doit dire depuis
  // quand on vit sur le stock (PLAN-SYNCHRO E1, maquette état 6). Le
  // décor Clarity pose `derniere_synchro` il y a 2 minutes — la minute
  // affichée peut glisser avec la durée du lancement, pas la forme.
  // (Le repos « Tous les messages sont à jour » reste couvert par la
  // spec onboarding, sans horodatage : boîte jamais relevée.)
  await expect(page.locator('[data-testid="progression"]')).toContainText(
    /Synchronisation impossible · nouvelle tentative automatique · dernière synchronisation il y a \d+ minutes?/,
  );
});

test('le bouton de relève vit dans la barre — « Réessayer » sur échec (E3)', async () => {
  // Même décor : la relève échoue, et le bouton devient le levier au
  // plus près de la panne (S-D1, maquette état 6). Le clic déclenche la
  // passe légère RÉELLE — les comptes du décor n'ont pas de serveur,
  // l'échec doit rester dit après le geste, et le bouton se réarmer.
  const bouton = page.locator('[data-testid="btn-releve"]');
  await expect(bouton).toBeVisible();
  await expect(bouton).toBeEnabled();
  await expect(bouton).toContainText('Réessayer');
  await bouton.click();
  await expect(page.locator('[data-testid="progression"]')).toContainText(
    /Synchronisation impossible/,
  );
  await expect(bouton).toBeEnabled();
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
  // Le total a quitté la nav (A29, W2-D4) : la preuve du coeur se lit
  // au dossier Archives — la barre de statut compte ses éléments.
  await dossier('archives').click();
  await expect(page.locator('[data-testid="statut"]')).toContainText('Archives · 65');
  await dossier('reception').click();
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
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
  // Le compte émetteur SE CHOISIT (A10) : deux comptes au décor, le
  // premier par défaut, l'autre sélectionnable.
  const de = page.locator('[data-testid="composition-de"]');
  await expect(de).toHaveValue('paul.merand@atelier-nord.fr');
  await expect(de.locator('option')).toHaveCount(2);
  await de.selectOption('paul@merand.fr');
  await expect(de).toHaveValue('paul@merand.fr');
  await page.locator('[data-testid="composition-annuler"]').click();
  await expect(page.locator('[data-testid="composition"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="toast"]')).toHaveCount(0);
});

test('« Répondre à tous » se tient entre Répondre et Transférer (A14)', async () => {
  // Pas de clic : le décor E2E est hors ligne garanti, et « Répondre à
  // tous » relit les destinataires sur le serveur (échec franc voulu).
  // Ici on prouve la place du bouton, dans les DEUX barres d'actions.
  const barre = await page
    .locator('[data-testid="volet-lecture"] .actions button')
    .evaluateAll((boutons) => boutons.map((bouton) => bouton.dataset.testid));
  expect(barre).toEqual(['repondre', 'repondre-tous', 'transferer', 'archiver', 'supprimer']);

  await page.locator('[data-testid="voir-conversation"]').click();
  const barreConv = await page
    .locator('[data-testid="conversation"] .actions button')
    .evaluateAll((boutons) => boutons.map((bouton) => bouton.dataset.testid));
  expect(barreConv).toEqual([
    'conv-repondre',
    'conv-repondre-tous',
    'conv-transferer',
    'conv-archiver',
    'conv-supprimer',
  ]);
  await page.locator('[data-testid="retour-boite"]').click();
  await expect(page.locator('[data-testid="lecture-sujet"]')).toHaveText(
    'Relecture du contrat Vantis',
  );
});

test("répondre préremplit depuis le coeur : adresse, Re :, amorce, citation — sans les pièces d'origine", async () => {
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
  // E3 (PJ-D4) : une réponse ne porte PAS les pièces d'origine — la
  // puce du prototype promettait un envoi qui n'existait pas, elle est
  // tombée avec la fiction.
  await expect(page.locator('[data-testid="composition"]')).not.toContainText(
    'Contrat_Vantis_v4.pdf',
  );
  await expect(page.locator('[data-testid="composition-pieces"]')).toHaveCount(0);
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

test("l'aperçu décode les entités HTML — jamais de résidu &eacute;", async () => {
  // Le corps du décor porte &eacute; et &nbsp; : le texte visible doit
  // être celui du prototype, sans une seule esperluette.
  const ligne = page.locator('[data-testid="ligne"]', { hasText: 'renouvellement du domaine' });
  await expect(ligne).toContainText('pour éviter toute interruption de service.');
  await expect(ligne).not.toContainText('&');
});

test('les fichiers joints se prennent AU VOLET — un message seul n\'a pas de conversation (Annexe A)', async () => {
  // « Compte rendu du 4 août » : message SEUL, une pièce jointe.
  await page.locator('[data-testid="ligne"]', { hasText: 'Compte rendu du 4 août' }).click();
  await expect(page.locator('[data-testid="lecture-fichiers"]')).toContainText('CR_04-08.pdf');
  await expect(
    page.locator('[data-testid="lecture-fichiers"] [data-testid="piece-jointe"]'),
  ).toBeEnabled();
});

test('la croix vide la recherche en un clic (verdict terrain)', async () => {
  await page.locator('[data-testid="champ-recherche"]').fill('Vantis');
  await expect(page.locator('[data-testid="resultats"]')).toBeVisible();
  await page.locator('[data-testid="vider-recherche"]').click();
  await expect(page.locator('[data-testid="champ-recherche"]')).toHaveValue('');
  await expect(page.locator('[data-testid="resultats"]')).toHaveCount(0);
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

test('le brouillon vit en liste : mention sur le fil, reprise au dossier, fente muette', async () => {
  // PLAN-BROUILLONS : la fente ne porte plus les brouillons — la
  // mention en Réception (variante B) et le dossier Brouillons, si.
  // Le brouillon du parcours P4 répond au fil Vantis : c'est LUI le
  // plus récent du fil, son corps prend l'aperçu.
  await expect(page.locator('[data-testid="fente-avis"]')).toHaveCount(0);
  const fil = page
    .locator('[data-testid="ligne"]', { hasText: 'Relecture du contrat Vantis' })
    .first();
  await expect(fil.locator('[data-testid="mention-brouillon"]')).toHaveText('Brouillon : ');
  await expect(fil).toContainText('Bonjour Camille,');

  // Le dossier : les brouillons LOCAUX (2 du décor + celui de P4), du
  // plus récent au plus ancien ; la barre de statut compte comme les
  // autres catégories ; le clic REPREND — jamais une lecture.
  await dossier('brouillons').click();
  await expect(page.locator('[data-testid="dossier-brouillons"]')).toBeVisible();
  await expect(page.locator('[data-testid="ligne-brouillon"]')).toHaveCount(3);
  await expect(page.locator('[data-testid="progression"]')).toContainText(
    'Brouillons · 3 éléments',
  );
  await page.locator('[data-testid="ligne-brouillon"]').first().click();
  await expect(page.locator('[data-testid="composition-objet"]')).toHaveValue(
    'Re : Relecture du contrat Vantis',
  );
  await expect(page.locator('[data-testid="composition-a"]')).toHaveValue(
    'c.rousseau@atelier-nord.fr',
  );

  // Vider puis fermer : le seul cas où fermer supprime — la ligne
  // quitte le dossier SANS attendre la sonde (onbrouillon).
  await page.locator('[data-testid="composition-a"]').fill('');
  await page.locator('[data-testid="composition-objet"]').fill('');
  await page.locator('[data-testid="composition-corps"]').fill('');
  await page.locator('[data-testid="composition-annuler"]').click();
  await expect(page.locator('[data-testid="composition"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="ligne-brouillon"]')).toHaveCount(2);

  // Retour en Réception : le fil Vantis garde sa mention — le brouillon
  // du DÉCOR le vise aussi, et c'est son corps qui reprend l'aperçu.
  await dossier('reception').click();
  const encore = page
    .locator('[data-testid="ligne"]', { hasText: 'Relecture du contrat Vantis' })
    .first();
  await expect(encore.locator('[data-testid="mention-brouillon"]')).toBeVisible();
  await expect(encore).toContainText('Merci pour la v4');
});

test('la conversation porte le brouillon en dernière position, le clic reprend (E3)', async () => {
  // La liste promettait un « dernier email » : l'écran 03 le tient
  // (B-D4-b) — bloc pointillé en fin de fil, corps du brouillon, clic
  // = reprise, la conversation reste montée sous le composeur.
  await page
    .locator('[data-testid="ligne"]', { hasText: 'Relecture du contrat Vantis' })
    .first()
    .click();
  await page.locator('[data-testid="voir-conversation"]').click();
  const bloc = page.locator('[data-testid="conv-brouillon"]');
  await expect(bloc).toContainText('Brouillon');
  await expect(bloc).toContainText('Merci pour la v4');
  await expect(bloc).toContainText('Reprendre');
  await bloc.click();
  await expect(page.locator('[data-testid="composition-objet"]')).toHaveValue(
    'Re : Relecture du contrat Vantis',
  );
  await expect(page.locator('[data-testid="composition-corps"]')).toHaveValue(/Merci pour la v4/);
  // Fermer conserve : le bloc reste, la conversation n'a pas bougé.
  await page.locator('[data-testid="composition-annuler"]').click();
  await expect(page.locator('[data-testid="composition"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="conversation"]')).toBeVisible();
  await expect(bloc).toBeVisible();
  // Retour boîte : la chaîne sérielle repart de la Réception.
  await page.locator('[data-testid="retour-boite"]').click();
  await expect(page.locator('[data-testid="conversation"]')).toHaveCount(0);
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
  // A13 : les thèmes vivent dans leur groupe, choisi au rail.
  await page.locator('[data-testid="reglages-groupe"][data-groupe="themes"]').click();
  await expect(page.locator('[data-testid="theme"]')).toHaveCount(7);
  await page.locator('[data-theme-id="nuit"]').click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'nuit');
  await page.locator('[data-testid="reglages-termine"]').click();
  await expect(page.locator('[data-testid="reglages-modal"]')).toHaveCount(0);
  // Persistance : le choix survit dans localStorage (rechargé au montage).
  expect(await page.evaluate(() => localStorage.getItem('wind-theme'))).toBe('nuit');
  // La coche suit le choix à la réouverture ; retour à `nature` pour ne
  // pas teinter d'autres parcours.
  await page.locator('[data-testid="reglages"]').click();
  await page.locator('[data-testid="reglages-groupe"][data-groupe="themes"]').click();
  await expect(page.locator('[data-theme-id="nuit"] .coche')).toBeVisible();
  await page.locator('[data-theme-id="nature"]').click();
  await page.locator('[data-testid="reglages-termine"]').click();
});

test("les réglages en deux volets se parcourent au clic ET au clavier (A13)", async () => {
  await page.locator('[data-testid="reglages"]').click();
  // Le rail porte les six groupes ; Comptes est le groupe d'ouverture.
  await expect(page.locator('[data-testid="reglages-groupe"]')).toHaveCount(6);
  await expect(page.locator('[data-testid="reglages-comptes"]')).toBeVisible();
  // Au clic : Raccourcis — la table D3 en référence, lecture seule.
  await page.locator('[data-testid="reglages-groupe"][data-groupe="raccourcis"]').click();
  await expect(page.locator('[data-testid="reglages-raccourcis"]')).toContainText('Suppr');
  await expect(page.locator('[data-testid="reglages-raccourcis"] kbd')).toHaveCount(7);
  // Au clavier (A8) : Entrée active le groupe comme le clic.
  await page.locator('[data-testid="reglages-groupe"][data-groupe="apropos"]').focus();
  await page.keyboard.press('Enter');
  // À propos : la version RÉELLE de l'application, pas un texte posé.
  await expect(page.locator('[data-testid="apropos-version"]')).toHaveText(/^\d+\.\d+\.\d+/);
  await expect(page.locator('[data-testid="reglages-apropos"]')).toContainText('Apache 2.0');
  // « Vérifier les mises à jour » traverse update_check pour de vrai ;
  // en E2E la commande répond « à jour » (aucun réseau, passation §7.5).
  await page.locator('[data-testid="apropos-verifier"]').click();
  await expect(page.locator('[data-testid="reglages-apropos"]')).toContainText(
    'Vous êtes à jour.',
  );
  await page.locator('[data-testid="reglages-termine"]').click();
  await expect(page.locator('[data-testid="reglages-modal"]')).toHaveCount(0);
});

test("la section Comptes liste les comptes réels et ouvre le guichet d'ajout (A11)", async () => {
  await page.locator('[data-testid="reglages"]').click();
  const section = page.locator('[data-testid="reglages-comptes"]');
  await expect(section).toContainText('paul.merand@atelier-nord.fr');
  await expect(section).toContainText('paul@merand.fr');
  // « Ajouter un compte » déplie LE guichet de l'écran 01 — même
  // implémentation : adresse, routage par domaine, champs génériques.
  await page.locator('[data-testid="reglages-ajouter"]').click();
  await page.locator('[data-testid="onboarding-adresse"]').fill('paul@exemple.fr');
  await page.locator('[data-testid="onboarding-continuer"]').click();
  await expect(page.locator('#ob-imap')).toHaveValue('imap.exemple.fr');
  // Rien n'est parti ; Terminé referme, le guichet se démonte propre.
  await page.locator('[data-testid="reglages-termine"]').click();
  await expect(page.locator('[data-testid="reglages-modal"]')).toHaveCount(0);
});

// ——— E2 des Réglages : les groupes à décision (R-D1, R-D2) —————————————

test("Affichage : le suivi de l'OS sombre affiche « La nuit » sans toucher au choix (D6)", async () => {
  await page.locator('[data-testid="reglages"]').click();
  await page.locator('[data-testid="reglages-groupe"][data-groupe="affichage"]').click();
  const bascule = page.locator('[data-testid="affichage-auto"]');
  await expect(bascule).toHaveAttribute('aria-checked', 'false');
  await bascule.click();
  // OS sombre : « La nuit » s'affiche ; le thème CHOISI reste `nature`.
  await page.emulateMedia({ colorScheme: 'dark' });
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'nuit');
  expect(await page.evaluate(() => localStorage.getItem('wind-theme'))).not.toBe('nuit');
  // OS clair : le choix revient tel quel.
  await page.emulateMedia({ colorScheme: 'light' });
  await expect(page.locator('html')).not.toHaveAttribute('data-theme', 'nuit');
  // Persistance : le booléen survit comme le thème.
  expect(await page.evaluate(() => localStorage.getItem('wind-theme-auto'))).toBe('1');
  await bascule.click();
  await page.emulateMedia({ colorScheme: null });
  await page.locator('[data-testid="reglages-termine"]').click();
});

test("Notifications : les bulles d'arrivée se coupent et la préférence tient en base (R-D2)", async () => {
  await page.locator('[data-testid="reglages"]').click();
  await page.locator('[data-testid="reglages-groupe"][data-groupe="notifications"]').click();
  const bascule = page.locator('[data-testid="notif-bulles"]');
  // Le défaut protège l'annonce : activées tant que rien n'est posé.
  await expect(bascule).toHaveAttribute('aria-checked', 'true');
  await bascule.click();
  await expect(bascule).toHaveAttribute('aria-checked', 'false');
  await page.locator('[data-testid="reglages-termine"]').click();
  // L'aller-retour RÉEL : recharger l'application relit la préférence
  // depuis la base — pas depuis un état de composant.
  await page.reload();
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
  await page.locator('[data-testid="reglages"]').click();
  await page.locator('[data-testid="reglages-groupe"][data-groupe="notifications"]').click();
  await expect(bascule).toHaveAttribute('aria-checked', 'false');
  // Retour au défaut pour ne pas teinter d'autres parcours.
  await bascule.click();
  await expect(bascule).toHaveAttribute('aria-checked', 'true');
  await page.locator('[data-testid="reglages-termine"]').click();
});

// ——— Pièces jointes (PLAN-PIECES-JOINTES E2) ——————————————————————————
// La boîte de dialogue native n'est pas pilotable : la couture
// `window.__e2ePieces` (transport.js) injecte les chemins de fixtures —
// le sélecteur ne s'ouvre jamais, tout le reste du chemin est le vrai.

const fixtures = path.resolve(import.meta.dirname, '..', '..', 'target', 'e2e', 'fixtures');

test('joindre est réel : puces nom + taille, poids total, retrait par puce', async () => {
  mkdirSync(fixtures, { recursive: true });
  const devis = path.join(fixtures, 'devis.pdf');
  const photo = path.join(fixtures, 'photo.jpg');
  writeFileSync(devis, Buffer.alloc(812 * 1024, 1));
  writeFileSync(photo, Buffer.alloc(2 * 1024 * 1024, 2));

  await page.locator('[data-testid="ecrire"]').click();
  await page.evaluate((chemins) => {
    window.__e2ePieces = chemins;
  }, [devis, photo]);
  await page.locator('[data-testid="composition-joindre"]').click();

  await expect(page.locator('[data-testid="piece-compo"]')).toHaveCount(2);
  await expect(page.locator('[data-testid="composition-pieces"]')).toContainText('devis.pdf');
  await expect(page.locator('[data-testid="composition-pieces"]')).toContainText('photo.jpg');
  // 812 Ko + 2 Mo — la même forme que les puces (point décimal du cœur).
  await expect(page.locator('[data-testid="composition-poids"]')).toContainText('2.8 Mo / 25 Mo');

  await page.locator('[data-testid="piece-retrait"]').first().click();
  await expect(page.locator('[data-testid="piece-compo"]')).toHaveCount(1);
  await expect(page.locator('[data-testid="composition-poids"]')).toContainText('2.0 Mo / 25 Mo');
});

test('fermer conserve les pièces, la reprise les restitue (PJ-D1)', async () => {
  await page.locator('[data-testid="composition-corps"]').fill('Corps avec pièce E2');
  await page.locator('[data-testid="composition-annuler"]').click();
  await expect(page.locator('[data-testid="composition"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="toast"]')).toContainText('Brouillon enregistré.');

  await dossier('brouillons').click();
  await expect(page.locator('[data-testid="dossier-brouillons"]')).toBeVisible();
  await page
    .locator('[data-testid="ligne-brouillon"]', { hasText: 'Corps avec pièce E2' })
    .click();
  await expect(page.locator('[data-testid="composition"]')).toBeVisible();
  await expect(page.locator('[data-testid="piece-compo"]')).toHaveCount(1);
  await expect(page.locator('[data-testid="composition-pieces"]')).toContainText('photo.jpg');
});

test("envoyer emporte la pièce : le journal la porte (PJ-D2)", async () => {
  await page.locator('[data-testid="composition-a"]').fill('dest@exemple.fr');
  await page.locator('[data-testid="composition-objet"]').fill('Envoi avec pièce E2');
  await page.locator('[data-testid="composition-envoyer"]').click();
  await expect(page.locator('[data-testid="composition"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="toast"]')).toContainText('Message envoyé.');

  // Les comptes du décor n'ont pas de serveur : l'envoi reste journalisé
  // en file — et le journal doit porter la pièce (assertion PJ-D2).
  const statut = await page.evaluate(() => window.__TAURI__.core.invoke('outbox_status'));
  const entree = statut.entries.find((e) => e.subject === 'Envoi avec pièce E2');
  expect(entree).toBeTruthy();
  expect(entree.pieces).toBe(1);
});

test('au-delà du plafond : le refus est dit, rien ne se joint (PJ-D3)', async () => {
  const enorme = path.join(fixtures, 'enorme.bin');
  writeFileSync(enorme, Buffer.alloc(26 * 1024 * 1024));

  await page.locator('[data-testid="ecrire"]').click();
  await page.evaluate((chemin) => {
    window.__e2ePieces = [chemin];
  }, enorme);
  await page.locator('[data-testid="composition-joindre"]').click();

  await expect(page.locator('[data-testid="composition-refus"]')).toContainText('enorme.bin');
  await expect(page.locator('[data-testid="composition-refus"]')).toContainText(
    'dépasse la place restante',
  );
  await expect(page.locator('[data-testid="piece-compo"]')).toHaveCount(0);

  await page.evaluate(() => {
    delete window.__e2ePieces;
  });
  await page.locator('[data-testid="composition-annuler"]').click();
  await expect(page.locator('[data-testid="composition"]')).toHaveCount(0);
});

test('le transfert rapatrie pour de vrai — hors ligne : échec dit, « Réessayer », envoi gardé (PJ-D4)', async () => {
  // Le parcours précédent vivait au dossier Brouillons : retour en
  // Réception, où la ligne Vantis existe.
  await dossier('reception').click();
  await page
    .locator('[data-testid="ligne"]', { hasText: 'Relecture du contrat Vantis' })
    .click();
  await page.locator('[data-testid="transferer"]').click();
  await expect(page.locator('[data-testid="composition-kicker"]')).toHaveText('Transférer');
  // Les comptes du décor n'ont pas de serveur : chaque rapatriement finit
  // en échec — nom en alerte, « Réessayer » — jamais une puce pleine, et
  // jamais une pièce silencieusement absente.
  await expect(page.locator('[data-testid="piece-echec"]').first()).toBeVisible();
  // Le dernier message du fil Vantis porte l'annexe tarifaire.
  await expect(page.locator('[data-testid="composition-pieces"]')).toContainText(
    'Annexe_tarifs.xlsx',
  );
  await expect(page.locator('[data-testid="piece-compo"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="piece-reessayer"]').first()).toBeVisible();

  // Envoyer est BLOQUÉ tant que des pièces manquent.
  await page.locator('[data-testid="composition-a"]').fill('dest@exemple.fr');
  await page.locator('[data-testid="composition-envoyer"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText(
    'Des pièces du transfert manquent',
  );
  await expect(page.locator('[data-testid="composition"]')).toBeVisible();

  // Renoncer (la croix) est le geste EXPLICITE qui libère l'envoi.
  const echecs = page.locator('[data-testid="piece-echec"]');
  const restantes = await echecs.count();
  for (let i = 0; i < restantes; i += 1) {
    await page.locator('[data-testid="piece-renoncer"]').first().click();
  }
  await expect(echecs).toHaveCount(0);
  await page.locator('[data-testid="composition-envoyer"]').click();
  await expect(page.locator('[data-testid="composition"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="toast"]')).toContainText('Message envoyé.');
});

// P0-bis (PLAN-SYNCHRO) : la coupure réseau se DIT à l'instant, sans
// attendre qu'un cycle cale sur le timeout socket. On pilote l'événement
// que l'OS émettrait (navigator.onLine lui-même n'est pas scriptable) :
// le câblage événement → barre est ce qui compte.
test("hors ligne : la barre le dit à l'instant, le retour la restaure (P0-bis)", async () => {
  await dossier('reception').click();
  const progression = page.locator('[data-testid="progression"]');
  await expect(progression).not.toContainText('Hors ligne');

  await page.evaluate(() => window.dispatchEvent(new Event('offline')));
  await expect(progression).toContainText('Hors ligne');

  await page.evaluate(() => window.dispatchEvent(new Event('online')));
  await expect(progression).not.toContainText('Hors ligne');
});

// E3 (PLAN-REACTIVITE, R-D1 « < 1 s ») : la destination d'un geste se
// montre depuis la base locale — les comptes du décor n'ont PAS de
// serveur, ce parcours est donc exactement le contrat hors ligne :
// suppression → écho visible en Corbeille tout de suite, compteur
// d'accord avec la liste, corps ouvrable en local ; le geste sur un
// écho est différé et LE DIT ; l'écho survit (l'action attend encore —
// le balayage ne retire jamais une intention en attente).
test("supprimer se voit en Corbeille à l'instant — hors ligne compris (E3)", async () => {
  await dossier('reception').click();

  await page
    .locator('[data-testid="ligne"]', { hasText: 'Facture 2026-0841' })
    .first()
    .click();
  await page.locator('[data-testid="supprimer"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText(
    'Conversation supprimée.',
  );
  // Le compteur a quitté la nav (A29, W2-D4) : la Corbeille elle-même
  // dit « 3 + l'écho » — la barre de statut compte ses éléments.
  await dossier('corbeille').click();
  await expect(page.locator('[data-testid="statut"]')).toContainText(
    'Corbeille · 4 éléments',
  );
  const echo = page.locator('[data-testid="ligne"]', { hasText: 'Facture 2026-0841' });
  await expect(echo).toBeVisible();

  // L'écho s'ouvre en LOCAL (echo_body) — le volet porte le sujet.
  await echo.click();
  await expect(page.locator('[data-testid="lecture-sujet"]')).toContainText(
    'Facture 2026-0841',
  );
  // Un geste sur l'écho attend la réconciliation — et le dit.
  await page.locator('[data-testid="supprimer"]').click();
  await expect(page.locator('[data-testid="toast"]')).toContainText(
    'Copie en cours de synchronisation',
  );
  // L'écho vit toujours : son intention (l'action journalisée) attend
  // le serveur — hors ligne, rien ne le balaie.
  await expect(echo).toBeVisible();
  await dossier('reception').click();
});

// ——— La course « vider puis fermer » (constat terrain du 2026-08-15) ——
// Une sauvegarde différée partie AVANT le vidage porte encore du
// contenu : sans sérialisation, son bilan ressuscitait le brouillon
// que fermer venait de supprimer (fantôme au dossier — vu deux fois
// par la suite sous charge). La retenue du transport rend la course
// déterministe : l'écriture est EN VOL quand le geste décide.
test('vider puis fermer ne ressuscite jamais le brouillon — la sauvegarde en vol se pose avant', async () => {
  await page.keyboard.press('c');
  await page.locator('[data-testid="composition-objet"]').fill('Course E2E');
  await page.locator('[data-testid="composition-corps"]').fill('Premier contenu.');
  // Première sauvegarde COMPLÈTE : le brouillon a un id.
  await page.waitForTimeout(2600);
  // Deuxième écriture, puis retenue : la sauvegarde part et se BLOQUE.
  await page.locator('[data-testid="composition-corps"]').fill('Contenu condamné.');
  await page.evaluate(() => {
    window.__e2eRetenue = new Promise((liberer) => {
      window.__e2eLiberer = liberer;
    });
  });
  await page.waitForTimeout(2300);
  // Le vidage et le geste, pendant le vol.
  await page.locator('[data-testid="composition-objet"]').fill('');
  await page.locator('[data-testid="composition-corps"]').fill('');
  await page.locator('[data-testid="composition-annuler"]').click();
  await page.evaluate(() => {
    window.__e2eLiberer?.();
    delete window.__e2eRetenue;
    delete window.__e2eLiberer;
  });
  // fermer a attendu le vol, puis supprimé : aucun fantôme.
  await expect(page.locator('[data-testid="composition"]')).toHaveCount(0);
  await page.locator('[data-testid="nav-dossier"][data-categorie="brouillons"]').click();
  await expect(page.locator('[data-testid="dossier-brouillons"]')).toBeVisible();
  await expect(
    page.locator('[data-testid="ligne-brouillon"]', { hasText: 'Course E2E' }),
  ).toHaveCount(0);
  await page.locator('[data-testid="nav-dossier"][data-categorie="reception"]').click();
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
});
