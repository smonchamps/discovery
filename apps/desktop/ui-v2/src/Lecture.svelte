<script>
  // Volet de lecture de l'écran 02 — VERBATIM du prototype : carte
  // signature, titre 24 px tronqué, puce méta + « Dernier message · … »
  // + « Voir la conversation », auteur, corps, barre des 4 actions.
  //
  // Invariant intact : le corps vit dans l'iframe sandbox, servie par
  // `message_body` (assaini côté coeur, images distantes bloquées, encre
  // bakée par thème — S1), jamais innerHTML.
  //
  import { appel } from './lib/transport.js';
  import { quandLong } from './lib/quand.js';

  let {
    onarchiver = () => {},
    onsupprimer = () => {},
    onconversation = () => {},
    onrepondre = () => {},
    ontransferer = () => {},
  } = $props();

  let ligne = $state(null);
  let corps = $state('');
  let derniereOuvertureMs = $state(null);
  // Jeton d'ouverture, PAS une comparaison d'objet : `$state` enveloppe
  // les objets dans un proxy, `ligne !== nouvelle` après un await est
  // donc toujours vrai — le corps ne se posait jamais.
  let jeton = 0;

  const meta = $derived.by(() => {
    if (!ligne) return '';
    const messages = ligne.thread_size > 1 ? `${ligne.thread_size} messages` : '1 message';
    const n = ligne.attachment_count;
    return n > 0 ? `${messages} · ${n} fichier${n > 1 ? 's' : ''}` : messages;
  });

  export async function ouvrir(nouvelle) {
    const t0 = performance.now();
    const mien = ++jeton;
    ligne = nouvelle;
    try {
      const vue = await appel('message_body', {
        accountId: nouvelle.account_id,
        mailbox: nouvelle.mailbox,
        uid: nouvelle.uid,
        showImages: false,
      });
      if (mien !== jeton) return derniereOuvertureMs; // sélection changée
      corps = vue.document;
    } catch (err) {
      corps = '';
      console.error('message_body :', err);
    }
    derniereOuvertureMs = performance.now() - t0;
    return derniereOuvertureMs;
  }
  export function fermer() {
    ligne = null;
    corps = '';
  }
  export function etat() {
    return { derniereOuvertureMs };
  }
</script>

<main aria-label="Message" data-testid="volet-lecture">
  {#if !ligne}
    <p class="vide">Sélectionnez un message pour le lire.</p>
  {:else}
    <div class="carte">
      <div class="entete">
        <h3 class="titre" data-testid="lecture-sujet">{ligne.subject}</h3>
        <div class="metas">
          <span class="puce">{meta}</span>
          <span class="dernier">Dernier message · {quandLong(ligne.epoch)}</span>
          <span class="puce" class:cliquable={ligne.thread_id != null}
                data-testid="voir-conversation"
                onclick={() => ligne.thread_id != null && onconversation(ligne)}>
            <span class="ms" aria-hidden="true">unfold_more</span>Voir la conversation</span>
        </div>
      </div>
      <div class="auteur">
        <span class="nom">{ligne.sender}</span>
        <span class="adresse">à {ligne.account_email}</span>
      </div>
      <iframe class="corps" sandbox srcdoc={corps} title="Contenu du message"></iframe>
      <div class="actions">
        <button type="button" class="principal" data-testid="repondre" onclick={() => onrepondre(ligne)}>
          <span class="ms" aria-hidden="true">reply</span>Répondre</button>
        <button type="button" data-testid="transferer" onclick={() => ontransferer(ligne)}>
          <span class="ms" aria-hidden="true">forward</span>Transférer</button>
        <button type="button" data-testid="archiver" onclick={() => onarchiver(ligne)}>
          <span class="ms" aria-hidden="true">archive</span>Archiver</button>
        <button type="button" data-testid="supprimer" onclick={() => onsupprimer(ligne)}>
          <span class="ms" aria-hidden="true">delete</span>Supprimer</button>
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
    flex:1; background:var(--surface); border:1px solid var(--border);
    border-left:2px solid var(--accent); border-radius:10px;
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
  .corps {
    flex:1; border:none; background:#ffffff; margin:18px 30px 0;
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
