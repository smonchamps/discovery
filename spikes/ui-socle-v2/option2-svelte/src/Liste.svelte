<script>
  // Option ② — même liste virtualisée, en Svelte 5 (runes). Le fenêtrage
  // est identique à ① ; seule change la manière de produire le DOM (Svelte
  // patche, on force la synchro par flushSync côté appelant).
  import { TOTAL, enveloppe } from '../../commun/donnees.js';

  const ROW = 112;
  const OVER = 8;
  const VISIBLE = Math.ceil((globalThis.innerHeight || 1000) / ROW);

  let premier = $state(0);
  const debut = $derived(Math.max(0, premier - OVER));
  const fin = $derived(Math.min(TOTAL, premier + VISIBLE + OVER));
  const fenetre = $derived.by(() => {
    const arr = [];
    for (let i = debut; i < fin; i++) arr.push({ i, ...enveloppe(i) });
    return arr;
  });

  export function aller(index) { premier = index; }
</script>

<div class="espace" style="height:{TOTAL * ROW}px">
  <div class="rows" style="transform:translateY({debut * ROW}px)">
    {#each fenetre as env (env.i)}
      <article class="ligne {env.nonlu ? 'nonlu' : ''}">
        <div class="rangee-exp">
          <span class="exp">{env.exp}</span>
          <span class="heure">{env.heure}</span>
        </div>
        <h3 class="objet">{env.objet}</h3>
        <p class="apercu">{env.apercu}</p>
        {#if env.messages || env.fichiers}
          <div class="puces">
            {#if env.messages}<span class="puce">{env.messages} messages</span>{/if}
            {#if env.fichiers}<span class="puce">{env.fichiers} fichiers</span>{/if}
          </div>
        {/if}
      </article>
    {/each}
  </div>
</div>
