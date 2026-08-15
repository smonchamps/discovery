<script>
  // Le trait hitofude (A28, A36, A40) — la signature du vent : un seul
  // coup de pinceau, épais puis effilé, couleur d'accent. Trois états :
  // statique (plein et immobile — marque, repos « À jour »), `anime`
  // (il se trace en 2 s, reste plein, s'efface en fondu — la boucle de
  // 4 s d'un cycle sans dénominateur), `progression` (0-100 : il se
  // dessine au rythme du pourcentage — l'indicateur de progression de
  // la barre d'état, la barre fine de 2 px est morte à A36).
  //
  // Le TRACÉ de la boucle est animé en SMIL (<animate>), pas en CSS :
  // le chemin vit dans le <mask>, sous-arbre non rendu, et Chromium n'y
  // fait PAS tourner les animations CSS — la boucle était morte-née,
  // playState `idle`, prouvé sur la vraie fenêtre (terrain 2026-08-15,
  // PLAN-GELS/A40). Le fondu, lui, porte sur le chemin RENDU : il reste
  // en CSS (keyframes hitofudeFade, systeme.css).
  //
  // prefers-reduced-motion : SMIL n'obéit pas au bloc CSS global (A8) —
  // l'<animate> n'est rendu que si le mouvement est permis ; sinon le
  // dashoffset de base fait foi : 0, le trait retombe plein (ou au
  // pourcentage) et immobile.
  let { anime = false, progression = null, largeur = 52, hauteur = 10 } = $props();
  const masque = `hitofude-${Math.random().toString(36).slice(2, 8)}`;
  const mouvementPermis =
    typeof window === 'undefined' ||
    !window.matchMedia?.('(prefers-reduced-motion: reduce)').matches;

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
              stroke-dasharray="115" stroke-dashoffset={offset}>
          {#if anime && mouvementPermis}
            <!-- Tracé 2 s, plein 2 s — le fondu CSS masque la reprise. -->
            <animate attributeName="stroke-dashoffset" values="115;0;0"
                     keyTimes="0;0.5;1" dur="4s" repeatCount="indefinite" />
          {/if}
        </path>
      </mask>
    </defs>
    <path class:fondu={anime && mouvementPermis}
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
     le même geste que l'ancienne barre fine (width .3s). (Dans le mask
     elle ne joue pas chez Chromium — le tracé avance alors par sauts,
     ce qui reste juste.) */
  .trace { transition:stroke-dashoffset .3s; }
  .fondu { animation:hitofudeFade 4s linear infinite; }
</style>
