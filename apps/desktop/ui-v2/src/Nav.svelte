<script>
  // Nav 236 px de l'écran 02 — géométrie et états VERBATIM du prototype
  // (navItems / mailboxes du template). Les six dossiers canoniques,
  // puis « Boîtes » : Toutes les boîtes + un rang par compte RÉEL — la
  // fiction « Travail / Personnel » n'existe pas ; icône `person` par
  // défaut (décision D7), libellé = adresse du compte.
  import { activation } from './lib/clavier.js';
  import { t } from './lib/texte.svelte.js';

  let { comptes = [], categorie, compte, onchoisir = () => {} } = $props();

  const somme = (champ) => comptes.reduce((n, c) => n + c[champ], 0);

  // Le filtre de compte borne les compteurs des dossiers, comme au
  // prototype où changer de Boîte re-filtre la liste.
  const vue = $derived(
    compte === null ? null : comptes.find((c) => c.account_id === compte),
  );
  const de = (champ) => (vue ? vue[champ] : somme(champ));

  const dossiers = $derived([
    {
      id: 'reception', icone: 'inbox', libelle: t('boite.reception'),
      heros: de('reception_non_lues'), total: de('reception_total'),
    },
    { id: 'envoyes', icone: 'send', libelle: t('boite.envoyes'), simple: de('envoyes') },
    { id: 'brouillons', icone: 'edit_note', libelle: t('boite.brouillons'), simple: de('brouillons') },
    {
      id: 'indesirables', icone: 'report', libelle: t('boite.indesirables'),
      heros: de('indesirables_non_lus'), total: de('indesirables_total'),
    },
    { id: 'archives', icone: 'archive', libelle: t('boite.archives'), simple: de('archives') },
    { id: 'corbeille', icone: 'delete', libelle: t('boite.corbeille'), simple: de('corbeille') },
  ]);

  const boites = $derived([
    {
      id: null, icone: 'all_inbox', libelle: t('nav.toutes'),
      nonLues: somme('reception_non_lues'),
    },
    ...comptes.map((c) => ({
      id: c.account_id, icone: 'person', libelle: c.email,
      nonLues: c.reception_non_lues,
    })),
  ]);
</script>

<nav aria-label={t('nav.aria')} data-testid="nav">
  {#each dossiers as d (d.id)}
    <div class="rang" class:actif={categorie === d.id}
         data-testid="nav-dossier" data-categorie={d.id}
         role="button" tabindex="0" aria-current={categorie === d.id}
         onclick={() => onchoisir({ categorie: d.id })}
         onkeydown={activation(() => onchoisir({ categorie: d.id }))}>
      <span class="ms icone" aria-hidden="true">{d.icone}</span>
      <span class="libelle">{d.libelle}</span>
      {#if d.heros !== undefined}
        <span class="heros">{d.heros}</span>
        <span class="total">/ {d.total}</span>
      {:else}
        <span class="total">{d.simple}</span>
      {/if}
    </div>
  {/each}

  <div class="boites">
    <p class="titre">{t('nav.boites')}</p>
    {#each boites as b (b.id)}
      <div class="rang" class:actif={compte === b.id}
           data-testid="nav-boite"
           role="button" tabindex="0" aria-current={compte === b.id}
           onclick={() => onchoisir({ compte: b.id })}
           onkeydown={activation(() => onchoisir({ compte: b.id }))}>
        <span class="ms icone" aria-hidden="true">{b.icone}</span>
        <span class="libelle">{b.libelle}</span>
        <span class="heros">{b.nonLues}</span>
      </div>
    {/each}
  </div>
</nav>

<style>
  nav {
    background:var(--panel); border-right:1px solid var(--border);
    padding:20px 16px; display:flex; flex-direction:column; gap:4px;
    min-height:0; overflow:auto;
  }
  .rang {
    display:flex; align-items:center; gap:10px; height:36px; flex:none;
    padding:0 12px; border-radius:6px; cursor:pointer;
    border:1px solid transparent;
  }
  .rang:hover { background:var(--sel); border-color:var(--border); }
  .rang.actif {
    background:var(--surface); border-color:var(--border);
    border-left:2px solid var(--accent); box-shadow:var(--shadow);
  }
  .icone { color:var(--muted); }
  .actif .icone {
    color:var(--accent); font-variation-settings:'FILL' 1, 'wght' 600;
  }
  .libelle {
    font-size:13px; color:var(--ink2); flex:1; min-width:0;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
  }
  .actif .libelle { font-weight:600; color:var(--ink); }
  .heros {
    font-size:15px; font-weight:600; color:var(--accent);
    font-variant-numeric:tabular-nums;
  }
  .total {
    font-size:12px; color:var(--muted); font-variant-numeric:tabular-nums;
  }
  .boites {
    margin-top:auto; padding-top:16px; border-top:1px solid var(--border);
    display:flex; flex-direction:column; gap:4px;
  }
  .titre {
    margin:0 0 6px; padding:0 12px; font-size:12px; letter-spacing:.1em;
    text-transform:uppercase; color:var(--muted); font-weight:600;
  }
</style>
