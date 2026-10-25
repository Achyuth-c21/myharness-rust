/**
 * Memory management (Phase 7; docs/11 §5, docs/screens/settings.md §Cowork:
 * "global memory toggle and memory list. Memory content is personal and
 * local-only"). The toggle is the `memoryEnabled` capability of the applied
 * profile: the effective value is resolved by the bridge per call and a
 * disabled state refuses every store operation with `MEMORY_DISABLED` — this
 * panel shows the state and explains where it is set, it does not duplicate
 * the config write path. Delete and reset require explicit confirmation
 * (docs/features/settings.md: destructive actions require confirmation).
 */
import { useEffect, useState } from "react";
import {
  chat,
  type BridgeError,
  type MemoryEntryRecord,
} from "../chat/client";

function MemoryPanel() {
  const [entries, setEntries] = useState<MemoryEntryRecord[] | null>(null);
  const [enabled, setEnabled] = useState<boolean | null>(null);
  const [baselineChanged, setBaselineChanged] = useState(false);
  const [error, setError] = useState<BridgeError | null>(null);
  const [draft, setDraft] = useState("");
  const [editing, setEditing] = useState<{ id: string; content: string } | null>(null);
  const [confirmingDelete, setConfirmingDelete] = useState<string | null>(null);
  const [confirmingReset, setConfirmingReset] = useState(false);
  const [busy, setBusy] = useState(false);

  const load = async () => {
    const [overview, list] = await Promise.all([
      chat({ type: "memoryOverview" }),
      chat({ type: "listMemoryEntries" }),
    ]);
    if (overview.type === "memoryOverview") {
      setEnabled(overview.policy.memoryEnabled);
      setBaselineChanged(overview.baselineChanged);
    } else if (overview.type === "error") {
      setError(overview.error);
    }
    if (list.type === "memoryEntries") {
      setEntries(list.entries);
    } else if (list.type === "error") {
      if (list.error.code === "MEMORY_DISABLED") setEntries([]);
      else setError(list.error);
    }
  };

  useEffect(() => {
    void load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const message = (err: BridgeError): string =>
    err.code === "BRIDGE_UNAVAILABLE"
      ? "Memory is available in the desktop app."
      : err.message;

  const create = async () => {
    if (busy || !draft.trim()) return;
    setBusy(true);
    setError(null);
    try {
      const response = await chat({ type: "createMemoryEntry", content: draft });
      if (response.type === "memoryEntry") {
        setDraft("");
        await load();
      } else if (response.type === "error") setError(response.error);
    } finally {
      setBusy(false);
    }
  };

  const saveEdit = async () => {
    if (busy || !editing) return;
    setBusy(true);
    setError(null);
    try {
      const response = await chat({
        type: "updateMemoryEntry",
        id: editing.id,
        content: editing.content,
      });
      if (response.type === "memoryEntry") {
        setEditing(null);
        await load();
      } else if (response.type === "error") setError(response.error);
    } finally {
      setBusy(false);
    }
  };

  const destroy = async (id: string) => {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      const response = await chat({ type: "deleteMemoryEntry", id });
      if (response.type === "ack") {
        setConfirmingDelete(null);
        await load();
      } else if (response.type === "error") setError(response.error);
    } finally {
      setBusy(false);
    }
  };

  const reset = async () => {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      const response = await chat({ type: "resetMemory" });
      if (response.type === "memoryReset") {
        setConfirmingReset(false);
        await load();
      } else if (response.type === "error") setError(response.error);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="settings-section-stack">
      <p className="settings-note">
        Memory is personal and local-only. It is stored as Markdown entries on
        this device and is never part of retention deletion.
      </p>
      <div className="settings-row">
        <span>
          <strong>Memory {enabled === null ? "…" : enabled ? "is on" : "is off"}</strong>
          <span className="settings-note">
            {" "}
            — set by the applied profile's Memory capability in the setup
            window; a disabled state refuses all memory operations.
          </span>
        </span>
      </div>
      {baselineChanged ? (
        <div className="info-banner" role="status">
          <span>
            The memory files were changed outside the app; the index was
            reconciled from the files on disk.
          </span>
        </div>
      ) : null}
      {error ? (
        <div className="composer-error" role="alert">
          <span className="error-code">{error.code}</span>
          <span>{message(error)}</span>
        </div>
      ) : null}

      {enabled === false ? (
        <p className="settings-note">
          Memory is disabled by the applied configuration. Enable the Memory
          capability in the setup window to manage entries here.
        </p>
      ) : (
        <>
          <div className="memory-add">
            <textarea
              placeholder="Add a memory entry (Markdown)…"
              value={draft}
              rows={2}
              onChange={(event) => setDraft(event.target.value)}
              aria-label="New memory entry"
            />
            <button
              type="button"
              className="btn-primary"
              disabled={!draft.trim() || busy}
              onClick={() => void create()}
            >
              Add
            </button>
          </div>
          <div className="memory-list">
            {entries === null ? (
              <p className="settings-note" aria-busy="true" />
            ) : entries.length === 0 ? (
              <p className="settings-note">No memory entries yet.</p>
            ) : (
              entries.map((entry) => (
                <div key={entry.id} className="memory-entry">
                  {editing?.id === entry.id ? (
                    <>
                      <textarea
                        value={editing.content}
                        rows={3}
                        aria-label="Edit memory entry"
                        onChange={(event) =>
                          setEditing({ ...editing, content: event.target.value })
                        }
                      />
                      <span className="memory-entry-actions">
                        <button
                          type="button"
                          className="btn-secondary"
                          onClick={() => setEditing(null)}
                        >
                          Cancel
                        </button>
                        <button
                          type="button"
                          className="btn-primary"
                          disabled={busy || !editing.content.trim()}
                          onClick={() => void saveEdit()}
                        >
                          Save
                        </button>
                      </span>
                    </>
                  ) : (
                    <>
                      <span className="memory-entry-content">{entry.content}</span>
                      <span className="memory-entry-actions">
                        {confirmingDelete === entry.id ? (
                          <>
                            <span className="settings-note">Delete this entry?</span>
                            <button
                              type="button"
                              className="text-btn"
                              onClick={() => setConfirmingDelete(null)}
                            >
                              Cancel
                            </button>
                            <button
                              type="button"
                              className="btn-danger"
                              disabled={busy}
                              onClick={() => void destroy(entry.id)}
                            >
                              Delete
                            </button>
                          </>
                        ) : (
                          <>
                            <button
                              type="button"
                              className="text-btn"
                              onClick={() => setEditing({ id: entry.id, content: entry.content })}
                            >
                              Edit
                            </button>
                            <button
                              type="button"
                              className="text-btn"
                              onClick={() => setConfirmingDelete(entry.id)}
                            >
                              Delete
                            </button>
                          </>
                        )}
                      </span>
                    </>
                  )}
                </div>
              ))
            )}
          </div>
          {confirmingReset ? (
            <div className="confirm-inline" role="alert">
              <span>Delete every memory entry? This cannot be undone.</span>
              <button type="button" className="text-btn" onClick={() => setConfirmingReset(false)}>
                Cancel
              </button>
              <button type="button" className="btn-danger" disabled={busy} onClick={() => void reset()}>
                Reset memory
              </button>
            </div>
          ) : (
            <button
              type="button"
              className="btn-danger"
              disabled={busy || (entries?.length ?? 0) === 0}
              onClick={() => setConfirmingReset(true)}
            >
              Reset memory…
            </button>
          )}
        </>
      )}
    </div>
  );
}

export { MemoryPanel };
