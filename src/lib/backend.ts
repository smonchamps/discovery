import { invoke } from "@tauri-apps/api/core";
import { listen, type Event } from "@tauri-apps/api/event";
import type {
  AppSnapshot,
  DraftDetail,
  DraftUpdateInput,
  GmailEnrollmentStatus,
} from "../types";
import { mockSnapshot } from "./mockData";

export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

let browserState: AppSnapshot = structuredClone(mockSnapshot);

function threadsForMailbox(snapshot: AppSnapshot, mailboxId: string) {
  const sourceThreads = snapshot.allThreads ?? snapshot.threads;
  if (mailboxId === "mail_unified") {
    const inboxMailboxIds = new Set(
      snapshot.mailboxes
        .filter((mailbox) => mailbox.kind === "inbox")
        .map((mailbox) => mailbox.id),
    );
    return sourceThreads.filter((thread) => inboxMailboxIds.has(thread.mailboxId));
  }

  return sourceThreads.filter((thread) => thread.mailboxId === mailboxId);
}

function detailForThread(snapshot: AppSnapshot, threadId: string) {
  if (snapshot.threadDetail?.id === threadId) {
    return snapshot.threadDetail;
  }

  const thread = (snapshot.allThreads ?? snapshot.threads).find((entry) => entry.id === threadId);
  if (!thread) {
    return null;
  }

  return {
    id: thread.id,
    accountId: thread.accountId,
    mailboxId: thread.mailboxId,
    subject: thread.subject,
    participants: [
      thread.from,
      {
        name: "You",
        email:
          snapshot.accounts.find((account) => account.id === thread.accountId)?.email ?? "",
      },
    ],
    receivedAt: thread.receivedAt,
    badge: thread.badge,
    messages: [
      {
        id: `${thread.id}_message`,
        from: thread.from,
        to: [
          {
            name: "You",
            email:
              snapshot.accounts.find((account) => account.id === thread.accountId)?.email ?? "",
          },
        ],
        sentAt: thread.receivedAt,
        htmlBody: null,
        textBody: thread.snippet,
        attachments: [],
      },
    ],
  };
}

export async function loadAppState(): Promise<AppSnapshot> {
  if (isTauri) {
    return invoke<AppSnapshot>("load_app_state");
  }

  return browserState;
}

export async function startGmailEnrollment(): Promise<GmailEnrollmentStatus> {
  if (isTauri) {
    return invoke<GmailEnrollmentStatus>("start_gmail_enrollment");
  }

  const nextAccountId = `acc_demo_${browserState.accounts.length + 1}`;
  browserState = {
    ...browserState,
    accounts: [
      ...browserState.accounts,
      {
        id: nextAccountId,
        email: `demo${browserState.accounts.length + 1}@gmail.com`,
        displayName: `Demo Gmail ${browserState.accounts.length + 1}`,
        color: "#22C55E",
        status: "connected",
        unreadCount: 0,
      },
    ],
    mailboxes: [
      ...browserState.mailboxes,
      { id: `mail_inbox_${nextAccountId}`, accountId: nextAccountId, kind: "inbox", label: "Inbox", unreadCount: 0 },
      { id: `mail_drafts_${nextAccountId}`, accountId: nextAccountId, kind: "drafts", label: "Drafts", unreadCount: 0 },
      { id: `mail_sent_${nextAccountId}`, accountId: nextAccountId, kind: "sent", label: "Sent", unreadCount: 0 },
      { id: `mail_archive_${nextAccountId}`, accountId: nextAccountId, kind: "archive", label: "Archive", unreadCount: 0 },
      { id: `mail_spam_${nextAccountId}`, accountId: nextAccountId, kind: "spam", label: "Spam", unreadCount: 0 },
    ],
    syncStatus: {
      ...browserState.syncStatus,
      [nextAccountId]: {
        state: "idle",
        detail: "Mock Gmail enrollment completed.",
        lastSuccessfulSyncAt: new Date().toISOString(),
      },
    },
  };

  return {
    phase: "success",
    message: "Mock Gmail enrollment completed.",
    enrolledEmail: `demo${browserState.accounts.length}@gmail.com`,
  };
}

export async function subscribeToGmailEnrollmentUpdates(
  callback: (status: GmailEnrollmentStatus) => void,
) {
  if (!isTauri) {
    return () => {};
  }

  return listen("discovery://gmail-enrollment-updated", (event: Event<GmailEnrollmentStatus>) => {
    callback(event.payload);
  });
}

export async function subscribeToSnapshotUpdates(
  callback: (snapshot: AppSnapshot) => void,
) {
  if (!isTauri) {
    return () => {};
  }

  return listen("discovery://snapshot-updated", (event: Event<AppSnapshot>) => {
    callback(event.payload);
  });
}

export async function selectMailbox(mailboxId: string): Promise<AppSnapshot> {
  if (isTauri) {
    return invoke<AppSnapshot>("load_threads", { mailboxId });
  }

  browserState = {
    ...browserState,
    selectedMailboxId: mailboxId,
    threads: threadsForMailbox(browserState, mailboxId),
  };

  browserState.selectedThreadId = browserState.threads[0]?.id ?? null;
  browserState.threadDetail = browserState.selectedThreadId
    ? detailForThread(browserState, browserState.selectedThreadId)
    : null;

  return browserState;
}

