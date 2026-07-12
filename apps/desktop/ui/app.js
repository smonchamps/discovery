// Règle absolue : les données du mail entrent dans le DOM par textContent,
// jamais par innerHTML.
const invoke = window.__TAURI__.core.invoke;
const el = (id) => document.getElementById(id);

async function init() {
  invoke('startup_report').then((report) => { el('perf').textContent = report; });
  try {
    const account = await invoke('connect_account', { interactive: false });
    await onConnected(account.email);
  } catch {
    el('connect').hidden = false;
    setStatus('Connectez votre compte Gmail pour commencer.');
    await renderList();
  }
}

async function onConnected(email) {
  el('account').textContent = email;
  el('account').hidden = false;
  el('connect').hidden = true;
  el('refresh').hidden = false;
  await renderList();
  await refresh();
}

async function refresh() {
  setStatus('synchronisation…');
  try {
    const report = await invoke('sync_inbox');
    setStatus(`synchro ${report.mode} : ${report.fetched} récupéré(s), `
      + `${report.deleted} supprimé(s) — ${report.total} messages, en ${report.elapsed_ms} ms`);
  } catch (err) {
    setStatus(`erreur de synchronisation : ${err}`, true);
  }
  await renderList();
}

async function renderList() {
  let messages = [];
  try {
    messages = await invoke('list_messages', { limit: 200 });
  } catch {
    return;
  }
  const list = el('list');
  list.replaceChildren();
  el('empty').hidden = messages.length > 0;
  for (const message of messages) {
    const li = document.createElement('li');
    if (!message.seen) li.classList.add('unread');
    for (const [cls, text] of [
      ['date', message.date],
      ['sender', message.sender],
      ['subject', message.subject],
    ]) {
      const span = document.createElement('span');
      span.className = cls;
      span.textContent = text;
      li.appendChild(span);
    }
    list.appendChild(li);
  }
}

function setStatus(text, isError = false) {
  const status = el('status');
  status.textContent = text;
  status.classList.toggle('error', isError);
}

el('connect').addEventListener('click', async () => {
  setStatus('autorisation en cours dans votre navigateur…');
  try {
    const account = await invoke('connect_account', { interactive: true });
    await onConnected(account.email);
  } catch (err) {
    setStatus(`connexion impossible : ${err}`, true);
  }
});

el('refresh').addEventListener('click', refresh);

init();
