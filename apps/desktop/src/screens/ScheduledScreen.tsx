/**
 * Scheduled tasks (UI-SCREEN-13; docs/features/scheduled-tasks.md,
 * docs/screens/scheduled-tasks.md; REQ-SCHED-01..06; roadmap Phase 6 slice 4).
 *
 * Captured vs designed:
 * - The header (title, /schedule hint), the banner copy, the keep-awake
 *   switch, the empty state, and the six template cards are the captured
 *   copy — every string is verbatim from the docs.
 * - Keep-awake ships disabled: it prevents idle sleep only, cannot override
 *   lid close or power-off (REQ-SCHED-06), and no platform implementation
 *   exists in this build (TEST-SCHED-005).
 * - The populated list and the editor are an explicit rebuild design
 *   (unobserved). Templates prefill the editor and never create a job
 *   (REQ-SCHED-02); every create action is an explicit Save.
 * - A task row shows the next pending occurrence and the last run or skip;
 *   a skipped run is recorded with its reason and is not a success
 *   (TEST-SCHED-003). A missing project/folder blocks a run clearly
 *   (REQ-SCHED-05).
 */
import { useEffect, useMemo, useRef, useState } from "react";
import { Link } from "react-router-dom";
import {
  chat,
  type BridgeError,
  type ScheduledRunRecord,
  type ScheduledTaskRecord,
  type ScheduledTemplateCard,
} from "../chat/client";
import { useFocusTrap } from "../components/focusTrap";

