<script>
  // Écran 02 du prototype (A6) : entête 60 px, grille 236/400/1fr,
  // barre de statut 36 px. Données et actions RÉELLES par le port.
  // P5 : migration bloquante d'abord (ADR 0012), fente d'avis (au plus
  // UN), ligne de progression (au plus UNE), recherche câblée (D1),
  // raccourcis (D3).
  import { onMount } from 'svelte';
  import { appel } from './lib/transport.js';
  import { t } from './lib/texte.svelte.js';
  import { depuis } from './lib/quand.js';
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
  // Le cycle en cours, vu par la sonde d'activité (E1) : null au repos.
  let activite = $state(null);
  // L'heure qui fait vieillir « il y a N minutes » : re-cadencée toutes
  // les 30 s, sans que personne ne clique.
  let maintenant = $state(Date.now());
  let toast = $state(null);
  let toastMinuterie;
  // La sélection courante, pour les raccourcis (D3) : r/f/e/Suppr
  // agissent sur elle.
  let selectionnee = $state(null);

  // --- Fente d'avis (§6) : au plus UN, priorité décroissante ----------
  // Les brouillons n'y vivent plus (PLAN-BROUILLONS) : ils sont en
  // liste — dossier Brouillons et mention sur le fil.
  let avisEnvoi = $state(null);
  let avisConnexion = $state(null);
  let avisMaj = $state(null);
  let avisCrash = $state(null);
  let avisTelemetrie = $state(null);
  const avis = $derived(avisEnvoi ?? avisConnexion ?? avisMaj ?? avisCrash ?? avisTelemetrie);

  // --- Ligne de progression (§6) : au plus UNE ------------------------
  let envoisEnAttente = $state(0);
  let rattrapageApercus = $state(false);
  let rattrapageCorps = $state(null); // restant, ou null si rien à faire
  // Total constaté au départ de la passe : le dénominateur de la barre.
  let rattrapageTotal = $state(null);
  // Échec TOTAL de la dernière synchro : dans la ligne, pas la fente —
  // §6 n'y met pas la synchro, et « hors ligne » n'est pas un incident.
  let synchroEchec = $state(false);
  // E3 : l'échec PARTIEL se dit — « 1 compte sur 2 injoignable ».
  // `synchroEchec` ne couvrait que la panne totale : un compte mort sur
  // deux était invisible, et l'horodatage rajeuni par le survivant.
  let synchroPartiel = $state(null);
  // Le verdict d'un bilan de relève (cycle complet OU passe légère) :
  // panne totale, panne partielle, ou rien — une seule écriture, les
  // deux états ne peuvent pas diverger.
  function majEchecs(bilan) {
    synchroEchec = bilan.accounts === 0 && bilan.errors.length > 0;
    synchroPartiel = bilan.accounts_failed > 0 && bilan.accounts > 0
      ? { n: bilan.accounts_failed, m: bilan.accounts_failed + bilan.accounts }
      : null;
  }

  const LIBELLES = {
    reception: 'boite.reception',
    envoyes: 'boite.envoyes',
    brouillons: 'boite.brouillons',
    indesirables: 'boite.indesirables',
    archives: 'boite.archives',
    corbeille: 'boite.corbeille',
  };

  // La ligne de statut ENTIÈRE — texte, barre fine (A16), point
  // d'alerte — sort d'une seule décision : les trois ne peuvent pas
  // diverger. Au plus UNE progression (Système A4) ; priorités
  // re-triées par la sincérité (PLAN-SYNCHRO E1) : le cycle courant
  // d'abord — c'est lui que l'utilisateur attend — puis l'intégrale,
  // les rattrapages, l'attente d'envoi, l'échec, le repos horodaté.
  const ligne = $derived.by(() => {
    if (nResultats !== null) {
      return { texte: t('statut.recherche', { n: nResultats }), fil: null, alerte: false };
    }
    if (categorie !== 'reception') {
      return {
        texte: t('statut.categorie', { boite: t(LIBELLES[categorie]), n: totalListe }),
        fil: null,
        alerte: false,
      };
    }
    // Le cycle courant : plus jamais « à jour » pendant que la machine
    // travaille — et TOUT ce qu'on sait s'affiche (terrain 2026-08-13 :
    // « 2/2 · compte » figé 7 minutes pendant le balayage des dossiers).
    // Rang, compte, boîte courante ; le % de l'intégrale quand il
    // existe, car l'intégrale EST le cycle — le masquer était une
    // régression sur l'affichage d'avant E1. Barre déterminée avec le
    // %, balayage sinon.
    if (enSynchro) {
      const pct =
        synchro && synchro.percent !== null && synchro.percent < 100 ? synchro.percent : null;
      const parts = [t('statut.cyclePrefixe')];
      if (activite) {
        if (activite.total > 1) {
          parts.push(`${Math.min(activite.fait + 1, activite.total)}/${activite.total}`);
        }
        if (activite.compte) parts.push(activite.compte);
        // La boîte en clair, ou l'étape traduite (le shell n'envoie
        // qu'une clé — A15) : l'observation terrain doit pouvoir NOMMER
        // ce qui est long.
        if (activite.boite) parts.push(activite.boite);
        else if (activite.phase) parts.push(t(`statut.phase.${activite.phase}`));
      }
      let texte = `${parts.join(' · ')}…`;
      if (pct !== null) texte += ` · ${t('statut.pourcent', { p: pct })}`;
      return {
        texte,
        fil: pct !== null ? { mode: 'plein', pct } : { mode: 'vague' },
        alerte: false,
      };
    }
    if (synchro && synchro.percent !== null && synchro.percent < 100) {
      return {
        texte: t('statut.synchro', { p: synchro.percent }),
        fil: { mode: 'plein', pct: synchro.percent },
        alerte: false,
      };
    }
    if (rattrapageCorps !== null && rattrapageCorps > 0) {
      // Jamais 100 % tant qu'il reste quelque chose — la règle de
      // sync_percent, tenue aussi ici.
      const fil = rattrapageTotal > 0
        ? {
            mode: 'plein',
            pct: Math.min(99, Math.max(0,
              Math.floor(((rattrapageTotal - rattrapageCorps) * 100) / rattrapageTotal))),
          }
        : { mode: 'vague' };
      return { texte: t('statut.rattrapageCorps', { n: rattrapageCorps }), fil, alerte: false };
    }
    if (rattrapageApercus) {
      return { texte: t('statut.rattrapageApercus'), fil: { mode: 'vague' }, alerte: false };
    }
    if (envoisEnAttente > 0) {
      return { texte: t('statut.envois', { n: envoisEnAttente }), fil: null, alerte: false };
    }
    // L'horodatage du prototype, enfin : « dernière synchronisation il
    // y a N minutes » — et sur échec, depuis quand on vit sur le stock.
    const derniere = synchro?.derniere ?? null;
    if (synchroEchec) {
      return {
        texte: derniere
          ? t('statut.synchroImpossibleDepuis', { depuis: depuis(derniere, maintenant) })
          : t('statut.synchroImpossible'),
        fil: null,
        alerte: true,
      };
    }
    // E3 : l'échec partiel — le courrier des comptes vivants est là,
    // mais un compte au moins est à sec, et ça se dit (alerte).
    if (synchroPartiel) {
      return {
        texte: t('statut.synchroPartielle', { n: synchroPartiel.n, m: synchroPartiel.m }),
        fil: null,
        alerte: true,
      };
    }
    return {
      texte: derniere
        ? t('statut.ajourDepuis', { depuis: depuis(derniere, maintenant) })
        : t('statut.ajour'),
      fil: null,
      alerte: false,
    };
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
      rattrapageTotal = etat.remaining;
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
      rattrapageTotal = null;
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
      const sort = probleme.state === 'rejected' ? 'avis.envoiRefuse' : 'avis.envoiInterrompu';
      avisEnvoi = {
        alerte: true,
        icone: 'error',
        texte: t(sort, {
          sujet: probleme.subject,
          erreur: probleme.error ? ` — ${probleme.error}` : '',
        }),
        actions: [
          { libelle: t('action.renvoyer'), principale: true, faire: async () => {
            await appel('outbox_requeue', { id: probleme.id }).catch((err) => flash(t('erreur.renvoi', { err })));
            await appel('flush_outbox').catch(() => {});
            sonderEnvois();
          } },
          { libelle: t('action.abandonner'), faire: async () => {
            await appel('outbox_delete', { id: probleme.id }).catch((err) => flash(t('erreur.abandon', { err })));
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
      texte: t('avis.maj', { version: maj.version }),
      actions: [
        { libelle: t('action.installer'), principale: true, faire: async () => {
          avisMaj.texte = t('reglages.installation');
          avisMaj.actions = [];
          try {
            // L'application redémarre sur la version neuve : cet appel
            // ne rend pas la main en cas de succès.
            await appel('update_install');
          } catch (err) {
            verifierMaj();
            flash(t('erreur.maj', { err }));
          }
        } },
        { libelle: t('action.plusTard'), faire: () => { avisMaj = null; } },
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
          texte: t('avis.crash', { n: rapports }),
          actions: [
            { libelle: t('action.ouvrirRapports'), principale: true, faire: async () => {
              await appel('telemetry_open_folder').catch((err) => flash(t('erreur.ouverture', { err })));
            } },
            { libelle: t('action.ignorer'), faire: () => { avisCrash = null; } },
          ],
        };
      }
      const consentement = await appel('telemetry_consent_get');
      if (consentement === 'unset') {
        const trancher = async (enabled) => {
          avisTelemetrie = null;
          await appel('telemetry_consent_set', { enabled })
            .catch((err) => flash(t('erreur.preference', { err })));
        };
        avisTelemetrie = {
          icone: 'volunteer_activism',
          texte: t('avis.telemetrie'),
          actions: [
            { libelle: t('action.activer'), principale: true, faire: () => trancher(true) },
            { libelle: t('action.nonMerci'), faire: () => trancher(false) },
          ],
        };
      }
    } catch { /* pas de télémétrie disponible : pas d'avis, pas de bruit */ }
  }

  // --- Brouillons en liste (PLAN-BROUILLONS) --------------------------
  // Sondés comme le reste — le port reste du sondage (R0-S5), aucun
  // canal neuf. La sonde nourrit le dossier Brouillons ET la mention
  // sur les fils de la Réception ; le composeur la réveille à chaque
  // geste local (onbrouillon) pour que la liste ne traîne pas 10 s.
  let brouillons = $state([]);
  async function sonderBrouillons() {
    try {
      brouillons = await appel('list_drafts');
    } catch { /* la prochaine sonde suffira */ }
  }
  function reprendreBrouillon(brouillon) {
    composition.ouvrirBrouillon(brouillon);
  }

  // --- R1 : la colonne vertébrale de synchro (PLAN-RETRAIT-V1) --------
  // v1 déclenchait tout ; v2 devient autonome — reconnexion silencieuse
  // au démarrage, puis cycle AUTOMATIQUE (D5 : pas de bouton) : synchro,
  // vidange de la boîte d'envoi (le réseau est peut-être revenu — règle
  // d'or), reflet des brouillons. Séquence v1 conservée à l'identique.

  async function connecter() {
    try {
      const bilan = await appel('connect_accounts');
      if (bilan.problems.length > 0) {
        // Dire LEQUEL manque et pourquoi — une pastille absente sans
        // explication laisse l'utilisateur démuni (leçon v1).
        avisConnexion = {
          alerte: true,
          icone: 'link_off',
          texte: t('avis.connexion', { details: bilan.problems.join(' ; ') }),
          actions: [
            { libelle: t('action.reessayer'), principale: true, faire: async () => {
              avisConnexion = null;
              await connecter();
              synchroniser();
            } },
            { libelle: t('action.ignorer'), faire: () => { avisConnexion = null; } },
          ],
        };
      } else {
        avisConnexion = null;
      }
    } catch (err) {
      console.error('connect_accounts :', err);
    }
  }

  // $state : la barre d'état raconte le cycle pendant qu'il tourne (E1).
  let enSynchro = $state(false);
  // La sonde d'activité ne vit QUE pendant le cycle : à la seconde,
  // purement mémoire côté shell (atomiques) — elle ne coûte rien à la
  // boucle et rien au repos.
  async function sonderActivite() {
    try {
      activite = await appel('sync_activity');
    } catch { /* la prochaine sonde suffira */ }
  }
  // P0 (PLAN-SYNCHRO) : le watchdog du cycle. Les timeouts socket de
  // mail-imap achèvent un réseau qui cale ; le watchdog couvre ce qu'ils
  // ne voient pas. 5 min : au-dessus du plus long silence légitime
  // mesuré — l'intégrale du terrain avançait par lots de ~75 s, et
  // chaque lot bouge l'avancement (`synchro.local`), donc la signature.
  const IMMOBILITE_MAX = 5 * 60 * 1000;
  // Le jeton interdit à la fin TARDIVE d'un cycle déclaré mort de
  // toucher l'état d'un cycle relancé depuis.
  let jetonCycle = 0;
  async function synchroniser() {
    if (enSynchro) return; // réentrance interdite : un cycle à la fois
    enSynchro = true;
    const jeton = ++jetonCycle;
    sonderActivite();
    // Un cycle dont NI l'activité NI l'avancement ne bougent pendant
    // 5 min est déclaré mort : garde réarmée, échec affiché. Le watchdog
    // ne tue rien (une commande en vol ne s'annule pas) — il rend la
    // main ; c'est le timeout socket qui achève le thread gelé.
    let signature = '';
    let dernierMouvement = Date.now();
    // P1 : le courrier d'INBOX se montre PAR COMPTE, dès que le
    // compteur du cycle bouge — la liste n'attend plus la fin du cycle
    // complet. Lu à la sonde existante : le port reste du sondage
    // (R0-S5), aucun canal d'événements.
    let courrierVu = 0;
    const surveiller = async () => {
      await sonderActivite();
      if (activite && activite.courrier > courrierVu) {
        courrierVu = activite.courrier;
        liste?.recharger();
        chargerNav();
      }
      const trace = JSON.stringify([activite, synchro?.local]);
      if (trace !== signature) {
        signature = trace;
        dernierMouvement = Date.now();
        return;
      }
      if (Date.now() - dernierMouvement < IMMOBILITE_MAX) return;
      clearInterval(sonde);
      jetonCycle += 1;
      synchroEchec = true;
      activite = null;
      enSynchro = false;
      console.error('sync_inbox : aucun mouvement depuis 5 min, cycle déclaré mort (P0)');
    };
    const sonde = setInterval(surveiller, 1000);
    try {
      const bilan = await appel('sync_inbox');
      if (jeton !== jetonCycle) return; // déclaré mort entre-temps : trop tard
      majEchecs(bilan);
      // Le réseau est peut-être revenu : la boîte d'envoi retente sa
      // chance, puis les brouillons se reflètent (poussée + purge).
      await appel('flush_outbox').catch((err) => console.error('flush_outbox :', err));
      await appel('sync_drafts').catch(() => { /* hors ligne : le cycle suivant retentera */ });
      sonderEnvois();
      chargerNav();
      sonderBrouillons();
      if (bilan.fetched > 0 || bilan.deleted > 0) {
        liste?.recharger();
        rattraperCorps();
      }
    } catch (err) {
      if (jeton === jetonCycle) synchroEchec = true;
      console.error('sync_inbox :', err);
    } finally {
      clearInterval(sonde);
      if (jeton === jetonCycle) {
        activite = null;
        enSynchro = false;
        // L'horodatage vient d'être posé par le shell : le relire tout de
        // suite, sans attendre la sonde de 5 s.
        sonderSynchro();
      }
    }
  }

  // E3 : le geste manuel (D5 rouverte) — la passe légère : STATUS INBOX
  // par compte, relève seulement si ça a bougé (E2a), puis la boîte
  // d'envoi retente sa chance (le réseau est peut-être revenu — c'est
  // souvent POUR ça qu'on clique). Réponse en secondes, tenue par la
  // gate d'E2a ; chaque commande est bornée par les timeouts P0, la
  // passe se termine toujours — pas de watchdog dédié.
  // `force` : le clic est un ORDRE — il traverse le recul par compte
  // (anti-martèlement, shell) ; le réveil de veille, lui, le respecte.
  async function relever(force) {
    if (enSynchro) return; // le cycle travaille déjà — bouton inhibé
    enSynchro = true;
    const jeton = ++jetonCycle;
    sonderActivite();
    const sonde = setInterval(sonderActivite, 1000);
    try {
      const bilan = await appel('sync_inbox_light', { force: force === true });
      if (jeton !== jetonCycle) return;
      majEchecs(bilan);
      await appel('flush_outbox').catch((err) => console.error('flush_outbox :', err));
      sonderEnvois();
      chargerNav();
      if (bilan.fetched > 0 || bilan.deleted > 0) {
        liste?.recharger();
        rattraperCorps();
      }
    } catch (err) {
      if (jeton === jetonCycle) synchroEchec = true;
      console.error('sync_inbox_light :', err);
    } finally {
      clearInterval(sonde);
      if (jeton === jetonCycle) {
        activite = null;
        enSynchro = false;
        sonderSynchro();
      }
    }
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
    // « il y a N minutes » vieillit tout seul : 30 s suffisent pour une
    // granularité à la minute.
    setInterval(() => (maintenant = Date.now()), 30000);
    sonderEnvois();
    setInterval(sonderEnvois, 10000);
    setTimeout(rattraperApercus, 1500);
    setTimeout(rattraperCorps, 3000);
    verifierMaj();
    verifierTelemetrie();
    sonderBrouillons();
    setInterval(sonderBrouillons, 10000);
    // R1 — le cycle de synchro : APRÈS les premiers rendus (la liste est
    // utilisable avant, « enveloppes d'abord ») ; jamais bloquant.
    (async () => {
      await connecter();
      await synchroniser();
    })();
    setInterval(synchroniser, 300000);
    // E3 : le réveil de veille — un tick en retard de plusieurs minutes
    // signe une veille (saut d'horloge : les minuteries dorment avec la
    // machine), et c'est LE moment où l'utilisateur regarde l'écran. La
    // passe légère part aussitôt, sans attendre le prochain sondage à
    // 5 min. Aucune API système : la dérive d'horloge suffit.
    let dernierTic = Date.now();
    setInterval(() => {
      const tic = Date.now();
      const retard = tic - dernierTic;
      dernierTic = tic;
      if (retard > 120000) relever(false);
    }, 15000);
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
  function repondreTous(ligne) {
    composition.ouvrir('reply_all', ligne);
  }
  function transferer(ligne) {
    composition.ouvrir('forward', ligne);
  }
  // Après une vidange : les compteurs (Envoyés) ont pu bouger.
  function apresEnvoi() {
    chargerNav();
  }
  // Porte simple (D4) : le compte est ajouté, la nav se recharge,
  // l'écran 01 s'efface de lui-même — et la première synchro part
  // aussitôt (la session vient d'être posée par l'ajout).
  function compteAjoute() {
    flash(t('toast.compteAjoute'));
    chargerNav();
    synchroniser();
  }
  // Le pendant du retrait : le courrier du compte a quitté la base, donc
  // tout ce qui pouvait le montrer se replie — filtre de nav, sélection,
  // volet de lecture — avant de recharger nav et liste. À zéro compte,
  // l'écran 01 revient de lui-même (navPrete && comptes.length === 0).
  function compteRetire(id) {
    flash(t('toast.compteRetire'));
    if (compte === id) compte = null;
    selectionnee = null;
    lecture.fermer();
    chargerNav();
    liste?.recharger();
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
      flash(t('toast.archivee'));
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
      flash(t('toast.supprimee'));
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
    perf = t('statut.perf', { total: l.total, ms: l.premierePageMs.toFixed(1) });
    startup = String(Math.round(performance.now()));
  }
  let perf = $state(t('statut.demarrage'));
  let startup = $state('');
</script>

<svelte:window onkeydown={surTouche} />

<div class="ecran">
  <header class="entete" data-testid="entete">
    <span class="marque">Discovery</span>
    <span class="recherche" data-testid="recherche">
      <span class="ms" aria-hidden="true">search</span>
      <input type="text" bind:this={champRecherche} bind:value={recherche}
             data-testid="champ-recherche" aria-label={t('entete.recherche')}
             placeholder={t('entete.chercher')}>
      {#if recherche}
        <!-- Verdict terrain (Annexe A) : vider la recherche en UN clic. -->
        <button type="button" class="vider" data-testid="vider-recherche"
                aria-label={t('entete.effacerRecherche')}
                onclick={() => { recherche = ''; champRecherche?.focus(); }}>
          <span class="ms" aria-hidden="true">close</span></button>
      {/if}</span>
    <button type="button" class="principal" data-testid="ecrire" onclick={ecrire}>
      <span class="ms" aria-hidden="true">edit_square</span>{t('entete.ecrire')}</button>
    <button type="button" data-testid="reglages" onclick={() => reglages.ouvrir()}>
      <span class="ms" aria-hidden="true">settings</span>{t('entete.reglages')}</button>
  </header>

  <FenteAvis {avis} />

  {#if prete}
    <div class="colonnes">
      <Nav {comptes} {categorie} {compte} onchoisir={choisir} />
      <Liste bind:this={liste} {categorie} {compte} {onglet} {recherche}
             {brouillons} onreprendre={reprendreBrouillon}
             onselect={surSelection} ononglet={surOnglet}
             ontotal={(t) => (totalListe = t)}
             onresultats={(n) => (nResultats = n)} />
      <Lecture bind:this={lecture} onarchiver={archiver} onsupprimer={supprimer}
               onconversation={ouvrirConversation}
               onrepondre={repondre} onrepondretous={repondreTous}
               ontransferer={transferer} onflash={flash} />
    </div>

    <div class="statut" data-testid="statut">
      {#if ligne.fil}
        <div class="fil" data-testid="fil" aria-hidden="true">
          {#if ligne.fil.mode === 'plein'}
            <div class="plein" style:width="{ligne.fil.pct}%"></div>
          {:else}
            <div class="vague"></div>
          {/if}
        </div>
      {/if}
      <span class="texte">
        {#if ligne.alerte}<span class="point-alerte" aria-hidden="true"></span>{/if}
        <span data-testid="progression">{ligne.texte}</span>
      </span>
      <span id="perf" data-testid="perf" data-startup={startup}>{perf}</span>
      <!-- E3 : le geste vit à côté de l'information qu'il rafraîchit
           (S-D1, variante A). Inhibé pendant un cycle (le glyphe tourne :
           la machine travaille déjà) ; sur échec, il devient le levier
           au plus près de la panne. -->
      <button type="button" class="btn-statut" data-testid="btn-releve"
              disabled={enSynchro} onclick={() => relever(true)}>
        <span class="ms" class:tourne={enSynchro} aria-hidden="true">sync</span>
        {#if enSynchro}{t('action.synchronisation')}{:else if synchroEchec || synchroPartiel}{t('action.reessayer')}{:else}{t('action.synchroniser')}{/if}
      </button>
    </div>

    <Conversation bind:this={conversation} {brouillons}
                  onreprendre={reprendreBrouillon} onretour={retourBoite}
                  onarchiver={async (l) => { await archiver(l); retourBoite(); }}
                  onsupprimer={async (l) => { await supprimer(l); retourBoite(); }}
                  onrepondre={repondre} onrepondretous={repondreTous}
                  ontransferer={transferer} onecrire={ecrire}
                  onflash={flash} />

    {#if navPrete && comptes.length === 0}
      <Onboarding onajoute={compteAjoute} />
    {/if}

    <Composition bind:this={composition} {comptes} {compte}
                 onflash={flash} onenvoye={apresEnvoi}
                 onbrouillon={sonderBrouillons} />
    <Reglages bind:this={reglages} {comptes} onajoute={compteAjoute}
              onsupprime={compteRetire} />
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
  .vider {
    height:22px; width:22px; padding:0; display:inline-flex; flex:none;
    align-items:center; justify-content:center; color:var(--muted);
    background:transparent; border:none; border-radius:6px; cursor:pointer;
  }
  .vider:hover { color:var(--ink); background:var(--sel); }
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
    position:relative; height:36px; flex:none; background:var(--panel);
    border-top:1px solid var(--border); display:flex; align-items:center;
    gap:14px; padding:0 24px;
    font-size:12px; color:var(--muted);
  }
  #perf { font-variant-numeric:tabular-nums; flex:none; }
  .texte { display:flex; align-items:center; gap:8px; min-width:0; flex:1; }
  .texte span { overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  /* Le bouton de relève (E3, S-D1 variante A) : 26 px, il tient dans
     les 36 px de la barre sans les forcer — cotes de la maquette
     validée (docs/design/maquette-synchro.html). */
  .btn-statut {
    height:26px; padding:0 12px; display:inline-flex; align-items:center;
    gap:7px; font-size:12px; font-weight:600; color:var(--ink2);
    background:var(--surface); border:1px solid var(--border);
    border-radius:6px; cursor:pointer; flex:none;
  }
  .btn-statut:hover { background:var(--sel); color:var(--ink); }
  .btn-statut[disabled] { opacity:.55; cursor:default; }
  .btn-statut[disabled]:hover { background:var(--surface); color:var(--ink2); }
  .btn-statut .ms { font-size:14px; }
  .tourne { animation:tourne 1.2s linear infinite; }
  @keyframes tourne { from { transform:rotate(0); } to { transform:rotate(360deg); } }
  .point-alerte {
    width:7px; height:7px; border-radius:99px; background:var(--alert);
    flex:none;
  }
  /* La barre fine (A16) : 2 px au ras supérieur, par-dessus le filet —
     déterminée quand un % honnête existe, balayage sinon. */
  .fil {
    position:absolute; top:-1px; left:0; right:0; height:2px;
    overflow:hidden; pointer-events:none;
  }
  .fil .plein { height:100%; background:var(--accent); transition:width .3s; }
  .fil .vague {
    height:100%; width:28%; background:var(--accent); border-radius:2px;
    animation:vague 1.6s ease-in-out infinite;
  }
  @keyframes vague {
    from { transform:translateX(-110%); }
    to { transform:translateX(460%); }
  }
</style>
