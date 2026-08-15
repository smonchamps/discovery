<script>
  // Écran 03 du prototype — la conversation PLEIN ÉCRAN : entête propre
  // (retour, Écrire inerte — P4), carte signature unique, messages
  // repliés/dépliés (dernier déplié par défaut), « Tout déplier »,
  // De/À/Objet, fichiers joints réels, barre des 4 actions.
  //
  // Invariant intact : chaque message déplié porte SA propre iframe
  // sandbox (S1), servie par `message_body`, chargée au dépliage
  // seulement — « Tout déplier » sur un long fil ne monte les corps que
  // des messages effectivement dépliés. Le jeton `allow-same-origin` —
  // SANS allow-scripts, le contenu reste inerte — sert l'interception
  // des liens (lib/liens.js, terrain 2026-08-15). Écart assumé : le
  // corps défile dans une fenêtre bornée au lieu de couler comme au
  // prototype (hauteur figée, héritée de l'époque du bac à sable
  // opaque — la mesurer serait désormais possible, troc non rejoué).
  //
  // « À » n'est pas stocké par le cœur : la règle du prototype
  // s'applique — message de soi → premier autre correspondant du fil,
  // sinon l'adresse du compte. Approximation dite, pas cachée.
  import { appel } from './lib/transport.js';
  import { paletteLecture } from './lib/theme.js';
  import { brancherLiens } from './lib/liens.js';
  import { quand } from './lib/quand.js';
  import { activation } from './lib/clavier.js';
  import { t } from './lib/texte.svelte.js';

  let {
    // Les brouillons locaux (sonde de l'App) : le fil ouvert qui en
    // porte un le montre en DERNIÈRE position (PLAN-BROUILLONS, B-D4-b)
    // — la liste promettait un « dernier email », l'écran 03 le tient.
    brouillons = [],
    onreprendre = () => {},
    onretour = () => {},
    onarchiver = () => {},
    onsupprimer = () => {},
    onrepondre = () => {},
    onrepondretous = () => {},
    ontransferer = () => {},
    onecrire = () => {},
    onflash = () => {},
  } = $props();

  let ligne = $state(null);
  let fil = $state([]);
  let deplies = $state({});
  let corps = $state({});
  let pieces = $state({});
  let jeton = 0;

  const cle = (m) => `${m.account_id}/${m.mailbox}/${m.uid}`;

  // Un écho local (PLAN-REACTIVITE E3) se reconnaît à sa boîte
  // synthétique — son corps est local (echo_body), jamais de fil.
  const estEcho = (m) => typeof m?.mailbox === 'string' && m.mailbox.startsWith('echo:');

  export async function ouvrir(nouvelle) {
    const mien = ++jeton;
    ligne = nouvelle;
    fil = [];
    deplies = {};
    corps = {};
    pieces = {};
    // V-D2 (PLAN-VOLETS) : sans fil — écho compris — l'écran 03 sert
    // le MESSAGE SEUL : le fil est la ligne elle-même, le corps vient
    // de message_body/echo_body (le repli de la Lecture). Une seule
    // surface de lecture plein écran dans le produit.
    if (nouvelle.thread_id == null) {
      fil = [nouvelle];
      basculer(nouvelle, true);
      return;
    }
    try {
      const messages = await appel('thread_messages', { threadId: nouvelle.thread_id });
      if (mien !== jeton) return;
      fil = messages;
      const dernier = messages[messages.length - 1];
      if (dernier) basculer(dernier, true);
    } catch (err) {
      console.error('thread_messages :', err);
    }
  }
  export function fermer() {
    jeton += 1;
    ligne = null;
    fil = [];
  }
  export function estOuverte() {
    return ligne !== null;
  }

  async function chargerMessage(m) {
    const k = cle(m);
    if (corps[k] === undefined) {
      corps[k] = '';
      try {
        const vue = estEcho(m)
          ? await appel('echo_body', {
              id: Number(m.mailbox.slice(5)),
              showImages: false,
              palette: paletteLecture(),
            })
          : await appel('message_body', {
              accountId: m.account_id,
              mailbox: m.mailbox,
              uid: m.uid,
              showImages: false,
              palette: paletteLecture(),
            });
        corps[k] = vue.document;
      } catch (err) {
        console.error('message_body :', err);
      }
    }
    // Les pièces d'un écho n'ont pas de métadonnées par pièce pendant
    // la fenêtre de réconciliation — même règle que la Lecture.
    if (m.attachment_count > 0 && !estEcho(m) && pieces[k] === undefined) {
      pieces[k] = [];
      try {
        pieces[k] = await appel('message_attachments', {
          accountId: m.account_id,
          mailbox: m.mailbox,
          uid: m.uid,
        });
      } catch (err) {
        console.error('message_attachments :', err);
      }
    }
  }
  function basculer(m, valeur = null) {
    const k = cle(m);
    const nouveau = valeur ?? !deplies[k];
    deplies[k] = nouveau;
    if (nouveau) chargerMessage(m);
  }
  function toutDeplier() {
    for (const m of fil) basculer(m, true);
  }

  // Enregistrer une pièce jointe (dû de bascule §6) : les octets sont
  // retéléchargés à la demande, une fois — jamais mis en cache (ADR
  // 0007). Le visuel des puces ne change pas, elles deviennent des
  // boutons.
  let enregistrements = $state({});
  async function enregistrer(m, piece) {
    const k = `${cle(m)}#${piece.index}`;
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

  // Répondre / Transférer depuis la conversation visent le DERNIER
  // message du fil — la règle du prototype (la réponse reprend ses
  // fichiers, la citation son corps).
  const dernier = () => fil[fil.length - 1] ?? ligne;

  // Le brouillon du fil ouvert — le plus récent s'il y en a plusieurs
  // (B-D5), même règle que la mention en liste. Le bloc disparaît de
  // lui-même à la sonde suivante quand le brouillon est réglé.
  const brouillonDuFil = $derived.by(() => {
    if (!ligne || ligne.thread_id == null) return null;
    let retenu = null;
    for (const b of brouillons) {
      if (b.thread_id !== ligne.thread_id) continue;
      if (!retenu || b.updated_epoch > retenu.updated_epoch) retenu = b;
    }
    return retenu;
  });

  const propre = (m) => ligne && m.sender_address === ligne.account_email;
  const ligneDe = (m) =>
    m.sender_address ? `${m.sender} <${m.sender_address}>` : m.sender;
  function ligneA(m) {
    if (!propre(m)) return ligne?.account_email ?? '';
    const autre = fil.find(
      (x) => x.sender_address && x.sender_address !== m.sender_address,
    );
    return autre ? `${autre.sender} <${autre.sender_address}>` : (ligne?.account_email ?? '');
  }
</script>

{#if ligne}
  <div class="ecran03" data-testid="conversation">
    <header class="entete">
      <button type="button" class="retour" data-testid="retour-boite" onclick={onretour}>
        <span class="ms" aria-hidden="true">arrow_back</span>{t('boite.reception')}</button>
      <span class="espace"></span>
      <button type="button" class="principal" onclick={onecrire}>
        <span class="ms" aria-hidden="true">edit_square</span>{t('entete.ecrire')}</button>
    </header>

    <div class="scene">
      <div class="carte">
        <div class="tete">
          <h3 class="titre" data-testid="conversation-sujet">{ligne.subject}</h3>
          <div class="puces">
            {#if ligne.thread_size > 1}
              <span class="puce"><span class="ms" aria-hidden="true">forum</span>{t('puce.messages', { n: ligne.thread_size })}</span>
            {/if}
            {#if ligne.attachment_count > 0}
              <span class="puce"><span class="ms" aria-hidden="true">attach_file</span>{t('puce.fichiers', { n: ligne.attachment_count })}</span>
            {/if}
            <span class="puce bouton" data-testid="tout-deplier"
                  role="button" tabindex="0"
                  onclick={toutDeplier} onkeydown={activation(toutDeplier)}>
              <span class="ms" aria-hidden="true">unfold_more</span>{t('conv.toutDeplier')}</span>
          </div>
        </div>

        <div class="fil">
          {#each fil as m (cle(m))}
            {#if deplies[cle(m)]}
              <article class="deplie" data-testid="message-deplie">
                <div class="tete-message" role="button" tabindex="0"
                     aria-expanded="true"
                     onclick={() => basculer(m)} onkeydown={activation(() => basculer(m))}>
                  <span class="auteur">{m.sender}</span>
                  {#if m.attachment_count > 0}
                    <span class="puce"><span class="ms" aria-hidden="true">attach_file</span>{t('puce.fichiers', { n: m.attachment_count })}</span>
                  {/if}
                  <span class="heure">{quand(m.epoch)}</span>
                </div>
                <div class="contenu">
                  <dl class="adresses">
                    <dt>{t('conv.de')}</dt><dd>{ligneDe(m)}</dd>
                    <dt>{t('conv.a')}</dt><dd>{ligneA(m)}</dd>
                    <dt>{t('conv.objet')}</dt><dd>{m.subject}</dd>
                  </dl>
                  <iframe class="corps" sandbox="allow-same-origin" srcdoc={corps[cle(m)] ?? ''}
                          title={t('lecture.corps')}
                          onload={(ev) => brancherLiens(ev.currentTarget)}></iframe>
                  {#if m.attachment_count > 0}
                    <div class="fichiers">
                      <p class="titre-fichiers">{t('conv.fichiersJoints')}</p>
                      <div class="puces">
                        {#each pieces[cle(m)] ?? [] as piece (piece.index)}
                          <button type="button" class="puce bouton" data-testid="piece-jointe"
                                  disabled={enregistrements[`${cle(m)}#${piece.index}`]}
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
                   onclick={() => basculer(m)} onkeydown={activation(() => basculer(m))}>
                <span class="auteur">{m.sender}</span>
                <span class="apercu">{m.preview ?? ''}</span>
                <span class="heure">{quand(m.epoch)}</span>
              </div>
            {/if}
          {/each}
          {#if brouillonDuFil}
            <!-- Le trait pointillé dit « pas encore parti » ; le bloc
                 ENTIER reprend, le bouton nomme le geste (maquette §3).
                 Le composeur se superpose, la conversation reste
                 montée dessous. -->
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
          <button type="button" class="principal" data-testid="conv-repondre"
                  onclick={() => onrepondre(dernier())}>
            <span class="ms" aria-hidden="true">reply</span>{t('action.repondre')}</button>
          <button type="button" data-testid="conv-repondre-tous"
                  onclick={() => onrepondretous(dernier())}>
            <span class="ms" aria-hidden="true">reply_all</span>{t('action.repondreTous')}</button>
          <button type="button" data-testid="conv-transferer"
                  onclick={() => ontransferer(dernier())}>
            <span class="ms miroir" aria-hidden="true">reply</span>{t('action.transferer')}</button>
          <button type="button" data-testid="conv-archiver" onclick={() => onarchiver(ligne)}>
            <span class="ms" aria-hidden="true">archive</span>{t('action.archiver')}</button>
          <button type="button" data-testid="conv-supprimer" onclick={() => onsupprimer(ligne)}>
            <span class="ms" aria-hidden="true">delete</span>{t('action.supprimer')}</button>
        </div>
      </div>
    </div>
  </div>
{/if}

<style>
  /* Géométrie VERBATIM de l'écran 03 du prototype. */
  .ecran03 {
    position:absolute; inset:0; display:flex; flex-direction:column;
    background:var(--bg); z-index:1;
  }
  .entete {
    height:60px; flex:none; background:var(--surface);
    border-bottom:1px solid var(--border); display:flex;
    align-items:center; gap:20px; padding:0 24px;
  }
  .espace { flex:1; }
  button {
    height:32px; padding:0 16px; display:inline-flex; align-items:center;
    gap:8px; font-size:13px; color:var(--ink); background:var(--surface);
    border:1px solid var(--border); border-radius:6px; cursor:pointer;
  }
  button:hover { background:var(--sel); }
  .retour { padding:0 14px; }
  .principal {
    font-weight:600; color:var(--onAccent); background:var(--accent);
    border-color:var(--accent);
  }
  .principal:hover { background:var(--accentH); border-color:var(--accentH); }

  .scene { flex:1; padding:28px; display:flex; min-height:0; }
  .carte {
    flex:1; background:var(--surface); border:1px solid var(--border); border-radius:10px;
    box-shadow:var(--shadow); display:flex; flex-direction:column;
    overflow:hidden; min-height:0;
  }
  .tete {
    padding:28px 36px 24px; border-bottom:1px solid var(--border);
    display:flex; flex-direction:column; gap:14px;
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

  .fil {
    flex:1; padding:24px 36px; display:flex; flex-direction:column;
    gap:12px; min-height:0; overflow:auto;
  }
  .replie {
    flex:none; border:1px solid var(--border); border-radius:10px;
    padding:14px 20px; display:flex; align-items:center; gap:14px;
    background:var(--bg); cursor:pointer;
  }
  .replie:hover { background:var(--sel); }
  .replie .auteur { font-size:13px; font-weight:600; flex:1; color:var(--ink); }
  .replie .apercu {
    font-size:13px; color:var(--ink2); overflow:hidden;
    text-overflow:ellipsis; white-space:nowrap; max-width:40ch;
  }
  .heure { font-size:12px; color:var(--muted); }
  /* Le bloc brouillon (PLAN-BROUILLONS §3, maquette validée) : la forme
     d'un message replié, le trait accent POINTILLÉ en plus. */
  .replie.brouillon { border:1px dashed var(--accent); }
  .brouillon .mention {
    display:inline-flex; align-items:center; gap:6px; flex:none;
    font-size:13px; font-weight:600; color:var(--alert);
  }
  .brouillon .mention .ms { font-size:15px; }
  .brouillon .apercu { flex:1; max-width:none; }
  .brouillon .reprendre {
    height:26px; padding:0 12px; display:inline-flex; align-items:center;
    flex:none; font-size:12px; font-weight:600; color:var(--ink2);
    background:var(--surface); border:1px solid var(--border);
    border-radius:6px;
  }
  .replie.brouillon:hover .reprendre { background:var(--sel); color:var(--ink); }

  .deplie {
    flex:none; border:1px solid var(--border); border-radius:10px;
    background:var(--surface); box-shadow:var(--shadow); overflow:hidden;
  }
  .tete-message {
    padding:20px 24px; display:flex; align-items:center; gap:14px;
    border-bottom:1px solid var(--border); cursor:pointer;
  }
  .tete-message .auteur { font-size:15px; font-weight:600; flex:1; color:var(--ink); }
  .contenu {
    padding:22px 24px 26px; display:flex; flex-direction:column; gap:20px;
  }
  .adresses {
    margin:0; display:grid; grid-template-columns:auto 1fr;
    column-gap:16px; row-gap:6px; font-size:13px;
  }
  .adresses dt { color:var(--muted); }
  .adresses dd { margin:0; color:var(--ink2); }
  .corps {
    /* Déborde de 12 px à gauche et à droite : la gouttière interne du
       document assaini (mail-render) ramène le texte au fil des
       De/À/Objet (verdict terrain — alignement). */
    border:none; background:var(--surface);
    margin-left:-12px; width:calc(100% + 24px);
    height:clamp(220px, 45vh, 520px);
  }
  .titre-fichiers {
    margin:0 0 12px; font-size:12px; letter-spacing:.1em;
    text-transform:uppercase; color:var(--muted); font-weight:600;
  }

  .actions {
    padding:20px 36px; border-top:1px solid var(--border);
    display:flex; gap:12px;
  }
</style>
