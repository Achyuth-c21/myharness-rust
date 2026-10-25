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
 * Phase 5 adds tool-call/result rows, the approval modal, the per-session tool
 * picker, and the skill picker. Not in this phase: the right Progress/Context
 * pane, attachments, the model picker, and system-prompt assembly.
 */
import { useCallback, useEffect, useRef, useState } from "react";
import { useLocation, useNavigate, useParams } from "react-router-dom";
import {
  chat,
  emitSessionsChanged,
  onToolApproval,
  onTurnEvent,
  SESSIONS_CHANGED,
  plainText,
  reasoningText,
  toolResults,
  toolUses,
  type ApprovalScope,
  type BridgeError,
  type SessionView,
  type SkillView,
  type ToolApprovalEvent,
  type ToolDescriptor,
  type TurnSummary,
} from "../chat/client";
import {
  clearDraft,
  loadDraft,
  saveDraft,
  sessionDraftKey,
} from "../chat/drafts";
import { LoadingShell } from "../components/shells";
import { useFocusTrap } from "../components/focusTrap";
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

  if (turn.role === "tool") {
    return <ToolResultBlock turn={turn} />;
  }

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
      <ToolUseBlocks turn={turn} />
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

function ToolUseBlocks({ turn }: { turn: TurnSummary }) {
  const uses = toolUses(turn.content);
  if (uses.length === 0) return null;
  return (
    <div className="tool-uses">
      {uses.map((use) => (
        <details key={use.id} className="tool-use">
          <summary>
            Tool call: <span className="tool-name">{use.name}</span>
          </summary>
          <pre>{JSON.stringify(use.input, null, 2)}</pre>
        </details>
      ))}
    </div>
  );
}

function ToolResultBlock({ turn }: { turn: TurnSummary }) {
  const results = toolResults(turn.content);
  if (results.length === 0) {
    const text = plainText(turn.content);
    return text ? <div className="turn turn-assistant"><AssistantText text={text} /></div> : null;
  }
  return (
    <div className="turn turn-tool">
      {results.map((result) => {
        const text = result.parts
          .filter((part): part is typeof part & { type: "text"; text: string } =>
            part.type === "text" && typeof part.text === "string",
          )
          .map((part) => part.text)
          .join("");
        const media = result.parts.filter(
          (part) => part.type === "image" || part.type === "document",
        ).length;
        return (
          <div
            key={result.toolUseId}
            className={`tool-result${result.isError ? " tool-result-error" : ""}`}
          >
            <div className="tool-result-label">
              {result.isError ? "Tool error" : "Tool result"}
              <span className="tool-result-id">{result.toolUseId}</span>
            </div>
            {text ? <pre>{text}</pre> : null}
            {media > 0 ? (
              <div className="tool-result-media">
                {media} media {media === 1 ? "part" : "parts"} preserved in the transcript
              </div>
            ) : null}
          </div>
        );
      })}
    </div>
  );
}

function permissionModeLabel(value: string | null): string {
  switch (value) {
    case "default":
      return "Default permissions";
    case "acceptEdits":
      return "Accept edits";
    case "plan":
      return "Plan";
    case "bypass":
      return "Bypass permissions";
    default:
      // A corrupt stored mode is never presented as Default; the backend also
      // refuses to send until it is repaired.
      return "Unknown permissions";
  }
}

function folderName(path: string | null): string {
  if (!path) return "No folder";
  const trimmed = path.replace(/[\\/]+$/, "");
  if (!trimmed) return path;
  const parts = trimmed.split(/[\\/]/);
  return parts[parts.length - 1] || trimmed;
}

const SCOPE_LABEL: Record<ApprovalScope, string> = {
  once: "Allow once",
  session: "Allow for this session",
  persistent: "Always allow (persistent)",
};

