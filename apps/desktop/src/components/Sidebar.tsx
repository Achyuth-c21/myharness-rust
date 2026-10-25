/**
 * Sidebar — structure per docs/screens/new-task.md §Sidebar, now matched to the
 * user-provided reference capture of the shipped home surface:
 *   1. Cowork|Code switch (with tab icons) at the top (docs/06 §4 "persistent in
 *      the sidebar");
 *   2. icon rows: New (+), Projects, Artifacts, Scheduled, Customize (docs/06 §2);
 *   3. Tasks header with disclosure chevron + search/filter icons (UI-INT-025
 *      triggers; the chevron is a section indicator, not a control);
 *   4. task rows from the local session store (Phase 4; empty until a task exists);
 *   5. bottom: "Update available" banner above the profile row (docs/05 §2).
 * The profile row renders the starburst avatar, the account display-name slot
 * (identity/API arrives with the accounts phase — the label stays a neutral
 * placeholder and no personal name is baked into source), the "· Gateway"
 * provider label, and a disclosure chevron; its menu behavior is unchanged.
 */
import { useEffect, useMemo, useState } from "react";
import { Link, NavLink, useLocation } from "react-router-dom";
import { NAV_ITEMS } from "../routes";
import { ModeSwitch } from "./ModeSwitch";
import { openSetupWindow } from "../setup/client";
import { useChatSessions } from "../chat/useSessions";
import { chat, type ProjectRecord } from "../chat/client";
import { formatRelative } from "../chat/format";
import {
  activeFilterCount,
  dateGroupLabel,
  DEFAULT_FILTERS,
  loadFilters,
  saveFilters,
  TaskFilterMenu,
  withinActivity,
  type TaskFilters,
} from "./TaskFilterMenu";

interface Props {
  onOpenSettings: () => void;
  onOpenSearch: () => void;
}

function PlusIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path d="M8 3.5v9M3.5 8h9" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" />
    </svg>
  );
}

function ProjectsIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      {/* archive-box glyph */}
      <rect x="2" y="2.5" width="12" height="4" rx="1.2" stroke="currentColor" strokeWidth="1.2" />
      <path d="M3.2 6.5v6a1.2 1.2 0 0 0 1.2 1.2h7.2a1.2 1.2 0 0 0 1.2-1.2v-6M6.4 9h3.2" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
    </svg>
  );
}

function ArtifactsIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      {/* stacked shapes glyph */}
      <rect x="2.5" y="6" width="7" height="7" rx="1.5" stroke="currentColor" strokeWidth="1.2" />
      <path d="M6.5 6V3.5A1.5 1.5 0 0 1 8 2h3.5A1.5 1.5 0 0 1 13 3.5V7A1.5 1.5 0 0 1 11.5 8.5H9.5" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
    </svg>
  );
}

function ScheduledIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      {/* clock glyph */}
      <circle cx="8" cy="8" r="5.75" stroke="currentColor" strokeWidth="1.2" />
      <path d="M8 4.8V8l2.2 1.6" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

function CustomizeIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      {/* toolbox glyph */}
      <rect x="2" y="5.5" width="12" height="8" rx="1.5" stroke="currentColor" strokeWidth="1.2" />
      <path d="M6 5.5V4.2A1.2 1.2 0 0 1 7.2 3h1.6A1.2 1.2 0 0 1 10 4.2v1.5M2 8.5h12" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
    </svg>
  );
}

function SearchIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <circle cx="7" cy="7" r="4.25" stroke="currentColor" strokeWidth="1.2" />
      <path d="m10.2 10.2 3 3" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
    </svg>
  );
}

function FilterIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path d="M2.5 4h11M4.5 8h7M6.5 12h3" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
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

