<script>
  // Le volet de lecture de l'écran 02 — depuis UI v3 (verdict CE du
  // 2026-08-16, ANNOTATIONS-V3 §6), le volet montre le FIL en cartes :
  // il n'est qu'un CADRE autour de Fil.svelte, le même objet que
  // l'écran 03 (décision D4). L'exclusivité des cadres est portée par
  // le store (`fil.cadre`, revue v3) : ce composant ne rend l'objet
  // que quand le cadre est le sien — rien à réconcilier à la main.
  import Fil from './Fil.svelte';
  import { fil, ouvrirFil, fermerFil } from './lib/fil.svelte.js';
  import { t } from './lib/texte.svelte.js';

  let {
    brouillons = [],
    onreprendre = () => {},
    onarchiver = () => {},
    onsupprimer = () => {},
    onconversation = () => {},
    onrepondre = () => {},
    onrepondretous = () => {},
    ontransferer = () => {},
    onflash = () => {},
  } = $props();

  export function ouvrir(nouvelle) {
    return ouvrirFil(nouvelle, 'volet');
  }
  export function fermer() {
    fermerFil();
  }
  export function etat() {
    return { derniereOuvertureMs: fil.derniereOuvertureMs };
  }
</script>

<main aria-label={t('lecture.aria')} data-testid="volet-lecture">
  {#if fil.cadre === 'volet' && fil.ligne}
    <Fil {brouillons} {onreprendre} {onarchiver} {onsupprimer}
         {onrepondre} {onrepondretous} {ontransferer} {onflash}
         onagrandir={onconversation} />
  {:else if fil.cadre !== 'plein'}
    <p class="vide">{t('lecture.vide')}</p>
  {/if}
</main>

<style>
  /* Le volet est À PLAT (terrain A46, .voletLecture du prototype) :
     un panneau qui défile en un seul flot — l'élévation appartient aux
     cartes de message, jamais au volet entier. */
  main {
    background:var(--bg); padding:18px 22px; min-width:0;
    display:flex; flex-direction:column; min-height:0; overflow-y:auto;
  }
  .vide {
    margin:auto; font-size:13px; line-height:1.5; color:var(--muted);
    text-align:center; padding:40px;
  }
</style>