export async function selectThread(threadId: string): Promise<AppSnapshot> {
  if (isTauri) {
    return invoke<AppSnapshot>("load_thread_detail", { threadId });
  }

  browserState = {
    ...browserState,
    selectedThreadId: threadId,
    threadDetail: detailForThread(browserState, threadId),
  };

  return browserState;
}

export async function refreshMailbox(mailboxId: string): Promise<AppSnapshot> {
  if (isTauri) {
    return invoke<AppSnapshot>("refresh_mailbox", { mailboxId });
  }

  return selectMailbox(mailboxId);
}

export async function archiveThread(threadId: string): Promise<AppSnapshot> {
  if (isTauri) {
    return invoke<AppSnapshot>("archive_thread", { threadId });
  }

  const targetThread = (browserState.allThreads ?? browserState.threads).find(
    (thread) => thread.id === threadId,
  );
  const archiveMailboxId = browserState.mailboxes.find(
    (mailbox) =>
      mailbox.accountId === targetThread?.accountId && mailbox.kind === "archive",
  )?.id;

  browserState = {
    ...browserState,
    allThreads: (browserState.allThreads ?? browserState.threads).map((thread) =>
      thread.id === threadId && archiveMailboxId
        ? { ...thread, mailboxId: archiveMailboxId }
        : thread,
    ),
    selectedThreadId:
      browserState.selectedThreadId === threadId ? null : browserState.selectedThreadId,
    threadDetail: browserState.threadDetail?.id === threadId ? null : browserState.threadDetail,
  };
  browserState.threads = threadsForMailbox(browserState, browserState.selectedMailboxId);
  browserState.selectedThreadId = browserState.threads[0]?.id ?? null;
  browserState.threadDetail = browserState.selectedThreadId
    ? detailForThread(browserState, browserState.selectedThreadId)
    : null;

  return browserState;
}

export async function markSpam(threadId: string): Promise<AppSnapshot> {
  if (isTauri) {
    return invoke<AppSnapshot>("mark_spam", { threadId });
  }

  const targetThread = (browserState.allThreads ?? browserState.threads).find(
    (thread) => thread.id === threadId,
  );
  const spamMailboxId = browserState.mailboxes.find(
    (mailbox) => mailbox.accountId === targetThread?.accountId && mailbox.kind === "spam",
  )?.id;

  browserState = {
    ...browserState,
    allThreads: (browserState.allThreads ?? browserState.threads).map((thread) =>
      thread.id === threadId && spamMailboxId ? { ...thread, mailboxId: spamMailboxId } : thread,
    ),
    selectedThreadId:
      browserState.selectedThreadId === threadId ? null : browserState.selectedThreadId,
    threadDetail: browserState.threadDetail?.id === threadId ? null : browserState.threadDetail,
  };
  browserState.threads = threadsForMailbox(browserState, browserState.selectedMailboxId);
  browserState.selectedThreadId = browserState.threads[0]?.id ?? null;
  browserState.threadDetail = browserState.selectedThreadId
    ? detailForThread(browserState, browserState.selectedThreadId)
    : null;

  return browserState;
}

export async function createDraft(): Promise<DraftDetail> {
  if (isTauri) {
    return invoke<DraftDetail>("create_draft");
  }

  const draft =
    browserState.activeDraft ??
    ({
      envelope: {
        ...mockSnapshot.activeDraft!.envelope,
        id: `draft_${browserState.drafts.length + 1}`,
        to: [],
        subject: "New draft",
        updatedAt: new Date().toISOString(),
      },
      content: {
        ...mockSnapshot.activeDraft!.content,
        htmlBody: "<p></p>",
        textBody: "",
      },
    } satisfies DraftDetail);

  browserState = {
    ...browserState,
    activeDraft: draft,
    drafts: [
      ...browserState.drafts.filter((entry) => entry.id !== draft.envelope.id),
      draft.envelope,
    ],
  };

  return draft;
}

export async function updateDraft(input: DraftUpdateInput): Promise<DraftDetail> {
  if (isTauri) {
    return invoke<DraftDetail>("update_draft", { input });
  }

  const updatedDraft: DraftDetail = {
    envelope: {
      ...(browserState.activeDraft?.envelope ?? mockSnapshot.activeDraft!.envelope),
      id: input.draftId,
      to: input.to,
      cc: input.cc,
      bcc: input.bcc,
      subject: input.subject,
      updatedAt: new Date().toISOString(),
    },
    content: {
      attachments: browserState.activeDraft?.content.attachments ?? [],
      htmlBody: input.htmlBody,
      textBody: input.textBody,
    },
  };

  browserState = {
    ...browserState,
    activeDraft: updatedDraft,
    drafts: browserState.drafts.some((draft) => draft.id === updatedDraft.envelope.id)
      ? browserState.drafts.map((draft) =>
          draft.id === updatedDraft.envelope.id ? updatedDraft.envelope : draft,
        )
      : [...browserState.drafts, updatedDraft.envelope],
  };

  return updatedDraft;
}

export async function sendDraft(draftId: string): Promise<AppSnapshot> {
  if (isTauri) {
    return invoke<AppSnapshot>("send_draft", { draftId });
  }

  browserState = {
    ...browserState,
    activeDraft: null,
    drafts: browserState.drafts.filter((draft) => draft.id !== draftId),
  };

  return browserState;
}
