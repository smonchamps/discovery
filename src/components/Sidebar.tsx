import type {
  AccountSummary,
  GmailEnrollmentStatus,
  MailboxRef,
  SyncStatus,
} from "../types";

interface SidebarProps {
  accounts: AccountSummary[];
  mailboxes: MailboxRef[];
  selectedMailboxId: string;
  syncStatus: Record<string, SyncStatus>;
  enrollmentStatus: GmailEnrollmentStatus | null;
  onSelectMailbox: (mailboxId: string) => void;
  onConnectGmail: () => void;
}

const mailboxIcons: Record<string, string> = {
  unified_inbox: "U",
  inbox: "I",
  drafts: "D",
  sent: "S",
  archive: "A",
  spam: "!",
};

export function Sidebar({
  accounts,
  mailboxes,
  selectedMailboxId,
  syncStatus,
  enrollmentStatus,
  onSelectMailbox,
  onConnectGmail,
}: SidebarProps) {
  const groupedMailboxes = accounts.map((account) => ({
    account,
    mailboxes: mailboxes.filter((mailbox) => mailbox.accountId === account.id),
  }));

  const unifiedInbox = mailboxes.find((mailbox) => mailbox.kind === "unified_inbox");

  return (
    <aside className="sidebar">
      <div className="sidebar__brand">
        <div className="sidebar__logo">D</div>
        <div>
          <strong>Discovery</strong>
          <p>Focused Gmail desktop client</p>
        </div>
      </div>

      <button className="sidebar__connect" onClick={onConnectGmail}>
        Connect Gmail
      </button>
      {enrollmentStatus ? (
        <div className={`sidebar__status sidebar__status--${enrollmentStatus.phase}`}>
          <strong>{enrollmentStatus.phase.replaceAll("_", " ")}</strong>
          <p>{enrollmentStatus.message}</p>
          {enrollmentStatus.phase === "configuration_required" ? (
            <code>
              DISCOVERY_GOOGLE_CLIENT_ID
              <br />
              DISCOVERY_GOOGLE_CLIENT_SECRET
            </code>
          ) : null}
        </div>
      ) : null}

      {unifiedInbox ? (
        <button
          className={`sidebar__mailbox ${selectedMailboxId === unifiedInbox.id ? "is-active" : ""}`}
          onClick={() => onSelectMailbox(unifiedInbox.id)}
        >
          <span>{mailboxIcons[unifiedInbox.kind]}</span>
          <span>{unifiedInbox.label}</span>
          <span>{unifiedInbox.unreadCount}</span>
        </button>
      ) : null}

      <div className="sidebar__section-label">Accounts</div>
      {groupedMailboxes.map(({ account, mailboxes: accountMailboxes }) => (
        <section key={account.id} className="sidebar__account">
          <header className="sidebar__account-header">
            <div className="sidebar__avatar" style={{ backgroundColor: account.color }}>
              {account.displayName[0]}
            </div>
            <div>
              <strong>{account.email}</strong>
              <p>{syncStatus[account.id]?.detail ?? "Waiting for first sync."}</p>
            </div>
          </header>
          <div className="sidebar__mailbox-list">
            {accountMailboxes.map((mailbox) => (
              <button
                key={mailbox.id}
                className={`sidebar__mailbox ${selectedMailboxId === mailbox.id ? "is-active" : ""}`}
                onClick={() => onSelectMailbox(mailbox.id)}
              >
                <span>{mailboxIcons[mailbox.kind]}</span>
                <span>{mailbox.label}</span>
                <span>{mailbox.unreadCount > 0 ? mailbox.unreadCount : ""}</span>
              </button>
            ))}
          </div>
        </section>
      ))}
    </aside>
  );
}
