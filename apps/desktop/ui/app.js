// Règle absolue : les données du mail entrent dans le DOM par textContent
// (ou par l'attribut srcdoc d'une iframe sandbox), jamais par innerHTML.
//
// Liste virtualisée : seules les lignes visibles existent dans le DOM ;
// les pages d'enveloppes arrivent du noyau au fil du défilement.
// Actions de triage : optimistes localement, rejouées au prochain sync.
// Envoi : journalisé dans la boîte d'envoi AVANT tout réseau, vidangé
// ensuite — jamais d'envoi perdu, jamais d'envoi fantôme.
const invoke = window.__TAURI__.core.invoke;
const el = (id) => document.getElementById(id);

const ROW_HEIGHT = 56;
const PAGE_SIZE = 200;
const OVERSCAN = 8;

let total = 0;
let pages = new Map();      // index de page -> lignes
let pending = new Set();    // pages en cours de chargement
let currentMessage = null;
let currentIndex = null;
let composeReplyUid = null; // UID du message auquel on répond, sinon null
// La BOITE de ce message. Sans elle, l UID ne le designe plus depuis
// qu un compte en synchronise plusieurs : mieux vaut ne pas citer que
// citer un inconnu (ADR 0008 §2).
let composeReplyMailbox = null;
let composeAccountId = null; // compte émetteur de la composition en cours
let composeDraftId = null;  // id du brouillon en cours d'édition, sinon null
// Horodatage de la version que le composeur a LUE. Renvoyé a chaque
// sauvegarde : c'est ce qui permet au noyau de detecter qu'un autre a
// ecrit entre-temps — le tirage, typiquement — et de conserver les deux
// textes au lieu d'en ecraser un.
let composeDraftEpoch = null;
let draftSaveTimer = null;  // autosauvegarde debouncée pendant la frappe
let connectedAccounts = []; // comptes connectés {id, email} — l'ordre du registre
let searchMode = false;     // la recherche remplace-t-elle la boîte unifiée ?
let searchResults = [];     // résultats de la recherche en cours
let searchTimer = null;     // debounce de la saisie

/// Active le mode recherche : le champ apparaît et la liste unifiée
/// cède la place aux résultats.
function showSearch() {
  if (searchMode) return;
  searchMode = true;
  el('scroll-space').hidden = true;
  el('empty').hidden = true;
  el('search-results').hidden = false;
  el('search').hidden = false;
  el('search').focus();
}

/// Quitte le mode recherche et revient à la boîte unifiée.
function hideSearch() {
  if (!searchMode) return;
  searchMode = false;
  searchResults = [];
  clearTimeout(searchTimer);
  el('search').value = '';
  el('search').hidden = true;
  el('search-results').hidden = true;
  el('search-results').replaceChildren();
  el('scroll-space').hidden = false;
  el('empty').hidden = total > 0;
  renderVisible();
}

async function runSearch() {
  const query = el('search').value.trim();
  if (query.length < 3) {
    searchResults = [];
    renderSearchResults();
    return;
  }
  try {
    searchResults = await invoke('search_messages', { query });
    renderSearchResults();
  } catch (err) {
    setStatus(`recherche impossible : ${err}`, true);
  }
}

function renderSearchResults() {
  const container = el('search-results');
  container.replaceChildren();
  if (searchResults.length === 0) {
    const p = document.createElement('p');
    p.className = 'empty-search';
    p.textContent = 'Aucun résultat.';
    container.appendChild(p);
    return;
  }
  for (const message of searchResults) {
    container.appendChild(buildResultRow(message));
  }
}

function buildResultRow(message) {
  const row = document.createElement('div');
  row.className = 'row search-result';
  row.dataset.testid = 'search-result';
  if (isUnread(message)) row.classList.add('unread');
  if (message.flagged) row.classList.add('flagged');
  if (currentMessage
    && message.uid === currentMessage.uid
    && message.account_id === currentMessage.account_id) {
    row.classList.add('selected');
  }
  for (const [cls, text] of [
    ['date', message.date],
    ['sender', message.sender],
    ['subject', message.subject],
  ]) {
    const span = document.createElement('span');
    span.className = cls;
    span.textContent = text;
    row.appendChild(span);
  }
  appendRowMarks(row, message);
  row.addEventListener('click', () => openMessage(message, null));
  return row;
}

// Une conversation est non lue tant qu'il lui reste UN message non lu,
// meme si celui qu'on affiche est deja lu. Lire l'etat du seul message
// visible donnerait la reponse inverse sur un fil deja parcouru.
// Les resultats de recherche portent 1/0 selon leur propre etat : la
// meme fonction sert donc partout.
function isUnread(message) {
  return message.thread_unseen > 0;
}

// La ligne reste selectionnee tant qu'on lit DANS la conversation, meme
// en passant d'un message a l'autre : c'est le fil qu'on a ouvert.
function isSelectedRow(message) {
  if (!currentMessage) return false;
  if (currentMessage.thread_id && message.thread_id) {
    return currentMessage.thread_id === message.thread_id;
  }
  return message.uid === currentMessage.uid
    && message.account_id === currentMessage.account_id;
}

/// Marqueurs de bout de ligne : compteur du fil, trombone, puis pastille
/// de compte, dans UN conteneur commun. Les poser separement les ferait
/// se chevaucher des qu'un message les cumule.
function appendRowMarks(row, message) {
  const marks = document.createElement('span');
  marks.className = 'row-marks';
  if (message.thread_size > 1) {
    const count = document.createElement('span');
    count.className = 'thread-count';
    count.textContent = message.thread_size;
    count.title = `${message.thread_size} messages dans cette conversation`;
    marks.appendChild(count);
  }
  if (message.has_attachment) {
    const clip = document.createElement('span');
    clip.className = 'clip';
    clip.textContent = '📎';
    clip.title = 'Contient une pièce jointe';
    marks.appendChild(clip);
  }
  if (connectedAccounts.length > 1) {
    const dot = document.createElement('span');
    dot.className = 'dot account-dot';
    dot.style.background = accountColor(message.account_id);
    dot.title = message.account_email;
    marks.appendChild(dot);
  }
  if (marks.childElementCount > 0) row.appendChild(marks);
}

// --- Migration visible et interruptible (Phase 5) --------------------
//
// Chaque commande ouvre sa propre connexion : sans cet écran, la
// PREMIÈRE commande venue paierait l'adoption d'une base héritée en
// silence, dans un gel d'interface. On sonde donc AVANT de toucher la
// base ; s'il y a du travail, l'écran bloque tout le reste. « Annuler »
// défait tout — la passe entière se rejouera au prochain lancement, ou
// tout de suite par « Reprendre ».

/// Ne rend la main qu'une fois la base migrée : tant que l'utilisateur
/// annule, l'écran reste — montrer la boîte exigerait la passe.
async function ensureMigrated() {
  let check;
  try {
    check = await invoke('migration_check');
  } catch {
    // Sonde impossible : l'ouverture normale le dira mieux qu'un écran
    // sans objet — on ne bloque pas le démarrage.
    return;
  }
  if (check.pending === null || check.pending === undefined) return;
  el('migration-dialog').hidden = false;
  el('migration-note').textContent =
    `Environ ${check.pending} messages vont être réorganisés en conversations. ` +
    'Cette mise à jour ne se fait qu’une fois et n’efface rien.';
  while (!(await runMigration())) {
    await new Promise((resolve) => {
      el('migration-retry').addEventListener('click', resolve, { once: true });
    });
  }
  el('migration-dialog').hidden = true;
}

