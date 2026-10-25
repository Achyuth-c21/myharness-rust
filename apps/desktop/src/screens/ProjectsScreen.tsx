/**
 * Projects (UI-SCREEN-04; docs/features/projects.md, docs/screens/projects.md;
 * REQ-PROJ-01..06; roadmap Phase 6 slice 2).
 *
 * What is captured vs designed:
 * - Empty state, choice dialog, and form are copied from the captures — every
 *   string below is verbatim from the docs (REQ-PROJ-01/02/03).
 * - The populated list and the detail screen were never observed (no project
 *   was created during inspection). They are an explicit rebuild design, kept
 *   minimal, and labelled as such — no card-grid invention.
 * - The shipped difference between "Start from scratch" and "Use an existing
 *   folder" was not captured (only the existing-folder form was). Both paths
 *   open the same form here; the folder is always an existing directory
   * because the bridge never creates one (`crates/bridge/src/policy.rs`).
 *
 * The folder is validated by the bridge (canonical + policy), never here:
 * the renderer treats every path as untrusted display data.
 */
import { useCallback, useEffect, useRef, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import {
  chat,
  emitSessionsChanged,
  pickWorkspaceFolder,
  revealWorkspaceFolder,
  type BridgeError,
  type ProjectRecord,
  type SessionRecord,
} from "../chat/client";
import { formatRelative } from "../chat/format";
import { useFocusTrap } from "../components/focusTrap";

function FolderGlyph({ size = 40 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 40 40" fill="none" aria-hidden="true">
      <path
        d="M5 12a3 3 0 0 1 3-3h7l4 4.5h13a3 3 0 0 1 3 3V28a3 3 0 0 1-3 3H8a3 3 0 0 1-3-3V12Z"
        stroke="currentColor"
        strokeWidth="1.6"
      />
    </svg>
  );
}

function PlusGlyph() {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path d="M8 3v10M3 8h10" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
    </svg>
  );
}

