<script>
  // La fente d'avis (PLAN-UI-V2 §6, région A4) : en haut, AU PLUS UN
  // avis à la fois, par priorité décroissante — échec d'envoi > mise à
  // jour > crash > télémétrie > brouillons. Le style est la signature
  // Clarity ; l'échec d'envoi porte la bordure d'alerte, c'est le seul.
  //
  // L'App possède les sources et fournit l'avis élu ; cette surface ne
  // fait qu'afficher et rapporter les décisions. « Plus tard » n'efface
  // que pour la session — l'avis suivant prend la place.
  let { avis = null } = $props();
</script>

{#if avis}
  <div class="fente" class:alerte={avis.alerte} data-testid="fente-avis">
    <span class="ms icone" aria-hidden="true">{avis.icone ?? 'info'}</span>
    <span class="texte" data-testid="avis-texte">{avis.texte}</span>
    {#each avis.actions as action (action.libelle)}
      <button type="button" class:principal={action.principale}
              disabled={action.desactivee}
              onclick={action.faire}>{action.libelle}</button>
    {/each}
  </div>
{/if}

<style>
  .fente {
    flex:none; background:var(--surface);
    border-bottom:1px solid var(--border);
    border-left:2px solid var(--accent);
    display:flex; align-items:center; gap:14px; padding:10px 24px;
  }
  .fente.alerte { border-left-color:var(--alert); }
  .icone { color:var(--accent); font-variation-settings:'FILL' 1; }
  .alerte .icone { color:var(--alert); }
  .texte { flex:1; font-size:13px; color:var(--ink); min-width:0; }
  button {
    height:32px; padding:0 16px; display:inline-flex; align-items:center;
    gap:8px; font-size:13px; color:var(--ink); background:var(--surface);
    border:1px solid var(--border); border-radius:6px; cursor:pointer;
    flex:none;
  }
  button:hover { background:var(--sel); }
  button:disabled { opacity:.6; cursor:default; }
  .principal {
    font-weight:600; color:var(--onAccent); background:var(--accent);
    border-color:var(--accent);
  }
  .principal:hover { background:var(--accentH); border-color:var(--accentH); }
</style>
