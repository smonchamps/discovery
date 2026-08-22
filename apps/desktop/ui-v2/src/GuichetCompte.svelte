<script>
  // Le guichet d'ajout de compte — UNE implémentation, deux surfaces :
  // l'écran 01 (zéro compte) et la section « Comptes » des Réglages
  // (A11). Porte simple (D4) : le domaine choisit le flux existant —
  // Gmail et Microsoft par le consentement navigateur, tout autre
  // domaine révèle les champs IMAP/SMTP du guichet générique.
  //
  // `compact` resserre la géométrie pour vivre dans la surimpression
  // Réglages (entrées 40 px) ; l'écran 01 garde ses 52 px du prototype.
  // `accueil` (terrain 2026-08-22, constats 1-3 des deux passes) : la
  // présentation de l'écran 01 refondu — la barre d'adresse (40 px, la
  // hauteur de son bouton) porte « Ajouter » à sa droite, et le
  // guichet générique révélé gagne un « Retour » qui replie les champs
  // serveur. `ajoutPrincipal` : tant que le Continuer de la marche est
  // grisé (aucun compte), « Ajouter » est LE geste — primaire ; dès
  // qu'un compte existe, il redevient secondaire. La note « serveur
  // détecté » ne se montre pas en accueil (2e passe, constat 2).
  import { appel } from './lib/transport.js';
  import { t } from './lib/texte.svelte.js';

  let {
    onajoute = () => {},
    compact = false,
    accueil = false,
    ajoutPrincipal = false,
    // 3e passe terrain (constat 2) : l'accueil masque son « Continuer »
    // quand le guichet générique est révélé — il a besoin de le savoir.
    ongenerique = () => {},
  } = $props();

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
      ongenerique(true);
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

<div class="guichet" class:compact class:accueil>
  <div class="formulaire">
    <label for="ob-adresse">{t('guichet.adresse')}</label>
    <div class="barre">
      <input id="ob-adresse" type="email" bind:value={adresse}
             data-testid="onboarding-adresse"
             onkeydown={(e) => e.key === 'Enter' && !occupe && continuer()}>
      {#if accueil && !generique}
        <button type="button" class={ajoutPrincipal ? 'primaire' : 'secondaire'}
                data-testid="onboarding-continuer"
                disabled={occupe} onclick={continuer}>{t('accueil.ajouter')}</button>
      {/if}
    </div>
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
    {#if !accueil}
      <button type="button" class="principal" data-testid="onboarding-continuer"
              disabled={occupe} onclick={continuer}>{t('action.continuer')}</button>
    {:else if generique}
      <!-- Constat 3 : sur le guichet générique révélé, l'action est
           « Ajouter » (secondaire) et « Retour » replie les champs. -->
      <!-- 3e passe (constat 2) : sur le guichet générique révélé,
           « Ajouter » est LE geste — toujours primaire (le Continuer
           de la marche est masqué pendant ce temps). -->
      <div class="actions">
        <button type="button" class="primaire"
                data-testid="onboarding-continuer"
                disabled={occupe} onclick={continuer}>{t('accueil.ajouter')}</button>
        <button type="button" class="secondaire" data-testid="guichet-retour"
                disabled={occupe}
                onclick={() => { generique = false; erreur = ''; ongenerique(false); }}>{t('accueil.retour')}</button>
      </div>
    {/if}
  </div>
  {#if erreur}
    <p class="erreur" data-testid="onboarding-erreur">{erreur}</p>
  {:else if attente}
    <p class="note">{attente}</p>
  {:else if generique}
    <p class="note">{t('guichet.noteGenerique')}</p>
  {:else if !accueil}
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
  /* La barre : l'entrée + son bouton, sur une même ligne en `accueil` —
     à la MÊME hauteur (2e passe terrain, constat 2). */
  .barre { display:flex; gap:12px; }
  .barre input { flex:1; min-width:0; }
  .accueil .barre input { height:40px; font-size:14px; }
  .secondaire, .primaire {
    height:40px; padding:0 18px; flex:none; align-self:center;
    font-size:13px; font-weight:600; border-radius:6px; cursor:pointer;
  }
  .secondaire {
    color:var(--ink); background:var(--surface);
    border:1px solid var(--border);
  }
  .secondaire:hover { background:var(--sel); }
  /* Constat 1 (2e passe) : tant que Continuer est grisé, « Ajouter »
     est LE geste — il prend le dessin principal. */
  .primaire {
    color:var(--onAccent); background:var(--accent);
    border:1px solid var(--accent);
  }
  .primaire:hover { background:var(--accentH); border-color:var(--accentH); }
  .secondaire:disabled, .primaire:disabled { opacity:.6; cursor:default; }
  .actions { display:flex; gap:12px; }
  .actions .secondaire, .actions .primaire { align-self:flex-start; }
  .note { margin:0; font-size:13px; line-height:1.5; color:var(--muted); }
  .erreur { margin:0; font-size:13px; line-height:1.5; color:var(--alert); }
</style>