function migrationTick(progress) {
  const gauge = el('migration-gauge');
  if (progress.percent === null || progress.percent === undefined) {
    // Pas encore de dénominateur : une barre indéterminée, jamais un
    // « 0 % » qui ferait croire à une panne.
    gauge.removeAttribute('value');
    el('migration-percent').textContent = 'Préparation…';
    return;
  }
  gauge.value = progress.percent;
  el('migration-percent').textContent = `${progress.percent} %`;
}

/// Une passe : vrai si la base est migrée, faux si annulée ou en échec —
/// l'écran propose alors « Reprendre ».
async function runMigration() {
  el('migration-cancel').hidden = false;
  el('migration-cancel').disabled = false;
  el('migration-retry').hidden = true;
  el('migration-percent').textContent = '';
  el('migration-gauge').removeAttribute('value');
  const poll = setInterval(async () => {
    try {
      migrationTick(await invoke('migration_progress'));
    } catch {
      // Le prochain relevé suffira.
    }
  }, 300);
  let outcome;
  try {
    const migrated = await invoke('migration_run');
    outcome = migrated
      ? true
      : 'Mise à jour annulée — tout est revenu comme avant. Elle reprendra au prochain lancement, ou tout de suite :';
  } catch (err) {
    outcome = `La mise à jour a échoué (${err}). Rien n’est perdu : elle peut être relancée.`;
  } finally {
    clearInterval(poll);
  }
  if (outcome === true) return true;
  el('migration-percent').textContent = outcome;
  el('migration-cancel').hidden = true;
  el('migration-retry').hidden = false;
  return false;
}

el('migration-cancel').addEventListener('click', () => {
  // Un seul clic suffit : la passe annule à son prochain palier.
  el('migration-cancel').disabled = true;
  invoke('migration_cancel').catch(() => {});
});

// --- Mise à jour signée (ADR 0013) -----------------------------------
//
// Vérification UNE fois au démarrage, en silence : un contrôle que
// l'utilisateur doit réclamer n'aurait pas lieu (leçon de l'ADR 0007).
// Hors ligne, l'endpoint est injoignable — ce n'est pas un défaut, le
// bandeau reste simplement absent. L'installation télécharge, vérifie la
// signature minisign, remplace le binaire et redémarre.

async function checkForUpdate() {
  let update;
  try {
    update = await invoke('update_check');
  } catch {
    // Hors ligne, ou endpoint injoignable : pas de bandeau, pas de bruit.
    return;
  }
  if (!update) return;
  el('update-summary').textContent = `Une mise à jour est disponible (version ${update.version}).`;
  el('update-bar').hidden = false;
}

el('update-install').addEventListener('click', async () => {
  el('update-install').disabled = true;
  el('update-summary').textContent = 'Téléchargement et installation…';
  try {
    // L'application redémarre sur la version neuve : cet appel ne rend
    // pas la main en cas de succès.
    await invoke('update_install');
  } catch (err) {
    el('update-install').disabled = false;
    el('update-summary').textContent = `Mise à jour impossible : ${err}`;
  }
});

el('update-later').addEventListener('click', () => {
  // « Plus tard » : le bandeau se referme, le contrôle reviendra au
  // prochain démarrage.
  el('update-bar').hidden = true;
});

// --- Télémétrie de crash (ADR 0014) ----------------------------------
//
// Opt-in explicite, off par défaut. On demande UNE fois (état « unset »),
// puis on n'y revient plus. Les rapports restent locaux : le bandeau
// d'incident propose seulement d'ouvrir le dossier, l'utilisateur envoie
// lui-même. Aucun réseau.

async function checkTelemetry() {
  try {
    const consent = await invoke('telemetry_consent_get');
    if (consent === 'unset') {
      el('telemetry-optin-bar').hidden = false;
    }
    const pending = await invoke('telemetry_pending');
    if (pending > 0) {
      el('crash-report-text').textContent =
        `Discovery a rencontré un problème lors d'une session précédente (${pending} rapport(s) en attente). Rien n'est envoyé sans vous.`;
      el('crash-report-bar').hidden = false;
    }
  } catch {
    // Pas de télémétrie disponible : pas de bandeau, pas de bruit.
  }
}

async function setTelemetryConsent(enabled) {
  el('telemetry-optin-bar').hidden = true;
  try {
    await invoke('telemetry_consent_set', { enabled });
  } catch (err) {
    setStatus(`préférence de télémétrie non enregistrée : ${err}`, true);
  }
}

el('telemetry-enable').addEventListener('click', () => setTelemetryConsent(true));
el('telemetry-decline').addEventListener('click', () => setTelemetryConsent(false));
el('crash-report-open').addEventListener('click', async () => {
  try {
    await invoke('telemetry_open_folder');
  } catch (err) {
    setStatus(`ouverture du dossier impossible : ${err}`, true);
  }
});

async function init() {
  invoke('startup_report').then((report) => {
    el('perf').textContent = report;
    // Conservé après écrasement par la liste : lu par l'outil de mesure
    // des revues de phase (e2e/mesure.mjs).
    el('perf').dataset.startup = report;
  });
  el('pane-list').addEventListener('scroll', onScroll);
  // La migration d'abord : rien d'autre ne touche la base tant qu'une
  // base héritée n'est pas adoptée.
  await ensureMigrated();
  // Le contrôle de mise à jour ne bloque pas le démarrage : la boîte
  // s'affiche, le bandeau viendra si besoin.
  checkForUpdate();
  // Consentement télémétrie (une fois) et rapports de plantage en attente.
  checkTelemetry();
  refreshDrafts(); // les brouillons sont locaux : visibles même sans compte
  let problems = [];
  try {
    const report = await invoke('connect_accounts');
    connectedAccounts = report.accounts;
    problems = report.problems;
  } catch (err) {
    connectedAccounts = [];
    problems = [`${err}`];
  }
  renderAccounts();
  el('connect').hidden = false; // ajouter un compte est toujours possible
  if (connectedAccounts.length > 0) {
    await onConnected();
    // Reconnexion partielle : dire LEQUEL manque et pourquoi, sinon
    // l'utilisateur constate une pastille absente sans savoir quoi faire.
    if (problems.length > 0) {
      setStatus(`compte non reconnecté — ${problems.join(' ; ')}`, true);
    }
  } else if (problems.length > 0) {
    setStatus(`aucun compte reconnecté — ${problems.join(' ; ')}`, true);
    await reloadList();
    await refreshOutbox();
  } else {
    setStatus('Ajoutez un compte Gmail pour commencer.');
    await reloadList();
    await refreshOutbox();
  }
}

/// Couleur stable d'un compte, dérivée de son id — la même d'une
/// session à l'autre, en liste comme dans les puces d'en-tête.
function accountColor(id) {
  return `hsl(${(id * 137) % 360} 60% 45%)`;
}

