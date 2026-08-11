<script>
  // Liste fenêtrée — la ligne EXACTE du prototype (A6), à hauteur
  // variable : une ligne porte des puces (fil > 1 ou pièce jointe) ou
  // n'en porte pas. Deux gabarits déterministes depuis les données,
  // mesurés UNE fois au montage sur des lignes sondes — jamais de mesure
  // par ligne au défilement, les décalages restent en O(pages).
  //
  // Les pages non servies sont supposées sans puces ; quand une page
  // arrive avec des puces AU-DESSUS de la fenêtre, le défilement est
  // compensé du même delta (ancrage) — rien ne bouge à l'écran.
  import { tick } from 'svelte';
  import { appel } from './lib/transport.js';

  let { onselect = () => {} } = $props();

  const PAGE = 200;
  const GAP = 8;
  const PAD = 12;
  const OVER = 8;

  let cadre = $state(null);     // l'élément défilable ($state : `visibles` en dépend)
  let total = $state(0);
  let premier = $state(0);
  let version = $state(0);      // bump à chaque page servie
  let h1 = $state(98);          // gabarit sans puces (estimation avant sonde)
  let h2 = $state(132);         // gabarit avec puces
  let sondees = $state(false);
  let selection = $state(null); // (account_id, mailbox, uid)
  let premierePageMs = $state(null);

  const pages = new Map();      // index de page -> lignes
  const chipsParPage = new Map();// index de page -> nombre de lignes à puces
  const pending = new Map();    // index de page -> Promise

  const aPuces = (l) => l.thread_size > 1 || l.has_attachment;
  const pitch1 = $derived(h1 + GAP);
  const extraPuce = $derived(h2 - h1);

  // Décalage exact d'une ligne : indices connus exacts, pages non servies
  // supposées sans puces (corrigé par ancrage à l'arrivée).
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

  // scrollTop -> premier index visible (point fixe, converge en 2-3 tours
  // car les extras sont petits devant le pas).
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
    const t0 = performance.now();
    const promesse = appel('list_messages', { offset: p * PAGE, limit: PAGE })
      .then(async (page) => {
        total = page.total;
        pages.set(p, page.rows);
        let n = 0;
        for (const l of page.rows) if (aPuces(l)) n += 1;
        chipsParPage.set(p, n);
        pending.delete(p);
        if (premierePageMs === null) premierePageMs = performance.now() - t0;
        // Ancrage : la page est entièrement au-dessus de la fenêtre ->
        // tout ce qui suit descend de n * extraPuce ; on suit.
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
        console.error(`list_messages page ${p} :`, err);
      });
    pending.set(p, promesse);
    return promesse;
  }

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
    // Toute fenêtre demande ses pages — y compris l'overscan.
    for (let p = Math.floor(debut / PAGE); p <= Math.floor(Math.max(0, fin - 1) / PAGE); p++) {
      servirPage(p);
    }
  });

  function surDefilement() {
    premier = indexPour(cadre.scrollTop);
  }

  function sonder(el, avecPuces) {
    // Ligne sonde : mesure le gabarit réel une fois, hors interaction.
    const h = el.offsetHeight;
    if (avecPuces) h2 = h;
    else h1 = h;
    sondees = true;
  }

  function choisir(l) {
    selection = `${l.account_id}/${l.mailbox}/${l.uid}`;
    onselect(l);
  }
  const estChoisie = (l) => selection === `${l.account_id}/${l.mailbox}/${l.uid}`;

  // --- API de mesure et de pilotage (banc P1, e2e) --------------------
  export function aller(index) {
    cadre.scrollTop = decalage(index) - PAD;
    surDefilement();
  }
  export async function allerEtServir(index) {
    // Demande EXPLICITE des pages de la fenêtre cible : le $effect qui
    // les demanderait ne s'exécute qu'au flush suivant — attendre
    // `pending` sans cela mesurerait un saut sans son service IPC.
    const t0 = performance.now();
    aller(index);
    const de = Math.floor(Math.max(0, index - OVER) / PAGE);
    const a = Math.floor(Math.min(Math.max(0, total - 1), index + visibles + OVER) / PAGE);
    const attentes = [];
    for (let p = de; p <= a; p++) attentes.push(servirPage(p));
    await Promise.all(attentes);
    await tick();
    void cadre.offsetHeight; // reflow forcé : le travail est réellement fait
    return performance.now() - t0;
  }
  export function etat() {
    return { total, premier, h1, h2, premierePageMs };
  }
  export function ligneA(index) {
    const page = pages.get(Math.floor(index / PAGE));
    return page ? page[index % PAGE] : null;
  }

  servirPage(0);
</script>

<section class="cadre" bind:this={cadre} onscroll={surDefilement}
         aria-label="Liste des messages" data-testid="liste">
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
  <div class="espace" style="height:{hauteurEspace}px">
    <div class="fenetre" style="transform:translateY({decalage(debut)}px)">
      {#each fenetre as { i, ligne } (i)}
        {#if ligne}
          <article class="ligne"
                   class:nonlu={ligne.thread_unseen > 0}
                   class:choisie={estChoisie(ligne)}
                   data-testid="ligne"
                   onclick={() => choisir(ligne)}>
            <div class="l1">
              <span class="exp">{ligne.sender}</span>
              <span class="heure">{ligne.date}</span>
            </div>
            <p class="objet">{ligne.subject}</p>
            <p class="apercu"></p>
            {#if aPuces(ligne)}
              <span class="puces">
                {#if ligne.thread_size > 1}
                  <span class="puce"><span class="ms" aria-hidden="true">forum</span>{ligne.thread_size} messages</span>
                {/if}
                {#if ligne.has_attachment}
                  <span class="puce"><span class="ms" aria-hidden="true">attach_file</span>fichiers</span>
                {/if}
              </span>
            {/if}
          </article>
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
</section>

<style>
  /* Géométrie et états VERBATIM du prototype (écran 02, listItems). */
  .cadre { height:100%; overflow:auto; background:var(--bg); }
  .espace { position:relative; }
  .fenetre {
    position:absolute; top:0; left:12px; right:12px;
    display:flex; flex-direction:column; gap:8px;
  }
  .sondes { position:absolute; visibility:hidden; left:12px; right:12px; }

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
</style>
