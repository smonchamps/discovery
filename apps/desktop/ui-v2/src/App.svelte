<script>
  // Écran 02 du prototype (A6) : entête 60 px, grille 236/400/1fr,
  // barre de statut 36 px. Données et actions RÉELLES par le port ;
  // recherche, Écrire et Réglages sont présents et inertes (D1 / P4).
  import { appel } from './lib/transport.js';
  import Nav from './Nav.svelte';
  import Liste from './Liste.svelte';
  import Lecture from './Lecture.svelte';
  import Conversation from './Conversation.svelte';
  import Toast from './Toast.svelte';

  let liste;
  let lecture;
  // La conversation REMPLACE l'écran (prototype) : elle se superpose en
  // plein écran, la boîte reste montée dessous — défilement, pages et
  // sélection sont intacts au retour.
  let conversation;

  let comptes = $state([]);
  let categorie = $state('reception');
  let compte = $state(null);
  let onglet = $state('tous');
  let totalListe = $state(0);
  let synchro = $state(null);
  let toast = $state(null);
  let toastMinuterie;

  const LIBELLES = {
    reception: 'Boîte de réception',
    envoyes: 'Envoyés',
    brouillons: 'Brouillons',
    indesirables: 'Indésirables',
    archives: 'Archives',
    corbeille: 'Corbeille',
  };

  const statut = $derived.by(() => {
    if (categorie !== 'reception') {
      return `${LIBELLES[categorie]} · ${totalListe} élément${totalListe > 1 ? 's' : ''}`;
    }
    if (synchro && synchro.percent !== null && synchro.percent < 100) {
      return `Synchronisation · ${synchro.percent} %`;
    }
    return 'Tous les messages sont à jour';
  });

  function flash(message) {
    toast = message;
    clearTimeout(toastMinuterie);
    toastMinuterie = setTimeout(() => (toast = null), 2200);
  }

  async function chargerNav() {
    try {
      comptes = await appel('nav_snapshot');
    } catch (err) {
      console.error('nav_snapshot :', err);
    }
  }
  chargerNav();
  setInterval(chargerNav, 10000);

  // Rattrapage des aperçus pour les corps écrits avant la colonne
  // `preview` : par lots, jamais sur le chemin d'ouverture ni au
  // défilement. Converge puis se tait ; la liste se rafraîchit une fois
  // la passe soldée pour montrer les aperçus rattrapés.
  async function rattraperApercus() {
    try {
      let restants = await appel('preview_catchup', { limit: 2000 });
      while (restants > 0) {
        await new Promise((r) => setTimeout(r, 250));
        restants = await appel('preview_catchup', { limit: 2000 });
      }
      liste?.recharger();
    } catch (err) {
      console.error('preview_catchup :', err);
    }
  }
  setTimeout(rattraperApercus, 1500);
  async function sonderSynchro() {
    try {
      synchro = await appel('sync_progress');
    } catch { /* hors ligne ou coeur occupé : le statut garde sa dernière valeur */ }
  }
  sonderSynchro();
  setInterval(sonderSynchro, 5000);

  function choisir(quoi) {
    if ('categorie' in quoi) {
      categorie = quoi.categorie;
      onglet = 'tous';
    }
    if ('compte' in quoi) compte = quoi.compte;
    lecture.fermer();
  }
  function surOnglet(id) {
    if (id === 'brouillons') {
      categorie = 'brouillons';
      return;
    }
    if (categorie === 'brouillons') categorie = 'reception';
    onglet = id;
    lecture.fermer();
  }

  function ouvrirConversation(ligne) {
    conversation.ouvrir(ligne);
  }
  function retourBoite() {
    conversation.fermer();
  }

  function surSelection(ligne) {
    lecture.ouvrir(ligne);
    if (ligne.thread_unseen > 0) {
      appel('mark_seen', {
        accountId: ligne.account_id,
        mailbox: ligne.mailbox,
        uid: ligne.uid,
        seen: true,
      })
        .then(() => {
          liste.marquerLue(ligne);
          chargerNav();
        })
        .catch((err) => console.error('mark_seen :', err));
    }
  }

  async function archiver(ligne) {
    try {
      await appel('archive_message', {
        accountId: ligne.account_id,
        mailbox: ligne.mailbox,
        uid: ligne.uid,
      });
      flash('Conversation archivée.');
      lecture.fermer();
      liste.recharger();
      chargerNav();
    } catch (err) {
      console.error('archive_message :', err);
    }
  }
  async function supprimer(ligne) {
    try {
      await appel('delete_message', {
        accountId: ligne.account_id,
        mailbox: ligne.mailbox,
        uid: ligne.uid,
      });
      flash('Conversation supprimée.');
      lecture.fermer();
      liste.recharger();
      chargerNav();
    } catch (err) {
      console.error('delete_message :', err);
    }
  }

  export function api() {
    return { liste, lecture };
  }
  export function marquerDemarrage() {
    const l = liste.etat();
    perf = `${l.total} conversations — première page servie+rendue en ${l.premierePageMs.toFixed(1)} ms`;
    startup = String(Math.round(performance.now()));
  }
  let perf = $state('démarrage…');
  let startup = $state('');