/// Puces des comptes connectés + options du sélecteur « De ».
function renderAccounts() {
  const container = el('accounts');
  container.replaceChildren();
  for (const account of connectedAccounts) {
    const chip = document.createElement('span');
    chip.className = 'account-chip';
    const dot = document.createElement('span');
    dot.className = 'dot';
    dot.style.background = accountColor(account.id);
    const label = document.createElement('span');
    label.textContent = account.email;
    chip.append(dot, label);
    container.appendChild(chip);
  }
  const from = el('compose-from');
  from.replaceChildren();
  for (const account of connectedAccounts) {
    const option = document.createElement('option');
    option.value = String(account.id);
    option.textContent = account.email;
    from.appendChild(option);
  }
  el('compose-from-row').hidden = connectedAccounts.length < 2;
}

async function onConnected() {
  el('refresh').hidden = false;
  el('compose-btn').hidden = false;
  await reloadList();
  await refresh();
  // La liste est utilisable AVANT que le rattrapage ne commence :
  // « enveloppes d'abord » reste la règle, le fond vient après.
  await refreshBackfill({ autoStart: true });
}

// --- Rattrapage des corps (ADR 0007) --------------------------------
//
// Sans corps téléchargé, la recherche ne porte que sur les sujets et les
// pièces jointes restent invisibles : le terrain a montré 18 corps sur
// 537. Le rattrapage les rapatrie par lots bornés — c'est ce qui rend
// l'arrêt gratuit : on cesse simplement de rappeler la commande.
//
// Il DÉMARRE SEUL, après la première synchro. L'ADR 0007 exige que ce
// téléchargement soit visible et interruptible, pas qu'il soit déclenché
// à la main : un travail de fond que l'utilisateur doit réclamer est un
// travail qui n'aura pas lieu. La visibilité reste entière — bandeau,
// avancement, bouton d'arrêt.
let backfillRunning = false;
// Arrêt DEMANDÉ : tant qu'il tient, plus de reprise automatique. Ne
// survit pas à la session — le travail, lui, doit finir un jour.
let backfillStopped = false;

async function refreshBackfill({ autoStart = false } = {}) {
  if (backfillRunning) return;
  try {
    const status = await invoke('backfill_status');
    showBackfill(status.remaining);
    if (autoStart && status.remaining > 0 && !backfillStopped) {
      runBackfill();
    }
  } catch {
    el('backfill-bar').hidden = true;
  }
}

function showBackfill(remaining, fetched = null) {
  const bar = el('backfill-bar');
  if (remaining === 0 && !backfillRunning) {
    bar.hidden = true;
    return;
  }
  bar.hidden = false;
  el('backfill-summary').textContent = backfillRunning
    ? `Lecture du contenu en cours — ${fetched ?? 0} lu(s), ${remaining} restant(s)`
    : `${remaining} message(s) dont le contenu n'a pas encore été lu (recherche et pièces jointes)`;
  el('backfill-start').hidden = backfillRunning;
  el('backfill-stop').hidden = !backfillRunning;
}

async function runBackfill() {
  if (backfillRunning) return;
  backfillRunning = true;
  let fetched = 0;
  showBackfill(0, 0);
  try {
    // Un lot par appel : la boucle s'arrête dès que l'utilisateur le
    // demande, sans jeton d'annulation à propager jusqu'au réseau.
    while (backfillRunning) {
      const report = await invoke('backfill_bodies');
      fetched += report.fetched;
      showBackfill(report.remaining, fetched);
      if (report.errors.length > 0) {
        setStatus(`rattrapage — ${report.errors.join(' ; ')}`, true);
      }
      // Plus rien à faire, ou plus rien qui avance : on s'arrête.
      if (report.remaining === 0 || report.fetched === 0) break;
    }
    setStatus(`rattrapage : ${fetched} message(s) lus`);
  } catch (err) {
    setStatus(`rattrapage impossible : ${err}`, true);
  } finally {
    backfillRunning = false;
    await refreshBackfill();
  }
}

// Le bouton ne sert plus qu'à REPRENDRE après un arrêt volontaire.
el('backfill-start').addEventListener('click', () => {
  backfillStopped = false;
  runBackfill();
});
el('backfill-stop').addEventListener('click', () => {
  backfillRunning = false; // le lot en cours se termine, puis la boucle sort
  backfillStopped = true; // et plus de reprise automatique de la session
  setStatus('rattrapage interrompu — il reprendra où il s\'est arrêté');
});

// --- Avancement de la synchronisation intégrale (ADR 0010 §5) --------
//
// Depuis qu'on rapatrie TOUTES les boîtes, la première synchronisation
// d'un compte peut durer très longtemps. Un travail long qui ne dit pas
// où il en est est indistinguable d'un travail bloqué — et l'utilisateur
// tue l'application.
//
// Le sondage est purement LOCAL (`sync_progress` ne touche pas au
// réseau) : il ne coûte donc rien à la synchronisation qu'il observe. Il
// la regarde avancer, il ne la ralentit pas.
let syncPollTimer = null;

async function showSyncProgress() {
  try {
    const progress = await invoke('sync_progress');
    const bar = el('sync-progress-bar');
    // `percent` nul = aucune boîte encore sélectionnée. On se TAIT :
    // « 0 % » ferait croire à une synchro en panne (voir `sync_percent`).
    if (progress.percent === null || progress.percent === undefined) {
      bar.hidden = true;
      return;
    }
    // Fini, et plus personne ne sonde : le bandeau s'efface. Le laisser
    // plein en permanence en ferait un meuble qu'on ne regarde plus — et
    // c'est justement quand il réapparaît qu'il doit se voir.
    if (progress.percent === 100 && syncPollTimer === null) {
      bar.hidden = true;
      return;
    }
    bar.hidden = false;
    el('sync-progress-gauge').value = progress.percent;
    el('sync-progress-summary').textContent =
      `Récupération des messages — ${progress.percent} % `
      + `(${progress.local} sur ${progress.remote})`;
  } catch {
    // On NE CACHE PAS. La base n'est pas en WAL : pendant qu'une
    // transaction d'écriture tient le verrou, cette lecture attend, et
    // peut expirer sur le `busy_timeout`. C'est-à-dire précisément quand
    // la synchronisation travaille le plus fort — le moment où le
    // bandeau sert. Le faire disparaître puis revenir donnerait un
    // clignotement que l'utilisateur lirait comme une panne.
    //
    // On garde donc la dernière valeur connue : périmée de 800 ms au
    // pire, ce qui est sans conséquence, et honnête.
  }
}

function startSyncProgress() {
  stopSyncProgress();
  showSyncProgress();
  syncPollTimer = setInterval(showSyncProgress, 800);
}

function stopSyncProgress() {
  if (syncPollTimer !== null) {
    clearInterval(syncPollTimer);
    syncPollTimer = null;
  }
}

