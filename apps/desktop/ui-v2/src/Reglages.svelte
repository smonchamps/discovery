<script>
  // Surimpression Réglages du prototype : 560 px, les 7 thèmes en
  // rangées (pastilles ×5, libellé, description, coche sur l'actif),
  // « Terminé ». Le choix s'applique immédiatement et persiste
  // (localStorage['discovery-theme'], défaut `nature` — l'OS sombre
  // automatique est en D6, après bascule).
  import { FICHES, appliquerTheme, themeActuel } from './lib/theme.js';
  import { activation } from './lib/clavier.js';
  import GuichetCompte from './GuichetCompte.svelte';

  // A11 — la section « Comptes » : v1 offrait l'ajout à tout moment,
  // l'écran 01 ne vient qu'à zéro compte ; la porte permanente vit ici.
  let { comptes = [], onajoute = () => {} } = $props();

  let visible = $state(false);
  let actif = $state(themeActuel());
  let ajoutOuvert = $state(false);

  export function ouvrir() {
    actif = themeActuel();
    ajoutOuvert = false;
    visible = true;
  }
  export function fermer() {
    visible = false;
  }
  export function estOuverte() {
    return visible;
  }
  function choisir(id) {
    appliquerTheme(id);
    actif = id;
  }
</script>

{#if visible}
  <div class="scrim" data-testid="reglages-modal">
    <div class="carte" role="dialog" aria-modal="true" aria-label="Réglages">
      <div class="tete">
        <span class="titre">Réglages</span>
        <button type="button" class="fermer" aria-label="Fermer" onclick={fermer}>
          <span class="ms" aria-hidden="true">close</span></button>
      </div>
      <div class="corps">
        <p class="section">Thème de couleur</p>
        <div class="rangees">
          {#each FICHES as fiche (fiche.id)}
            <div class="rangee" class:active={actif === fiche.id}
                 data-testid="theme" data-theme-id={fiche.id}
                 role="button" tabindex="0" aria-pressed={actif === fiche.id}
                 onclick={() => choisir(fiche.id)}
                 onkeydown={activation(() => choisir(fiche.id))}>
              <span class="pastilles">
                {#each fiche.pastilles as couleur (couleur)}
                  <span class="pastille" style="background:{couleur}"></span>
                {/each}
              </span>
              <span class="libelles">
                <span class="nom">{fiche.label}</span>
                <span class="desc">{fiche.desc}</span>
              </span>
              {#if actif === fiche.id}
                <span class="ms coche" aria-hidden="true">check_circle</span>
              {/if}
            </div>
          {/each}
        </div>

        <p class="section">Comptes</p>
        <div class="rangees" data-testid="reglages-comptes">
          {#each comptes as c (c.account_id)}
            <div class="compte">
              <span class="ms" aria-hidden="true">person</span>
              <span class="adresse">{c.email}</span>
            </div>
          {/each}
          {#if ajoutOuvert}
            <!-- Carte signature : le guichet est un BLOC voulu, pas un
                 formulaire qui flotte (verdict terrain). Démonté au repli
                 ou au succès : il repart toujours propre. -->
            <div class="carte-ajout" data-testid="reglages-guichet">
              <div class="tete-ajout">
                <span class="titre-ajout">Ajouter un compte</span>
                <button type="button" class="fermer" aria-label="Replier"
                        onclick={() => (ajoutOuvert = false)}>
                  <span class="ms" aria-hidden="true">close</span></button>
              </div>
              <GuichetCompte compact onajoute={() => { ajoutOuvert = false; onajoute(); }} />
            </div>
          {:else}
            <button type="button" class="ajouter" data-testid="reglages-ajouter"
                    onclick={() => (ajoutOuvert = true)}>
              <span class="ms" aria-hidden="true">person_add</span>Ajouter un compte</button>
          {/if}
        </div>
      </div>
      <div class="pied">
        <button type="button" class="principal" data-testid="reglages-termine" onclick={fermer}>
          Terminé</button>
      </div>
    </div>
  </div>
{/if}

<style>
  /* Géométrie VERBATIM de la surimpression Réglages du prototype. */
  .scrim {
    position:absolute; inset:0; background:var(--scrim); z-index:2;
    display:flex; align-items:center; justify-content:center; padding:36px;
  }
  .carte {
    width:560px; max-height:100%; background:var(--surface);
    border:1px solid var(--border); border-left:2px solid var(--accent);
    border-radius:10px; box-shadow:var(--shadow);
    display:flex; flex-direction:column; overflow:hidden;
  }
  .tete {
    height:48px; flex:none; padding:0 16px 0 22px; display:flex;
    align-items:center; gap:14px; border-bottom:1px solid var(--border);
  }
  .titre { font-size:15px; font-weight:600; flex:1; color:var(--ink); }
  .fermer {
    height:32px; width:32px; padding:0; display:inline-flex;
    align-items:center; justify-content:center; color:var(--ink2);
    background:var(--surface); border:1px solid var(--border);
    border-radius:6px; cursor:pointer;
  }
  .fermer:hover { background:var(--sel); }
  .corps {
    padding:22px; display:flex; flex-direction:column; gap:14px;
    overflow:auto;
  }
  .section {
    margin:0; font-size:12px; letter-spacing:.1em; text-transform:uppercase;
    color:var(--muted); font-weight:600;
  }
  .rangees { display:flex; flex-direction:column; gap:6px; }
  .rangee {
    display:flex; align-items:center; gap:16px; padding:14px 16px;
    border-radius:10px; cursor:pointer; border:1px solid transparent;
  }
  .rangee:hover { background:var(--sel); }
  .rangee.active {
    background:var(--surface); border:1px solid var(--border);
    border-left:2px solid var(--accent); box-shadow:var(--shadow);
  }
  .rangee.active:hover { background:var(--surface); }
  .pastilles { display:flex; gap:5px; flex:none; }
  .pastille {
    width:22px; height:22px; border-radius:6px;
    border:1px solid var(--border);
  }
  .libelles {
    display:flex; flex-direction:column; gap:2px; flex:1; min-width:0;
  }
  .nom { font-size:14px; font-weight:600; color:var(--ink); }
  .desc { font-size:12px; line-height:1.4; color:var(--muted); }
  .coche { color:var(--accent); font-variation-settings:'FILL' 1; }
  .compte {
    display:flex; align-items:center; gap:12px; padding:10px 16px;
    font-size:13px; color:var(--ink2);
  }
  .compte .ms { color:var(--muted); }
  .adresse {
    color:var(--ink); overflow:hidden; text-overflow:ellipsis;
    white-space:nowrap;
  }
  .ajouter {
    height:32px; padding:0 16px; align-self:flex-start; display:inline-flex;
    align-items:center; gap:8px; font-size:13px; color:var(--ink);
    background:var(--surface); border:1px solid var(--border);
    border-radius:6px; cursor:pointer;
  }
  .ajouter:hover { background:var(--sel); }
  .carte-ajout {
    border:1px solid var(--border); border-left:2px solid var(--accent);
    border-radius:10px; padding:14px 16px 16px;
    display:flex; flex-direction:column; gap:12px;
  }
  .tete-ajout { display:flex; align-items:center; gap:14px; }
  .titre-ajout { flex:1; font-size:14px; font-weight:600; color:var(--ink); }
  .pied {
    flex:none; padding:14px 22px 18px; border-top:1px solid var(--border);
    display:flex; align-items:center;
  }
  .principal {
    height:32px; padding:0 16px; margin-left:auto; display:inline-flex;
    align-items:center; gap:8px; font-size:13px; font-weight:600;
    color:var(--onAccent); background:var(--accent);
    border:1px solid var(--accent); border-radius:6px; cursor:pointer;
  }
  .principal:hover { background:var(--accentH); border-color:var(--accentH); }
</style>
