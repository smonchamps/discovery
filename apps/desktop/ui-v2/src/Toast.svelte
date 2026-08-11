<script>
  // Toast du prototype : bas centré, signature, coche pleine.
  let { message = null } = $props();
</script>

{#if message}
  <div class="toast" data-testid="toast">
    <span class="ms coche" aria-hidden="true">check_circle</span>
    <span class="texte">{message}</span>
  </div>
{/if}

<style>
  .toast {
    position:absolute; left:50%; bottom:28px; transform:translateX(-50%);
    background:var(--surface); border:1px solid var(--border);
    border-left:2px solid var(--accent); border-radius:6px;
    box-shadow:var(--shadow); padding:10px 16px;
    display:flex; align-items:center; gap:10px;
  }
  .coche { color:var(--accent); font-variation-settings:'FILL' 1; }
  .texte { font-size:13px; color:var(--ink); }
</style>