async function refresh() {
  setStatus('synchronisation…');
  startSyncProgress();
  try {
    const report = await invoke('sync_inbox');
    const actions = report.replayed > 0 ? `, ${report.replayed} action(s) envoyée(s)` : '';
    const failures = report.errors.length > 0 ? ` — échecs : ${report.errors.join(' ; ')}` : '';
    setStatus(`synchro de ${report.accounts} compte(s) : ${report.fetched} récupéré(s), `
      + `${report.deleted} supprimé(s)${actions} — ${report.total} messages, `
      + `en ${report.elapsed_ms} ms${failures}`, report.errors.length > 0 && report.accounts === 0);
  } catch (err) {
    setStatus(`erreur de synchronisation : ${err}`, true);
  }
  // Dans les DEUX chemins, succès comme échec : un sondage laissé en
  // vie sur une synchro morte afficherait un avancement figé pour
  // toujours, ce qui est pire que pas d'avancement du tout.
  stopSyncProgress();
  await showSyncProgress();
  await reloadList();
  // La synchro TIRE les brouillons commencés ailleurs : sans ce
  // rafraîchissement, ils resteraient invisibles jusqu'au prochain
  // évènement, et la liste afficherait encore ceux qu'elle vient de
  // remplacer.
  await refreshDrafts();
  // Le réseau est peut-être revenu : la boîte d'envoi retente sa chance,
  // et les brouillons se reflètent dans Gmail.
  await flushOutbox();
  pushDrafts();
}

/// Poussée des brouillons vers Gmail — silencieuse : hors ligne, le
/// cycle suivant retentera, rien à dire ; on ne parle qu'en cas de succès.
///
/// `discret` fait taire jusqu'à ce succès : l'appelant vient d'afficher
/// quelque chose que l'utilisateur doit lire, et ce bilan-ci — qui arrive
/// du réseau bien après — le recouvrirait.
function pushDrafts({ discret = false } = {}) {
  invoke('sync_drafts')
    .then((summary) => {
      if (discret) return;
      if (summary.pushed > 0 || summary.purged > 0) {
        setStatus(`brouillons Gmail : ${summary.pushed} poussé(s), ${summary.purged} purgé(s)`);
      }
    })
    .catch(() => {});
}

async function reloadList() {
  pages.clear();
  pending.clear();
  try {
    const first = await fetchPage(0);
    total = first.total;
    // « conversations », pas « messages » : la liste en groupe plusieurs
    // par ligne, et annoncer un nombre de messages qu'on n'affiche pas
    // serait faux — c'est aussi ce nombre qui dimensionne le defilement.
    const unit = total > 1 ? 'conversations' : 'conversation';
    el('perf').textContent =
      `${total} ${unit} — page servie en ${(first.elapsed_us / 1000).toFixed(2)} ms`;
  } catch {
    total = 0;
  }
  el('scroll-space').style.height = `${total * ROW_HEIGHT}px`;
  el('empty').hidden = total > 0;
  renderVisible();
}

async function fetchPage(index) {
  const page = await invoke('list_messages', {
    offset: index * PAGE_SIZE,
    limit: PAGE_SIZE,
  });
  pages.set(index, page.rows);
  return page;
}

function ensurePage(index) {
  if (index < 0 || index * PAGE_SIZE >= total) return;
  if (pages.has(index) || pending.has(index)) return;
  pending.add(index);
  fetchPage(index)
    .then(() => { pending.delete(index); renderVisible(); })
    .catch(() => pending.delete(index));
}

function rowAt(i) {
  const page = pages.get(Math.floor(i / PAGE_SIZE));
  return page ? page[i % PAGE_SIZE] : null;
}

let framePending = false;
function onScroll() {
  if (framePending) return;
  framePending = true;
  requestAnimationFrame(() => {
    framePending = false;
    renderVisible();
  });
}

function renderVisible() {
  const pane = el('pane-list');
  const first = Math.max(0, Math.floor(pane.scrollTop / ROW_HEIGHT) - OVERSCAN);
  const last = Math.min(
    total,
    Math.ceil((pane.scrollTop + pane.clientHeight) / ROW_HEIGHT) + OVERSCAN,
  );
  ensurePage(Math.floor(first / PAGE_SIZE));
  ensurePage(Math.max(0, Math.floor((last - 1) / PAGE_SIZE)));

  const container = el('rows');
  container.replaceChildren();
  for (let i = first; i < last; i++) {
    container.appendChild(buildRow(i));
  }
}

function buildRow(index) {
  const row = document.createElement('div');
  row.className = 'row';
  row.dataset.testid = 'message-row';
  row.style.top = `${index * ROW_HEIGHT}px`;
  const message = rowAt(index);
  if (!message) {
    row.classList.add('loading');
    return row;
  }
  if (isUnread(message)) row.classList.add('unread');
  if (message.flagged) row.classList.add('flagged');
  if (isSelectedRow(message)) row.classList.add('selected');
  for (const [cls, text] of [
    ['date', message.date],
    ['sender', message.sender],
    ['subject', message.subject],
  ]) {
    const span = document.createElement('span');
    span.className = cls;
    span.textContent = text;
    row.appendChild(span);
  }
  appendRowMarks(row, message);
  row.addEventListener('click', () => openMessage(message, index));
  return row;
}

async function openMessage(message, index) {
  currentMessage = message;
  currentIndex = index;

  // Ouvrir un message le marque lu : localement tout de suite, le serveur
  // suivra à la prochaine synchro via la file d'actions.
  if (!message.seen) {
    message.seen = true;
    markThreadRead(message);
    invoke('mark_seen', {
      accountId: message.account_id,
      mailbox: message.mailbox,
      uid: message.uid,
      seen: true,
    }).catch(() => {});
  }
  renderVisible();

  el('detail-placeholder').hidden = true;
  // Une composition en cours reste au premier plan : le brouillon ne
  // disparaît pas parce qu'on a cliqué sur la liste.
  if (el('compose').hidden) el('detail').hidden = false;
  updateStarButton();
  el('detail-subject').textContent = message.subject;
  el('detail-meta').textContent = `${message.sender} — ${message.date}`;
  el('detail-note').hidden = true;
  el('detail-frame').setAttribute('srcdoc', '');
  renderAttachments([]);
  renderThread([], message);
  setStatus('chargement du message…');
  await loadBody(message, false);
  await refreshAttachments(message);
  await refreshThread(message);
}

// --- Conversations --------------------------------------------------

// Le compteur non-lu du fil vit sur la ligne de LISTE, qui n'est pas
// l'objet ouvert quand on lit un message depuis le bandeau. Sans ce
// decompte, un fil resterait en gras apres avoir ete lu en entier.
function markThreadRead(message) {
  if (!message.thread_id) return;
  for (const page of pages.values()) {
    for (const row of page) {
      if (row && row.thread_id === message.thread_id && row.thread_unseen > 0) {
        row.thread_unseen -= 1;
      }
    }
  }
}

async function refreshThread(message) {
  if (!message.thread_id || message.thread_size <= 1) {
    renderThread([], message);
    return;
  }
  try {
    const messages = await invoke('thread_messages', { threadId: message.thread_id });
    // Le message affiche a pu changer pendant l'aller-retour.
    if (currentMessage && currentMessage.uid === message.uid
      && currentMessage.account_id === message.account_id) {
      renderThread(messages, message);
    }
  } catch (err) {
    // Purement local : un echec ici ne peut pas venir du reseau. On le
    // dit plutot que de laisser un bandeau vide inexplique.
    setStatus(`conversation indisponible : ${err}`, true);
  }
}

