/**
 * Task detail / transcript — `UI-SCREEN-03`, route `/cowork/<session-id>`
 * (docs/screens/task-detail.md; docs/features/chat-and-composer.md).
 *
 * Phase 4 surface:
 * - opens the stored session through the bridge; an interrupted live turn is
 *   recovered by the core before the transcript is read (`TEST-SESSION-002`);
 * - streams a follow-up through `chat://turn-event`, with Send replaced by Stop
 *   while the turn runs (`TEST-SESSION-001` stop clause);
 * - keeps the composer draft in renderer storage on any failure and clears it
 *   only when a turn completes (`TEST-SESSION-009`);
 * - shows provider failures verbatim, inline, with no retry button — retry was
 *   never observed (docs/14 §9);
 * - renders reasoning behind a disclosure, partial output as stopped, and
 *   preserved non-`end_turn` stop reasons as a small status line.
 *
 * Not in this phase: tool-call blocks (tools arrive with MCP), the right
 * Progress/Context pane, attachments, and the model picker.
 */
import { useCallback, useEffect, useRef, useState } from "react";
import { useLocation, useNavigate, useParams } from "react-router-dom";
import {
  chat,
  emitSessionsChanged,
  onTurnEvent,
  SESSIONS_CHANGED,
  plainText,
  reasoningText,
  type BridgeError,
  type SessionView,
  type TurnSummary,
} from "../chat/client";
import {
  clearDraft,
  loadDraft,
  saveDraft,
  sessionDraftKey,
} from "../chat/drafts";
import { LoadingShell } from "../components/shells";
import { modelLabel } from "../chat/format";

function ArrowUpIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path
        d="M8 12.5V4M8 4 4.6 7.4M8 4l3.4 3.4"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

function StopIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <rect x="4" y="4" width="8" height="8" rx="1.5" fill="currentColor" />
    </svg>
  );
}

