<script>
  // Écran 03 — la conversation PLEIN ÉCRAN : depuis UI v3 (décision D4
  // du 2026-08-16), un CADRE (entête retour/Écrire + scène) autour de
  // Fil.svelte, le même objet que le volet de lecture. L'exclusivité
  // vit dans le store (`fil.cadre`, revue v3) : ce composant rend
  // quand le cadre est 'plein', point — aucun booléen local à
  // désynchroniser. Agrandir ne recharge rien (`agrandirFil`) ; une
  // sélection directe en 1-2 volets recharge (`ouvrirFil`).
  import Fil from './Fil.svelte';
  import { fil, ouvrirFil, agrandirFil } from './lib/fil.svelte.js';
  import { t } from './lib/texte.svelte.js';

  let {
    brouillons = [],
    onreprendre = () => {},
    onretour = () => {},
    onarchiver = () => {},
    onsupprimer = () => {},
    onrepondre = () => {},
    onrepondretous = () => {},
    ontransferer = () => {},
    onspam = () => {},
    onnonspam = () => {},
    estIndesirable = false,
    onecrire = () => {},
    onflash = () => {},
  } = $props();

  export function ouvrir(nouvelle) {
    // Le MÊME objet déjà tenu par le volet : changement de taille pur.
    if (fil.ligne && fil.cadre === 'volet'
        && fil.ligne.account_id === nouvelle.account_id
        && fil.ligne.mailbox === nouvelle.mailbox
        && fil.ligne.uid === nouvelle.uid) {
      agrandirFil();
      return Promise.resolve(fil.derniereOuvertureMs);
    }
    return ouvrirFil(nouvelle, 'plein');
  }
  export function estOuverte() {
    return fil.cadre === 'plein';
  }
</script>

{#if fil.cadre === 'plein' && fil.ligne}
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
        <Fil {brouillons} {onreprendre} {onarchiver} {onsupprimer}
             {onrepondre} {onrepondretous} {ontransferer}
             {onspam} {onnonspam} {estIndesirable} {onflash} />
      </div>
    </div>
  </div>
{/if}

<style>
  /* Géométrie de l'écran 03 — l'entête à 52 px depuis UI v3 (E4) :
     les deux cadres du même objet partagent le même chrome, sans saut
     à l'agrandissement (revue v3 : 60 px faisait sauter de 8 px). */
  .ecran03 {
    position:absolute; inset:0; display:flex; flex-direction:column;
    background:var(--bg); z-index:1;
  }
  .entete {
    height:52px; flex:none; background:var(--surface);
    border-bottom:1px solid var(--border); display:flex;
    align-items:center; gap:12px; padding:0 14px;
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
</style>
