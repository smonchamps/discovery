<script>
  // Écran 01, refondu en parcours de premier démarrage (PLAN-RETOURS-8
  // R2, A75 — renverse « l'accueil qui ne réclame qu'une adresse » ;
  // forme arrêtée au terrain du 2026-08-22, constats 1-8) : quatre
  // étapes — comptes, disposition, thème, récapitulatif. Chaque étape
  // dit son titre, puis « Étape n/4 », puis son texte. Deux régimes :
  // `complet` (première installation : le parcours entier, la marque
  // `wind-accueil-fait` posée au Terminer) et guichet seul (un poste
  // revenu à zéro compte : l'accueil sans étapes, qui s'efface au
  // premier compte — le comportement d'avant, inchangé).
  //
  // L'étape 2 montre des CAPTURES RÉELLES de l'application (décor
  // Clarity, e2e/capture-accueil.mjs — constat 5) ; l'étape 3 dessine
  // ses fenêtres aux couleurs de FICHES ET dans la disposition choisie
  // à l'étape 2 (constat 6). L'étape 4 récapitule les trois choix —
  // chacun est une porte qui ramène à son étape (constat 8). Les
  // étapes 2 et 3 ne touchent ni la base ni le réseau :
  // `appliquerVolets` / `appliquerTheme` appliquent ET persistent.
  // Seule l'étape 1 (GuichetCompte, A11) parle au shell.
  import GuichetCompte from './GuichetCompte.svelte';
  import Hitofude from './Hitofude.svelte';
  import { t } from './lib/texte.svelte.js';
  import { FICHES, appliquerTheme, themeAffiche } from './lib/theme.js';
  import { voletsActuels, appliquerVolets } from './lib/volets.svelte.js';
  import { marquerAccueilFait, marquerAccueilCommence } from './lib/accueil.js';
  import apercu3 from './assets/accueil/disposition-3.png';
  import apercu2 from './assets/accueil/disposition-2.png';
  import apercu1 from './assets/accueil/disposition-1.png';

  let { comptes = [], complet = false, onajoute = () => {}, onfini = () => {} } = $props();

  const ETAPES = 4;
  const APERCUS = { 3: apercu3, 2: apercu2, 1: apercu1 };
  let etape = $state(1);
  // Constat 2 : dès qu'une adresse existe, la barre d'ajout se replie
  // derrière « Ajouter une autre adresse email » — et se rouvre au
  // clic. `surAjout` la replie après chaque ajout réussi.
  let ajoutOuvert = $state(false);
  // 2e passe terrain, constat 3 : l'aperçu de l'étape 2 est UNE image —
  // celle du bouton survolé tant qu'il l'est, celle du choix sinon.
  let survolVolets = $state(null);
  // 3e passe, constat 2 : le guichet générique révélé prend toute la
  // marche — le « Continuer » du parcours se masque pendant ce temps.
  let generiqueOuvert = $state(false);
  // D4 : l'étape 1 exige au moins un compte — Wind est utilisable dès
  // la fin du parcours.
  const peutContinuer = $derived(etape !== 1 || comptes.length > 0);
  // Le volet choisi se LIT du module partagé ($state runes) — aucun
  // miroir local à resynchroniser (revue 2026-08-22).
  const volets = $derived(voletsActuels());
  // La coche suit la fiche AFFICHÉE (le patron des Réglages, revue
  // A42) : sous suivi OS sombre, c'est la déclinaison -nuit qui coche —
  // y compris quand l'OS bascule pendant l'étape 3 (même écouteur que
  // Réglages).
  let themeActif = $state(themeAffiche());
  // La fiche du thème AFFICHÉ — la miniature du récapitulatif s'en
  // colore (2e passe, constat 4).
  const ficheActive = $derived(FICHES.find((f) => f.id === themeActif) ?? FICHES[0]);
  $effect(() => {
    const suivre = () => (themeActif = themeAffiche());
    document.addEventListener('wind:theme-affiche', suivre);
    return () => document.removeEventListener('wind:theme-affiche', suivre);
  });
  // La marque « commencé » se pose dès que le parcours S'AFFICHE : un
  // compte ajouté à l'étape 1 puis l'app quittée avant Terminer ne
  // fait pas une installation « déjà accueillie » — le parcours
  // reprend (revue 2026-08-22).
  $effect(() => {
    if (complet) marquerAccueilCommence();
  });

  function surAjout() {
    ajoutOuvert = false;
    generiqueOuvert = false;
    onajoute();
  }
  function continuer() {
    if (!peutContinuer) return;
    if (etape < ETAPES) etape += 1;
  }
  function retour() {
    if (etape > 1) etape -= 1;
  }
  function choisirTheme(id) {
    appliquerTheme(id);
    themeActif = themeAffiche();
  }
  function terminer() {
    marquerAccueilFait();
    onfini();
  }
