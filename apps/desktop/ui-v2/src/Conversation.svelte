<script>
  // Écran 03 du prototype — la conversation PLEIN ÉCRAN : entête propre
  // (retour, Écrire inerte — P4), carte signature unique, messages
  // repliés/dépliés (dernier déplié par défaut), « Tout déplier »,
  // De/À/Objet, fichiers joints réels, barre des 4 actions.
  //
  // Invariant intact : chaque message déplié porte SA propre iframe
  // sandbox (S1), servie par `message_body`, chargée au dépliage
  // seulement — « Tout déplier » sur un long fil ne monte les corps que
  // des messages effectivement dépliés. Écart assumé : le bac à sable
  // opaque interdit de mesurer la hauteur du contenu, le corps défile
  // dans une fenêtre bornée au lieu de couler comme au prototype —
  // relâcher le sandbox pour de la hauteur serait un troc refusé.
  //
  // « À » n'est pas stocké par le cœur : la règle du prototype
  // s'applique — message de soi → premier autre correspondant du fil,
  // sinon l'adresse du compte. Approximation dite, pas cachée.
  import { appel } from './lib/transport.js';
  import { quand } from './lib/quand.js';
  import { activation } from './lib/clavier.js';

  let {
    onretour = () => {},
    onarchiver = () => {},
    onsupprimer = () => {},
    onrepondre = () => {},
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

  export async function ouvrir(nouvelle) {
    const mien = ++jeton;
    ligne = nouvelle;
    fil = [];
    deplies = {};
    corps = {};
    pieces = {};
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
        const vue = await appel('message_body', {
          accountId: m.account_id,
          mailbox: m.mailbox,
          uid: m.uid,
          showImages: false,
        });
        corps[k] = vue.document;
      } catch (err) {
        console.error('message_body :', err);
      }
    }
    if (m.attachment_count > 0 && pieces[k] === undefined) {
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
      onflash(`Pièce enregistrée : ${chemin}`);
    } catch (err) {
      onflash(`Enregistrement impossible : ${err}`);
    } finally {
      enregistrements[k] = false;
    }
  }

  // Répondre / Transférer depuis la conversation visent le DERNIER
  // message du fil — la règle du prototype (la réponse reprend ses
  // fichiers, la citation son corps).
  const dernier = () => fil[fil.length - 1] ?? ligne;

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
        <span class="ms" aria-hidden="true">arrow_back</span>Boîte de réception</button>
      <span class="espace"></span>
      <button type="button" class="principal" onclick={onecrire}>
        <span class="ms" aria-hidden="true">edit_square</span>Écrire</button>
    </header>

    <div class="scene">
      <div class="carte">
        <div class="tete">
          <h3 class="titre" data-testid="conversation-sujet">{ligne.subject}</h3>
          <div class="puces">
            {#if ligne.thread_size > 1}
              <span class="puce"><span class="ms" aria-hidden="true">forum</span>{ligne.thread_size} messages</span>
            {/if}
            {#if ligne.attachment_count > 0}
              <span class="puce"><span class="ms" aria-hidden="true">attach_file</span>{ligne.attachment_count} fichier{ligne.attachment_count > 1 ? 's' : ''}</span>
            {/if}
            <span class="puce bouton" data-testid="tout-deplier"
                  role="button" tabindex="0"
                  onclick={toutDeplier} onkeydown={activation(toutDeplier)}>
              <span class="ms" aria-hidden="true">unfold_more</span>Tout déplier</span>
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
                    <span class="puce"><span class="ms" aria-hidden="true">attach_file</span>{m.attachment_count} fichier{m.attachment_count > 1 ? 's' : ''}</span>
                  {/if}
                  <span class="heure">{quand(m.epoch)}</span>
                </div>
                <div class="contenu">
                  <dl class="adresses">
                    <dt>De</dt><dd>{ligneDe(m)}</dd>
                    <dt>À</dt><dd>{ligneA(m)}</dd>
                    <dt>Objet</dt><dd>{m.subject}</dd>
                  </dl>
                  <iframe class="corps" sandbox srcdoc={corps[cle(m)] ?? ''}
                          title="Contenu du message"></iframe>
                  {#if m.attachment_count > 0}
                    <div class="fichiers">
                      <p class="titre-fichiers">Fichiers joints</p>
                      <div class="puces">
                        {#each pieces[cle(m)] ?? [] as piece (piece.index)}
                          <button type="button" class="puce bouton" data-testid="piece-jointe"
                                  disabled={enregistrements[`${cle(m)}#${piece.index}`]}
                                  onclick={() => enregistrer(m, piece)}
                                  title="Enregistrer dans Téléchargements">
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
        </div>

        <div class="actions">
          <button type="button" class="principal" data-testid="conv-repondre"
                  onclick={() => onrepondre(dernier())}>
            <span class="ms" aria-hidden="true">reply</span>Répondre</button>
          <button type="button" data-testid="conv-transferer"
                  onclick={() => ontransferer(dernier())}>
            <span class="ms miroir" aria-hidden="true">reply</span>Transférer</button>
          <button type="button" data-testid="conv-archiver" onclick={() => onarchiver(ligne)}>
            <span class="ms" aria-hidden="true">archive</span>Archiver</button>
          <button type="button" data-testid="conv-supprimer" onclick={() => onsupprimer(ligne)}>
            <span class="ms" aria-hidden="true">delete</span>Supprimer</button>
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
    flex:1; background:var(--surface); border:1px solid var(--border);
    border-left:2px solid var(--accent); border-radius:10px;
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

  .deplie {
    flex:none; border:1px solid var(--border);
    border-left:2px solid var(--accent); border-radius:10px;
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
    border:none; background:#ffffff;
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
