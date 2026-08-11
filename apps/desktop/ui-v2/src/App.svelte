<script>
  // Écran 02 du prototype (A6) : entête 60 px, grille 236/400/1fr,
  // barre de statut 36 px. Données et actions RÉELLES par le port.
  // P5 : migration bloquante d'abord (ADR 0012), fente d'avis (au plus
  // UN), ligne de progression (au plus UNE), recherche câblée (D1),
  // raccourcis (D3).
  import { onMount } from 'svelte';
  import { appel } from './lib/transport.js';
  import Nav from './Nav.svelte';
  import Liste from './Liste.svelte';
  import Lecture from './Lecture.svelte';
  import Conversation from './Conversation.svelte';
  import Composition from './Composition.svelte';
  import Reglages from './Reglages.svelte';
  import Onboarding from './Onboarding.svelte';
  import FenteAvis from './FenteAvis.svelte';
  import ModaleMigration from './ModaleMigration.svelte';
  import Toast from './Toast.svelte';

  let liste;
  let lecture;
  // La conversation REMPLACE l'écran (prototype) : elle se superpose en
  // plein écran, la boîte reste montée dessous — défilement, pages et
  // sélection sont intacts au retour.
  let conversation;
  let composition;
  let reglages;
  let modaleMigration;
  let champRecherche = $state(null);

  // Rien ne touche la base tant qu'une base héritée n'est pas adoptée :
  // les colonnes ne montent qu'après la modale de migration (ADR 0012).
  let prete = $state(false);

  let comptes = $state([]);
  // L'écran 01 ne s'affiche qu'une fois la nav CONNUE vide — jamais
  // pendant le premier chargement, sinon il clignoterait à chaque
  // démarrage.
  let navPrete = $state(false);
  let categorie = $state('reception');
  let compte = $state(null);
  let onglet = $state('tous');
  let recherche = $state('');
  let nResultats = $state(null);
  let totalListe = $state(0);
  let synchro = $state(null);
  let toast = $state(null);
  let toastMinuterie;
  // La sélection courante, pour les raccourcis (D3) : r/f/e/Suppr
  // agissent sur elle.
  let selectionnee = $state(null);

  // --- Fente d'avis (§6) : au plus UN, priorité décroissante ----------
  let avisEnvoi = $state(null);
  let avisMaj = $state(null);
  let avisCrash = $state(null);
  let avisTelemetrie = $state(null);
  let avisBrouillons = $state(null);
  const avis = $derived(
    avisEnvoi ?? avisMaj ?? avisCrash ?? avisTelemetrie ?? avisBrouillons,
  );

  // --- Ligne de progression (§6) : au plus UNE ------------------------
  let envoisEnAttente = $state(0);
  let rattrapageApercus = $state(false);
  let rattrapageCorps = $state(null); // restant, ou null si rien à faire

  const LIBELLES = {
    reception: 'Boîte de réception',
    envoyes: 'Envoyés',
    brouillons: 'Brouillons',
    indesirables: 'Indésirables',
    archives: 'Archives',
    corbeille: 'Corbeille',
  };

  const statut = $derived.by(() => {
    if (nResultats !== null) {
      return `Recherche · ${nResultats} résultat${nResultats > 1 ? 's' : ''}`;
    }
    if (categorie !== 'reception') {
      return `${LIBELLES[categorie]} · ${totalListe} élément${totalListe > 1 ? 's' : ''}`;
    }
    // La ligne de progression : au plus UNE — synchro OU rattrapage,
    // puis l'attente non fautive de la boîte d'envoi.
    if (synchro && synchro.percent !== null && synchro.percent < 100) {
      return `Synchronisation · ${synchro.percent} %`;
    }
    if (rattrapageCorps !== null && rattrapageCorps > 0) {
      return `Rattrapage des messages · ${rattrapageCorps} restants`;
    }
    if (rattrapageApercus) {
      return 'Rattrapage des aperçus…';
    }
    if (envoisEnAttente > 0) {
      return `Boîte d'envoi · ${envoisEnAttente} envoi${envoisEnAttente > 1 ? 's' : ''} en attente`;
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
      navPrete = true;
    } catch (err) {
      console.error('nav_snapshot :', err);
    }
  }

  // Rattrapage des aperçus pour les corps écrits avant la colonne
  // `preview` : par lots, jamais sur le chemin d'ouverture ni au
  // défilement. Converge puis se tait ; la liste se rafraîchit une fois
  // la passe soldée pour montrer les aperçus rattrapés.
  async function rattraperApercus() {
    try {
      let restants = await appel('preview_catchup', { limit: 2000 });
      rattrapageApercus = restants > 0;
      while (restants > 0) {
        await new Promise((r) => setTimeout(r, 250));
        restants = await appel('preview_catchup', { limit: 2000 });
      }
      liste?.recharger();
    } catch (err) {
      console.error('preview_catchup :', err);
    } finally {
      rattrapageApercus = false;
    }
  }
  async function sonderSynchro() {
    try {
      synchro = await appel('sync_progress');
    } catch { /* hors ligne ou coeur occupé : le statut garde sa dernière valeur */ }
  }

  // Rattrapage des corps (v1 l'avait en bandeau ; ici la ligne de
  // progression) : un lot à la fois, arrêt franc si un lot ne rapporte
  // rien — hors ligne, la boucle ne tourne pas à vide.
  async function rattraperCorps() {
    try {
      const etat = await appel('backfill_status');
      if (etat.remaining === 0) return;
      rattrapageCorps = etat.remaining;
      let restant = etat.remaining;
      while (restant > 0) {
        const bilan = await appel('backfill_bodies');
        restant = bilan.remaining;
        rattrapageCorps = restant;
        if (bilan.fetched === 0) break;
      }
    } catch (err) {
      console.error('backfill_bodies :', err);
    } finally {
      rattrapageCorps = null;
    }
  }

  // --- Sources de la fente d'avis (§6), par priorité ------------------

  // 1. Échec d'envoi — corollaire UI des règles d'or : JAMAIS invisible.
  //    L'attente non fautive (queued) vit dans la ligne de progression.
  async function sonderEnvois() {
    try {
      const etat = await appel('outbox_status');
      envoisEnAttente = etat.queued;
      const probleme = etat.entries.find(
        (e) => e.state === 'interrupted' || e.state === 'rejected',
      );
      if (!probleme) {
        avisEnvoi = null;
        return;
      }
      const sort = probleme.state === 'rejected' ? 'a été refusé' : 'a été interrompu';
      avisEnvoi = {
        alerte: true,
        icone: 'error',
        texte: `L'envoi « ${probleme.subject} » ${sort}`
          + (probleme.error ? ` — ${probleme.error}` : '') + '.',
        actions: [
          { libelle: 'Renvoyer', principale: true, faire: async () => {
            await appel('outbox_requeue', { id: probleme.id }).catch((err) => flash(`Renvoi impossible : ${err}`));
            await appel('flush_outbox').catch(() => {});
            sonderEnvois();
          } },
          { libelle: 'Abandonner', faire: async () => {
            await appel('outbox_delete', { id: probleme.id }).catch((err) => flash(`Abandon impossible : ${err}`));
            sonderEnvois();
          } },
        ],
      };
    } catch { /* la prochaine sonde suffira */ }
  }

  // 2. Mise à jour signée (ADR 0013) : une vérification au démarrage,
  //    en silence — hors ligne, pas d'avis, pas de bruit.
  async function verifierMaj() {
    let maj;
    try {
      maj = await appel('update_check');
    } catch { return; }
    if (!maj) return;
    avisMaj = {
      icone: 'system_update_alt',
      texte: `Une mise à jour est disponible (version ${maj.version}).`,
      actions: [
        { libelle: 'Installer', principale: true, faire: async () => {
          avisMaj.texte = 'Téléchargement et installation…';
          avisMaj.actions = [];
          try {
            // L'application redémarre sur la version neuve : cet appel
            // ne rend pas la main en cas de succès.
            await appel('update_install');
          } catch (err) {
            verifierMaj();
            flash(`Mise à jour impossible : ${err}`);
          }
        } },
        { libelle: 'Plus tard', faire: () => { avisMaj = null; } },
      ],
    };
  }

  // 3 et 4. Télémétrie de crash (ADR 0014) : opt-in explicite, off par
  //    défaut, rapports locaux — rien n'est envoyé sans l'utilisateur.
  async function verifierTelemetrie() {
    try {
      const rapports = await appel('telemetry_pending');
      if (rapports > 0) {
        avisCrash = {
          icone: 'report',
          texte: `Discovery a rencontré un problème lors d'une session précédente `
            + `(${rapports} rapport(s) en attente). Rien n'est envoyé sans vous.`,
          actions: [
            { libelle: 'Ouvrir le dossier des rapports', principale: true, faire: async () => {
              await appel('telemetry_open_folder').catch((err) => flash(`Ouverture impossible : ${err}`));
            } },
            { libelle: 'Ignorer', faire: () => { avisCrash = null; } },
          ],
        };
      }
      const consentement = await appel('telemetry_consent_get');
      if (consentement === 'unset') {
        const trancher = async (enabled) => {
          avisTelemetrie = null;
          await appel('telemetry_consent_set', { enabled })
            .catch((err) => flash(`Préférence non enregistrée : ${err}`));
        };
        avisTelemetrie = {
          icone: 'volunteer_activism',
          texte: 'Aider à améliorer Discovery ? En cas de plantage, un rapport '
            + 'technique serait enregistré sur votre machine — jamais le contenu '
            + "de vos mails. Vous choisissez ensuite de l'envoyer.",
          actions: [
            { libelle: 'Activer', principale: true, faire: () => trancher(true) },
            { libelle: 'Non merci', faire: () => trancher(false) },
          ],
        };
      }
    } catch { /* pas de télémétrie disponible : pas d'avis, pas de bruit */ }
  }

  // 5. Brouillons en cours : reprendre où on s'était arrêté. Re-sondé
  //    comme le bandeau v1 — l'état persiste tant que le brouillon
  //    existe ; « Plus tard » l'éteint pour la session.
  let brouillonsIgnores = false;
  async function verifierBrouillons() {
    if (brouillonsIgnores) return;
    try {
      const brouillons = await appel('list_drafts');
      if (brouillons.length === 0) {
        avisBrouillons = null;
        return;
      }
      avisBrouillons = {
        icone: 'edit_note',
        texte: brouillons.length > 1
          ? `${brouillons.length} brouillons en cours.`
          : `Un brouillon en cours : « ${brouillons[0].subject || 'sans objet'} ».`,
        actions: [
          { libelle: 'Reprendre', principale: true, faire: () => {
            avisBrouillons = null;
            composition.ouvrirBrouillon(brouillons[0]);
          } },
          { libelle: 'Plus tard', faire: () => {
            brouillonsIgnores = true;
            avisBrouillons = null;
          } },
        ],
      };
    } catch { /* les brouillons reviendront à la prochaine session */ }
  }

  // Le démarrage, dans l'ordre qui protège : migration d'abord (rien ne
  // touche la base avant), puis les boucles — et les contrôles uniques.
  onMount(async () => {
    await modaleMigration.assurer();
    prete = true;
    chargerNav();
    setInterval(chargerNav, 10000);
    sonderSynchro();
    setInterval(sonderSynchro, 5000);
    sonderEnvois();
    setInterval(sonderEnvois, 10000);
    setTimeout(rattraperApercus, 1500);
    setTimeout(rattraperCorps, 3000);
    verifierMaj();
    verifierTelemetrie();
    verifierBrouillons();
    setInterval(verifierBrouillons, 10000);
  });

  function choisir(quoi) {
    if ('categorie' in quoi) {
      categorie = quoi.categorie;
      onglet = 'tous';
    }
    if ('compte' in quoi) compte = quoi.compte;
    recherche = '';
    selectionnee = null;
    lecture.fermer();
  }
  function surOnglet(id) {
    if (id === 'brouillons') {
      categorie = 'brouillons';
      return;
    }
    if (categorie === 'brouillons') categorie = 'reception';
    onglet = id;
    selectionnee = null;
    lecture.fermer();
  }

  // --- Raccourcis (D3) : c / r / f / e / Suppr / « / » / Échap --------
  // Dans un champ de saisie, les lettres redeviennent des lettres — seul
  // Échap garde un sens (sortir du champ, sans jeter le brouillon).
  // s (étoile) et v (déplacer) suivent D2 : coupés à la bascule.
  function surTouche(event) {
    if (event.ctrlKey || event.metaKey || event.altKey) return;
    const saisie = event.target instanceof HTMLInputElement
      || event.target instanceof HTMLTextAreaElement;
    if (saisie) {
      if (event.key === 'Escape') {
        if (event.target === champRecherche) recherche = '';
        event.target.blur();
      }
      return;
    }
    switch (event.key) {
      case 'c':
        ecrire();
        break;
      case 'r':
        if (selectionnee) repondre(selectionnee);
        break;
      case 'f':
        if (selectionnee) transferer(selectionnee);
        break;
      case 'e':
        if (selectionnee) archiver(selectionnee);
        break;
      case 'Delete':
        if (selectionnee) supprimer(selectionnee);
        break;
      case '/':
        champRecherche?.focus();
        break;
      case 'Escape':
        if (composition?.estOuverte()) composition.fermer();
        else if (reglages?.estOuverte()) reglages.fermer();
        else if (conversation?.estOuverte()) retourBoite();
        else if (recherche) recherche = '';
        else return;
        break;
      default:
        return;
    }
    event.preventDefault();
  }

  function ouvrirConversation(ligne) {
    conversation.ouvrir(ligne);
  }
  function retourBoite() {
    conversation.fermer();
  }

  function ecrire() {
    composition.ouvrir('new');
  }
  function repondre(ligne) {
    composition.ouvrir('reply', ligne);
  }
  function transferer(ligne) {
    composition.ouvrir('forward', ligne);
  }
  // Après une vidange : les compteurs (Envoyés) ont pu bouger.
  function apresEnvoi() {
    chargerNav();
  }
  // Porte simple (D4) : le compte est ajouté, la nav se recharge et
  // l'écran 01 s'efface de lui-même (comptes non vides).
  function compteAjoute() {
    flash('Compte ajouté.');
    chargerNav();
  }

  function surSelection(ligne) {
    selectionnee = ligne;
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

<svelte:window onkeydown={surTouche} />

<div class="ecran">
  <header class="entete" data-testid="entete">
    <span class="marque">Discovery</span>
    <span class="recherche" data-testid="recherche">
      <span class="ms" aria-hidden="true">search</span>
      <input type="text" bind:this={champRecherche} bind:value={recherche}
             data-testid="champ-recherche"
             placeholder="Chercher un message, une personne, un fichier"></span>
    <button type="button" class="principal" data-testid="ecrire" onclick={ecrire}>
      <span class="ms" aria-hidden="true">edit_square</span>Écrire</button>
    <button type="button" data-testid="reglages" onclick={() => reglages.ouvrir()}>
      <span class="ms" aria-hidden="true">settings</span>Réglages</button>
  </header>

  <FenteAvis {avis} />

  {#if prete}
    <div class="colonnes">
      <Nav {comptes} {categorie} {compte} onchoisir={choisir} />
      <Liste bind:this={liste} {categorie} {compte} {onglet} {recherche}
             onselect={surSelection} ononglet={surOnglet}
             ontotal={(t) => (totalListe = t)}
             onresultats={(n) => (nResultats = n)} />
      <Lecture bind:this={lecture} onarchiver={archiver} onsupprimer={supprimer}
               onconversation={ouvrirConversation}
               onrepondre={repondre} ontransferer={transferer} />
    </div>

    <div class="statut" data-testid="statut">
      <span data-testid="progression">{statut}</span>
      <span id="perf" data-testid="perf" data-startup={startup}>{perf}</span>
    </div>

    <Conversation bind:this={conversation} onretour={retourBoite}
                  onarchiver={async (l) => { await archiver(l); retourBoite(); }}
                  onsupprimer={async (l) => { await supprimer(l); retourBoite(); }}
                  onrepondre={repondre} ontransferer={transferer} onecrire={ecrire}
                  onflash={flash} />

    {#if navPrete && comptes.length === 0}
      <Onboarding onajoute={compteAjoute} />
    {/if}

    <Composition bind:this={composition} {comptes} {compte}
                 onflash={flash} onenvoye={apresEnvoi} />
    <Reglages bind:this={reglages} />
  {/if}

  <ModaleMigration bind:this={modaleMigration} />

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
  .recherche input {
    flex:1; font-size:13px; color:var(--ink); border:none; outline:none;
    background:transparent; min-width:0;
  }
  .recherche input::placeholder { color:var(--muted); }
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
