/**
 * Sidebar — structure per docs/screens/new-task.md §Sidebar (verified layout):
 *   1. Cowork|Code switch at the top (docs/06 §4 "persistent in the sidebar");
 *   2. nav New(PMR "＋ New"), Projects, Artifacts, Scheduled, Customize (docs/06 §2);
 *   3. Tasks header with search + filter icons (UI-INT-025 triggers);
 *   4. task rows (session data arrives in Phase 4 — empty list renders no rows);
 *   5. bottom: update banner region, account row pinned (docs/05 §2 "account/update
 *      pinned to sidebar bottom").
 * Customize opens an in-app view whose route was not captured (docs/06 §1) — disabled
 * until specified. Identity/display name + provider arrive with profiles (Phase 2).
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
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path d="M8 3.5v9M3.5 8h9" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
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

function AccountIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <circle cx="8" cy="5.25" r="2.4" stroke="currentColor" strokeWidth="1.2" />
      <path d="M3.2 13c.7-2.2 2.6-3.4 4.8-3.4s4.1 1.2 4.8 3.4" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
    </svg>
  );
}

export function Sidebar({ onOpenSettings, onOpenSearch }: Props) {
  const [accountOpen, setAccountOpen] = useState(false);

  return (
    <nav className="sidebar" aria-label="Primary">
      <div className="sidebar-top">
        <ModeSwitch />
        <div className="sidebar-list">
          {NAV_ITEMS.map((item) => (
            <NavLink
              key={item.path}
              to={item.path}
              className={({ isActive }) => `nav-item${isActive ? " active" : ""}`}
            >
              {item.path === "/new" ? <PlusIcon /> : null}
              <span>{item.label}</span>
            </NavLink>
          ))}
          <button
            type="button"
            className="nav-item nav-button"
            disabled
            title="Customize route was not captured in the evidence — docs/06 §1"
          >
            <span>Customize</span>
          </button>
        </div>

        {/* Tasks header + independent list scroll (docs/05 §2 sidebar scrolls alone).
            Filter is disabled until a list exists to filter (Unavailable state,
            docs/05 §3); search opens the palette (docs/06 §2 search source). */}
        <div className="sidebar-scroll">
          <div className="tasks-head">
            <span className="tasks-title">Tasks</span>
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
        {/* Update banner region (docs/05 §3/§4.1) — renders only when an update exists. */}
        <button
          type="button"
          className="nav-item nav-button account-row"
          aria-expanded={accountOpen}
          onClick={() => setAccountOpen((open) => !open)}
        >
          <AccountIcon />
          <span>Account</span>
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
