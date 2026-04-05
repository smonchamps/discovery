import type { ThreadDetail } from "../types";

interface ReaderPaneProps {
  threadDetail?: ThreadDetail | null;
  onArchive: () => void;
  onSpam: () => void;
}

function formatDateTime(value: string) {
  return new Intl.DateTimeFormat("en-GB", {
    day: "2-digit",
    month: "short",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(value));
}

function buildMessageDocument(htmlBody: string) {
  return `<!doctype html>
<html>
  <head>
    <meta charset="utf-8" />
    <base target="_blank" />
    <style>
      html, body {
        margin: 0;
        padding: 0;
        background: #ffffff;
      }
    </style>
  </head>
  <body>${htmlBody}</body>
</html>`;
}

export function ReaderPane({ threadDetail, onArchive, onSpam }: ReaderPaneProps) {
  if (!threadDetail) {
    return (
      <section className="reader reader--empty">
        <div>
          <h2>Select a message</h2>
          <p>Discovery keeps reading focused by showing one thread at a time.</p>
        </div>
      </section>
    );
  }

  const latestMessage = threadDetail.messages[threadDetail.messages.length - 1];

  return (
    <section className="reader">
      <header className="reader__header">
        <div>
          <h1>{threadDetail.subject}</h1>
          <p>{threadDetail.participants.map((participant) => participant.email).join(" | ")}</p>
        </div>
        <div className="reader__toolbar">
          <button className="toolbar-button" onClick={onArchive}>
            Archive
          </button>
          <button className="toolbar-button toolbar-button--danger" onClick={onSpam}>
            Spam
          </button>
        </div>
      </header>

      {latestMessage ? (
        <article className="reader__message">
          <div className="reader__message-meta">
            <div className="reader__sender-badge">{threadDetail.badge}</div>
            <div>
              <strong>{latestMessage.from.name}</strong>
              <p>{latestMessage.from.email}</p>
            </div>
            <time>{formatDateTime(latestMessage.sentAt)}</time>
          </div>

          {latestMessage.htmlBody ? (
            <iframe
              className="reader__html-frame"
              sandbox="allow-popups allow-popups-to-escape-sandbox"
              srcDoc={buildMessageDocument(latestMessage.htmlBody)}
              title={threadDetail.subject}
            />
          ) : (
            <pre className="reader__text">{latestMessage.textBody}</pre>
          )}
        </article>
      ) : null}
    </section>
  );
}
