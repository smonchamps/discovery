<script>
  // Le guichet d'ajout de compte — UNE implémentation, deux surfaces :
  // l'écran 01 (zéro compte) et la section « Comptes » des Réglages
  // (A11). Porte simple (D4) : le domaine choisit le flux existant —
  // Gmail et Microsoft par le consentement navigateur, tout autre
  // domaine révèle les champs IMAP/SMTP du guichet générique.
  //
  // `compact` resserre la géométrie pour vivre dans la surimpression
  // Réglages (entrées 40 px) ; l'écran 01 garde ses 52 px du prototype.
  import { appel } from './lib/transport.js';
  import { t } from './lib/texte.svelte.js';

  let { onajoute = () => {}, compact = false } = $props();

  let adresse = $state('');
  let generique = $state(false);
  let motDePasse = $state('');
  let imapHote = $state('');
  let imapPort = $state('993');
  let smtpHote = $state('');
  let smtpPort = $state('465');
  let occupe = $state(false);
  let attente = $state('');
  let erreur = $state('');

  const domaine = () => (adresse.split('@')[1] ?? '').toLowerCase();
  const estGoogle = () => ['gmail.com', 'googlemail.com'].includes(domaine());
  const estMicrosoft = () =>
    /^(outlook|hotmail|live|msn)\./.test(domaine()) || domaine() === 'outlook.com';

  async function continuer() {
    erreur = '';
    const saisie = adresse.trim();
    if (!saisie.includes('@')) {
      erreur = t('guichet.adresseInvalide');
      return;
    }
    if (estGoogle() || estMicrosoft()) {
      occupe = true;
      attente = t('guichet.autorisation');
      try {
        await (estGoogle()
          ? appel('add_account')
          : appel('add_microsoft_account', { email: saisie }));
        onajoute();
      } catch (err) {
        erreur = t('erreur.connexion', { err });
      } finally {
        occupe = false;
        attente = '';
      }
      return;
    }
    if (!generique) {
      // Domaine inconnu : le guichet générique se révèle, rien ne part.
      generique = true;
      if (!imapHote) imapHote = `imap.${domaine()}`;
      if (!smtpHote) smtpHote = `smtp.${domaine()}`;
      return;
    }
    occupe = true;
    attente = t('guichet.verification');
    try {
      await appel('add_generic_account', {
        input: {
          email: saisie,
          username: null,
          password: motDePasse,
          imapHost: imapHote.trim(),
          imapPort: Number(imapPort) || 993,
          smtpHost: smtpHote.trim(),
          smtpPort: Number(smtpPort) || 465,
        },
      });
      onajoute();
    } catch (err) {
      erreur = t('erreur.connexion', { err });
    } finally {
      occupe = false;
      attente = '';
    }
  }
</script>

<div class="guichet" class:compact>
  <div class="formulaire">
    <label for="ob-adresse">{t('guichet.adresse')}</label>
    <input id="ob-adresse" type="email" bind:value={adresse}
           data-testid="onboarding-adresse"
           onkeydown={(e) => e.key === 'Enter' && !occupe && continuer()}>
    {#if generique}
      <label for="ob-mdp">{t('guichet.mdp')}</label>
      <input id="ob-mdp" type="password" bind:value={motDePasse}>
      <div class="serveurs">
        <span>
          <label for="ob-imap">{t('guichet.imap')}</label>
          <input id="ob-imap" type="text" bind:value={imapHote}>
        </span>
        <span class="port">
          <label for="ob-imap-port">{t('guichet.port')}</label>
          <input id="ob-imap-port" type="text" bind:value={imapPort}>
        </span>
      </div>
      <div class="serveurs">
        <span>
          <label for="ob-smtp">{t('guichet.smtp')}</label>
          <input id="ob-smtp" type="text" bind:value={smtpHote}>
        </span>
        <span class="port">
          <label for="ob-smtp-port">{t('guichet.port')}</label>
          <input id="ob-smtp-port" type="text" bind:value={smtpPort}>
        </span>
      </div>
    {/if}
    <button type="button" class="principal" data-testid="onboarding-continuer"
            disabled={occupe} onclick={continuer}>{t('action.continuer')}</button>
  </div>
  {#if erreur}
    <p class="erreur" data-testid="onboarding-erreur">{erreur}</p>
  {:else if attente}
    <p class="note">{attente}</p>
  {:else if generique}
    <p class="note">{t('guichet.noteGenerique')}</p>
  {:else}
    <p class="note">{t('guichet.noteAuto')}</p>
  {/if}
</div>

<style>
  /* Jetons de l'écran 01 du prototype ; `compact` pour les Réglages. */
  .guichet { display:flex; flex-direction:column; gap:14px; }
  .formulaire { display:flex; flex-direction:column; gap:12px; }
  label { font-size:13px; color:var(--ink2); }
  input {
    height:52px; font-size:15px; padding:0 16px; background:var(--surface);
    color:var(--ink); border:1px solid var(--border); border-radius:6px;
    box-shadow:var(--shadow); outline:none; width:100%;
  }
  .compact input { height:40px; font-size:13px; box-shadow:none; }
  .serveurs { display:flex; gap:12px; }
  .serveurs span { display:flex; flex-direction:column; gap:12px; flex:1; }
  .serveurs .port { flex:0 0 110px; }
  .principal {
    height:32px; padding:0 16px; align-self:flex-start; font-size:13px;
    font-weight:600; color:var(--onAccent); background:var(--accent);
    border:1px solid var(--accent); border-radius:6px; cursor:pointer;
  }
  .principal:hover { background:var(--accentH); border-color:var(--accentH); }
  .principal:disabled { opacity:.6; cursor:default; }
  .note { margin:0; font-size:13px; line-height:1.5; color:var(--muted); }
  .erreur { margin:0; font-size:13px; line-height:1.5; color:var(--alert); }
</style>
