/**
 * Search palette — UI-INT-020..028 (docs/08 §2), docs/screens/search-palette.md
 * and docs/features/search-and-filter.md (Phase 6 slice 5).
 *
 * Centered overlay ~726×470, strong shadow, scrim over the app while the
 * sidebar stays visible; 48px search row with a leading magnifier, placeholder
 * "Search chats and projects", and a clear button; result rows = surface icon +
 * title (+ right-side relative time). Selected row is a light-gray rounded
 * highlight across the row; "No results" keeps the query editable.
 *
 * Phase 6: the results are LOCAL DATA — Cowork and Code sessions plus
 * projects (REQ-SEARCH-01/02/03). Search is client-side over the metadata the
 * bridge already returned; queries and content never leave the device and are
 * never sent to telemetry (REQ-SEARCH-03). Transcript-content search is not
 * built: the shipped transcript-search worker was never observed, so this
 * palette searches titles/names only (recorded in docs/14 §9).
 *
 * The route entries ("Go to") are a rebuild-defined fallback, shown when the
 * local data is unavailable (the browser preview) or as a labelled tail after
 * data results. They are navigation, never claimed as the shipped palette's
 * action rows.
 */
import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { chat, type ProjectRecord, type SessionRecord } from "../chat/client";
import { formatRelative } from "../chat/format";
import { SCREENS } from "../routes";
import { useFocusTrap } from "./focusTrap";

interface Props {
  onClose: () => void;
}

interface PaletteData {
  loaded: boolean;
  sessions: SessionRecord[];
  projects: ProjectRecord[];
}

type ResultRow =
  | { kind: "session"; session: SessionRecord }
  | { kind: "project"; project: ProjectRecord }
  | { kind: "route"; path: string; label: string };

function Magnifier() {
  return (
    <svg width="15" height="15" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <circle cx="7" cy="7" r="4.25" stroke="currentColor" strokeWidth="1.2" />
      <path d="m10.2 10.2 3 3" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
    </svg>
  );
}

function ClearGlyph() {
  return (
    <svg width="12" height="12" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path d="m4 4 8 8M12 4l-8 8" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" />
    </svg>
  );
}

function ListGlyph() {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path d="M2.5 4.5h11M2.5 8h11M2.5 11.5h7" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
    </svg>
  );
}

function CodeGlyph() {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path d="m5.5 4.5-3 3.5 3 3.5M10.5 4.5l3 3.5-3 3.5" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

function FolderGlyph() {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path
        d="M1.75 4.5A1.5 1.5 0 0 1 3.25 3h3l1.5 1.75h4.5a1.5 1.5 0 0 1 1.5 1.5v5.25a1.5 1.5 0 0 1-1.5 1.5H3.25a1.5 1.5 0 0 1-1.5-1.5V4.5Z"
        stroke="currentColor"
        strokeWidth="1.2"
      />
    </svg>
  );
}

function RowIcon({ row }: { row: ResultRow }) {
  if (row.kind === "project") return <FolderGlyph />;
  if (row.kind === "session" && row.session.surface === "code") return <CodeGlyph />;
  return <ListGlyph />;
}

function rowTarget(row: ResultRow): string {
  if (row.kind === "session") {
    return row.session.surface === "code"
      ? `/epitaxy/${row.session.id}`
      : `/cowork/${row.session.id}`;
  }
  if (row.kind === "project") return `/projects/${row.project.id}`;
  return row.path;
}

function rowTitle(row: ResultRow): string {
  if (row.kind === "session") return row.session.title;
  if (row.kind === "project") return row.project.name;
  return row.label;
}

function rowTime(row: ResultRow): string | null {
  if (row.kind === "session") return formatRelative(row.session.lastActivityAt);
  if (row.kind === "project") return formatRelative(row.project.lastActivityAt);
  return null;
}

const ROUTE_TAIL_LIMIT = 4;