function ClockGlyph({ size = 40 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 40 40" fill="none" aria-hidden="true">
      <circle cx="20" cy="21.5" r="13" stroke="currentColor" strokeWidth="1.6" />
      <path d="M20 14.5v7l4.5 3" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
      <path d="M13 5.5h14" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
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

function CloseGlyph() {
  return (
    <svg width="12" height="12" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path d="m4 4 8 8M12 4l-8 8" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" />
    </svg>
  );
}

type TasksState =
  | { kind: "loading" }
  | { kind: "unavailable"; error: BridgeError }
  | { kind: "ready"; tasks: ScheduledTaskRecord[] };

type SortKey = "next-run" | "name" | "created";

const WEEKDAYS: { value: number; label: string }[] = [
  { value: 1, label: "Mon" },
  { value: 2, label: "Tue" },
  { value: 3, label: "Wed" },
  { value: 4, label: "Thu" },
  { value: 5, label: "Fri" },
  { value: 6, label: "Sat" },
  { value: 0, label: "Sun" },
];

/** The editor's schedule shape → the 5-field cron subset the core accepts. */
type ScheduleKind = "daily" | "weekly" | "interval";

interface ScheduleDraft {
  kind: ScheduleKind;
  time: string; // HH:MM (daily/weekly)
  days: number[]; // cron weekdays (weekly)
  intervalMinutes: number; // interval
}

function scheduleToCron(draft: ScheduleDraft): string | null {
  const [hh, mm] = draft.time.split(":").map((part) => Number.parseInt(part, 10));
  if (draft.kind === "interval") {
    const minutes = Math.floor(draft.intervalMinutes);
    if (!Number.isFinite(minutes) || minutes < 15 || minutes > 1440 || 1440 % minutes !== 0) {
      return null;
    }
    return `*/${minutes} * * * *`;
  }
  if (!Number.isFinite(hh) || !Number.isFinite(mm) || hh < 0 || hh > 23 || mm < 0 || mm > 59) {
    return null;
  }
  if (draft.kind === "daily") return `${mm} ${hh} * * *`;
  if (draft.days.length === 0) return null;
  const days = [...draft.days].sort((a, b) => a - b).join(",");
  return `${mm} ${hh} * * ${days}`;
}

/** Last run chip: a skip is recorded with its reason and is not a success. */
function RunChip({ run }: { run: ScheduledRunRecord | null }) {
  if (!run) return <span className="sched-run-chip none">Never run</span>;
  const label =
    run.status === "completed"
      ? "Last run completed"
      : run.status === "failed"
        ? "Last run failed"
        : run.status === "skipped"
          ? "Skipped"
          : "Running…";
  return (
    <span
      className={`sched-run-chip ${run.status}`}
      title={run.reason ?? undefined}
    >
      {label}
      {run.status === "skipped" && run.reason ? " — the app was not running" : ""}
    </span>
  );
}

/**
 * The task editor (rebuild design — the shipped editor was never reached).
 * Templates prefill it; Save is the only creation path. The next-run preview
 * comes from the bridge's schedule arithmetic, never a local re-implementation.
 */
function ScheduledTaskEditor({
  initial,
  onClose,
  onSaved,
}: {
  initial: { name: string; prompt: string; cron: string; id?: string };
  onClose: () => void;
  onSaved: (task: ScheduledTaskRecord) => void;
}) {
  const editing = initial.id !== undefined;
  const [name, setName] = useState(initial.name);
  const [prompt, setPrompt] = useState(initial.prompt);
  const [enabled, setEnabled] = useState(true);
  const [schedule, setSchedule] = useState<ScheduleDraft>(() => {
    // Parse the stored cron back into the editor's shape; anything outside
    // the supported subset falls back to a daily default rather than lying.
    const parts = initial.cron.trim().split(/\s+/);
    if (parts.length !== 5) return { kind: "daily", time: "09:00", days: [1, 2, 3, 4, 5], intervalMinutes: 60 };
    const [minute, hour, , , dow] = parts;
    if (hour.startsWith("*/")) {
      return { kind: "interval", time: "09:00", days: [], intervalMinutes: Number.parseInt(hour.slice(2), 10) || 60 };
    }
    const time = `${hour.padStart(2, "0")}:${minute.padStart(2, "0")}`;
    if (dow === "*") return { kind: "daily", time, days: [], intervalMinutes: 60 };
    const days = dow
      .split(",")
      .map((day) => Number.parseInt(day, 10))
      .filter((day) => Number.isFinite(day));
    return days.length > 0
      ? { kind: "weekly", time, days, intervalMinutes: 60 }
      : { kind: "daily", time, days: [], intervalMinutes: 60 };
  });
  const [preview, setPreview] = useState<number[] | null>(null);
  const [error, setError] = useState<BridgeError | null>(null);
  const [saving, setSaving] = useState(false);
  const trapRef = useRef<HTMLDivElement>(null);
  useFocusTrap(trapRef, true);

  const cron = useMemo(() => scheduleToCron(schedule), [schedule]);

  useEffect(() => {
    let cancelled = false;
    setPreview(null);
    if (!cron) return;
    void chat({ type: "previewSchedule", cron, count: 3 }).then((response) => {
      if (cancelled) return;
      if (response.type === "schedulePreview") setPreview(response.times);
    });
    return () => {
      cancelled = true;
    };
  }, [cron]);

  const save = async () => {
    if (!cron || saving) return;
    setSaving(true);
    setError(null);
    try {
      const response = editing
        ? await chat({ type: "updateScheduledTask", id: initial.id!, name, prompt, cron })
        : await chat({
            type: "createScheduledTask",
            name,
            prompt,
            cron,
            projectId: null,
            cwd: null,
            enabled,
          });
      if (response.type === "scheduledTask") onSaved(response.task);
      else if (response.type === "error") setError(response.error);
    } finally {
      setSaving(false);
    }
  };

  const formatPreview = (ms: number) =>
    new Date(ms).toLocaleString(undefined, {
      weekday: "short",
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });

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
        aria-label={editing ? "Edit scheduled task" : "New scheduled task"}
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
        <h2>{editing ? "Edit scheduled task" : "New scheduled task"}</h2>
        <div className="project-form">
          <label className="field">
            <span className="field-label">Name</span>
            <input
              type="text"
              placeholder="Task name"
              value={name}
              maxLength={200}
              onChange={(event) => setName(event.target.value)}
            />
          </label>
          <label className="field">
            <span className="field-label">Prompt</span>
            <textarea
              placeholder="What should Claude do each time this task runs?"
              value={prompt}
              rows={3}
              onChange={(event) => setPrompt(event.target.value)}
            />
          </label>
          <div className="field">
            <span className="field-label">Schedule</span>
            <div className="sched-editor-row">
              <select
                aria-label="Schedule kind"
                value={schedule.kind}
                onChange={(event) =>
                  setSchedule({ ...schedule, kind: event.target.value as ScheduleKind })
                }
              >
                <option value="daily">Every day</option>
                <option value="weekly">Every week on</option>
                <option value="interval">Every N minutes</option>
              </select>
              {schedule.kind === "weekly" ? (
                <select
                  aria-label="Weekday"
                  value={schedule.days[0] ?? 1}
                  onChange={(event) =>
                    setSchedule({ ...schedule, days: [Number.parseInt(event.target.value, 10)] })
                  }
                >
                  {WEEKDAYS.map((day) => (
                    <option key={day.value} value={day.value}>
                      {day.label}
                    </option>
                  ))}
                </select>
              ) : null}
              {schedule.kind === "interval" ? (
                <input
                  type="number"
                  aria-label="Interval minutes"
                  min={15}
                  max={1440}
                  step={15}
                  value={schedule.intervalMinutes}
                  onChange={(event) =>
                    setSchedule({ ...schedule, intervalMinutes: Number.parseInt(event.target.value, 10) || 0 })
                  }
                />
              ) : (
                <input
                  type="time"
                  aria-label="Time of day"
                  value={schedule.time}
                  onChange={(event) => setSchedule({ ...schedule, time: event.target.value })}
                />
              )}
            </div>
            <p className="field-hint">
              {cron
                ? preview && preview.length > 0
                  ? `Next: ${preview.map(formatPreview).join(" · ")}`
                  : "Computing next runs…"
                : "Choose a valid time (intervals divide a day and are at least 15 minutes)."}
            </p>
          </div>
          {!editing ? (
            <label className="switch">
              <input
                type="checkbox"
                checked={enabled}
                onChange={(event) => setEnabled(event.target.checked)}
              />
              <span className="switch-track" aria-hidden="true" />
              <span className="switch-label">Enabled — runs on schedule</span>
            </label>
          ) : null}
          {error ? (
            <div className="composer-error" role="alert">
              <span className="error-code">{error.code}</span>
              <span>
                {error.code === "BRIDGE_UNAVAILABLE"
                  ? "Scheduling is available in the desktop app."
                  : error.message}
              </span>
            </div>
          ) : null}
          <div className="dialog-footer">
            <span />
            <span className="dialog-footer-actions">
              <button type="button" className="btn-secondary" onClick={onClose}>
                Cancel
              </button>
              <button
                type="button"
                className="btn-primary"
                disabled={!cron || name.trim().length === 0 || prompt.trim().length === 0 || saving}
                onClick={() => void save()}
              >
                {saving ? "Saving…" : editing ? "Save changes" : "Create task"}
              </button>
            </span>
          </div>
        </div>
      </div>
    </div>
  );
}

export function ScheduledScreen() {
  const [state, setState] = useState<TasksState>({ kind: "loading" });
  const [templates, setTemplates] = useState<ScheduledTemplateCard[]>([]);
  const [filter, setFilter] = useState("");
  const [sort, setSort] = useState<SortKey>("next-run");
  const [editor, setEditor] = useState<{ name: string; prompt: string; cron: string; id?: string } | null>(null);
  const [error, setError] = useState<BridgeError | null>(null);
  const [confirmingDelete, setConfirmingDelete] = useState<string | null>(null);
  const [running, setRunning] = useState<string | null>(null);

  const load = async () => {
    const response = await chat({ type: "listScheduledTasks" });
    if (response.type === "scheduledTasks") {
      setState({ kind: "ready", tasks: response.tasks });
    } else if (response.type === "error") {
      setState({ kind: "unavailable", error: response.error });
    }
  };

  useEffect(() => {
    void load();
    void chat({ type: "scheduledTemplates" }).then((response) => {
      if (response.type === "templates") setTemplates(response.templates);
    });
  }, []);

  const visible = useMemo(() => {
    if (state.kind !== "ready") return [];
    const filtered = state.tasks.filter((task) =>
      filter.trim() ? task.name.toLowerCase().includes(filter.trim().toLowerCase()) : true,
    );
    return [...filtered].sort((a, b) => {
      if (sort === "name") return a.name.localeCompare(b.name);
      if (sort === "created") return b.createdAt - a.createdAt;
      // Next run: enabled tasks with a pending occurrence first, soonest at
      // the top; everything else trails in creation order.
      const aNext = a.enabled && a.armedAt !== null ? a.armedAt : Number.MAX_SAFE_INTEGER;
      const bNext = b.enabled && b.armedAt !== null ? b.armedAt : Number.MAX_SAFE_INTEGER;
      return aNext - bNext || b.createdAt - a.createdAt;
    });
  }, [state, filter, sort]);

  const toggle = async (task: ScheduledTaskRecord) => {
    setError(null);
    const response = await chat({ type: "setScheduledTaskEnabled", id: task.id, enabled: !task.enabled });
    if (response.type === "scheduledTask") await load();
    else if (response.type === "error") setError(response.error);
  };

  const runNow = async (task: ScheduledTaskRecord) => {
    if (running) return;
    setRunning(task.id);
    setError(null);
    try {
      const response = await chat({ type: "runScheduledTaskNow", id: task.id });
      if (response.type === "scheduledRun" || response.type === "error") {
        if (response.type === "error" && response.error.code !== "BRIDGE_UNAVAILABLE") {
          // A failed run is recorded by the bridge; the row's lastRun shows it.
          setError(response.error);
        }
        await load();
      }
    } finally {
      setRunning(null);
    }
  };

  const destroy = async (task: ScheduledTaskRecord) => {
    setError(null);
    const response = await chat({ type: "deleteScheduledTask", id: task.id });
    if (response.type === "ack") {
      setConfirmingDelete(null);
      await load();
    } else if (response.type === "error") setError(response.error);
  };

  const formatInstant = (ms: number) =>
    new Date(ms).toLocaleString(undefined, {
      weekday: "short",
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });

  const hasTasks = state.kind === "ready" && state.tasks.length > 0;

  return (
    <>
      <header className="screen-head sched-head">
        <div>
          <h1>Scheduled tasks</h1>
          <p className="screen-subtitle">
            Run tasks on a schedule or whenever you need them. Type <code>/schedule</code> in any
            existing task to set one up.
          </p>
        </div>
        {hasTasks ? (
          <input
            type="search"
            className="projects-search"
            placeholder="Search tasks"
            aria-label="Search tasks"
            value={filter}
            onChange={(event) => setFilter(event.target.value)}
          />
        ) : null}
        {hasTasks ? (
          <label className="sched-sort">
            <span className="field-label">Sort by</span>
            <select aria-label="Sort by" value={sort} onChange={(event) => setSort(event.target.value as SortKey)}>
              <option value="next-run">Next run</option>
              <option value="name">Name</option>
              <option value="created">Date created</option>
            </select>
            <ChevronDown />
          </label>
        ) : null}
        <button
          type="button"
          className="btn-primary"
          onClick={() => setEditor({ name: "", prompt: "", cron: "0 9 * * *" })}
        >
          New task
        </button>
      </header>

      <div className="info-banner" role="status">
        <span>Scheduled tasks only run while your computer is awake and online.</span>
        <label className="switch">
          <input
            type="checkbox"
            disabled
            title="Keep-awake is not implemented in this build. It can prevent idle sleep only — never lid close, power-off, or being offline."
          />
          <span className="switch-track" aria-hidden="true" />
          <span className="switch-label">Keep awake</span>
        </label>
      </div>

      {error ? (
        <div className="composer-error" role="alert">
          <span className="error-code">{error.code}</span>
          <span>
            {error.code === "BRIDGE_UNAVAILABLE"
              ? "Scheduling is available in the desktop app."
              : error.message}
          </span>
        </div>
      ) : null}

      {state.kind === "loading" ? (
        <div className="screen-empty" aria-busy="true" />
      ) : state.kind === "unavailable" ? (
        <div className="screen-empty">
          <ClockGlyph />
          <h2>No scheduled tasks yet.</h2>
          <p>Scheduling is available in the desktop app.</p>
        </div>
      ) : !hasTasks ? (
        <>
          <div className="screen-empty">
            <ClockGlyph />
            <h2>No scheduled tasks yet.</h2>
            <p />
          </div>
          <div className="sched-templates" aria-label="Templates">
            {templates.map((template) => (
              <button
                key={template.name}
                type="button"
                className="sched-template"
                title={`Prefills the editor — nothing is created until you save`}
                onClick={() =>
                  setEditor({ name: template.name, prompt: template.prompt, cron: template.cron })
                }
              >
                <span className="sched-template-icon" aria-hidden="true">
                  <ClockGlyph size={18} />
                </span>
                <span className="sched-template-main">
                  <span className="project-row-name">{template.name}</span>
                  <span className="project-row-folder">{template.prompt}</span>
                </span>
                <span className="sched-template-schedule">
                  <ClockGlyph size={12} />
                  {template.scheduleLabel}
                </span>
              </button>
            ))}
          </div>
        </>
      ) : (
        <div className="projects-list">
          {visible.map((task) => (
            <div key={task.id} className="sched-row">
              <span className="project-row-icon" aria-hidden="true">
                <ClockGlyph size={18} />
              </span>
              <span className="project-row-main">
                <span className="project-row-name">{task.name}</span>
                <span className="project-row-folder">
                  {task.scheduleSummary}
                  {task.enabled && task.armedAt !== null
                    ? ` — next ${formatInstant(task.armedAt)}`
                    : " — disabled"}
                </span>
              </span>
              {task.contextUnavailable ? (
                <span className="project-row-state unavailable" title="The bound project or folder is gone; fix the context before running.">
                  Context unavailable
                </span>
              ) : null}
              <RunChip run={task.lastRun} />
              <label className="switch">
                <input
                  type="checkbox"
                  checked={task.enabled}
                  onChange={() => void toggle(task)}
                  aria-label={`Enable ${task.name}`}
                />
                <span className="switch-track" aria-hidden="true" />
              </label>
              <button
                type="button"
                className="btn-secondary"
                disabled={running !== null || task.contextUnavailable}
                title={
                  task.contextUnavailable
                    ? "The context is not available; fix it before running"
                    : "Run this task now"
                }
                onClick={() => void runNow(task)}
              >
                {running === task.id ? "Running…" : "Run now"}
              </button>
              <button
                type="button"
                className="text-btn"
                onClick={() => setEditor({ name: task.name, prompt: task.prompt, cron: task.cron, id: task.id })}
              >
                Edit
              </button>
              {confirmingDelete === task.id ? (
                <span className="confirm-inline">
                  <span>Delete the schedule; prior run sessions stay.</span>
                  <button type="button" className="text-btn" onClick={() => setConfirmingDelete(null)}>
                    Cancel
                  </button>
                  <button type="button" className="btn-danger" onClick={() => void destroy(task)}>
                    Delete
                  </button>
                </span>
              ) : (
                <button type="button" className="text-btn" onClick={() => setConfirmingDelete(task.id)}>
                  Delete
                </button>
              )}
            </div>
          ))}
          {visible.length === 0 ? <div className="palette-empty">No results</div> : null}
          {state.kind === "ready" && state.tasks.length > 0 ? (
            <p className="field-hint">
              Skipped runs are recorded with their reason and never backfilled.{" "}
              {state.tasks.some((task) => task.lastRun?.sessionId) ? "Runs open as normal tasks in " : "Runs appear in "}
              <Link to="/cowork">history</Link>.
            </p>
          ) : null}
        </div>
      )}

      {editor ? (
        <ScheduledTaskEditor
          initial={editor}
          onClose={() => setEditor(null)}
          onSaved={(task) => {
            setEditor(null);
            void load();
            if (!task.enabled) return;
          }}
        />
      ) : null}
    </>
  );
}