</script>

<div class="ecran">
  <header class="entete" data-testid="entete">
    <span class="marque">Discovery</span>
    <span class="recherche" data-testid="recherche">
      <span class="ms" aria-hidden="true">search</span>
      Chercher un message, une personne, un fichier</span>
    <button type="button" class="principal" data-testid="ecrire">
      <span class="ms" aria-hidden="true">edit_square</span>Écrire</button>
    <button type="button" data-testid="reglages">
      <span class="ms" aria-hidden="true">settings</span>Réglages</button>
  </header>

  <div class="colonnes">
    <Nav {comptes} {categorie} {compte} onchoisir={choisir} />
    <Liste bind:this={liste} {categorie} {compte} {onglet}
           onselect={surSelection} ononglet={surOnglet}
           ontotal={(t) => (totalListe = t)} />
    <Lecture bind:this={lecture} onarchiver={archiver} onsupprimer={supprimer}
             onconversation={ouvrirConversation} />
  </div>

  <div class="statut" data-testid="statut">
    <span>{statut}</span>
    <span id="perf" data-testid="perf" data-startup={startup}>{perf}</span>
  </div>

  <Conversation bind:this={conversation} onretour={retourBoite}
                onarchiver={async (l) => { await archiver(l); retourBoite(); }}
                onsupprimer={async (l) => { await supprimer(l); retourBoite(); }} />

  <Toast message={toast} />
</div>

<style>
  .ecran {
    display:flex; flex-direction:column; height:100vh; position:relative;
    background:var(--bg); overflow:hidden;
  }
  .entete {
    height:60px; flex:none; background:var(--surface);
    border-bottom:1px solid var(--border); display:flex;
    align-items:center; gap:20px; padding:0 24px;
  }
  .marque { font-size:15px; font-weight:600; width:212px; color:var(--ink); }
  .recherche {
    flex:1; height:32px; display:flex; align-items:center; gap:10px;
    padding:0 14px; font-size:13px; color:var(--muted);
    background:var(--bg); border:1px solid var(--border); border-radius:6px;
  }
  .recherche .ms { color:var(--muted); }
  button {
    height:32px; padding:0 16px; display:inline-flex; align-items:center;
    gap:8px; font-size:13px; color:var(--ink); background:var(--surface);
    border:1px solid var(--border); border-radius:6px; cursor:pointer;
  }
  button:hover { background:var(--sel); }
  .principal {
    font-weight:600; color:var(--onAccent); background:var(--accent);
    border-color:var(--accent);
  }
  .principal:hover { background:var(--accentH); border-color:var(--accentH); }

  .colonnes {
    flex:1; display:grid; grid-template-columns:236px 400px minmax(0,1fr);
    min-height:0;
  }

  .statut {
    height:36px; flex:none; background:var(--panel);
    border-top:1px solid var(--border); display:flex; align-items:center;
    justify-content:space-between; padding:0 24px;
    font-size:12px; color:var(--muted);
  }
  #perf { font-variant-numeric:tabular-nums; }
</style>
