<script>
  // Liste fenêtrée de l'écran 02 — la ligne EXACTE du prototype (A6), à
  // deux gabarits déterministes (voir P1), servie par `list_category` :
  // la source est (catégorie, compte, non-lus), les onglets du prototype
  // vivent dans le pied de cette colonne.
  //
  // Changement de source = nouvelle génération : les pages en vol de la
  // source précédente sont jetées à l'arrivée, jamais mélangées.
  import { tick, untrack } from 'svelte';
  import { appel } from './lib/transport.js';
  import { quand } from './lib/quand.js';
  import { activation } from './lib/clavier.js';

  let {
    categorie = 'reception',
    compte = null,
    onglet = 'tous',
    recherche = '',
    onselect = () => {},
    ononglet = () => {},
    ontotal = () => {},
    onresultats = () => {},
  } = $props();

  const PAGE = 200;
  const GAP = 8;
  const PAD = 12;
  const OVER = 8;

  let cadre = $state(null);
  let total = $state(0);
  let premier = $state(0);
  let version = $state(0);
  let h1 = $state(98);
  let h2 = $state(132);
  let sondees = $state(false);
  let selection = $state(null);
  let premierePageMs = $state(null);

  let generation = 0;
  let pages = new Map();
  let chipsParPage = new Map();
  let pending = new Map();

  const aPuces = (l) => l.thread_size > 1 || l.has_attachment;
  const pitch1 = $derived(h1 + GAP);
  const extraPuce = $derived(h2 - h1);

  function chipsAvant(i) {
    let extra = 0;
    const pleine = Math.floor(i / PAGE);
    for (const [p, n] of chipsParPage) {
      if (p < pleine) extra += n;
    }
    const page = pages.get(pleine);
    if (page) {
      const borne = i - pleine * PAGE;
      for (let k = 0; k < borne && k < page.length; k++) {
        if (aPuces(page[k])) extra += 1;
      }
    }
    return extra;
  }
  function decalage(i) {
    return PAD + i * pitch1 + chipsAvant(i) * extraPuce;
  }

  const hauteurEspace = $derived.by(() => {
    void version;
    if (total === 0) return 0;
    let extra = 0;
    for (const n of chipsParPage.values()) extra += n;
    return PAD * 2 + total * pitch1 - GAP + extra * extraPuce;
  });

  function indexPour(scrollTop) {
    let i = Math.max(0, Math.floor((scrollTop - PAD) / pitch1));
    for (let tour = 0; tour < 4; tour++) {
      const corrige = Math.max(
        0,
        Math.floor((scrollTop - PAD - chipsAvant(i) * extraPuce) / pitch1),
      );
      if (corrige === i) break;
      i = corrige;
    }
    return Math.min(i, Math.max(0, total - 1));
  }

  function servirPage(p) {
    if (pages.has(p) || pending.has(p)) return pending.get(p) || Promise.resolve();
    const nee = generation;
    const t0 = performance.now();
    const promesse = appel('list_category', {
      category: categorie,
      accountId: compte,
      nonLus: onglet === 'nonlus',
      offset: p * PAGE,
      limit: PAGE,
    })
      .then(async (page) => {
        if (nee !== generation) return; // source changée : page périmée
        total = page.total;
        pages.set(p, page.rows);
        let n = 0;
        for (const l of page.rows) if (aPuces(l)) n += 1;
        chipsParPage.set(p, n);
        pending.delete(p);
        if (premierePageMs === null) premierePageMs = performance.now() - t0;
        if (n > 0 && (p + 1) * PAGE <= premier && cadre) {
          version += 1;
          await tick();
          cadre.scrollTop += n * extraPuce;
        } else {
          version += 1;
        }
      })
      .catch((err) => {
        pending.delete(p);
        console.error(`list_category ${categorie} page ${p} :`, err);
      });
    pending.set(p, promesse);
    return promesse;
  }

  // Nouvelle source -> repartir du haut, tout jeter, resservir. SEULE la
  // clé de source est une dépendance : tout le reste est sous `untrack`,
  // sans quoi l'effet dépendrait de ce qu'il modifie (boucle).
  $effect(() => {
    void categorie;
    void compte;
    void onglet;
    untrack(() => {
      generation += 1;
      pages = new Map();
      chipsParPage = new Map();
      pending = new Map();
      total = 0;
      premier = 0;
      selection = null;
      if (cadre) cadre.scrollTop = 0;
      version += 1;
      servirPage(0);
    });
  });

  const visibles = $derived(
    cadre ? Math.ceil(cadre.clientHeight / pitch1) + 1 : 12,
  );
  const debut = $derived(Math.max(0, premier - OVER));
  const fin = $derived(Math.min(total, premier + visibles + OVER));

  const fenetre = $derived.by(() => {
    void version;
    const arr = [];
    for (let i = debut; i < fin; i++) {
      const page = pages.get(Math.floor(i / PAGE));
      arr.push({ i, ligne: page ? page[i % PAGE] : null });
    }
    return arr;
  });

  $effect(() => {
    const de = Math.floor(debut / PAGE);
    const a = Math.floor(Math.max(0, fin - 1) / PAGE);
    untrack(() => {
      for (let p = de; p <= a; p++) servirPage(p);
    });
  });

  function surDefilement() {
    premier = indexPour(cadre.scrollTop);
  }

  // D1 — recherche (FTS5, `search_messages`) : les résultats prennent la
  // place de la liste, AUX LIGNES MÊMES du prototype — aucune UI neuve.
  // Bornés côté coeur : pas de fenêtrage. En deçà de 3 caractères, la
  // boîte revient telle quelle.
  let resultats = $state(null);
  let minuterieRecherche;
  let jetonRecherche = 0;
  $effect(() => {
    const q = recherche.trim();
    untrack(() => {
      clearTimeout(minuterieRecherche);
      const mien = ++jetonRecherche;
      if (q.length < 3) {
        resultats = null;
        onresultats(null);
        return;
      }
      minuterieRecherche = setTimeout(async () => {
        try {
          const lignes = await appel('search_messages', { query: q });
          if (mien !== jetonRecherche) return; // frappe plus récente
          resultats = lignes;
          onresultats(lignes.length);
        } catch (err) {
          console.error('search_messages :', err);
        }
      }, 150);
    });
  });

  $effect(() => {
    const t = total;
    untrack(() => ontotal(t));
  });

  function sonder(el, avecPuces) {
    const h = el.offsetHeight;
    if (avecPuces) h2 = h;
    else h1 = h;
    sondees = true;
  }

  const cle = (l) => `${l.account_id}/${l.mailbox}/${l.uid}`;
  function choisir(l) {
    selection = cle(l);
    onselect(l);
  }
  const estChoisie = (l) => selection === cle(l);

  const ONGLETS = [
    { id: 'tous', icone: 'inbox', libelle: 'Tous' },
    { id: 'nonlus', icone: 'mark_email_unread', libelle: 'Non lus' },
    { id: 'brouillons', icone: 'edit_note', libelle: 'Brouillons' },
  ];
  const ongletActif = $derived(categorie === 'brouillons' ? 'brouillons' : onglet);

  // --- API (App, banc P1, e2e) ---------------------------------------
  export function aller(index) {
    cadre.scrollTop = decalage(index) - PAD;
    surDefilement();
  }
  export async function allerEtServir(index) {
    const t0 = performance.now();
    aller(index);
    const de = Math.floor(Math.max(0, index - OVER) / PAGE);
    const a = Math.floor(Math.min(Math.max(0, total - 1), index + visibles + OVER) / PAGE);
    const attentes = [];
    for (let p = de; p <= a; p++) attentes.push(servirPage(p));
    await Promise.all(attentes);
    await tick();
    void cadre.offsetHeight;
    return performance.now() - t0;
  }
  export function etat() {
    return { total, premier, h1, h2, premierePageMs };
  }
  export function ligneA(index) {
    const page = pages.get(Math.floor(index / PAGE));
    return page ? page[index % PAGE] : null;
  }
  export function marquerLue(ligne) {
    const id = cle(ligne);
    for (const page of pages.values()) {
      for (const l of page) {
        if (cle(l) === id) l.thread_unseen = 0;
      }
    }
    version += 1;
  }
  export function recharger() {
    generation += 1;
    pages = new Map();
    chipsParPage = new Map();
    pending = new Map();
    version += 1;
    servirPage(Math.floor(premier / PAGE));
    servirPage(0);
  }
