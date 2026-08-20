// L'écran 02 de la refonte (PLAN-UI-V2 §P2), joué sur le décor Clarity :
// nav réelle, onglets filtrés côté coeur, volet de lecture, action
// réelle. Le fichier est nommé pour passer APRÈS les parcours v1
// (ordre alphabétique) : une seule reconstruction d'assets par gate.
import { mkdirSync, writeFileSync } from 'node:fs';
import { execSync } from 'node:child_process';
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

test('le volet liste porte son bandeau de titre — le nom de la boîte, sans bouton (UI v3, E1)', async () => {
  // Verdict CE du 2026-08-16 (ANNOTATIONS-V3 §3) : le bandeau de la
  // maquette Classique entre, SANS « Tout marquer lu » — le titre seul.
  const titre = page.locator('[data-testid="liste-titre"]');
  await expect(titre).toHaveText('Boîte de réception');
  await expect(titre.locator('button')).toHaveCount(0);
  // PLAN-RETOURS-V3 R2 : le bandeau du haut au MÊME format visuel que
  // le bandeau de filtre du bas — même hauteur (52 px), même fond
  // (--panel), un filet le sépare de la liste comme le filet du bas.
  const gabarit = (loc) =>
    loc.evaluate((el) => {
      const s = getComputedStyle(el);
      return { h: el.offsetHeight, fond: s.backgroundColor };
    });
  const haut = await gabarit(titre);
  const bas = await gabarit(page.locator('[data-testid="onglets"]'));
  expect(haut.h).toBe(bas.h);
  expect(haut.fond).toBe(bas.fond);
  // La valeur calculée est arrondie au pixel MACHINE (0.666667px à
  // l'échelle 150 %) : on asserte l'existence du filet, pas sa cote.
  const filet = await titre.evaluate(
    (el) => parseFloat(getComputedStyle(el).borderBottomWidth),
  );
  expect(filet).toBeGreaterThan(0);
  // Le bandeau suit la boîte courante.
  await dossier('archives').click();
  await expect(titre).toHaveText('Archives');
  // Retour à l'état de départ : la suite est sérielle.
  await dossier('reception').click();
  await expect(titre).toHaveText('Boîte de réception');
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
});

