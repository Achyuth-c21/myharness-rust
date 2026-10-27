/**
 * Code dashboard — route `/epitaxy` (`docs/features/code.md` §Dashboard;
 * `docs/screens/code-dashboard.md`; `TEST-CODE-001`, `TEST-UX-001`).
 *
 * The dashboard is local analytics, not a provider dashboard. Every number
 * below comes from the bridge's Code-only ledger query; when the bridge is
 * unavailable the screen says so and renders no zeros, rows, or models in its
 * place. The folder picker is native; cancellation is not an error. The
 * permission selector keeps `Bypass` visible but disabled unless the effective
 * policy explicitly allows it (fail-closed).
 */
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import {
  chat,
  emitSessionsChanged,
  pickWorkspaceFolder,
  type BridgeError,
  type ChatResponse,
  type CodePermissionMode,
  type CodeRange,
  type SessionRecord,
} from "../chat/client";
import { codeDraftKey, loadDraft, saveDraft } from "../chat/drafts";
import { formatRelative, titleFromMessage } from "../chat/format";
import { LoadingShell } from "../components/shells";

type CodeDashboard = Extract<ChatResponse, { type: "codeDashboard" }>;

const RANGES: { value: CodeRange; label: string }[] = [
  { value: "all", label: "All" },
  { value: "30d", label: "30d" },
  { value: "7d", label: "7d" },
];

const PERMISSIONS: { value: CodePermissionMode; label: string }[] = [
  { value: "default", label: "Default" },
  { value: "acceptEdits", label: "Accept edits" },
  { value: "plan", label: "Plan" },
  { value: "bypass", label: "Bypass permissions" },
];

function Starburst() {
  const rays = Array.from({ length: 8 }, (_, i) => {
    const a = (i * Math.PI * 2) / 8;
    return (
      <line
        key={i}
        x1={12 + Math.cos(a) * 3.4}
        y1={12 + Math.sin(a) * 3.4}
        x2={12 + Math.cos(a) * 10.6}
        y2={12 + Math.sin(a) * 10.6}
        stroke="currentColor"
        strokeWidth="2.6"
        strokeLinecap="round"
      />
    );
  });
  return (
    <svg width="40" height="40" viewBox="0 0 24 24" fill="none" className="starburst" aria-hidden="true">
      {rays}
    </svg>
  );
}

function FolderGlyph() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path
        d="M1.75 4.5A1.5 1.5 0 0 1 3.25 3h3l1.5 1.75h4.5a1.5 1.5 0 0 1 1.5 1.5v5.25a1.5 1.5 0 0 1-1.5 1.5H3.25a1.5 1.5 0 0 1-1.5-1.5V4.5Z"
        stroke="currentColor"
        strokeWidth="1.2"
      />
    </svg>
  );
}

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

