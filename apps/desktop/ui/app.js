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

async function init() {
  invoke('startup_report').then((report) => { el('perf').textContent = report; });
  el('pane-list').addEventListener('scroll', onScroll);
  try {
    const account = await invoke('connect_account', { interactive: false });
    await onConnected(account.email);
  } catch {
    el('connect').hidden = false;
    setStatus('Connectez votre compte Gmail pour commencer.');
    await reloadList();
    await refreshOutbox();
  }
}

async function onConnected(email) {
  el('account').textContent = email;
  el('account').hidden = false;
  el('connect').hidden = true;
  el('refresh').hidden = false;
  el('compose-btn').hidden = false;
  await reloadList();
  await refresh();
}

async function refresh() {
  setStatus('synchronisation…');
  try {
    const report = await invoke('sync_inbox');
    const actions = report.replayed > 0 ? `, ${report.replayed} action(s) envoyée(s)` : '';
    setStatus(`synchro ${report.mode} : ${report.fetched} récupéré(s), `
      + `${report.deleted} supprimé(s)${actions} — ${report.total} messages, en ${report.elapsed_ms} ms`);
  } catch (err) {
    setStatus(`erreur de synchronisation : ${err}`, true);
  }
  await reloadList();
  // Le réseau est peut-être revenu : la boîte d'envoi retente sa chance.
  await flushOutbox();
}

async function reloadList() {
  pages.clear();
  pending.clear();
  try {
    const first = await fetchPage(0);
    total = first.total;
    el('perf').textContent =
      `${total} messages — page servie en ${(first.elapsed_us / 1000).toFixed(2)} ms`;
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
  row.style.top = `${index * ROW_HEIGHT}px`;
  const message = rowAt(index);
  if (!message) {
    row.classList.add('loading');
    return row;
  }
  if (!message.seen) row.classList.add('unread');
  if (currentMessage && message.uid === currentMessage.uid) {
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
    invoke('mark_seen', { uid: message.uid, seen: true }).catch(() => {});
  }
  renderVisible();

  el('detail-placeholder').hidden = true;
  // Une composition en cours reste au premier plan : le brouillon ne
  // disparaît pas parce qu'on a cliqué sur la liste.
  if (el('compose').hidden) el('detail').hidden = false;
  el('detail-subject').textContent = message.subject;
  el('detail-meta').textContent = `${message.sender} — ${message.date}`;
  el('detail-note').hidden = true;
  el('detail-frame').setAttribute('srcdoc', '');
  setStatus('chargement du message…');
  await loadBody(message, false);
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

/// Archive ou supprime le message ouvert, puis avance au suivant.
async function performAction(kind) {
  if (!currentMessage) return;
  const index = currentIndex;
  const uid = currentMessage.uid;
  try {
    await invoke(kind === 'archive' ? 'archive_message' : 'delete_message', { uid });
  } catch (err) {
    setStatus(`action impossible : ${err}`, true);
    return;
  }
  setStatus(kind === 'archive'
    ? 'archivé — le serveur suivra au prochain sync'
    : 'supprimé — le serveur suivra au prochain sync');
  closeDetail();
  await reloadList();
  if (total > 0 && index !== null) {
    await openMessageAt(Math.min(index, total - 1));
  }
}

async function loadBody(message, showImages) {
  try {
    const view = await invoke('message_body', { uid: message.uid, showImages });
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

function openCompose({ to = '', subject = '', replyToUid = null, title = 'Nouveau message' } = {}) {
  composeReplyUid = replyToUid;
  el('compose-title').textContent = title;
  el('compose-to').value = to;
  el('compose-subject').value = subject;
  el('compose-body').value = '';
  el('detail').hidden = true;
  el('detail-placeholder').hidden = true;
  el('compose').hidden = false;
  (to ? el('compose-body') : el('compose-to')).focus();
}

function closeCompose() {
  composeReplyUid = null;
  el('compose').hidden = true;
  if (currentMessage) {
    el('detail').hidden = false;
  } else {
    el('detail-placeholder').hidden = false;
  }
}

async function replyToCurrent() {
  if (!currentMessage) return;
  try {
    const context = await invoke('reply_context', { uid: currentMessage.uid });
    openCompose({
      to: context.to,
      subject: context.subject,
      replyToUid: context.uid,
      title: 'Répondre',
    });
  } catch (err) {
    setStatus(`réponse impossible : ${err}`, true);
  }
}

/// Journalise l'envoi (retour immédiat, même hors ligne), puis vidange.
async function sendCompose() {
  const send = el('compose-send');
  if (send.disabled) return; // double-clic = un seul envoi
  send.disabled = true;
  try {
    await invoke('queue_send', {
      to: el('compose-to').value,
      subject: el('compose-subject').value.trim(),
      body: el('compose-body').value,
      replyToUid: composeReplyUid,
    });
  } catch (err) {
    setStatus(`envoi impossible : ${err}`, true);
    return;
  } finally {
    send.disabled = false;
  }
  closeCompose();
  setStatus("remis à la boîte d'envoi…");
  await flushOutbox();
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

function problemRow(entry) {
  const row = document.createElement('div');
  row.className = 'outbox-problem';
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

// La voie silencieuse d'abord : après un démarrage hors ligne, le coffre
// contient souvent un refresh token encore valide — le navigateur ne doit
// s'ouvrir qu'en dernier recours (friction observée en validation Phase 2).
el('connect').addEventListener('click', async () => {
  setStatus('reconnexion…');
  try {
    const account = await invoke('connect_account', { interactive: false });
    await onConnected(account.email);
    return;
  } catch {
    // pas de session récupérable sans interaction : parcours complet
  }
  setStatus('autorisation en cours dans votre navigateur…');
  try {
    const account = await invoke('connect_account', { interactive: true });
    await onConnected(account.email);
  } catch (err) {
    setStatus(`connexion impossible : ${err}`, true);
  }
});

el('refresh').addEventListener('click', refresh);
el('archive').addEventListener('click', () => performAction('archive'));
el('delete').addEventListener('click', () => performAction('delete'));
el('compose-btn').addEventListener('click', () => openCompose());
el('reply').addEventListener('click', replyToCurrent);
el('compose-send').addEventListener('click', sendCompose);
el('compose-cancel').addEventListener('click', closeCompose);

el('show-images').addEventListener('click', async () => {
  if (!currentMessage) return;
  setStatus('affichage des images…');
  await loadBody(currentMessage, true);
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
    if (event.key === 'Escape') event.target.blur();
    return;
  }
  switch (event.key) {
    case 'c':
      openCompose();
      break;
    case 'r':
      replyToCurrent();
      break;
    case 'e':
      performAction('archive');
      break;
    case 'Delete':
      performAction('delete');
      break;
    case 'j':
      if (currentIndex !== null) openMessageAt(currentIndex + 1);
      break;
    case 'k':
      if (currentIndex !== null) openMessageAt(currentIndex - 1);
      break;
    case 'Escape':
      if (el('compose').hidden) return;
      closeCompose();
      break;
    default:
      return;
  }
  event.preventDefault();
});

init();
