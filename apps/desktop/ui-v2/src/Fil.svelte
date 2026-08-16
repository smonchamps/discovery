<script>
  // Le FIL en cartes — l'objet unique de lecture (UI v3, verdict CE du
  // 2026-08-16, décision D4) : titre, puces méta, « Tout déplier »,
  // messages repliés une ligne / dépliés en carte, fichiers joints,
  // brouillon du fil, barre des cinq gestes directs (D5 — jamais de
  // menu « Plus », exception b des annotations). Le volet de lecture
  // et l'écran 03 le montent TEL QUEL : deux cadres, un objet — l'état
  // vit dans lib/fil.svelte.js et survit au changement de cadre.
  //
  // Invariant S1 : une iframe sandbox `allow-same-origin` SANS
  // allow-scripts par message déplié, corps servi par le cœur,
  // liens interceptés (lib/liens.js).
  import {
    fil, cleMsg, basculerMessage, toutDeplier, afficherImages,
  } from './lib/fil.svelte.js';
  import { appel } from './lib/transport.js';
  import { brancherLiens } from './lib/liens.js';
  import { quand } from './lib/quand.js';
  import { activation } from './lib/clavier.js';
  import { t } from './lib/texte.svelte.js';

  let {
    brouillons = [],
    onreprendre = () => {},
    onarchiver = () => {},
    onsupprimer = () => {},
    onrepondre = () => {},
    onrepondretous = () => {},
    ontransferer = () => {},
    onflash = () => {},
    // Le cadre volet passe le geste d'agrandissement ; l'écran 03, non.
    onagrandir = null,
  } = $props();

  // Transférer vise le DERNIER message du fil (son contenu) ;
  // Répondre / Répondre à tous visent le dernier message d'AUTRUI —
  // thread_messages joint les Envoyés, et répondre à sa propre copie
  // composait vers soi-même (revue v3), divergent du raccourci r.
  const dernier = () => fil.messages[fil.messages.length - 1] ?? fil.ligne;
  // Le compte de pièces FRAIS (après-scan, terrain CE 2026-08-14) : la
  // ligne porte celui d'AVANT l'ouverture — le store retient celui de
  // message_body dès que le corps est servi.
  const nbPiecesDe = (m) => fil.nbPieces[cleMsg(m)] ?? m.attachment_count;
  const cible = () =>
    [...fil.messages].reverse().find((m) => !propre(m)) ?? dernier();

  // Le brouillon du fil ouvert — le plus récent (B-D5).
  const brouillonDuFil = $derived.by(() => {
    if (!fil.ligne || fil.ligne.thread_id == null) return null;
    let retenu = null;
    for (const b of brouillons) {
      if (b.thread_id !== fil.ligne.thread_id) continue;
      if (!retenu || b.updated_epoch > retenu.updated_epoch) retenu = b;
    }
    return retenu;
  });

  // « À » n'est pas stocké par le cœur : la règle du prototype —
  // message de soi → premier autre correspondant, sinon le compte.
  const propre = (m) => fil.ligne && m.sender_address === fil.ligne.account_email;
  const ligneDe = (m) =>
    m.sender_address ? `${m.sender} <${m.sender_address}>` : m.sender;
  function ligneA(m) {
    if (!propre(m)) return fil.ligne?.account_email ?? '';
    const autre = fil.messages.find(
      (x) => x.sender_address && x.sender_address !== m.sender_address,
    );
    return autre ? `${autre.sender} <${autre.sender_address}>` : (fil.ligne?.account_email ?? '');
  }

  // En-vol transitoire : local au composant, rien à partager entre
  // cadres (revue v3 — le store ne porte que l'état du fil).
  let enregistrements = $state({});
  async function enregistrer(m, piece) {
    const k = `${cleMsg(m)}#${piece.index}`;
    if (enregistrements[k]) return;
    enregistrements[k] = true;
    try {
      const chemin = await appel('save_attachment', {
        accountId: m.account_id,
        mailbox: m.mailbox,
        uid: m.uid,
        index: piece.index,
      });
      onflash(t('toast.pieceEnregistree', { chemin }));
    } catch (err) {
      onflash(t('erreur.enregistrement', { err }));
    } finally {
      enregistrements[k] = false;
    }
  }