export function SearchPalette({ onClose }: Props) {
  const navigate = useNavigate();
  const inputRef = useRef<HTMLInputElement>(null);
  const trapRef = useRef<HTMLDivElement>(null);
  const [query, setQuery] = useState("");
  const [index, setIndex] = useState(0);
  const [data, setData] = useState<PaletteData>({ loaded: false, sessions: [], projects: [] });
  // UI-INT-026 + docs/06 §5: the palette traps focus and restores it to the trigger.
  useFocusTrap(trapRef, true);

  useEffect(() => {
    let cancelled = false;
    // Both surfaces plus projects, in parallel. A failure degrades that
    // group to absent — never a fabricated row.
    void Promise.all([
      chat({ type: "listSessions", archived: null }),
      chat({ type: "listCodeSessions", archived: null }),
      chat({ type: "listProjects" }),
    ]).then(([chatRows, codeRows, projectRows]) => {
      if (cancelled) return;
      const sessions = [
        ...(chatRows.type === "sessions" ? chatRows.sessions : []),
        ...(codeRows.type === "sessions" ? codeRows.sessions : []),
      ].sort((a, b) => b.lastActivityAt - a.lastActivityAt);
      const projects = projectRows.type === "projects" ? projectRows.projects : [];
      setData({ loaded: true, sessions, projects });
    });
    return () => {
      cancelled = true;
    };
  }, []);

  const results = useMemo<ResultRow[]>(() => {
    const queryTrimmed = query.trim().toLowerCase();
    const sessionRows = (match: (s: SessionRecord) => boolean) =>
      data.sessions.filter(match).map((session) => ({ kind: "session" as const, session }));
    const projectRows = (match: (p: ProjectRecord) => boolean) =>
      data.projects.filter(match).map((project) => ({ kind: "project" as const, project }));
    let dataRows: ResultRow[];
    if (queryTrimmed) {
      dataRows = [
        ...sessionRows((session) => session.title.toLowerCase().includes(queryTrimmed)),
        ...projectRows((project) => project.name.toLowerCase().includes(queryTrimmed)),
      ];
    } else {
      // Empty query: the recent set (strong inference per the feature doc).
      dataRows = [
        ...sessionRows(() => true).slice(0, 6),
        ...projectRows(() => true).slice(0, 4),
      ];
    }
    // The labelled navigation tail: full when no local data exists (browser
    // preview), otherwise a short tail after the data results.
    const tail: ResultRow[] = dataRows.some((row) => row.kind !== "route")
      ? []
      : SCREENS.filter(
          (screen) => !queryTrimmed || screen.label.toLowerCase().includes(queryTrimmed),
        )
          .slice(0, ROUTE_TAIL_LIMIT)
          .map((screen) => ({ kind: "route" as const, path: screen.path, label: screen.label }));
    return [...dataRows, ...tail];
  }, [query, data]);

  // Keep the highlight inside the live result set.
  useEffect(() => {
    setIndex((current) => Math.min(current, Math.max(results.length - 1, 0)));
  }, [results.length]);

  // UI-INT-022: the highlighted row stays visible while arrowing.
  useEffect(() => {
    trapRef.current
      ?.querySelector(".palette-row.highlighted")
      ?.scrollIntoView({ block: "nearest" });
  }, [index]);

  const open = (row: ResultRow) => {
    navigate(rowTarget(row));
    onClose();
  };

  const onKeyDown = (event: React.KeyboardEvent) => {
    // UI-INT-022: arrows move highlight and keep it visible; UI-INT-023: Enter opens.
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setIndex((i) => Math.min(i + 1, Math.max(results.length - 1, 0)));
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setIndex((i) => Math.max(i - 1, 0));
    } else if (event.key === "Enter") {
      event.preventDefault();
      const selected = results[index];
      if (selected) open(selected);
    } else if (event.key === "Escape") {
      event.preventDefault();
      // docs/08 §13: Escape closes the TOPMOST layer only — keep the event from
      // reaching underneath (e.g. the settings modal when both are open).
      event.stopPropagation();
      onClose();
    }
  };

  return (
    <div
      className="overlay-backdrop"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div className="palette" role="dialog" aria-label="Search" aria-modal="true" onKeyDown={onKeyDown} ref={trapRef}>
        <div className="palette-search-row">
          <span className="palette-glyph">
            <Magnifier />
          </span>
          <input
            ref={inputRef}
            className="palette-input"
            placeholder="Search chats and projects"
            value={query}
            aria-label="Search chats and projects"
            onChange={(event) => {
              setQuery(event.target.value);
              setIndex(0);
            }}
          />
          {/* Clear keeps the palette open (search-palette.md behavior table). */}
          {query ? (
            <button
              type="button"
              className="icon-btn icon-btn-sm"
              aria-label="Clear search"
              onClick={() => {
                setQuery("");
                setIndex(0);
                inputRef.current?.focus();
              }}
            >
              <ClearGlyph />
            </button>
          ) : null}
        </div>
        <div className="palette-results" aria-live="polite">
          {results.length === 0 ? (
            <div className="palette-empty" title={query.trim()}>
              No results
            </div>
          ) : (
            results.map((row, i) => (
              <button
                key={row.kind === "route" ? row.path : `${row.kind}-${rowTitle(row)}`}
                type="button"
                className={`palette-row${i === index ? " highlighted" : ""}`}
                onMouseEnter={() => setIndex(i)}
                onClick={() => open(row)}
                title={rowTitle(row)}
              >
                <span className="palette-glyph">
                  <RowIcon row={row} />
                </span>
                <span className="palette-title">{rowTitle(row)}</span>
                {/* Right-side relative time from the row's own metadata. */}
                <span className="palette-time">{rowTime(row) ?? ""}</span>
                {row.kind === "route" && data.sessions.length + data.projects.length > 0 ? (
                  <span className="palette-route-mark">Go to</span>
                ) : null}
              </button>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