function renderThread(messages, current) {
  const strip = el('thread-strip');
  strip.replaceChildren();
  strip.hidden = messages.length <= 1;
  if (strip.hidden) return;

  for (const message of messages) {
    const item = document.createElement('button');
    item.className = 'thread-item';
    item.dataset.testid = 'thread-item';
    if (message.uid === current.uid && message.account_id === current.account_id) {
      item.classList.add('current');
    }
    if (!message.seen) item.classList.add('unread');
    // textContent, jamais innerHTML : ces chaines viennent d'un mail.
    item.textContent = `${message.date} — ${message.sender}`;
    item.title = message.subject;
    item.addEventListener('click', () => openMessage(message, currentIndex));
    strip.appendChild(item);
  }
}

// --- Pièces jointes -------------------------------------------------
//
// Les métadonnées sont locales et gratuites (elles arrivent avec le
// corps) ; les octets se paient à la demande, au clic. Rien n'est mis en
// cache : le budget disque de l'ADR 0007 ne survivrait pas aux fichiers.

async function refreshAttachments(message) {
  try {
    const found = await invoke('message_attachments', {
      accountId: message.account_id,
      mailbox: message.mailbox,
      uid: message.uid,
    });
    // Le message affiché a pu changer pendant l'aller-retour.
    if (currentMessage && currentMessage.uid === message.uid
      && currentMessage.account_id === message.account_id) {
      renderAttachments(found, message);
    }
  } catch {
    renderAttachments([]);
  }
}

function renderAttachments(found, message) {
  const bar = el('attachments');
  bar.replaceChildren();
  bar.hidden = found.length === 0;
  for (const attachment of found) {
    bar.appendChild(buildAttachmentButton(attachment, message));
  }
}

function buildAttachmentButton(attachment, message) {
  const button = document.createElement('button');
  button.className = 'attachment';
  button.dataset.testid = 'attachment';
  button.type = 'button';
  // textContent, jamais innerHTML : ce nom vient du réseau.
  button.textContent = `📎 ${attachment.name} (${attachment.size})`;
  button.title = attachment.mime;
  button.addEventListener('click', async () => {
    button.disabled = true;
    setStatus(`téléchargement de ${attachment.name}…`);
    try {
      const path = await invoke('save_attachment', {
        accountId: message.account_id,
        mailbox: message.mailbox,
        uid: message.uid,
        index: attachment.index,
      });
      setStatus(`enregistré : ${path}`);
    } catch (err) {
      setStatus(`enregistrement impossible : ${err}`, true);
    } finally {
      button.disabled = false;
    }
  });
  return button;
}

async function openMessageAt(index) {
  if (index < 0 || index >= total) return;
  let message = rowAt(index);
  if (!message) {
    try {
      const page = await invoke('list_messages', { offset: index, limit: 1 });
      message = page.rows[0];
    } catch {
      return;
    }
  }
  if (!message) return;
  scrollToIndex(index);
  await openMessage(message, index);
}

function scrollToIndex(index) {
  const pane = el('pane-list');
  const top = index * ROW_HEIGHT;
  if (top < pane.scrollTop) {
    pane.scrollTop = top;
  } else if (top + ROW_HEIGHT > pane.scrollTop + pane.clientHeight) {
    pane.scrollTop = top + ROW_HEIGHT - pane.clientHeight;
  }
}

function closeDetail() {
  currentMessage = null;
  el('detail').hidden = true;
  el('detail-placeholder').hidden = false;
}

function updateStarButton() {
  const star = el('star');
  const on = Boolean(currentMessage && currentMessage.flagged);
  star.textContent = on ? '★' : '☆';
  star.title = on ? "Retirer l'étoile (s)" : 'Étoiler (s)';
}

/// Étoile : optimiste localement, le serveur suivra au prochain sync.
async function toggleStar() {
  if (!currentMessage) return;
  currentMessage.flagged = !currentMessage.flagged;
  updateStarButton();
  renderVisible();
  try {
    await invoke('mark_flagged', {
      accountId: currentMessage.account_id,
      mailbox: currentMessage.mailbox,
      uid: currentMessage.uid,
      flagged: currentMessage.flagged,
    });
  } catch (err) {
    setStatus(`étoile impossible : ${err}`, true);
  }
}

/// Archive ou supprime le message ouvert, puis avance au suivant.
// Les trois actions qui retirent le message de la boîte disent la même
// chose : c'est local tout de suite, le serveur suit au prochain sync.
const ACTION_DONE = {
  archive: () => 'archivé — le serveur suivra au prochain sync',
  delete: () => 'supprimé — le serveur suivra au prochain sync',
  move: (folder) => `déplacé vers ${folder.display} — le serveur suivra au prochain sync`,
};

// --- Déplacer vers un dossier ---------------------------------------

async function openMoveDialog() {
  if (!currentMessage) return;
  const list = el('move-list');
  list.replaceChildren();
  el('move-dialog').hidden = false;
  setStatus('lecture des dossiers…');
  try {
    const folders = await invoke('list_folders', { accountId: currentMessage.account_id });
    if (folders.length === 0) {
      const empty = document.createElement('p');
      empty.className = 'dialog-note';
      empty.textContent = 'Aucun dossier de destination sur ce compte.';
      list.appendChild(empty);
      setStatus('');
      return;
    }
    for (const folder of folders) {
      list.appendChild(buildFolderChoice(folder));
    }
    setStatus('');
  } catch (err) {
    el('move-dialog').hidden = true;
    setStatus(`dossiers indisponibles : ${err}`, true);
  }
}

function buildFolderChoice(folder) {
  const button = document.createElement('button');
  button.type = 'button';
  // textContent : ce nom vient du serveur.
  button.textContent = folder.display;
  button.addEventListener('click', async () => {
    el('move-dialog').hidden = true;
    await performAction('move', folder);
  });
  return button;
}

async function performAction(kind, folder = null) {
  if (!currentMessage) return;
  const index = currentIndex;
  const accountId = currentMessage.account_id;
  // La BOITE, pas seulement l UID : les UID repartent de 1 dans
  // chaque boite, donc agir sans elle viserait un autre message.
  const mailbox = currentMessage.mailbox;
  const uid = currentMessage.uid;
  try {
    if (kind === 'move') {
      // On renvoie le nom RÉSEAU, jamais le nom affiché : le rejeu se
      // fera peut-être des jours plus tard, contre le serveur.
      await invoke('move_message', { accountId, mailbox, uid, folder: folder.wire });
    } else {
      await invoke(kind === 'archive' ? 'archive_message' : 'delete_message', { accountId, mailbox, uid });
    }
  } catch (err) {
    setStatus(`action impossible : ${err}`, true);
    return;
  }
  setStatus(ACTION_DONE[kind](folder));
  closeDetail();
  await reloadList();
  if (searchMode) {
    // La recherche occupe le volet : on retire le message traité des
    // résultats (sinon il y reste, périmé et cliquable) sans revenir à
    // la boîte unifiée.
    el('empty').hidden = true;
    await runSearch();
  } else if (total > 0 && index !== null) {
    await openMessageAt(Math.min(index, total - 1));
  }
}

