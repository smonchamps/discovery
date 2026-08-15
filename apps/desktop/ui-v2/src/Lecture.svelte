<script>
  // Volet de lecture de l'écran 02 — VERBATIM du prototype : carte
  // signature, titre 24 px tronqué, puce méta + « Dernier message · … »
  // + « Voir la conversation », auteur, corps, barre des 4 actions.
  //
  // Invariant intact : le corps vit dans l'iframe sandbox, servie par
  // `message_body` (assaini côté coeur, images distantes bloquées, encre
  // bakée par thème — S1), jamais innerHTML. Le jeton `allow-same-origin`
  // — SANS allow-scripts, le contenu reste inerte — sert l'interception
  // des liens (lib/liens.js, terrain 2026-08-15) : le clic part au
  // navigateur système, l'iframe ne navigue jamais.
  //
  import { appel } from './lib/transport.js';
  import { brancherLiens } from './lib/liens.js';
  import { quandLong } from './lib/quand.js';
  import { activation } from './lib/clavier.js';
  import { t } from './lib/texte.svelte.js';

  let {
    onarchiver = () => {},
    onsupprimer = () => {},
    onconversation = () => {},
    onrepondre = () => {},
    onrepondretous = () => {},
    ontransferer = () => {},
    onflash = () => {},
  } = $props();

  let ligne = $state(null);
  let corps = $state('');
  // Fichiers joints du message affiché — verdict terrain (Annexe A) :
  // un message SEUL n'a pas de conversation à ouvrir, ses fichiers
  // doivent se prendre ICI. Puces cliquables -> Téléchargements, comme
  // dans la conversation (ADR 0007 : octets à la demande, jamais de
  // cache).
  let pieces = $state([]);
  // Le compte de pièces FRAIS, rendu par message_body d'après-scan : la
  // ligne de liste porte celui d'AVANT l'ouverture — un message reçu à
  // l'instant vient d'écrire ses pièces en base PENDANT l'ouverture, et
  // s'en tenir à la ligne ouvrait ses fichiers sur une rangée vide
  // (terrain CE, 2026-08-14).
  let nbPieces = $state(0);
  let enregistrements = $state({});
  // Images distantes : bloquées par DÉFAUT (invariant), comptées par le
  // coeur ; l'opt-in est PAR MESSAGE et ne survit pas à la sélection.
  let imagesBloquees = $state(0);
  let imagesVoulues = false;
  let derniereOuvertureMs = $state(null);
  // Jeton d'ouverture, PAS une comparaison d'objet : `$state` enveloppe
  // les objets dans un proxy, `ligne !== nouvelle` après un await est
  // donc toujours vrai — le corps ne se posait jamais.
  let jeton = 0;

  const meta = $derived.by(() => {
    if (!ligne) return '';
    const messages = t('puce.messages', { n: ligne.thread_size > 1 ? ligne.thread_size : 1 });
    return nbPieces > 0 ? `${messages} · ${t('puce.fichiers', { n: nbPieces })}` : messages;
  });

  // E3 (PLAN-REACTIVITE) : un écho local — la copie de destination d'un
  // geste, en attente de sa vraie ligne — se reconnaît à sa boîte
  // synthétique. Son corps est LOCAL (echo_body) ; ses pièces n'ont pas
  // de métadonnées par pièce (fenêtre de quelques secondes), la puce
  // méta dit le compte, les puces cliquables attendent la vraie ligne.
  const estEcho = (l) => typeof l?.mailbox === 'string' && l.mailbox.startsWith('echo:');

  export async function ouvrir(nouvelle) {
    const t0 = performance.now();
    imagesVoulues = false;
    pieces = [];
    const duree = await servir(nouvelle, false, t0);
    // Hors du chemin d'ouverture mesuré : les métadonnées de pièces
    // arrivent après le corps, jamais avant. Le compte est celui
    // d'après-scan (servir), jamais celui de la ligne.
    if (nbPieces > 0 && !estEcho(nouvelle)) {
      const mien = jeton;
      appel('message_attachments', {
        accountId: nouvelle.account_id,
        mailbox: nouvelle.mailbox,
        uid: nouvelle.uid,
      })
        .then((lues) => {
          if (mien === jeton) pieces = lues;
        })
        .catch((err) => console.error('message_attachments :', err));
    }
    return duree;
  }

  async function enregistrer(piece) {
    if (!ligne || enregistrements[piece.index]) return;
    enregistrements[piece.index] = true;
    try {
      const chemin = await appel('save_attachment', {
        accountId: ligne.account_id,
        mailbox: ligne.mailbox,
        uid: ligne.uid,
        index: piece.index,
      });
      onflash(t('toast.pieceEnregistree', { chemin }));
    } catch (err) {
      onflash(t('erreur.enregistrement', { err }));
    } finally {
      enregistrements[piece.index] = false;
    }
  }

  async function servir(nouvelle, avecImages, t0 = null) {
    const mien = ++jeton;
    ligne = nouvelle;
    try {
      const vue = estEcho(nouvelle)
        ? await appel('echo_body', {
            id: Number(nouvelle.mailbox.slice(5)),
            showImages: avecImages,
          })
        : await appel('message_body', {
            accountId: nouvelle.account_id,
            mailbox: nouvelle.mailbox,
            uid: nouvelle.uid,
            showImages: avecImages,
          });
      if (mien !== jeton) return derniereOuvertureMs; // sélection changée
      corps = vue.document;
      imagesBloquees = avecImages ? 0 : vue.remote_images_blocked;
      nbPieces = vue.attachment_count;
    } catch (err) {
      corps = '';
      imagesBloquees = 0;
      nbPieces = 0;
      console.error('message_body :', err);
    }
    if (t0 !== null) derniereOuvertureMs = performance.now() - t0;
    return derniereOuvertureMs;
  }

  function afficherImages() {
    if (!ligne || imagesVoulues) return;
    imagesVoulues = true;
    servir(ligne, true);
  }

  export function fermer() {
    jeton += 1;
    ligne = null;
    corps = '';
    imagesBloquees = 0;
    pieces = [];
    nbPieces = 0;
  }
  export function etat() {
    return { derniereOuvertureMs };
  }