function ChevronRight() {
  return (
    <svg width="12" height="12" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path d="m6 3.5 4.5 4.5L6 12.5" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

function formatCount(value: number): string {
  return new Intl.NumberFormat().format(value);
}

function folderName(path: string | null): string {
  if (!path) return "No folder selected";
  const trimmed = path.replace(/[\\/]+$/, "");
  if (!trimmed) return path;
  const parts = trimmed.split(/[\\/]/);
  return parts[parts.length - 1] || trimmed;
}

function peakLabel(hour: number | null): string {
  return hour === null ? "—" : `${String(hour).padStart(2, "0")}:00`;
}

function totalTokens(totals: { inputTokens: number; outputTokens: number }): number {
  return totals.inputTokens + totals.outputTokens;
}

function ActivityStrip({
  daily,
  truncated,
}: {
  daily: CodeDashboard["summary"]["daily"];
  truncated: boolean;
}) {
  if (daily.length === 0) {
    return <p className="code-empty-line">No activity in this range.</p>;
  }
  const totalEvents = daily.reduce((sum, day) => sum + day.totals.events, 0);
  const totalTokensAll = daily.reduce((sum, day) => sum + totalTokens(day.totals), 0);
  return (
    <div className="code-activity-wrap">
      <div
        className="code-activity"
        role="img"
        aria-label={`Daily Code activity for ${daily.length} days: ${formatCount(totalEvents)} events, ${formatCount(totalTokensAll)} tokens`}
      >
        {daily.map((day) => {
          const tokens = totalTokens(day.totals);
          const level =
            day.totals.events === 0
              ? 0
              : tokens > 100_000
                ? 4
                : tokens > 20_000
                  ? 3
                  : tokens > 2_000
                    ? 2
                    : 1;
          return (
            <span
              key={day.date}
              className={`code-day code-day-${level}`}
              title={`${day.date}: ${formatCount(day.totals.events)} events · ${formatCount(tokens)} tokens`}
            />
          );
        })}
      </div>
      {truncated ? (
        <p className="code-note">
          The all-time activity strip shows the most recent 10 years of daily buckets. The
          totals above include all Code activity, including earlier days not drawn here.
        </p>
      ) : null}
    </div>
  );
}

function Stat({
  label,
  value,
  hint,
}: {
  label: string;
  value: string;
  hint?: string;
}) {
  return (
    <div className="code-stat">
      <div className="code-stat-label">{label}</div>
      <div className="code-stat-value">{value}</div>
      {hint ? <div className="code-stat-hint">{hint}</div> : null}
    </div>
  );
}

function SessionRow({ session }: { session: SessionRecord }) {
  return (
    <Link to={`/epitaxy/${session.id}`} className="code-session-row">
      <span className="code-session-title">{session.title}</span>
      <span className="code-session-folder">{folderName(session.cwd)}</span>
      <span className="code-session-time">{formatRelative(session.lastActivityAt)}</span>
      <span className="code-session-chevron">
        <ChevronRight />
      </span>
    </Link>
  );
}

export function CodeDashboardScreen() {
  const navigate = useNavigate();
  const [range, setRange] = useState<CodeRange>("all");
  const [tab, setTab] = useState<"overview" | "models">("overview");
  const [dashboard, setDashboard] = useState<CodeDashboard | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<BridgeError | null>(null);
  const [folder, setFolder] = useState<string | null>(null);
  const [folderError, setFolderError] = useState<BridgeError | null>(null);
  const [folderNote, setFolderNote] = useState<string | null>(null);
  const [permission, setPermission] = useState<CodePermissionMode>("default");
  const [draft, setDraft] = useState(() => loadDraft(codeDraftKey()));
  const [creating, setCreating] = useState(false);
  const [createError, setCreateError] = useState<BridgeError | null>(null);
  const taRef = useRef<HTMLTextAreaElement>(null);
  const sendingRef = useRef(false);
  const loadRequest = useRef(0);

  const load = useCallback(async () => {
    const request = ++loadRequest.current;
    setLoading(true);
    const response = await chat({
      type: "codeDashboard",
      range,
      utcOffsetMinutes: -new Date().getTimezoneOffset(),
    });
    // A range switch can resolve out of order; only the newest request may
    // write the dashboard or its error.
    if (request !== loadRequest.current) return;
    if (response.type === "codeDashboard") {
      setDashboard(response);
      setError(null);
      // If policy changed while the screen was open, do not keep a selection
      // the backend will refuse. Functional update keeps `load` independent of
      // the current selector value.
      setPermission((current) =>
        current === "bypass" && !response.policy.bypassAllowed ? "default" : current,
      );
    } else if (response.type === "error") {
      setDashboard(null);
      setError(response.error);
    }
    setLoading(false);
  }, [range]);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    const onChange = () => void load();
    window.addEventListener("SESSIONS_CHANGED", onChange);
    return () => window.removeEventListener("SESSIONS_CHANGED", onChange);
  }, [load]);

  const policy = dashboard?.policy ?? null;
  const codeAvailable =
    policy !== null && policy.codeEnabled && policy.localSessionsEnabled;
  const bypassAllowed = policy?.bypassAllowed === true;
  const hasText = draft.trim().length > 0;
  const canSend = codeAvailable && !!folder && hasText && !creating;

  const summary = dashboard?.summary ?? null;
  const stats = useMemo(() => {
    if (!summary) return null;
    return {
      sessions: summary.sessions,
      messages: summary.messages,
      tokens: totalTokens(summary.totals),
      activeDays: summary.activeDays,
      peakHour: peakLabel(summary.peakHour),
      favoriteModel: summary.favoriteModel ?? "—",
    };
  }, [summary]);

  const chooseFolder = async () => {
    setFolderError(null);
    setFolderNote(null);
    setCreateError(null);
    const result = await pickWorkspaceFolder();
    if (result.kind === "picked") {
      setFolder(result.path);
      taRef.current?.focus();
      return;
    }
    if (result.kind === "cancelled") {
      setFolderNote("No folder was selected. Your current folder is unchanged.");
      return;
    }
    setFolderError(result.error);
  };

  const send = async () => {
    if (!canSend || sendingRef.current) return;
    const cwd = folder;
    if (!cwd) return;
    sendingRef.current = true;
    setCreating(true);
    setCreateError(null);
    const text = draft;
    const title = titleFromMessage(text);
    try {
      const response = await chat({
        type: "createCodeSession",
        title: title === "New chat" ? "New Code task" : title,
        cwd,
        permissionMode: permission,
      });
      if (response.type === "session") {
        // Keep the origin draft until the session screen has taken it over and
        // the first turn succeeds; an open/send failure must not lose the text.
        emitSessionsChanged();
        navigate(`/epitaxy/${response.session.session.id}`, {
          state: { pendingText: text, originKey: codeDraftKey() },
        });
      } else if (response.type === "error") {
        setCreateError(response.error);
        setDraft(text);
        saveDraft(codeDraftKey(), text);
      }
    } finally {
      sendingRef.current = false;
      setCreating(false);
    }
  };

  const updateDraft = (value: string) => {
    setDraft(value);
    setCreateError(null);
    saveDraft(codeDraftKey(), value);
  };

  if (loading && !dashboard) {
    return <LoadingShell label="Loading Code…" />;
  }

  const bridgeUnavailable = error?.code === "BRIDGE_UNAVAILABLE";

  return (
    <div className="code-dashboard">
      <header className="code-hero">
        <h1 className="greeting">
          What’s up next? <Starburst />
        </h1>
        <p className="code-subtitle">Run a Code session against a local folder.</p>
      </header>

      <div className="composer-card code-composer">
        <div className="code-context-row">
          <span className="code-placement">Local</span>
          <button
            type="button"
            className="code-folder-chip"
            title={folder ?? "Choose a folder"}
            onClick={() => void chooseFolder()}
          >
            <FolderGlyph />
            <span>{folderName(folder)}</span>
          </button>
          <button
            type="button"
            className="text-btn code-choose-folder"
            onClick={() => void chooseFolder()}
          >
            Choose folder
          </button>
        </div>
        <textarea
          ref={taRef}
          className="composer-input"
          placeholder="Describe a task or ask a question"
          value={draft}
          rows={1}
          aria-label="Code message"
          onChange={(event) => {
            updateDraft(event.target.value);
            const element = event.target;
            element.style.height = "auto";
            element.style.height = `${Math.min(element.scrollHeight, 200)}px`;
          }}
          onKeyDown={(event) => {
            if (event.key === "Enter" && !event.shiftKey) {
              event.preventDefault();
              if (!creating && !(event.nativeEvent as KeyboardEvent).isComposing) {
                void send();
              }
            }
          }}
        />
        <div className="composer-toolbar">
          <label className="code-permission">
            <span className="visually-hidden">Permission mode</span>
            <select
              value={permission}
              aria-label="Permission mode"
              onChange={(event) => {
                setCreateError(null);
                setPermission(event.target.value as CodePermissionMode);
              }}
            >
              {PERMISSIONS.map((option) => (
                <option
                  key={option.value}
                  value={option.value}
                  disabled={option.value === "bypass" && !bypassAllowed}
                  title={
                    option.value === "bypass" && !bypassAllowed
                      ? "Bypass permissions are disabled by policy"
                      : undefined
                  }
                >
                  {option.label}
                  {option.value === "bypass" && !bypassAllowed ? " (disabled)" : ""}
                </option>
              ))}
            </select>
          </label>
          <button
            type="button"
            className="text-btn code-fast"
            disabled
            title="Fast mode was not observed; it is deferred (docs/14 §9)"
          >
            Fast
          </button>
          <span className="composer-spacer" />
          <span className="composer-model" title="The model comes from the applied profile">
            Model from applied profile
          </span>
          <button
            type="button"
            className={`send-btn${canSend ? " primary" : ""}`}
            aria-label="Send"
            disabled={!canSend}
            onClick={() => void send()}
          >
            <ArrowUpIcon />
          </button>
        </div>
      </div>

      {folderError ? (
        <div className="composer-error" role="alert">
          <span className="error-code">{folderError.code}</span>
          <span>{folderError.message}</span>
        </div>
      ) : null}
      {folderNote ? <p className="code-note" role="status">{folderNote}</p> : null}
      {createError ? (
        <div className="composer-error" role="alert">
          <span className="error-code">{createError.code}</span>
          <span>{createError.message} Your draft and folder are kept.</span>
        </div>
      ) : null}
      {bridgeUnavailable ? (
        <div className="code-notice" role="status">
          <strong>Code is available in the desktop app.</strong>
          <span>This browser preview has no local session store or folder picker.</span>
        </div>
      ) : error ? (
        <div className="composer-error" role="alert">
          <span className="error-code">{error.code}</span>
          <span>{error.message}</span>
        </div>
      ) : null}
      {policy && !policy.codeEnabled ? (
        <div className="code-notice" role="status">
          Code is disabled by the applied configuration.
        </div>
      ) : null}
      {policy && policy.codeEnabled && !policy.localSessionsEnabled ? (
        <div className="code-notice" role="status">
          Local Code sessions are disabled. Remote placement is not implemented in this build.
        </div>
      ) : null}

      {dashboard && summary ? (
        <>
          <section className="code-overview" aria-label="Code overview">
            <div className="code-overview-head">
              <div className="code-tabs" role="tablist" aria-label="Code dashboard view">
                <button
                  type="button"
                  role="tab"
                  aria-selected={tab === "overview"}
                  className={`code-tab${tab === "overview" ? " code-tab-active" : ""}`}
                  onClick={() => setTab("overview")}
                >
                  Overview
                </button>
                <button
                  type="button"
                  role="tab"
                  aria-selected={tab === "models"}
                  className={`code-tab${tab === "models" ? " code-tab-active" : ""}`}
                  onClick={() => setTab("models")}
                >
                  Models
                </button>
              </div>
              <div className="code-ranges" role="group" aria-label="Time range">
                {RANGES.map((option) => (
                  <button
                    key={option.value}
                    type="button"
                    className={`code-range${range === option.value ? " code-range-active" : ""}`}
                    aria-pressed={range === option.value}
                    onClick={() => setRange(option.value)}
                  >
                    {option.label}
                  </button>
                ))}
              </div>
            </div>

            {stats ? (
              <div className="code-stats">
                <Stat label="Sessions" value={formatCount(stats.sessions)} />
                <Stat label="Messages" value={formatCount(stats.messages)} />
                <Stat label="Total tokens" value={formatCount(stats.tokens)} />
                <Stat label="Active days" value={formatCount(stats.activeDays)} />
                <Stat label="Peak hour" value={stats.peakHour} />
                <Stat label="Favorite model" value={stats.favoriteModel} />
              </div>
            ) : null}

            {tab === "overview" ? (
              <>
                <h2 className="code-section-title">Activity</h2>
                {summary.totals.events === 0 ? (
                  <p className="code-empty-line">
                    No Code usage events in this range
                    {summary.messages > 0 ? "; messages are counted above" : ""}.
                  </p>
                ) : (
                  <ActivityStrip daily={summary.daily} truncated={summary.truncated} />
                )}
              </>
            ) : summary.models.length === 0 ? (
              <p className="code-empty-line">No model usage in this range.</p>
            ) : (
              <div className="code-models">
                <div className="code-model-head">
                  <span>Model</span>
                  <span>Tokens</span>
                  <span>Share</span>
                </div>
                {summary.models.map((model) => (
                  <div key={model.model} className="code-model-row">
                    <span className="code-model-name">{model.model}</span>
                    <span>{formatCount(totalTokens(model.totals))}</span>
                    <span>{model.sharePercent.toFixed(1)}%</span>
                  </div>
                ))}
              </div>
            )}
          </section>

          <div className="code-columns">
            <section className="code-panel" aria-label="Recent Code sessions">
              <h2 className="code-section-title">Recent sessions</h2>
              {dashboard.sessions.length === 0 ? (
                <p className="code-empty-line">
                  No Code sessions yet. Choose a folder and send the first message.
                </p>
              ) : (
                dashboard.sessions.slice(0, 6).map((session) => (
                  <SessionRow key={session.id} session={session} />
                ))
              )}
            </section>
            <section className="code-panel" aria-label="Code folders">
              <h2 className="code-section-title">Folders</h2>
              {dashboard.repositories.length === 0 ? (
                <p className="code-empty-line">No folder-backed Code sessions yet.</p>
              ) : (
                dashboard.repositories.slice(0, 6).map((repository) => (
                  <button
                    key={repository.cwd}
                    type="button"
                    className="code-repo-row"
                    title={repository.cwd}
                    onClick={() => {
                      setFolder(repository.cwd);
                      setFolderNote(`Using ${repository.cwd}`);
                      setFolderError(null);
                      setCreateError(null);
                      taRef.current?.focus();
                    }}
                  >
                    <span className="code-repo-name">{folderName(repository.cwd)}</span>
                    <span className="code-repo-meta">
                      {repository.sessionCount} {repository.sessionCount === 1 ? "session" : "sessions"} ·{" "}
                      {formatRelative(repository.lastActivityAt)}
                    </span>
                  </button>
                ))
              )}
            </section>
          </div>
        </>
      ) : null}
    </div>
  );
}
