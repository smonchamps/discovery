// Liens du corps d'un message (constat terrain 2026-08-15) : le clic
// dans l'iframe sandbox naviguait le CADRE vers le site — refusé par
// les sites (X-Frame-Options / frame-ancestors) et par la CSP, WebView2
// remplaçait le corps par sa page « Ce contenu a été bloqué ».
//
// Le remède : le jeton `allow-same-origin` (toujours SANS allow-scripts
// — le contenu reste inerte, l'invariant S1 tient) rend le document de
// l'iframe accessible au parent ; on intercepte le clic ICI, on
// l'annule TOUJOURS (l'iframe ne navigue jamais, même pour un lien
// refusé), et le navigateur SYSTÈME reçoit le lien via `open_link` —
// dont la garde Rust revalide le schéma, ce filtre-ci n'est que du
// confort.
import { appel } from './transport.js';

const SCHEMAS = new Set(['http:', 'https:', 'mailto:']);

// Couture e2e (même patron que __e2ePieces) : un tableau posé dans
// `window.__e2eLiens` capte les URL au lieu d'ouvrir un navigateur
// réel — tout le chemin amont (iframe, interception, filtre) est le
// vrai. Hors e2e la variable n'existe pas.
function ouvrir(url) {
  const captes = globalThis.window?.__e2eLiens;
  if (captes !== undefined) {
    captes.push(url);
    return;
  }
  appel('open_link', { url }).catch((err) => console.error('open_link :', err));
}

// À brancher au `onload` de l'iframe : chaque affectation de srcdoc
// charge un document NEUF, l'écouteur se repose à chaque chargement.
// En capture : aucun contenu ne peut se glisser avant lui.
export function brancherLiens(iframe) {
  const doc = iframe?.contentDocument;
  if (!doc) return;
  doc.addEventListener(
    'click',
    (ev) => {
      // Pas de `instanceof Element` : la cible vit dans le royaume de
      // l'iframe, pas celui du parent.
      const ancre = ev.target?.closest?.('a[href]');
      if (!ancre) return;
      ev.preventDefault();
      let lien;
      try {
        // L'attribut brut, jamais la propriété résolue : un href
        // relatif ne pointe nulle part dans un mail — ignoré.
        lien = new URL(ancre.getAttribute('href'));
      } catch {
        return;
      }
      if (SCHEMAS.has(lien.protocol)) ouvrir(lien.href);
    },
    true,
  );
}