</script>

<main aria-label={t('lecture.aria')} data-testid="volet-lecture">
  {#if !ligne}
    <p class="vide">{t('lecture.vide')}</p>
  {:else}
    <div class="carte">
      <div class="entete">
        <h3 class="titre" data-testid="lecture-sujet">{ligne.subject}</h3>
        <div class="metas">
          <span class="puce">{meta}</span>
          <span class="dernier">{t('lecture.dernier', { quand: quandLong(ligne.epoch) })}</span>
          <span class="puce" class:cliquable={ligne.thread_id != null}
                data-testid="voir-conversation"
                role="button" tabindex={ligne.thread_id != null ? 0 : -1}
                aria-disabled={ligne.thread_id == null}
                onclick={() => ligne.thread_id != null && onconversation(ligne)}
                onkeydown={activation(() => ligne.thread_id != null && onconversation(ligne))}>
            <span class="ms" aria-hidden="true">unfold_more</span>{t('lecture.voirConversation')}</span>
        </div>
      </div>
      <div class="auteur">
        <span class="nom">{ligne.sender}</span>
        <span class="adresse">{t('lecture.a', { adresse: ligne.account_email })}</span>
      </div>
      {#if imagesBloquees > 0}
        <div class="garde-images" data-testid="garde-images">
          <span class="ms" aria-hidden="true">visibility_off</span>
          <span class="garde-texte">{t('lecture.imagesBloquees', { n: imagesBloquees })}</span>
          <button type="button" data-testid="afficher-images" onclick={afficherImages}>
            {t('lecture.afficherImages')}</button>
        </div>
      {/if}
      <iframe class="corps" sandbox="allow-same-origin" srcdoc={corps}
              title={t('lecture.corps')}
              onload={(ev) => brancherLiens(ev.currentTarget)}></iframe>
      {#if pieces.length > 0}
        <div class="fichiers" data-testid="lecture-fichiers">
          {#each pieces as piece (piece.index)}
            <button type="button" class="puce cliquable" data-testid="piece-jointe"
                    disabled={enregistrements[piece.index]}
                    onclick={() => enregistrer(piece)}
                    title={t('lecture.enregistrer')}>
              <span class="ms" aria-hidden="true">description</span>{piece.name}</button>
            <span class="puce">{piece.size}</span>
          {/each}
        </div>
      {/if}
      <div class="actions">
        <button type="button" class="principal" data-testid="repondre" onclick={() => onrepondre(ligne)}>
          <span class="ms" aria-hidden="true">reply</span>{t('action.repondre')}</button>
        <button type="button" data-testid="repondre-tous" onclick={() => onrepondretous(ligne)}>
          <span class="ms" aria-hidden="true">reply_all</span>{t('action.repondreTous')}</button>
        <button type="button" data-testid="transferer" onclick={() => ontransferer(ligne)}>
          <span class="ms miroir" aria-hidden="true">reply</span>{t('action.transferer')}</button>
        <button type="button" data-testid="archiver" onclick={() => onarchiver(ligne)}>
          <span class="ms" aria-hidden="true">archive</span>{t('action.archiver')}</button>
        <button type="button" data-testid="supprimer" onclick={() => onsupprimer(ligne)}>
          <span class="ms" aria-hidden="true">delete</span>{t('action.supprimer')}</button>
      </div>
    </div>
  {/if}
</main>

<style>
  main {
    background:var(--bg); padding:12px 20px 20px; min-width:0;
    display:flex; flex-direction:column; min-height:0;
  }
  .vide {
    margin:auto; font-size:13px; line-height:1.5; color:var(--muted);
    text-align:center; padding:40px;
  }
  .carte {
    flex:1; background:var(--surface); border:1px solid var(--border); border-radius:10px;
    box-shadow:var(--shadow); display:flex; flex-direction:column;
    min-height:0; overflow:hidden;
  }
  .entete {
    padding:26px 30px 22px; border-bottom:1px solid var(--border);
    display:flex; flex-direction:column; gap:14px;
  }
  .titre {
    margin:0; font-size:24px; font-weight:600; line-height:1.2;
    letter-spacing:-.01em; color:var(--ink);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
  }
  .metas { display:flex; align-items:center; gap:10px; flex-wrap:wrap; }
  .puce {
    height:32px; padding:0 12px; display:inline-flex; align-items:center;
    gap:8px; font-size:13px; color:var(--ink2); background:var(--surface);
    border:1px solid var(--border); border-radius:6px; white-space:nowrap;
  }
  .dernier { font-size:12px; color:var(--muted); }
  .cliquable { cursor:pointer; }
  .cliquable:hover { background:var(--sel); }
  .auteur {
    padding:26px 30px 0; display:flex; flex-direction:column; gap:4px;
  }
  .nom { font-size:15px; font-weight:600; color:var(--ink); }
  .adresse { font-size:13px; color:var(--muted); }
  .garde-images {
    margin:18px 30px 0; padding:10px 14px; display:flex;
    align-items:center; gap:10px; font-size:13px; color:var(--ink2);
    background:var(--panel); border:1px solid var(--border);
    border-radius:6px;
  }
  .garde-images .ms { color:var(--muted); }
  .garde-texte { flex:1; }
  .fichiers {
    margin:14px 30px 0; display:flex; gap:10px; flex-wrap:wrap;
    flex:none;
  }
  .corps {
    /* 18 px et non 30 : le document assaini porte sa propre gouttière
       de 12 px (mail-render, body margin) — 18 + 12 = 30, le texte du
       corps s'aligne sur l'objet et l'auteur (verdict terrain). */
    flex:1; border:none; background:#ffffff; margin:18px 18px 0;
    min-height:0;
  }
  .actions {
    padding:18px 30px; border-top:1px solid var(--border);
    display:flex; gap:12px; margin-top:18px;
  }
  button {
    height:32px; padding:0 16px; display:inline-flex; align-items:center;
    gap:8px; font-size:13px; color:var(--ink); background:var(--surface);
    border:1px solid var(--border); border-radius:6px; cursor:pointer;
  }
  button:hover { background:var(--sel); }
  .principal {
    font-weight:600; color:var(--onAccent); background:var(--accent);
    border-color:var(--accent);
  }
  .principal:hover { background:var(--accentH); border-color:var(--accentH); }
</style>
