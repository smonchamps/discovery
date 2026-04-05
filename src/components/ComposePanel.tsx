import { useEffect, useEffectEvent, useState } from "react";
import type { DraftDetail } from "../types";

interface ComposePanelProps {
  draft?: DraftDetail | null;
  onCreateDraft: () => void;
  onSaveDraft: (draft: DraftDetail) => void;
  onSendDraft: (draftId: string) => void;
}

export function ComposePanel({
  draft,
  onCreateDraft,
  onSaveDraft,
  onSendDraft,
}: ComposePanelProps) {
  const [localDraft, setLocalDraft] = useState<DraftDetail | null>(draft ?? null);
  const saveDraft = useEffectEvent((nextDraft: DraftDetail) => {
    onSaveDraft(nextDraft);
  });

  useEffect(() => {
    setLocalDraft(draft ?? null);
  }, [draft]);

  useEffect(() => {
    if (!localDraft) {
      return;
    }

    const timeoutId = window.setTimeout(() => {
      saveDraft(localDraft);
    }, 600);

    return () => window.clearTimeout(timeoutId);
  }, [localDraft, saveDraft]);

  if (!localDraft) {
    return (
      <aside className="compose compose--empty">
        <button className="compose__new" onClick={onCreateDraft}>
          New draft
        </button>
        <p>Autosaved rich drafts appear here.</p>
      </aside>
    );
  }

  return (
    <aside className="compose">
      <header className="compose__header">
        <div>
          <strong>Draft</strong>
          <p>Autosaved locally and ready for Gmail sync</p>
        </div>
        <button
          className="toolbar-button"
          onClick={() => onSendDraft(localDraft.envelope.id)}
        >
          Send
        </button>
      </header>

      <label className="compose__field">
        <span>To</span>
        <input
          value={localDraft.envelope.to.join(", ")}
          onChange={(event) =>
            setLocalDraft({
              ...localDraft,
              envelope: {
                ...localDraft.envelope,
                to: event.target.value
                  .split(",")
                  .map((value) => value.trim())
                  .filter(Boolean),
              },
            })
          }
        />
      </label>

      <label className="compose__field">
        <span>Subject</span>
        <input
          value={localDraft.envelope.subject}
          onChange={(event) =>
            setLocalDraft({
              ...localDraft,
              envelope: {
                ...localDraft.envelope,
                subject: event.target.value,
              },
            })
          }
        />
      </label>

      <label className="compose__editor">
        <span>Body</span>
        <textarea
          value={localDraft.content.textBody}
          onChange={(event) =>
            setLocalDraft({
              ...localDraft,
              content: {
                ...localDraft.content,
                textBody: event.target.value,
                htmlBody: `<p>${event.target.value}</p>`,
              },
            })
          }
        />
      </label>
    </aside>
  );
}