function ApprovalDialog({
  request,
  scope,
  error,
  onScope,
  onAllow,
  onDeny,
  dialogRef,
}: {
  request: ToolApprovalEvent;
  scope: ApprovalScope;
  error: BridgeError | null;
  onScope: (scope: ApprovalScope) => void;
  onAllow: () => void;
  onDeny: () => void;
  dialogRef: React.RefObject<HTMLDivElement | null>;
}) {
  const preview = JSON.stringify(request.argumentsPreview, null, 2) ?? "";
  return (
    <div className="overlay-backdrop">
      <div
        className="approval-modal"
        role="dialog"
        aria-modal="true"
        aria-label="Tool approval"
        ref={dialogRef as React.Ref<HTMLDivElement>}
        onKeyDown={(event) => {
          if (event.key === "Escape") {
            event.preventDefault();
            onDeny();
          }
        }}
      >
        <h2>Allow this tool call?</h2>
        <p className="approval-tool">{request.displayName || request.toolName}</p>
        <p className="approval-name">{request.toolName}</p>
        <pre className="approval-preview">{preview.slice(0, 4096)}</pre>
        <div className="approval-scopes" role="radiogroup" aria-label="Approval scope">
          {request.allowedScopes.map((option) => (
            <label key={option} className="approval-scope">
              <input
                type="radio"
                name="approval-scope"
                checked={scope === option}
                onChange={() => onScope(option)}
              />
              {SCOPE_LABEL[option]}
            </label>
          ))}
        </div>
        {error ? (
          <div className="composer-error" role="alert">
            <span className="error-code">{error.code}</span>
            <span>{error.message}</span>
          </div>
        ) : null}
        <div className="approval-actions">
          <button type="button" className="text-btn" onClick={onDeny}>
            Deny
          </button>
          <button type="button" className="send-btn primary approval-allow" onClick={onAllow}>
            Allow
          </button>
        </div>
        <p className="approval-note">
          The shipped approval dialog layout was not captured; this minimal dialog is
          rebuild-defined. A denial is returned to the model as a tool error.
        </p>
      </div>
    </div>
  );
}

function ToolPickerDialog({
  tools,
  selected,
  loading,
  error,
  onToggle,
  onApply,
  onClose,
  dialogRef,
}: {
  tools: ToolDescriptor[];
  selected: string[];
  loading: boolean;
  error: BridgeError | null;
  onToggle: (name: string) => void;
  onApply: () => void;
  onClose: () => void;
  dialogRef: React.RefObject<HTMLDivElement | null>;
}) {
  const [filter, setFilter] = useState("");
  const query = filter.trim().toLowerCase();
  const visible = tools.filter(
    (tool) =>
      !query ||
      tool.name.toLowerCase().includes(query) ||
      (tool.description ?? "").toLowerCase().includes(query),
  );
  const missing = selected.filter((name) => !tools.some((tool) => tool.name === name));
  return (
    <div className="overlay-backdrop">
      <div
        className="tool-picker-modal"
        role="dialog"
        aria-modal="true"
        aria-label="Session tools"
        ref={dialogRef as React.Ref<HTMLDivElement>}
        onKeyDown={(event) => {
          if (event.key === "Escape") {
            event.preventDefault();
            onClose();
          }
        }}
      >
        <h2>Session tools</h2>
        <p className="settings-note">
          The shipped tool picker layout was not captured; this minimal picker is
          rebuild-defined. Selecting a tool does not grant approval — every call still
          follows the policy and approval path. The selection is frozen for this session
          until you change it here.
        </p>
        <input
          className="settings-search"
          placeholder="Filter tools…"
          aria-label="Filter tools"
          value={filter}
          onChange={(event) => setFilter(event.target.value)}
        />
        {error ? (
          <div className="composer-error" role="alert">
            <span className="error-code">{error.code}</span>
            <span>{error.message}</span>
          </div>
        ) : null}
        {loading ? (
          <div className="settings-note">Loading tools…</div>
        ) : visible.length === 0 && missing.length === 0 ? (
          <div className="settings-note">
            No live tools. Connect an MCP server in Settings → Developer first.
          </div>
        ) : (
          <div className="settings-list tool-picker-list">
            {visible.map((tool) => (
              <label key={tool.name} className="settings-row tool-picker-row">
                <div>
                  <div className="settings-row-title">{tool.title || tool.name}</div>
                  <div className="settings-row-sub">{tool.name}</div>
                  {tool.description ? (
                    <div className="settings-row-sub">{tool.description}</div>
                  ) : null}
                </div>
                <input
                  type="checkbox"
                  checked={selected.includes(tool.name)}
                  onChange={() => onToggle(tool.name)}
                />
              </label>
            ))}
            {missing.map((name) => (
              <label key={name} className="settings-row tool-picker-row">
                <div>
                  <div className="settings-row-title">{name}</div>
                  <div className="settings-row-sub">
                    Not available from a connected server. The next send will fail until
                    it reconnects; remove it or retry the server.
                  </div>
                </div>
                <input
                  type="checkbox"
                  checked
                  onChange={() => onToggle(name)}
                />
              </label>
            ))}
          </div>
        )}
        <div className="approval-actions">
          <button type="button" className="text-btn" onClick={onClose}>
            Cancel
          </button>
          <button type="button" className="send-btn primary approval-allow" onClick={onApply}>
            Apply
          </button>
        </div>
      </div>
    </div>
  );
}