async function loadBody(message, showImages) {
  try {
    const view = await invoke('message_body', {
      accountId: message.account_id,
      mailbox: message.mailbox,
      uid: message.uid,
      showImages,
    });
    if (currentMessage !== message) return; // l'utilisateur a changé de message
    el('detail-frame').setAttribute('srcdoc', view.document);
    const note = el('detail-note');
    if (!showImages && view.remote_images_blocked > 0) {
      el('note-text').textContent = `${view.remote_images_blocked} image(s) distante(s) `
        + 'bloquée(s) pour protéger votre vie privée.';
      note.hidden = false;
    } else {
      note.hidden = true;
    }
    setStatus('');
  } catch (err) {
    setStatus(`impossible de charger le message : ${err}`, true);
  }
}

// --- Composer, répondre, envoyer -----------------------------------------

/// Ouvre une composition en conservant d'abord celle qui serait en cours :
/// aucun chemin de l'application ne jette du texte.
async function startCompose(options) {
  await closeCompose();
  openCompose(options);
}

function openCompose({ to = '', subject = '', body = '', replyToMailbox = null, replyToUid = null, draftId = null, draftEpoch = null, accountId = null, title = 'Nouveau message' } = {}) {
  composeReplyUid = replyToUid;
  composeReplyMailbox = replyToMailbox;
  composeDraftId = draftId;
  composeDraftEpoch = draftEpoch;
  // Le compte émetteur : celui du message répondu/repris, sinon le premier.
  composeAccountId = accountId
    ?? (connectedAccounts.length > 0 ? connectedAccounts[0].id : null);
  if (composeAccountId !== null) {
    el('compose-from').value = String(composeAccountId);
  }
  el('compose-title').textContent = title;
  el('compose-to').value = to;
  el('compose-subject').value = subject;
  el('compose-body').value = body;
  el('detail').hidden = true;
  el('detail-placeholder').hidden = true;
  el('compose').hidden = false;
  // Top-posting : le curseur se pose AU-DESSUS de la citation.
  const field = to ? el('compose-body') : el('compose-to');
  field.focus();
  if (field === el('compose-body')) field.setSelectionRange(0, 0);
}

/// Masque le panneau sans rien décider du sort du brouillon (interne).
function hideCompose() {
  clearTimeout(draftSaveTimer);
  composeReplyUid = null;
  composeReplyMailbox = null;
  composeAccountId = null;
  composeDraftId = null;
  composeDraftEpoch = null;
  el('compose').hidden = true;
  if (currentMessage) {
    el('detail').hidden = false;
  } else {
    el('detail-placeholder').hidden = false;
  }
}

/// Fermer = conserver : un contenu non vide devient (ou reste) un
/// brouillon ; un brouillon vidé de son texte est jeté — c'est le seul
/// cas où fermer supprime, et c'est l'utilisateur qui a effacé.
async function closeCompose() {
  if (el('compose').hidden) return;
  let forked = false;
  if (composeIsEmpty()) {
    if (composeDraftId !== null) {
      await invoke('delete_draft', { id: composeDraftId }).catch(() => {});
    }
  } else {
    const saved = await saveDraftNow();
    // Le bilan decide du message. Poser « brouillon conservé » sans
    // regarder ecrasait l'avertissement de conflit une ligne apres que
    // saveDraftNow l'ait affiche : la fusion avait bien lieu, mais rien
    // ne le disait -- le defaut trouve en validation terrain.
    forked = Boolean(saved && saved.forked);
    setStatus(forked
      ? 'ce brouillon avait changé ailleurs — votre version a été conservée '
        + 'à part, retrouvez-la dans la liste'
      : 'brouillon conservé', forked);
  }
  hideCompose();
  await refreshDrafts();
  // Et la poussée non plus ne doit pas l'effacer. Elle revient du réseau
  // une seconde plus tard et posait SON bilan par-dessus — or le
  // brouillon conservé à part est neuf, donc toujours à pousser : la
  // collision était certaine, pas fortuite. L'avertissement est la seule
  // ligne qui signale à l'utilisateur qu'il a deux textes à arbitrer.
  pushDrafts({ discret: forked });
}

function composeIsEmpty() {
  return !el('compose-to').value.trim()
    && !el('compose-subject').value.trim()
    && !el('compose-body').value.trim();
}

function scheduleDraftSave() {
  clearTimeout(draftSaveTimer);
  draftSaveTimer = setTimeout(saveDraftNow, 2000);
}

/// Le filet : un crash ne coûte que les deux dernières secondes de frappe.
///
/// Rend le bilan de la sauvegarde, ou `null` s'il n'y avait rien a faire —
/// l'appelant a besoin de savoir si un conflit a ete resolu par
/// duplication, pour ne pas ecraser l'avertissement.
async function saveDraftNow() {
  clearTimeout(draftSaveTimer);
  if (el('compose').hidden || composeIsEmpty() || composeAccountId === null) return null;
  try {
    const saved = await invoke('save_draft', {
      accountId: composeAccountId,
      id: composeDraftId,
      baseEpoch: composeDraftEpoch,
      content: {
        to: el('compose-to').value,
        subject: el('compose-subject').value,
        body: el('compose-body').value,
        replyToMailbox: composeReplyMailbox,
      replyToUid: composeReplyUid,
      },
    });
    if (el('compose').hidden) {
      // Le panneau s'est fermé pendant la sauvegarde (envoi parti) :
      // ne pas ressusciter un brouillon déjà réglé.
      await invoke('delete_draft', { id: saved.id }).catch(() => {});
      return null;
    }
    composeDraftId = saved.id;
    composeDraftEpoch = saved.updated_epoch;
    if (saved.forked) {
      // Ne JAMAIS taire ce cas : deux textes existent desormais, et
      // l'utilisateur est le seul a pouvoir trancher. Le lui cacher
      // reviendrait a lui faire perdre celui qu'il ne verra pas.
      setStatus('ce brouillon avait changé ailleurs — votre version a été '
        + 'conservée à part, retrouvez-la dans la liste', true);
      await refreshDrafts();
    }
    return saved;
  } catch {
    // La prochaine frappe retentera — le filet n'alarme pas pour rien.
  }
  return null;
}

function replyToCurrent() {
  return composeFromContext('reply_context', 'Répondre');
}

function forwardCurrent() {
  return composeFromContext('forward_context', 'Transférer');
}

/// Réponse ou transfert : le noyau prépare destinataire, sujet et
/// citation (corps depuis le cache local, serveur sinon — d'où l'attente).
async function composeFromContext(command, title) {
  if (!currentMessage) return;
  setStatus('préparation…');
  try {
    const context = await invoke(command, {
      accountId: currentMessage.account_id,
      mailbox: currentMessage.mailbox,
      uid: currentMessage.uid,
    });
    setStatus('');
    await startCompose({
      to: context.to,
      subject: context.subject,
      body: context.body,
      replyToMailbox: context.reply ? context.mailbox : null,
      replyToUid: context.reply ? context.uid : null,
      accountId: context.account_id,
      title,
    });
  } catch (err) {
    setStatus(`${title} impossible : ${err}`, true);
  }
}

