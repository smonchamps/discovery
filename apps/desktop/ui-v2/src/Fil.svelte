<script>
  // Le FIL en cartes — l'objet unique de lecture (UI v3, verdict CE du
  // 2026-08-16, décision D4 ; dessin au trait de la maquette depuis le
  // terrain A45) : titre, puces d'inventaire, boutons nus à droite,
  // cartes aux avatars — repliées une ligne, dépliées en carte pleine
  // (« adresse · à destinataire », heure longue) —, fichiers joints,
  // brouillon du fil, barre des cinq gestes directs (D5 — jamais de
  // menu « Plus », exception b des annotations ; le « ⋯ » par message
  // de la maquette attend ses actions). Le volet de lecture
  // et l'écran 03 le montent TEL QUEL : deux cadres, un objet — l'état
  // vit dans lib/fil.svelte.js et survit au changement de cadre.
  //
  // Invariant S1 : une iframe sandbox `allow-same-origin` SANS
  // allow-scripts par message déplié, corps servi par le cœur,
  // liens interceptés (lib/liens.js).
  import {
    fil, cleMsg, basculerMessage, toutDeplier, toutReplier, afficherImages, estEcho,
  } from './lib/fil.svelte.js';
  import { appel, choisirDestination } from './lib/transport.js';
  import { brancherLiens } from './lib/liens.js';
  import { quand, quandLong } from './lib/quand.js';
  import { initiales } from './lib/initiales.js';
  import { activation } from './lib/clavier.js';
  import { t } from './lib/texte.svelte.js';

  let {
    brouillons = [],
    onreprendre = () => {},
    onarchiver = () => {},
    onsupprimer = () => {},
    onrepondre = () => {},
    onrepondretous = () => {},
    ontransferer = () => {},
    // R2 (PLAN-RETOURS-3, D2) : signaler un fil comme indésirable, ou le
    // ramener en Réception. Le geste vit dans la barre du fil, par fil ;
    // `estIndesirable` bascule le libellé selon la vue courante.
    onspam = () => {},
    onnonspam = () => {},
    estIndesirable = false,
    onflash = () => {},
    // Le cadre volet passe le geste d'agrandissement ; l'écran 03, non.
    onagrandir = null,
  } = $props();

  // Depuis R4 (PLAN-RETOURS-3, D4) Répondre / Répondre à tous /
  // Transférer visent CHAQUE message (barre par message) — plus de
  // `cible()` unique au fil. Le raccourci clavier `r`, lui, reste sur la
  // sélection de liste (App.svelte), inchangé.
  //
  // Le compte de pièces FRAIS (après-scan, terrain CE 2026-08-14) : la
  // ligne porte celui d'AVANT l'ouverture — le store retient celui de
  // message_body dès que le corps est servi.
  const nbPiecesDe = (m) => fil.nbPieces[cleMsg(m)] ?? m.attachment_count;

  // Le brouillon du fil ouvert — le plus récent (B-D5).
  const brouillonDuFil = $derived.by(() => {
    if (!fil.ligne || fil.ligne.thread_id == null) return null;
    let retenu = null;
    for (const b of brouillons) {
      if (b.thread_id !== fil.ligne.thread_id) continue;
      if (!retenu || b.updated_epoch > retenu.updated_epoch) retenu = b;
    }
    return retenu;
  });

  // Les puces d'inventaire de la maquette (terrain A45) : n messages
  // TOUJOURS dit — « 1 message » compris —, fichiers SOMMÉS sur le fil
  // (la ligne ne porte que le compte de SON message).
  const nbMessages = $derived(fil.messages.length || fil.ligne?.thread_size || 1);
  // La bascule se DÉRIVE de l'état réel (terrain A47) : tout déplié →
  // « Tout replier » — un fil d'un message s'ouvre donc dessus.
  const tousDeplies = $derived(
    fil.messages.length > 0 && fil.messages.every((m) => fil.deplies[cleMsg(m)]),
  );
  const totalPieces = $derived(
    fil.messages.length
      ? fil.messages.reduce((n, m) => n + (nbPiecesDe(m) || 0), 0)
      : (fil.ligne ? nbPiecesDe(fil.ligne) : 0),
  );

  // « À » n'est pas stocké par le cœur : la règle du prototype —
  // message de soi → premier autre correspondant, sinon le compte.
  // Depuis A45 l'en-tête déplié dit « adresse · à NOM » (maquette) :
  // pour un message d'autrui, notre nom vient de notre propre copie du
  // fil (Envoyés) ; sans elle, l'adresse du compte — le fait honnête.
  const propre = (m) => fil.ligne && m.sender_address === fil.ligne.account_email;
  // R4 (PLAN-RETOURS-MAIL) : pour NOTRE message, le destinataire stocké
  // (`to_addrs`, tiré de la même ENVELOPE à la synchro) dit à qui il est
  // parti — un envoi ISOLÉ n'a personne d'autre à deviner dans le fil, et
  // le repli sur l'adresse du compte affichait « à soi-même » (le cas
  // « Test PJ 3 »). Ordre : la donnée stockée d'abord, puis l'ancienne
  // heuristique (autre correspondant du fil), enfin l'adresse du compte.
  function destinataire(m) {
    if (propre(m) && (m.to_addrs?.length ?? 0) > 0) {
      return m.to_addrs.join(', ');
    }
    const vise = propre(m)
      ? fil.messages.find((x) => x.sender_address && x.sender_address !== m.sender_address)
      : fil.messages.find((x) => propre(x));
    return vise ? vise.sender : (fil.ligne?.account_email ?? '');
  }

  // La hauteur du corps suit le CONTENU (terrain A47), jamais un
  // gabarit fixe : l'iframe est same-origin SANS scripts (S1) — le
  // parent mesure le document assaini et pose la hauteur. Re-mesure au
  // chargement (srcdoc posé, images accordées) et au changement de
  // LARGEUR seulement (re-flow du texte) — jamais sur sa propre pose
  // de hauteur, pour ne pas boucler l'observateur.
  function corpsAuto(iframe) {
    let largeur = 0;
    const mesurer = () => {
      const doc = iframe.contentDocument;
      if (!doc?.documentElement) return;
      iframe.style.height = '0';
      iframe.style.height = `${doc.documentElement.scrollHeight}px`;
    };
    const surLoad = () => {
      largeur = iframe.offsetWidth;
      mesurer();
    };
    iframe.addEventListener('load', surLoad);
    const observateur = new ResizeObserver(() => {
      if (iframe.offsetWidth === largeur) return;
      largeur = iframe.offsetWidth;
      mesurer();
    });
    observateur.observe(iframe);
    return {
      destroy() {
        observateur.disconnect();
        iframe.removeEventListener('load', surLoad);
      },
    };
  }

  // En-vol transitoire : local au composant, rien à partager entre
  // cadres (revue v3 — le store ne porte que l'état du fil).
  let enregistrements = $state({});
  async function enregistrer(m, piece) {
    const k = `${cleMsg(m)}#${piece.index}`;
    if (enregistrements[k]) return;
    enregistrements[k] = true;
    try {
      // R1 (PLAN-RETOURS-4, D2) : le chemin proposé (Téléchargements +
      // nom assaini par le cœur), puis le dialogue « Enregistrer sous »
      // — l'utilisateur choisit dossier ET nom. Annuler = rien, ni
      // toast ni erreur ; le rapatriement des octets n'a lieu qu'après
      // le choix (jamais de fetch inutile si l'on renonce).
      const defaut = await appel('chemin_enregistrement_suggere', { name: piece.name });
      const dest = await choisirDestination(defaut);
      if (!dest) return;
      const chemin = await appel('save_attachment', {
        accountId: m.account_id,
        mailbox: m.mailbox,
        uid: m.uid,
        index: piece.index,
        dest,
      });
      onflash(t('toast.pieceEnregistree', { chemin }));
    } catch (err) {
      onflash(t('erreur.enregistrement', { err }));
    } finally {
      enregistrements[k] = false;
    }
  }
