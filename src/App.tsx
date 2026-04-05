import { useEffect, useMemo, useState } from "react";
import {
  archiveThread,
  createDraft,
  loadAppState,
  markSpam,
  refreshMailbox,
  selectMailbox,
  selectThread,
  sendDraft,
  updateDraft,
} from "./lib/backend";
import { ComposePanel } from "./components/ComposePanel";
import { ReaderPane } from "./components/ReaderPane";
import { Sidebar } from "./components/Sidebar";
import { ThreadList } from "./components/ThreadList";
import type { AppSnapshot, DraftDetail } from "./types";

function App() {
  const [snapshot, setSnapshot] = useState<AppSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    loadAppState()
      .then(setSnapshot)
      .catch((reason) => setError(String(reason)));
  }, []);

  const activeThreadId = snapshot?.selectedThreadId ?? null;
  const activeMailboxId = snapshot?.selectedMailboxId ?? "mail_unified";
  const activeMailboxLabel =
    snapshot?.mailboxes.find((mailbox) => mailbox.id === activeMailboxId)?.label ??
    "Inbox";

  const activeDraft = useMemo(() => snapshot?.activeDraft ?? null, [snapshot]);

  async function handleMailboxSelect(mailboxId: string) {
    setSnapshot(await selectMailbox(mailboxId));
  }

  async function handleThreadSelect(threadId: string) {
    setSnapshot(await selectThread(threadId));
  }

  async function handleRefresh() {
    if (!snapshot) {
      return;
    }

    setSnapshot(await refreshMailbox(snapshot.selectedMailboxId));
  }

  async function handleArchive() {
    if (!activeThreadId) {
      return;
    }

    setSnapshot(await archiveThread(activeThreadId));
  }

  async function handleSpam() {
    if (!activeThreadId) {
      return;
    }

    setSnapshot(await markSpam(activeThreadId));
  }

  async function handleCreateDraft() {
    const draft = await createDraft();
    setSnapshot((current) =>
      current
        ? {
            ...current,
            activeDraft: draft,
          }
        : current,
    );
  }

  async function handleSaveDraft(draft: DraftDetail) {
    const updated = await updateDraft({
      draftId: draft.envelope.id,
      to: draft.envelope.to,
      cc: draft.envelope.cc,
      bcc: draft.envelope.bcc,
      subject: draft.envelope.subject,
      htmlBody: draft.content.htmlBody,
      textBody: draft.content.textBody,
    });

    setSnapshot((current) =>
      current
        ? {
            ...current,
            activeDraft: updated,
          }
        : current,
    );
  }

  async function handleSendDraft(draftId: string) {
    setSnapshot(await sendDraft(draftId));
  }

  if (error) {
    return <main className="app-shell">{error}</main>;
  }

  if (!snapshot) {
    return <main className="app-shell">Loading Discovery...</main>;
  }

  return (
    <main className="app-shell">
      <Sidebar
        accounts={snapshot.accounts}
        mailboxes={snapshot.mailboxes}
        selectedMailboxId={activeMailboxId}
        syncStatus={snapshot.syncStatus}
        onSelectMailbox={handleMailboxSelect}
      />
      <ThreadList
        accounts={snapshot.accounts}
        mailboxLabel={activeMailboxLabel}
        threads={snapshot.threads}
        selectedThreadId={activeThreadId}
        onSelectThread={handleThreadSelect}
        onRefresh={handleRefresh}
      />
      <ReaderPane
        threadDetail={snapshot.threadDetail}
        onArchive={handleArchive}
        onSpam={handleSpam}
      />
      <ComposePanel
        draft={activeDraft}
        onCreateDraft={handleCreateDraft}
        onSaveDraft={handleSaveDraft}
        onSendDraft={handleSendDraft}
      />
    </main>
  );
}

export default App;