/// Journalise l'envoi (retour immédiat, même hors ligne), puis vidange.
async function sendCompose() {
  const send = el('compose-send');
  if (send.disabled) return; // double-clic = un seul envoi
  if (composeAccountId === null) {
    setStatus('aucun compte émetteur — ajoutez un compte', true);
    return;
  }
  send.disabled = true;
  try {
    await invoke('queue_send', {
      accountId: composeAccountId,
      to: el('compose-to').value,
      subject: el('compose-subject').value.trim(),
      body: el('compose-body').value,
      replyToMailbox: composeReplyMailbox,
      replyToUid: composeReplyUid,
    });
  } catch (err) {
    setStatus(`envoi impossible : ${err}`, true);
    return;
  } finally {
    send.disabled = false;
  }
  // L'envoi est journalisé : le brouillon a rempli son office.
  const draftId = composeDraftId;
  hideCompose();
  setStatus("remis à la boîte d'envoi…");
  if (draftId !== null) {
    await invoke('delete_draft', { id: draftId }).catch(() => {});
  }
  await refreshDrafts();
  await flushOutbox();
  pushDrafts(); // purge de la copie distante du brouillon réglé
}

async function flushOutbox() {
  try {
    const report = await invoke('flush_outbox');
    if (report.error) {
      setStatus(`hors ligne — ${report.queued} envoi(s) en attente, réessai au prochain sync`);
    } else if (report.sent > 0) {
      setStatus(`${report.sent} message(s) envoyé(s)`);
    }
  } catch (err) {
    setStatus(`boîte d'envoi : ${err}`, true);
  }
  await refreshOutbox();
}

/// Le bandeau : rien à cacher — ce qui attend, ce qui est interrompu ou
/// refusé est visible, avec la décision explicite laissée à l'utilisateur.
async function refreshOutbox() {
  let status;
  try {
    status = await invoke('outbox_status');
  } catch {
    return;
  }
  const bar = el('outbox-bar');
  const total = status.queued + status.interrupted + status.rejected;
  if (total === 0) {
    bar.hidden = true;
    return;
  }
  const parts = [];
  if (status.queued > 0) parts.push(`${status.queued} en attente`);
  if (status.interrupted > 0) parts.push(`${status.interrupted} interrompu(s)`);
  if (status.rejected > 0) parts.push(`${status.rejected} refusé(s)`);
  el('outbox-summary').textContent = `Boîte d'envoi : ${parts.join(', ')}`;

  const problems = el('outbox-problems');
  problems.replaceChildren();
  for (const entry of status.entries) {
    if (entry.state === 'interrupted' || entry.state === 'rejected') {
      problems.appendChild(problemRow(entry));
    }
  }
  bar.hidden = false;
}

/// Le bandeau des brouillons : reprendre où on s'était arrêté, ou jeter.
async function refreshDrafts() {
  let drafts;
  try {
    drafts = await invoke('list_drafts');
  } catch {
    return;
  }
  const bar = el('drafts-bar');
  if (drafts.length === 0) {
    bar.hidden = true;
    return;
  }
  el('drafts-summary').textContent = `Brouillon(s) : ${drafts.length}`;
  const list = el('drafts-list');
  list.replaceChildren();
  for (const draft of drafts) {
    list.appendChild(draftRow(draft));
  }
  bar.hidden = false;
}

/// Longueur de l'extrait de corps affiché dans le bandeau. Assez pour
/// séparer deux versions d'un même brouillon, assez court pour tenir sur
/// une ligne à côté des deux boutons.
const EXTRAIT_BROUILLON_MAX = 70;

/// Ce qui distingue deux versions d'un même brouillon quand le sujet et
/// le destinataire sont identiques : le corps, réduit à une ligne.
///
/// Découpé en **points de code** (`Array.from`) et non en unités UTF-16 :
/// `slice` couperait un emoji en deux moitiés invalides.
function extraitBrouillon(body) {
  const ligne = (body || '').replace(/\s+/g, ' ').trim();
  const points = Array.from(ligne);
  if (points.length <= EXTRAIT_BROUILLON_MAX) return ligne;
  const coupe = points.slice(0, EXTRAIT_BROUILLON_MAX).join('');
  // Reculer jusqu'au dernier espace pour ne pas trancher un mot — sauf
  // si cela ampute trop (un seul mot très long), auquel cas on coupe net.
  const espace = coupe.lastIndexOf(' ');
  return `${espace > EXTRAIT_BROUILLON_MAX / 2 ? coupe.slice(0, espace) : coupe}…`;
}

function draftRow(draft) {
  const row = document.createElement('div');
  row.className = 'bar-row';
  const label = document.createElement('span');
  label.textContent = `« ${draft.subject || '(sans objet)'} »${draft.to ? ` à ${draft.to}` : ''}`;
  label.title = label.textContent;

  // Sans lui, deux versions du même brouillon sont indiscernables — et
  // toute consigne de validation qui porte sur elles est invérifiable.
  // Le corps est déjà là (« Reprendre » s'en sert) : aucun aller-retour.
  const extrait = document.createElement('span');
  extrait.className = 'draft-excerpt';
  extrait.textContent = extraitBrouillon(draft.body);

  const resume = document.createElement('button');
  resume.textContent = 'Reprendre';
  resume.addEventListener('click', () => startCompose({
    to: draft.to,
    subject: draft.subject,
    body: draft.body,
    replyToUid: draft.reply_to_uid,
    draftId: draft.id,
    draftEpoch: draft.updated_epoch,
    accountId: draft.account_id,
    title: 'Brouillon',
  }));

  const discard = document.createElement('button');
  discard.textContent = 'Supprimer';
  discard.addEventListener('click', async () => {
    await invoke('delete_draft', { id: draft.id }).catch(() => {});
    await refreshDrafts();
  });

  row.append(label, extrait, resume, discard);
  return row;
}

function problemRow(entry) {
  const row = document.createElement('div');
  row.className = 'bar-row';
  const label = document.createElement('span');
  const kind = entry.state === 'interrupted'
    ? 'interrompu en plein envoi — vérifiez le dossier Envoyés avant de renvoyer'
    : `refusé : ${entry.error ?? 'raison inconnue'}`;
  label.textContent = `« ${entry.subject || '(sans objet)'} » à ${entry.to} — ${kind}`;
  label.title = label.textContent;

  const resend = document.createElement('button');
  resend.textContent = 'Renvoyer';
  resend.addEventListener('click', async () => {
    try {
      await invoke('outbox_requeue', { id: entry.id });
    } catch (err) {
      setStatus(`renvoi impossible : ${err}`, true);
      return;
    }
    await flushOutbox();
  });

  const abandon = document.createElement('button');
  abandon.textContent = 'Abandonner';
  abandon.addEventListener('click', async () => {
    try {
      await invoke('outbox_delete', { id: entry.id });
    } catch (err) {
      setStatus(`abandon impossible : ${err}`, true);
      return;
    }
    await refreshOutbox();
  });

  row.append(label, resend, abandon);
  return row;
}

function setStatus(text, isError = false) {
  const status = el('status');
  status.textContent = text;
  status.classList.toggle('error', isError);
}

