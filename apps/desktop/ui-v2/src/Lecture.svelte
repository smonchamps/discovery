<script>
  // Lecture MINIMALE — le refus de périmètre P1 : de quoi mesurer
  // l'ouverture d'un message, rien de plus. Le volet complet du
  // prototype (méta, puces, barre des 4 actions) est P2.
  //
  // Invariant intact : le corps vit dans une iframe sandbox, servie par
  // `message_body` (assaini côté coeur, images distantes bloquées),
  // jamais innerHTML.
  import { appel } from './lib/transport.js';

  let sujet = $state('');
  let expediteur = $state('');
  let corps = $state('');
  let vide = $state(true);
  let derniereOuvertureMs = $state(null);

  export async function ouvrir(ligne) {
    const t0 = performance.now();
    sujet = ligne.subject;
    expediteur = ligne.sender;
    vide = false;
    try {
      const vue = await appel('message_body', {
        accountId: ligne.account_id,
        mailbox: ligne.mailbox,
        uid: ligne.uid,
        showImages: false,
      });
      corps = vue.document;
    } catch (err) {
      corps = '';
      console.error('message_body :', err);
    }
    derniereOuvertureMs = performance.now() - t0;
    return derniereOuvertureMs;
  }
  export function etat() {
    return { derniereOuvertureMs };
  }
</script>

<main aria-label="Message" data-testid="volet-lecture">
  {#if vide}
    <p class="vide">Sélectionnez un message pour le lire.</p>
  {:else}
    <div class="carte">
      <div class="entete">
        <h3 class="titre" data-testid="lecture-sujet">{sujet}</h3>
        <span class="exp">{expediteur}</span>
      </div>
      <iframe class="corps" sandbox srcdoc={corps} title="Contenu du message"></iframe>
    </div>
  {/if}
</main>

<style>
  main {
    background:var(--bg); padding:12px 20px 20px; min-width:0;
    display:flex; flex-direction:column; height:100%;
  }
  .vide {
    margin:auto; font-size:13px; line-height:1.5; color:var(--muted);
    text-align:center;
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
  .exp { font-size:15px; font-weight:600; color:var(--ink); }
  .corps { flex:1; border:none; background:#ffffff; }
</style>
