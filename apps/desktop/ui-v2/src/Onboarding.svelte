<script>
  // Écran 01 du prototype, affiché à zéro compte : « Votre adresse.
  // C'est tout. » — une adresse, « Continuer ». Le guichet lui-même est
  // partagé avec la section « Comptes » des Réglages (A11) :
  // GuichetCompte porte le routage par domaine (D4) et le formulaire
  // générique.
  import GuichetCompte from './GuichetCompte.svelte';

  let { onajoute = () => {} } = $props();
</script>

<div class="ecran01" data-testid="onboarding">
  <div class="colonne">
    <p class="kicker">Discovery</p>
    <h3 class="titre">Votre adresse.<br>C'est tout.</h3>
    <GuichetCompte {onajoute} />
  </div>
</div>

<style>
  /* Géométrie VERBATIM de l'écran 01 du prototype. */
  .ecran01 {
    position:absolute; inset:0; display:flex; align-items:center;
    justify-content:center; background:var(--bg); z-index:1;
  }
  .colonne { width:520px; display:flex; flex-direction:column; gap:26px; }
  .kicker {
    margin:0; font-size:12px; letter-spacing:.14em; text-transform:uppercase;
    color:var(--muted); font-weight:600;
  }
  .titre {
    margin:0; font-size:40px; line-height:1.1; font-weight:600;
    letter-spacing:-.02em; color:var(--ink);
  }
</style>
