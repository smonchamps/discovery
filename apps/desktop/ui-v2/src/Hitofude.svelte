<script>
  // Le trait hitofude (A28, A36) — la signature du vent : un seul coup
  // de pinceau, épais puis effilé, couleur d'accent. Trois états :
  // statique (plein et immobile — marque, repos « À jour »), `anime`
  // (il se trace en 2 s, reste plein 1 s, s'efface en fondu — la
  // boucle de 4 s d'un cycle sans dénominateur), `progression` (0-100 :
  // il se dessine au rythme du pourcentage — l'indicateur de
  // progression de la barre d'état, la barre fine de 2 px est morte à
  // A36). Les keyframes vivent dans systeme.css, à côté des jetons.
  //
  // prefers-reduced-motion : la règle A8 globale coupe animation et
  // transition en une frame — le dashoffset de BASE est la valeur
  // finale, le trait retombe plein (ou au pourcentage) et immobile.
  let { anime = false, progression = null, largeur = 52, hauteur = 10 } = $props();
  const masque = `hitofude-${Math.random().toString(36).slice(2, 8)}`;

  // La course du masque est de 115 unités : 0 = plein, 115 = rien.
  const offset = $derived(
    anime || progression === null
      ? 0
      : 115 - (115 * Math.max(0, Math.min(100, progression))) / 100,
  );
</script>

<svg width={largeur} height={hauteur} viewBox="0 0 110 20" aria-hidden="true">
  {#if anime || progression !== null}
    <defs>
      <mask id={masque}>
        <path class="trace" class:boucle={anime}
              d="M2 14 Q30 4 60 7 Q88 9.5 108 3" fill="none"
              stroke="#ffffff" stroke-width="24" stroke-linecap="round"
              stroke-dasharray="115" stroke-dashoffset={offset} />
      </mask>
    </defs>
    <path class:fondu={anime}
          d="M2 14 Q30 4 60 7 Q88 9.5 108 3 Q90 13 60 14 Q32 15 10 18 Q4 19 2 14 Z"
          fill="var(--accent)" mask="url(#{masque})" />
  {:else}
    <path d="M2 14 Q30 4 60 7 Q88 9.5 108 3 Q90 13 60 14 Q32 15 10 18 Q4 19 2 14 Z"
          fill="var(--accent)" />
  {/if}
</svg>

<style>
  svg { flex:none; }
  /* Le pourcentage avance par sauts : la transition lisse le tracé —
     le même geste que l'ancienne barre fine (width .3s). */
  .trace { transition:stroke-dashoffset .3s; }
  .boucle { animation:hitofudeDraw 4s linear infinite; }
  .fondu { animation:hitofudeFade 4s linear infinite; }
</style>
