<script>
  // Surimpression de composition du prototype : 860 px, trois modes
  // (nouveau / répondre / transférer), câblée aux flux réels.
  //
  // Préremplissages : formes du prototype (« Re : » / « Tr : », amorce
  // « Bonjour Prénom, ») ; la citation est RÉELLE — `reply_context` /
  // `forward_context` du cœur la préparent depuis le corps effectif.
  // Les pièces d'un transfert sont RÉELLES aussi (PJ-D4) : rapatriées
  // du serveur, versées au brouillon, trois états par puce ; la réponse
  // n'affiche rien — l'usage du courrier ne transmet pas les pièces
  // d'origine en réponse, la puce du prototype mentait.
  //
  // Envoi par la boîte d'envoi (règles d'or : journalisé AVANT toute
  // tentative réseau, puis vidange) — le toast du prototype dit « Message
  // envoyé. » dès la remise ; l'incident d'envoi visible est la fente
  // d'avis, dû de bascule P5.
  //
  // L'autosave v1 est conservé sous le bouton : brouillon sauvé 2 s après
  // la frappe, conflit d'édition (`forked`) JAMAIS tu, fermer = conserver
  // (un contenu vidé par l'utilisateur est le seul cas où fermer jette).
  //
  // Inertes comme au prototype : la barre de format G/I/S/Liste/Lien/
  // Citation. (« Rendre indépendante » RETIRÉE — A53, D2 : le multi-
  // fenêtre est reporté en chantier dédié. Cc/Cci sont désormais CÂBLÉS
  // — A54.) Écart dit : la ligne « De » montre l'adresse seule — le cœur
  // ne stocke ni nom d'affichage ni étiquette de compte.
  //
  // « Joindre » est RÉEL (PLAN-PIECES-JOINTES E2) : sélecteur natif,
  // octets copiés au brouillon dès le geste (PJ-D1 — le brouillon-ancre
  // naît au premier fichier), refus au plafond dit sous la rangée
  // (PJ-D3), retrait par puce, poids total. Chaque geste rend l'epoch
  // du brouillon et on L'ADOPTE : sans cela, l'autosave suivant verrait
  // un conflit fantôme et bifurquerait le brouillon.
  import { appel, choisirFichiers } from './lib/transport.js';
  import { t } from './lib/texte.svelte.js';

  let {
    comptes = [],
    compte = null,
    onflash = () => {},
    onenvoye = () => {},
    // Chaque geste qui change les brouillons le rapporte : la liste
    // (dossier, mention sur le fil) se ressonde sans attendre les 10 s.
    onbrouillon = () => {},
    // E2 (PLAN-REACTIVITE) : la copie Envoyés vient d'ENTRER en base
    // (bilan de la relève ciblée) — l'App resert liste et nav tout de
    // suite, sans attendre la sonde de génération.
    oncourrier = () => {},
  } = $props();

  let visible = $state(false);
  let mode = $state('new');
  let expediteur = $state(null); // { account_id, email }
  let a = $state('');
  // Cc et Cci (A54) : leurs rangs ne s'affichent qu'à la demande
  // (`montrerCc`/`montrerCci`) — ou d'office si un contenu arrive (reprise
  // de brouillon, « Répondre à tous » qui remet les Cc d'origine, D3).
  let cc = $state('');
  let cci = $state('');
  let montrerCc = $state(false);
  let montrerCci = $state(false);
  let champCc = $state(null);
  let champCci = $state(null);
  let objet = $state('');
  let corps = $state('');
  // Les pièces RÉELLES du brouillon (métadonnées) — ce que le composeur
  // montre est ce que le message emporte, sans exception (PJ-D4).
  let pieces = $state([]);
  // Le rapatriement des pièces d'origine d'un transfert : une entrée
  // par pièce pas encore acquise — { index, name, statut } avec statut
  // 'encours' | 'echec'. Une entrée qui aboutit devient une pièce.
  let rapatriements = $state([]);
  // Le refus au plafond, affiché sous la rangée ; effacé au geste
  // suivant qui aboutit (ajout accepté ou retrait).
  let refus = $state(null);
  let envoiEnCours = $state(false);
  // La source d'un transfert (account_id, mailbox, uid) — « Réessayer »
  // doit savoir d'où rapatrier.
  let sourceTransfert = null;
  let replyToMailbox = null;
  let replyToUid = null;
  let brouillonId = null;
  let brouillonEpoch = null;
  let minuterie;
  // La sauvegarde EN VOL : sa promesse, tant qu'elle court. Les
  // sauvegardes sont SÉRIALISÉES derrière elle, et les gestes qui
  // décident du sort du brouillon (fermer, envoyer) l'attendent —
  // sans quoi une sauvegarde partie AVANT un « vider puis fermer »
  // ressuscitait le brouillon que le geste venait de supprimer
  // (fantôme au dossier, constaté deux fois par la suite e2e sous
  // charge, toujours au même geste).
  let volSauvegarde = null;
  let jeton = 0;

  let champA = $state(null);
  let champCorps = $state(null);

  const KICKERS = {
    new: 'compo.nouveau',
    reply: 'action.repondre',
    reply_all: 'action.repondreTous',
    forward: 'action.transferer',
  };
  const COMMANDES = {
    reply: 'reply_context',
    reply_all: 'reply_all_context',
    forward: 'forward_context',
  };

  // Formes du prototype, à la lettre — le cœur produit « Re: » / « Fwd: »,
  // la surface parle la langue de l'interface (« Re : » / « Tr : » en
  // français, "Re:" / "Fwd:" en anglais — A15, décision L-4).
  const sujetRe = (s) => (/^re\s*:/i.test(s ?? '') ? s : t('compo.re', { sujet: s ?? '' }));
  const sujetTr = (s) => (/^(tr|fwd|fw)\s*:/i.test(s ?? '') ? s : t('compo.tr', { sujet: s ?? '' }));

  function compteDe(accountId) {
    const connu = comptes.find((c) => c.account_id === accountId);
    return connu ? { account_id: connu.account_id, email: connu.email } : null;
  }

  export async function ouvrir(nouveauMode, source = null) {
    const mien = ++jeton;
    mode = nouveauMode;
    a = '';
    cc = '';
    cci = '';
    montrerCc = false;
    montrerCci = false;
    objet = '';
    corps = '';
    pieces = [];
    rapatriements = [];
    refus = null;
    sourceTransfert = null;
    replyToMailbox = null;
    replyToUid = null;
    brouillonId = null;
    brouillonEpoch = null;
    expediteur = source
      ? compteDe(source.account_id)
      : compteDe(compte) ?? (comptes.length > 0 ? compteDe(comptes[0].account_id) : null);
    visible = true;

    if (nouveauMode !== 'new' && source) {
      const enReponse = nouveauMode === 'reply' || nouveauMode === 'reply_all';
      try {
        const contexte = await appel(COMMANDES[nouveauMode], {
          accountId: source.account_id,
          mailbox: source.mailbox,
          uid: source.uid,
        });
        if (mien !== jeton) return;
        objet = enReponse ? sujetRe(source.subject) : sujetTr(source.subject);
        if (enReponse) {
          a = contexte.to;
          // D3 : « Répondre à tous » remet les Cc d'origine EN Cc — le
          // rang s'ouvre de lui-même s'il y en a.
          cc = contexte.cc ?? '';
          if (cc) montrerCc = true;
          const prenom = (source.sender ?? '').split(' ')[0];
          // La citation du cœur mène par deux sauts (la place du curseur en
          // v1) ; l'amorce du prototype les apporte déjà — sans cette taille,
          // quatre lignes vides sépareraient l'amorce de la citation.
          const citation = contexte.body.replace(/^\n+/, '');
          corps = prenom ? `${t('compo.bonjour', { prenom })}\n\n${citation}` : contexte.body;
          replyToMailbox = source.mailbox;
          replyToUid = source.uid;
        } else {
          corps = contexte.body;
        }
      } catch (err) {
        if (mien !== jeton) return;
        if (nouveauMode !== 'reply') {
          // Sans corps, un transfert ne transmettrait rien ; sans la
          // liste complète, un « à tous » enverrait à moins de monde que
          // promis (le cœur la relit sur le serveur) : échec franc.
          visible = false;
          onflash(
            nouveauMode === 'forward'
              ? t('erreur.transfert', { err })
              : t('erreur.repondreTous', { err }),
          );
          return;
        }
        // Réponse sans citation : le cœur le permet, on écrit quand même.
        objet = sujetRe(source.subject);
        replyToMailbox = source.mailbox;
        replyToUid = source.uid;
      }
      // Le transfert transmet ses pièces POUR DE VRAI (PJ-D4) : chacune
      // est rapatriée du serveur et versée au brouillon — une puce par
      // état. La réponse, elle, n'affiche plus rien : l'usage du
      // courrier n'a jamais transmis les pièces d'origine en réponse,
      // et la puce du prototype promettait un envoi qui n'existait pas.
      //
      // SANS garde sur `source.attachment_count` : la ligne porte le
      // compte d'AVANT l'ouverture du message — sur un message reçu à
      // l'instant, s'y fier sautait le rapatriement en silence, la
      // faute exacte que PJ-D4 interdit (terrain CE, 2026-08-14). La
      // lecture des métadonnées est locale : zéro pièce = zéro coût.
      if (nouveauMode === 'forward') {
        try {
          const lues = await appel('message_attachments', {
            accountId: source.account_id,
            mailbox: source.mailbox,
            uid: source.uid,
          });
          if (mien !== jeton) return;
          sourceTransfert = {
            account_id: source.account_id,
            mailbox: source.mailbox,
            uid: source.uid,
          };
          rapatriements = lues.map((piece) => ({
            index: piece.index,
            name: piece.name,
            statut: 'encours',
          }));
          // Sans await : la frappe n'attend pas le réseau — les puces
          // changent d'état à l'arrivée de chaque pièce.
          rapatrierTout([...rapatriements], mien);
        } catch (err) {
          console.error('message_attachments :', err);
        }
      }
    }
    // Top-posting : le curseur se pose AU-DESSUS de la citation.
    setTimeout(() => {
      if (mien !== jeton || !visible) return;
      if (a && champCorps) {
        champCorps.focus();
        champCorps.setSelectionRange(0, 0);
        // Le focus a pu défiler vers le caret de fin avant la repose à 0 :
        // l'amorce doit être VISIBLE, pas seulement première.
        champCorps.scrollTop = 0;
      } else {
        champA?.focus();
      }
    }, 0);
  }

  // Reprendre un brouillon local (fente d'avis, dû §6) : le contenu
  // revient tel quel, l'autosave repart de SON epoch — le conflit
  // d'édition reste couvert.
  export function ouvrirBrouillon(brouillon) {
    jeton += 1;
    const mien = jeton;
    mode = 'new';
    expediteur = compteDe(brouillon.account_id);
    a = brouillon.to;
    // Cc/Cci reviennent avec le brouillon (A54) — leur rang s'ouvre s'il
    // y a du contenu à montrer.
    cc = brouillon.cc ?? '';
    cci = brouillon.bcc ?? '';
    montrerCc = cc.trim() !== '';
    montrerCci = cci.trim() !== '';
    objet = brouillon.subject;
    corps = brouillon.body;
    pieces = [];
    rapatriements = [];
    refus = null;
    sourceTransfert = null;
    // Les puces reviennent avec le texte (PJ-D1) : les octets vivaient
    // au brouillon, pas dans la session du composeur.
    appel('draft_attachments', { draftId: brouillon.id })
      .then((lues) => {
        if (mien === jeton) pieces = lues;
      })
      .catch((err) => console.error('draft_attachments :', err));
    // La boîte revient AVEC l'UID : la chaîne réponse → brouillon →
    // reprise → sauvegarde ne doit pas perdre le lien au fil (B-D2).
    replyToMailbox = brouillon.reply_to_mailbox ?? null;
    replyToUid = brouillon.reply_to_uid ?? null;
    brouillonId = brouillon.id;
    brouillonEpoch = brouillon.updated_epoch;
    visible = true;
    setTimeout(() => champCorps?.focus(), 0);
  }

  // Un brouillon sans texte mais avec pièce n'est PAS vide : fermer le
  // conserve, le contrat des brouillons couvre les octets.
  const vide = () =>
    !a.trim() && !cc.trim() && !cci.trim() && !objet.trim() && !corps.trim() && pieces.length === 0;

  function programmerSauvegarde() {
    clearTimeout(minuterie);
    minuterie = setTimeout(sauverMaintenant, 2000);
  }

  // Le filet : un crash ne coûte que les deux dernières secondes de
  // frappe. Rend le bilan, ou null s'il n'y avait rien à faire.
  // Une seule sauvegarde à la fois : chaque tour part derrière le vol
  // précédent, et `volSauvegarde` porte toujours le dernier tour.
  function sauverMaintenant() {
    clearTimeout(minuterie);
    const tour = (volSauvegarde ?? Promise.resolve()).then(sauverSeul);
    volSauvegarde = tour;
    tour.finally(() => {
      if (volSauvegarde === tour) volSauvegarde = null;
    });
    return tour;
  }

  async function sauverSeul() {
    if (!visible || vide() || !expediteur) return null;
    try {
      const bilan = await appel('save_draft', {
        accountId: expediteur.account_id,
        id: brouillonId,
        baseEpoch: brouillonEpoch,
        content: { to: a, cc, bcc: cci, subject: objet, body: corps, replyToUid, replyToMailbox },
      });
      if (!visible) {
        // Le panneau s'est fermé pendant la sauvegarde (envoi parti) :
        // ne pas ressusciter un brouillon déjà réglé.
        await appel('delete_draft', { id: bilan.id })
          .catch((err) => console.error('delete_draft (panneau fermé pendant la sauvegarde) :', err));
        onbrouillon();
        return null;
      }
      brouillonId = bilan.id;
      brouillonEpoch = bilan.updated_epoch;
      if (bilan.forked) {
        // Ne JAMAIS taire ce cas : deux textes existent désormais, seul
        // l'utilisateur peut trancher.
        onflash(t('toast.brouillonFork'));
      }
      onbrouillon();
      return bilan;
    } catch {
      // La prochaine frappe retentera — le filet n'alarme pas pour rien.
    }
    return null;
  }

  export function estOuverte() {
    return visible;
  }

  // Fermer = conserver : un contenu non vide devient (ou reste) un
  // brouillon ; un brouillon vidé de son texte est jeté — c'est le seul
  // cas où fermer supprime, et c'est l'utilisateur qui a effacé.
  export async function fermer() {
    if (!visible) return;
    clearTimeout(minuterie);
    // La sauvegarde en vol d'abord : elle peut porter un contenu
    // d'AVANT le vidage et ressusciter ce que le geste supprime — on
    // décide du sort du brouillon sur un sol immobile, jamais pendant
    // qu'une écriture court.
    if (volSauvegarde) await volSauvegarde;
    if (vide()) {
      if (brouillonId !== null) {
        await appel('delete_draft', { id: brouillonId })
          .catch((err) => console.error('delete_draft (brouillon vidé) :', err));
        onbrouillon();
      }
      visible = false;
      return;
    }
    const bilan = await sauverMaintenant();
    visible = false;
    if (!(bilan && bilan.forked)) onflash(t('toast.brouillonEnregistre'));
    // Le reflet part TOUT DE SUITE, en silence (R1, séquence v1) : hors
    // ligne, le cycle suivant retentera — rien à dire.
    appel('sync_drafts').catch(() => {});
  }

  async function enregistrerBrouillon() {
    if (vide()) return;
    await fermer();
  }

  async function envoyer() {
    if (envoiEnCours) return; // double-clic = un seul envoi
    if (!expediteur) {
      onflash(t('erreur.aucunCompte'));
      return;
    }
    // Des pièces du transfert manquent (en cours ou en échec) : partir
    // sans elles serait une absence silencieuse (PJ-D4). Attendre,
    // réessayer — ou y renoncer d'un geste explicite (la croix).
    if (rapatriements.length > 0) {
      onflash(t('erreur.piecesManquantes'));
      return;
    }
    envoiEnCours = true;
    // Même règle que fermer : la sauvegarde en vol se pose avant le
    // départ — le brouillon-ancre (`draftId`) doit être son id FINAL,
    // pas celui d'avant une écriture encore en route.
    clearTimeout(minuterie);
    if (volSauvegarde) await volSauvegarde;
    try {
      await appel('queue_send', {
        accountId: expediteur.account_id,
        to: a,
        cc,
        bcc: cci,
        subject: objet.trim(),
        body: corps,
        replyToMailbox,
        replyToUid,
        // Le brouillon-ancre : ses pièces rejoignent le journal dans la
        // même transaction (PJ-D2).
        draftId: brouillonId,
      });
    } catch (err) {
      onflash(t('erreur.envoi', { err }));
      return;
    } finally {
      envoiEnCours = false;
    }
    // L'envoi est journalisé : le brouillon a rempli son office.
    const regle = brouillonId;
    clearTimeout(minuterie);
    visible = false;
    onflash(t('toast.envoye'));
    if (regle !== null) {
      await appel('delete_draft', { id: regle })
        .catch((err) => console.error('delete_draft (après envoi) :', err));
      onbrouillon();
    }
    // Vidange en arrière-plan ; hors ligne, la file attend — l'incident
    // visible est la fente d'avis (P5). Vidange faite ET fructueuse :
    // relève ciblée du dossier Envoyés (la copie qu'ajoute le serveur
    // doit se voir sans attendre le cycle complet — terrain 0.1.4),
    // lancée en PARALLÈLE de la suite : elle retente en interne (+5 s,
    // +15 s) si la copie asynchrone n'est pas encore là (E2), et ni le
    // reflet des brouillons ni le bilan d'envoi n'ont à l'attendre.
    // Son bilan dit tout — les incidents en console (le `.catch` muet
    // du terrain 0.1.5 a rendu l'instruction aveugle : plus jamais),
    // le courrier rapporté resert la liste par `oncourrier`.
    const compteEnvoi = expediteur.account_id;
    appel('flush_outbox')
      .then((bilan) => {
        // Un envoi DIFFÉRÉ (hors ligne) n'a rien déposé chez le
        // serveur : rien à réconcilier, le retour en ligne s'en charge
        // (R-D3) — et l'envoi n'ayant pas eu lieu, il n'a pas d'écho.
        if (bilan.sent > 0) {
          // E3 : l'écho Envoyés est NÉ à la vidange (transaction du
          // passage à `sent`) — la copie se montre < 1 s, sans le
          // serveur. La passe d'après-geste réconcilie derrière.
          oncourrier();
          appel('sync_apres_geste', { accountId: compteEnvoi })
            .then((releve) => {
              for (const incident of releve.errors) {
                console.error('sync_apres_geste :', incident);
              }
              if (releve.fetched > 0 || releve.deleted > 0 || releve.reconcilies > 0
                  || releve.balayes > 0) {
                oncourrier();
              }
            })
            .catch((err) => console.error('sync_apres_geste :', err));
        }
        return appel('sync_drafts').catch(() => {});
      })
      .catch((err) => console.error('flush_outbox :', err))
      .finally(() => onenvoye());
  }

  // Même forme que `human_size` du cœur (point décimal compris) : le
  // poids total doit parler comme les puces qu'il somme.
  const KO = 1024;
  const MO = KO * 1024;
  function poidsHumain(octets) {
    if (octets < KO) return `${octets} o`;
    if (octets < MO) return `${Math.round(octets / KO)} Ko`;
    return `${(octets / MO).toFixed(1)} Mo`;
  }
  const poidsTotal = $derived(pieces.reduce((somme, piece) => somme + piece.size, 0));

  async function joindre() {
    if (!expediteur) {
      onflash(t('erreur.aucunCompte'));
      return;
    }
    const chemins = await choisirFichiers().catch((err) => {
      onflash(t('erreur.piece', { err }));
      return [];
    });
    if (chemins.length === 0) return;
    try {
      const bilan = await appel('attach_files', {
        accountId: expediteur.account_id,
        draftId: brouillonId,
        paths: chemins,
      });
      // `null` : tout refusé sans brouillon préexistant — rien à adopter.
      brouillonId = bilan.draft_id ?? brouillonId;
      // L'epoch du geste, sinon l'autosave verrait un conflit fantôme.
      if (bilan.updated_epoch != null) brouillonEpoch = bilan.updated_epoch;
      pieces = bilan.pieces;
      refus =
        bilan.refused.length > 0
          ? t('compo.pieceRefusee', {
              nom: bilan.refused[0].name,
              reste: bilan.refused[0].remaining,
            })
          : null;
      onbrouillon();
    } catch (err) {
      onflash(t('erreur.piece', { err }));
      // Un échec en cours de route a pu laisser des pièces entrées :
      // relire plutôt que deviner.
      if (brouillonId !== null) {
        appel('draft_attachments', { draftId: brouillonId })
          .then((lues) => {
            pieces = lues;
          })
          .catch(() => {});
      }
    }
  }

  async function retirer(piece) {
    try {
      const epoch = await appel('detach_file', { attachmentId: piece.id });
      pieces = pieces.filter((p) => p.id !== piece.id);
      if (epoch != null) brouillonEpoch = epoch;
      refus = null;
      onbrouillon();
    } catch (err) {
      onflash(t('erreur.piece', { err }));
    }
  }

  // Rapatrie UNE pièce du message d'origine (PJ-D4). Trois issues :
  // versée (elle devient une puce pleine), refusée au plafond (elle
  // disparaît, le refus est dit — définitif), échec réseau (la puce
  // passe en échec, « Réessayer » reste).
  async function rapatrierUne(entree, mien) {
    try {
      const bilan = await appel('fetch_source_attachment', {
        accountId: sourceTransfert.account_id,
        mailbox: sourceTransfert.mailbox,
        uid: sourceTransfert.uid,
        index: entree.index,
        draftId: brouillonId,
      });
      if (mien !== jeton) return;
      brouillonId = bilan.draft_id ?? brouillonId;
      if (bilan.updated_epoch != null) brouillonEpoch = bilan.updated_epoch;
      if (bilan.piece) {
        pieces = [...pieces, bilan.piece];
        rapatriements = rapatriements.filter((r) => r.index !== entree.index);
        onbrouillon();
      } else if (bilan.refused) {
        rapatriements = rapatriements.filter((r) => r.index !== entree.index);
        refus = t('compo.pieceRefusee', {
          nom: bilan.refused.name,
          reste: bilan.refused.remaining,
        });
      }
    } catch (err) {
      if (mien !== jeton) return;
      console.error('fetch_source_attachment :', err);
      rapatriements = rapatriements.map((r) =>
        r.index === entree.index ? { ...r, statut: 'echec' } : r,
      );
    }
  }

  // L'enchaînement est SÉQUENTIEL : la première pièce crée le
  // brouillon-ancre, les suivantes doivent le connaître.
  async function rapatrierTout(entrees, mien) {
    for (const entree of entrees) {
      if (mien !== jeton) return;
      await rapatrierUne(entree, mien);
    }
  }

  function reessayer(entree) {
    rapatriements = rapatriements.map((r) =>
      r.index === entree.index ? { ...r, statut: 'encours' } : r,
    );
    rapatrierUne({ ...entree, statut: 'encours' }, jeton);
  }

  // Renoncer à une pièce en échec — le geste EXPLICITE qui autorise un
  // envoi sans elle : jamais d'absence silencieuse (PJ-D4).
  function renoncer(entree) {
    rapatriements = rapatriements.filter((r) => r.index !== entree.index);
  }
