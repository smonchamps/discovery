<script>
  // La modale de migration (ADR 0012, dû de bascule §6) : exclusive et
  // bloquante au démarrage. Sans elle, la PREMIÈRE commande venue
  // paierait l'adoption d'une base héritée en silence, dans un gel
  // d'interface. « Annuler » défait tout — la passe entière se rejouera
  // au prochain lancement, ou tout de suite par « Reprendre ».
  //
  // `assurer()` ne rend la main qu'une fois la base migrée (ou s'il n'y
  // avait rien à faire) : l'App n'ouvre rien avant. Elle rend `true`
  // quand la base est CONFIRMÉE claire ; `false` quand la sonde n'a pas
  // pu répondre — l'App s'interdit alors toute écriture facultative
  // (la pose de la langue détectée), qui serait la première ouverture
  // pleine, l'adoption silencieuse même (A41).
  import { appel } from './lib/transport.js';
  import { t } from './lib/texte.svelte.js';

  let visible = $state(false);
  let note = $state('');
  let pourcent = $state(null); // null = préparation, jamais « 0 % »
  let bilan = $state('');
  let annulable = $state(true);
  let reprise = $state(false);
  let resoudreReprise = null;

  export async function assurer() {
    let sonde;
    try {
      sonde = await appel('migration_check');
    } catch {
      // Sonde impossible : l'ouverture normale le dira mieux qu'un écran
      // sans objet — on ne bloque pas le démarrage.
      return false;
    }
    if (sonde.pending === null || sonde.pending === undefined) return true;
    note = t('migration.note', { n: sonde.pending });
    visible = true;
    while (!(await unePasse())) {
      reprise = true;
      await new Promise((resolve) => { resoudreReprise = resolve; });
      reprise = false;
    }
    visible = false;
    return true;
  }

  async function unePasse() {
    bilan = '';
    pourcent = null;
    annulable = true;
    const sondage = setInterval(async () => {
      try {
        const avancement = await appel('migration_progress');
        pourcent = avancement.percent ?? null;
      } catch { /* le prochain relevé suffira */ }
    }, 300);
    try {
      const migree = await appel('migration_run');
      if (migree) return true;
      bilan = t('migration.annulee');
    } catch (err) {
      bilan = t('migration.echec', { err });
    } finally {
      clearInterval(sondage);
    }
    return false;
  }

  function annuler() {
    // Un seul clic suffit : la passe annule à son prochain palier.
    annulable = false;
    appel('migration_cancel').catch(() => {});
  }
</script>

{#if visible}
  <div class="scrim" data-testid="migration-modale">
    <div class="carte" role="dialog" aria-modal="true"
         aria-label={t('migration.aria')}>
      <p class="kicker">Wind</p>
      <h3 class="titre">{t('migration.titre')}</h3>
      <p class="note">{note}</p>
      {#if !bilan}
        <div class="jauge" class:indeterminee={pourcent === null}>
          <div class="remplie" style="width:{pourcent ?? 0}%"></div>
        </div>
        <p class="etat" data-testid="migration-etat">
          {pourcent === null ? t('migration.preparation') : t('migration.pourcent', { p: pourcent })}</p>
        <button type="button" disabled={!annulable} onclick={annuler}>{t('action.annuler')}</button>
      {:else}
        <p class="etat">{bilan}</p>
        {#if reprise}
          <button type="button" class="principal"
                  onclick={() => resoudreReprise?.()}>{t('action.reprendre')}</button>
        {/if}
      {/if}
    </div>
  </div>
{/if}

<style>
  /* La géométrie de l'écran 01 (colonne 520 px), la modale étant muette
     au prototype : le Système complète, sans invention de forme. */
  .scrim {
    position:absolute; inset:0; background:var(--bg); z-index:4;
    display:flex; align-items:center; justify-content:center;
  }
  .carte { width:520px; display:flex; flex-direction:column; gap:18px; }
  .kicker {
    margin:0; font-size:12px; letter-spacing:.14em; text-transform:uppercase;
    color:var(--muted); font-weight:600;
  }
  .titre {
    margin:0; font-size:32px; line-height:1.15; font-weight:600;
    letter-spacing:-.02em; color:var(--ink);
  }
  .note, .etat { margin:0; font-size:13px; line-height:1.5; color:var(--muted); }
  .jauge {
    height:6px; background:var(--panel); border:1px solid var(--border);
    border-radius:6px; overflow:hidden;
  }
  .remplie { height:100%; background:var(--accent); transition:width .3s; }
  .indeterminee .remplie { width:30%; animation:va-et-vient 1.2s ease-in-out infinite alternate; }
  @keyframes va-et-vient { from { margin-left:0; } to { margin-left:70%; } }
  button {
    height:32px; padding:0 16px; align-self:flex-start; display:inline-flex;
    align-items:center; gap:8px; font-size:13px; color:var(--ink);
    background:var(--surface); border:1px solid var(--border);
    border-radius:6px; cursor:pointer;
  }
  button:hover { background:var(--sel); }
  button:disabled { opacity:.6; cursor:default; }
  .principal {
    font-weight:600; color:var(--onAccent); background:var(--accent);
    border-color:var(--accent);
  }
  .principal:hover { background:var(--accentH); border-color:var(--accentH); }
</style>