function ChevronDown() {
  return (
    <svg width="12" height="12" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path d="m4 6.5 4 4 4-4" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

function WorkingMark() {
  const rays = Array.from({ length: 8 }, (_, i) => {
    const a = (i * Math.PI * 2) / 8;
    return (
      <line
        key={i}
        x1={12 + Math.cos(a) * 3.2}
        y1={12 + Math.sin(a) * 3.2}
        x2={12 + Math.cos(a) * 9.6}
        y2={12 + Math.sin(a) * 9.6}
        stroke="currentColor"
        strokeWidth="2.2"
        strokeLinecap="round"
      />
    );
  });
  return (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" className="starburst" aria-hidden="true">
      {rays}
    </svg>
  );
}

function CopyButton({ text }: { text: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <button
      type="button"
      className="copy-btn"
      onClick={async () => {
        try {
          await navigator.clipboard.writeText(text);
          setCopied(true);
          window.setTimeout(() => setCopied(false), 1500);
        } catch {
          // No clipboard permission: do not claim a copy that did not happen.
        }
      }}
    >
      {copied ? "Copied" : "Copy"}
    </button>
  );
}

function AssistantText({ text }: { text: string }) {
  return (
    <>
      {text.split(/\n{2,}/).map((paragraph, index) => (
        <p key={index} className="turn-text">
          {paragraph}
        </p>
      ))}
    </>
  );
}

function TurnBlock({ turn }: { turn: TurnSummary }) {
  const text = plainText(turn.content);
  const reasoning = reasoningText(turn.content);

  if (turn.role === "user") {
    return (
      <div className="turn turn-user">
        <div className="user-bubble">{text}</div>
      </div>
    );
  }

  const stopNote =
    turn.status === "complete" && turn.stopReason && turn.stopReason !== "end_turn"
      ? turn.stopReason
      : null;

  return (
    <div className="turn turn-assistant">
      {reasoning ? (
        <details className="turn-reasoning">
          <summary>Thought process</summary>
          <p>{reasoning}</p>
        </details>
      ) : null}
      {text ? <AssistantText text={text} /> : null}
      {turn.status === "partial" ? <div className="turn-status">Stopped</div> : null}
      {turn.status === "error" && turn.error ? (
        <div className="turn-error" role="alert">
          {turn.error}
        </div>
      ) : null}
      {!text && turn.status === "error" && !turn.error ? (
        <div className="turn-status">No output</div>
      ) : null}
      {stopNote ? <div className="turn-meta">Stop reason: {stopNote}</div> : null}
      {text ? <CopyButton text={text} /> : null}
    </div>
  );
}

export function CoworkSessionScreen() {
  const { sessionId = "" } = useParams();
  // Keying by session id gives each task its own composer, draft, and stream
  // state. Without this, navigating from task A to task B while A streams
  // would show A's Stop button and live text in B's composer, and A's
  // completion would overwrite B's draft.
  return <SessionScreen key={sessionId} sessionId={sessionId} />;
}

function SessionScreen({ sessionId }: { sessionId: string }) {
  const navigate = useNavigate();
  const location = useLocation();
  const pending = location.state as
    | { pendingText?: string; originKey?: string }
    | null;

  const [view, setView] = useState<SessionView | null>(null);
  const [loadError, setLoadError] = useState<BridgeError | null>(null);
  const [loading, setLoading] = useState(true);
  const [draft, setDraft] = useState("");
  const [streaming, setStreaming] = useState(false);
  const [liveText, setLiveText] = useState("");
  const [sendError, setSendError] = useState<{
    error: BridgeError;
    recorded: boolean;
  } | null>(null);
  const [actionError, setActionError] = useState<BridgeError | null>(null);
  const [menuOpen, setMenuOpen] = useState(false);
  const [renaming, setRenaming] = useState(false);
  const [renameValue, setRenameValue] = useState("");
  const [confirmDelete, setConfirmDelete] = useState(false);
  const taRef = useRef<HTMLTextAreaElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const bodyRef = useRef<HTMLDivElement>(null);
  const autoSent = useRef(false);
  const sendingRef = useRef(false);
  const stickToBottom = useRef(true);

  const draftKey = sessionDraftKey(sessionId);

  const load = useCallback(async () => {
    const response = await chat({ type: "openSession", id: sessionId });
    if (response.type === "session") {
      // A live marker means a turn is still running in the core (open skips
      // crash recovery for it). Reflect that in the composer: Stop, not Send.
      const live = response.session.turns.some(
        (turn) =>
          turn.role === "assistant" &&
          turn.status === "partial" &&
          turn.error === null,
      );
      setStreaming(live);
      setView(response.session);
      setLoadError(null);
    } else if (response.type === "error") {
      setView(null);
      setLoadError(response.error);
    }
    setLoading(false);
  }, [sessionId]);

  // Route change: reset per-session state and load the transcript.
  useEffect(() => {
    setView(null);
    setLoadError(null);
    setLoading(true);
    setDraft(loadDraft(sessionDraftKey(sessionId)));
    setStreaming(false);
    setLiveText("");
    setSendError(null);
    setActionError(null);
    setMenuOpen(false);
    setRenaming(false);
    setConfirmDelete(false);
    autoSent.current = false;
    void load();
  }, [sessionId, load]);

  // The title menu closes on an outside click or Escape.
  useEffect(() => {
    if (!menuOpen) return;
    const onDown = (event: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(event.target as Node)) {
        setMenuOpen(false);
      }
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setMenuOpen(false);
        setConfirmDelete(false);
      }
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [menuOpen]);

  // Stream deltas for this session only.
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let cancelled = false;
    void onTurnEvent((sid, event) => {
      if (sid !== sessionId) return;
      if (event.type === "text_delta") {
        setLiveText((previous) => previous + event.text);
      }
    }).then((stop) => {
      if (cancelled) stop();
      else unlisten = stop;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [sessionId]);

  const send = useCallback(
    async (text: string) => {
      // A ref guard makes a double Enter (or Stop racing Send) one turn.
      if (!text.trim() || sendingRef.current) return;
      sendingRef.current = true;
      setStreaming(true);
      setSendError(null);
      setActionError(null);
      setLiveText("");
      try {
        const response = await chat({ type: "sendTurn", sessionId, text });
        if (response.type === "turn") {
          // The turn completed: the draft's job is done.
          saveDraft(draftKey, "");
          setDraft("");
          if (pending?.originKey) clearDraft(pending.originKey);
          emitSessionsChanged();
          setLiveText("");
          await load();
        } else if (response.type === "error") {
          // A recoverable failure preserves the draft (docs/features/
          // chat-and-composer.md §Errors). The user turn may already be in the
          // transcript; the banner says which happened, and the text stays in
          // the composer so nothing the user typed is lost.
          if (response.error.code !== "TURN_CANCELLED") {
            setSendError({
              error: response.error,
              recorded: response.userTurnRecorded,
            });
          }
          setDraft(text);
          saveDraft(draftKey, text);
          setLiveText("");
          if (response.userTurnRecorded) {
            emitSessionsChanged();
            await load();
          }
        }
      } finally {
        sendingRef.current = false;
        setStreaming(false);
      }
    },
    [draftKey, load, pending?.originKey, sessionId],
  );

  // A turn that was running when this screen opened finishes in the core even
  // if its original component is gone. The send emits SESSIONS_CHANGED on
  // completion; reload the transcript (unless this screen is the sender, whose
  // own send path already reloads).
  useEffect(() => {
    const onChanged = () => {
      if (!sendingRef.current) void load();
    };
    window.addEventListener(SESSIONS_CHANGED, onChanged);
    return () => window.removeEventListener(SESSIONS_CHANGED, onChanged);
  }, [load]);

  // Follow the stream unless the reader scrolled up to read earlier turns.
  useEffect(() => {
    const body = bodyRef.current;
    if (body && stickToBottom.current) body.scrollTop = body.scrollHeight;
  }, [liveText, streaming, view?.turns.length]);

  // A send handed over from /new runs once, after the transcript is open.
  useEffect(() => {
    if (!view || view.session.id !== sessionId || !pending?.pendingText || autoSent.current) {
      return;
    }
    autoSent.current = true;
    // Drop the navigation state so a reload cannot resend the same text.
    navigate(location.pathname, { replace: true, state: null });
    // The session composer now owns the text. Move it before the first await
    // so a crash between navigation and the send cannot lose it.
    setDraft(pending.pendingText);
    saveDraft(draftKey, pending.pendingText);
    if (pending.originKey) clearDraft(pending.originKey);
    void send(pending.pendingText);
  }, [view, pending, send, navigate, location.pathname, sessionId]);

  const stop = async () => {
    const response = await chat({ type: "stopTurn", sessionId });
    if (response.type === "error") setActionError(response.error);
  };

  const updateDraft = (value: string) => {
    setDraft(value);
    saveDraft(draftKey, value);
  };

  const autoGrow = (element: HTMLTextAreaElement) => {
    element.style.height = "auto";
    element.style.height = `${Math.min(element.scrollHeight, 200)}px`;
  };

  const onKeyDown = (event: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      if (streaming) return;
      if (!(event.nativeEvent as KeyboardEvent).isComposing) void send(draft);
    }
  };

  const rename = async () => {
    const title = renameValue.trim();
    if (!title) return;
    const response = await chat({ type: "renameSession", id: sessionId, title });
    if (response.type === "ack") {
      setRenaming(false);
      emitSessionsChanged();
      await load();
    } else if (response.type === "error") {
      setActionError(response.error);
    }
  };

  const archive = async () => {
    const response = await chat({ type: "archiveSession", id: sessionId, archived: true });
    if (response.type === "ack") {
      emitSessionsChanged();
      navigate("/new");
    } else if (response.type === "error") {
      setActionError(response.error);
      setMenuOpen(false);
    }
  };

  const remove = async () => {
    const response = await chat({ type: "deleteSession", id: sessionId });
    if (response.type === "ack") {
      clearDraft(draftKey);
      emitSessionsChanged();
      navigate("/new");
    } else if (response.type === "error") {
      setActionError(response.error);
      setConfirmDelete(false);
    }
  };

  if (loading || (view && view.session.id !== sessionId)) {
    return <LoadingShell label="Opening task…" />;
  }

  if (loadError || !view) {
    const preview = loadError?.code === "BRIDGE_UNAVAILABLE";
    return (
      <div className="shell empty" role="alert">
        <h2>{preview ? "Chat is available in the desktop app" : "This task could not be opened"}</h2>
        <p>
          {preview
            ? "This browser preview has no local session store."
            : loadError?.message ?? "The session is not available."}
        </p>
        <button type="button" className="text-btn" onClick={() => navigate("/new")}>
          Back to New
        </button>
      </div>
    );
  }

  const session = view.session;
  const extra = session.extra as { providerId?: unknown } | null;
  const label = modelLabel(extra?.providerId, session.model);
  const hasDraft = draft.trim().length > 0;

  return (
    <div className="session-screen">
      <header className="session-head">
        <div className="session-title-wrap" ref={menuRef}>
          {renaming ? (
            <form
              className="rename-form"
              onSubmit={(event) => {
                event.preventDefault();
                void rename();
              }}
            >
              <input
                className="rename-input"
                aria-label="Task title"
                value={renameValue}
                autoFocus
                onChange={(event) => setRenameValue(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Escape") {
                    event.preventDefault();
                    setRenaming(false);
                  }
                }}
              />
              <button type="submit" className="text-btn" disabled={!renameValue.trim()}>
                Save
              </button>
              <button type="button" className="text-btn" onClick={() => setRenaming(false)}>
                Cancel
              </button>
            </form>
          ) : (
            <>
              <button
                type="button"
                className="session-title"
                aria-expanded={menuOpen}
                aria-haspopup="menu"
                onClick={() => setMenuOpen((open) => !open)}
              >
                {session.title}
                <ChevronDown />
              </button>
              {menuOpen ? (
                <div className="session-menu" role="menu">
                  <button
                    type="button"
                    role="menuitem"
                    className="account-item"
                    onClick={() => {
                      setRenameValue(session.title);
                      setRenaming(true);
                      setMenuOpen(false);
                    }}
                  >
                    Rename
                  </button>
                  <button
                    type="button"
                    role="menuitem"
                    className="account-item"
                    disabled={streaming}
                    title={streaming ? "Stop the running turn first" : undefined}
                    onClick={() => void archive()}
                  >
                    Archive
                  </button>
                  {confirmDelete ? (
                    <div className="session-confirm">
                      <span>Delete this task?</span>
                      <button type="button" className="text-btn" onClick={() => void remove()}>
                        Delete
                      </button>
                      <button
                        type="button"
                        className="text-btn"
                        onClick={() => setConfirmDelete(false)}
                      >
                        Cancel
                      </button>
                    </div>
                  ) : (
                    <button
                      type="button"
                      role="menuitem"
                      className="account-item"
                      disabled={streaming}
                      title={streaming ? "Stop the running turn first" : undefined}
                      onClick={() => setConfirmDelete(true)}
                    >
                      Delete…
                    </button>
                  )}
                </div>
              ) : null}
            </>
          )}
        </div>
        <span className="session-model-label" title="The model is fixed for this task">
          {label}
        </span>
      </header>

      <div
        className="session-body"
        ref={bodyRef}
        onScroll={(event) => {
          const body = event.currentTarget;
          stickToBottom.current =
            body.scrollHeight - body.scrollTop - body.clientHeight < 80;
        }}
      >
        <div className="transcript" role="log" aria-live="polite" aria-label="Transcript">
          {view.turns
            .filter(
              (turn) =>
                !(
                  turn.role === "assistant" &&
                  turn.status === "partial" &&
                  turn.error === null
                ),
            )
            .map((turn) => (
              <TurnBlock key={turn.id} turn={turn} />
            ))}
          {streaming ? (
            <div className="turn turn-assistant">
              {liveText ? (
                <AssistantText text={liveText} />
              ) : (
                <div className="working">
                  <WorkingMark />
                  <span>Working on it…</span>
                </div>
              )}
            </div>
          ) : null}
        </div>
      </div>

      <div className="session-composer-wrap">
        {actionError ? (
          <div className="composer-error" role="alert">
            <span className="error-code">{actionError.code}</span>
            <span>{actionError.message}</span>
          </div>
        ) : null}
        {sendError ? (
          <div className="composer-error" role="alert">
            <span className="error-code">{sendError.error.code}</span>
            <span>
              {sendError.error.message}
              <span className="error-note">
                {sendError.recorded
                  ? " The message is in the transcript; the reply failed. Your draft is kept."
                  : " The message was not sent. Your draft is kept."}
              </span>
            </span>
          </div>
        ) : null}
        <div className="composer-card">
          <textarea
            ref={taRef}
            id="composer"
            className="composer-input"
            placeholder="Write a message…"
            value={draft}
            rows={1}
            aria-label="Message"
            onChange={(event) => {
              updateDraft(event.target.value);
              autoGrow(event.target);
            }}
            onKeyDown={onKeyDown}
          />
          <div className="composer-toolbar">
            <button
              type="button"
              className="plus-btn"
              aria-label="Add"
              disabled
              title="Attachments arrive in a later phase"
            >
              +
            </button>
            <span className="composer-spacer" />
            <span className="composer-model" title="The model is fixed for this task">
              {label}
            </span>
            {streaming ? (
              <button
                type="button"
                className="send-btn stop"
                aria-label="Stop"
                title="Stop this turn"
                onClick={() => void stop()}
              >
                <StopIcon />
              </button>
            ) : (
              <button
                type="button"
                className={`send-btn${hasDraft ? " primary" : ""}`}
                aria-label="Send"
                disabled={!hasDraft}
                onClick={() => void send(draft)}
              >
                <ArrowUpIcon />
              </button>
            )}
          </div>
        </div>
        <p className="session-disclaimer">
          Claude is AI and can make mistakes. Please double-check responses.
        </p>
      </div>
    </div>
  );
}
