<script>
  // Coquille P1 — liste + lecture minimale, de quoi mesurer. Ni nav, ni
  // entête, ni onglets : refus de périmètre du plan (P1), ils arrivent
  // en P2 avec l'écran 02 complet.
  import Liste from './Liste.svelte';
  import Lecture from './Lecture.svelte';

  let liste;
  let lecture;
  let perf = $state('démarrage…');
  let startup = $state('');

  function surSelection(ligne) {
    lecture.ouvrir(ligne);
  }

  export function api() {
    return { liste, lecture };
  }
  export function marquerDemarrage() {
    const l = liste.etat();
    startup = String(Math.round(performance.now()));
    perf = `${l.total} conversations — première page servie+rendue en ${l.premierePageMs.toFixed(1)} ms`;
  }
</script>

<div class="coquille">
  <Liste bind:this={liste} onselect={surSelection} />
  <Lecture bind:this={lecture} />
</div>
<footer id="perf" data-testid="perf" data-startup={startup}>{perf}</footer>

<style>
  .coquille {
    display:grid; grid-template-columns:400px minmax(0,1fr);
    height:calc(100vh - 20px);
  }
  .coquille > :global(.cadre) { border-right:1px solid var(--border); }
  #perf {
    height:20px; display:flex; align-items:center; padding:0 12px;
    font-size:12px; color:var(--muted); background:var(--panel);
    border-top:1px solid var(--border);
  }
</style>
