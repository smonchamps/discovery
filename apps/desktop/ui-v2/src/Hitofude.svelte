<script>
  // Le trait hitofude (A28) — la signature du vent : un seul coup de
  // pinceau, épais puis effilé, couleur d'accent. Statique, il est
  // complètement dessiné et immobile (marque, états « À jour ») ;
  // animé (`anime`), il se trace en 2 s, reste plein 1 s, s'efface en
  // fondu — la boucle de 4 s d'un cycle de synchronisation. Les
  // keyframes vivent dans systeme.css, à côté des jetons.
  //
  // prefers-reduced-motion : la règle A8 globale coupe l'animation en
  // une frame — le dashoffset de BASE est à 0, le trait retombe donc
  // plein et immobile (A35 : le fondu ne subsiste pas).
  let { anime = false, largeur = 52, hauteur = 10 } = $props();
  const masque = `hitofude-${Math.random().toString(36).slice(2, 8)}`;
</script>

<svg width={largeur} height={hauteur} viewBox="0 0 110 20" aria-hidden="true">
  {#if anime}
    <defs>
      <mask id={masque}>
        <path class="trace" d="M2 14 Q30 4 60 7 Q88 9.5 108 3" fill="none"
              stroke="#ffffff" stroke-width="24" stroke-linecap="round"
              stroke-dasharray="115" stroke-dashoffset="0" />
      </mask>
    </defs>
    <path class="corps" d="M2 14 Q30 4 60 7 Q88 9.5 108 3 Q90 13 60 14 Q32 15 10 18 Q4 19 2 14 Z"
          fill="var(--accent)" mask="url(#{masque})" />
  {:else}
    <path d="M2 14 Q30 4 60 7 Q88 9.5 108 3 Q90 13 60 14 Q32 15 10 18 Q4 19 2 14 Z"
          fill="var(--accent)" />
  {/if}
</svg>

<style>
  svg { flex:none; }
  .trace { animation:hitofudeDraw 4s linear infinite; }
  .corps { animation:hitofudeFade 4s linear infinite; }
</style>