</script>

{#if fil.ligne}
  <div class="objet-fil">
    <div class="tete">
      <h3 class="titre" data-testid="fil-sujet">{fil.ligne.subject}</h3>
      <div class="puces">
        {#if fil.ligne.thread_size > 1}
          <span class="puce"><span class="ms" aria-hidden="true">forum</span>{t('puce.messages', { n: fil.ligne.thread_size })}</span>
        {/if}
        {#if nbPiecesDe(fil.ligne) > 0}
          <span class="puce"><span class="ms" aria-hidden="true">attach_file</span>{t('puce.fichiers', { n: nbPiecesDe(fil.ligne) })}</span>
        {/if}
        {#if onagrandir}
          <!-- V-D2 : un message SEUL n'a pas de conversation à ouvrir
               — la puce reste, inerte et dite telle (revue v3 :
               aria-disabled/tabindex avaient sauté à l'extraction). -->
          <span class="puce bouton" data-testid="voir-conversation"
                class:inerte={fil.ligne.thread_id == null}
                role="button" tabindex={fil.ligne.thread_id != null ? 0 : -1}
                aria-disabled={fil.ligne.thread_id == null}
                onclick={() => fil.ligne.thread_id != null && onagrandir(fil.ligne)}
                onkeydown={activation(() => fil.ligne.thread_id != null && onagrandir(fil.ligne))}>
            <span class="ms" aria-hidden="true">unfold_more</span>{t('lecture.voirConversation')}</span>
        {/if}
        <span class="puce bouton" data-testid="tout-deplier"
              role="button" tabindex="0"
              onclick={toutDeplier} onkeydown={activation(toutDeplier)}>
          <span class="ms" aria-hidden="true">unfold_more</span>{t('conv.toutDeplier')}</span>
      </div>
    </div>

    <div class="fil">
      {#each fil.messages as m (cleMsg(m))}
        {@const k = cleMsg(m)}
        {#if fil.deplies[k]}
          <article class="deplie" data-testid="message-deplie">
            <div class="tete-message" role="button" tabindex="0"
                 aria-expanded="true"
                 onclick={() => basculerMessage(m)} onkeydown={activation(() => basculerMessage(m))}>
              <span class="auteur">{m.sender}</span>
              {#if nbPiecesDe(m) > 0}
                <span class="puce"><span class="ms" aria-hidden="true">attach_file</span>{t('puce.fichiers', { n: nbPiecesDe(m) })}</span>
              {/if}
              <span class="heure">{quand(m.epoch)}</span>
            </div>
            <div class="contenu">
              <dl class="adresses">
                <dt>{t('conv.de')}</dt><dd>{ligneDe(m)}</dd>
                <dt>{t('conv.a')}</dt><dd>{ligneA(m)}</dd>
                <dt>{t('conv.objet')}</dt><dd>{m.subject}</dd>
              </dl>
              {#if (fil.imagesBloquees[k] ?? 0) > 0}
                <div class="garde-images" data-testid="garde-images">
                  <span class="ms" aria-hidden="true">visibility_off</span>
                  <span class="garde-texte">{t('lecture.imagesBloquees', { n: fil.imagesBloquees[k] })}</span>
                  <button type="button" data-testid="afficher-images"
                          onclick={() => afficherImages(m)}>
                    {t('lecture.afficherImages')}</button>
                </div>
              {/if}
              <iframe class="corps" sandbox="allow-same-origin" srcdoc={fil.corps[k] ?? ''}
                      title={t('lecture.corps')}
                      onload={(ev) => brancherLiens(ev.currentTarget)}></iframe>
              {#if nbPiecesDe(m) > 0}
                <div class="fichiers" data-testid="lecture-fichiers">
                  <p class="titre-fichiers">{t('conv.fichiersJoints')}</p>
                  <div class="puces">
                    {#each fil.pieces[k] ?? [] as piece (piece.index)}
                      <button type="button" class="puce bouton" data-testid="piece-jointe"
                              disabled={enregistrements[`${k}#${piece.index}`]}
                              onclick={() => enregistrer(m, piece)}
                              title={t('lecture.enregistrer')}>
                        <span class="ms" aria-hidden="true">description</span>{piece.name}</button>
                      <span class="puce"><span class="ms" aria-hidden="true">storage</span>{piece.size}</span>
                    {/each}
                  </div>
                </div>
              {/if}
            </div>
          </article>
        {:else}
          <div class="replie" data-testid="message-replie"
               role="button" tabindex="0" aria-expanded="false"
               onclick={() => basculerMessage(m)} onkeydown={activation(() => basculerMessage(m))}>
            <span class="auteur">{m.sender}</span>
            <span class="apercu">{m.preview ?? ''}</span>
            <span class="heure">{quand(m.epoch)}</span>
          </div>
        {/if}
      {/each}
      {#if brouillonDuFil}
        <div class="replie brouillon" data-testid="conv-brouillon"
             role="button" tabindex="0"
             onclick={() => onreprendre(brouillonDuFil)}
             onkeydown={activation(() => onreprendre(brouillonDuFil))}>
          <span class="mention"><span class="ms" aria-hidden="true">edit_note</span>{t('conv.brouillon')}</span>
          <span class="apercu">{brouillonDuFil.body}</span>
          <span class="heure">{quand(Math.floor(brouillonDuFil.updated_epoch / 1000))}</span>
          <span class="reprendre">{t('action.reprendre')}</span>
        </div>
      {/if}
    </div>

    <div class="actions">
      <button type="button" class="principal" data-testid="repondre"
              onclick={() => onrepondre(cible())}>
        <span class="ms" aria-hidden="true">reply</span>{t('action.repondre')}</button>
      <button type="button" data-testid="repondre-tous"
              onclick={() => onrepondretous(cible())}>
        <span class="ms" aria-hidden="true">reply_all</span>{t('action.repondreTous')}</button>
      <button type="button" data-testid="transferer"
              onclick={() => ontransferer(dernier())}>
        <span class="ms miroir" aria-hidden="true">reply</span>{t('action.transferer')}</button>
      <button type="button" data-testid="archiver" onclick={() => onarchiver(fil.ligne)}>
        <span class="ms" aria-hidden="true">archive</span>{t('action.archiver')}</button>
      <button type="button" data-testid="supprimer" onclick={() => onsupprimer(fil.ligne)}>
        <span class="ms" aria-hidden="true">delete</span>{t('action.supprimer')}</button>
    </div>
  </div>
{/if}

<style>
  /* Géométrie héritée de l'écran 03 (verbatim du prototype) — le même
     objet dans les deux cadres, seule la largeur disponible change. */
  .objet-fil { display:flex; flex-direction:column; min-height:0; flex:1; }
  .tete {
    padding:22px 26px 16px; border-bottom:1px solid var(--border);
    display:flex; flex-direction:column; gap:12px; flex:none;
  }
  .titre {
    margin:0; font-size:24px; font-weight:600; line-height:1.2;
    letter-spacing:-.01em; color:var(--ink);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
  }
  .puces { display:flex; align-items:center; gap:10px; flex-wrap:wrap; }
  .puce {
    height:32px; padding:0 12px; display:inline-flex; align-items:center;
    gap:8px; font-size:13px; color:var(--ink2); background:var(--surface);
    border:1px solid var(--border); border-radius:6px; white-space:nowrap;
  }
  .puce.bouton { cursor:pointer; }
  .puce.bouton:hover { background:var(--sel); }
  .puce.bouton.inerte { cursor:default; opacity:.55; }
  .puce.bouton.inerte:hover { background:var(--surface); }
  .fil { flex:1; overflow-y:auto; padding:14px 26px; min-height:0; }
  .replie {
    display:flex; align-items:center; gap:12px; padding:12px 16px;
    margin-bottom:10px; background:var(--surface);
    border:1px solid var(--border); border-radius:10px; cursor:pointer;
    font-size:13px;
  }
  .replie:hover { background:var(--hover); }
  .replie .auteur { font-weight:600; color:var(--ink); flex:none; }
  .replie .apercu {
    flex:1; min-width:0; color:var(--muted);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
  }
  .replie .heure { color:var(--muted); font-size:12px; flex:none; }
  .replie.brouillon { border:1.5px dashed var(--accent); background:none; }
  .replie.brouillon .mention {
    color:var(--alert); font-weight:600; display:inline-flex;
    align-items:center; gap:6px; flex:none;
  }
  .replie.brouillon .reprendre { color:var(--accent); font-weight:600; flex:none; }
  .deplie {
    background:var(--surface); border:1px solid var(--border);
    border-radius:10px; box-shadow:var(--shadow); margin-bottom:10px;
    display:flex; flex-direction:column;
  }
  .tete-message {
    display:flex; align-items:center; gap:12px; padding:12px 16px;
    border-bottom:1px solid var(--border); cursor:pointer; font-size:13px;
  }
  .tete-message .auteur { font-weight:600; color:var(--ink); flex:1; min-width:0; }
  .tete-message .heure { color:var(--muted); font-size:12px; flex:none; }
  .tete-message .puce { height:26px; padding:0 9px; font-size:12px; }
  .contenu { padding:14px 16px 16px; display:flex; flex-direction:column; gap:12px; }
  .adresses {
    margin:0; display:grid; grid-template-columns:auto 1fr;
    column-gap:16px; row-gap:6px; font-size:13px;
  }
  .adresses dt { color:var(--muted); }
  .adresses dd { margin:0; color:var(--ink2); }
  .garde-images {
    padding:10px 14px; display:flex; align-items:center; gap:10px;
    font-size:13px; color:var(--ink2); background:var(--panel);
    border:1px solid var(--border); border-radius:6px;
  }
  .garde-images .ms { color:var(--muted); }
  .garde-texte { flex:1; }
  .garde-images button {
    height:26px; padding:0 10px; font-size:12px; color:var(--ink);
    background:var(--surface); border:1px solid var(--border);
    border-radius:6px; cursor:pointer;
  }
  .garde-images button:hover { background:var(--sel); }
  .corps {
    /* Déborde de 12 px : la gouttière interne du document assaini
       (mail-render) ramène le texte au fil des De/À/Objet. Le fond au
       jeton — le document bake la même valeur (revue A42). */
    border:none; background:var(--surface);
    margin-left:-12px; width:calc(100% + 24px);
    height:clamp(220px, 45vh, 520px);
  }
  .titre-fichiers {
    margin:0 0 8px; font-size:12px; font-weight:600; letter-spacing:.1em;
    text-transform:uppercase; color:var(--muted);
  }
  .fichiers .puces { gap:8px; }
  .fichiers .puce { height:28px; }
  .actions {
    flex:none; padding:14px 26px; border-top:1px solid var(--border);
    display:flex; gap:12px; flex-wrap:wrap;
  }
  .actions button {
    height:32px; padding:0 16px; display:inline-flex; align-items:center;
    gap:8px; font-size:13px; color:var(--ink); background:var(--surface);
    border:1px solid var(--border); border-radius:6px; cursor:pointer;
  }
  .actions button:hover { background:var(--sel); }
  .actions .principal {
    font-weight:600; color:var(--onAccent); background:var(--accent);
    border-color:var(--accent);
  }
  .actions .principal:hover { background:var(--accentH); border-color:var(--accentH); }
</style>
