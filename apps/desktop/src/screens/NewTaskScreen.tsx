/**
 * New task / Cowork home (UI-SCREEN-02) — matched to the user-provided reference
 * capture of the shipped surface (2026-10-21, 1920×1080):
 * - greeting "You're here!" with the orange starburst directly beside it
 *   (serif display type; the section sits lower in the viewport than earlier
 *   builds, per the reference);
 * - composer: a rounded input card (transparent textarea; plain "+" lower-left;
 *   a neutral model label and a rounded-square outlined send with an upward
 *   arrow at the right), with a DISTINCT section strip directly beneath
 *   it holding the "Project or folder" selector and the info icon;
 * - "Ideas for you" suggestion rows under the composer (the shipped empty-home
 *   state; the Active list appears with session data — Phase 4).
 *
 * Phase 4 surface: Enter (or Send) creates the session through the bridge and
 * hands the draft to `/cowork/<session-id>`, which streams the first turn. The
 * text moves into that task's stored draft before the first await; it is
 * cleared only when a turn completes, so a recoverable failure never loses it
 * (`TEST-SESSION-009`). When session metadata exists the Active list replaces
 * "Ideas for you"; a browser preview has no core, so it keeps the documented
 * empty home instead of inventing rows.
 */
import { useEffect, useRef, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import {
  chat,
  emitSessionsChanged,
  pickWorkspaceFolder,
  type BridgeError,
  type ProjectRecord,
  type SessionRecord,
} from "../chat/client";
import { loadDraft, newDraftKey, saveDraft } from "../chat/drafts";
import { formatRelative, titleFromMessage } from "../chat/format";
import { useChatSessions } from "../chat/useSessions";

interface Props {
  /** UI-INT-005: the composer focuses on the explicit New invocation (the Ctrl+N
   *  remount driven by newKey in App). Boot/plain navigation do not steal focus —
   *  boot focus visuals are unobserved (docs/14 §9 decision note, report Run 4). */
  focusOnMount?: boolean;
}

/**
 * The composer's working context (REQ-PROJ-05): a project or a one-off
 * folder. "Project or folder" is intentionally broad — a one-off folder does
 * not require creating a project (docs/features/projects.md). The bridge
 * validates whichever path is chosen; the renderer only displays it.
 */
type ComposerContext =
  | { kind: "project"; id: string; name: string }
  | { kind: "folder"; path: string }
  | null;

function Starburst() {
  // Orange starburst mark (docs/07 §1.5: starburst uses the orange accent).
  const rays = Array.from({ length: 8 }, (_, i) => {
    const a = (i * Math.PI * 2) / 8;
    const x1 = 12 + Math.cos(a) * 3.4;
    const y1 = 12 + Math.sin(a) * 3.4;
    const x2 = 12 + Math.cos(a) * 10.6;
    const y2 = 12 + Math.sin(a) * 10.6;
    return <line key={i} x1={x1} y1={y1} x2={x2} y2={y2} stroke="currentColor" strokeWidth="2.6" strokeLinecap="round" />;
  });
  return (
    <svg width="40" height="40" viewBox="0 0 24 24" fill="none" className="starburst" aria-hidden="true">
      {rays}
    </svg>
  );
}

function FolderGlyph({ size = 16 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path
        d="M1.75 4.5A1.5 1.5 0 0 1 3.25 3h3l1.5 1.75h4.5a1.5 1.5 0 0 1 1.5 1.5v5.25a1.5 1.5 0 0 1-1.5 1.5H3.25a1.5 1.5 0 0 1-1.5-1.5V4.5Z"
        stroke="currentColor"
        strokeWidth="1.2"
      />
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

function ArrowUpIcon() {
  // Rounded-square send: upward arrow (reference capture).
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path d="M8 12.5V4M8 4 4.6 7.4M8 4l3.4 3.4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

function InfoIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <circle cx="8" cy="8" r="5.9" stroke="currentColor" strokeWidth="1.2" />
      <path d="M8 7.2v3.6" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" />
      <circle cx="8" cy="5" r="0.8" fill="currentColor" stroke="none" />
    </svg>
  );
}

function BriefingIcon() {
  return (
    <svg width="26" height="26" viewBox="0 0 26 26" fill="none" aria-hidden="true">
      <rect x="3.5" y="3.5" width="19" height="19" rx="4" stroke="currentColor" strokeWidth="1.2" />
      <path d="M8.5 10.5h9M8.5 15.5h5.5" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" />
    </svg>
  );
}

function InboxIcon() {
  return (
    <svg width="26" height="26" viewBox="0 0 26 26" fill="none" aria-hidden="true">
      {/* two overlapping message glyphs */}
      <path d="M4 7h11a2 2 0 0 1 2 2v4.5a2 2 0 0 1-2 2H8.5L5 18v-2.5H4a2 2 0 0 1-2-2V9a2 2 0 0 1 2-2Z" stroke="currentColor" strokeWidth="1.2" strokeLinejoin="round" />
      <path d="M18.5 10.5H21a2 2 0 0 1 2 2v3.5a2 2 0 0 1-2 2h-.5v2.5l-3.5-2.5h-2.2" stroke="currentColor" strokeWidth="1.2" strokeLinejoin="round" />
    </svg>
  );
}

function CustomizeSuggestionIcon() {
  return (
    <svg width="26" height="26" viewBox="0 0 26 26" fill="none" aria-hidden="true">
      {/* document + slider glyph */}
      <path d="M7 2.5h8l4 4V19a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V4.5a2 2 0 0 1 2-2Z" stroke="currentColor" strokeWidth="1.2" strokeLinejoin="round" />
      <path d="M4.5 21.5c2 .5 3.5-.5 4-2l4-7.5 3 1.8-4.2 7.4c-.7 1.2-1.8 1.6-3.2 1.4" stroke="currentColor" strokeWidth="1.2" strokeLinejoin="round" />
    </svg>
  );
}

function ChatGlyph() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path
        d="M2.5 4.5A1.5 1.5 0 0 1 4 3h8a1.5 1.5 0 0 1 1.5 1.5v5A1.5 1.5 0 0 1 12 11H7l-3 2.5V11H4a1.5 1.5 0 0 1-1.5-1.5v-5Z"
        stroke="currentColor"
        strokeWidth="1.2"
        strokeLinejoin="round"
      />
    </svg>
  );
}