</script>

{#if fil.ligne}
  <!-- Le cadre volet est À PLAT (terrain A46, dessin du prototype :
       .voletLecture) : pas d'élévation englobante, pas de filets —
       seules les cartes de message s'élèvent, et TOUT défile en flot,
       barre d'actions comprise. L'écran 03 garde sa carte pleine. -->
  <div class="objet-fil" class:volet={fil.cadre === 'volet'}>
    <div class="tete">
      <h3 class="titre" data-testid="fil-sujet">{fil.ligne.subject}</h3>
      <!-- Le rang de la maquette (terrain A45) : puces d'inventaire à
           gauche, boutons NUS à droite — « Tout déplier » au bord. -->
      <div class="puces" data-testid="fil-puces">
        <span class="puce"><span class="ms" aria-hidden="true">forum</span>{t('puce.messages', { n: nbMessages })}</span>
        {#if totalPieces > 0}
          <span class="puce"><span class="ms" aria-hidden="true">attach_file</span>{t('puce.fichiers', { n: totalPieces })}</span>
        {/if}
        <span class="essor"></span>
        {#if onagrandir}
          <!-- V-D2 : un message SEUL n'a pas de conversation à ouvrir
               — le bouton reste, inerte et dit tel. « Ouvrir » porte
               son propre glyphe (A46) : une icône, un sens (A3). -->
          <button type="button" class="nu" data-testid="voir-conversation"
                  class:inerte={fil.ligne.thread_id == null}
                  aria-disabled={fil.ligne.thread_id == null}
                  tabindex={fil.ligne.thread_id != null ? 0 : -1}
                  onclick={() => fil.ligne.thread_id != null && onagrandir(fil.ligne)}>
            <span class="ms" aria-hidden="true">open_in_full</span>{t('lecture.ouvrir')}</button>
        {/if}
        <!-- La bascule (A46, dérivée depuis A47) : « Tout replier »
             quand TOUT est déplié — fil d'un message compris —, sinon
             « Tout déplier » ; les dépliages manuels la font suivre. -->
        {#if tousDeplies}
          <button type="button" class="nu" data-testid="tout-replier" onclick={toutReplier}>
            <span class="ms" aria-hidden="true">unfold_less</span>{t('conv.replier')}</button>
        {:else}
          <button type="button" class="nu" data-testid="tout-deplier" onclick={toutDeplier}>
            <span class="ms" aria-hidden="true">unfold_more</span>{t('conv.deplier')}</button>
        {/if}
      </div>
    </div>

    <div class="fil">
      {#each fil.messages as m (cleMsg(m))}
        {@const k = cleMsg(m)}
        {#if fil.deplies[k]}
          <article class="deplie" data-testid="message-deplie">
            <!-- L'en-tête de la maquette (A45) : avatar, nom sur
                 l'adresse · destinataire, heure longue — le bloc
                 De/À/Objet a disparu, la tête dit tout. -->
            <div class="tete-message" role="button" tabindex="0"
                 aria-expanded="true"
                 onclick={() => basculerMessage(m)} onkeydown={activation(() => basculerMessage(m))}>
              <span class="avatar" aria-hidden="true">{initiales(m.sender)}</span>
              <span class="qui">
                <span class="auteur">{m.sender}</span>
                <span class="adr">{t('conv.adrDest', { adr: m.sender_address || m.sender, qui: destinataire(m) })}</span>
              </span>
              <span class="quand">{quandLong(m.epoch)}</span>
            </div>
            <div class="contenu">
              {#if (fil.imagesBloquees[k] ?? 0) > 0}
                <div class="garde-images" data-testid="garde-images">
                  <span class="ms" aria-hidden="true">visibility_off</span>
                  <span class="garde-texte">{t('lecture.imagesBloquees', { n: fil.imagesBloquees[k] })}</span>
                  <button type="button" data-testid="afficher-images"
                          onclick={() => afficherImages(m)}>
                    {t('lecture.afficherImages')}</button>
                </div>
              {/if}
              <iframe class="corps" sandbox="allow-same-origin" srcdoc={fil.corps[k] ?? ''}
                      title={t('lecture.corps')} use:corpsAuto
                      onload={(ev) => brancherLiens(ev.currentTarget)}></iframe>
              <!-- Un écho de GESTE n'a pas de métadonnées par pièce
                   (elles meurent avec la source) : la section ne se
                   montre que si des puces existent — jamais un titre
                   sans rien dessous (PLAN-RETOURS-5). -->
              {#if nbPiecesDe(m) > 0 && (!estEcho(m) || (fil.pieces[k] ?? []).length > 0)}
                <div class="fichiers" data-testid="lecture-fichiers">
                  <p class="titre-fichiers">{t('conv.fichiersJoints')}</p>
                  <!-- R2 (PLAN-RETOURS-4, D4) : nom ET poids dans la MÊME
                       puce cliquable — exception assumée à « 1 puce = 1
                       information », icône unique (même objet manipulable
                       que la puce du composeur, pas deux lectures). -->
                  <!-- Les puces d'un écho sont INERTES (PLAN-RETOURS-5,
                       D2) : les octets ont quitté le journal à l'envoi —
                       nom et poids se montrent, rien ne s'enregistre
                       pendant la fenêtre de réconciliation. -->
                  <div class="puces">
                    {#each fil.pieces[k] ?? [] as piece (piece.index)}
                      <button type="button" class="puce bouton" data-testid="piece-jointe"
                              disabled={estEcho(m) || enregistrements[`${k}#${piece.index}`]}
                              onclick={() => !estEcho(m) && enregistrer(m, piece)}
                              title={estEcho(m) ? undefined : t('lecture.enregistrer')}>
                        <span class="ms" aria-hidden="true">description</span>
                        <span class="nom">{piece.name}</span><span class="taille">{piece.size}</span></button>
                    {/each}
                  </div>
                </div>
              {/if}
            </div>
            <!-- R4 (PLAN-RETOURS-3, D4) : les gestes de réponse EN BAS de
                 CHAQUE message, visant CE message — on répond quand on a
                 fini de lire (convention Gmail/Outlook). « Répondre à
                 tous » entre Répondre et Transférer (A14). -->
            <!-- R4 (constat terrain 2026-08-18) : les TROIS gestes sur
                 CHAQUE message, le nôtre compris — on répond parfois sur
                 son propre message. Le cœur adresse alors la réponse aux
                 destinataires d'origine (reply_context/reply_all), jamais
                 à soi-même. -->
            <div class="actions-message" data-testid="actions-message">
              <button type="button" class="principal" data-testid="repondre"
                      onclick={() => onrepondre(m)}>
                <span class="ms" aria-hidden="true">reply</span>{t('action.repondre')}</button>
              <button type="button" data-testid="repondre-tous"
                      onclick={() => onrepondretous(m)}>
                <span class="ms" aria-hidden="true">reply_all</span>{t('action.repondreTous')}</button>
              <button type="button" data-testid="transferer"
                      onclick={() => ontransferer(m)}>
                <span class="ms miroir" aria-hidden="true">reply</span>{t('action.transferer')}</button>
            </div>
          </article>
        {:else}
          <div class="replie" data-testid="message-replie"
               role="button" tabindex="0" aria-expanded="false"
               onclick={() => basculerMessage(m)} onkeydown={activation(() => basculerMessage(m))}>
            <span class="avatar petit" aria-hidden="true">{initiales(m.sender)}</span>
            <span class="auteur">{m.sender}</span>
            <span class="apercu">{m.preview ?? ''}</span>
            <span class="quand">{quandLong(m.epoch)}</span>
          </div>
        {/if}
      {/each}
      {#if brouillonDuFil}
        <div class="replie brouillon" data-testid="conv-brouillon"
             role="button" tabindex="0"
             onclick={() => onreprendre(brouillonDuFil)}
             onkeydown={activation(() => onreprendre(brouillonDuFil))}>
          <span class="mention"><span class="ms" aria-hidden="true">edit_note</span>{t('conv.brouillon')}</span>
          <span class="apercu">{brouillonDuFil.body}</span>
          <span class="quand">{quand(Math.floor(brouillonDuFil.updated_epoch / 1000))}</span>
          <span class="reprendre">{t('action.reprendre')}</span>
        </div>
      {/if}
    </div>

    <!-- La barre du fil = gestes de TRI seuls (D5) : Répondre/Répondre à
         tous/Transférer ont rejoint chaque message (D4). Signaler comme
         spam s'y range (D2), ou « Ce n'est pas un spam » en vue
         Indésirables. -->
    <div class="actions">
      <button type="button" data-testid="archiver" onclick={() => onarchiver(fil.ligne)}>
        <span class="ms" aria-hidden="true">archive</span>{t('action.archiver')}</button>
      <button type="button" data-testid="supprimer" onclick={() => onsupprimer(fil.ligne)}>
        <span class="ms" aria-hidden="true">delete</span>{t('action.supprimer')}</button>
      {#if estIndesirable}
        <button type="button" data-testid="pas-spam" onclick={() => onnonspam(fil.ligne)}>
          <span class="ms" aria-hidden="true">inbox</span>{t('action.pasSpam')}</button>
      {:else}
        <button type="button" data-testid="signaler-spam" onclick={() => onspam(fil.ligne)}>
          <span class="ms" aria-hidden="true">report</span>{t('action.signalerSpam')}</button>
      {/if}
    </div>
  </div>
{/if}

<style>
  /* Géométrie héritée de l'écran 03 (verbatim du prototype) — le même
     objet dans les deux cadres, seule la largeur disponible change. */
  .objet-fil { display:flex; flex-direction:column; min-height:0; flex:1; }
  .tete {
    padding:22px 26px 16px; border-bottom:1px solid var(--border);
    display:flex; flex-direction:column; gap:12px; flex:none;
  }
  .titre {
    margin:0; font-size:24px; font-weight:600; line-height:1.2;
    letter-spacing:-.01em; color:var(--ink);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
  }
  .puces { display:flex; align-items:center; gap:10px; flex-wrap:wrap; }
  .puce {
    height:32px; padding:0 12px; display:inline-flex; align-items:center;
    gap:8px; font-size:13px; color:var(--ink2); background:var(--surface);
    border:1px solid var(--border); border-radius:6px; white-space:nowrap;
  }
  .puce.bouton { cursor:pointer; }
  .puce.bouton:hover { background:var(--sel); }
  /* Les boutons NUS de la maquette (A45) : bordure et fond effacés,
     gabarit mini 26 px — « Tout déplier », « Voir la conversation ». */
  .essor { flex:1; }
  .nu {
    height:26px; padding:0 9px; display:inline-flex; align-items:center;
    gap:6px; font-size:12px; color:var(--ink2); background:none;
    border:1px solid transparent; border-radius:6px; cursor:pointer;
    white-space:nowrap;
  }
  .nu:hover { background:var(--sel); }
  .nu.inerte { cursor:default; opacity:.55; }
  .nu.inerte:hover { background:none; }
  /* L'avatar aux initiales des cartes (A45) — le dessin de la liste
     (E2) : 28 px déplié, 26 px replié. */
  .avatar {
    width:28px; height:28px; border-radius:50%; background:var(--panel);
    border:1px solid var(--border); display:grid; place-items:center;
    font-size:11px; font-weight:600; color:var(--ink2); flex:none;
  }
  .avatar.petit { width:26px; height:26px; }
  .fil { flex:1; overflow-y:auto; padding:14px 26px; min-height:0; }
  .replie {
    display:flex; align-items:center; gap:10px; padding:12px 20px;
    margin-bottom:12px; background:var(--surface);
    border:1px solid var(--border); border-radius:10px; cursor:pointer;
    font-size:13px;
  }
  .replie:hover { background:var(--hover); }
  .replie .auteur { font-weight:600; color:var(--ink); flex:none; }
  .replie .apercu {
    flex:1; min-width:0; color:var(--muted);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
  }
  .quand { margin-left:auto; color:var(--muted); font-size:12px; flex:none; white-space:nowrap; }
  .replie.brouillon { border:1.5px dashed var(--accent); background:none; }
  .replie.brouillon .mention {
    color:var(--alert); font-weight:600; display:inline-flex;
    align-items:center; gap:6px; flex:none;
  }
  .replie.brouillon .reprendre { color:var(--accent); font-weight:600; flex:none; }
  .deplie {
    background:var(--surface); border:1px solid var(--border);
    border-radius:10px; box-shadow:var(--shadow); margin-bottom:12px;
    display:flex; flex-direction:column;
  }
  /* L'en-tête de la maquette : avatar · (nom / adresse · à X) · quand. */
  .tete-message {
    display:flex; align-items:center; gap:10px; padding:12px 20px;
    border-bottom:1px solid var(--border); cursor:pointer;
  }
  .tete-message .qui { min-width:0; display:flex; flex-direction:column; }
  .tete-message .auteur {
    font-size:15px; font-weight:600; color:var(--ink);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
  }
  .tete-message .adr {
    font-size:12px; color:var(--muted);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
  }
  .contenu { padding:14px 20px 18px; display:flex; flex-direction:column; gap:12px; }
  .garde-images {
    padding:10px 14px; display:flex; align-items:center; gap:10px;
    font-size:13px; color:var(--ink2); background:var(--panel);
    border:1px solid var(--border); border-radius:6px;
  }
  .garde-images .ms { color:var(--muted); }
  .garde-texte { flex:1; }
  .garde-images button {
    height:26px; padding:0 10px; font-size:12px; color:var(--ink);
    background:var(--surface); border:1px solid var(--border);
    border-radius:6px; cursor:pointer;
  }
  .garde-images button:hover { background:var(--sel); }
  .corps {
    /* Déborde de 12 px : la gouttière interne du document assaini
       (mail-render) ramène le texte au fil du padding de la carte. Le
       fond au jeton — le document bake la même valeur (revue A42).
       La HAUTEUR n'est pas ici : elle suit le contenu, posée par
       corpsAuto (A47) — jamais de gabarit fixe. */
    border:none; background:var(--surface); display:block;
    margin-left:-12px; width:calc(100% + 24px); height:0;
  }
  .titre-fichiers {
    margin:0 0 8px; font-size:12px; font-weight:600; letter-spacing:.1em;
    text-transform:uppercase; color:var(--muted);
  }
  .fichiers .puces { gap:8px; }
  .fichiers .puce { height:28px; }
  /* R2 : nom + poids dans la puce (dessin de la puce du composeur) —
     le nom à l'encre pleine, le poids atténué, l'espacement au gap. */
  .fichiers .puce .nom { color:var(--ink); }
  .fichiers .puce .taille { font-size:12px; color:var(--muted); }
  .actions {
    flex:none; padding:14px 26px; border-top:1px solid var(--border);
    display:flex; gap:12px; flex-wrap:wrap;
  }
  .actions button {
    height:32px; padding:0 16px; display:inline-flex; align-items:center;
    gap:8px; font-size:13px; color:var(--ink); background:var(--surface);
    border:1px solid var(--border); border-radius:6px; cursor:pointer;
  }
  .actions button:hover { background:var(--sel); }

  /* R4/D4 : la barre de réponse d'UN message — en bas de la carte, un
     filet la sépare du corps ; même gabarit de boutons que la barre du
     fil, un peu plus compacte (26 px, jetons nus au repos). */
  .actions-message {
    padding:12px 20px; border-top:1px solid var(--border);
    display:flex; gap:10px; flex-wrap:wrap;
  }
  .actions-message button {
    height:30px; padding:0 14px; display:inline-flex; align-items:center;
    gap:8px; font-size:13px; color:var(--ink); background:var(--surface);
    border:1px solid var(--border); border-radius:6px; cursor:pointer;
  }
  .actions-message button:hover { background:var(--sel); }
  .actions-message .principal {
    font-weight:600; color:var(--onAccent); background:var(--accent);
    border-color:var(--accent);
  }
  .actions-message .principal:hover { background:var(--accentH); border-color:var(--accentH); }

  /* Le cadre VOLET, à plat (A46) — la géométrie du prototype
     (.voletLecture / .lecture) : le volet défile en un seul flot, les
     filets et l'élévation appartiennent aux seules cartes ; le titre
     colle au dessin (.titreFil margin 2/4, sousTitre à 10 px des
     cartes), la barre d'actions suit le flot (barreActions, 14 px). */
  .objet-fil.volet { flex:none; min-height:100%; }
  .volet .tete { padding:0; border-bottom:none; gap:0; }
  .volet .titre { margin:2px 0 4px; }
  .volet .puces { margin:0 0 10px; }
  .volet .fil { flex:none; overflow-y:visible; padding:0; }
  .volet .replie, .volet .deplie { margin-bottom:0; margin-top:12px; }
  .volet .replie.brouillon { margin-top:12px; }
  .volet .actions { padding:14px 0 0; border-top:none; }
</style>