</script>

{#if visible}
  <div class="scrim" data-testid="composition">
    <div class="carte" role="dialog" aria-modal="true" aria-label={t(KICKERS[mode])}>
      <!-- Terrain A46 : l'entête ne répète plus l'objet — le champ
           Objet le dit, juste dessous. -->
      <div class="tete">
        <span class="kicker" data-testid="composition-kicker">{t(KICKERS[mode])}</span>
        <span class="essor"></span>
        <button type="button" class="fermer" aria-label={t('action.fermer')} onclick={fermer}>
          <span class="ms" aria-hidden="true">close</span></button>
      </div>
      <div class="champs">
        <div class="rang">
          <span class="etiquette">{t('conv.de')}</span>
          {#if comptes.length > 1}
            <!-- A10 : le compte émetteur SE CHOISIT (verdict terrain) —
                 le prototype figeait la ligne, v1 avait le sélecteur. -->
            <select class="valeur" data-testid="composition-de" aria-label={t('compo.compteEmetteur')}
                    value={expediteur?.email ?? ''}
                    onchange={(e) => {
                      const choisi = comptes.find((c) => c.email === e.target.value);
                      if (choisi) expediteur = { account_id: choisi.account_id, email: choisi.email };
                      programmerSauvegarde();
                    }}>
              {#each comptes as c (c.account_id)}
                <option value={c.email}>{c.email}</option>
              {/each}
            </select>
          {:else}
            <span class="valeur" data-testid="composition-de">{expediteur?.email ?? ''}</span>
          {/if}
        </div>
        <div class="rang">
          <span class="etiquette">{t('conv.a')}</span>
          <input type="text" bind:this={champA} bind:value={a} oninput={programmerSauvegarde}
                 placeholder={t('compo.destinataire')} data-testid="composition-a">
          <!-- A54 : Cc/Cci ouvrent leur rang à la demande (ou d'office si
               un contenu est déjà là — reprise, « Répondre à tous »). -->
          {#if !montrerCc}
            <button type="button" class="puce" data-testid="composition-bouton-cc"
                    onclick={() => { montrerCc = true; setTimeout(() => champCc?.focus(), 0); }}>
              <span class="ms" aria-hidden="true">group_add</span>{t('compo.cc')}</button>
          {/if}
          {#if !montrerCci}
            <button type="button" class="puce" data-testid="composition-bouton-cci"
                    onclick={() => { montrerCci = true; setTimeout(() => champCci?.focus(), 0); }}>
              <span class="ms" aria-hidden="true">visibility_off</span>{t('compo.cci')}</button>
          {/if}
        </div>
        {#if montrerCc}
          <div class="rang">
            <span class="etiquette">{t('compo.cc')}</span>
            <input type="text" bind:this={champCc} bind:value={cc} oninput={programmerSauvegarde}
                   placeholder={t('compo.destinataire')} data-testid="composition-cc">
          </div>
        {/if}
        {#if montrerCci}
          <div class="rang">
            <span class="etiquette">{t('compo.cci')}</span>
            <input type="text" bind:this={champCci} bind:value={cci} oninput={programmerSauvegarde}
                   placeholder={t('compo.destinataire')} data-testid="composition-cci">
          </div>
        {/if}
        <div class="rang">
          <span class="etiquette">{t('conv.objet')}</span>
          <input type="text" bind:value={objet} oninput={programmerSauvegarde}
                 placeholder={t('compo.objetPlaceholder')} data-testid="composition-objet">
        </div>
      </div>
      <div class="zone-corps">
        <textarea bind:this={champCorps} bind:value={corps} oninput={programmerSauvegarde}
                  placeholder={t('compo.corpsPlaceholder')} data-testid="composition-corps"></textarea>
      </div>
      {#if pieces.length > 0 || rapatriements.length > 0}
        <div class="fichiers" data-testid="composition-pieces">
          {#each pieces as piece (piece.id)}
            <span class="piece" data-testid="piece-compo">
              <span class="ms" aria-hidden="true">description</span>
              <span class="nom">{piece.name}</span><span class="taille">{piece.human}</span>
              <button type="button" class="retrait" data-testid="piece-retrait"
                      aria-label={t('compo.retirerPiece', { nom: piece.name })}
                      onclick={() => retirer(piece)}>
                <span class="ms" aria-hidden="true">close</span></button>
            </span>
          {/each}
          {#each rapatriements as entree (entree.index)}
            {#if entree.statut === 'encours'}
              <span class="piece attente" data-testid="piece-rapatriement">
                <span class="ms" aria-hidden="true">hourglass_empty</span>
                {t('compo.rapatriement', { nom: entree.name })}</span>
            {:else}
              <span class="piece echec" data-testid="piece-echec">
                <span class="ms" aria-hidden="true">description</span>
                <span class="nom">{entree.name}</span>
                <button type="button" class="reessayer" data-testid="piece-reessayer"
                        onclick={() => reessayer(entree)}>{t('action.reessayer')}</button>
                <button type="button" class="retrait" data-testid="piece-renoncer"
                        aria-label={t('compo.retirerPiece', { nom: entree.name })}
                        onclick={() => renoncer(entree)}>
                  <span class="ms" aria-hidden="true">close</span></button>
              </span>
            {/if}
          {/each}
          {#if pieces.length > 0}
            <span class="poids" data-testid="composition-poids">
              {t('compo.poidsTotal', { poids: poidsHumain(poidsTotal) })}</span>
          {/if}
        </div>
      {/if}
      {#if refus}
        <div class="refus" data-testid="composition-refus">
          <span class="ms" aria-hidden="true">warning</span>{refus}
        </div>
      {/if}
      <div class="format">
        <span class="bouton-format gras">{t('compo.gras')}</span>
        <span class="bouton-format italique">{t('compo.italique')}</span>
        <span class="bouton-format souligne">{t('compo.souligne')}</span>
        <span class="puce"><span class="ms" aria-hidden="true">format_list_bulleted</span>{t('compo.liste')}</span>
        <span class="puce"><span class="ms" aria-hidden="true">link</span>{t('compo.lien')}</span>
        <span class="puce"><span class="ms" aria-hidden="true">format_quote</span>{t('compo.citation')}</span>
      </div>
      <div class="pied">
        <button type="button" class="principal" data-testid="composition-envoyer"
                disabled={envoiEnCours} onclick={envoyer}>
          <span class="ms" aria-hidden="true">send</span>{t('action.envoyer')}</button>
        <button type="button" onclick={joindre} data-testid="composition-joindre">
          <span class="ms" aria-hidden="true">attach_file</span>{t('compo.joindre')}</button>
        <button type="button" onclick={enregistrerBrouillon} data-testid="composition-brouillon">
          <span class="ms" aria-hidden="true">drafts</span>{t('compo.enregistrerBrouillon')}</button>
        <button type="button" class="annuler" data-testid="composition-annuler"
                onclick={fermer}>{t('action.annuler')}</button>
      </div>
    </div>
  </div>
{/if}

<style>
  /* Géométrie VERBATIM de la surimpression de composition du prototype. */
  .scrim {
    position:absolute; inset:0; background:var(--scrim); z-index:2;
    display:flex; align-items:center; justify-content:center; padding:36px;
  }
  .carte {
    width:860px; max-height:100%; background:var(--surface);
    border:1px solid var(--border);
    border-radius:10px; box-shadow:var(--shadow);
    display:flex; flex-direction:column; overflow:hidden;
  }
  .tete {
    height:48px; flex:none; padding:0 16px 0 22px; display:flex;
    align-items:center; gap:14px; border-bottom:1px solid var(--border);
  }
  .kicker {
    font-size:12px; letter-spacing:.1em; text-transform:uppercase;
    color:var(--muted); font-weight:600; white-space:nowrap;
  }
  .essor { flex:1; }
  .puce {
    height:32px; padding:0 12px; display:inline-flex; align-items:center;
    gap:8px; font-size:13px; color:var(--ink2); background:var(--surface);
    border:1px solid var(--border); border-radius:6px; white-space:nowrap;
    flex:none;
  }
  .fermer {
    height:32px; width:32px; padding:0; display:inline-flex; flex:none;
    align-items:center; justify-content:center; color:var(--ink2);
    background:var(--surface); border:1px solid var(--border);
    border-radius:6px; cursor:pointer;
  }
  .fermer:hover { background:var(--sel); }

  /* A46 : l'écart entête → « De » au dessin du composeur du prototype
     (.ccorps : 6 px), plus les 18 px d'antan. */
  .champs { padding:6px 22px 0; display:flex; flex-direction:column; }
  .rang {
    height:44px; display:flex; align-items:center; gap:14px;
    border-bottom:1px solid var(--border);
  }
  .etiquette { width:52px; font-size:13px; color:var(--muted); flex:none; }
  .valeur { flex:1; font-size:13px; color:var(--ink); }
  select.valeur {
    border:none; background:transparent; cursor:pointer; padding:0;
    font:inherit; font-size:13px; color:var(--ink); min-width:0;
  }
  select.valeur option { background:var(--surface); color:var(--ink); }
  .rang input {
    flex:1; font-size:13px; color:var(--ink); border:none; outline:none;
    background:transparent; min-width:0;
  }

  .zone-corps {
    padding:20px 22px; display:flex; flex-direction:column;
    min-height:220px; flex:1;
  }
  textarea {
    flex:1; width:100%; min-height:180px; font-size:15px; line-height:1.65;
    color:var(--ink); border:none; outline:none; resize:none;
    background:transparent; font-family:inherit;
  }

  .fichiers { padding:0 22px 14px; display:flex; gap:10px; flex-wrap:wrap; align-items:center; }

  /* La puce d'une pièce à joindre (maquette §1) : nom + taille + retrait
     dans la MÊME puce — un objet manipulable, pas deux lectures.
     Marges symétriques (A33) : 12 px des deux côtés — la croix de
     retrait ne réduit pas la marge de son côté. */
  .piece {
    height:32px; padding:0 12px; display:inline-flex; align-items:center;
    gap:8px; font-size:13px; color:var(--ink2); background:var(--surface);
    border:1px solid var(--border); border-radius:6px; white-space:nowrap;
  }
  .piece .nom { color:var(--ink); }
  .piece .taille { font-size:12px; color:var(--muted); }
  .retrait {
    height:22px; width:22px; padding:0; display:inline-flex; align-items:center;
    justify-content:center; color:var(--muted); background:transparent;
    border:none; border-radius:4px; cursor:pointer;
  }
  .retrait:hover { background:var(--sel); color:var(--ink); }
  .retrait .ms { font-size:13px; }
  /* Les états du rapatriement (maquette §3) : attente atténuée italique,
     échec au bord --alert avec « Réessayer ». */
  .piece.attente { color:var(--muted); font-style:italic; }
  .piece.echec { border-color:var(--alert); }
  .piece.echec .nom { color:var(--alert); font-weight:600; }
  .reessayer {
    height:22px; padding:0 8px; display:inline-flex; align-items:center;
    font-size:12px; font-family:inherit; font-weight:600; color:var(--ink2);
    background:var(--surface); border:1px solid var(--border); border-radius:4px;
    cursor:pointer;
  }
  .reessayer:hover { background:var(--sel); color:var(--ink); }
  .poids { margin-left:auto; font-size:12.5px; color:var(--muted); white-space:nowrap; }
  .refus {
    padding:0 22px 14px; font-size:13px; color:var(--alert);
    display:flex; align-items:center; gap:8px;
  }
  .refus .ms { font-size:14px; }

  .format {
    flex:none; padding:8px 18px; border-top:1px solid var(--border);
    background:var(--panel); display:flex; align-items:center; gap:8px;
  }
  .bouton-format {
    height:32px; min-width:32px; padding:0 10px; display:inline-flex;
    align-items:center; justify-content:center; font-size:13px;
    color:var(--ink); background:var(--surface);
    border:1px solid var(--border); border-radius:6px;
  }
  .gras { font-weight:600; }
  .italique { font-style:italic; }
  .souligne { text-decoration:underline; }

  .pied {
    flex:none; padding:14px 22px 18px; border-top:1px solid var(--border);
    display:flex; align-items:center; gap:12px;
  }
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
  .principal:disabled { opacity:.6; cursor:default; }
  .annuler {
    margin-left:auto; height:auto; padding:0; border:none;
    background:transparent; font-size:13px; color:var(--muted);
    text-decoration:underline; cursor:pointer;
  }
  .annuler:hover { background:transparent; color:var(--ink2); }
</style>