// Ferme le dialogue d'ajout ouvert, s'il y en a un. Renvoie `true` si
// quelque chose a effectivement été fermé — Échap doit consommer la
// touche plutôt que de la laisser filer vers une autre action.
function closeAddDialog() {
  for (const name of ['imap', 'ms', 'move']) {
    if (!el(`${name}-dialog`).hidden) {
      el(`${name}-dialog`).hidden = true;
      // Le sélecteur de dossier n'est pas un formulaire : rien à vider.
      if (typeof el(`${name}-form`).reset === 'function') el(`${name}-form`).reset();
      return true;
    }
  }
  return false;
}

// Les reconnexions silencieuses ont eu lieu au démarrage
// (connect_accounts) : ce bouton AJOUTE un compte — parcours navigateur.
function toggleAddMenu() {
  el('add-menu').hidden = !el('add-menu').hidden;
}

el('connect').addEventListener('click', toggleAddMenu);

el('add-gmail').addEventListener('click', async () => {
  el('add-menu').hidden = true;
  setStatus('autorisation en cours dans votre navigateur…');
  try {
    const account = await invoke('add_account');
    if (!connectedAccounts.some((known) => known.id === account.id)) {
      connectedAccounts.push(account);
    }
    renderAccounts();
    await onConnected();
  } catch (err) {
    setStatus(`connexion impossible : ${err}`, true);
  }
});

// Microsoft ne communique pas l'adresse du compte dans le périmètre de
// scopes mesuré (ADR 0006) : on la demande AVANT d'ouvrir le navigateur.
el('add-microsoft').addEventListener('click', () => {
  el('add-menu').hidden = true;
  el('ms-dialog').hidden = false;
  el('ms-email').focus();
});

el('ms-cancel').addEventListener('click', () => {
  el('ms-dialog').hidden = true;
  el('ms-form').reset();
});

el('ms-form').addEventListener('submit', async (event) => {
  event.preventDefault();
  const email = el('ms-email').value.trim();
  // Le dialogue se ferme avant le consentement : le navigateur prend la
  // main, et laisser une modale par-dessus l'app serait déroutant.
  el('ms-dialog').hidden = true;
  el('ms-form').reset();
  setStatus('autorisation Microsoft en cours dans votre navigateur…');
  try {
    const account = await invoke('add_microsoft_account', { email });
    if (!connectedAccounts.some((known) => known.id === account.id)) {
      connectedAccounts.push(account);
    }
    renderAccounts();
    setStatus('compte Microsoft ajouté');
    await onConnected();
  } catch (err) {
    setStatus(`ajout Microsoft impossible : ${err}`, true);
  }
});

el('add-imap').addEventListener('click', () => {
  el('add-menu').hidden = true;
  el('imap-dialog').hidden = false;
  el('imap-email').focus();
});

el('imap-cancel').addEventListener('click', () => {
  el('imap-dialog').hidden = true;
  el('imap-form').reset();
});

// Fermer le menu d'ajout en cliquant ailleurs.
document.addEventListener('click', (event) => {
  if (!el('add-menu').hidden
    && !el('connect').contains(event.target)
    && !el('add-menu').contains(event.target)) {
    el('add-menu').hidden = true;
  }
});

el('imap-form').addEventListener('submit', async (event) => {
  event.preventDefault();
  const email = el('imap-email').value.trim();
  const username = el('imap-username').value.trim() || email;
  const password = el('imap-password').value;
  const imapHost = el('imap-host').value.trim();
  const imapPort = Number(el('imap-port').value) || 993;
  const smtpHost = el('smtp-host').value.trim();
  const smtpPort = Number(el('smtp-port').value) || 465;

  setStatus('vérification du compte IMAP…');
  try {
    // La commande prend UN argument `input` : les champs vont dans un
    // objet imbriqué, pas à plat (sinon Tauri refuse l'appel).
    const account = await invoke('add_generic_account', {
      input: {
        email,
        username: username === email ? null : username,
        password,
        imapHost,
        imapPort,
        smtpHost,
        smtpPort,
      },
    });
    if (!connectedAccounts.some((known) => known.id === account.id)) {
      connectedAccounts.push(account);
    }
    renderAccounts();
    el('imap-dialog').hidden = true;
    el('imap-form').reset();
    setStatus('compte IMAP ajouté');
    await onConnected();
  } catch (err) {
    setStatus(`ajout IMAP impossible : ${err}`, true);
  }
});

el('refresh').addEventListener('click', refresh);
el('archive').addEventListener('click', () => performAction('archive'));
el('move').addEventListener('click', openMoveDialog);
el('move-cancel').addEventListener('click', () => { el('move-dialog').hidden = true; });
el('delete').addEventListener('click', () => performAction('delete'));
el('compose-btn').addEventListener('click', () => startCompose());
el('star').addEventListener('click', toggleStar);
el('reply').addEventListener('click', replyToCurrent);
el('forward').addEventListener('click', forwardCurrent);
el('compose-send').addEventListener('click', sendCompose);
el('compose-cancel').addEventListener('click', closeCompose);

// Chaque frappe (re)programme la sauvegarde du brouillon.
for (const id of ['compose-to', 'compose-subject', 'compose-body']) {
  el(id).addEventListener('input', scheduleDraftSave);
}

// Changer de compte émetteur re-scope le brouillon en cours.
el('compose-from').addEventListener('change', () => {
  composeAccountId = Number(el('compose-from').value);
  scheduleDraftSave();
});

el('show-images').addEventListener('click', async () => {
  if (!currentMessage) return;
  setStatus('affichage des images…');
  await loadBody(currentMessage, true);
});

el('search').addEventListener('input', () => {
  clearTimeout(searchTimer);
  searchTimer = setTimeout(runSearch, 150);
});

el('search').addEventListener('keydown', (event) => {
  if (event.key === 'Escape') {
    event.preventDefault();
    hideSearch();
  }
});

// Raccourcis : c (écrire), r (répondre), e (archiver), Suppr (supprimer),
// j/k (naviguer), Échap (fermer la composition). Dans un champ de saisie,
// les lettres redeviennent des lettres — seul Échap garde un sens (sortir
// du champ, sans jeter le brouillon).
document.addEventListener('keydown', (event) => {
  if (event.ctrlKey || event.metaKey || event.altKey) return;
  const typing = event.target instanceof HTMLInputElement
    || event.target instanceof HTMLTextAreaElement;
  if (typing) {
    if (event.key === 'Escape') {
      if (!closeAddDialog()) {
        event.target.blur();
      }
    }
    return;
  }
  switch (event.key) {
    case 'c':
      startCompose();
      break;
    case 'r':
      replyToCurrent();
      break;
    case 'f':
      forwardCurrent();
      break;
    case 's':
      toggleStar();
      break;
    case 'e':
      performAction('archive');
      break;
    case 'v':
      openMoveDialog();
      break;
    case 'Delete':
      performAction('delete');
      break;
    case 'j':
      if (currentIndex !== null && !searchMode) openMessageAt(currentIndex + 1);
      break;
    case 'k':
      if (currentIndex !== null && !searchMode) openMessageAt(currentIndex - 1);
      break;
    case '/':
      showSearch();
      break;
    case 'Escape':
      if (closeAddDialog()) {
        break;
      } else if (!el('compose').hidden) {
        closeCompose();
      } else if (searchMode) {
        hideSearch();
      } else {
        return;
      }
      break;
    default:
      return;
  }
  event.preventDefault();
});

init();
