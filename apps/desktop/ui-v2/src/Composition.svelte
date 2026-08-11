<script>
  // Surimpression de composition du prototype : 860 px, trois modes
  // (nouveau / répondre / transférer), câblée aux flux réels.
  //
  // Préremplissages : formes du prototype (« Re : » / « Tr : », amorce
  // « Bonjour Prénom, », puces des fichiers du message source) ; la
  // citation, elle, est RÉELLE — `reply_context` / `forward_context` du
  // cœur la préparent depuis le corps effectif (le prototype n'en avait
  // pas besoin, sa fiction s'arrêtait à l'amorce).
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
  // Inertes comme au prototype : barre G/I/S/Liste/Lien/Citation,
  // « Rendre indépendante », Cc/Cci ; « Joindre » répond par le toast du
  // prototype. Écart dit : la ligne « De » montre l'adresse seule — le
  // cœur ne stocke ni nom d'affichage ni étiquette de compte.
  import { appel } from './lib/transport.js';

  let {
    comptes = [],
    compte = null,
    onflash = () => {},
    onenvoye = () => {},
  } = $props();

  let visible = $state(false);
  let mode = $state('new');
  let expediteur = $state(null); // { account_id, email }
  let a = $state('');
  let objet = $state('');
  let corps = $state('');
  let fichiers = $state([]);
  let envoiEnCours = $state(false);
  let replyToMailbox = null;
  let replyToUid = null;
  let brouillonId = null;
  let brouillonEpoch = null;
  let minuterie;
  let jeton = 0;

  let champA = $state(null);
  let champCorps = $state(null);

  const KICKERS = { new: 'Nouveau message', reply: 'Répondre', forward: 'Transférer' };

  // Formes du prototype, à la lettre — le cœur produit « Re: » / « Fwd: »,
  // la surface parle français.
  const sujetRe = (s) => (/^re\s*:/i.test(s ?? '') ? s : `Re : ${s ?? ''}`);
  const sujetTr = (s) => (/^(tr|fwd|fw)\s*:/i.test(s ?? '') ? s : `Tr : ${s ?? ''}`);

  function compteDe(accountId) {
    const connu = comptes.find((c) => c.account_id === accountId);
    return connu ? { account_id: connu.account_id, email: connu.email } : null;
  }

  export async function ouvrir(nouveauMode, source = null) {
    const mien = ++jeton;
    mode = nouveauMode;
    a = '';
    objet = '';
    corps = '';
    fichiers = [];
    replyToMailbox = null;
    replyToUid = null;
    brouillonId = null;
    brouillonEpoch = null;
    expediteur = source
      ? compteDe(source.account_id)
      : compteDe(compte) ?? (comptes.length > 0 ? compteDe(comptes[0].account_id) : null);
    visible = true;

    if (nouveauMode !== 'new' && source) {
      const commande = nouveauMode === 'reply' ? 'reply_context' : 'forward_context';
      try {
        const contexte = await appel(commande, {
          accountId: source.account_id,
          mailbox: source.mailbox,
          uid: source.uid,
        });
        if (mien !== jeton) return;
        objet = nouveauMode === 'reply' ? sujetRe(source.subject) : sujetTr(source.subject);
        if (nouveauMode === 'reply') {
          a = contexte.to;
          const prenom = (source.sender ?? '').split(' ')[0];
          // La citation du cœur mène par deux sauts (la place du curseur en
          // v1) ; l'amorce du prototype les apporte déjà — sans cette taille,
          // quatre lignes vides sépareraient l'amorce de la citation.
          const citation = contexte.body.replace(/^\n+/, '');
          corps = prenom ? `Bonjour ${prenom},\n\n${citation}` : contexte.body;
          replyToMailbox = source.mailbox;
          replyToUid = source.uid;
        } else {
          corps = contexte.body;
        }
      } catch (err) {
        if (mien !== jeton) return;
        if (nouveauMode === 'forward') {
          // Sans corps, un transfert ne transmettrait rien : échec franc.
          visible = false;
          onflash(`Transfert impossible : ${err}`);
          return;
        }
        // Réponse sans citation : le cœur le permet, on écrit quand même.
        objet = sujetRe(source.subject);
        replyToMailbox = source.mailbox;
        replyToUid = source.uid;
      }
      if (source.attachment_count > 0) {
        try {
          const lues = await appel('message_attachments', {
            accountId: source.account_id,
            mailbox: source.mailbox,
            uid: source.uid,
          });
          if (mien === jeton) fichiers = lues;
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
    mode = 'new';
    expediteur = compteDe(brouillon.account_id);
    a = brouillon.to;
    objet = brouillon.subject;
    corps = brouillon.body;
    fichiers = [];
    replyToMailbox = null;
    replyToUid = brouillon.reply_to_uid ?? null;
    brouillonId = brouillon.id;
    brouillonEpoch = brouillon.updated_epoch;
    visible = true;
    setTimeout(() => champCorps?.focus(), 0);
  }

  const vide = () => !a.trim() && !objet.trim() && !corps.trim();

  function programmerSauvegarde() {
    clearTimeout(minuterie);
    minuterie = setTimeout(sauverMaintenant, 2000);
  }

  // Le filet : un crash ne coûte que les deux dernières secondes de
  // frappe. Rend le bilan, ou null s'il n'y avait rien à faire.
  async function sauverMaintenant() {
    clearTimeout(minuterie);
    if (!visible || vide() || !expediteur) return null;
    try {
      const bilan = await appel('save_draft', {
        accountId: expediteur.account_id,
        id: brouillonId,
        baseEpoch: brouillonEpoch,
        content: { to: a, subject: objet, body: corps, replyToUid },
      });
      if (!visible) {
        // Le panneau s'est fermé pendant la sauvegarde (envoi parti) :
        // ne pas ressusciter un brouillon déjà réglé.
        await appel('delete_draft', { id: bilan.id }).catch(() => {});
        return null;
      }
      brouillonId = bilan.id;
      brouillonEpoch = bilan.updated_epoch;
      if (bilan.forked) {
        // Ne JAMAIS taire ce cas : deux textes existent désormais, seul
        // l'utilisateur peut trancher.
        onflash('Ce brouillon avait changé ailleurs — votre version a été conservée à part.');
      }
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
    if (vide()) {
      if (brouillonId !== null) {
        await appel('delete_draft', { id: brouillonId }).catch(() => {});
      }
      visible = false;
      return;
    }
    const bilan = await sauverMaintenant();
    visible = false;
    if (!(bilan && bilan.forked)) onflash('Brouillon enregistré.');
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
      onflash('Aucun compte émetteur — ajoutez un compte.');
      return;
    }
    envoiEnCours = true;
    try {
      await appel('queue_send', {
        accountId: expediteur.account_id,
        to: a,
        subject: objet.trim(),
        body: corps,
        replyToMailbox,
        replyToUid,
      });
    } catch (err) {
      onflash(`Envoi impossible : ${err}`);
      return;
    } finally {
      envoiEnCours = false;
    }
    // L'envoi est journalisé : le brouillon a rempli son office.
    const regle = brouillonId;
    clearTimeout(minuterie);
    visible = false;
    onflash('Message envoyé.');
    if (regle !== null) {
      await appel('delete_draft', { id: regle }).catch(() => {});
    }
    // Vidange en arrière-plan ; hors ligne, la file attend — l'incident
    // visible est la fente d'avis (P5). Puis purge du reflet distant du
    // brouillon réglé (séquence v1).
    appel('flush_outbox')
      .catch((err) => console.error('flush_outbox :', err))
      .then(() => appel('sync_drafts').catch(() => {}))
      .finally(() => onenvoye());
  }

  function joindre() {
    onflash('Sélecteur de fichiers — à venir.');
  }
</script>

{#if visible}
  <div class="scrim" data-testid="composition">
    <div class="carte" role="dialog" aria-modal="true" aria-label={KICKERS[mode]}>
      <div class="tete">
        <span class="kicker" data-testid="composition-kicker">{KICKERS[mode]}</span>
        <span class="rappel">{objet}</span>
        <span class="puce"><span class="ms" aria-hidden="true">open_in_new</span>Rendre indépendante</span>
        <button type="button" class="fermer" aria-label="Fermer" onclick={fermer}>
          <span class="ms" aria-hidden="true">close</span></button>
      </div>
      <div class="champs">
        <div class="rang">
          <span class="etiquette">De</span>
          {#if comptes.length > 1}
            <!-- A10 : le compte émetteur SE CHOISIT (verdict terrain) —
                 le prototype figeait la ligne, v1 avait le sélecteur. -->
            <select class="valeur" data-testid="composition-de" aria-label="Compte émetteur"
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
          <span class="etiquette">À</span>
          <input type="text" bind:this={champA} bind:value={a} oninput={programmerSauvegarde}
                 placeholder="Destinataire" data-testid="composition-a">
          <span class="puce"><span class="ms" aria-hidden="true">group_add</span>Cc</span>
          <span class="puce"><span class="ms" aria-hidden="true">visibility_off</span>Cci</span>
        </div>
        <div class="rang">
          <span class="etiquette">Objet</span>
          <input type="text" bind:value={objet} oninput={programmerSauvegarde}
                 placeholder="Objet du message" data-testid="composition-objet">
        </div>
      </div>
      <div class="zone-corps">
        <textarea bind:this={champCorps} bind:value={corps} oninput={programmerSauvegarde}
                  placeholder="Votre message…" data-testid="composition-corps"></textarea>
      </div>
      {#if fichiers.length > 0}
        <div class="fichiers">
          {#each fichiers as fichier (fichier.index)}
            <span class="puce"><span class="ms" aria-hidden="true">description</span>{fichier.name}</span>
            <span class="puce"><span class="ms" aria-hidden="true">storage</span>{fichier.size}</span>
          {/each}
        </div>
      {/if}
      <div class="format">
        <span class="bouton-format gras">G</span>
        <span class="bouton-format italique">I</span>
        <span class="bouton-format souligne">S</span>
        <span class="puce"><span class="ms" aria-hidden="true">format_list_bulleted</span>Liste</span>
        <span class="puce"><span class="ms" aria-hidden="true">link</span>Lien</span>
        <span class="puce"><span class="ms" aria-hidden="true">format_quote</span>Citation</span>
      </div>
      <div class="pied">
        <button type="button" class="principal" data-testid="composition-envoyer"
                disabled={envoiEnCours} onclick={envoyer}>
          <span class="ms" aria-hidden="true">send</span>Envoyer</button>
        <button type="button" onclick={joindre} data-testid="composition-joindre">
          <span class="ms" aria-hidden="true">attach_file</span>Joindre</button>
        <button type="button" onclick={enregistrerBrouillon} data-testid="composition-brouillon">
          <span class="ms" aria-hidden="true">drafts</span>Enregistrer le brouillon</button>
        <button type="button" class="annuler" data-testid="composition-annuler"
                onclick={fermer}>Annuler</button>
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
    border:1px solid var(--border); border-left:2px solid var(--accent);
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
  .rappel {
    font-size:13px; color:var(--muted); flex:1; overflow:hidden;
    text-overflow:ellipsis; white-space:nowrap;
  }
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

  .champs { padding:18px 22px 0; display:flex; flex-direction:column; }
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

  .fichiers { padding:0 22px 14px; display:flex; gap:10px; flex-wrap:wrap; }

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