</script>

<div class="ecran01" data-testid="onboarding">
  <!-- L'aperçu-fenêtre de l'étape 3 : n = disposition dessinée (celle
       choisie à l'étape 2, constat 6), c = couleurs d'une fiche. -->
  {#snippet fenetre(n, c)}
    <span class="fenetre" aria-hidden="true" style="background:{c.bg}">
      <span class="f-tete" style="background:{c.panel}"></span>
      <span class="f-corps">
        {#if n !== 1}<span class="f-nav" style="background:{c.panel}"></span>{/if}
        <span class="f-liste">
          <span style="background:{c.surface}"></span>
          <span style="background:{c.accent}"></span>
          <span style="background:{c.surface}"></span>
        </span>
        {#if n === 3}<span class="f-lecture" style="background:{c.surface}"></span>{/if}
      </span>
    </span>
  {/snippet}
  {#snippet progression(n)}
    {#if complet}
      <p class="progression" data-testid="accueil-progression">
        {t('accueil.etape', { n, total: ETAPES })}</p>
    {/if}
  {/snippet}
  <div class="colonne" class:large={complet && etape !== 1}>
    {#if !complet || etape === 1}
      <!-- Constat 1 : « Bienvenue dans Wind », le trait hitofude
           derrière la marque — le bloc du guichet est UN (étape 1 du
           parcours ET écran d'un poste accueilli revenu à zéro
           compte). -->
      <h3 class="titre">{t('accueil.bienvenue')}
        <span class="marque">Wind<Hitofude largeur={52} hauteur={12} /></span></h3>
      {@render progression(1)}
      <p class="sous">{t('accueil.ajouterSous')}</p>
      {#if complet && comptes.length > 0}
        <ul class="ajoutes" data-testid="accueil-comptes">
          {#each comptes as c (c.account_id)}
            <li><span class="ms" aria-hidden="true">check_circle</span>{c.email}</li>
          {/each}
        </ul>
      {/if}
      {#if comptes.length === 0 || ajoutOuvert}
        <!-- Constat 1 (2e passe) : tant que Continuer n'est pas là
             (aucun compte), « Ajouter » est LE geste — primaire. -->
        <GuichetCompte accueil ajoutPrincipal={comptes.length === 0}
                       ongenerique={(v) => (generiqueOuvert = v)}
                       onajoute={surAjout} />
      {:else}
        <!-- Constat 2 : la barre repliée derrière une porte explicite. -->
        <button type="button" class="secondaire" data-testid="accueil-ajouter-autre"
                onclick={() => (ajoutOuvert = true)}>
          {t('accueil.ajouterAutre')}</button>
      {/if}
    {:else if etape === 2}
      <h3 class="titre">{t('accueil.voletsTitre')}</h3>
      {@render progression(2)}
      <p class="sous">{t('accueil.voletsSous')}</p>
      <!-- Constats 5 (1re passe), 3 (2e) et 3 (3e passe) : UNE capture
           RÉELLE de l'application — actualisée au survol (et au focus
           clavier, A8) du bouton visé, revenue au choix sinon —,
           l'ensemble image + boutons dans une élévation, boutons
           centrés. -->
      <div class="cadre-volets">
        <img class="capture-grande" data-testid="accueil-apercu"
             data-volets={survolVolets ?? volets}
             src={APERCUS[survolVolets ?? volets]} alt="" />
        <div class="boutons-volets" data-testid="accueil-volets">
          {#each [3, 2, 1] as n (n)}
            <button type="button" class="choix-volet" class:choisie={volets === n}
                    data-testid="accueil-volet" data-volets={n}
                    aria-pressed={volets === n}
                    onclick={() => appliquerVolets(n)}
                    onmouseenter={() => (survolVolets = n)}
                    onmouseleave={() => (survolVolets = null)}
                    onfocus={() => (survolVolets = n)}
                    onblur={() => (survolVolets = null)}>
              {t(`volets.${n}`)}</button>
          {/each}
        </div>
      </div>
    {:else if etape === 3}
      <h3 class="titre">{t('accueil.themeTitre')}</h3>
      {@render progression(3)}
      <p class="sous">{t('accueil.themeSous')}</p>
      <div class="cartes themes" data-testid="accueil-themes">
        {#each FICHES as fiche (fiche.id)}
          {@const [accent, bg, panel, surface] = fiche.pastilles}
          <button type="button" class="carte" class:choisie={themeActif === fiche.id}
                  data-testid="accueil-theme" data-theme-id={fiche.id}
                  aria-pressed={themeActif === fiche.id}
                  onclick={() => choisirTheme(fiche.id)}>
            <!-- La fenêtre aux couleurs de LA fiche (FICHES est mesuré
                 contre systeme.css par la gate), dans la disposition
                 choisie (constat 6). -->
            {@render fenetre(volets, { accent, bg, panel, surface })}
            <span class="carte-nom">{t(`theme.${fiche.id}.nom`)}</span>
          </button>
        {/each}
      </div>
    {:else}
      <h3 class="titre">{t('accueil.finTitre')}</h3>
      {@render progression(4)}
      <p class="sous">{t('accueil.finTexte')}</p>
      <!-- Constat 8 (1re passe) : chaque choix récapitulé est une
           porte vers son étape ; constat 4 (2e passe) : miniatures
           pour Disposition et Thème, et au survol comme au focus
           clavier un VOILE couvre la rangée et dit « Revenir à cette
           étape » — les règles visuelles du voile des pièces jointes
           (A70). -->
      <div class="recap" data-testid="accueil-recap">
        <button type="button" class="ligne-recap" data-testid="recap-comptes"
                aria-label="{t('groupe.comptes')} — {t('accueil.revenir')}"
                onclick={() => (etape = 1)}>
          <span class="recap-titre">{t('groupe.comptes')}</span>
          <span class="recap-valeur">{comptes.map((c) => c.email).join(' · ')}</span>
          <span class="voile" aria-hidden="true">
            <span class="ms">arrow_back</span>{t('accueil.revenir')}</span>
        </button>
        <button type="button" class="ligne-recap" data-testid="recap-volets"
                aria-label="{t('reglages.volets')} — {t('accueil.revenir')}"
                onclick={() => (etape = 2)}>
          <span class="recap-titre">{t('reglages.volets')}</span>
          <!-- 4e passe terrain : le texte AU-DESSUS de l'image. -->
          <span class="recap-valeur">{t(`volets.${volets}`)}</span>
          <img class="mini" src={APERCUS[volets]} alt="" />
          <span class="voile" aria-hidden="true">
            <span class="ms">arrow_back</span>{t('accueil.revenir')}</span>
        </button>
        <button type="button" class="ligne-recap" data-testid="recap-theme"
                aria-label="{t('accueil.theme')} — {t('accueil.revenir')}"
                onclick={() => (etape = 3)}>
          <span class="recap-titre">{t('accueil.theme')}</span>
          <!-- 4e passe terrain : le texte AU-DESSUS de l'image. -->
          <span class="recap-valeur">{t(`theme.${themeActif}.nom`)}</span>
          {#if ficheActive}
            {@const [accent, bg, panel, surface] = ficheActive.pastilles}
            <span class="mini-theme">{@render fenetre(volets, { accent, bg, panel, surface })}</span>
          {/if}
          <span class="voile" aria-hidden="true">
            <span class="ms">arrow_back</span>{t('accueil.revenir')}</span>
        </button>
      </div>
    {/if}
    {#if complet}
      <div class="marche">
        {#if etape > 1}
          <button type="button" class="secondaire" data-testid="accueil-retour"
                  onclick={retour}>{t('accueil.retour')}</button>
        {/if}
        {#if etape < ETAPES}
          <!-- 3e passe, constats 1-2 : « Continuer » ne s'affiche
               JAMAIS grisé — absent tant qu'aucun compte n'existe, et
               masqué pendant que le guichet générique est révélé
               (« Ajouter » y est le geste primaire). -->
          {#if peutContinuer && !(etape === 1 && generiqueOuvert)}
            <button type="button" class="principal" data-testid="accueil-continuer"
                    onclick={continuer}>{t('accueil.continuer')}</button>
          {/if}
        {:else}
          <button type="button" class="principal" data-testid="accueil-terminer"
                  onclick={terminer}>{t('accueil.terminer')}</button>
        {/if}
      </div>
    {/if}
  </div>
</div>

<style>
  /* Géométrie de l'écran 01 du prototype — la colonne s'élargit aux
     étapes 2-4, l'écran défile si la grille des thèmes dépasse. */
  .ecran01 {
    position:absolute; inset:0; display:flex; align-items:safe center;
    justify-content:center; background:var(--bg); z-index:1;
    overflow:auto; padding:32px 0;
  }
  .colonne { width:520px; display:flex; flex-direction:column; gap:22px; }
  .colonne.large { width:760px; }
  .titre {
    margin:0; font-size:40px; line-height:1.1; font-weight:600;
    letter-spacing:-.02em; color:var(--ink);
  }
  /* La marque dans le titre : « Wind » et son trait, insécables. */
  .marque { display:inline-flex; align-items:center; gap:10px; white-space:nowrap; }
  .progression {
    margin:0; font-size:13px; font-weight:600; color:var(--muted);
    font-variant-numeric:tabular-nums;
  }
  .sous { margin:0; font-size:15px; line-height:1.5; color:var(--ink2); }
  .ajoutes {
    margin:0; padding:0; list-style:none; display:flex;
    flex-direction:column; gap:6px;
  }
  .ajoutes li {
    display:flex; align-items:center; gap:8px; font-size:14px;
    color:var(--ink);
  }
  .ajoutes .ms { color:var(--accent); }

  .cartes { display:flex; gap:14px; flex-wrap:wrap; }
  .cartes.themes {
    display:grid; grid-template-columns:repeat(4, 1fr); gap:12px;
  }
  .carte {
    display:flex; flex-direction:column; gap:8px; padding:10px;
    background:var(--surface); border:1px solid var(--border);
    border-radius:10px; cursor:pointer; flex:1;
  }
  .carte:hover { background:var(--sel); }
  /* Constats 5/7 : la sélection se voit — contour épaissi (liseré
     interne, aucun décalage de grille) + la teinte de sélection des
     lignes du volet central. */
  .carte.choisie {
    border-color:var(--accent);
    box-shadow:inset 0 0 0 1px var(--accent);
    background:var(--sel);
  }
  .carte-nom { font-size:13px; font-weight:600; color:var(--ink); }
  /* Constat 3 (2e et 3e passes) : l'aperçu unique de l'étape 2 et sa
     rangée de boutons, dans UNE élévation (surface + ombre unique),
     boutons centrés. */
  .cadre-volets {
    display:flex; flex-direction:column; gap:14px; padding:14px;
    background:var(--surface); border:1px solid var(--border);
    border-radius:10px; box-shadow:var(--shadow);
  }
  .capture-grande {
    width:100%; height:auto; display:block; border-radius:8px;
    border:1px solid var(--border);
  }
  .boutons-volets { display:flex; gap:12px; justify-content:center; }
  .choix-volet {
    height:40px; padding:0 18px; font-size:14px; font-weight:600;
    color:var(--ink); background:var(--surface);
    border:1px solid var(--border); border-radius:8px; cursor:pointer;
  }
  .choix-volet:hover { background:var(--sel); }
  .choix-volet.choisie {
    border-color:var(--accent);
    box-shadow:inset 0 0 0 1px var(--accent);
    background:var(--sel);
  }

  /* La fenêtre d'aperçu de l'étape 3, aux couleurs de la fiche. */
  .fenetre {
    display:flex; flex-direction:column; height:110px; border-radius:6px;
    border:1px solid var(--border); overflow:hidden;
  }
  .f-tete { height:14px; flex:none; border-bottom:1px solid var(--border); }
  .f-corps { display:flex; flex:1; min-height:0; }
  .f-nav { width:22%; flex:none; }
  .f-liste {
    flex:1; display:flex; flex-direction:column; gap:4px; padding:6px;
  }
  .f-liste span { height:10px; border-radius:3px; }
  .f-liste span:nth-child(2) { opacity:.85; }
  .f-lecture { width:38%; flex:none; border-left:1px solid var(--border); }

  /* Constat 4 (3e passe) : les trois récaps côte à côte, chacun en
     colonne (titre, miniature, valeur). */
  .recap { display:flex; gap:12px; align-items:stretch; }
  .ligne-recap {
    position:relative; flex:1; min-width:0; display:flex;
    flex-direction:column; align-items:flex-start; gap:10px;
    text-align:left; padding:14px 16px; background:var(--surface);
    border:1px solid var(--border); border-radius:10px; cursor:pointer;
  }
  .recap-titre { font-size:13px; font-weight:600; color:var(--ink); }
  .recap-valeur {
    font-size:13px; line-height:1.4; color:var(--ink2); min-width:0;
    overflow:hidden; overflow-wrap:anywhere;
  }
  /* Constat 4 (2e passe) : miniatures — la capture de la disposition,
     la fenêtre aux couleurs du thème. */
  .mini {
    width:100%; height:auto; flex:none; display:block;
    border-radius:4px; border:1px solid var(--border);
  }
  .mini-theme { width:100%; flex:none; display:block; }
  .mini-theme .fenetre { height:64px; border-radius:4px; }
  .mini-theme .f-tete { height:8px; }
  .mini-theme .f-liste { padding:3px; gap:2px; }
  .mini-theme .f-liste span { height:5px; border-radius:2px; }
  /* Le voile « Revenir à cette étape » : les règles du voile des
     pièces jointes (A70) — recouvrement absolu, fond --sel opaque,
     montré au survol ET au focus clavier (A8), géométrie stable. */
  .ligne-recap .voile {
    position:absolute; inset:0; display:none; align-items:center;
    justify-content:center; gap:6px; font-size:13px; font-weight:600;
    color:var(--ink); background:var(--sel); border-radius:9px;
    white-space:nowrap; overflow:hidden;
  }
  .ligne-recap:hover .voile, .ligne-recap:focus-visible .voile {
    display:inline-flex;
  }

  .marche { display:flex; align-items:center; gap:12px; }
  .principal {
    height:40px; padding:0 22px; font-size:14px; font-weight:600;
    color:var(--onAccent); background:var(--accent); border:none;
    border-radius:8px; cursor:pointer;
  }
  .principal:hover { background:var(--accentH); }
  .principal:disabled { opacity:.5; cursor:default; }
  .secondaire {
    height:40px; padding:0 18px; font-size:14px; color:var(--ink);
    background:var(--surface); border:1px solid var(--border);
    border-radius:8px; cursor:pointer; align-self:flex-start;
  }
  .secondaire:hover { background:var(--sel); }
</style>
