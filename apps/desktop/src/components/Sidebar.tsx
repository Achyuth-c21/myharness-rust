/**
 * Sidebar — ~321px at default width (docs/07 §1.4), nav item fills per UI-INT-006.
 * Items per docs/06 §2: New, Projects, Artifacts, Scheduled. Customize opens an in-app
 * view whose route was not captured (docs/06 §1) — rendered disabled until specified.
 * The account row opens a menu per UI-INT-011 (Settings, Language, Inference
 * configuration, changelog, learn more, sign out).
 */
import { useState } from "react";
import { NavLink } from "react-router-dom";
import { NAV_ITEMS } from "../routes";

interface Props {
  onOpenSettings: () => void;
}

export function Sidebar({ onOpenSettings }: Props) {
  const [accountOpen, setAccountOpen] = useState(false);

  return (
    <nav className="sidebar" aria-label="Primary">
      <div className="sidebar-list">
        {NAV_ITEMS.map((item) => (
          <NavLink
            key={item.path}
            to={item.path}
            className={({ isActive }) => `nav-item${isActive ? " active" : ""}`}
          >
            <span>{item.label}</span>
            {item.path === "/new" ? <span className="nav-shortcut">Ctrl+N</span> : null}
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

      <div className="account">
        <button
          type="button"
          className="nav-item nav-button account-row"
          aria-expanded={accountOpen}
          onClick={() => setAccountOpen((open) => !open)}
        >
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
