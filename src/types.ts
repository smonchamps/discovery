export type MailboxKind =
  | "unified_inbox"
  | "inbox"
  | "drafts"
  | "sent"
  | "archive"
  | "spam";

export type SyncState = "idle" | "syncing" | "degraded" | "error";

export interface SyncStatus {
  state: SyncState;
  lastSuccessfulSyncAt?: string | null;
  detail?: string | null;
}

export interface AccountSummary {
  id: string;
  email: string;
  displayName: string;
  color: string;
  status: "connected" | "syncing" | "error";
  unreadCount: number;
}

export interface MailboxRef {
  id: string;
  accountId?: string | null;
  kind: MailboxKind;
  label: string;
  unreadCount: number;
}

export interface Participant {
  name: string;
  email: string;
}

export interface Attachment {
  id: string;
  filename: string;
  mediaType: string;
  sizeLabel: string;
}

export interface MessageView {
  id: string;
  from: Participant;
  to: Participant[];
  sentAt: string;
  htmlBody?: string | null;
  textBody: string;
  attachments: Attachment[];
}

export interface ThreadSummary {
  id: string;
  accountId: string;
  mailboxId: string;
  subject: string;
  snippet: string;
  from: Participant;
  receivedAt: string;
  isUnread: boolean;
  hasAttachments: boolean;
  badge: string;
}

export interface ThreadDetail {
  id: string;
  accountId: string;
  mailboxId: string;
  subject: string;
  participants: Participant[];
  receivedAt: string;
  badge: string;
  messages: MessageView[];
}

export interface DraftEnvelope {
  id: string;
  accountId: string;
  mailboxId: string;
  to: string[];
  cc: string[];
  bcc: string[];
  subject: string;
  updatedAt: string;
}

export interface DraftContent {
  htmlBody?: string | null;
  textBody: string;
  attachments: Attachment[];
}

export interface DraftDetail {
  envelope: DraftEnvelope;
  content: DraftContent;
}

export interface AppSnapshot {
  accounts: AccountSummary[];
  mailboxes: MailboxRef[];
  selectedMailboxId: string;
  syncStatus: Record<string, SyncStatus>;
  allThreads?: ThreadSummary[];
  threads: ThreadSummary[];
  selectedThreadId?: string | null;
  threadDetail?: ThreadDetail | null;
  drafts: DraftEnvelope[];
  activeDraft?: DraftDetail | null;
}

export interface DraftUpdateInput {
  draftId: string;
  to: string[];
  cc: string[];
  bcc: string[];
  subject: string;
  htmlBody?: string | null;
  textBody: string;
}

export type GmailEnrollmentPhase =
  | "idle"
  | "configuration_required"
  | "waiting_for_browser"
  | "waiting_for_callback"
  | "exchanging_code"
  | "success"
  | "error";

export interface GmailEnrollmentStatus {
  phase: GmailEnrollmentPhase;
  message: string;
  authorizeUrl?: string | null;
  callbackUrl?: string | null;
  enrolledEmail?: string | null;
}
