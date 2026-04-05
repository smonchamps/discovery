import type { AccountSummary, ThreadSummary } from "../types";

interface ThreadListProps {
  accounts: AccountSummary[];
  threads: ThreadSummary[];
  selectedThreadId?: string | null;
  onSelectThread: (threadId: string) => void;
  onRefresh: () => void;
}

function formatTime(value: string) {
  return new Intl.DateTimeFormat("en-GB", {
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(value));
}

export function ThreadList({
  accounts,
  threads,
  selectedThreadId,
  onSelectThread,
  onRefresh,
}: ThreadListProps) {
  return (
    <section className="thread-list">
      <header className="thread-list__header">
        <div>
          <h2>Inbox</h2>
          <p>Sorted by latest activity</p>
        </div>
        <button className="toolbar-button" onClick={onRefresh}>
          Refresh
        </button>
      </header>

      <div className="thread-list__items">
        {threads.map((thread) => {
          const account = accounts.find((entry) => entry.id === thread.accountId);
          return (
            <button
              key={thread.id}
              className={`thread-row ${selectedThreadId === thread.id ? "is-selected" : ""}`}
              onClick={() => onSelectThread(thread.id)}
            >
              <div
                className="thread-row__badge"
                style={{ backgroundColor: account?.color ?? "#2563eb" }}
              >
                {thread.badge}
              </div>
              <div className="thread-row__content">
                <div className="thread-row__meta">
                  <strong>{thread.from.name}</strong>
                  <span>{formatTime(thread.receivedAt)}</span>
                </div>
                <div className="thread-row__subject">
                  {thread.subject}
                  {thread.hasAttachments ? <span className="thread-row__attachment">⎙</span> : null}
                </div>
                <div className="thread-row__snippet">{thread.snippet}</div>
                <div className="thread-row__account">{account?.email}</div>
              </div>
            </button>
          );
        })}
      </div>
    </section>
  );
}
