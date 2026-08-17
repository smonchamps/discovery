<script>
  // Le trait hitofude (A28, A36, A40, A52) — la signature du vent : un
  // seul coup de pinceau, épais puis effilé, couleur d'accent. DEUX
  // états : statique (plein et immobile — marque, repos « À jour ») et
  // `anime` (la boucle de chargement, jouée dès qu'une action tourne).
  //
  // A52 : le mode « au pourcentage » est mort. Le retour terrain
  // (2026-08-17) demandait UNE animation complète dès qu'une action est
  // en cours — l'ancien tracé masqué à une longueur partielle restait
  // FIGÉ (la transition CSS ne tourne pas dans le <mask> chez Chromium,
  // A40) et paraissait cassé. Le pourcentage, quand il existe, vit
  // désormais dans le TEXTE de la ligne, jamais dans le trait.
  //
  // La boucle : fondu-in AVEC remplissage de gauche à droite, brève
  // tenue pleine, fondu-out (hitofudeFade, systeme.css). Le TRACÉ est
  // animé en SMIL (<animate>) : le chemin vit dans le <mask>, sous-arbre
  // non rendu où Chromium ne fait PAS tourner les animations CSS (A40).
  // Le fondu, lui, porte sur le chemin RENDU : il reste en CSS.
  //
  // prefers-reduced-motion : SMIL n'obéit pas au bloc CSS global (A8) —
  // l'<animate> n'est rendu que si le mouvement est permis ; sinon le
  // dashoffset de base (0) fait foi, le trait retombe plein et immobile.
  let { anime = false, largeur = 52, hauteur = 10 } = $props();
  const masque = `hitofude-${Math.random().toString(36).slice(2, 8)}`;
  const mouvementPermis =
    typeof window === 'undefined' ||
    !window.matchMedia?.('(prefers-reduced-motion: reduce)').matches;
</script>

<svg width={largeur} height={hauteur} viewBox="0 0 110 20" aria-hidden="true">
  {#if anime}
    <defs>
      <mask id={masque}>
        <path class="boucle"
              d="M2 14 Q30 4 60 7 Q88 9.5 108 3" fill="none"
              stroke="#ffffff" stroke-width="24" stroke-linecap="round"
              stroke-dasharray="115" stroke-dashoffset="0">
          {#if mouvementPermis}
            <!-- Tracé 2 s, plein 2 s — le fondu CSS masque la reprise. -->
            <animate attributeName="stroke-dashoffset" values="115;0;0"
                     keyTimes="0;0.5;1" dur="4s" repeatCount="indefinite" />
          {/if}
        </path>
      </mask>
    </defs>
    <path class:fondu={mouvementPermis}
          d="M2 14 Q30 4 60 7 Q88 9.5 108 3 Q90 13 60 14 Q32 15 10 18 Q4 19 2 14 Z"
          fill="var(--accent)" mask="url(#{masque})" />
  {:else}
    <path d="M2 14 Q30 4 60 7 Q88 9.5 108 3 Q90 13 60 14 Q32 15 10 18 Q4 19 2 14 Z"
          fill="var(--accent)" />
  {/if}
</svg>

<style>
  svg { flex:none; }
  .fondu { animation:hitofudeFade 4s linear infinite; }
</style>