function CloseGlyph() {
  return (
    <svg width="12" height="12" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path d="m4 4 8 8M12 4l-8 8" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" />
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

/** The projects list payload, or the honest reason there is none. */
type ProjectsState =
  | { kind: "loading" }
  | { kind: "unavailable"; error: BridgeError }
  | { kind: "ready"; projects: ProjectRecord[] };

function useProjects(): ProjectsState & { reload: () => void } {
  const [state, setState] = useState<ProjectsState>({ kind: "loading" });
  const [tick, setTick] = useState(0);
  useEffect(() => {
    let cancelled = false;
    setState({ kind: "loading" });
    void chat({ type: "listProjects" }).then((response) => {
      if (cancelled) return;
      if (response.type === "projects") {
        setState({ kind: "ready", projects: response.projects });
      } else if (response.type === "error") {
        setState({ kind: "unavailable", error: response.error });
      } else {
        setState({ kind: "unavailable", error: { code: "BRIDGE_ERROR", message: "Unexpected response.", retryable: true } });
      }
    });
    return () => {
      cancelled = true;
    };
  }, [tick]);
  return { ...state, reload: () => setTick((t) => t + 1) };
}

/**
 * The two-step create flow: choice dialog (step 1) then the form (step 2).
 * Escape/outside-click dismissal is inferred (docs/features/projects.md);
 * the dirty-form confirmation behavior is unknown and not claimed.
 */
export function CreateProjectDialog({
  onClose,
  onCreated,
}: {
  onClose: () => void;
  onCreated: (project: ProjectRecord) => void;
}) {
  const [step, setStep] = useState<"choice" | "form">("choice");
  const [name, setName] = useState("");
  const [instructions, setInstructions] = useState("");
  const [folder, setFolder] = useState<string | null>(null);
  const [memoryEnabled, setMemoryEnabled] = useState(true);
  const [picking, setPicking] = useState(false);
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState<BridgeError | null>(null);
  const trapRef = useRef<HTMLDivElement>(null);
  useFocusTrap(trapRef, true);
  // Swapping steps unmounts the clicked card/footer button, which drops focus
  // to <body> and out of the trapped layer. Re-seed focus on the container so
  // keyboard dismissal (Escape) keeps working after each step change.
  useEffect(() => {
    trapRef.current?.focus();
  }, [step]);

  const pickFolder = async () => {
    setPicking(true);
    setError(null);
    try {
      const picked = await pickWorkspaceFolder();
      if (picked.kind === "picked") setFolder(picked.path);
      else if (picked.kind === "error") setError(picked.error);
      // Cancellation leaves the form unchanged (docs/features/projects.md).
    } finally {
      setPicking(false);
    }
  };

  const create = async () => {
    if (!name.trim() || !folder || creating) return;
    setCreating(true);
    setError(null);
    try {
      const response = await chat({
        type: "createProject",
        name,
        instructions: instructions.trim() ? instructions : null,
        cwd: folder,
        memoryEnabled,
      });
      if (response.type === "project") {
        onCreated(response.project);
      } else if (response.type === "error") {
        // Inline error; input retained (docs/features/projects.md §States).
        setError(response.error);
      }
    } finally {
      setCreating(false);
    }
  };

  const canCreate = name.trim().length > 0 && folder !== null && !creating;

  return (
    <div
      className="overlay-backdrop"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div
        className="project-dialog"
        role="dialog"
        aria-modal="true"
        aria-label={step === "choice" ? "Create a new project" : "Start a new project"}
        ref={trapRef}
        tabIndex={-1}
        onKeyDown={(event) => {
          if (event.key === "Escape") {
            event.preventDefault();
            event.stopPropagation();
            onClose();
          }
        }}
      >
        <button type="button" className="dialog-close" aria-label="Close" onClick={onClose}>
          <CloseGlyph />
        </button>
        {step === "choice" ? (
          <>
            <h2>Create a new project</h2>
            <p className="dialog-lede">
              A dedicated place for ongoing work, where context builds over time. Files and
              instructions stay in a folder on your computer.
            </p>
            <div className="project-choices">
              <button type="button" className="project-choice" onClick={() => setStep("form")}>
                <span className="project-choice-icon" aria-hidden="true">
                  <PlusGlyph />
                </span>
                <span>
                  <span className="project-choice-title">Start from scratch</span>
                  <span className="project-choice-copy">Set up a new folder with instructions and files.</span>
                </span>
                <ChevronRight />
              </button>
              <button type="button" className="project-choice" onClick={() => setStep("form")}>
                <span className="project-choice-icon" aria-hidden="true">
                  <FolderGlyph size={16} />
                </span>
                <span>
                  <span className="project-choice-title">Use an existing folder</span>
                  <span className="project-choice-copy">Give Claude a folder you already work from.</span>
                </span>
                <ChevronRight />
              </button>
            </div>
          </>
        ) : (
          <>
            <h2>Start a new project</h2>
            <div className="project-form">
              <label className="field">
                <span className="field-label">Name</span>
                <input
                  type="text"
                  placeholder="Project name"
                  value={name}
                  maxLength={200}
                  onChange={(event) => {
                    setName(event.target.value);
                    if (error) setError(null);
                  }}
                />
              </label>
              <label className="field">
                <span className="field-label">Instructions</span>
                <textarea
                  placeholder="Tell Claude how to work in this project (optional)"
                  value={instructions}
                  rows={4}
                  onChange={(event) => setInstructions(event.target.value)}
                />
              </label>
              <div className="field">
                <span className="field-label">Choose project location</span>
                <div className="folder-field">
                  <button
                    type="button"
                    className="btn-secondary"
                    onClick={() => void pickFolder()}
                    disabled={picking}
                  >
                    {picking ? "Choosing…" : "Choose…"}
                  </button>
                  <span className={`folder-value${folder ? "" : " empty"}`}>
                    {folder ?? "Select a folder..."}
                  </span>
                </div>
                <p className="field-hint">
                  The folder must already exist. The app never creates one and never moves files.
                </p>
              </div>
              {error ? (
                <div className="composer-error" role="alert">
                  <span className="error-code">{error.code}</span>
                  <span>
                    {error.code === "BRIDGE_UNAVAILABLE"
                      ? "Creating a project is available in the desktop app."
                      : error.message}
                  </span>
                </div>
              ) : null}
              <div className="dialog-footer">
                <div className="memory-status">
                  <label className="switch">
                    <input
                      type="checkbox"
                      checked={memoryEnabled}
                      onChange={(event) => setMemoryEnabled(event.target.checked)}
                    />
                    <span className="switch-track" aria-hidden="true" />
                    <span className="switch-label">{memoryEnabled ? "Memory is on" : "Memory is off"}</span>
                  </label>
                </div>
                <span className="dialog-footer-actions">
                  <button type="button" className="btn-secondary" onClick={() => setStep("choice")}>
                    Back
                  </button>
                  <button
                    type="button"
                    className="btn-primary"
                    disabled={!canCreate}
                    title={canCreate ? "Create the project" : "Name and folder are required"}
                    onClick={() => void create()}
                  >
                    {creating ? "Creating…" : "Create"}
                  </button>
                </span>
              </div>
            </div>
          </>
        )}
      </div>
    </div>
  );
}

/**
 * /projects — the list. The empty state is the captured copy; the populated
 * rows are a rebuild design (unobserved), one row per project with the
 * folder-availability fact the bridge computed.
 */
export function ProjectsScreen() {
  const projects = useProjects();
  const [creating, setCreating] = useState(false);
  const [filter, setFilter] = useState("");
  const navigate = useNavigate();

  const visible =
    projects.kind === "ready"
      ? projects.projects.filter((project) =>
          filter.trim() ? project.name.toLowerCase().includes(filter.trim().toLowerCase()) : true,
        )
      : [];

  return (
    <>
      <header className="screen-head">
        <h1>Projects</h1>
        {projects.kind === "ready" && projects.projects.length > 0 ? (
          <input
            type="search"
            className="projects-search"
            placeholder="Search projects"
            aria-label="Search projects"
            value={filter}
            onChange={(event) => setFilter(event.target.value)}
          />
        ) : null}
        <button
          type="button"
          className="btn-primary"
          onClick={() => setCreating(true)}
          title="Create a new project"
        >
          New project
        </button>
      </header>
      {projects.kind === "loading" ? (
        <div className="screen-empty" aria-busy="true" />
      ) : projects.kind === "ready" && visible.length === 0 ? (
        <div className="screen-empty">
          <FolderGlyph />
          <h2>Looking to start a project?</h2>
          <p>Give Claude a folder you already work from</p>
          <button type="button" className="btn-secondary" onClick={() => setCreating(true)}>
            New project
          </button>
        </div>
      ) : projects.kind === "unavailable" ? (
        // The captured empty state still renders: the dialog opens client-side
        // and the folder picker reports the desktop-only boundary honestly
        // (the same posture the Code dashboard preview uses).
        <div className="screen-empty">
          <FolderGlyph />
          <h2>Looking to start a project?</h2>
          <p>Give Claude a folder you already work from</p>
          <button type="button" className="btn-secondary" onClick={() => setCreating(true)}>
            New project
          </button>
        </div>
      ) : (
        <div className="projects-list">
          {visible.map((project) => (
            <Link key={project.id} to={`/projects/${project.id}`} className="project-row">
              <span className="project-row-icon" aria-hidden="true">
                <FolderGlyph size={18} />
              </span>
              <span className="project-row-main">
                <span className="project-row-name">{project.name}</span>
                <span className="project-row-folder" title={project.cwd}>
                  {project.folderAvailable
                    ? project.cwd
                    : `${project.cwd} — folder unavailable`}
                </span>
              </span>
              {!project.folderAvailable ? (
                <span className="project-row-state unavailable">Folder unavailable</span>
              ) : null}
              <span className="project-row-time">{formatRelative(project.lastActivityAt)}</span>
              <ChevronRight />
            </Link>
          ))}
          {visible.length === 0 ? <div className="palette-empty">No results</div> : null}
        </div>
      )}
      {creating ? (
        <CreateProjectDialog
          onClose={() => setCreating(false)}
          onCreated={(project) => {
            setCreating(false);
            projects.reload();
            navigate(`/projects/${project.id}`);
          }}
        />
      ) : null}
    </>
  );
}

/** The detail sessions payload, or the honest reason there is none. */
type DetailState =
  | { kind: "loading" }
  | { kind: "missing" }
  | { kind: "unavailable"; error: BridgeError }
  | { kind: "ready"; project: ProjectRecord; sessions: SessionRecord[] };

function sessionHref(session: SessionRecord): string {
  return session.surface === "code" ? `/epitaxy/${session.id}` : `/cowork/${session.id}`;
}

/**
 * /projects/:projectId — the detail screen. Unobserved in the shipped app; the
 * rebuild minimum from docs/features/projects.md §Project detail: edit
 * name/instructions, manage memory, open the folder, list sessions, delete the
 * project without deleting unrelated files. A deleted project is "project
 * missing" — never an empty shell (REQ-PROJ-06).
 */
export function ProjectDetailScreen() {
  const { projectId } = useParams();
  const navigate = useNavigate();
  const [state, setState] = useState<DetailState>({ kind: "loading" });
  const [name, setName] = useState("");
  const [instructions, setInstructions] = useState("");
  const [memoryEnabled, setMemoryEnabled] = useState(true);
  const [dirty, setDirty] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<BridgeError | null>(null);
  const [revealing, setRevealing] = useState(false);
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const [startingTask, setStartingTask] = useState(false);

  const load = useCallback(async () => {
    if (!projectId) return;
    setState({ kind: "loading" });
    const response = await chat({ type: "openProject", id: projectId, archived: null });
    if (response.type === "projectDetail") {
      setState({ kind: "ready", project: response.project, sessions: response.sessions });
      setName(response.project.name);
      setInstructions(response.project.instructions ?? "");
      setMemoryEnabled(response.project.memory_enabled);
      setDirty(false);
    } else if (response.type === "error") {
      if (response.error.code === "PROJECT_MISSING") setState({ kind: "missing" });
      else setState({ kind: "unavailable", error: response.error });
    }
  }, [projectId]);

  useEffect(() => {
    void load();
  }, [load]);

  if (state.kind === "loading") return <div className="screen-empty" aria-busy="true" />;
  if (state.kind === "missing") {
    return (
      <div className="screen-empty">
        <FolderGlyph />
        <h2>Project missing</h2>
        <p>This project no longer exists. Its tasks keep their own history.</p>
        <Link className="btn-secondary" to="/projects">
          Back to Projects
        </Link>
      </div>
    );
  }
  if (state.kind === "unavailable") {
    return (
      <div className="screen-empty">
        <FolderGlyph />
        <h2>Projects are available in the desktop app.</h2>
        <p>
          {state.error.code}: {state.error.message}
        </p>
        <Link className="btn-secondary" to="/projects">
          Back to Projects
        </Link>
      </div>
    );
  }

  const { project, sessions } = state;

  const save = async () => {
    if (saving) return;
    setSaving(true);
    setError(null);
    try {
      const response = await chat({
        type: "updateProject",
        id: project.id,
        name,
        instructions: instructions.trim() ? instructions : null,
        memoryEnabled,
      });
      if (response.type === "project") {
        setState({ kind: "ready", project: response.project, sessions });
        setName(response.project.name);
        setInstructions(response.project.instructions ?? "");
        setMemoryEnabled(response.project.memory_enabled);
        setDirty(false);
      } else if (response.type === "error") {
        setError(response.error);
      }
    } finally {
      setSaving(false);
    }
  };

  const reveal = async () => {
    setRevealing(true);
    setError(null);
    try {
      const failure = await revealWorkspaceFolder(project.cwd);
      if (failure) setError(failure);
    } finally {
      setRevealing(false);
    }
  };

  const startTask = async () => {
    if (startingTask) return;
    setStartingTask(true);
    setError(null);
    try {
      const response = await chat({ type: "createSession", title: null, projectId: project.id, cwd: null });
      if (response.type === "session") {
        emitSessionsChanged();
        navigate(`/cowork/${response.session.session.id}`);
      } else if (response.type === "error") {
        setError(response.error);
      }
    } finally {
      setStartingTask(false);
    }
  };

  const destroy = async () => {
    const response = await chat({ type: "deleteProject", id: project.id });
    if (response.type === "projectDeleted") {
      navigate("/projects");
    } else if (response.type === "error") {
      setError(response.error);
      setConfirmingDelete(false);
    }
  };

  return (
    <div className="project-detail">
      <header className="screen-head">
        <h1>{project.name}</h1>
        <button
          type="button"
          className="btn-primary"
          onClick={() => void startTask()}
          disabled={startingTask || !project.folderAvailable}
          title={
            project.folderAvailable
              ? "New task in this project"
              : "The project folder is unavailable"
          }
        >
          New task
        </button>
      </header>

      {error ? (
        <div className="composer-error" role="alert">
          <span className="error-code">{error.code}</span>
          <span>
            {error.code === "BRIDGE_UNAVAILABLE"
              ? "This action is available in the desktop app."
              : error.message}
          </span>
        </div>
      ) : null}

      {!project.folderAvailable ? (
        <div className="info-banner" role="status">
          <span>
            Folder unavailable — {project.cwd} no longer resolves to the recorded path. Nothing
            was moved or recreated; restore the folder to work in it again.
          </span>
        </div>
      ) : null}

      <section className="project-detail-grid">
        <div className="project-detail-main">
          <label className="field">
            <span className="field-label">Name</span>
            <input
              type="text"
              value={name}
              maxLength={200}
              onChange={(event) => {
                setName(event.target.value);
                setDirty(true);
              }}
            />
          </label>
          <label className="field">
            <span className="field-label">Instructions</span>
            <textarea
              value={instructions}
              rows={5}
              placeholder="Tell Claude how to work in this project (optional)"
              onChange={(event) => {
                setInstructions(event.target.value);
                setDirty(true);
              }}
            />
          </label>
          <div className="dialog-footer">
            <label className="switch">
              <input
                type="checkbox"
                checked={memoryEnabled}
                onChange={(event) => {
                  setMemoryEnabled(event.target.checked);
                  setDirty(true);
                }}
              />
              <span className="switch-track" aria-hidden="true" />
              <span className="switch-label">{memoryEnabled ? "Memory is on" : "Memory is off"}</span>
            </label>
            <span className="dialog-footer-actions">
              <button
                type="button"
                className="btn-primary"
                disabled={!dirty || saving || name.trim().length === 0}
                onClick={() => void save()}
              >
                {saving ? "Saving…" : "Save changes"}
              </button>
            </span>
          </div>
          <p className="field-hint">
            The project folder is not editable here: past tasks ran in the recorded folder, so a
            different folder is a different project.
          </p>
        </div>
        <aside className="project-detail-side">
          <div className="project-folder-card">
            <span className="field-label">Folder</span>
            <span className="project-row-folder" title={project.cwd}>
              {project.cwd}
            </span>
            <button
              type="button"
              className="btn-secondary"
              onClick={() => void reveal()}
              disabled={revealing || !project.folderAvailable}
              title={
                project.folderAvailable
                  ? "Open the folder in your file manager"
                  : "The folder is not available"
              }
            >
              {revealing ? "Opening…" : "Open folder"}
            </button>
          </div>
          <div className="project-delete-card">
            <span className="field-label">Delete project</span>
            {confirmingDelete ? (
              <div className="confirm-block">
                <p>
                  {sessions.length === 1
                    ? "The project is removed. Its task keeps its own folder and history; files on disk are never touched."
                    : `The project is removed. Its ${sessions.length} tasks keep their own folders and history; files on disk are never touched.`}{" "}
                  Project memory stores nothing in this build, so nothing else is deleted.
                </p>
                <div className="dialog-footer-actions">
                  <button type="button" className="btn-secondary" onClick={() => setConfirmingDelete(false)}>
                    Cancel
                  </button>
                  <button type="button" className="btn-danger" onClick={() => void destroy()}>
                    Delete project
                  </button>
                </div>
              </div>
            ) : (
              <button type="button" className="btn-danger" onClick={() => setConfirmingDelete(true)}>
                Delete…
              </button>
            )}
          </div>
        </aside>
      </section>

      <section className="project-sessions" aria-label="Project tasks">
        <h2>Tasks</h2>
        {sessions.length === 0 ? (
          <p className="field-hint">No tasks in this project yet.</p>
        ) : (
          sessions.map((session) => (
            <Link key={session.id} to={sessionHref(session)} className="project-row">
              <span className="project-row-main">
                <span className="project-row-name">{session.title}</span>
                <span className="project-row-folder">
                  {session.surface === "code" ? "Code" : "Cowork"}
                  {session.archived ? " · archived" : ""}
                </span>
              </span>
              <span className="project-row-time">{formatRelative(session.lastActivityAt)}</span>
              <ChevronRight />
            </Link>
          ))
        )}
      </section>
    </div>
  );
}
