<script>
  // Écran 02 du prototype (A6) : entête 60 px, grille 236/400/1fr
  // (236/1fr en deux volets — PLAN-VOLETS), barre de statut 36 px.
  // Données et actions RÉELLES par le port.
  // P5 : migration bloquante d'abord (ADR 0012), fente d'avis (au plus
  // UN), ligne de progression (au plus UNE), recherche câblée (D1),
  // raccourcis (D3).
  import { onMount } from 'svelte';
  import { appel } from './lib/transport.js';
  import { t, poserLangueDetectee } from './lib/texte.svelte.js';
  import { voletsActuels } from './lib/volets.svelte.js';
  import {
    largeurActuelle,
    reglerLargeur,
    persisterLargeurs,
    appliquerLargeur,
    defautLargeur,
    BORNES,
  } from './lib/largeurs.svelte.js';
  import { depuis, quandLong } from './lib/quand.js';
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
  import Hitofude from './Hitofude.svelte';
  import { fil, fermerFil, reduireFil, estEcho } from './lib/fil.svelte.js';

  let liste = $state(null);
  let lecture = $state(null);
  // La conversation REMPLACE l'écran (prototype) : elle se superpose en
  // plein écran, la boîte reste montée dessous — défilement, pages et
  // sélection sont intacts au retour.
  let conversation = $state(null);
  let composition = $state(null);
  let reglages = $state(null);
  let modaleMigration = $state(null);
  let champRecherche = $state(null);

  // Rien ne touche la base tant qu'une base héritée n'est pas adoptée :
  // les colonnes ne montent qu'après la modale de migration (ADR 0012).
  let prete = $state(false);

  let comptes = $state([]);
  // Les adresses actuellement CONNECTÉES (une session vit côté Rust) —
  // sous-ensemble du registre `comptes` ; la différence, ce sont les
  // comptes au jeton mort, que Réglages sait désormais reconnecter.
  let connectes = $state([]);
  // L'écran 01 ne s'affiche qu'une fois la nav CONNUE vide — jamais
  // pendant le premier chargement, sinon il clignoterait à chaque
  // démarrage.
  let navPrete = $state(false);
  let categorie = $state('reception');
  let compte = $state(null);
  let onglet = $state('tous');
  let recherche = $state('');
  let nResultats = $state(null);
  let nTotal = $state(null);
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

  // PLAN-VOLETS (V-D1) : le nombre de volets commande la grille ET la
  // surface d'ouverture — 3 : le volet de lecture ; 2 et 1 : l'écran
  // 03, plein écran (V-D2). La Lecture est DÉMONTÉE sous trois volets,
  // d'où les gardes `lecture?.` partout ; en un volet la Nav quitte la
  // grille et vit en TIROIR (E2). Au retour en trois volets, la
  // sélection courante rouvre son volet — l'écran ne revient pas vide
  // quand une ligne est encore choisie.
  const volets = $derived(voletsActuels());
  let voletsAvant = voletsActuels();

  // PLAN-RETOURS-V3 R3 (verdict CE D3) : les frontières de la grille se
  // règlent à la souris — nav|liste et liste|fil en trois volets,
  // nav|liste seule en deux. La poignée capture le pointeur : le
  // glissement suit hors de sa surface, l'iframe du fil ne l'avale
  // jamais ; le double-clic rend le défaut ; les flèches font le même
  // geste au clavier (A8), 16 px par pas. Les bornes vivent au module ;
  // le PLAFOND vit ici (la fenêtre est une connaissance d'UI) : en
  // trois volets, une frontière ne monte jamais au point d'écraser le
  // fil sous RESERVE_FIL — les bornes maximales cumulées (400 + 640)
  // dépassent la fenêtre par défaut, et une poignée poussée hors écran
  // serait irrécupérable (revue 2026-08-16). Le glissement RÈGLE (état
  // seul) ; le relâchement PERSISTE — jamais une écriture par
  // pointermove. La saisie se défait aussi sur pointercancel et
  // lostpointercapture (tactile, stylet, démontage du bloc en cours de
  // geste) : sans quoi elle resterait armée et le prochain survol
  // redimensionnerait sans bouton pressé.
  const lNav = $derived(largeurActuelle('nav'));
  const lListe = $derived(largeurActuelle('liste'));
  const RESERVE_FIL = 120;
  const plafondPoignee = (volet) =>
    volets === 3
      ? window.innerWidth -
        largeurActuelle(volet === 'nav' ? 'liste' : 'nav') -
        RESERVE_FIL
      : Infinity;
  let saisiePoignee = null; // { volet, x0, l0 } — hors $state : seul l'état du module bouge
  function saisirPoignee(volet, e) {
    if (e.button !== 0) return; // le bouton principal seul saisit
    saisiePoignee = { volet, x0: e.clientX, l0: largeurActuelle(volet) };
    e.currentTarget.setPointerCapture(e.pointerId);
  }
  function glisserPoignee(e) {
    if (!saisiePoignee) return;
    const { volet, x0, l0 } = saisiePoignee;
    reglerLargeur(volet, l0 + (e.clientX - x0), plafondPoignee(volet));
  }
  function relacherPoignee() {
    if (!saisiePoignee) return;
    saisiePoignee = null;
    persisterLargeurs();
  }
  function toucherPoignee(volet, e) {
    const pas = e.key === 'ArrowLeft' ? -16 : e.key === 'ArrowRight' ? 16 : 0;
    if (!pas) return;
    e.preventDefault();
    appliquerLargeur(volet, largeurActuelle(volet) + pas, plafondPoignee(volet));
  }
  $effect(() => {
    const v = volets;
    if (v === 3 && voletsAvant !== 3 && selectionnee && fil.cadre !== 'plein') lecture?.ouvrir(selectionnee);
    // Quitter le mode un volet emporte le tiroir — il n'a plus de sens.
    if (v !== 1) tiroirOuvert = false;
    voletsAvant = v;
  });

  // Le tiroir de nav (mode un volet) : surimpression sous scrim, la
  // Nav réutilisée telle quelle. Choisir un dossier ou un compte FERME
  // — le geste accompli n'a plus besoin du panneau ; Échap et le scrim
  // ferment aussi.
  let tiroirOuvert = $state(false);
  function choisirDuTiroir(quoi) {
    tiroirOuvert = false;
    choisir(quoi);
  }

  // --- Fente d'avis (§6) : au plus UN, priorité décroissante ----------
  // Les brouillons n'y vivent plus (PLAN-BROUILLONS) : ils sont en
  // liste — dossier Brouillons et mention sur le fil.
  let avisEnvoi = $state(null);
  let avisConnexion = $state(null);
  let avisMaj = $state(null);
  let avisCrash = $state(null);
  let avisTelemetrie = $state(null);
  // R2 (PLAN-RETOURS-6, D2) : l'envoi programmé se voit et s'annule
  // d'ici — informatif, donc DERNIER de la priorité (un incident prime).
  let avisProgramme = $state(null);
  const avis = $derived(
    avisEnvoi ?? avisConnexion ?? avisMaj ?? avisCrash ?? avisTelemetrie ?? avisProgramme,
  );

  // --- Ligne de progression (§6) : au plus UNE ------------------------
  let envoisEnAttente = $state(0);
  // R2 : les envois programmés pas encore échus, et la plus proche
  // échéance — séparés des « en attente » (eux n'attendent pas le
  // réseau, ils attendent leur heure).
  let envoisProgrammes = $state(0);
  let prochainProgramme = $state(null);
  let rattrapageApercus = $state(false);
  let rattrapageCorps = $state(null); // restant, ou null si rien à faire
  // R1 (PLAN-RETOURS-3, D1) : le % de corps déjà là sur le corpus en
  // portée — calculé par le cœur (`backfill_percent`), affiché dans le
  // TEXTE à côté du reste (A52 : le trait ne fait qu'une boucle).
  let rattrapagePct = $state(null);
  // Échec TOTAL de la dernière synchro : dans la ligne, pas la fente —
  // §6 n'y met pas la synchro, et « hors ligne » n'est pas un incident.
  let synchroEchec = $state(false);
  // E3 : l'échec PARTIEL se dit — « 1 compte sur 2 injoignable ».
  // `synchroEchec` ne couvrait que la panne totale : un compte mort sur
  // deux était invisible, et l'horodatage rajeuni par le survivant.
  let synchroPartiel = $state(null);
  // P0-bis : l'état réseau de l'OS, remonté par la WebView quasi
  // instantanément (l'équivalent de l'observer réseau de Thunderbird) —
  // au lieu d'attendre qu'un cycle cale sur le timeout socket (120 s)
  // pour comprendre qu'on est hors ligne. `navigator.onLine` peut mentir
  // (Wi-Fi sans internet), mais le terrain a montré le cas qui compte :
  // câble/Wi-Fi coupé, où il bascule juste.
  let enLigne = $state(navigator.onLine);
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

  // La ligne de statut ENTIÈRE — texte, trait hitofude (A52 : la boucle
  // `fil` dès qu'une action tourne, plein au repos `trait` ; le mode « au
  // pourcentage » est mort, le % vit dans le TEXTE), point d'alerte —
  // sort d'une seule décision : les trois ne peuvent pas diverger. Au
  // plus UNE progression (Système A4) ; priorités
  // re-triées par la sincérité (PLAN-SYNCHRO E1) : le cycle courant
  // d'abord — c'est lui que l'utilisateur attend — puis l'intégrale,
  // les rattrapages, l'attente d'envoi, l'échec, le repos horodaté.
  const ligne = $derived.by(() => {
    if (nResultats !== null) {
      // « N sur M » quand le rendu est plafonné (M > N), sinon « N résultats ».
      const texte =
        nTotal !== null && nTotal > nResultats
          ? t('statut.recherchePlafond', { n: nResultats, total: nTotal })
          : t('statut.recherche', { n: nResultats });
      return { texte, fil: null, alerte: false };
    }
    if (categorie !== 'reception') {
      // Un compte non PROUVÉ (null : la source n'a pas encore répondu,
      // PLAN-DEFILEMENT-PROFOND E2) ne s'affiche pas — le nom de la
      // boîte seul, jamais un « 0 éléments » d'attente.
      return {
        texte:
          totalListe === null
            ? t(LIBELLES[categorie])
            : t('statut.categorie', { boite: t(LIBELLES[categorie]), n: totalListe }),
        fil: null,
        alerte: false,
      };
    }
    // P0-bis : hors ligne, on le DIT — et tout de suite. Prime sur le
    // bloc synchro : un « Synchronisation… » ou un « à jour » serait
    // faux sans réseau. On vit sur le stock, et on dit depuis quand.
    if (!enLigne) {
      const derniere = synchro?.derniere ?? null;
      return {
        texte: derniere
          ? t('statut.horsLigneDepuis', { depuis: depuis(derniere, maintenant) })
          : t('statut.horsLigne'),
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
      // A52/D1 : le pourcentage de l'intégrale reste dans le TEXTE ; le
      // trait, lui, ne fait plus que boucler pendant qu'une action tourne.
      if (pct !== null) texte += ` · ${t('statut.pourcent', { p: pct })}`;
      return { texte, fil: true, alerte: false };
    }
    if (synchro && synchro.percent !== null && synchro.percent < 100) {
      return {
        texte: t('statut.synchro', { p: synchro.percent }),
        fil: true,
        alerte: false,
      };
    }
    if (rattrapageCorps !== null && rattrapageCorps > 0) {
      // Le % accompagne le reste (D1) ; garde-fou si le dénominateur
      // manquait (corpus vide — impossible quand il reste des corps).
      return {
        texte: rattrapagePct !== null
          ? t('statut.rattrapageCorps', { n: rattrapageCorps, p: rattrapagePct })
          : t('statut.rattrapageCorpsSeul', { n: rattrapageCorps }),
        fil: true,
        alerte: false,
      };
    }
    if (rattrapageApercus) {
      return { texte: t('statut.rattrapageApercus'), fil: true, alerte: false };
    }
    if (envoisEnAttente > 0) {
      // Un envoi en file est une action en cours (A52) : le trait boucle
      // jusqu'à la vidange. Hors ligne est capté plus haut — le trait ne
      // tourne jamais dans le vide.
      return { texte: t('statut.envois', { n: envoisEnAttente }), fil: true, alerte: false };
    }
    if (envoisProgrammes > 0 && prochainProgramme !== null) {
      // R2 : un programmé n'attend pas le réseau, il attend son heure —
      // un état de repos daté, pas un trait qui boucle.
      return {
        texte: t('statut.programmes', {
          n: envoisProgrammes,
          quand: quandLong(prochainProgramme),
        }),
        fil: null,
        alerte: false,
      };
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
    // A28/A29 : aux états « À jour », le trait hitofude précède le
    // texte — complètement dessiné et immobile (il ne boucle que
    // pendant un cycle, dans le bouton de relève).
    return {
      texte: derniere
        ? t('statut.ajourDepuis', { depuis: depuis(derniere, maintenant) })
        : t('statut.ajour'),
      fil: null,
      alerte: false,
      trait: true,
    };
  });

  function flash(message) {
    toast = message;
    clearTimeout(toastMinuterie);
    toastMinuterie = setTimeout(() => (toast = null), 2200);
  }

  // Garde de génération (même motif que Liste.svelte) : deux instantanés
  // en vol peuvent se résoudre dans le désordre depuis que les commandes
  // vivent hors de la pompe (PLAN-GELS) — sans elle, le plus VIEUX
  // écraserait le frais et la pastille de non-lus remonterait seule.
  let jetonNav = 0;
  async function chargerNav() {
    const jeton = ++jetonNav;
    try {
      const instantane = await appel('nav_snapshot');
      if (jeton !== jetonNav) return;
      comptes = instantane;
      navPrete = true;
    } catch (err) {
      console.error('nav_snapshot :', err);
    }
  }

  // Rattrapage des aperçus pour les corps écrits avant la colonne
  // `preview` : par lots, jamais sur le chemin d'ouverture ni au
  // défilement. Converge puis se tait ; la liste se rafraîchit une fois
  // la passe soldée pour montrer les aperçus rattrapés. Lot de 500
  // (PLAN-GELS D2) : à 2 000 corps (~130 Mo lus, mesure du 2026-08-15)
  // la transaction d'écriture s'allongeait — le verrou court protège les
  // gestes UI concurrents du BUSY (leçon delete_draft).
  async function rattraperApercus() {
    try {
      let restants = await appel('preview_catchup', { limit: 500 });
      rattrapageApercus = restants > 0;
      while (restants > 0) {
        await new Promise((r) => setTimeout(r, 250));
        restants = await appel('preview_catchup', { limit: 500 });
      }
      liste?.recharger();
    } catch (err) {
      console.error('preview_catchup :', err);
    } finally {
      rattrapageApercus = false;
    }
  }
  // E4 : la génération de courrier, monotone — bumpée par toute relève
  // INBOX qui a rapporté (cycle, bouton, veilleur IDLE). Quand elle
  // bouge au repos, c'est un veilleur qui a relevé : la liste se
  // recharge au battement de cette sonde (5 s), sans canal neuf (R0-S5).
  let generationVue = null;
  async function sonderSynchro() {
    try {
      synchro = await appel('sync_progress');
      const generation = synchro?.generation ?? null;
      if (generation !== null) {
        if (generationVue !== null && generation !== generationVue) {
          liste?.recharger();
          chargerNav();
          // E4 (PLAN-REACTIVITE) : la génération a bougé — un lot vient
          // d'entrer. Ses corps sont déjà là (relève, R-D2) SAUF s'il a
          // débordé la borne : la pompe couvre le débordement et le
          // stock, gardée en réentrance — un no-op quand tout est là.
          rattraperCorps();
        }
        generationVue = generation;
      }
    } catch { /* hors ligne ou coeur occupé : le statut garde sa dernière valeur */ }
  }

  // Rattrapage des corps (v1 l'avait en bandeau ; ici la ligne de
  // progression) : un lot à la fois, arrêt franc si un lot ne rapporte
  // rien — hors ligne, la boucle ne tourne pas à vide. Gardée en
  // réentrance (E4) : la génération peut l'amorcer pendant qu'une passe
  // court déjà — une pompe à la fois, le verrou shell n'empile rien.
  let corpsEnCours = false;
  async function rattraperCorps() {
    if (corpsEnCours) return;
    corpsEnCours = true;
    try {
      const etat = await appel('backfill_status');
      if (etat.remaining === 0) return;
      rattrapageCorps = etat.remaining;
      rattrapagePct = etat.percent;
      let restant = etat.remaining;
      while (restant > 0) {
        const bilan = await appel('backfill_bodies');
        restant = bilan.remaining;
        rattrapageCorps = restant;
        rattrapagePct = bilan.percent;
        if (bilan.fetched === 0) break;
        // E4 : les aperçus rattrapés se montrent au fil des lots — la
        // resservie est invisible depuis E1, plus besoin d'attendre une
        // recharge fortuite.
        liste?.recharger();
      }
    } catch (err) {
      console.error('backfill_bodies :', err);
    } finally {
      corpsEnCours = false;
      rattrapageCorps = null;
      rattrapagePct = null;
    }
  }

  // --- Sources de la fente d'avis (§6), par priorité ------------------

  // 1. Échec d'envoi — corollaire UI des règles d'or : JAMAIS invisible.
  //    L'attente non fautive (queued) vit dans la ligne de progression.
  // R2 : le départ d'un envoi programmé. La sonde (10 s) réarme cette
  // minuterie courte quand l'échéance approche (< 60 s) — à l'heure
  // dite, UNE vidange part. Jamais de minuterie longue : une annulation
  // entre-temps est vue par la sonde suivante, qui désarme.
  let minuterieProgramme = null;
  function armerDepart(echeance) {
    clearTimeout(minuterieProgramme);
    minuterieProgramme = null;
    if (echeance === null) return;
    const delai = Math.max(0, echeance * 1000 - Date.now()) + 1000;
    if (delai > 60000) return; // la sonde suivante réarmera, plus près
    minuterieProgramme = setTimeout(async () => {
      try {
        const bilan = await appel('flush_outbox');
        if (bilan.sent > 0) {
          // L'écho Envoyés est né à la vidange (E3) — la copie se
          // montre sans attendre ; la réconciliation suivra au cycle.
          liste?.recharger();
          chargerNav();
        }
      } catch (err) {
        console.error('flush_outbox (départ programmé) :', err);
      }
      sonderEnvois();
    }, delai);
  }

  async function sonderEnvois() {
    try {
      const etat = await appel('outbox_status');
      envoisEnAttente = etat.queued;
      envoisProgrammes = etat.scheduled ?? 0;
      prochainProgramme = etat.next_scheduled_epoch ?? null;
      armerDepart(prochainProgramme);
      // R2/D2 : le plus proche programmé se voit dans la fente, avec
      // son geste d'annulation (retour en brouillon — réversible).
      const programme = etat.entries.find((e) => e.send_at_epoch != null);
      avisProgramme = programme
        ? {
            icone: 'schedule_send',
            texte: t('avis.programme', {
              sujet: programme.subject,
              quand: quandLong(programme.send_at_epoch),
            }),
            actions: [
              { libelle: t('action.annulerEnvoi'), principale: true, faire: async () => {
                try {
                  const brouillon = await appel('outbox_cancel_scheduled', { id: programme.id });
                  flash(brouillon !== null ? t('toast.envoiAnnule') : t('erreur.annulerTard'));
                } catch (err) {
                  flash(t('erreur.annulerEnvoi', { err }));
                }
                sonderBrouillons();
                sonderEnvois();
              } },
            ],
          }
        : null;
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
  // Jeton de fraîcheur : deux sondes peuvent voler ensemble (celle du
  // geste et la périodique) — seule la DERNIÈRE émise a le droit de
  // servir, sinon une réponse périmée ressert un brouillon supprimé.
  let sondeBrouillons = 0;
  async function sonderBrouillons() {
    const mienne = ++sondeBrouillons;
    try {
      const lignes = await appel('list_drafts');
      if (mienne === sondeBrouillons) brouillons = lignes;
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
      // Les adresses qui tiennent une session : Réglages > Comptes en
      // dérive l'état par compte — un jeton mort se VOIT et se répare
      // sur place (« Reconnecter », constat terrain 2026-08-20).
      connectes = bilan.accounts.map((a) => a.email);
      if (bilan.problems.length > 0) {
        // Dire LEQUEL manque et pourquoi — une pastille absente sans
        // explication laisse l'utilisateur démuni (leçon v1).
        avisConnexion = {
          alerte: true,
          icone: 'link_off',
          texte: t('avis.connexion', { details: bilan.problems.join(' ; ') }),
          actions: [
            // Terrain 2026-08-20 : « Réessayer » rejouait la connexion
            // SILENCIEUSE — condamnée avec un jeton mort. La porte utile
            // est Réglages > Comptes, où l'état se voit et « Reconnecter »
            // relance le consentement (A63). L'avis reste affiché : il
            // tombera de lui-même à la reconnexion (connecter() le
            // reposera ou l'effacera).
            { libelle: t('entete.reglages'), principale: true, faire: () => {
              reglages?.ouvrir();
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
  // Cadences de synchro (PLAN-RETOURS-2, ADR 0021). Le cycle COMPLET
  // (inventaire + balayage des dossiers + fils + brouillons) est cher sur
  // un compte à beaucoup de dossiers ; depuis qu'IDLE (ADR 0018) tient
  // INBOX en temps réel, il tourne à 30 min. La passe LÉGÈRE (STATUS INBOX
  // seul) tourne à 5 min en filet — si un veilleur IDLE est tombé sans
  // s'être reconnecté, INBOX reste fraîche à 5 min près malgré tout.
  const CYCLE_COMPLET_MS = 30 * 60 * 1000;
  const PASSE_LEGERE_MS = 5 * 60 * 1000;
  // Le jeton interdit à la fin TARDIVE d'un cycle déclaré mort de
  // toucher l'état d'un cycle relancé depuis.
  let jetonCycle = 0;
  async function synchroniser() {
    if (enSynchro) return; // réentrance interdite : un cycle à la fois
    // P0-bis : hors ligne, un cycle automatique ne partirait que pour
    // caller sur les timeouts — la barre dit déjà « hors ligne », et le
    // retour du réseau relèvera. Le geste manuel, lui, force (voir
    // `relever`) : le clic est un ordre.
    if (!enLigne) return;
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
    // Hors ligne, seul le geste manuel (clic, `force`) tente encore —
    // le réveil de veille et le retour réseau attendent d'être en ligne.
    if (!force && !enLigne) return;
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
    const baseClaire = await modaleMigration.assurer();
    // La langue détectée au premier lancement se pose ICI, pas avant :
    // `lang_set` ouvre la base — avant la modale, il paierait l'adoption
    // en silence (ADR 0012, A41). Et seulement si la sonde de migration
    // a RÉPONDU : sinon cette écriture facultative serait elle-même la
    // première ouverture pleine. Attendue, pas tirée : au premier
    // lancement c'est elle qui crée le schéma — sérialisée avant la
    // flotte des sondes, comme quand elle vivait avant le montage.
    if (baseClaire) await poserLangueDetectee();
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
    // Le cycle complet à 30 min (IDLE tient INBOX, ADR 0018/0021),
    // et une passe légère INBOX à 5 min en filet contre un veilleur tombé.
    // La passe légère se sabre pendant un cycle (`enSynchro`) — jamais
    // deux relèves du même INBOX.
    setInterval(synchroniser, CYCLE_COMPLET_MS);
    setInterval(() => relever(false), PASSE_LEGERE_MS);
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
    // P0-bis : l'état réseau de l'OS, en direct. `offline` bascule la
    // barre à l'instant (plus d'attente du timeout 120 s) ; `online`
    // relève tout de suite — le courrier retenu pendant la coupure
    // arrive au retour, comme le fait Thunderbird. Événement, pas
    // sondage : c'est le seul moyen d'être aussi prompt que l'OS.
    window.addEventListener('offline', () => {
      enLigne = false;
      // E4 : les veilleurs IDLE dorment hors ligne — reconnecter en
      // boucle sans réseau ne servirait à rien.
      appel('reseau_etat', { enLigne: false }).catch(() => {});
    });
    window.addEventListener('online', () => {
      enLigne = true;
      // Le retour du réseau efface les reculs (côté shell) et réveille
      // les veilleurs ; la relève immédiate couvre le courrier retenu.
      appel('reseau_etat', { enLigne: true }).catch(() => {});
      relever(false);
      // R-D3 (E3) : les gestes joués hors ligne attendent — la passe
      // d'après-geste rejoue leurs actions et réconcilie leurs échos.
      // Sans travail, elle ne coûte aucune connexion.
      passeApresGeste(null);
    });
    // L'état initial : si l'app démarre hors ligne, les veilleurs le
    // savent tout de suite.
    appel('reseau_etat', { enLigne: navigator.onLine }).catch(() => {});
  });

  function choisir(quoi) {
    if ('categorie' in quoi) {
      categorie = quoi.categorie;
      onglet = 'tous';
    }
    if ('compte' in quoi) compte = quoi.compte;
    recherche = '';
    selectionnee = null;
    fermerFil();
  }
  function surOnglet(id) {
    if (id === 'brouillons') {
      categorie = 'brouillons';
      return;
    }
    if (categorie === 'brouillons') categorie = 'reception';
    onglet = id;
    selectionnee = null;
    fermerFil();
  }

  // --- Raccourcis (D3) : c / r / f / e / Suppr / « / » / Échap --------
  // Dans un champ de saisie, les lettres redeviennent des lettres — seul
  // Échap garde un sens (sortir du champ, sans jeter le brouillon).
  // s (étoile) et v (déplacer) suivent D2 : coupés à la bascule.
  function surTouche(event) {
    if (event.ctrlKey || event.metaKey || event.altKey) return;
    // L'éditeur riche du composeur (PLAN-COMPOSITION-HTML) est un
    // contenteditable : ni input ni textarea, mais une SAISIE — sans
    // `isContentEditable`, taper « c », « e » ou Suppr dans le corps
    // déclenchait les raccourcis globaux (Suppr supprimait la
    // conversation sélectionnée pendant la frappe — vu à l'e2e).
    const saisie = event.target instanceof HTMLInputElement
      || event.target instanceof HTMLTextAreaElement
      || event.target.isContentEditable;
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
        if (selectionnee) avancerApres(selectionnee, archiver);
        break;
      case 'Delete':
        if (selectionnee) avancerApres(selectionnee, supprimer);
        break;
      case '/':
        champRecherche?.focus();
        break;
      case 'Escape':
        if (composition?.estOuverte()) composition.fermer();
        else if (reglages?.estOuverte()) reglages.fermer();
        else if (conversation?.estOuverte()) retourBoite();
        else if (tiroirOuvert) tiroirOuvert = false;
        else if (recherche) recherche = '';
        else return;
        break;
      default:
        return;
    }
    event.preventDefault();
  }

  // E3 (PLAN-REACTIVITE) : un écho local est une copie en cours de
  // synchronisation — le geste dessus attend la réconciliation (une
  // fenêtre de quelques secondes), et le toast le dit au lieu d'un
  // échec silencieux. La ligne se reconnaît à sa boîte synthétique.
  function gesteSurEcho(ligne) {
    if (!estEcho(ligne)) return false;
    flash(t('toast.echoAttente'));
    return true;
  }
  // E3 : la réconciliation court derrière le geste — le serveur suit,
  // l'écho s'efface sous sa vraie ligne (invisible à l'œil, E1). Le
  // bilan dit tout : incidents en console, courrier/balayage → liste
  // resservie. `accountId: null` = tous les comptes qui ont du travail
  // (le déclencheur du retour en ligne, R-D3).
  function passeApresGeste(accountId) {
    appel('sync_apres_geste', { accountId })
      .then((bilan) => {
        for (const incident of bilan.errors) console.error('sync_apres_geste :', incident);
        if (bilan.fetched > 0 || bilan.deleted > 0 || bilan.reconcilies > 0 || bilan.balayes > 0) {
          chargerNav();
          liste?.recharger();
        }
      })
      .catch((err) => console.error('sync_apres_geste :', err));
  }

  function ouvrirConversation(ligne) {
    // D4 (UI v3) : l'exclusivité des cadres vit au store (fil.cadre) —
    // agrandir est un changement de taille, jamais un rechargement.
    conversation.ouvrir(ligne);
  }
  function retourBoite() {
    reduireFil(volets === 3);
  }

  function ecrire() {
    composition.ouvrir('new');
  }
  function repondre(ligne) {
    if (gesteSurEcho(ligne)) return;
    composition.ouvrir('reply', ligne);
  }
  function repondreTous(ligne) {
    if (gesteSurEcho(ligne)) return;
    composition.ouvrir('reply_all', ligne);
  }
  function transferer(ligne) {
    if (gesteSurEcho(ligne)) return;
    composition.ouvrir('forward', ligne);
  }
  // Après une vidange : les compteurs (Envoyés) ont pu bouger.
  function apresEnvoi() {
    chargerNav();
  }
  // E2 (PLAN-REACTIVITE) : la relève ciblée d'Envoyés rapporte du
  // courrier — la copie est EN BASE, la liste se resert tout de suite
  // (le cas exact du constat 0.1.5 : on envoie, on regarde Envoyés).
  // E1 rend la resservie invisible ; la sonde de génération, bumpée par
  // la même relève, repassera derrière sans coût.
  function apresCourrierEnvoye() {
    chargerNav();
    liste?.recharger();
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
    fermerFil();
    chargerNav();
    liste?.recharger();
  }

  function marquerVue(ligne) {
    if (!(ligne.thread_unseen > 0)) return;
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
  function surSelection(ligne) {
    selectionnee = ligne;
    // V-D2 : en deux volets, l'ouverture EST l'écran 03 — qui sait
    // servir un message sans fil (écho compris). Le marquage lu ne
    // bouge pas : seule la surface de destination change.
    if (volets === 3) lecture.ouvrir(ligne);
    else conversation.ouvrir(ligne);
    marquerVue(ligne);
  }

  // archiver/supprimer disent leur succès : le triage clavier n'avance
  // que sur un geste ABOUTI — jamais sur un écho différé ni un échec.
  async function archiver(ligne) {
    if (gesteSurEcho(ligne)) return false;
    try {
      await appel('archive_message', {
        accountId: ligne.account_id,
        mailbox: ligne.mailbox,
        uid: ligne.uid,
      });
      flash(t('toast.archivee'));
      fermerFil();
      // L'écho de destination est DÉJÀ en base (même transaction que le
      // geste, E3) : la resservie le montre en Archives < 1 s — le
      // serveur suit par la passe, en silence.
      liste.recharger();
      chargerNav();
      passeApresGeste(ligne.account_id);
      return true;
    } catch (err) {
      console.error('archive_message :', err);
      return false;
    }
  }
  async function supprimer(ligne) {
    if (gesteSurEcho(ligne)) return false;
    try {
      await appel('delete_message', {
        accountId: ligne.account_id,
        mailbox: ligne.mailbox,
        uid: ligne.uid,
      });
      flash(t('toast.supprimee'));
      fermerFil();
      // Même mécanique qu'archiver : l'écho est en base, la Corbeille
      // le montre tout de suite, la passe réconcilie derrière.
      liste.recharger();
      chargerNav();
      passeApresGeste(ligne.account_id);
      return true;
    } catch (err) {
      console.error('delete_message :', err);
      return false;
    }
  }
  // R2 (PLAN-RETOURS-3) : signaler indésirable / le contraire. Même
  // mécanique optimiste qu'archiver/supprimer — disparition locale,
  // action MoveTo journalisée, le serveur suit. Le fil se ferme, la
  // liste et la nav se rafraîchissent, la passe réconcilie derrière.
  async function signalerSpam(ligne) {
    if (gesteSurEcho(ligne)) return false;
    try {
      await appel('report_spam', {
        accountId: ligne.account_id,
        mailbox: ligne.mailbox,
        uid: ligne.uid,
      });
      flash(t('toast.spamSignale'));
      fermerFil();
      liste.recharger();
      chargerNav();
      passeApresGeste(ligne.account_id);
      return true;
    } catch (err) {
      // Le seul échec attendu : le compte n'a pas de dossier indésirable.
      console.error('report_spam :', err);
      flash(t('erreur.spamImpossible'));
      return false;
    }
  }
  async function marquerLegitime(ligne) {
    if (gesteSurEcho(ligne)) return false;
    try {
      await appel('mark_not_spam', {
        accountId: ligne.account_id,
        mailbox: ligne.mailbox,
        uid: ligne.uid,
      });
      flash(t('toast.pasSpam'));
      fermerFil();
      liste.recharger();
      chargerNav();
      passeApresGeste(ligne.account_id);
      return true;
    } catch (err) {
      console.error('mark_not_spam :', err);
      return false;
    }
  }

  // Le triage clavier s'enchaîne (A38) : après e/Suppr, la ligne du
  // DESSOUS devient la sélection — capturée AVANT le geste (les lignes
  // glissent à la resservie) ; dernière ligne : rien n'avance. En trois
  // volets elle ouvre son volet comme au clic (vue, marquée lue) ; en
  // 2/1 volets elle s'allume seulement — l'écran 03 ne s'impose jamais
  // de lui-même. Conversation ouverte : le geste seul, comme avant. Le
  // geste à la souris (boutons des volets) ne bouge pas la sélection.
  async function avancerApres(ligne, geste) {
    // Constat terrain (2026-08-15) : le clic laisse le focus sur une
    // rangée ; la touche bascule le navigateur en modalité clavier et
    // l'anneau :focus-visible surgirait sur ce nœud RECYCLÉ (rangées
    // clées par index — il montre déjà une autre conversation) : des
    // traits d'accent sans signification. La sélection (liseré) dit la
    // position — le raccourci retire le focus de la rangée.
    if (document.activeElement instanceof HTMLElement) document.activeElement.blur();
    const suivante = conversation?.estOuverte() ? null : (liste?.suivante(ligne) ?? null);
    if (!(await geste(ligne)) || !suivante) return;
    liste?.selectionner(suivante);
    selectionnee = suivante;
    if (volets === 3) {
      lecture?.ouvrir(suivante);
      marquerVue(suivante);
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

<!-- Au rétrécissement de la fenêtre, la LISTE cède : des largeurs
     posées sur un grand écran ne doivent jamais écraser le fil sous sa
     réserve ni pousser une poignée hors de l'écran (revue 2026-08-16,
     même racine que le plafond des poignées). -->
<svelte:window onkeydown={surTouche}
               onresize={() => {
                 if (volets === 3) {
                   reglerLargeur('liste', lListe, plafondPoignee('liste'));
                   persisterLargeurs();
                 }
               }} />

<div class="ecran">
  <header class="entete" data-testid="entete">
    {#if volets === 1}
      <!-- Mode un volet (PLAN-VOLETS E2) : la nav vit en tiroir, le
           bouton l'ouvre — 32 px, la grammaire des boutons d'entête. -->
      <button type="button" class="btn-tiroir" data-testid="btn-tiroir"
              aria-label={t('nav.ouvrirTiroir')} aria-expanded={tiroirOuvert}
              onclick={() => (tiroirOuvert = true)}>
        <span class="ms" aria-hidden="true">menu</span></button>
    {/if}
    <!-- A30 : la marque sans tuile-enveloppe — le mot « Wind » (18 px)
         suivi du trait hitofude statique (A28), décalé de 3 px sous la
         ligne de base ; la mini-tuile reste aux contextes OS. -->
    <span class="marque" class:marque--libre={volets === 1}>Wind<Hitofude /></span>
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
    <div class="colonnes" class:colonnes--2={volets === 2}
         class:colonnes--1={volets === 1}
         style="--l-nav:{lNav}px; --l-liste:{lListe}px">
      {#if volets !== 1}
        <Nav {comptes} {categorie} {compte} onchoisir={choisir} />
      {/if}
      <Liste bind:this={liste} {categorie} {compte} {onglet} {recherche}
             {brouillons} onreprendre={reprendreBrouillon}
             onselect={surSelection} ononglet={surOnglet}
             ontotal={(t) => (totalListe = t)}
             onresultats={(n, total) => { nResultats = n; nTotal = total; }} />
      {#if volets === 3}
        <Lecture bind:this={lecture} {brouillons} onreprendre={reprendreBrouillon}
                 onarchiver={archiver} onsupprimer={supprimer}
                 onconversation={ouvrirConversation}
                 onrepondre={repondre} onrepondretous={repondreTous}
                 ontransferer={transferer}
                 onspam={signalerSpam} onnonspam={marquerLegitime}
                 estIndesirable={categorie === 'indesirables'} onflash={flash} />
      {/if}
      <!-- Les poignées (R3) : posées SUR les frontières de la grille,
           hors flux — la grille ne gagne pas de colonne. Le motif ARIA
           est le « window splitter » : separator focalisable,
           aria-valuenow — la règle Svelte ne le connaît pas. UN seul
           gabarit (revue 2026-08-16) : tout durcissement du geste vaut
           pour les deux frontières par construction. -->
      {#snippet poignee(volet, libelle, gauche)}
        <!-- svelte-ignore a11y_no_noninteractive_element_interactions, a11y_no_noninteractive_tabindex -->
        <div class="poignee" data-testid="poignee-{volet}" role="separator"
             aria-orientation="vertical" aria-label={libelle}
             tabindex="0" aria-valuemin={BORNES[volet][0]}
             aria-valuemax={BORNES[volet][1]} aria-valuenow={largeurActuelle(volet)}
             style="left:{gauche}px"
             onpointerdown={(e) => saisirPoignee(volet, e)}
             onpointermove={glisserPoignee} onpointerup={relacherPoignee}
             onpointercancel={relacherPoignee}
             onlostpointercapture={relacherPoignee}
             ondblclick={() => defautLargeur(volet)}
             onkeydown={(e) => toucherPoignee(volet, e)}></div>
      {/snippet}
      {#if volets !== 1}
        {@render poignee('nav', t('volets.poigneeNav'), lNav - 3)}
      {/if}
      {#if volets === 3}
        {@render poignee('liste', t('volets.poigneeListe'), lNav + lListe - 3)}
      {/if}
    </div>

    <div class="statut" data-testid="statut">
      <!-- A52 : le trait hitofude, à gauche de la ligne, porte son
           animation de boucle dès qu'une action tourne (`ligne.fil`) et
           reste plein et immobile au repos (`ligne.trait`). Le mode « au
           pourcentage » est mort : le % vit dans le TEXTE. La barre fine
           de 2 px est morte depuis A36. -->
      <span class="texte">
        {#if ligne.alerte}<span class="point-alerte" aria-hidden="true"></span>{/if}
        {#if ligne.fil}
          <Hitofude anime largeur={38} hauteur={9} />
        {:else if ligne.trait}
          <Hitofude largeur={38} hauteur={9} />
        {/if}
        <span data-testid="progression">{ligne.texte}</span>
      </span>
      <span id="perf" data-testid="perf" data-startup={startup}>{perf}</span>
      <!-- E3 : le geste vit à côté de l'information qu'il rafraîchit
           (S-D1, variante A). Inhibé pendant un cycle (le glyphe tourne :
           la machine travaille déjà) ; sur échec, il devient le levier
           au plus près de la panne. -->
      <!-- Le bouton garde son glyphe sync, immobile (A36 : l'animation
           vit dans le trait de la ligne, jamais ici). -->
      <button type="button" class="btn-statut" data-testid="btn-releve"
              disabled={enSynchro} onclick={() => relever(true)}>
        <span class="ms" aria-hidden="true">sync</span>
        {#if enSynchro}{t('action.synchronisation')}{:else if synchroEchec || synchroPartiel}{t('action.reessayer')}{:else}{t('action.synchroniser')}{/if}
      </button>
    </div>

    {#if volets === 1 && tiroirOuvert}
      <!-- Le tiroir (PLAN-VOLETS E2) : géométrie du prototype validé
           au GO — 268 px, en-tête 60 px (tuile de marque + fermer), la
           Nav réutilisée TELLE QUELLE. Le scrim est un bouton : le
           clic ferme, le clavier aussi (A8). -->
      <button type="button" class="scrim-tiroir" data-testid="tiroir-scrim"
              aria-label={t('nav.fermerTiroir')}
              onclick={() => (tiroirOuvert = false)}></button>
      <div class="tiroir" data-testid="tiroir" role="dialog" aria-modal="true"
           aria-label={t('nav.aria')}>
        <div class="tete-tiroir">
          Wind<Hitofude />
          <button type="button" class="btn-tiroir fermer-tiroir" data-testid="tiroir-fermer"
                  aria-label={t('nav.fermerTiroir')}
                  onclick={() => (tiroirOuvert = false)}>
            <span class="ms" aria-hidden="true">close</span></button>
        </div>
        <Nav {comptes} {categorie} {compte} onchoisir={choisirDuTiroir} />
      </div>
    {/if}

    <Conversation bind:this={conversation} {brouillons}
                  onreprendre={reprendreBrouillon} onretour={retourBoite}
                  onarchiver={async (l) => { await archiver(l); retourBoite(); }}
                  onsupprimer={async (l) => { await supprimer(l); retourBoite(); }}
                  onrepondre={repondre} onrepondretous={repondreTous}
                  ontransferer={transferer}
                  onspam={async (l) => { await signalerSpam(l); retourBoite(); }}
                  onnonspam={async (l) => { await marquerLegitime(l); retourBoite(); }}
                  estIndesirable={categorie === 'indesirables'}
                  onecrire={ecrire}
                  onflash={flash} />

    {#if navPrete && comptes.length === 0}
      <Onboarding onajoute={compteAjoute} />
    {/if}

    <Composition bind:this={composition} {comptes} {compte}
                 onflash={flash} onenvoye={apresEnvoi}
                 oncourrier={apresCourrierEnvoye}
                 onbrouillon={sonderBrouillons} />
    <Reglages bind:this={reglages} {comptes} {connectes} onajoute={compteAjoute}
              onsupprime={compteRetire}
              onreconnecte={async () => { await connecter(); synchroniser(); }} />
  {/if}

  <ModaleMigration bind:this={modaleMigration} />

  <Toast message={toast} />
</div>

<style>
  .ecran {
    display:flex; flex-direction:column; height:100vh; position:relative;
    background:var(--bg); overflow:hidden;
  }
  /* A30 : l'entête au jeton des panneaux, la recherche sur blanc.
     UI v3, E4 (verdict CE 2026-08-16) : le gabarit de la maquette
     Classique — 52 px, gouttières 14/12, recherche bornée à 520 px. */
  .entete {
    height:52px; flex:none; background:var(--panel);
    border-bottom:1px solid var(--border); display:flex;
    align-items:center; gap:12px; padding:0 14px;
  }
  .marque {
    font-size:18px; font-weight:600; width:212px; color:var(--ink);
    display:flex; align-items:center; gap:10px;
  }
  /* Le trait de la marque : décalé de 3 px sous la ligne de base (A28). */
  .marque :global(svg), .tete-tiroir :global(svg) { margin-top:3px; }
  .recherche {
    flex:1; max-width:520px; height:32px; display:flex; align-items:center; gap:10px;
    padding:0 14px; font-size:13px; color:var(--ink2);
    background:var(--surface); border:1px solid var(--border);
    border-radius:6px;
  }
  .recherche .ms { color:var(--ink2); }
  .recherche input {
    flex:1; font-size:13px; color:var(--ink); border:none; outline:none;
    background:transparent; min-width:0;
  }
  .recherche input::placeholder { color:var(--ink2); }
  /* La recherche est bornée (520 px) : les gestes d'entête tiennent la
     droite, comme au gabarit de la maquette. */
  .entete [data-testid="ecrire"] { margin-left:auto; }
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

  /* A29 : la nav des pistes vit à 248 px (236 avant la v2) — depuis
     R3 (PLAN-RETOURS-V3), 248 et 400 sont les DÉFAUTS : les largeurs
     vivent en variables, réglées à la poignée, bornées au module. */
  .colonnes {
    flex:1; display:grid;
    grid-template-columns:var(--l-nav, 248px) var(--l-liste, 400px) minmax(0,1fr);
    min-height:0; position:relative;
  }
  /* PLAN-VOLETS (V-D1) : en deux volets la liste prend la largeur —
     gabarit de ligne inchangé (V-D3), l'aperçu respire. En un volet
     (E2) la liste est seule : son filet droit n'a plus de voisin. */
  .colonnes--2 { grid-template-columns:var(--l-nav, 248px) minmax(0,1fr); }
  .colonnes--1 { grid-template-columns:minmax(0,1fr); }
  /* La poignée (R3) : 7 px à cheval sur le filet, hors flux ; au
     survol, à la saisie et au focus clavier, un trait d'accent de 2 px
     dit la frontière — la grille, elle, ne bouge pas d'un pixel. */
  .poignee {
    position:absolute; top:0; bottom:0; width:7px; z-index:1;
    cursor:col-resize; touch-action:none;
  }
  .poignee::after {
    content:''; position:absolute; top:0; bottom:0; left:2px; width:2px;
    background:transparent;
  }
  .poignee:hover::after, .poignee:active::after,
  .poignee:focus-visible::after { background:var(--accent); }
  .colonnes--1 > :global(.colonne) { border-right:none; }

  /* Le bouton du tiroir (E2) : 32 px, la grammaire des boutons
     d'entête ; la marque perd sa largeur de colonne en un volet. */
  .btn-tiroir {
    width:32px; height:32px; padding:0; flex:none; display:inline-flex;
    align-items:center; justify-content:center; color:var(--ink2);
    background:var(--surface); border:1px solid var(--border);
    border-radius:6px; cursor:pointer;
  }
  .btn-tiroir:hover { background:var(--sel); color:var(--ink); }
  .marque--libre { width:auto; }

  /* Le tiroir : surimpression 268 px sous scrim, au niveau des
     surimpressions (le scrim est un BOUTON — clic et clavier ferment). */
  .scrim-tiroir {
    position:absolute; inset:0; height:auto; padding:0; z-index:2;
    background:var(--scrim); border:none; border-radius:0; cursor:default;
  }
  .tiroir {
    position:absolute; top:0; bottom:0; left:0; width:268px; z-index:2;
    background:var(--panel); border-right:1px solid var(--border);
    box-shadow:var(--shadow); display:flex; flex-direction:column;
  }
  .tiroir > :global(nav) { flex:1; border-right:none; }
  .tete-tiroir {
    height:60px; flex:none; display:flex; align-items:center; gap:10px;
    padding:0 16px 0 20px; border-bottom:1px solid var(--border);
    font-size:18px; font-weight:600; color:var(--ink);
  }
  .fermer-tiroir { margin-left:auto; }

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
     les 36 px de la barre sans les forcer — cotes de la section
     « Barre d'état et synchronisation » du Système (la maquette,
     reversée, est morte au GO — DC-D4). */
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
  .point-alerte {
    width:7px; height:7px; border-radius:99px; background:var(--alert);
    flex:none;
  }
</style>
