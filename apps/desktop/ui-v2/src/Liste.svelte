<script>
  // Liste fenêtrée de l'écran 02 — lignes continues séparées au filet,
  // le dessin des pistes (A29/A30) : UN gabarit déterministe (les puces
  // fil/fichiers vivent au volet de lecture depuis A29 — la ligne est
  // nue, A2), servie par `list_category` : la source est (catégorie,
  // compte, non-lus), les onglets vivent dans le pied de cette colonne.
  //
  // Changement de source = nouvelle génération : les pages en vol de la
  // source précédente sont jetées à l'arrivée, jamais mélangées.
  import { tick, untrack } from 'svelte';
  import { appel } from './lib/transport.js';
  import { quand } from './lib/quand.js';
  import { activation } from './lib/clavier.js';
  import { t } from './lib/texte.svelte.js';

  let {
    categorie = 'reception',
    compte = null,
    onglet = 'tous',
    recherche = '',
    // PLAN-BROUILLONS : les brouillons locaux (rows de `list_drafts`),
    // sondés par l'App — le dossier Brouillons les affiche, la
    // Réception mentionne les fils qui en portent un.
    brouillons = [],
    onreprendre = () => {},
    onselect = () => {},
    ononglet = () => {},
    ontotal = () => {},
    onresultats = () => {},
  } = $props();

  const PAGE = 200;
  const OVER = 8;

  let cadre = $state(null);
  let total = $state(0);
  let premier = $state(0);
  let version = $state(0);
  let h1 = $state(90);
  let sondees = $state(false);
  let selection = $state(null);
  let premierePageMs = $state(null);

  let generation = 0;
  let pages = new Map();
  let pending = new Map();
  // Stale-while-revalidate (PLAN-REACTIVITE E1) : la génération à
  // laquelle chaque page a été servie. Une recharge bump `generation`
  // SANS jeter `pages` — les lignes affichées restent le fond, et une
  // page ne se resert que si sa génération est dépareillée.
  let servieA = new Map();

  // Un seul gabarit depuis A29 (lignes continues, sans puces ni
  // marges) : la géométrie du fenêtrage est une multiplication.
  const pitch = $derived(h1);

  function decalage(i) {
    return i * pitch;
  }

  const hauteurEspace = $derived(total === 0 ? 0 : total * pitch);

  function indexPour(scrollTop) {
    const i = Math.max(0, Math.floor(scrollTop / pitch));
    return Math.min(i, Math.max(0, total - 1));
  }

  function servirPage(p) {
    // Le dossier Brouillons ne se sert pas ici : sa page vient de
    // `list_drafts` (PLAN-BROUILLONS, B-D1), pas de `list_category`.
    if (categorie === 'brouillons') return Promise.resolve();
    if (servieA.get(p) === generation || pending.has(p)) {
      return pending.get(p) || Promise.resolve();
    }
    const nee = generation;
    const t0 = performance.now();
    const promesse = appel('list_category', {
      category: categorie,
      accountId: compte,
      nonLus: onglet === 'nonlus',
      offset: p * PAGE,
      limit: PAGE,
    })
      .then((page) => {
        if (nee !== generation) return; // source changée : page périmée
        total = page.total;
        pages.set(p, page.rows);
        servieA.set(p, nee);
        pending.delete(p);
        if (premierePageMs === null) premierePageMs = performance.now() - t0;
        version += 1;
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
      servieA = new Map();
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
    cadre ? Math.ceil(cadre.clientHeight / pitch) + 1 : 12,
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
  async function executerRecherche(q) {
    const mien = ++jetonRecherche;
    try {
      const lignes = await appel('search_messages', { query: q });
      if (mien !== jetonRecherche) return; // frappe plus récente
      resultats = lignes;
      onresultats(lignes.length);
    } catch (err) {
      console.error('search_messages :', err);
    }
  }
  $effect(() => {
    const q = recherche.trim();
    untrack(() => {
      clearTimeout(minuterieRecherche);
      if (q.length < 3) {
        jetonRecherche += 1;
        resultats = null;
        onresultats(null);
        return;
      }
      minuterieRecherche = setTimeout(() => executerRecherche(q), 150);
    });
  });

  $effect(() => {
    // Le dossier Brouillons compte ses lignes à lui — la barre de
    // statut dit « Brouillons · N éléments » sur la même mécanique.
    const n = lignesBrouillons !== null ? lignesBrouillons.length : total;
    untrack(() => ontotal(n));
  });

  function sonder(el) {
    h1 = el.offsetHeight;
    sondees = true;
  }

  const cle = (l) => `${l.account_id}/${l.mailbox}/${l.uid}`;
  function choisir(l) {
    selection = cle(l);
    onselect(l);
  }
  const estChoisie = (l) => selection === cle(l);

  // --- Brouillons (PLAN-BROUILLONS) -----------------------------------
  // Le fil -> son brouillon le plus récent : la mention de la Réception
  // (variante B validée) montre le préfixe et le CORPS du brouillon en
  // aperçu — première ligne et heure intactes (B-D3). Jamais sur une
  // recherche : un résultat est un message, pas une conversation.
  const brouillonsParFil = $derived.by(() => {
    const carte = new Map();
    for (const b of brouillons) {
      if (b.thread_id == null) continue;
      const connu = carte.get(b.thread_id);
      if (!connu || b.updated_epoch > connu.updated_epoch) carte.set(b.thread_id, b);
    }
    return carte;
  });
  const brouillonDe = (l) =>
    categorie === 'reception' && resultats === null
      ? (brouillonsParFil.get(l.thread_id) ?? null)
      : null;
  // Le dossier : les brouillons du compte borné par la nav, déjà du
  // plus récent au plus ancien (`list_drafts`). Peu nombreux par
  // construction : le chemin non fenêtré des résultats suffit.
  const lignesBrouillons = $derived(
    categorie === 'brouillons'
      ? brouillons.filter((b) => compte === null || b.account_id === compte)
      : null,
  );

  const ONGLETS = [
    { id: 'tous', icone: 'inbox', libelle: 'onglet.tous' },
    { id: 'nonlus', icone: 'mark_email_unread', libelle: 'onglet.nonlus' },
    { id: 'brouillons', icone: 'edit_note', libelle: 'boite.brouillons' },
  ];
  const ongletActif = $derived(categorie === 'brouillons' ? 'brouillons' : onglet);

  // --- API (App, banc P1, e2e) ---------------------------------------
  export function aller(index) {
    cadre.scrollTop = decalage(index);
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
    return { total, premier, h1, premierePageMs };
  }
  export function ligneA(index) {
    const page = pages.get(Math.floor(index / PAGE));
    return page ? page[index % PAGE] : null;
  }
  // Le triage clavier (App) : la sélection se pose sans passer par le
  // clic — même clé, même liseré, aucun rappel onselect.
  export function selectionner(ligne) {
    selection = cle(ligne);
  }
  // La ligne SOUS celle-ci. Recherche active : la suivante des
  // résultats ; sinon l'index absolu dans les pages fenêtrées — une
  // page voisine non servie rend null (rare : la fenêtre sert large).
  export function suivante(ligne) {
    const id = cle(ligne);
    if (resultats !== null) {
      const i = resultats.findIndex((l) => cle(l) === id);
      return i >= 0 && i + 1 < resultats.length ? resultats[i + 1] : null;
    }
    for (const [p, rows] of pages) {
      const i = rows.findIndex((l) => cle(l) === id);
      if (i >= 0) return ligneA(p * PAGE + i + 1) ?? null;
    }
    return null;
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
    // Stale-while-revalidate (PLAN-REACTIVITE E1) : les lignes servies
    // RESTENT affichées — chaque page est remplacée à l'arrivée de sa
    // version fraîche, jamais jetée avant. Le squelette n'existe qu'au
    // premier chargement d'une source et au défilement vers l'inconnu ;
    // les pages hors écran, gardées telles quelles, se resservent au
    // défilement (génération dépareillée).
    generation += 1;
    pending = new Map();
    // L'écran d'abord — tout le rang visible, pas la seule page de
    // `premier` (une fenêtre à cheval laissait sa seconde page sans
    // resservie) — puis la page 0, qui porte le total frais.
    const de = Math.floor(debut / PAGE);
    const a = Math.floor(Math.max(0, fin - 1) / PAGE);
    for (let p = de; p <= a; p++) servirPage(p);
    servirPage(0);
    // Une recherche ACTIVE se resert aussi : archiver un résultat doit
    // le retirer des résultats — la régression #4 de v1, même trou.
    if (resultats !== null) {
      const q = recherche.trim();
      if (q.length >= 3) executerRecherche(q);
    }
  }
</script>

<section class="colonne" aria-label={t('liste.aria')} data-testid="liste">
  <div class="cadre" bind:this={cadre} onscroll={surDefilement}>
    {#if !sondees}
      <div class="sondes" aria-hidden="true">
        <article class="ligne" use:sonder>
          <div class="l1"><span class="exp">Sonde</span><span class="heure">00:00</span></div>
          <p class="objet">Sonde</p>
          <p class="apercu">Sonde</p>
        </article>
      </div>
    {/if}
    {#snippet rangee(ligne)}
      <div class="ligne"
           class:nonlu={ligne.thread_unseen > 0}
           class:choisie={estChoisie(ligne)}
           data-testid="ligne"
           role="button" tabindex="0"
           onclick={(e) => {
             // Constat terrain (2026-08-15, suite d'A38) : un clic
             // laissait le focus sur la rangée, et la PREMIÈRE touche
             // venue (raccourci ou non) basculait en modalité clavier —
             // l'anneau :focus-visible surgissait sur un nœud recyclé
             // par index. Choisie à la souris (detail > 0 : jamais vrai
             // au clavier ni aux technologies d'assistance), la rangée
             // rend le focus ; le liseré dit la position. Entrée/Espace
             // passent par activation(), focus et anneau intacts (A8).
             if (e.detail > 0) e.currentTarget.blur();
             choisir(ligne);
           }}
           onkeydown={activation(() => choisir(ligne))}>
        <div class="l1">
          <span class="exp">{ligne.sender}</span>
          <span class="heure">{quand(ligne.epoch)}</span>
        </div>
        <p class="objet">{ligne.subject}</p>
        {#if brouillonDe(ligne)}
          <!-- Variante B (PLAN-BROUILLONS §3) : l'aperçu dit le
               brouillon — préfixe et corps ; le reste de la ligne ne
               bouge pas. -->
          <p class="apercu"><span class="prefixe" data-testid="mention-brouillon">{t('liste.prefixeBrouillon')}</span>{brouillonDe(ligne).body}</p>
        {:else}
          <p class="apercu">{ligne.preview ?? ''}</p>
        {/if}
        <!-- Les puces fil/fichiers vivent au volet de lecture (A29/A2) :
             la ligne est nue, le non-lu se lit à la graisse. -->
      </div>
    {/snippet}
    {#if resultats !== null}
      <div class="fenetre-recherche" data-testid="resultats">
        {#if resultats.length === 0}
          <div class="vide-recherche"><p>{t('liste.aucunResultat')}</p></div>
        {/if}
        {#each resultats as ligne (`${ligne.account_id}/${ligne.mailbox}/${ligne.uid}`)}
          {@render rangee(ligne)}
        {/each}
      </div>
    {:else if lignesBrouillons !== null}
      <!-- Le dossier Brouillons (B-D1) : les brouillons locaux, du plus
           récent au plus ancien. Le clic REPREND — jamais mark_seen, il
           n'y a rien à lire ici, seulement à finir. -->
      <div class="fenetre-recherche" data-testid="dossier-brouillons">
        {#if lignesBrouillons.length === 0}
          <div class="vide-recherche"><p>{t('liste.vide')}</p></div>
        {/if}
        {#each lignesBrouillons as b (b.id)}
          <div class="ligne" data-testid="ligne-brouillon"
               role="button" tabindex="0"
               onclick={() => onreprendre(b)}
               onkeydown={activation(() => onreprendre(b))}>
            <div class="l1">
              <span class="exp" class:sans={!b.to}>
                {b.to ? t('brouillons.a', { a: b.to }) : t('brouillons.sansDestinataire')}</span>
              <span class="heure">{quand(Math.floor(b.updated_epoch / 1000))}</span>
            </div>
            <p class="objet" class:sans={!b.subject}>{b.subject || t('brouillons.sansObjet')}</p>
            <p class="apercu">{b.body}</p>
          </div>
        {/each}
      </div>
    {:else}
      {#if total === 0 && premierePageMs !== null}
        <div class="vide"><p>{t('liste.vide')}</p></div>
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
        <span class="ms" aria-hidden="true">{o.icone}</span>{t(o.libelle)}
      </span>
    {/each}
  </div>
</section>

<style>
  /* Géométrie et états du dessin des pistes (A29/A30) : lignes
     continues séparées au filet, sans carte ni ombre. */
  .colonne {
    display:flex; flex-direction:column; min-height:0;
    background:var(--bg); border-right:1px solid var(--border);
  }
  .cadre { flex:1; overflow:auto; position:relative; }
  .espace { position:relative; }
  .fenetre {
    position:absolute; top:0; left:0; right:0;
    display:flex; flex-direction:column;
  }
  .sondes { position:absolute; visibility:hidden; left:0; right:0; }
  .vide {
    position:absolute; inset:0; display:flex; align-items:center;
    justify-content:center; padding:40px; text-align:center;
  }
  .vide p { margin:0; font-size:13px; line-height:1.5; color:var(--muted); }
  .fenetre-recherche { display:flex; flex-direction:column; }
  .vide-recherche { padding:40px; text-align:center; }
  .vide-recherche p { margin:0; font-size:13px; line-height:1.5; color:var(--muted); }

  /* Quatre états (A30) : repos transparent, survol en teinte légère,
     sélection en teinte + liseré d'accent de 2 px — jamais d'ombre ni
     de surface blanche (A29). Le liseré est réservé en transparent :
     la sélection ne déplace pas le contenu. */
  .ligne {
    padding:13px 16px; border-top:1px solid var(--border);
    border-left:2px solid transparent;
    display:flex; flex-direction:column; gap:3px; cursor:pointer;
  }
  .ligne:hover { background:var(--hover); }
  .ligne.choisie {
    background:var(--sel); border-left-color:var(--accent);
  }
  .l1 { display:flex; align-items:baseline; gap:10px; }
  .exp {
    font-size:14px; color:var(--ink); flex:1; min-width:0;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
  }
  .nonlu .exp { font-weight:700; }
  .heure { font-size:12px; color:var(--muted); flex:none; }
  .objet {
    /* 14 px (A29 — amende A9) : le gabarit des pistes. */
    margin:0; font-size:14px; font-weight:400; line-height:1.3;
    color:var(--ink);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
  }
  .nonlu .objet { font-weight:700; }
  .apercu {
    margin:0; font-size:13px; line-height:1.45; color:var(--ink2);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
    min-height:1.45em;
  }
  /* La mention « Brouillon : » (variante B, PLAN-BROUILLONS §3) : le
     jeton d'alerte en texte — mesuré par contraste.mjs sur les trois
     fonds de rangée (repos, survol, choisie). */
  .prefixe { color:var(--alert); font-weight:600; }
  /* Champs vides du dossier : l'atténué italique le dit, jamais un
     blanc (« (sans objet) », « (sans destinataire) »). */
  .sans, .objet.sans { font-style:italic; color:var(--muted); font-weight:400; }
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
  .onglet:hover { background:var(--hover); }
  .onglet.actif {
    font-weight:600; color:var(--ink); background:var(--sel);
    border-color:var(--accent);
  }
</style>