test("la ligne de liste porte l'avatar aux initiales — visuel seul (UI v3, E2)", async () => {
  // Verdict CE du 2026-08-16 (ANNOTATIONS-V3 §4, décision D2) :
  // l'avatar 28 px de la maquette entre au gabarit, SANS geste — la
  // sélection en lot est une feature à part, différée.
  const premiere = page.locator('[data-testid="ligne"]').first();
  const avatar = premiere.locator('[data-testid="avatar"]');
  await expect(avatar).toBeVisible();
  // Les initiales disent l'expéditeur de la rangée (deux lettres).
  const exp = (await premiere.locator('.exp').innerText()).trim();
  const attendu = exp.split(/\s+/).slice(0, 2).map((m) => m[0]).join('').toUpperCase();
  await expect(avatar).toHaveText(attendu);
  // Visuel seul : jamais un bouton, rien à activer.
  expect(await avatar.evaluate((el) => el.tagName)).not.toBe('BUTTON');
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

test('pendant un cycle, le trait hitofude de la barre porte son animation SMIL (A40)', async () => {
  // Constat terrain 2026-08-15 (PLAN-GELS) : le trait restait fixe
  // pendant la synchronisation. Le chemin animé vit dans le <mask>,
  // sous-arbre non rendu où Chromium ne fait PAS tourner les
  // animations CSS — la boucle était morte-née (playState `idle`,
  // prouvé sur la vraie fenêtre). Depuis A40 le tracé est SMIL
  // (<animate>, qui tourne dans un mask). Le cycle du décor est bref
  // (comptes sans serveur) : on s'assert sur la PRÉSENCE de l'<animate>
  // dans le trait `vague` attrapé pendant la fenêtre — c'est elle que
  // la régression (retour au CSS) ferait disparaître ; la vie de
  // l'horloge SMIL est une garantie moteur, pas un comportement à nous.
  const bouton = page.locator('[data-testid="btn-releve"]');
  await expect(bouton).toBeEnabled();
  await bouton.click();
  await expect(
    page.locator('[data-testid="statut"] path.boucle animate'),
  ).toBeAttached({ timeout: 8000 });
});

test('sélectionner ouvre le volet, lit le corps, et le non-lu tombe', async () => {
  await page.locator('[data-testid="ligne"]').first().click();
  await expect(page.locator('[data-testid="volet-lecture"] [data-testid="fil-sujet"]')).toHaveText(
    'Relecture du contrat Vantis',
  );
  // Le corps vit dans l'iframe sandbox — invariant S1.
  await expect(
    page.frameLocator('[data-testid="volet-lecture"] iframe').locator('body'),
  ).toContainText('Bonjour Paul');
  // mark_seen est RÉEL : le héros de la réception retombe.
  await expect(dossier('reception')).toContainText('3');
});

test('un lien du corps part au navigateur système — le corps ne bouge pas', async () => {
  // Constat terrain 2026-08-15 : le clic naviguait l'iframe sandbox
  // vers le site, refusé (X-Frame-Options / CSP) — WebView2 remplaçait
  // le corps par sa page « Ce contenu a été bloqué ». Depuis, le clic
  // est intercepté (lib/liens.js) et part à open_link ; la couture
  // `__e2eLiens` capte l'URL au lieu d'ouvrir un navigateur réel —
  // tout l'amont (iframe allow-same-origin, interception, filtre de
  // schéma) est le vrai chemin.
  await page.evaluate(() => {
    window.__e2eLiens = [];
  });
  const cadre = page.frameLocator('[data-testid="volet-lecture"] iframe');
  try {
    await cadre.locator('a[href="https://espace.exemple/vantis"]').click();
    await expect
      .poll(() => page.evaluate(() => window.__e2eLiens))
      .toEqual(['https://espace.exemple/vantis']);
  } finally {
    await page.evaluate(() => {
      delete window.__e2eLiens;
    });
  }
  // Le corps est toujours là — jamais de page « contenu bloqué ».
  await expect(cadre.locator('body')).toContainText('Bonjour Paul');
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

test('le volet de lecture montre le FIL en cartes — anciens repliés, dernier déplié (UI v3, E3)', async () => {
  // Verdict CE du 2026-08-16 (ANNOTATIONS-V3 §6, décision D4) : le
  // volet et l'écran 03 sont deux cadres du MÊME objet (Fil) — ici le
  // cadre volet : titre, cartes repliées une ligne, dernière dépliée
  // dans sa propre iframe sandbox (S1 intact).
  await page.locator('[data-testid="ligne"]').first().click();
  const volet = page.locator('[data-testid="volet-lecture"]');
  await expect(volet.locator('[data-testid="fil-sujet"]')).toHaveText(
    'Relecture du contrat Vantis',
  );
  await expect(volet.locator('[data-testid="message-replie"]')).toHaveCount(2);
  await expect(volet.locator('[data-testid="message-deplie"]')).toHaveCount(1);
  await expect(
    volet.frameLocator('[data-testid="message-deplie"] iframe').locator('body'),
  ).toContainText('Bonjour Paul');
  // Les fichiers joints du dernier message, dans le volet.
  await expect(volet.locator('[data-testid="message-deplie"]')).toContainText(
    'Contrat_Vantis_v4.pdf',
  );
});

test('le fil au dessin exact de la maquette — avatars, adresse · destinataire, heure longue (terrain A45)', async () => {
  // Retour CE du 2026-08-16 (captures du volet du prototype Classique,
  // ANNOTATIONS-V3 §6) : puces d'inventaire à gauche — n messages
  // TOUJOURS dit, fichiers SOMMÉS sur le fil —, boutons nus à droite,
  // cartes aux avatars, en-tête déplié « adresse · à destinataire »,
  // heure longue ; le bloc De/À/Objet a disparu.
  await page.locator('[data-testid="ligne"]').first().click();
  const volet = page.locator('[data-testid="volet-lecture"]');
  const puces = volet.locator('[data-testid="fil-puces"]');
  // 3 messages, et 3 fichiers = la somme du fil sur CE décor (PDF +
  // XLSX de Camille, après-scan, + XLSX de Sofia) — la ligne seule
  // n'en portait qu'un. La somme se stabilise quand message_body a
  // servi le compte d'après-scan du dernier message (2, pas 1) :
  // asserter 2 attrapait la valeur d'avant-scan, par course.
  await expect(puces).toContainText('3 messages');
  await expect(puces).toContainText('3 fichiers');
  // Les boutons de droite sont NUS (bouton, sans bordure ni fond).
  for (const testid of ['voir-conversation', 'tout-deplier']) {
    const bouton = volet.locator(`[data-testid="${testid}"]`);
    await expect(bouton).toHaveClass(/nu/);
    expect(await bouton.evaluate((el) => el.tagName)).toBe('BUTTON');
  }
  // Les cartes portent l'avatar aux initiales, comme la liste (E2).
  const replies = volet.locator('[data-testid="message-replie"]');
  await expect(replies.nth(0).locator('.avatar')).toHaveText('PM');
  await expect(replies.nth(1).locator('.avatar')).toHaveText('SN');
  const deplie = volet.locator('[data-testid="message-deplie"]');
  await expect(deplie.locator('.avatar')).toHaveText('CR');
  // L'en-tête déplié : « adresse · à destinataire » — le nom du compte
  // vient de notre propre copie du fil (Envoyés) — et l'heure longue.
  await expect(deplie.locator('.adr')).toHaveText(
    'c.rousseau@atelier-nord.fr · à Paul Mérand',
  );
  await expect(deplie.locator('.tete-message .quand')).toHaveText(/^Aujourd'hui, 09:12$/);
  await expect(replies.nth(0).locator('.quand')).toHaveText(/, 18:20$/);
  await expect(replies.nth(1).locator('.quand')).toHaveText(/, 11:05$/);
  // Le bloc De/À/Objet n'existe plus (la maquette dit tout en tête).
  await expect(deplie.locator('dl')).toHaveCount(0);
});

test('le fil au message seul dit « 1 message » — et s\'ouvre sur « Tout replier » (terrains A45/A47)', async () => {
  // La seconde capture CE : un fil d'un message garde le rang complet.
  // Le test « archiver » a rangé Planning aux Archives — on l'y suit.
  await dossier('archives').click();
  await page
    .locator('[data-testid="ligne"]', { hasText: 'Planning de la semaine 33' })
    .first()
    .click();
  const volet = page.locator('[data-testid="volet-lecture"]');
  const puces = volet.locator('[data-testid="fil-puces"]');
  await expect(puces).toContainText('1 message');
  await expect(puces).not.toContainText('fichier');
  // A47 : le message seul s'ouvre DÉPLIÉ — la bascule, dérivée de
  // l'état, dit donc « Tout replier » dès l'ouverture.
  await expect(volet.locator('[data-testid="tout-replier"]')).toBeVisible();
  const deplie = volet.locator('[data-testid="message-deplie"]');
  await expect(deplie.locator('.avatar')).toHaveText('YB');
  // Sans copie à nous dans le fil, le destinataire est l'adresse du
  // compte — le fait honnête, le cœur ne connaît pas notre nom.
  await expect(deplie.locator('.adr')).toHaveText(
    'y.belkacem@atelier-nord.fr · à paul.merand@atelier-nord.fr',
  );
  await expect(deplie.locator('.tete-message .quand')).toHaveText(/^Aujourd'hui, 08:40$/);
  await dossier('reception').click();
  await page.locator('[data-testid="ligne"]').first().click();
});

test("le volet est à plat, « Ouvrir » et « Déplier » à leur glyphe propre (terrain A46)", async () => {
  // Retours CE du 2026-08-16 : le volet ne s'enferme plus dans une
  // élévation — il défile en un seul flot, la tête du fil sans filet
  // (dessin .voletLecture du prototype) ; « Voir la conversation »
  // devient « Ouvrir » (open_in_full — une icône, un sens, A3) ; les
  // libellés de bascule sont « Tout déplier »/« Tout replier » (A47).
  await page.locator('[data-testid="ligne"]').first().click();
  const volet = page.locator('[data-testid="volet-lecture"]');
  const ouvrir = volet.locator('[data-testid="voir-conversation"]');
  await expect(ouvrir).toContainText('Ouvrir');
  await expect(ouvrir.locator('.ms')).toHaveText('open_in_full');
  const deplier = volet.locator('[data-testid="tout-deplier"]');
  await expect(deplier).toContainText('Tout déplier');
  await expect(deplier.locator('.ms')).toHaveText('unfold_more');
  // À plat : le volet lui-même défile, la tête ne porte aucun filet.
  expect(await volet.evaluate((el) => getComputedStyle(el).overflowY)).toBe('auto');
  expect(
    await volet.locator('.tete').evaluate((el) => getComputedStyle(el).borderBottomWidth),
  ).toBe('0px');
});

test("la bascule « Tout déplier »/« Tout replier » SUIT l'état réel des dépliages (terrain A47)", async () => {
  const volet = page.locator('[data-testid="volet-lecture"]');
  await volet.locator('[data-testid="tout-deplier"]').click();
  await expect(volet.locator('[data-testid="message-deplie"]')).toHaveCount(3);
  await expect(volet.locator('[data-testid="tout-deplier"]')).toHaveCount(0);
  const replier = volet.locator('[data-testid="tout-replier"]');
  await expect(replier).toContainText('Tout replier');
  await expect(replier.locator('.ms')).toHaveText('unfold_less');
  // Dérivée de l'état (A47, renverse le « geste seul » d'A46) :
  // replier un message à la MAIN la fait retomber sur « Tout
  // déplier »…
  await volet.locator('[data-testid="message-deplie"]').first().locator('.tete-message').click();
  await expect(volet.locator('[data-testid="message-replie"]')).toHaveCount(1);
  await expect(volet.locator('[data-testid="tout-deplier"]')).toBeVisible();
  // …et le redéplier à la main la remet sur « Tout replier ».
  await volet.locator('[data-testid="message-replie"]').click();
  await expect(replier).toBeVisible();
  // « Tout replier » referme TOUT — le dernier compris.
  await replier.click();
  await expect(volet.locator('[data-testid="message-deplie"]')).toHaveCount(0);
  await expect(volet.locator('[data-testid="message-replie"]')).toHaveCount(3);
  await expect(volet.locator('[data-testid="tout-deplier"]')).toBeVisible();
  // Remettre le fil dans l'état d'ouverture : le dernier déplié.
  await volet.locator('[data-testid="message-replie"]').last().click();
  await expect(volet.locator('[data-testid="message-deplie"]')).toHaveCount(1);
});

test('la hauteur du corps suit le contenu — jamais de gabarit fixe (terrain A47)', async () => {
  // Le corps de Camille est court : l'iframe colle à son document
  // (l'ancien plancher figeait 220 px), à l'épaisseur d'un filet près.
  const volet = page.locator('[data-testid="volet-lecture"]');
  const corps = volet.locator('[data-testid="message-deplie"] iframe');
  await expect(
    volet.frameLocator('[data-testid="message-deplie"] iframe').locator('body'),
  ).toContainText('Bonjour Paul');
  // La preuve NON circulaire : le contenu se mesure iframe à zéro
  // (scrollHeight ≥ hauteur posée sinon), puis on compare à la
  // hauteur posée — elles coïncident, au filet près.
  const mesure = () =>
    corps.evaluate((el) => {
      const posee = el.offsetHeight;
      el.style.height = '0';
      const brut = el.contentDocument.documentElement.scrollHeight;
      el.style.height = `${posee}px`;
      return { posee, brut };
    });
  await expect
    .poll(async () => {
      const { posee, brut } = await mesure();
      return posee > 60 && Math.abs(posee - brut) <= 2;
    })
    .toBe(true);
});

test("l'entête de composition ne répète plus l'objet, « De » colle à l'entête (terrain A46)", async () => {
  // Reprendre le brouillon du fil Vantis : la fenêtre s'ouvre comme
  // avant — mais l'entête ne porte plus le rappel d'objet (le champ
  // Objet le dit dessous), et l'écart entête → « De » est celui du
  // composeur du prototype (6 px).
  await page.locator('[data-testid="conv-brouillon"]').click();
  const compo = page.locator('[data-testid="composition"]');
  await expect(compo).toBeVisible();
  await expect(compo.locator('[data-testid="composition-kicker"]')).toBeVisible();
  // L'objet du brouillon ne vit que dans SON champ (valeur d'input,
  // hors textContent) — aucun rappel en texte dans la fenêtre.
  await expect(compo).not.toContainText('Relecture du contrat Vantis');
  expect(
    await compo
      .locator('[data-testid="composition-de"]')
      .evaluate((el) => getComputedStyle(el.closest('.champs')).paddingTop),
  ).toBe('6px');
  await page.locator('[data-testid="composition-annuler"]').click();
  await expect(compo).toHaveCount(0);
});

// ——— Écran 03 : la conversation plein écran (P3) ————————————————————

test('voir la conversation ouvre le fil plein écran, dernier message déplié', async () => {
  await page.locator('[data-testid="ligne"]').first().click();
  await page.locator('[data-testid="voir-conversation"]').click();
  await expect(page.locator('[data-testid="conversation"] [data-testid="fil-sujet"]')).toHaveText(
    'Relecture du contrat Vantis',
  );
  // Exclusivité des cadres (D4, revue v3) : UN SEUL Fil monté.
  await expect(page.locator('[data-testid="fil-sujet"]')).toHaveCount(1);
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
  await expect(page.locator('[data-testid="fil-sujet"]')).toHaveText(
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

test('« Répondre à tous » se tient entre Répondre et Transférer, par message (A14, R4/D4)', async () => {
  // R4 (PLAN-RETOURS-3, D4) : les gestes de réponse vivent EN BAS de
  // chaque message — A14 tient toujours, « Répondre à tous » entre
  // Répondre et Transférer. La barre du FIL ne garde que le TRI (D5) et
  // « Signaler comme spam » (R2/D2). Pas de clic : hors ligne garanti,
  // « Répondre à tous » relit les destinataires sur le serveur.
  const barreMsg = await page
    .locator('[data-testid="volet-lecture"] [data-testid="actions-message"]')
    .last()
    .locator('button')
    .evaluateAll((boutons) => boutons.map((bouton) => bouton.dataset.testid));
  expect(barreMsg).toEqual(['repondre', 'repondre-tous', 'transferer']);
  const barreFil = await page
    .locator('[data-testid="volet-lecture"] .actions button')
    .evaluateAll((boutons) => boutons.map((bouton) => bouton.dataset.testid));
  expect(barreFil).toEqual(['archiver', 'supprimer', 'signaler-spam']);

  await page.locator('[data-testid="voir-conversation"]').click();
  const barreMsgConv = await page
    .locator('[data-testid="conversation"] [data-testid="actions-message"]')
    .last()
    .locator('button')
    .evaluateAll((boutons) => boutons.map((bouton) => bouton.dataset.testid));
  expect(barreMsgConv).toEqual(['repondre', 'repondre-tous', 'transferer']);
  const barreFilConv = await page
    .locator('[data-testid="conversation"] .actions button')
    .evaluateAll((boutons) => boutons.map((bouton) => bouton.dataset.testid));
  expect(barreFilConv).toEqual(['archiver', 'supprimer', 'signaler-spam']);

  await page.locator('[data-testid="retour-boite"]').click();
  await expect(page.locator('[data-testid="fil-sujet"]')).toHaveText(
    'Relecture du contrat Vantis',
  );
});

test("répondre préremplit depuis le coeur : adresse, Re :, amorce, citation — sans les pièces d'origine", async () => {
  // R4 : la réponse est PAR message ; le dernier message déplié du fil
  // Vantis est celui de Camille Rousseau (`.last()`).
  await page.locator('[data-testid="repondre"]').last().click();
  await expect(page.locator('[data-testid="composition-kicker"]')).toHaveText('Répondre');
  await expect(page.locator('[data-testid="composition-a"]')).toHaveValue(
    'c.rousseau@atelier-nord.fr',
  );
  await expect(page.locator('[data-testid="composition-objet"]')).toHaveValue(
    'Re : Relecture du contrat Vantis',
  );
  const corps = await page.locator('[data-testid="composition-corps"]').innerText();
  // L'ÉCART amorce → citation fait partie du contrat (une ligne vide,
  // pas quatre) : l'assertion mesure les deux sauts, pas juste l'amorce.
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
  // R4 : la réponse est PAR message ; le dernier message déplié du fil
  // Vantis est celui de Camille Rousseau (`.last()`).
  await page.locator('[data-testid="repondre"]').last().click();
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
  // R2 (PLAN-RETOURS-4, D4) : nom ET poids dans la MEME puce cliquable —
  // une seule puce par piece, portant les deux informations.
  const puce = page.locator('[data-testid="lecture-fichiers"] [data-testid="piece-jointe"]');
  await expect(puce).toHaveCount(1);
  await expect(puce).toContainText('CR_04-08.pdf');
  await expect(puce).toContainText('220 Ko');
  await expect(puce).toBeEnabled();
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

test('R3 : le corps reste sur dalle claire même sous un thème sombre (PLAN-RETOURS-4, D3)', async () => {
  // La dalle sombre d'A42 rendait illisible le texte à couleurs
  // d'expéditeur (terrain 2026-08-18). Le corps bake désormais TOUJOURS
  // une dalle claire (mail-render Palette::default — fond blanc, encre
  // sombre), quel que soit le thème : le front ne transmet plus de
  // palette. On force un thème -nuit AVANT d'ouvrir le message (ouvrirFil
  // vide le cache des corps → relève fraîche sous ce thème) ; l'ancien
  // code aurait baké un fond sombre ici — réintroduire une palette de
  // thème casserait ce test.
  await page.evaluate(() => { document.documentElement.dataset.theme = 'estampe-nuit'; });
  await page.locator('[data-testid="ligne"]', { hasText: 'renouvellement du domaine' }).click();
  await expect(page.locator('[data-testid="garde-images"]')).toBeVisible();
  const srcdoc = await page.locator('iframe.corps').first().getAttribute('srcdoc');
  expect(srcdoc).toContain('background:#ffffff');
  expect(srcdoc).toContain('color:#222222');
  expect(srcdoc).not.toContain('color-scheme:dark');
  await page.evaluate(() => { delete document.documentElement.dataset.theme; });
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
  // `fill('')` sur un contenteditable est un no-op Chromium : on vide
  // comme l'utilisateur — tout sélectionner, supprimer.
  await page.locator('[data-testid="composition-corps"]').click();
  await page.keyboard.press('Control+a');
  await page.keyboard.press('Delete');
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
  await expect(page.locator('[data-testid="composition-corps"]')).toContainText('Merci pour la v4');
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
  await expect(page.locator('[data-testid="volet-lecture"] [data-testid="fil-sujet"]')).not.toBeEmpty();
  // Retour réception par le clavier.
  await page.locator('[data-testid="nav-dossier"][data-categorie="reception"]').focus();
  await page.keyboard.press('Enter');
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
});

test('les réglages appliquent et persistent le thème', async () => {
  await page.locator('[data-testid="reglages"]').click();
  // A13 : les thèmes vivent dans leur groupe, choisi au rail.
  await page.locator('[data-testid="reglages-groupe"][data-groupe="themes"]').click();
  // A42 : 28 fiches — 14 claires et leurs 14 déclinaisons -nuit,
  // toutes choisissables (décision D1 de PLAN-WADA-ELARGI).
  await expect(page.locator('[data-testid="theme"]')).toHaveCount(28);
  await page.locator('[data-theme-id="nature-nuit"]').click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'nature-nuit');
  await page.locator('[data-testid="reglages-termine"]').click();
  await expect(page.locator('[data-testid="reglages-modal"]')).toHaveCount(0);
  // Persistance : le choix survit dans localStorage (rechargé au montage).
  expect(await page.evaluate(() => localStorage.getItem('wind-theme'))).toBe('nature-nuit');
  // La coche suit le choix à la réouverture ; retour à `nature` pour ne
  // pas teinter d'autres parcours.
  await page.locator('[data-testid="reglages"]').click();
  await page.locator('[data-testid="reglages-groupe"][data-groupe="themes"]').click();
  await expect(page.locator('[data-theme-id="nature-nuit"] .coche')).toBeVisible();
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

test("Affichage : le suivi de l'OS sombre suffixe le thème choisi en -nuit (D6, A42)", async () => {
  await page.locator('[data-testid="reglages"]').click();
  await page.locator('[data-testid="reglages-groupe"][data-groupe="affichage"]').click();
  const bascule = page.locator('[data-testid="affichage-auto"]');
  await expect(bascule).toHaveAttribute('aria-checked', 'false');
  await bascule.click();
  // OS sombre : la déclinaison nuit du thème choisi (nature) s'affiche ;
  // le choix persisté reste le thème de BASE — le suffixe est un état
  // dérivé, jamais enregistré (A42).
  await page.emulateMedia({ colorScheme: 'dark' });
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'nature-nuit');
  expect(await page.evaluate(() => localStorage.getItem('wind-theme'))).not.toBe('nature-nuit');
  // Le suffixe suit le thème choisi, pas un sombre unique : safran
  // choisi sous OS sombre s'affiche safran-nuit, et safran est persisté.
  await page.locator('[data-testid="reglages-groupe"][data-groupe="themes"]').click();
  await page.locator('[data-theme-id="safran"]').click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'safran-nuit');
  expect(await page.evaluate(() => localStorage.getItem('wind-theme'))).toBe('safran');
  // Un thème -nuit choisi à la main reste en paix : déjà sombre…
  await page.locator('[data-theme-id="estampe-nuit"]').click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'estampe-nuit');
  // …y compris quand l'OS repasse au clair — le choix explicite prime
  // (revue A42 : cette direction-là n'était pas assertée).
  await page.emulateMedia({ colorScheme: 'light' });
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'estampe-nuit');
  await page.emulateMedia({ colorScheme: 'dark' });
  // La coche suit la fiche AFFICHÉE (revue A42) : nature choisi sous
  // OS sombre s'affiche nature-nuit — la coche aussi, sinon le clic de
  // « correction » sur la fiche -nuit enferme dans le sombre permanent.
  await page.locator('[data-theme-id="nature"]').click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'nature-nuit');
  await expect(page.locator('[data-theme-id="nature-nuit"] .coche')).toBeVisible();
  // OS clair : le choix revient tel quel — l'attribut TOMBE (nature),
  // et la coche revient sur la fiche claire. Assertion pleine : pas
  // « autre chose que nature-nuit », l'absence d'attribut (revue A42).
  await page.emulateMedia({ colorScheme: 'light' });
  await expect(page.locator('html')).not.toHaveAttribute('data-theme');
  await expect(page.locator('[data-theme-id="nature"] .coche')).toBeVisible();
  // Persistance : le booléen survit comme le thème.
  expect(await page.evaluate(() => localStorage.getItem('wind-theme-auto'))).toBe('1');
  // Retour au groupe Affichage : la bascule n'existe que sous son
  // groupe — le rail est resté sur Thèmes depuis le choix de safran.
  await page.locator('[data-testid="reglages-groupe"][data-groupe="affichage"]').click();
  await bascule.click();
  await page.emulateMedia({ colorScheme: null });
  await page.locator('[data-testid="reglages-termine"]').click();
});

test("le suivi OS lit l'API Tauri : une vraie bascule Windows suffixe et revient (terrain A42)", async () => {
  // Constat terrain du 2026-08-16 : prefers-color-scheme est MORT dans
  // le WebView2 de Tauri (jamais sombre, zéro événement) — le test D6
  // ci-dessus, joué à emulateMedia, n'exerce que le repli. Ici la
  // bascule est RÉELLE : registre + diffusion WM_SETTINGCHANGE
  // (bascule-sombre.ps1, le geste des Paramètres Windows), et c'est le
  // canal Tauri theme()/onThemeChanged qui doit refléter.
  test.skip(process.platform !== 'win32', 'bascule AppsUseLightTheme — Windows seulement');
  const cle = String.raw`HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Themes\Personalize`;
  const initial = Number(execSync(
    `powershell -NoProfile -c "(Get-ItemProperty '${cle}' -Name AppsUseLightTheme).AppsUseLightTheme"`,
  ).toString().trim());
  const script = path.resolve(import.meta.dirname, '..', 'bascule-sombre.ps1');
  const basculer = (v) => execSync(
    `powershell -NoProfile -ExecutionPolicy Bypass -File "${script}" -v ${v}`,
  );
  try {
    await page.locator('[data-testid="reglages"]').click();
    await page.locator('[data-testid="reglages-groupe"][data-groupe="affichage"]').click();
    const bascule = page.locator('[data-testid="affichage-auto"]');
    await bascule.click();
    await expect(bascule).toHaveAttribute('aria-checked', 'true');
    // OS clair d'abord (l'état de référence), puis sombre : la
    // déclinaison nuit du thème choisi (nature) doit se poser SANS
    // emulateMedia — la livraison de l'événement Tauri prend ~1 s.
    basculer(1);
    await expect(page.locator('html')).not.toHaveAttribute('data-theme', /nuit/, { timeout: 10_000 });
    basculer(0);
    await expect(page.locator('html')).toHaveAttribute('data-theme', 'nature-nuit', { timeout: 10_000 });
    // Et le RETOUR — le sens exact du constat terrain (point 4 KO).
    basculer(1);
    await expect(page.locator('html')).not.toHaveAttribute('data-theme', /nuit/, { timeout: 10_000 });
    await bascule.click();
    await expect(bascule).toHaveAttribute('aria-checked', 'false');
    await page.locator('[data-testid="reglages-termine"]').click();
  } finally {
    // La machine retrouve son réglage, quoi qu'il arrive au test.
    basculer(initial);
  }
});

test("l'ancien choix « La nuit » migre vers nature-nuit au montage (A42)", async () => {
  // Un profil d'avant A42 porte `nuit` : le choix SURVIT au renommage
  // (le motif de la migration Discovery → Wind, PLAN-WIND E3).
  await page.evaluate(() => localStorage.setItem('wind-theme', 'nuit'));
  await page.reload();
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'nature-nuit');
  expect(await page.evaluate(() => localStorage.getItem('wind-theme'))).toBe('nature-nuit');
  // Un thème RETIRÉ (l'air) retombe sur le défaut, silencieusement.
  await page.evaluate(() => localStorage.setItem('wind-theme', 'air'));
  await page.reload();
  await expect(page.locator('[data-testid="ligne"]').first()).toBeVisible();
  await expect(page.locator('html')).not.toHaveAttribute('data-theme');
  // Retour au défaut pour ne pas teinter d'autres parcours.
  await page.evaluate(() => localStorage.removeItem('wind-theme'));
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
  // R4 : transférer PAR message ; le dernier message du fil Vantis porte
  // l'annexe tarifaire (`.last()`).
  await page.locator('[data-testid="transferer"]').last().click();
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
  await expect(page.locator('[data-testid="volet-lecture"] [data-testid="fil-sujet"]')).toContainText(
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

test('le triage clavier avance : e/Suppr sélectionnent la ligne du dessous (A38)', async () => {
  // Joué APRÈS le parcours E3 : sa Corbeille compte « 3 + l'écho » —
  // le Suppr d'ici ajouterait un écho de trop avant l'assertion. Départ
  // d'une source fraîche (aller-retour de nav), et parcours sur des
  // lignes SANS rôle dans la suite (« Atelier de septembre », puis la
  // ligne dessous) : le fil Vantis (transfert PJ-D4) reste intact.
  await page.locator('[data-testid="nav-dossier"][data-categorie="archives"]').click();
  await page.locator('[data-testid="nav-dossier"][data-categorie="reception"]').click();
  const lignes = page.locator('[data-testid="ligne"]');
  await expect(lignes.first()).toBeVisible();
  // La ligne du dessous se capture AVANT le geste — après, elle a
  // glissé d'un rang.
  let sujets = await lignes.locator('.objet').allTextContents();
  const depart = sujets.indexOf('Atelier de septembre');
  expect(depart).toBeGreaterThan(-1);
  const dessous = sujets[depart + 1];
  await lignes.nth(depart).click();
  // Le clic de souris ne laisse PAS le focus sur la rangée : aucune
  // touche ultérieure (raccourci ou non) ne peut allumer l'anneau
  // :focus-visible sur un nœud recyclé par index.
  expect(
    await page.evaluate(() => document.activeElement === document.body),
  ).toBe(true);
  await page.keyboard.press('e');
  await expect(page.locator('[data-testid="toast"]')).toContainText(
    'Conversation archivée.',
  );
  // Le raccourci retire le focus de la rangée cliquée : l'anneau
  // :focus-visible ne surgit jamais sur un nœud recyclé (les rangées
  // sont clées par index — il montrerait une AUTRE conversation).
  expect(
    await page.evaluate(() => document.activeElement === document.body),
  ).toBe(true);
  // La sélection a avancé : la ligne du dessous porte le liseré ET son
  // volet est ouvert (trois volets — comme au clic).
  const choisie = page.locator('[data-testid="ligne"].choisie');
  await expect(choisie).toHaveCount(1);
  await expect(choisie.locator('.objet')).toHaveText(dessous);
  await expect(page.locator('[data-testid="volet-lecture"] [data-testid="fil-sujet"]')).toHaveText(dessous);
  // La liste FRAÎCHE d'abord (stale-while-revalidate : les lignes
  // servies restent affichées un instant) — la ligne archivée partie,
  // la capture de la prochaine ligne dessous est sûre.
  await expect(
    page.locator('[data-testid="ligne"]', { hasText: 'Atelier de septembre' }),
  ).toHaveCount(0);
  sujets = await lignes.locator('.objet').allTextContents();
  const suivante = sujets[sujets.indexOf(dessous) + 1];
  // Le geste s'enchaîne sans reprendre la souris : Suppr agit sur la
  // sélection avancée, et avance encore.
  await page.keyboard.press('Delete');
  await expect(page.locator('[data-testid="toast"]')).toContainText(
    'Conversation supprimée.',
  );
  await expect(choisie.locator('.objet')).toHaveText(suivante);
  await expect(page.locator('[data-testid="volet-lecture"] [data-testid="fil-sujet"]')).toHaveText(suivante);
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
  // Le vidage et le geste, pendant le vol. (`fill('')` ne vide pas un
  // contenteditable : Ctrl+A + Suppr, comme l'utilisateur.)
  await page.locator('[data-testid="composition-objet"]').fill('');
  await page.locator('[data-testid="composition-corps"]').click();
  await page.keyboard.press('Control+a');
  await page.keyboard.press('Delete');
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

// ——— Revue v3 : exclusivité des cadres (joué en fin de chaîne — il archive) ———

test("archiver au raccourci depuis l'écran 03 ferme le cadre — jamais de plein écran fantôme (revue v3)", async () => {
  // Revue v3 : trois booléens réconciliés à la main laissaient
  // `visible` armé quand `e` archivait depuis le plein écran — le
  // prochain clic de liste rouvrait l'écran 03 non demandé, avec DEUX
  // Fil montés. Depuis, l'exclusivité vit au store (fil.cadre).
  await page.locator('[data-testid="ligne"]').first().click();
  await page.locator('[data-testid="voir-conversation"]').click();
  await expect(page.locator('[data-testid="conversation"]')).toBeVisible();
  await page.keyboard.press('e');
  await expect(page.locator('[data-testid="toast"]')).toContainText('Conversation archivée.');
  // Le cadre plein écran est tombé avec le fil.
  await expect(page.locator('[data-testid="conversation"]')).toHaveCount(0);
  // Le clic suivant ouvre le VOLET, jamais l'écran 03 ressuscité —
  // et l'objet reste unique.
  await page.locator('[data-testid="ligne"]').first().click();
  await expect(page.locator('[data-testid="conversation"]')).toHaveCount(0);
  await expect(page.locator('[data-testid="fil-sujet"]')).toHaveCount(1);
  await expect(page.locator('[data-testid="volet-lecture"] [data-testid="fil-sujet"]')).not.toBeEmpty();
  // Le triage clavier (A38) est VIVANT après coup : e avance encore.
  const objet = await page.locator('[data-testid="volet-lecture"] [data-testid="fil-sujet"]').innerText();
  await page.keyboard.press('e');
  await expect(page.locator('[data-testid="toast"]')).toContainText('Conversation archivée.');
  await expect(page.locator('[data-testid="volet-lecture"] [data-testid="fil-sujet"]')).not.toHaveText(objet);
});
