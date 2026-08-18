// L'état du FIL ouvert — UN objet, DEUX cadres (UI v3, décision D4 du
// 2026-08-16 : « une coexistence qui n'est qu'un changement de taille
// des mêmes objets »). Le volet de lecture et l'écran 03 montent tous
// deux Fil.svelte sur CET état ; `cadre` dit lequel tient l'objet —
// c'est le SEUL interrupteur, l'exclusivité est structurelle (revue
// v3 : trois booléens réconciliés à la main se désynchronisaient au
// premier chemin oublié — archiver au clavier, bascule de disposition).
//
// Invariant S1 intact : chaque corps vient de message_body/echo_body
// (assaini côté cœur), chargé au dépliage seulement, affiché dans une
// iframe sandbox par message — jamais innerHTML.
import { appel } from './transport.js';

const VIDE = () => ({
  messages: [],
  deplies: {},
  corps: {},
  pieces: {},
  // Le compte de pièces d'APRÈS-SCAN par message (vue.attachment_count
  // de message_body) : la ligne de liste porte celui d'AVANT
  // l'ouverture — s'y fier ouvrait les pièces fraîchement reçues sur
  // une rangée vide (terrain CE, 2026-08-14 ; régression attrapée à la
  // revue v3).
  nbPieces: {},
  imagesBloquees: {},
  imagesVoulues: {},
});

export const fil = $state({
  // Quel cadre tient l'objet : null (aucun), 'volet', 'plein'.
  cadre: null,
  // La ligne d'origine (sélection de liste) et l'état par message.
  ligne: null,
  ...VIDE(),
  // Le chrono d'ouverture (banc P1, e2e) : sélection → corps du
  // dernier message posé. Les pièces restent HORS chrono, comme au
  // volet d'avant v3.
  derniereOuvertureMs: null,
});

let jeton = 0;

export const cleMsg = (m) => `${m.account_id}/${m.mailbox}/${m.uid}`;

// Un écho local (PLAN-REACTIVITE E3) se reconnaît à sa boîte
// synthétique — son corps est local (echo_body), jamais de fil.
export const estEcho = (m) =>
  typeof m?.mailbox === 'string' && m.mailbox.startsWith('echo:');

// Ouvre le fil de `nouvelle` dans `cadre` — TOUJOURS rechargé : la
// mémoïsation de la première v3 rendait un fil périmé (sa propre
// réponse absente après un envoi) et figeait un échec de chargement.
// L'agrandissement ne passe PAS ici : c'est `agrandirFil()`, zéro
// rechargement — le cadre change, pas l'objet (D4).
export async function ouvrirFil(nouvelle, cadre = 'volet') {
  const t0 = performance.now();
  const mien = ++jeton;
  fil.cadre = cadre;
  fil.ligne = nouvelle;
  Object.assign(fil, VIDE());
  // V-D2 : sans fil — écho compris — le MESSAGE SEUL est le fil.
  if (nouvelle.thread_id == null) {
    fil.messages = [nouvelle];
    await basculerMessage(nouvelle, true);
    if (mien === jeton) fil.derniereOuvertureMs = performance.now() - t0;
    return fil.derniereOuvertureMs;
  }
  try {
    const messages = await appel('thread_messages', { threadId: nouvelle.thread_id });
    if (mien !== jeton) return fil.derniereOuvertureMs;
    fil.messages = messages;
    const dernier = messages[messages.length - 1];
    if (dernier) await basculerMessage(dernier, true);
  } catch (err) {
    console.error('thread_messages :', err);
  }
  if (mien === jeton) fil.derniereOuvertureMs = performance.now() - t0;
  return fil.derniereOuvertureMs;
}

// Le changement de taille (D4) : aucun rechargement, aucun jeton.
export function agrandirFil() {
  if (fil.ligne) fil.cadre = 'plein';
}
// Le retour : au volet si le mode en a un, sinon fermeture.
export function reduireFil(versVolet) {
  if (versVolet && fil.ligne) fil.cadre = 'volet';
  else fermerFil();
}

export function fermerFil() {
  jeton += 1;
  fil.cadre = null;
  fil.ligne = null;
  Object.assign(fil, VIDE());
  fil.derniereOuvertureMs = null;
}

async function chargerMessage(m, avecImages = false) {
  const k = cleMsg(m);
  const mien = jeton;
  if (fil.corps[k] === undefined || avecImages) {
    if (fil.corps[k] === undefined) fil.corps[k] = '';
    try {
      const vue = estEcho(m)
        ? await appel('echo_body', {
            id: Number(m.mailbox.slice(5)),
            showImages: avecImages,
          })
        : await appel('message_body', {
            accountId: m.account_id,
            mailbox: m.mailbox,
            uid: m.uid,
            showImages: avecImages,
          });
      // Le jeton d'ouverture garde chaque écriture : une réponse
      // tardive (images accordées puis sélection changée) n'écrase
      // jamais l'état d'un fil plus récent — l'opt-in d'images ne
      // survit pas à la sélection (invariant, revenu à la revue v3).
      if (mien !== jeton) return;
      fil.corps[k] = vue.document;
      fil.imagesBloquees[k] = avecImages ? 0 : vue.remote_images_blocked;
      fil.nbPieces[k] = vue.attachment_count;
    } catch (err) {
      console.error('message_body :', err);
    }
  }
  // Les métadonnées de pièces : HORS du chemin mesuré (elles arrivent
  // après le corps, jamais avant), gatées sur le compte d'après-scan.
  // Les pièces d'un écho n'ont pas de métadonnées par pièce pendant la
  // fenêtre de réconciliation.
  const nb = fil.nbPieces[k] ?? m.attachment_count;
  if (nb > 0 && !estEcho(m) && fil.pieces[k] === undefined) {
    fil.pieces[k] = [];
    appel('message_attachments', {
      accountId: m.account_id,
      mailbox: m.mailbox,
      uid: m.uid,
    })
      .then((lues) => {
        if (mien === jeton) fil.pieces[k] = lues;
      })
      .catch((err) => console.error('message_attachments :', err));
  }
}

export function basculerMessage(m, valeur = null) {
  const k = cleMsg(m);
  const nouveau = valeur ?? !fil.deplies[k];
  fil.deplies[k] = nouveau;
  return nouveau ? chargerMessage(m) : Promise.resolve();
}

export function toutDeplier() {
  for (const m of fil.messages) basculerMessage(m, true);
}

// Le geste inverse (terrain A46) : TOUT se replie, le dernier compris.
// La bascule « Tout déplier »/« Tout replier » n'est PAS un drapeau :
// elle se DÉRIVE de l'état réel des dépliages (terrain A47 — un fil
// d'un message s'ouvre déplié, le bouton dit « Tout replier » ; les
// dépliages manuels la font suivre).
export function toutReplier() {
  for (const m of fil.messages) basculerMessage(m, false);
}

// Images distantes : bloquées par DÉFAUT (invariant), opt-in PAR
// MESSAGE — l'accord ne survit pas à la fermeture du fil.
export function afficherImages(m) {
  const k = cleMsg(m);
  if (fil.imagesVoulues[k]) return;
  fil.imagesVoulues[k] = true;
  chargerMessage(m, true);
}
