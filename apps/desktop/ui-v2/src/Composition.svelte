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
  // La barre de mise en forme est RÉELLE (PLAN-COMPOSITION-HTML, R4) :
  // le corps est un `contenteditable` piloté par `execCommand` en mode
  // legacy (`styleWithCSS` éteint) — sa sortie (b/i/u/strike, font
  // color/face/size, align, listes, blockquote) est mot pour mot le
  // vocabulaire que l'allowlist ammonia conserve. Le HTML est ASSAINI
  // côté Rust à chaque écriture (save_draft, queue_send) ; le texte du
  // repli en est DÉRIVÉ là-bas aussi — une seule autorité. Lien et
  // Citation sont RETIRÉS de la barre (décision CE D1, périmètre R4
  // strict). (« Rendre indépendante » RETIRÉE — A53, D2. Cc/Cci CÂBLÉS
  // — A54.) Écart dit : la ligne « De » montre l'adresse seule — le cœur
  // ne stocke ni nom d'affichage ni étiquette de compte.
  //
  // Un brouillon réouvert puis fermé SANS FRAPPE repart À L'OCTET PRÈS
  // (les valeurs stockées sont ré-émises telles quelles, jamais relues
  // du DOM) : le navigateur re-sérialise `innerHTML` normalisé (styles,
  // entités) — relire l'éditeur marquerait le brouillon modifié à chaque
  // ouverture et re-pousserait une copie vers Gmail, le churn exact que
  // la détection « contenu identique » du cœur est venue tuer.
  //
  // « Joindre » est RÉEL (PLAN-PIECES-JOINTES E2) : sélecteur natif,
  // octets copiés au brouillon dès le geste (PJ-D1 — le brouillon-ancre
  // naît au premier fichier), refus au plafond dit sous la rangée
  // (PJ-D3), retrait par puce, poids total. Chaque geste rend l'epoch
  // du brouillon et on L'ADOPTE : sans cela, l'autosave suivant verrait
  // un conflit fantôme et bifurquerait le brouillon.
  import { tick } from 'svelte';
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
  // Le corps vit dans le DOM du `contenteditable` (`champCorps`), pas en
  // état Svelte. Tant que `corpsModifie` est faux, la sauvegarde ré-émet
  // les valeurs INITIALES (posées par `poserCorps`) — l'anti-churn ; dès
  // la première frappe, `innerHTML` devient la vérité. `corpsVersion`
  // est le pouls réactif du corps : les dérivés Svelte ne voient pas le
  // DOM, ils voient ce compteur.
  let corpsModifie = false;
  let corpsTexteInitial = '';
  let corpsHtmlInitial = null;
  let corpsVersion = $state(0);
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
  // $state : la visibilité du geste « Supprimer le brouillon » se dérive
  // de l'existence d'un brouillon persisté (R3) — un `let` nu ne
  // rafraîchirait pas le pied.
  let brouillonId = $state(null);
  let brouillonEpoch = null;
  // R3 (PLAN-RETOURS-3, D3) : la suppression VOLONTAIRE d'un brouillon
  // depuis la composition passe une confirmation — un irréversible ne
  // part jamais du premier clic (même règle que le retrait de compte).
  // `Annuler`, lui, CONSERVE : les deux gestes ne se confondent pas.
  let demandeSuppr = $state(false);
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
  let zoneCorps = $state(null);
  let carte = $state(null);

  // L'autocomplétion des adresses (PLAN-RETOURS-5, D3-D4) : le menu
  // suit le champ actif (À, Cc, Cci), suggère l'annuaire des
  // correspondants sur le SEGMENT en cours (après la dernière virgule)
  // et insère l'adresse NUE (D3 — le nom se montre, ne s'insère pas).
  // Débobinage 150 ms + dernier-préfixe-gagne (jeton) : une frappe
  // rapide ne part jamais en rafale dans la file sérialisée (leçon
  // PLAN-DEFILEMENT-PROFOND).
  let suggestions = $state([]);
  let champSuggere = $state(null); // 'a' | 'cc' | 'cci' | null
  let choixSuggere = $state(0);
  let jetonSuggere = 0;
  let minuterieSuggere = null;

  const segmentCourant = (valeur) => valeur.split(',').pop().trim();

  function fermerSuggestions() {
    jetonSuggere += 1;
    clearTimeout(minuterieSuggere);
    suggestions = [];
    champSuggere = null;
    choixSuggere = 0;
  }

  function surFrappeAdresse(champ, valeur) {
    programmerSauvegarde();
    const prefixe = segmentCourant(valeur);
    clearTimeout(minuterieSuggere);
    if (prefixe.length < 2) {
      fermerSuggestions();
      return;
    }
    minuterieSuggere = setTimeout(async () => {
      const mien = ++jetonSuggere;
      try {
        const trouvees = await appel('completer_adresses', { prefixe, limite: 8 });
        if (mien !== jetonSuggere || !visible) return;
        suggestions = trouvees;
        champSuggere = trouvees.length > 0 ? champ : null;
        choixSuggere = 0;
      } catch (err) {
        console.error('completer_adresses :', err);
      }
    }, 150);
  }

  function insererSuggestion(choisie) {
    const champ = champSuggere;
    if (!champ || !choisie) return;
    const valeurs = { a, cc, cci };
    const morceaux = valeurs[champ].split(',');
    morceaux[morceaux.length - 1] = ` ${choisie.address}`;
    const neuve = morceaux.join(',').replace(/^ /, '');
    if (champ === 'a') a = neuve;
    else if (champ === 'cc') cc = neuve;
    else cci = neuve;
    fermerSuggestions();
    ({ a: champA, cc: champCc, cci: champCci })[champ]?.focus();
    programmerSauvegarde();
  }

  function clavierAdresse(ev) {
    if (!champSuggere || suggestions.length === 0) return;
    if (ev.key === 'ArrowDown') {
      ev.preventDefault();
      choixSuggere = (choixSuggere + 1) % suggestions.length;
    } else if (ev.key === 'ArrowUp') {
      ev.preventDefault();
      choixSuggere = (choixSuggere - 1 + suggestions.length) % suggestions.length;
    } else if (ev.key === 'Enter' || ev.key === 'Tab') {
      ev.preventDefault();
      insererSuggestion(suggestions[choixSuggere]);
    } else if (ev.key === 'Escape') {
      // Le menu se ferme, le focus RESTE au champ : on coupe la route
      // du Échap global (App) qui rendrait le focus à la liste.
      ev.stopPropagation();
      fermerSuggestions();
    }
  }

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

  // Miroir de `texte_en_html` du cœur : échappé, retours préservés — la
  // reprise d'un brouillon TEXTE (et lui seul) passe par là.
  function texteEnHtml(texte) {
    const echappe = (texte ?? '')
      .replaceAll('&', '&amp;')
      .replaceAll('<', '&lt;')
      .replaceAll('>', '&gt;');
    return `<div>${echappe.replaceAll('\n', '<br>')}</div>`;
  }

  const corpsHtml = () => champCorps?.innerHTML ?? '';

  // Ce que la sauvegarde et l'envoi remettent au Rust. Sans frappe, les
  // valeurs INITIALES repartent à l'octet près (anti-churn, tous
  // brouillons — texte ET riches : la re-sérialisation du navigateur
  // n'est jamais fidèle). Modifié : le HTML de l'éditeur seul — le
  // texte du repli est dérivé côté Rust (`frontiere_corps`), le `body`
  // transmis serait jeté, on ne le calcule pas.
  function chargeCorps() {
    if (!corpsModifie) {
      return { body: corpsTexteInitial, bodyHtml: corpsHtmlInitial };
    }
    return { body: '', bodyHtml: corpsHtml() };
  }

  // Pose le contenu de l'éditeur. `tick()` d'abord : le nœud n'existe
  // qu'une fois la surimpression rendue — poser avant serait perdu.
  // `htmlInitial: null` = brouillon TEXTE (la sauvegarde sans frappe ne
  // doit pas le convertir) ; par défaut, le HTML posé est l'initial.
  async function poserCorps(html, { texteInitial = '', htmlInitial = html } = {}) {
    corpsModifie = false;
    corpsTexteInitial = texteInitial;
    corpsHtmlInitial = htmlInitial;
    await tick();
    if (champCorps) champCorps.innerHTML = html;
    corpsVersion += 1;
  }

  function surFrappeCorps() {
    // Chromium laisse un <br> orphelin après « tout sélectionner puis
    // supprimer » : le corps est vide mais plus `:empty` — sans cette
    // renormalisation, le placeholder ne reviendrait jamais.
    if (champCorps && !champCorps.textContent && champCorps.innerHTML !== '') {
      champCorps.innerHTML = '';
    }
    corpsModifie = true;
    corpsVersion += 1;
    programmerSauvegarde();
  }

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
    pieces = [];
    rapatriements = [];
    refus = null;
    sourceTransfert = null;
    replyToMailbox = null;
    replyToUid = null;
    brouillonId = null;
    brouillonEpoch = null;
    demandeSuppr = false;
    // Le nuancier et la photo de sélection sont des états de MODULE :
    // ils survivraient à la fermeture de la carte — un Range du corps
    // précédent colorierait un fantôme.
    montrerCouleurs = false;
    selectionCorps = null;
    expediteur = source
      ? compteDe(source.account_id)
      : compteDe(compte) ?? (comptes.length > 0 ? compteDe(comptes[0].account_id) : null);
    fermerSuggestions();
    visible = true;
    await poserCorps('');

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
          // La citation riche du cœur mène par deux <br> (la place du
          // curseur) ; l'amorce les apporte déjà — sans cette taille,
          // quatre lignes vides sépareraient l'amorce de la citation.
          const citation = contexte.body_html ?? '';
          const contenu = prenom
            ? `${texteEnHtml(t('compo.bonjour', { prenom }))}<div><br></div>${citation.replace(/^(<br>)+/, '')}`
            : citation;
          // La frappe déjà posée PRIME : le contexte peut mettre des
          // secondes (corps à rapatrier) — écraser ce que l'utilisateur
          // a tapé entre-temps serait pire qu'une citation absente.
          if (!corpsModifie) await poserCorps(contenu);
          replyToMailbox = source.mailbox;
          replyToUid = source.uid;
        } else if (!corpsModifie) {
          await poserCorps(contexte.body_html ?? '');
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
      // Ne JAMAIS voler un focus déjà posé dans la carte : si
      // l'utilisateur a commencé à taper pendant l'ouverture, la
      // pré-mise au point n'a plus lieu d'être — sinon sa frappe
      // déménage de champ en plein mot (course vue à l'e2e : le corps
      // atterrissait dans le À). La carte est tenue par référence,
      // jamais par un sélecteur d'attribut de test.
      if (carte?.contains(document.activeElement)) return;
      if (a && champCorps) {
        champCorps.focus();
        // L'équivalent contenteditable du `setSelectionRange(0, 0)` du
        // textarea : un Range replié au tout début du corps.
        const selection = window.getSelection();
        const range = document.createRange();
        range.setStart(champCorps, 0);
        range.collapse(true);
        selection.removeAllRanges();
        selection.addRange(range);
        // Le focus a pu défiler vers le caret de fin avant la repose à
        // 0 — et le CONTENEUR de défilement est `.zone-corps`, pas
        // l'éditeur (le scrollTop de l'éditeur est toujours 0) :
        // l'amorce doit être VISIBLE, pas seulement première.
        if (zoneCorps) zoneCorps.scrollTop = 0;
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
    // Brouillon riche : son HTML tel quel. Brouillon texte : converti
    // pour l'éditeur (`htmlInitial: null` — sans frappe il ne devient
    // pas riche). Dans les deux cas l'anti-churn ré-émettra le stocké.
    poserCorps(brouillon.body_html ?? texteEnHtml(brouillon.body), {
      texteInitial: brouillon.body,
      htmlInitial: brouillon.body_html ?? null,
    });
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
    demandeSuppr = false;
    montrerCouleurs = false;
    selectionCorps = null;
    fermerSuggestions();
    visible = true;
    setTimeout(() => {
      // Même garde que `ouvrir()` : le focus déjà posé prime.
      if (carte?.contains(document.activeElement)) return;
      champCorps?.focus();
    }, 0);
  }

  // Un brouillon sans texte mais avec pièce n'est PAS vide : fermer le
  // conserve, le contrat des brouillons couvre les octets. Le corps se
  // juge sur son TEXTE (`textContent` — pas de reflow) ; la lecture de
  // `corpsVersion` rend la fonction RÉACTIVE à la frappe du corps, que
  // Svelte ne voit pas dans le DOM (sans elle, « Supprimer le
  // brouillon » n'apparaissait qu'à l'autosave).
  function vide() {
    void corpsVersion;
    return (
      !a.trim() &&
      !cc.trim() &&
      !cci.trim() &&
      !objet.trim() &&
      !(champCorps?.textContent ?? '').trim() &&
      pieces.length === 0
    );
  }

  // R3 : le geste « Supprimer le brouillon » n'a de sens que s'il y a une
  // matière à jeter — un brouillon déjà persisté, ou du contenu en cours.
  // Sur une composition vierge, « Annuler » suffit.
  const peutSupprimer = $derived(brouillonId !== null || !vide());

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
      const { body, bodyHtml } = chargeCorps();
      const bilan = await appel('save_draft', {
        accountId: expediteur.account_id,
        id: brouillonId,
        baseEpoch: brouillonEpoch,
        content: {
          to: a,
          cc,
          bcc: cci,
          subject: objet,
          body,
          bodyHtml,
          replyToUid,
          replyToMailbox,
        },
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
    fermerSuggestions();
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

  // R3 (PLAN-RETOURS-3, D3) : JETER le brouillon en cours, sur
  // confirmation. Contraire de `fermer()` — qui conserve : ici on
  // supprime la trace au dossier, quoi qu'elle contienne.
  async function supprimerBrouillon() {
    demandeSuppr = false;
    clearTimeout(minuterie);
    // Le sol immobile de `fermer()` : une sauvegarde en vol peut porter
    // un contenu d'AVANT le geste et ressusciter ce qu'on supprime —
    // on l'attend, puis on efface l'id FINAL.
    if (volSauvegarde) await volSauvegarde;
    // `brouillonId` peut avoir été posé PAR la sauvegarde qu'on vient
    // d'attendre — on le lit après, jamais avant.
    const avaitBrouillon = brouillonId !== null;
    if (avaitBrouillon) {
      await appel('delete_draft', { id: brouillonId })
        .catch((err) => console.error('delete_draft (suppression volontaire) :', err));
      onbrouillon();
    }
    // Plus aucun id ne subsiste : une réouverture repart vierge, jamais
    // sur un brouillon supprimé.
    brouillonId = null;
    brouillonEpoch = null;
    visible = false;
    // « Supprimé » ne se dit que si un brouillon existait VRAIMENT : sur
    // une composition jamais sauvée, il n'y avait rien à supprimer.
    if (avaitBrouillon) onflash(t('toast.brouillonSupprime'));
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
      const { body, bodyHtml } = chargeCorps();
      await appel('queue_send', {
        accountId: expediteur.account_id,
        to: a,
        cc,
        bcc: cci,
        subject: objet.trim(),
        body,
        bodyHtml,
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

  // --- La barre de mise en forme (R4, décisions CE D1-D3) --------------
  //
  // `execCommand` en mode legacy : `styleWithCSS` éteint à chaque geste,
  // pour que la sortie (<b>, <font>, align…) reste le vocabulaire exact
  // de l'allowlist ammonia — jamais de style CSS généré à traduire.
  //
  // La sélection SURVIT aux contrôles qui prennent le focus (les
  // <select> Police/Taille) : photographiée à chaque `selectionchange`
  // dans le corps, reposée avant chaque commande.
  let selectionCorps = null;
  let actifs = $state({});
  // D3 : nuancier fixe — douze teintes sûres sur la dalle claire du
  // corps (le courriel est composé pour un fond blanc, A61).
  const COULEURS = [
    '#000000',
    '#666666',
    '#cc0000',
    '#e69138',
    '#bf9000',
    '#38761d',
    '#45818e',
    '#3d85c6',
    '#1155cc',
    '#674ea7',
    '#a64d79',
    '#85200c',
  ];
  let montrerCouleurs = $state(false);

  function surSelection() {
    if (!visible || !champCorps) return;
    const selection = window.getSelection();
    if (selection.rangeCount > 0 && champCorps.contains(selection.anchorNode)) {
      selectionCorps = selection.getRangeAt(0).cloneRange();
      majActifs();
    }
  }

  function majActifs() {
    actifs = {
      gras: document.queryCommandState('bold'),
      italique: document.queryCommandState('italic'),
      souligne: document.queryCommandState('underline'),
      barre: document.queryCommandState('strikeThrough'),
      puces: document.queryCommandState('insertUnorderedList'),
      numerotee: document.queryCommandState('insertOrderedList'),
    };
  }

  function commande(nom, valeur = null) {
    if (!champCorps) return;
    champCorps.focus();
    if (selectionCorps) {
      const selection = window.getSelection();
      selection.removeAllRanges();
      selection.addRange(selectionCorps);
    }
    document.execCommand('styleWithCSS', false, false);
    document.execCommand(nom, false, valeur);
    corpsModifie = true;
    montrerCouleurs = false;
    majActifs();
    programmerSauvegarde();
  }

  // Les <select> reviennent à leur étiquette après le geste : ce sont
  // des COMMANDES (appliquer une police à la sélection), pas des états —
  // une sélection mêlée n'a pas UNE police à montrer.
  function commandeSelect(evenement, nom) {
    const valeur = evenement.target.value;
    evenement.target.value = '';
    if (valeur) commande(nom, valeur);
  }
</script>

<svelte:document onselectionchange={surSelection} />

<!-- Le menu de suggestions (PLAN-RETOURS-5) : nom d'affichage montré,
     adresse NUE insérée (D3). Un seul menu à la fois, sous le champ
     actif ; `onmousedown` neutralisé pour que le clic n'emporte pas le
     focus (le blur fermerait le menu avant le clic). -->
{#snippet menuSuggestions()}
  <ul class="suggestions" role="listbox" aria-label={t('compo.suggestions')}
      data-testid="composition-suggestions">
    {#each suggestions as choisie, i (choisie.address)}
      <li role="option" aria-selected={i === choixSuggere}>
        <button type="button" class="suggestion" class:choisie={i === choixSuggere}
                data-testid="suggestion-adresse" tabindex="-1"
                onmousedown={(e) => e.preventDefault()}
                onclick={() => insererSuggestion(choisie)}>
          {#if choisie.name}<span class="nom">{choisie.name}</span>{/if}
          <span class="adresse">{choisie.address}</span>
        </button>
      </li>
    {/each}
  </ul>
{/snippet}

{#if visible}
  <div class="scrim" data-testid="composition">
    <div class="carte" bind:this={carte} role="dialog" aria-modal="true" aria-label={t(KICKERS[mode])}>
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
          <input type="text" bind:this={champA} bind:value={a}
                 oninput={(e) => surFrappeAdresse('a', e.currentTarget.value)}
                 onkeydown={clavierAdresse} onblur={fermerSuggestions}
                 placeholder={t('compo.destinataire')} data-testid="composition-a">
          {#if champSuggere === 'a'}{@render menuSuggestions()}{/if}
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
            <input type="text" bind:this={champCc} bind:value={cc}
                   oninput={(e) => surFrappeAdresse('cc', e.currentTarget.value)}
                   onkeydown={clavierAdresse} onblur={fermerSuggestions}
                   placeholder={t('compo.destinataire')} data-testid="composition-cc">
            {#if champSuggere === 'cc'}{@render menuSuggestions()}{/if}
          </div>
        {/if}
        {#if montrerCci}
          <div class="rang">
            <span class="etiquette">{t('compo.cci')}</span>
            <input type="text" bind:this={champCci} bind:value={cci}
                   oninput={(e) => surFrappeAdresse('cci', e.currentTarget.value)}
                   onkeydown={clavierAdresse} onblur={fermerSuggestions}
                   placeholder={t('compo.destinataire')} data-testid="composition-cci">
            {#if champSuggere === 'cci'}{@render menuSuggestions()}{/if}
          </div>
        {/if}
        <div class="rang">
          <span class="etiquette">{t('conv.objet')}</span>
          <input type="text" bind:value={objet} oninput={programmerSauvegarde}
                 placeholder={t('compo.objetPlaceholder')} data-testid="composition-objet">
        </div>
      </div>
      <div class="zone-corps" bind:this={zoneCorps}>
        <!-- L'éditeur riche (R4) : contenteditable, contenu posé par
             `poserCorps`, lu par `chargeCorps` — jamais de bind. Le
             placeholder vit en CSS (:empty::before). La sélection est
             suivie par le seul `selectionchange` du document (il couvre
             clavier ET souris — pas de doublon onkeyup/onmouseup). -->
        <div class="corps-editeur" contenteditable="true" role="textbox" aria-multiline="true"
             tabindex="0"
             bind:this={champCorps} oninput={surFrappeCorps}
             data-placeholder={t('compo.corpsPlaceholder')}
             aria-label={t('compo.corpsPlaceholder')}
             data-testid="composition-corps"></div>
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
      <!-- La barre RÉELLE (R4, D1 : exactement les boutons demandés —
           Lien et Citation retirés). `onmousedown` neutralisé partout :
           un bouton de format ne vole jamais la sélection du corps. -->
      <div class="format" data-testid="composition-format">
        <select class="select-format" aria-label={t('compo.police')} title={t('compo.police')}
                data-testid="composition-format-police"
                onchange={(e) => commandeSelect(e, 'fontName')}>
          <option value="" disabled selected hidden>{t('compo.police')}</option>
          <option value="sans-serif">{t('compo.policeSans')}</option>
          <option value="serif">{t('compo.policeSerif')}</option>
          <option value="monospace">{t('compo.policeMono')}</option>
        </select>
        <select class="select-format" aria-label={t('compo.taille')} title={t('compo.taille')}
                data-testid="composition-format-taille"
                onchange={(e) => commandeSelect(e, 'fontSize')}>
          <option value="" disabled selected hidden>{t('compo.taille')}</option>
          <option value="2">{t('compo.taillePetit')}</option>
          <option value="3">{t('compo.tailleNormal')}</option>
          <option value="4">{t('compo.tailleGrand')}</option>
          <option value="6">{t('compo.tailleTresGrand')}</option>
        </select>
        <span class="sep" aria-hidden="true"></span>
        <button type="button" class="bouton-format" class:actif={actifs.gras}
                aria-label={t('compo.gras')} title={t('compo.gras')} aria-pressed={actifs.gras}
                data-testid="composition-format-gras"
                onmousedown={(e) => e.preventDefault()} onclick={() => commande('bold')}>
          <span class="ms" aria-hidden="true">format_bold</span></button>
        <button type="button" class="bouton-format" class:actif={actifs.italique}
                aria-label={t('compo.italique')} title={t('compo.italique')} aria-pressed={actifs.italique}
                data-testid="composition-format-italique"
                onmousedown={(e) => e.preventDefault()} onclick={() => commande('italic')}>
          <span class="ms" aria-hidden="true">format_italic</span></button>
        <button type="button" class="bouton-format" class:actif={actifs.souligne}
                aria-label={t('compo.souligne')} title={t('compo.souligne')} aria-pressed={actifs.souligne}
                data-testid="composition-format-souligne"
                onmousedown={(e) => e.preventDefault()} onclick={() => commande('underline')}>
          <span class="ms" aria-hidden="true">format_underlined</span></button>
        <button type="button" class="bouton-format" class:actif={actifs.barre}
                aria-label={t('compo.barre')} title={t('compo.barre')} aria-pressed={actifs.barre}
                data-testid="composition-format-barre"
                onmousedown={(e) => e.preventDefault()} onclick={() => commande('strikeThrough')}>
          <span class="ms" aria-hidden="true">strikethrough_s</span></button>
        <span class="groupe-couleur">
          <button type="button" class="bouton-format"
                  aria-label={t('compo.couleur')} title={t('compo.couleur')}
                  data-testid="composition-format-couleur"
                  onmousedown={(e) => e.preventDefault()}
                  onclick={() => (montrerCouleurs = !montrerCouleurs)}>
            <span class="ms" aria-hidden="true">format_color_text</span></button>
          {#if montrerCouleurs}
            <div class="palette" data-testid="composition-palette">
              {#each COULEURS as couleur (couleur)}
                <button type="button" class="teinte" style="background:{couleur}"
                        aria-label={couleur}
                        onmousedown={(e) => e.preventDefault()}
                        onclick={() => commande('foreColor', couleur)}></button>
              {/each}
            </div>
          {/if}
        </span>
        <span class="sep" aria-hidden="true"></span>
        <button type="button" class="bouton-format"
                aria-label={t('compo.alignerGauche')} title={t('compo.alignerGauche')}
                data-testid="composition-format-gauche"
                onmousedown={(e) => e.preventDefault()} onclick={() => commande('justifyLeft')}>
          <span class="ms" aria-hidden="true">format_align_left</span></button>
        <button type="button" class="bouton-format"
                aria-label={t('compo.alignerCentre')} title={t('compo.alignerCentre')}
                data-testid="composition-format-centre"
                onmousedown={(e) => e.preventDefault()} onclick={() => commande('justifyCenter')}>
          <span class="ms" aria-hidden="true">format_align_center</span></button>
        <button type="button" class="bouton-format"
                aria-label={t('compo.alignerDroite')} title={t('compo.alignerDroite')}
                data-testid="composition-format-droite"
                onmousedown={(e) => e.preventDefault()} onclick={() => commande('justifyRight')}>
          <span class="ms" aria-hidden="true">format_align_right</span></button>
        <span class="sep" aria-hidden="true"></span>
        <button type="button" class="bouton-format" class:actif={actifs.puces}
                aria-label={t('compo.listePuces')} title={t('compo.listePuces')} aria-pressed={actifs.puces}
                data-testid="composition-format-puces"
                onmousedown={(e) => e.preventDefault()} onclick={() => commande('insertUnorderedList')}>
          <span class="ms" aria-hidden="true">format_list_bulleted</span></button>
        <button type="button" class="bouton-format" class:actif={actifs.numerotee}
                aria-label={t('compo.listeNumerotee')} title={t('compo.listeNumerotee')} aria-pressed={actifs.numerotee}
                data-testid="composition-format-numerotee"
                onmousedown={(e) => e.preventDefault()} onclick={() => commande('insertOrderedList')}>
          <span class="ms" aria-hidden="true">format_list_numbered</span></button>
        <button type="button" class="bouton-format"
                aria-label={t('compo.retraitMoins')} title={t('compo.retraitMoins')}
                data-testid="composition-format-retrait-moins"
                onmousedown={(e) => e.preventDefault()} onclick={() => commande('outdent')}>
          <span class="ms" aria-hidden="true">format_indent_decrease</span></button>
        <button type="button" class="bouton-format"
                aria-label={t('compo.retraitPlus')} title={t('compo.retraitPlus')}
                data-testid="composition-format-retrait-plus"
                onmousedown={(e) => e.preventDefault()} onclick={() => commande('indent')}>
          <span class="ms" aria-hidden="true">format_indent_increase</span></button>
        <span class="sep" aria-hidden="true"></span>
        <button type="button" class="bouton-format"
                aria-label={t('compo.effacerFormat')} title={t('compo.effacerFormat')}
                data-testid="composition-format-effacer"
                onmousedown={(e) => e.preventDefault()} onclick={() => commande('removeFormat')}>
          <span class="ms" aria-hidden="true">format_clear</span></button>
      </div>
      {#if demandeSuppr}
        <!-- R3/D3 : la confirmation vit DANS le pied, à la place des
             boutons — un brouillon jeté ne revient pas, le geste dit ce
             qu'il fait avant de le faire. -->
        <div class="pied confirmation" data-testid="composition-suppr-carte">
          <span class="avert-suppr">{t('compo.supprConfirme')}</span>
          <span class="essor"></span>
          <button type="button" class="danger" data-testid="composition-suppr-confirmer"
                  onclick={supprimerBrouillon}>
            <span class="ms" aria-hidden="true">delete</span>{t('action.supprimer')}</button>
          <button type="button" class="annuler" data-testid="composition-suppr-annuler"
                  onclick={() => (demandeSuppr = false)}>{t('action.annuler')}</button>
        </div>
      {:else}
        <div class="pied">
          <button type="button" class="principal" data-testid="composition-envoyer"
                  disabled={envoiEnCours} onclick={envoyer}>
            <span class="ms" aria-hidden="true">send</span>{t('action.envoyer')}</button>
          <button type="button" onclick={joindre} data-testid="composition-joindre">
            <span class="ms" aria-hidden="true">attach_file</span>{t('compo.joindre')}</button>
          <button type="button" onclick={enregistrerBrouillon} data-testid="composition-brouillon">
            <span class="ms" aria-hidden="true">drafts</span>{t('compo.enregistrerBrouillon')}</button>
          <span class="essor"></span>
          {#if peutSupprimer}
            <!-- Le geste destructif à DROITE, détaché du cluster d'envoi
                 (moins de mégarde), avant « Annuler » qui, lui, conserve. -->
            <button type="button" class="supprimer" data-testid="composition-supprimer"
                    onclick={() => (demandeSuppr = true)}>
              <span class="ms" aria-hidden="true">delete</span>{t('compo.supprimerBrouillon')}</button>
          {/if}
          <button type="button" class="annuler" data-testid="composition-annuler"
                  onclick={fermer}>{t('action.annuler')}</button>
        </div>
      {/if}
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
    /* Le menu de suggestions s'ancre au rang de SON champ. */
    position:relative;
  }
  .suggestions {
    position:absolute; top:100%; left:66px; z-index:5;
    min-width:280px; max-width:440px;
    margin:2px 0 0; padding:6px; list-style:none;
    background:var(--surface); border:1px solid var(--border);
    border-radius:8px; box-shadow:var(--shadow);
    display:flex; flex-direction:column; gap:2px;
  }
  .suggestion {
    width:100%; display:flex; align-items:baseline; gap:8px;
    padding:6px 8px; border:none; background:transparent; border-radius:6px;
    cursor:pointer; font-size:13px; text-align:left; font-family:inherit;
  }
  .suggestion:hover { background:var(--hover); }
  .suggestion.choisie { background:var(--sel); }
  .suggestion .nom { color:var(--ink); font-weight:600; white-space:nowrap; }
  .suggestion .adresse { color:var(--muted); overflow:hidden; text-overflow:ellipsis; }
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
    min-height:220px; flex:1; overflow:auto;
  }
  .corps-editeur {
    flex:1; width:100%; min-height:180px; font-size:15px; line-height:1.65;
    color:var(--ink); border:none; outline:none;
    background:transparent; font-family:inherit;
    overflow-wrap:break-word;
  }
  /* Le placeholder du textarea, refait : visible tant que le corps est
     vide, dans la teinte atténuée. */
  .corps-editeur:empty::before {
    content:attr(data-placeholder); color:var(--muted); pointer-events:none;
  }
  /* La citation riche : le filet gauche que `quote_reply_html` pose en
     style inline est la référence ; ceci ne stylise que les blockquotes
     nés du retrait (indent), sans style propre. */
  .corps-editeur :global(blockquote) { margin:0 0 0 0.8ex; }

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
    background:var(--panel); display:flex; align-items:center; gap:6px;
    flex-wrap:wrap;
  }
  .bouton-format {
    height:32px; min-width:32px; padding:0 6px; display:inline-flex;
    align-items:center; justify-content:center; font-size:13px;
    color:var(--ink2); background:var(--surface); cursor:pointer;
    border:1px solid var(--border); border-radius:6px;
  }
  .bouton-format:hover { background:var(--sel); color:var(--ink); }
  /* L'état actif dit ce que porte la sélection (aria-pressed idem). */
  .bouton-format.actif {
    background:var(--sel); color:var(--accent); border-color:var(--accent);
  }
  .bouton-format .ms { font-size:18px; }
  .select-format {
    height:32px; padding:0 8px; font:inherit; font-size:13px;
    color:var(--ink2); background:var(--surface); cursor:pointer;
    border:1px solid var(--border); border-radius:6px;
  }
  .select-format option { background:var(--surface); color:var(--ink); }
  .sep {
    width:1px; height:20px; background:var(--border); flex:none;
    margin:0 4px;
  }
  /* Le nuancier (D3) : douze teintes fixes, au-dessus de la barre. */
  .groupe-couleur { position:relative; display:inline-flex; }
  .palette {
    position:absolute; bottom:38px; left:0; z-index:1;
    display:grid; grid-template-columns:repeat(6, 22px); gap:6px;
    padding:10px; background:var(--surface);
    border:1px solid var(--border); border-radius:8px;
    box-shadow:var(--shadow);
  }
  .teinte {
    height:22px; width:22px; min-width:0; padding:0;
    border:1px solid var(--border); border-radius:4px; cursor:pointer;
  }
  .teinte:hover { outline:2px solid var(--accent); outline-offset:1px; }

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
  /* Le ressort pousse le geste destructif et « Annuler » à droite,
     séparés du cluster Envoyer/Joindre/Enregistrer. */
  .essor { flex:1; }
  /* R3 : « Supprimer le brouillon » et sa confirmation — teinte d'alerte,
     jamais la couleur d'accent (qui appelle au clic). */
  .supprimer { color:var(--alert); border-color:var(--border); }
  .supprimer:hover { background:var(--alert); color:var(--onAccent); border-color:var(--alert); }
  .supprimer .ms { font-size:18px; }
  .confirmation .avert-suppr { font-size:13px; color:var(--alert); font-weight:600; }
  .danger {
    font-weight:600; color:var(--onAccent); background:var(--alert);
    border-color:var(--alert);
  }
  .danger:hover { background:var(--alert); border-color:var(--alert); filter:brightness(1.08); }
</style>