function ChevronDown() {
  return (
    <svg width="12" height="12" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path d="m4 6.5 4 4 4-4" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

function TaskIcon() {
  return (
    <svg width="15" height="15" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path
        d="M2.5 4.5A1.5 1.5 0 0 1 4 3h8a1.5 1.5 0 0 1 1.5 1.5v5A1.5 1.5 0 0 1 12 11H7l-3 2.5V11H4a1.5 1.5 0 0 1-1.5-1.5v-5Z"
        stroke="currentColor"
        strokeWidth="1.2"
        strokeLinejoin="round"
      />
    </svg>
  );
}

function ArrowRight() {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path d="M3 8h10M13 8l-3.6-3.6M13 8l-3.6 3.6" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

function AvatarMark() {
  // The shipped profile row opens with the orange starburst mark (reference
  // capture); docs/07 §1.5: the starburst uses the orange accent.
  const rays = Array.from({ length: 8 }, (_, i) => {
    const a = (i * Math.PI * 2) / 8;
    return (
      <line
        key={i}
        x1={12 + Math.cos(a) * 3.2}
        y1={12 + Math.sin(a) * 3.2}
        x2={12 + Math.cos(a) * 10}
        y2={12 + Math.sin(a) * 10}
        stroke="currentColor"
        strokeWidth="2.2"
        strokeLinecap="round"
      />
    );
  });
  return (
    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" className="starburst" aria-hidden="true">
      {rays}
    </svg>
  );
}

const NAV_ICONS: Record<string, () => JSX.Element> = {
  "/new": PlusIcon,
  "/projects": ProjectsIcon,
  "/cowork-artifact": ArtifactsIcon,
  "/scheduled-task": ScheduledIcon,
};

export function Sidebar({ onOpenSettings, onOpenSearch }: Props) {
  const [accountOpen, setAccountOpen] = useState(false);
  const { pathname } = useLocation();
  const codeMode = pathname.startsWith("/epitaxy");
  const surface = codeMode ? "code" : "chat";
  // The Status filter chooses which side of the archive the list shows; the
  // remaining filters compose client-side over those rows (AND, immediate).
  const [filters, setFilters] = useState<TaskFilters>(loadFilters);
  const [filterMenuOpen, setFilterMenuOpen] = useState(false);
  const [projects, setProjects] = useState<ProjectRecord[]>([]);
  const { sessions, unavailable } = useChatSessions(
    filters.status === "archived" ? true : false,
    surface,
  );

  useEffect(() => {
    saveFilters(filters);
  }, [filters]);

  // Group-by-Project needs the project names; fetch lazily for that mode only.
  useEffect(() => {
    if (filters.groupBy !== "project") return;
    let cancelled = false;
    void chat({ type: "listProjects" }).then((response) => {
      if (cancelled) return;
      if (response.type === "projects") setProjects(response.projects);
    });
    return () => {
      cancelled = true;
    };
  }, [filters.groupBy]);

  const matching = useMemo(
    () =>
      sessions.filter(
        (session) =>
          (filters.type === "all" || session.surface === (filters.type === "code" ? "code" : "chat")) &&
          withinActivity(filters, session.lastActivityAt),
      ),
    [sessions, filters],
  );

  const ordered = useMemo(() => {
    const rows = [...matching];
    if (filters.sortBy === "name") rows.sort((a, b) => a.title.localeCompare(b.title));
    else if (filters.sortBy === "created") rows.sort((a, b) => b.createdAt - a.createdAt);
    else rows.sort((a, b) => b.lastActivityAt - a.lastActivityAt);
    return rows;
  }, [matching, filters.sortBy]);

  const projectName = (session: (typeof sessions)[number]): string => {
    if (!session.projectId) return "No project";
    return projects.find((project) => project.id === session.projectId)?.name ?? "Project missing";
  };

  // Grouped view: contiguous groups in the ordered row order, each with a
  // header row. Date uses the captured recency vocabulary; Project and
  // Status group by their value; None renders no headers.
  const grouped = useMemo(() => {
    if (filters.groupBy === "none") return null;
    const groups: { label: string; rows: typeof ordered }[] = [];
    for (const row of ordered) {
      const label =
        filters.groupBy === "date"
          ? dateGroupLabel(row.lastActivityAt)
          : filters.groupBy === "project"
            ? projectName(row)
            : row.archived
              ? "Archived"
              : "Active";
      const last = groups[groups.length - 1];
      if (last && last.label === label) last.rows.push(row);
      else groups.push({ label, rows: [row] });
    }
    // Group by Date/Project keep recency order; grouping by Name should not
    // resurrect the sort order, so groups follow the applied sort.
    return groups;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ordered, filters.groupBy, projects]);

  const filterBadge = activeFilterCount(filters);

  return (
    <nav className="sidebar" aria-label="Primary">
      <div className="sidebar-top">
        <ModeSwitch />
        <div className="sidebar-list">
          {NAV_ITEMS.map((item) => {
            const Icon = NAV_ICONS[item.path];
            const to = item.path === "/new" && codeMode ? "/epitaxy" : item.path;
            return (
              <NavLink
                key={item.path}
                to={to}
                className={({ isActive }) => `nav-item${isActive ? " active" : ""}`}
              >
                {Icon ? <Icon /> : null}
                <span>{item.label}</span>
              </NavLink>
            );
          })}
          <button
            type="button"
            className="nav-item nav-button"
            disabled
            title="Customize route was not captured in the evidence — docs/06 §1"
          >
            <CustomizeIcon />
            <span>Customize</span>
          </button>
        </div>

        {/* Tasks header + independent list scroll (docs/05 §2 sidebar scrolls alone).
            The chevron is the section's disclosure indicator in the reference, not
            an interactive control. Filter composes immediately (UI-SCREEN-23). */}
        <div className="sidebar-scroll">
          <div className="tasks-head">
            <span className="tasks-title">
              Tasks
              <ChevronRight />
            </span>
            <div className="tasks-tools">
              <button
                type="button"
                className="icon-btn icon-btn-sm"
                aria-label="Search tasks"
                title="Search (Ctrl+K)"
                onClick={onOpenSearch}
              >
                <SearchIcon />
              </button>
              <span className="filter-anchor">
                <button
                  type="button"
                  className="icon-btn icon-btn-sm"
                  aria-label="Filter tasks"
                  aria-haspopup="menu"
                  aria-expanded={filterMenuOpen}
                  title="Filter tasks"
                  onClick={() => setFilterMenuOpen((open) => !open)}
                >
                  <FilterIcon />
                  {filterBadge > 0 ? (
                    <span className="filter-badge" aria-label={`${filterBadge} active filters`} />
                  ) : null}
                </button>
                {filterMenuOpen ? (
                  <TaskFilterMenu
                    filters={filters}
                    onChange={setFilters}
                    surface={surface}
                    onClose={() => setFilterMenuOpen(false)}
                  />
                ) : null}
              </span>
            </div>
          </div>
          {/* Filtered-empty (docs/screens/search-palette.md): with non-default
              filters and no rows, say so and offer the reset — never the plain
              empty state, which belongs to a store with no tasks at all. */}
          {filterBadge > 0 && !unavailable && ordered.length === 0 ? (
            <div className="tasks-filtered-empty">
              <span>No tasks</span>
              <button type="button" className="text-btn" onClick={() => setFilters(DEFAULT_FILTERS)}>
                Reset filters
              </button>
            </div>
          ) : null}
          {(grouped ?? ordered.map((row) => ({ label: null as string | null, rows: [row] }))).map(
            (group, groupIndex) => (
              <div key={group.label ?? `g${groupIndex}`} className="task-group">
                {group.label !== null ? (
                  <div className="task-group-label">{group.label}</div>
                ) : null}
                {group.rows.map((session) => (
                  <Link
                    key={session.id}
                    to={`${codeMode ? "/epitaxy" : "/cowork"}/${session.id}`}
                    className="task-row"
                  >
                    <span className="task-icon">
                      <TaskIcon />
                    </span>
                    <span className="task-title">{session.title}</span>
                    <span className="task-time">{formatRelative(session.lastActivityAt)}</span>
                  </Link>
                ))}
              </div>
            ),
          )}
        </div>
      </div>

      <div className="account">
        {/* "Update available" banner above the profile row (docs/05 §2/§4.1).
            The update flow arrives with the updates module (FEAT-SHELL-10/
            docs/13) — presented as a disabled control with a runtime reason. */}
        <button
          type="button"
          className="update-banner"
          disabled
          title="Update flow arrives with the updates module (docs/13)"
        >
          <span>Update available</span>
          <ArrowRight />
        </button>

        {/* Profile row: starburst avatar + display-name slot + provider label +
            disclosure chevron, per the reference. The display name renders from
            the accounts phase (neutral placeholder until then — no personal name
            is baked into source; CHANGE-ME guard in the report). The menu
            behavior is unchanged. */}
        <button
          type="button"
          className="nav-item nav-button account-row"
          aria-expanded={accountOpen}
          onClick={() => setAccountOpen((open) => !open)}
        >
          <AvatarMark />
          <span className="profile-name">Account</span>
          <span className="profile-provider">· Gateway</span>
          <span className="account-spacer" />
          <span className={`profile-chevron${accountOpen ? " open" : ""}`}>
            <ChevronDown />
          </span>
        </button>
        {accountOpen ? (
          <div className="account-menu" role="menu">
            <button type="button" role="menuitem" onClick={onOpenSettings} className="account-item">
              Settings
            </button>
            <button
              type="button"
              role="menuitem"
              disabled
              title="Language dialog — later phase (docs/05)"
              className="account-item"
            >
              Language
            </button>
            {/* The inference configuration lives in its own window
                (docs/09 §2.1: "a separate Electron window, not a modal in the main
                renderer"; docs/02 §3). Opening it is a bridge action, not a route. */}
            <button
              type="button"
              role="menuitem"
              className="account-item"
              onClick={() => void openSetupWindow()}
            >
              Inference configuration
            </button>
            <button type="button" role="menuitem" disabled className="account-item">
              Changelog
            </button>
            <button type="button" role="menuitem" disabled className="account-item">
              Learn more
            </button>
            <button type="button" role="menuitem" disabled className="account-item">
              Sign out
            </button>
          </div>
        ) : null}
      </div>
    </nav>
  );
}
