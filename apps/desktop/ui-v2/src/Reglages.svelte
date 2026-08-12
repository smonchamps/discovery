<script>
  // Surimpression Réglages en deux volets (A13) : à gauche le rail des
  // GROUPES (grammaire de la nav de l'écran 02 — rangées 36 px, état
  // actif = surface + bordure accent + ombre), à droite le contenu du
  // groupe choisi. Carte signature élargie à 800 px, en-tête 48 px et
  // pied « Terminé » inchangés. Le prototype est muet sur cette
  // surface : le Système complète (A6), l'écart s'inscrit au journal.
  //
  // Règle : un groupe ne s'expédie qu'avec du contenu RÉEL — aucun
  // réglage inventé pour meubler, aucun groupe vide.
  import { FICHES, appliquerTheme, themeActuel } from './lib/theme.js';
  import { activation } from './lib/clavier.js';
  import { appel } from './lib/transport.js';
  import GuichetCompte from './GuichetCompte.svelte';

  // A11 — la section « Comptes » : v1 offrait l'ajout à tout moment,
  // l'écran 01 ne vient qu'à zéro compte ; la porte permanente vit ici.
  let { comptes = [], onajoute = () => {} } = $props();

  const GROUPES = [
    { id: 'comptes', icone: 'person', libelle: 'Comptes' },
    { id: 'themes', icone: 'bookmark', libelle: 'Thèmes' },
    { id: 'raccourcis', icone: 'keyboard', libelle: 'Raccourcis' },
    { id: 'apropos', icone: 'info', libelle: 'À propos' },
  ];

  // La table D3, en RÉFÉRENCE seulement — pas de re-mappage.
  const RACCOURCIS = [
    { touche: 'c', geste: 'Écrire un nouveau message' },
    { touche: 'r', geste: 'Répondre à la conversation sélectionnée' },
    { touche: 'f', geste: 'Transférer la conversation sélectionnée' },
    { touche: 'e', geste: 'Archiver la conversation sélectionnée' },
    { touche: 'Suppr', geste: 'Supprimer la conversation sélectionnée' },
    { touche: '/', geste: 'Aller à la recherche' },
    { touche: 'Échap', geste: 'Fermer la surimpression, sortir du champ, revenir à la boîte' },
  ];

  let visible = $state(false);
  let groupe = $state('comptes');
  let actif = $state(themeActuel());
  let ajoutOuvert = $state(false);

  // À propos : la version se lit UNE fois (elle ne change pas en cours
  // de session) ; hors Tauri le rejet laisse le tiret — jamais un vide
  // silencieux qui ressemblerait à un oubli.
  let version = $state('');
  // null (repos) | 'controle' | 'ajour' | {version} | {erreur}
  let maj = $state(null);

  export function ouvrir() {
    actif = themeActuel();
    ajoutOuvert = false;
    groupe = 'comptes';
    maj = null;
    visible = true;
    if (!version) {
      appel('app_version')
        .then((v) => (version = v))
        .catch(() => (version = '—'));
    }
  }
  export function fermer() {
    visible = false;
  }
  export function estOuverte() {
    return visible;
  }
  function choisirGroupe(id) {
    groupe = id;
    ajoutOuvert = false;
  }
  function choisir(id) {
    appliquerTheme(id);
    actif = id;
  }

  // Le même flux que la fente d'avis (ADR 0013) : update_check en
  // silence, update_install ne rend pas la main en cas de succès.
  async function verifierMaj() {
    maj = 'controle';
    try {
      const info = await appel('update_check');
      maj = info ? { version: info.version } : 'ajour';
    } catch (err) {
      maj = { erreur: String(err) };
    }
  }
  async function installerMaj() {
    maj = 'installation';
    try {
      await appel('update_install');
    } catch (err) {
      maj = { erreur: String(err) };
    }
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
      <div class="milieu">
        <div class="rail" role="group" aria-label="Groupes de réglages">
          {#each GROUPES as g (g.id)}
            <div class="rang" class:actif={groupe === g.id}
                 data-testid="reglages-groupe" data-groupe={g.id}
                 role="button" tabindex="0" aria-current={groupe === g.id}
                 onclick={() => choisirGroupe(g.id)}
                 onkeydown={activation(() => choisirGroupe(g.id))}>
              <span class="ms icone" aria-hidden="true">{g.icone}</span>
              <span class="libelle">{g.libelle}</span>
            </div>
          {/each}
        </div>
        <div class="volet" data-testid="reglages-volet">
          {#if groupe === 'comptes'}
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
          {:else if groupe === 'themes'}
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
          {:else if groupe === 'raccourcis'}
            <p class="section">Raccourcis clavier</p>
            <div class="rangees" data-testid="reglages-raccourcis">
              {#each RACCOURCIS as r (r.touche)}
                <div class="raccourci">
                  <kbd>{r.touche}</kbd>
                  <span class="geste">{r.geste}</span>
                </div>
              {/each}
              <p class="note">Dans un champ de saisie, les lettres redeviennent
                des lettres — seul Échap garde un sens.</p>
            </div>
          {:else if groupe === 'apropos'}
            <p class="section">À propos</p>
            <div class="rangees" data-testid="reglages-apropos">
              <div class="ligne-apropos">
                <span class="cle">Version</span>
                <span class="valeur" data-testid="apropos-version">{version || '…'}</span>
              </div>
              <div class="ligne-apropos">
                <span class="cle">Mises à jour</span>
                <span class="valeur">
                  {#if maj === null}
                    <button type="button" class="ajouter" data-testid="apropos-verifier"
                            onclick={verifierMaj}>Vérifier les mises à jour</button>
                  {:else if maj === 'controle'}
                    Vérification…
                  {:else if maj === 'ajour'}
                    Vous êtes à jour.
                  {:else if maj === 'installation'}
                    Téléchargement et installation…
                  {:else if maj.version}
                    Version {maj.version} disponible.
                    <button type="button" class="ajouter" onclick={installerMaj}>
                      Installer</button>
                  {:else}
                    Vérification impossible — {maj.erreur}
                  {/if}
                </span>
              </div>
              <div class="ligne-apropos">
                <span class="cle">Icônes</span>
                <span class="valeur">Material Symbols Rounded (Google),
                  licence Apache 2.0 — police embarquée, aucun réseau.</span>
              </div>
            </div>
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
  /* Carte signature du prototype, élargie à 800 px (A13). La hauteur est
     POSÉE (640 px, bornée à l'écran) : le rail ne doit pas respirer au
     gré du groupe affiché. */
  .scrim {
    position:absolute; inset:0; background:var(--scrim); z-index:2;
    display:flex; align-items:center; justify-content:center; padding:36px;
  }
  .carte {
    width:800px; height:min(640px, 100%); background:var(--surface);
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

  .milieu { flex:1; display:flex; min-height:0; }

  /* Le rail : la grammaire de la nav de l'écran 02, réutilisée à
     l'identique — rangées 36 px, icône + libellé, actif = surface +
     bordure accent gauche + ombre. */
  .rail {
    width:220px; flex:none; background:var(--panel);
    border-right:1px solid var(--border); padding:20px 16px;
    display:flex; flex-direction:column; gap:4px; overflow:auto;
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

  .volet {
    flex:1; padding:22px; display:flex; flex-direction:column; gap:14px;
    overflow:auto; min-width:0;
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

  /* Raccourcis : référence en lecture seule, aux jetons. */
  .raccourci {
    display:flex; align-items:center; gap:14px; padding:8px 16px;
    font-size:13px; color:var(--ink2);
  }
  kbd {
    min-width:44px; padding:3px 8px; text-align:center; flex:none;
    font-family:inherit; font-size:12px; font-weight:600; color:var(--ink);
    background:var(--panel); border:1px solid var(--border);
    border-bottom-width:2px; border-radius:6px;
  }
  .geste { color:var(--ink2); }
  .note {
    margin:6px 0 0; padding:0 16px; font-size:12px; line-height:1.4;
    color:var(--muted);
  }

  /* À propos : clé / valeur, sans invention de forme. */
  .ligne-apropos {
    display:flex; align-items:baseline; gap:14px; padding:10px 16px;
    font-size:13px;
  }
  .cle { width:110px; flex:none; color:var(--muted); }
  .valeur {
    color:var(--ink); display:inline-flex; flex-wrap:wrap;
    align-items:center; gap:10px; min-width:0;
  }

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
