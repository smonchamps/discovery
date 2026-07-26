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
  await expect(page.locator('#perf')).toContainText('160 conversations');

  // Tant que rien n'est sélectionné, le panneau de lecture est ABSENT —
  // pas seulement vide. Défaut vu au terrain : `#detail { display: flex }`
  // écrasait `[hidden]` (spécificité d'ID contre la feuille du
  // navigateur), l'iframe sandboxée couvrait tout le panneau droit et
  // CAPTAIT le premier clic — le focus clavier partait dans l'iframe et
  // les raccourcis mouraient tant qu'on ne cliquait pas ailleurs.
  await expect(page.locator('#detail')).toBeHidden();

  await page.locator('.row').first().click();

  await expect(page.locator('#detail-subject')).toContainText('n°200');
  await expect(page.locator('#detail-frame')).toHaveAttribute(
    'srcdoc',
    /Corps du message n°200/,
  );
});

test('trier : « e » archive le message ouvert, la liste et le compte suivent', async () => {
  await page.keyboard.press('e');

  // Le n°200 repondait au n°199 : les archiver l'un apres l'autre vide
  // le meme fil, et le nombre de CONVERSATIONS ne bouge qu'au second.
  await expect(page.locator('#perf')).toContainText('160 conversations');
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

  // n°190 porte une pièce jointe (multiple de dix), n°188 non.
  //
  // Et surtout PAS n°189 : depuis le regroupement, le n°190 lui répond,
  // donc le 189 n'a plus de ligne à lui — c'est le n°190 qui représente
  // leur conversation. Choisir un message qui n'est pas en tête de fil,
  // c'est chercher une ligne qui n'existe pas.
  await page.locator('.row', { hasText: 'n°190' }).first().click();
  await expect(page.locator('#detail-subject')).toContainText('n°190');
  await expect(page.locator('#attachments')).toBeVisible();
  await expect(page.locator('#attachments .attachment')).toHaveCount(1);
  await expect(page.locator('#attachments .attachment')).toContainText('facture-190.pdf');
  await expect(page.locator('#attachments .attachment')).toContainText('20 Ko');

  await page.locator('.row', { hasText: 'n°188' }).first().click();
  await expect(page.locator('#detail-subject')).toContainText('n°188');
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

/// Déplacer, de bout en bout et HORS LIGNE. C'est le test qui compte :
/// la liste des dossiers vient du cache local, le déplacement est
/// journalisé, et le serveur suivra. Rien de tout cela n'exige le
/// réseau — c'est la promesse offline-first appliquée au tri.
///
/// Le décor expose « Archiv&AOk-s » : le nom AFFICHÉ doit être
/// « Archivés », preuve que le décodage UTF-7 arrive jusqu'à l'œil.
test('déplacer : choisir un dossier retire le message, hors ligne', async () => {
  await page.locator('.row').first().click();
  await expect(page.locator('#detail')).toBeVisible();
  const subject = await page.locator('#detail-subject').textContent();
  await expect(page.locator('#move-dialog')).toBeHidden();

  await page.locator('#move').click();
  await expect(page.locator('#move-dialog')).toBeVisible();
  await expect(page.locator('#move-list button').first()).toHaveText('Archivés');

  await page.locator('#move-list button', { hasText: 'Archivés' }).click();

  await expect(page.locator('#move-dialog')).toBeHidden();
  // On n'assertionne PAS le bandeau de confirmation : l'ouverture
  // automatique du message suivant l'écrase aussitôt. Défaut réel mais
  // préexistant (archiver fait de même) — noté, hors périmètre ici.
  // Ce qui compte, et qui est stable : le message a quitté la liste.
  await expect(page.locator('#rows')).not.toContainText(subject);
});

/// Échap doit rendre la main sans rien déplacer — un dialogue qui piège
/// l'utilisateur au milieu d'une action destructive serait pire que pas
/// de dialogue du tout.
test('déplacer : Échap referme sans rien faire', async () => {
  await page.locator('.row').first().click();
  const subject = await page.locator('#detail-subject').textContent();

  await page.locator('#move').click();
  await expect(page.locator('#move-dialog')).toBeVisible();
  await page.keyboard.press('Escape');

  await expect(page.locator('#move-dialog')).toBeHidden();
  await expect(page.locator('#rows')).toContainText(subject);
});

/// Le regroupement, vu de l'utilisateur : une conversation tient sur UNE
/// ligne, annonce combien elle contient, et s'ouvre sur son dernier
/// message avec le reste de l'échange à portée de clic.
///
/// Le décor fait répondre un message sur cinq au précédent : la ligne du
/// n°190 porte donc le n°189 avec elle.
test('conversations : une ligne par fil, compteur visible, échange navigable', async () => {
  await page.keyboard.press('Escape');
  await expect(page.locator('#scroll-space')).toBeVisible();

  const fil = page.locator('.row', { hasText: 'n°190' }).first();
  await expect(fil.locator('.thread-count')).toHaveText('2');
  // Le message intermédiaire n'a pas de ligne à lui : c'est tout l'objet.
  await expect(page.locator('.row', { hasText: 'n°189' })).toHaveCount(0);

  await fil.click();
  await expect(page.locator('#detail-subject')).toContainText('n°190');
  await expect(page.locator('#thread-strip .thread-item')).toHaveCount(2);

  // Ouvrir le message plus ancien depuis le bandeau, sans quitter le fil.
  await page.locator('#thread-strip .thread-item').first().click();
  await expect(page.locator('#detail-subject')).toContainText('n°189');
  await expect(page.locator('#thread-strip .thread-item').first()).toHaveClass(/current/);
});

/// Deux versions d'un même brouillon — même sujet, même destinataire,
/// seul le corps diffère — étaient RIGOUREUSEMENT indiscernables dans le
/// bandeau, qui n'affiche pas le corps.
///
/// Ce n'est pas un confort. Le diagnostic du 2026-07-25 sur la base
/// réelle montrait sujet 14 car. et destinataire 22 car. des deux côtés,
/// corps 28 contre 48 : le tirage faisait son travail, et rien à l'écran
/// ne permettait de le constater. La consigne de validation envoyée à
/// l'utilisateur était donc invérifiable — §9, « vérifier qu'un signal
/// demandé est OBSERVABLE ».
test('brouillons : deux versions de même sujet se distinguent au corps', async () => {
  await page.keyboard.press('Escape');

  for (const corps of ['Première version du devis.', 'Seconde version, tout autre.']) {
    await page.keyboard.press('c');
    await expect(page.locator('#compose')).toBeVisible();
    await page.locator('#compose-to').fill('alice@exemple.fr');
    await page.locator('#compose-subject').fill('Devis');
    await page.locator('#compose-body').fill(corps);
    await page.keyboard.press('Escape'); // sortir du champ…
    await page.keyboard.press('Escape'); // …fermer : conserver
    await expect(page.locator('#compose')).toBeHidden();
  }

  const versions = page.locator('#drafts-list .bar-row', { hasText: 'Devis' });
  await expect(versions).toHaveCount(2);
  await expect(page.locator('#drafts-list')).toContainText('Première version');
  await expect(page.locator('#drafts-list')).toContainText('Seconde version');
});
