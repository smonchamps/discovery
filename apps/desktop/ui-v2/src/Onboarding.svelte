<script>
  // Écran 01 du prototype, affiché à zéro compte : « Votre adresse.
  // C'est tout. » — une adresse, « Continuer ».
  //
  // Porte simple (D4) : « Continuer » BRANCHE les flux d'ajout existants,
  // il n'invente rien. Le domaine choisit le guichet — Gmail et
  // Microsoft passent par le consentement navigateur (le serveur est
  // effectivement connu d'avance, la promesse du prototype est tenue) ;
  // tout autre domaine révèle les champs IMAP/SMTP du flux générique,
  // dans la même colonne, en style Clarity. L'auto-détection de serveur
  // (SRV/autoconfig) est une capacité neuve, à instruire séparément.
  import { appel } from './lib/transport.js';

  let { onajoute = () => {} } = $props();

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
      erreur = 'Saisissez votre adresse e-mail complète.';
      return;
    }
    if (estGoogle() || estMicrosoft()) {
      occupe = true;
      attente = 'Autorisation en cours dans votre navigateur…';
      try {
        await (estGoogle()
          ? appel('add_account')
          : appel('add_microsoft_account', { email: saisie }));
        onajoute();
      } catch (err) {
        erreur = `Connexion impossible : ${err}`;
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
    attente = 'Vérification de la connexion au serveur…';
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
      erreur = `Connexion impossible : ${err}`;
    } finally {
      occupe = false;
      attente = '';
    }
  }
</script>

<div class="ecran01" data-testid="onboarding">
  <div class="colonne">
    <p class="kicker">Discovery</p>
    <h3 class="titre">Votre adresse.<br>C'est tout.</h3>
    <div class="formulaire">
      <label for="ob-adresse">Adresse e-mail</label>
      <input id="ob-adresse" type="email" bind:value={adresse}
             data-testid="onboarding-adresse"
             onkeydown={(e) => e.key === 'Enter' && !occupe && continuer()}>
      {#if generique}
        <label for="ob-mdp">Mot de passe</label>
        <input id="ob-mdp" type="password" bind:value={motDePasse}>
        <div class="serveurs">
          <span>
            <label for="ob-imap">Serveur IMAP</label>
            <input id="ob-imap" type="text" bind:value={imapHote}>
          </span>
          <span class="port">
            <label for="ob-imap-port">Port</label>
            <input id="ob-imap-port" type="text" bind:value={imapPort}>
          </span>
        </div>
        <div class="serveurs">
          <span>
            <label for="ob-smtp">Serveur SMTP</label>
            <input id="ob-smtp" type="text" bind:value={smtpHote}>
          </span>
          <span class="port">
            <label for="ob-smtp-port">Port</label>
            <input id="ob-smtp-port" type="text" bind:value={smtpPort}>
          </span>
        </div>
      {/if}
      <button type="button" class="principal" data-testid="onboarding-continuer"
              disabled={occupe} onclick={continuer}>Continuer</button>
    </div>
    {#if erreur}
      <p class="erreur" data-testid="onboarding-erreur">{erreur}</p>
    {:else if attente}
      <p class="note">{attente}</p>
    {:else if generique}
      <p class="note">Renseignez les serveurs de votre fournisseur — le mot de passe rejoint le coffre du système, jamais un fichier.</p>
    {:else}
      <p class="note">Le serveur est détecté automatiquement. Rien d'autre à régler.</p>
    {/if}
  </div>
</div>

<style>
  /* Géométrie VERBATIM de l'écran 01 du prototype ; le bloc générique
     (domaine inconnu) prolonge la colonne dans les mêmes jetons. */
  .ecran01 {
    position:absolute; inset:0; display:flex; align-items:center;
    justify-content:center; background:var(--bg); z-index:1;
  }
  .colonne { width:520px; display:flex; flex-direction:column; gap:26px; }
  .kicker {
    margin:0; font-size:12px; letter-spacing:.14em; text-transform:uppercase;
    color:var(--muted); font-weight:600;
  }
  .titre {
    margin:0; font-size:40px; line-height:1.1; font-weight:600;
    letter-spacing:-.02em; color:var(--ink);
  }
  .formulaire { display:flex; flex-direction:column; gap:12px; }
  label { font-size:13px; color:var(--ink2); }
  input {
    height:52px; font-size:15px; padding:0 16px; background:var(--surface);
    color:var(--ink); border:1px solid var(--border);
    border-left:2px solid var(--accent); border-radius:6px;
    box-shadow:var(--shadow); outline:none; width:100%;
  }
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
