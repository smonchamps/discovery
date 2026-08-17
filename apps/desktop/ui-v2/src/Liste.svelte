<script>
  // Liste fenêtrée de l'écran 02 — lignes continues séparées au filet,
  // le dessin des pistes (A29/A30), servie par `list_category` : la
  // source est (catégorie, compte, non-lus), les onglets vivent dans le
  // pied de cette colonne. Depuis A44 (PLAN-RETOURS-V3, renverse la
  // « ligne nue » d'A29/A2 ; terrain du 2026-08-16 : hauteur AU
  // CONTENU, pas de rang réservé) une ligne qui a quelque chose à dire
  // porte le rang de puces du prototype et s'en agrandit : DEUX
  // gabarits (h1 nue, h2 porteuse), la mécanique de fenêtrage d'avant
  // A29 — chipsParPage, extraPuce, correction itérative — reprend du
  // service, à l'identique de ce qu'elle était (848f286~1).
  //
  // Changement de source = nouvelle génération : les pages en vol de la
  // source précédente sont jetées à l'arrivée, jamais mélangées.
  import { tick, untrack } from 'svelte';
  import { appel } from './lib/transport.js';
  import { quand } from './lib/quand.js';
  import { initiales } from './lib/initiales.js';
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

  // R4 (PLAN-RETOURS-MAIL) : dans le dossier d'envois, l'expéditeur est
  // SOI — répéter son nom sur chaque ligne n'apprend rien. La colonne
  // dit le DESTINATAIRE (« À : X »), tiré de `to_addrs` stocké à la
  // synchro. À défaut (ancien envoi non encore rattrapé) on garde le nom
  // d'expéditeur d'avant — jamais de ligne muette.
  const versEnvoi = (ligne) => categorie === 'envoyes' && (ligne.to_addrs?.length ?? 0) > 0;
  const correspondant = (ligne) =>
    versEnvoi(ligne) ? ligne.to_addrs.join(', ') : ligne.sender;

  let cadre = $state(null);
  let total = $state(0);
  let premier = $state(0);
  let version = $state(0);
  let h1 = $state(90);
  let h2 = $state(117);
  let sondees = $state(false);
  let selection = $state(null);
  let premierePageMs = $state(null);

  let generation = 0;
  let pages = new Map();
  let chipsParPage = new Map();
  let pending = new Map();
  // Stale-while-revalidate (PLAN-REACTIVITE E1) : la génération à
  // laquelle chaque page a été servie. Une recharge bump `generation`
  // SANS jeter `pages` — les lignes affichées restent le fond, et une
  // page ne se resert que si sa génération est dépareillée.
  let servieA = new Map();

  // Deux gabarits (A44, terrain : hauteur au contenu) : h1 la ligne
  // nue, h2 la porteuse — la géométrie corrige la multiplication par
  // le compte de porteuses AVANT l'index, tenu par page.
  const aPuces = (l) => l.thread_size > 1 || l.attachment_count > 0;
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
    return i * h1 + chipsAvant(i) * extraPuce;
  }

  const hauteurEspace = $derived.by(() => {
    void version;
    if (total === 0) return 0;
    let extra = 0;
    for (const n of chipsParPage.values()) extra += n;
    return total * h1 + extra * extraPuce;
  });

  function indexPour(scrollTop) {
    let i = Math.max(0, Math.floor(scrollTop / h1));
    for (let tour = 0; tour < 4; tour++) {
      const corrige = Math.max(
        0,
        Math.floor((scrollTop - chipsAvant(i) * extraPuce) / h1),
      );
      if (corrige === i) break;
      i = corrige;
    }
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
      .then(async (page) => {
        if (nee !== generation) return; // source changée : page périmée
        total = page.total;
        // Le delta de puces, pas le compte brut : une page REMPLACÉE
        // affichait déjà les siennes — l'ancrage du défilement ne doit
        // bouger que de la différence (première servie : avant = 0).
        const avant = chipsParPage.get(p) ?? 0;
        pages.set(p, page.rows);
        servieA.set(p, nee);
        let n = 0;
        for (const l of page.rows) if (aPuces(l)) n += 1;
        chipsParPage.set(p, n);
        pending.delete(p);
        if (premierePageMs === null) premierePageMs = performance.now() - t0;
        const delta = n - avant;
        if (delta !== 0 && (p + 1) * PAGE <= premier && cadre) {
          version += 1;
          await tick();
          cadre.scrollTop += delta * extraPuce;
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
    cadre ? Math.ceil(cadre.clientHeight / h1) + 1 : 12,
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
      const res = await appel('search_messages', { query: q });
      if (mien !== jetonRecherche) return; // frappe plus récente
      resultats = res.rows;
      // Le nombre rendu (plafonné) ET le total : la barre dira « N sur M »
      // quand le plafond mord.
      onresultats(res.rows.length, res.total);
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
        onresultats(null, null);
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
    return { total, premier, h1, h2, premierePageMs };
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
  <!-- UI v3, E1 (verdict CE 2026-08-16) : le bandeau de la maquette
       Classique — le nom de la boîte courante, SEUL (« Tout marquer
       lu » écarté). Les clés boite.* sont celles de la nav. -->
  <header class="bandeau" data-testid="liste-titre">
    <h1>{t(`boite.${categorie}`)}</h1>
  </header>
  <div class="cadre" bind:this={cadre} onscroll={surDefilement}>
    {#if !sondees}
      <div class="sondes" aria-hidden="true">
        <article class="ligne" use:sonder={false}>
          <span class="avatar" aria-hidden="true">SO</span>
          <div class="l1"><span class="exp">Sonde</span><span class="heure">00:00</span></div>
          <p class="objet">Sonde</p>
          <p class="apercu">Sonde</p>
        </article>
        <article class="ligne" use:sonder={true}>
          <span class="avatar" aria-hidden="true">SO</span>
          <div class="l1"><span class="exp">Sonde</span><span class="heure">00:00</span></div>
          <p class="objet">Sonde</p>
          <p class="apercu">Sonde</p>
          <div class="puces"><span class="puce"><span class="ms" aria-hidden="true">forum</span>2</span></div>
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
        <span class="avatar" data-testid="avatar" aria-hidden="true">{initiales(correspondant(ligne))}</span>
        <div class="l1">
          <span class="exp">{#if versEnvoi(ligne)}{t('liste.dest', { a: correspondant(ligne) })}{:else}{ligne.sender}{/if}</span>
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
        <!-- PLAN-RETOURS-V3 R1 (verdict CE 2026-08-16, D1/D2) : la
             « ligne nue » d'A29 est renversée — le rang de puces du
             prototype revient à la ligne, aux règles de la tête du Fil
             (« N messages » si le fil en a plus d'un, « N fichiers »
             si pièces). Hauteur AU CONTENU (terrain CE 2026-08-16,
             renverse D1) : le rang n'existe que sur les porteurs et
             agrandit leur ligne — deux gabarits, le fenêtrage corrige
             par chipsAvant. En
             RECHERCHE, un résultat est un message, pas une conversation
             (le coeur sert thread_size=1 sans joindre threads) : la
             puce de fil n'y figure pas, par construction. Le compte de
             pièces est celui d'AVANT lecture du corps : 0 tant que le
             corps n'est pas rapatrié — la puce apparaît au fil du
             rattrapage, jamais à tort. -->
        {#if aPuces(ligne)}
          <div class="puces" data-testid="puces-ligne">
            {#if ligne.thread_size > 1}
              <span class="puce"><span class="ms" aria-hidden="true">forum</span>{t('puce.messages', { n: ligne.thread_size })}</span>
            {/if}
            {#if ligne.attachment_count > 0}
              <span class="puce"><span class="ms" aria-hidden="true">attach_file</span>{t('puce.fichiers', { n: ligne.attachment_count })}</span>
            {/if}
          </div>
        {/if}
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
            <span class="avatar" aria-hidden="true">{initiales(b.to)}</span>
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
                <span class="avatar" aria-hidden="true"></span>
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
  /* Le bandeau (UI v3, E1 — reformé PLAN-RETOURS-V3 R2) : le MÊME
     format visuel que le bandeau de filtre du bas — 52 px, fond
     --panel, filet vers la liste ; le titre 16 px 600 ne bouge pas. */
  .bandeau {
    flex:none; height:52px; display:flex; align-items:center;
    padding:0 16px; background:var(--panel);
    border-bottom:1px solid var(--border);
  }
  .bandeau h1 {
    margin:0; font-size:16px; font-weight:600; line-height:1.3;
    color:var(--ink); flex:1; min-width:0;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
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
  /* UI v3, E2 : la grille de la maquette — l'avatar en première
     colonne, enjambant les trois rangs du contenu ; le reste du dessin
     des pistes (filet, états, graisses) ne bouge pas. Le rang de puces
     (A44, terrain : hauteur au contenu) n'existe que sur les lignes
     porteuses, en 4e rang de colonne 2 — hors de l'avatar, comme au
     prototype ; les DEUX hauteurs sont sondées (h1/h2). */
  .ligne {
    padding:13px 16px; border-top:1px solid var(--border);
    border-left:2px solid transparent;
    display:grid; grid-template-columns:auto 1fr; column-gap:10px;
    row-gap:3px; align-items:start; cursor:pointer;
  }
  .avatar {
    grid-row:1 / span 3; width:28px; height:28px; border-radius:50%;
    background:var(--panel); border:1px solid var(--border);
    display:grid; place-items:center;
    font-size:11px; font-weight:600; color:var(--ink2);
  }
  .l1, .objet, .apercu, .puces { grid-column:2; min-width:0; }
  /* Le rang de puces (PLAN-RETOURS-V3 R1) : le gabarit 24 px du
     prototype Classique — présent sur les seules lignes porteuses. */
  .puces {
    height:24px; display:flex; align-items:center; gap:6px;
    overflow:hidden;
  }
  .puce {
    display:inline-flex; align-items:center; gap:5px; height:24px;
    padding:0 9px; font-size:12px; color:var(--ink2);
    background:var(--surface); border:1px solid var(--border);
    border-radius:6px; white-space:nowrap;
  }
  .puce .ms { font-size:14px; }
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
