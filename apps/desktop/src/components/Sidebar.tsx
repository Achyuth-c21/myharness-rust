/**
 * Sidebar — structure per docs/screens/new-task.md §Sidebar, now matched to the
 * user-provided reference capture of the shipped home surface:
 *   1. Cowork|Code switch (with tab icons) at the top (docs/06 §4 "persistent in
 *      the sidebar");
 *   2. icon rows: New (+), Projects, Artifacts, Scheduled, Customize (docs/06 §2);
 *   3. Tasks header with disclosure chevron + search/filter icons (UI-INT-025
 *      triggers; the chevron is a section indicator, not a control);
 *   4. task rows (session data arrives in Phase 4 — empty list renders no rows);
 *   5. bottom: "Update available" banner above the profile row (docs/05 §2).
 * The profile row renders the starburst avatar, the account display-name slot
 * (identity/API arrives with the accounts phase — the label stays a neutral
 * placeholder and no personal name is baked into source), the "· Gateway"
 * provider label, and a disclosure chevron; its menu behavior is unchanged.
 */
import { useState } from "react";
import { NavLink } from "react-router-dom";
import { NAV_ITEMS } from "../routes";
import { ModeSwitch } from "./ModeSwitch";

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

  return (
    <nav className="sidebar" aria-label="Primary">
      <div className="sidebar-top">
        <ModeSwitch />
        <div className="sidebar-list">
          {NAV_ITEMS.map((item) => {
            const Icon = NAV_ICONS[item.path];
            return (
              <NavLink
                key={item.path}
                to={item.path}
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
            an interactive control. Filter is disabled until a list exists to filter
            (Unavailable state, docs/05 §3); search opens the palette (docs/06 §2). */}
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
              <button
                type="button"
                className="icon-btn icon-btn-sm"
                aria-label="Filter tasks"
                disabled
                title="Filter arrives with the task list (Phase 4 — docs/13)"
              >
                <FilterIcon />
              </button>
            </div>
          </div>
          {/* Task rows: no session store yet (Phase 4) — empty list, no faux data. */}
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
            <button
              type="button"
              role="menuitem"
              disabled
              title="Separate setup-desktop-3p window — docs/02 §3"
              className="account-item"
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
