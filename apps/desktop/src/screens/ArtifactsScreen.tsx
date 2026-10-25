/**
 * Artifacts (UI-SCREEN-05; docs/features/artifacts.md; REQ-ART-01/02/06;
 * roadmap Phase 6 slice 3).
 *
 * Captured vs designed:
 * - The empty state ("Create your first artifact" + the suggestion "What needs
 *   my attention") and the header (title, search, black New artifact) are the
 *   captured copy.
 * - "New artifact" does not open a modal: it creates one live-artifact task
 *   through the bridge and navigates to it (REQ-ART-02, explicit click). The
 *   shipped task's auto-generated prompt was never captured, so nothing is
 *   auto-sent — the composer is focused and the user's first send starts
 *   generation (recorded deviation, docs/14 §9).
 * - The empty-state suggestion starts the task with the prompt the button
 *   itself displays — the generated prompt IS the visible copy, so nothing is
 *   invented; the task streams immediately.
 * - The populated library rows are a rebuild design (unobserved), derived only
 *   from real session rows carrying the artifactTask marker. There is no
 *   artifact content store, no versions, and no viewer in this slice: the
 *   artifactsPane gate was unavailable during inspection, so the viewer is
 *   stated unavailable rather than emulated (REQ-ART-06).
 */
import { useEffect, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import {
  chat,
  type BridgeError,
  type SessionRecord,
} from "../chat/client";
import { formatRelative } from "../chat/format";

function DocGlyph() {
  return (
    <svg width="40" height="40" viewBox="0 0 40 40" fill="none" aria-hidden="true">
      <path d="M11 6h13l6 6v22H11V6Z" stroke="currentColor" strokeWidth="1.6" strokeLinejoin="round" />
      <path d="M24 6v6h6M16 18h9M16 23h9M16 28h6" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
    </svg>
  );
}

function GlassesGlyph() {
  return (
    <svg width="18" height="18" viewBox="0 0 20 20" fill="none" aria-hidden="true">
      <circle cx="5.5" cy="12" r="3.2" stroke="currentColor" strokeWidth="1.2" />
      <circle cx="14.5" cy="12" r="3.2" stroke="currentColor" strokeWidth="1.2" />
      <path d="M8.7 11.4c.8-.8 1.8-.8 2.6 0M2.3 12l1-5.4M17.7 12l-1-5.4" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
    </svg>
  );
}

function ChevronRight() {
  return (
    <svg width="12" height="12" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path d="m6 4 4 4-4 4" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

/** True when the stored row is an artifact task (the bridge-owned marker). */
function isArtifactTask(session: SessionRecord): boolean {
  const extra = session.extra as { artifactTask?: unknown } | null;
  return extra?.artifactTask === true;
}

type ArtifactsState =
  | { kind: "loading" }
  | { kind: "unavailable"; error: BridgeError }
  | { kind: "ready"; tasks: SessionRecord[] };

export function ArtifactsScreen() {
  const navigate = useNavigate();
  const [state, setState] = useState<ArtifactsState>({ kind: "loading" });
  const [filter, setFilter] = useState("");
  const [starting, setStarting] = useState(false);
  const [error, setError] = useState<BridgeError | null>(null);

  useEffect(() => {
    let cancelled = false;
    void chat({ type: "listSessions", archived: null }).then((response) => {
      if (cancelled) return;
      if (response.type === "sessions") {
        setState({ kind: "ready", tasks: response.sessions.filter(isArtifactTask) });
      } else if (response.type === "error") {
        setState({ kind: "unavailable", error: response.error });
      } else {
        setState({ kind: "unavailable", error: { code: "BRIDGE_ERROR", message: "Unexpected response.", retryable: true } });
      }
    });
    return () => {
      cancelled = true;
    };
  }, []);

  const startTask = async (prompt: string | null) => {
    if (starting) return; // double-click creates exactly one task (TEST-ART-002)
    setStarting(true);
    setError(null);
    try {
      const response = await chat({ type: "createArtifactTask" });
      if (response.type === "session") {
        navigate(`/cowork/${response.session.session.id}`, {
          // The suggestion's prompt is its own visible copy; "New artifact"
          // starts with an empty composer (no captured prompt to send).
          state: prompt ? { pendingText: prompt, originKey: null } : { pendingText: null, originKey: null },
        });
      } else if (response.type === "error") {
        setError(response.error);
      }
    } finally {
      setStarting(false);
    }
  };

  const visible =
    state.kind === "ready"
      ? state.tasks.filter((task) =>
          filter.trim() ? task.title.toLowerCase().includes(filter.trim().toLowerCase()) : true,
        )
      : [];

  return (
    <>
      <header className="screen-head">
        <h1>Artifacts</h1>
        {state.kind === "ready" && state.tasks.length > 0 ? (
          <input
            type="search"
            className="projects-search"
            placeholder="Search artifacts"
            aria-label="Search artifacts"
            value={filter}
            onChange={(event) => setFilter(event.target.value)}
          />
        ) : null}
        <button
          type="button"
          className="btn-primary"
          disabled={starting}
          onClick={() => void startTask(null)}
          title="Creates one artifact task and opens it — this starts immediately after your first send"
        >
          {starting ? "Creating…" : "New artifact"}
        </button>
      </header>

      {error ? (
        <div className="composer-error" role="alert">
          <span className="error-code">{error.code}</span>
          <span>
            {error.code === "BRIDGE_UNAVAILABLE"
              ? "Artifacts are available in the desktop app."
              : error.message}
          </span>
        </div>
      ) : null}

      {state.kind === "loading" ? (
        <div className="screen-empty" aria-busy="true" />
      ) : (state.kind === "unavailable" || (state.kind === "ready" && state.tasks.length === 0)) ? (
        // The captured empty state: copy + the single suggestion CTA. Without
        // a desktop host the creation reports the boundary honestly.
        <div className="screen-empty">
          <DocGlyph />
          <h2>Create your first artifact</h2>
          <p />
          <button
            type="button"
            className="btn-secondary"
            disabled={starting}
            onClick={() => void startTask("What needs my attention")}
            title="Starts one artifact task with this prompt"
          >
            <span className="artifact-suggestion">
              <GlassesGlyph />
              What needs my attention
            </span>
          </button>
        </div>
      ) : (
        <>
          <div className="artifacts-note" role="note">
            Artifact previews are unavailable in this build — tasks open as normal transcripts
            and the library never claims a completed artifact from them.
          </div>
          <div className="projects-list">
            {visible.map((task) => (
              <Link key={task.id} to={`/cowork/${task.id}`} className="project-row">
                <span className="project-row-icon" aria-hidden="true">
                  <DocGlyph />
                </span>
                <span className="project-row-main">
                  <span className="project-row-name">{task.title}</span>
                  <span className="project-row-folder">Artifact task</span>
                </span>
                {task.archived ? <span className="project-row-state unavailable">Archived</span> : null}
                <span className="project-row-time">{formatRelative(task.lastActivityAt)}</span>
                <ChevronRight />
              </Link>
            ))}
            {visible.length === 0 ? <div className="palette-empty">No results</div> : null}
          </div>
        </>
      )}
    </>
  );
}