function ChevronRight() {
  return (
    <svg width="12" height="12" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path d="m6 3.5 4.5 4.5L6 12.5" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

/** One Active-list row (docs/screens/new-task.md §Active list). */
function ActiveList({ sessions }: { sessions: SessionRecord[] }) {
  return (
    <section className="active-list" aria-label="Active">
      <div className="active-head">
        <h2>Active</h2>
        <button
          type="button"
          className="text-btn"
          disabled
          title="Clearing the active list was not observed; the rebuild keeps history (docs/14 §9)"
        >
          Clear active
        </button>
      </div>
      {sessions.map((session) => (
        <Link key={session.id} to={`/cowork/${session.id}`} className="active-row">
          <span className="active-icon">
            <ChatGlyph />
          </span>
          <span className="active-title">{session.title}</span>
          <span className="active-time">{formatRelative(session.lastActivityAt)}</span>
          <span className="active-chevron">
            <ChevronRight />
          </span>
        </Link>
      ))}
    </section>
  );
}

/** Suggestion rows of the shipped empty home ("Ideas for you", reference capture). */
const IDEAS = [
  { label: "Send me a daily briefing", Icon: BriefingIcon },
  { label: "Organize my inbox", Icon: InboxIcon },
  { label: "Customize Cowork for me", Icon: CustomizeSuggestionIcon },
];

export function NewTaskScreen({ focusOnMount = false }: Props) {
  const navigate = useNavigate();
  const [draft, setDraft] = useState(() => loadDraft(newDraftKey()));
  const [sending, setSending] = useState(false);
  const [sendError, setSendError] = useState<BridgeError | null>(null);
  const { sessions } = useChatSessions(false);
  const taRef = useRef<HTMLTextAreaElement>(null);
  // State updates are asynchronous; the ref makes a double Enter (or a click
  // racing a keypress) create exactly one session.
  const sendingRef = useRef(false);
  // The working context (REQ-PROJ-05). The selector loads real projects from
  // the bridge; a browser preview shows why nothing can be selected.
  const [context, setContext] = useState<ComposerContext>(null);
  const [pickerOpen, setPickerOpen] = useState(false);
  const [projects, setProjects] = useState<ProjectRecord[] | null>(null);
  const [projectsError, setProjectsError] = useState<string | null>(null);
  const [picking, setPicking] = useState(false);

  useEffect(() => {
    if (focusOnMount) taRef.current?.focus(); // UI-INT-005 (Ctrl+N remount only)
  }, [focusOnMount]);

  const hasText = draft.trim().length > 0;

  const send = async () => {
    if (!hasText || sendingRef.current) return;
    sendingRef.current = true;
    setSending(true);
    setSendError(null);
    const text = draft;
    try {
      // Sending creates the session immediately (docs/features/chat-and-composer.md).
      // The draft stays in storage until the task screen owns it; a failure
      // before then never loses text. The selected project/folder, if any,
      // travels as an id/path — the bridge resolves and validates it.
      const response = await chat({
        type: "createSession",
        title: titleFromMessage(text),
        projectId: context?.kind === "project" ? context.id : null,
        cwd: context?.kind === "folder" ? context.path : null,
      });
      if (response.type !== "session") {
        if (response.type === "error") setSendError(response.error);
        return;
      }
      emitSessionsChanged();
      navigate(`/cowork/${response.session.session.id}`, {
        state: { pendingText: text, originKey: newDraftKey() },
      });
    } finally {
      sendingRef.current = false;
      setSending(false);
    }
  };

  const onKeyDown = (event: React.KeyboardEvent) => {
    // UI-INT-041: Enter sends, Shift+Enter newlines. IME composition must not
    // send a half-composed message (REQ-COMPOSER-02).
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      if (!(event.nativeEvent as KeyboardEvent).isComposing) void send();
    }
  };

  const autoGrow = (el: HTMLTextAreaElement) => {
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, 200)}px`; // grows to a max, then scrolls (UI-INT-040)
  };

  const seedDraft = (label: string) => {
    // Reuse the existing composer draft: a suggestion prefills the input. No
    // session starts until the send flow exists (docs/13 §4). C7
    // (report-bug-hunt): the programmatic prefill bypasses onChange, so the
    // textarea must re-measure after the value lands or the card keeps its
    // old height.
    setDraft(label);
    saveDraft(newDraftKey(), label);
    requestAnimationFrame(() => {
      const el = taRef.current;
      if (el) autoGrow(el);
      el?.focus();
    });
  };

  const openPicker = async () => {
    if (pickerOpen) {
      setPickerOpen(false);
      return;
    }
    setPickerOpen(true);
    setProjectsError(null);
    const response = await chat({ type: "listProjects" });
    if (response.type === "projects") {
      setProjects(response.projects);
    } else if (response.type === "error") {
      setProjects(null);
      setProjectsError(
        response.error.code === "BRIDGE_UNAVAILABLE"
          ? "Projects and folders are available in the desktop app."
          : response.error.message,
      );
    }
  };

  const addFolder = async () => {
    setPicking(true);
    try {
      const picked = await pickWorkspaceFolder();
      if (picked.kind === "picked") {
        setContext({ kind: "folder", path: picked.path });
        setPickerOpen(false);
      } else if (picked.kind === "error") {
        setProjectsError(
          picked.error.code === "BRIDGE_UNAVAILABLE"
            ? "Folder selection is available in the desktop app."
            : picked.error.message,
        );
      }
      // Cancellation keeps the selector open.
    } finally {
      setPicking(false);
    }
  };

  return (
    <div className="new-task">
      <h1 className="greeting">
        <Starburst />
        You’re here!
      </h1>

      {sendError ? (
        <div className="composer-error" role="alert">
          <span className="error-code">{sendError.code}</span>
          <span>
            {sendError.code === "BRIDGE_UNAVAILABLE"
              ? "Sending is available in the desktop app. Your draft is kept."
              : sendError.message}
          </span>
        </div>
      ) : null}

      <div className="composer-stack">
        <div className="composer-card">
          <textarea
            ref={taRef}
            id="composer"
            className="composer-input"
            placeholder="How can I help you today?"
            value={draft}
            rows={1}
            aria-label="Message"
            onChange={(event) => {
              const value = event.target.value;
              setDraft(value);
              saveDraft(newDraftKey(), value);
              if (sendError) setSendError(null);
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
              title="Add menu arrives with attachments (docs/13 §later phases)"
            >
              +
            </button>
            <span className="composer-spacer" />
            <button
              type="button"
              className="composer-model"
              disabled
              title="The applied profile chooses the model; it is shown on the task. The picker arrives with the model phase."
            >
              model
            </button>
            <button
              type="button"
              className={`send-btn${hasText ? " primary" : ""}`}
              aria-label="Send"
              disabled={!hasText || sending}
              title={sending ? "Creating the task…" : "Send"}
              onClick={() => void send()}
            >
              <ArrowUpIcon />
            </button>
          </div>
        </div>

        {/* Distinct selector section directly beneath the input (reference:
            same width, its own rounded strip). The selector lists real
            projects and offers one-off folders (REQ-PROJ-05); the info
            affordance is an inert pointer. */}
        <div className="composer-strip">
          <span className="project-chip-wrap">
            <button
              type="button"
              className={`project-chip${context ? " selected" : ""}`}
              aria-expanded={pickerOpen}
              aria-haspopup="menu"
              title={context ? (context.kind === "project" ? context.name : context.path) : "Choose a project or folder"}
              onClick={() => void openPicker()}
            >
              <FolderGlyph />
              {context
                ? context.kind === "project"
                  ? context.name
                  : context.path.split("/").filter(Boolean).pop() ?? context.path
                : "Project or folder"}
              <ChevronDown />
            </button>
            {pickerOpen ? (
              <div className="project-chip-menu" role="menu">
                {projectsError ? (
                  <p className="project-chip-note" role="status">
                    {projectsError}
                  </p>
                ) : projects === null ? (
                  <p className="project-chip-note">Loading…</p>
                ) : (
                  <>
                    {projects.map((project) => (
                      <button
                        key={project.id}
                        type="button"
                        role="menuitem"
                        className="project-chip-item"
                        disabled={!project.folderAvailable}
                        title={project.folderAvailable ? project.cwd : "Folder unavailable"}
                        onClick={() => {
                          setContext({ kind: "project", id: project.id, name: project.name });
                          setPickerOpen(false);
                        }}
                      >
                        <FolderGlyph size={14} />
                        <span>{project.name}</span>
                        {!project.folderAvailable ? <span className="project-row-state unavailable">unavailable</span> : null}
                      </button>
                    ))}
                    {projects.length === 0 ? (
                      <p className="project-chip-note">No projects yet. Create one on the Projects page.</p>
                    ) : null}
                    <button type="button" role="menuitem" className="project-chip-item" onClick={() => void addFolder()} disabled={picking}>
                      <FolderGlyph size={14} />
                      <span>{picking ? "Choosing…" : "Add folder"}</span>
                    </button>
                    {context ? (
                      <button
                        type="button"
                        role="menuitem"
                        className="project-chip-item"
                        onClick={() => {
                          setContext(null);
                          setPickerOpen(false);
                        }}
                      >
                        <span className="project-chip-clear">No context</span>
                      </button>
                    ) : null}
                  </>
                )}
              </div>
            ) : null}
          </span>
          <span className="composer-spacer" />
          <button
            type="button"
            className="icon-btn icon-btn-sm"
            aria-label="Info"
            disabled
            title="Info affordance arrives in a later phase"
          >
            <InfoIcon />
          </button>
        </div>
      </div>

      {sessions.length > 0 ? (
        <ActiveList sessions={sessions} />
      ) : (
        <section className="ideas" aria-label="Ideas for you">
          <h2 className="ideas-title">Ideas for you</h2>
          {IDEAS.map(({ label, Icon }) => (
            <button key={label} type="button" className="idea-row" onClick={() => seedDraft(label)}>
              <Icon />
              <span>{label}</span>
            </button>
          ))}
        </section>
      )}
    </div>
  );
}
