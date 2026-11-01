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

function SearchGlyph() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <circle cx="7" cy="7" r="4.25" stroke="currentColor" strokeWidth="1.2" />
      <path d="m10.2 10.2 3 3" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
    </svg>
  );
}

function ViewGlyph() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path d="M2.5 4h11M2.5 8h11M2.5 12h11" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
    </svg>
  );
}
import {
  chat,
  type BridgeError,
  type SessionRecord,
} from "../chat/client";
import { formatRelative } from "../chat/format";

function DocGlyph() {
  /* The observed empty-state illustration is a document/folder line-art scene
     (docs/23 §3.5 item 29); redrawn as hand-drawn-style SVG — no raster
     assets, license-clean (IMP-50). */
  return (
    <svg width="96" height="78" viewBox="0 0 96 78" fill="none" aria-hidden="true" className="empty-art">
      {/* folder back */}
      <path d="M10 20a3 3 0 0 1 3-3h16l6 6h28a3 3 0 0 1 3 3v8" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round" />
      {/* document rising from the folder */}
      <path d="M30 34h30a3 3 0 0 1 3 3v28a3 3 0 0 1-3 3H30a3 3 0 0 1-3-3V37a3 3 0 0 1 3-3Z" stroke="currentColor" strokeWidth="2.2" />
      <path d="M34 42h22M34 49h22M34 56h12" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
      {/* house glyph */}
      <path d="M66 46h18l-9-9-9 9Z" stroke="currentColor" strokeWidth="2.2" strokeLinejoin="round" />
      <path d="M69 46v10h12V46" stroke="currentColor" strokeWidth="2.2" strokeLinejoin="round" />
      <path d="M10 66c10 6 22 8 38 8" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeDasharray="2.5 5.5" />
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
        {/* The captured header carries search and a view toggle beside New
            artifact (docs/screens/artifacts.md layout). The toggle's states
            were never observed, so it stays disabled-honest. */}
        <div className="screen-head-tools">
          <button
            type="button"
            className="icon-btn"
            aria-label="Search artifacts"
            title="Search artifacts"
            onClick={() => {
              document.querySelector<HTMLInputElement>(".projects-search")?.focus();
            }}
          >
            <SearchGlyph />
          </button>
          <button
            type="button"
            className="icon-btn"
            aria-label="Toggle artifact view"
            disabled
            title="The view toggle's states were not observed in the reference capture"
          >
            <ViewGlyph />
          </button>
        </div>
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