</script>

<section class="colonne" aria-label="Liste des messages" data-testid="liste">
  <div class="cadre" bind:this={cadre} onscroll={surDefilement}>
    {#if !sondees}
      <div class="sondes" aria-hidden="true">
        <article class="ligne" use:sonder={false}>
          <div class="l1"><span class="exp">Sonde</span><span class="heure">00:00</span></div>
          <p class="objet">Sonde</p>
          <p class="apercu">Sonde</p>
        </article>
        <article class="ligne" use:sonder={true}>
          <div class="l1"><span class="exp">Sonde</span><span class="heure">00:00</span></div>
          <p class="objet">Sonde</p>
          <p class="apercu">Sonde</p>
          <span class="puces"><span class="puce"><span class="ms" aria-hidden="true">forum</span>3 messages</span></span>
        </article>
      </div>
    {/if}
    {#snippet rangee(ligne)}
      <div class="ligne"
           class:nonlu={ligne.thread_unseen > 0}
           class:choisie={estChoisie(ligne)}
           data-testid="ligne"
           role="button" tabindex="0"
           onclick={() => choisir(ligne)}
           onkeydown={activation(() => choisir(ligne))}>
        <div class="l1">
          <span class="exp">{ligne.sender}</span>
          <span class="heure">{quand(ligne.epoch)}</span>
        </div>
        <p class="objet">{ligne.subject}</p>
        <p class="apercu">{ligne.preview ?? ''}</p>
        {#if aPuces(ligne)}
          <span class="puces">
            {#if ligne.thread_size > 1}
              <span class="puce"><span class="ms" aria-hidden="true">forum</span>{ligne.thread_size} messages</span>
            {/if}
            {#if ligne.attachment_count > 0}
              <span class="puce"><span class="ms" aria-hidden="true">attach_file</span>{ligne.attachment_count} fichier{ligne.attachment_count > 1 ? 's' : ''}</span>
            {/if}
          </span>
        {/if}
      </div>
    {/snippet}
    {#if resultats !== null}
      <div class="fenetre-recherche" data-testid="resultats">
        {#if resultats.length === 0}
          <div class="vide-recherche"><p>Aucun résultat.</p></div>
        {/if}
        {#each resultats as ligne (`${ligne.account_id}/${ligne.mailbox}/${ligne.uid}`)}
          {@render rangee(ligne)}
        {/each}
      </div>
    {:else}
      {#if total === 0 && premierePageMs !== null}
        <div class="vide"><p>Aucun message ici.</p></div>
      {/if}
      <div class="espace" style="height:{hauteurEspace}px">
        <div class="fenetre" style="transform:translateY({decalage(debut)}px)">
          {#each fenetre as { i, ligne } (i)}
            {#if ligne}
              {@render rangee(ligne)}
            {:else}
              <article class="ligne attente" data-testid="ligne-attente">
                <div class="l1"><span class="exp">…</span><span class="heure"></span></div>
                <p class="objet">…</p>
                <p class="apercu"></p>
              </article>
            {/if}
          {/each}
        </div>
      </div>
    {/if}
  </div>
  <div class="onglets" data-testid="onglets">
    {#each ONGLETS as o (o.id)}
      <span class="onglet" class:actif={ongletActif === o.id}
            data-testid="onglet" data-onglet={o.id}
            role="button" tabindex="0" aria-pressed={ongletActif === o.id}
            onclick={() => ononglet(o.id)}
            onkeydown={activation(() => ononglet(o.id))}>
        <span class="ms" aria-hidden="true">{o.icone}</span>{o.libelle}
      </span>
    {/each}
  </div>
</section>

<style>
  /* Géométrie et états VERBATIM du prototype (écran 02). */
  .colonne {
    display:flex; flex-direction:column; min-height:0;
    background:var(--bg); border-right:1px solid var(--border);
  }
  .cadre { flex:1; overflow:auto; position:relative; }
  .espace { position:relative; }
  .fenetre {
    position:absolute; top:0; left:12px; right:12px;
    display:flex; flex-direction:column; gap:8px;
  }
  .sondes { position:absolute; visibility:hidden; left:12px; right:12px; }
  .vide {
    position:absolute; inset:0; display:flex; align-items:center;
    justify-content:center; padding:40px; text-align:center;
  }
  .vide p { margin:0; font-size:13px; line-height:1.5; color:var(--muted); }
  .fenetre-recherche {
    padding:12px; display:flex; flex-direction:column; gap:8px;
  }
  .vide-recherche { padding:40px; text-align:center; }
  .vide-recherche p { margin:0; font-size:13px; line-height:1.5; color:var(--muted); }

  .ligne {
    padding:14px 16px; border-radius:10px; border:1px solid transparent;
    display:flex; flex-direction:column; gap:6px; cursor:pointer;
  }
  .ligne:hover { background:var(--sel); border-color:var(--border); }
  .ligne.choisie {
    background:var(--surface); border-color:var(--border);
    border-left:2px solid var(--accent); box-shadow:var(--shadow);
  }
  .l1 { display:flex; align-items:baseline; gap:12px; }
  .exp { font-size:13px; color:var(--ink2); flex:1; }
  .nonlu .exp { font-weight:600; color:var(--ink); }
  .heure { font-size:12px; color:var(--muted); }
  .objet {
    margin:0; font-size:18px; font-weight:600; line-height:1.3;
    letter-spacing:-.01em; color:var(--ink2);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
  }
  .nonlu .objet { color:var(--ink); }
  .apercu {
    margin:0; font-size:13px; line-height:1.45; color:var(--muted);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
    min-height:1.45em;
  }
  .nonlu .apercu { color:var(--ink2); }
  .puces { display:flex; gap:8px; margin-top:2px; }
  .puce {
    height:32px; padding:0 12px; display:inline-flex; align-items:center;
    gap:8px; font-size:13px; color:var(--ink2); background:var(--surface);
    border:1px solid var(--border); border-radius:6px; white-space:nowrap;
  }
  .attente { color:var(--muted); }

  .onglets {
    flex:none; height:52px; padding:0 12px; display:flex;
    align-items:center; gap:10px; border-top:1px solid var(--border);
    background:var(--panel);
  }
  .onglet {
    height:32px; padding:0 14px; display:inline-flex; align-items:center;
    gap:8px; font-size:13px; border-radius:6px; cursor:pointer;
    color:var(--ink2); background:var(--surface);
    border:1px solid var(--border);
  }
  .onglet:hover { background:var(--sel); }
  .onglet.actif {
    font-weight:600; color:var(--ink); background:var(--sel);
    border-color:var(--accent);
  }
</style>