export function CoworkSessionScreen() {
  const { sessionId = "" } = useParams();
  // Keying by session id gives each task its own composer, draft, and stream
  // state. Without this, navigating from task A to task B while A streams
  // would show A's Stop button and live text in B's composer, and A's
  // completion would overwrite B's draft.
  return <SessionScreen key={sessionId} sessionId={sessionId} surface="chat" />;
}

/** Code session detail — same transcript/tool/approval machinery, Code route
 * and Code-specific bridge commands (`docs/features/code.md`; Phase 6). */
export function CodeSessionScreen() {
  const { sessionId = "" } = useParams();
  return <SessionScreen key={sessionId} sessionId={sessionId} surface="code" />;
}

export function SessionScreen({
  sessionId,
  surface,
}: {
  sessionId: string;
  surface: "chat" | "code";
}) {
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
  const [approval, setApproval] = useState<ToolApprovalEvent | null>(null);
  const [approvalScope, setApprovalScope] = useState<ApprovalScope>("once");
  const [approvalError, setApprovalError] = useState<BridgeError | null>(null);
  const [liveTool, setLiveTool] = useState<string | null>(null);
  const [skills, setSkills] = useState<SkillView[]>([]);
  const [skillsAvailable, setSkillsAvailable] = useState(false);
  const [skillMenuOpen, setSkillMenuOpen] = useState(false);
  const [skillError, setSkillError] = useState<BridgeError | null>(null);
  const [toolsOpen, setToolsOpen] = useState(false);
  const [toolCatalog, setToolCatalog] = useState<ToolDescriptor[]>([]);
  const [selectedTools, setSelectedTools] = useState<string[]>([]);
  const [toolsLoading, setToolsLoading] = useState(false);
  const [toolsError, setToolsError] = useState<BridgeError | null>(null);
  const approvalRef = useRef<HTMLDivElement>(null);
  const toolPickerRef = useRef<HTMLDivElement>(null);
  useFocusTrap(approvalRef, approval !== null);
  useFocusTrap(toolPickerRef, toolsOpen);
  const taRef = useRef<HTMLTextAreaElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const bodyRef = useRef<HTMLDivElement>(null);
  const autoSent = useRef(false);
  const sendingRef = useRef(false);
  const stickToBottom = useRef(true);

  const draftKey = sessionDraftKey(sessionId);
  const home = surface === "code" ? "/epitaxy" : "/new";

  const load = useCallback(async () => {
    const response =
      surface === "code"
        ? await chat({ type: "openCodeSession", id: sessionId })
        : await chat({ type: "openSession", id: sessionId });
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
  }, [sessionId, surface]);

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
    setApproval(null);
    setApprovalError(null);
    setLiveTool(null);
    setSkillMenuOpen(false);
    setSkillError(null);
    setToolsOpen(false);
    setToolsError(null);
    setToolCatalog([]);
    setSelectedTools([]);
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
      } else if (event.type === "tool_call") {
        setLiveTool(event.name);
      } else if (event.type === "tool_result") {
        setLiveTool(null);
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

  // Skill catalog for the composer picker. A browser preview has no host, so
  // the picker stays absent rather than showing invented rows.
  useEffect(() => {
    let cancelled = false;
    void chat({ type: "listSkills" }).then((response) => {
      if (cancelled) return;
      if (response.type === "skills") {
        // Disabled skills stay in the catalog so Settings can re-enable them,
        // but the composer must not offer a selection the bridge will refuse.
        setSkills(
          response.view.skills.filter(
            (skill) => !response.view.disabled.includes(skill.name),
          ),
        );
        setSkillsAvailable(true);
      }
    });
    return () => {
      cancelled = true;
    };
  }, []);

  // Approval prompts are per-session; another task's prompt must never appear
  // over this transcript.
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let cancelled = false;
    void onToolApproval((event) => {
      if (event.sessionId !== sessionId) return;
      setApproval(event);
      setApprovalScope(event.allowedScopes[0] ?? "once");
      setApprovalError(null);
    }).then((stop) => {
      if (cancelled) stop();
      else unlisten = stop;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [sessionId]);

  const respondApproval = useCallback(
    async (decision: "allow" | "deny") => {
      if (!approval) return;
      const response = await chat({
        type: "respondToolApproval",
        approvalId: approval.id,
        decision,
        scope: decision === "allow" ? approvalScope : null,
      });
      if (response.type === "ack") {
        setApproval(null);
        setApprovalError(null);
      } else if (response.type === "error") {
        setApprovalError(response.error);
      }
    },
    [approval, approvalScope],
  );

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
        const response =
          surface === "code"
            ? await chat({ type: "sendCodeTurn", sessionId, text })
            : await chat({ type: "sendTurn", sessionId, text });
        if (response.type === "turn" || response.type === "toolTurn") {
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
        setLiveTool(null);
      }
    },
    [draftKey, load, pending?.originKey, sessionId, surface],
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
    void send(pending.pendingText);
  }, [view, pending, send, navigate, location.pathname, sessionId]);

  const stop = async () => {
    const response = await chat({ type: "stopTurn", sessionId });
    if (response.type === "error") setActionError(response.error);
    setApproval(null);
  };

  const openTools = async () => {
    const extra = view?.session.extra as { mcpTools?: unknown } | null;
    setSelectedTools(
      Array.isArray(extra?.mcpTools)
        ? extra.mcpTools.filter((name): name is string => typeof name === "string")
        : [],
    );
    setToolsOpen(true);
    setToolsLoading(true);
    setToolsError(null);
    const response = await chat({ type: "mcpTools" });
    if (response.type === "mcpTools") {
      setToolCatalog(response.tools);
    } else if (response.type === "error") {
      setToolsError(response.error);
      setToolCatalog([]);
    }
    setToolsLoading(false);
  };

  const applyTools = async () => {
    setToolsError(null);
    const response = await chat({
      type: "setSessionTools",
      sessionId,
      tools: selectedTools,
    });
    if (response.type === "ack") {
      setToolsOpen(false);
      await load();
    } else if (response.type === "error") {
      setToolsError(response.error);
    }
  };

  const insertSkill = async (name: string) => {
    setSkillMenuOpen(false);
    setSkillError(null);
    const response = await chat({ type: "selectSkill", name });
    if (response.type === "skillSelection") {
      const next = draft.trim()
        ? `${draft}\n\n${response.selection.instructions}`
        : response.selection.instructions;
      updateDraft(next);
      taRef.current?.focus();
    } else if (response.type === "error") {
      setSkillError(response.error);
    }
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
      navigate(home);
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
      navigate(home);
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
        <h2>
          {preview
            ? `${surface === "code" ? "Code" : "Chat"} is available in the desktop app`
            : "This task could not be opened"}
        </h2>
        <p>
          {preview
            ? "This browser preview has no local session store."
            : loadError?.message ?? "The session is not available."}
        </p>
        <button type="button" className="text-btn" onClick={() => navigate(home)}>
          {surface === "code" ? "Back to Code" : "Back to New"}
        </button>
      </div>
    );
  }

  const session = view.session;
  const extra = session.extra as { providerId?: unknown } | null;
  const label = modelLabel(extra?.providerId, session.model);
  const hasDraft = draft.trim().length > 0;
  const codeContext =
    surface === "code"
      ? `${folderName(session.cwd)} · ${permissionModeLabel(session.permission_mode)}`
      : null;

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
                    disabled={streaming}
                    title={streaming ? "Stop the running turn first" : undefined}
                    onClick={() => {
                      setMenuOpen(false);
                      void openTools();
                    }}
                  >
                    Tools…
                  </button>
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
        {surface === "code" ? (
          <span className="session-code-context" title={session.cwd ?? undefined}>
            {codeContext}
          </span>
        ) : null}
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
                  <span>{liveTool ? `Using ${liveTool}…` : "Working on it…"}</span>
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
        {skillError ? (
          <div className="composer-error" role="alert">
            <span className="error-code">{skillError.code}</span>
            <span>{skillError.message}</span>
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
            {skillsAvailable ? (
              <div className="skill-picker">
                <button
                  type="button"
                  className="text-btn"
                  aria-expanded={skillMenuOpen}
                  aria-haspopup="menu"
                  onClick={() => setSkillMenuOpen((open) => !open)}
                >
                  Skills
                </button>
                {skillMenuOpen ? (
                  <div className="skill-menu" role="menu">
                    {skills.length === 0 ? (
                      <div className="skill-empty">No skills found</div>
                    ) : (
                      skills.map((skill) => (
                        <button
                          key={skill.name}
                          type="button"
                          role="menuitem"
                          className="account-item skill-item"
                          disabled={!skill.runnable}
                          title={
                            skill.runnable
                              ? skill.description
                              : "Bundled body not captured; this catalog entry is listed only"
                          }
                          onClick={() => void insertSkill(skill.name)}
                        >
                          <span>{skill.displayName}</span>
                          <span className="skill-source">
                            {skill.runnable ? skill.source : `${skill.source} · not runnable`}
                          </span>
                        </button>
                      ))
                    )}
                    <div className="skill-note">
                      Selecting inserts instructions; it does not grant tool approvals.
                    </div>
                  </div>
                ) : null}
              </div>
            ) : null}
            {codeContext ? (
              <span className="composer-code-context" title={session.cwd ?? undefined}>
                {codeContext}
              </span>
            ) : null}
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
      {toolsOpen ? (
        <ToolPickerDialog
          tools={toolCatalog}
          selected={selectedTools}
          loading={toolsLoading}
          error={toolsError}
          onToggle={(name) =>
            setSelectedTools((current) =>
              current.includes(name)
                ? current.filter((item) => item !== name)
                : [...current, name],
            )
          }
          onApply={() => void applyTools()}
          onClose={() => setToolsOpen(false)}
          dialogRef={toolPickerRef}
        />
      ) : null}
      {approval ? (
        <ApprovalDialog
          request={approval}
          scope={approvalScope}
          error={approvalError}
          onScope={setApprovalScope}
          onAllow={() => void respondApproval("allow")}
          onDeny={() => void respondApproval("deny")}
          dialogRef={approvalRef}
        />
      ) : null}
    </div>
  );
}
