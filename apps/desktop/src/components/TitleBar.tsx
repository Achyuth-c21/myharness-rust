/**
 * Title bar — 36px high (docs/07 §1.4), frameless custom bar (docs/05 §2).
 * Left cluster in the reference order: sidebar hamburger (UI-INT-001), panel
 * toggle (UI-INT-002), back/forward (UI-INT-003) — matching the user-provided
 * capture of the shipped home surface. The home bar carries no globe; web/window
 * context stays a task-surface affordance (task-detail.md §Layout).
 * Right: a single round close control at the corner — the only window control
 * visible in the reference (no min/max at this size). Inert until the
 * desktop/window bridge lands (UI-INT-013 area, crates/bridge).
 */
import { useEffect, useState } from "react";
import { useLocation } from "react-router-dom";

interface Props {
  sidebarOpen: boolean;
  onToggleSidebar: () => void;
}

/* Global history depth tracker for greyed ends (module-scoped, app lifetime). */
let histDepth = 0;
let histMax = 0;

function useHistoryDepth(): { canBack: boolean; canForward: boolean } {
  const { pathname } = useLocation();
  const [depth, setDepth] = useState(0);
  const [max, setMax] = useState(0);
  useEffect(() => {
    histDepth += 1;
    histMax = Math.max(histMax, histDepth);
    setDepth(histDepth);
    setMax(histMax);
  }, [pathname]);
  return { canBack: depth >= 2, canForward: depth < max };
}

/* Icon-only SVG glyphs: aria-labels live on the buttons; glyphs are decorative. */

function SidebarIcon() {
  // Hamburger glyph (3 equal lines) as attested from the shipped app's sidebar
  // toggle; docs/07 §1.5 leaves exact codepoints to the icon set. 18px in a
  // 28px ghost button (docs/07 §3.1 ghost spec).
  return (
    <svg width="18" height="18" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path d="M2 4.25h12M2 8h12M2 11.75h12" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
    </svg>
  );
}

function PanelIcon() {
  return (
    <svg width="18" height="18" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <rect x="1.5" y="2.5" width="13" height="11" rx="2" stroke="currentColor" strokeWidth="1.2" />
      <line x1="10" y1="2.5" x2="10" y2="13.5" stroke="currentColor" strokeWidth="1.2" />
    </svg>
  );
}

function ArrowIcon({ dir }: { dir: "back" | "forward" }) {
  const d = dir === "back" ? "M10.5 3 5.5 8l5 5" : "M5.5 3l5 5-5 5";
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path d={d} stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

export function TitleBar({ sidebarOpen, onToggleSidebar }: Props) {
  const { canBack, canForward } = useHistoryDepth();

  return (
    <header className="titlebar" data-tauri-drag-region>
      <div className="titlebar-left">
        <button
          type="button"
          className="icon-btn"
          aria-label="Toggle sidebar"
          aria-pressed={sidebarOpen}
          title="Toggle sidebar (Ctrl+B)"
          onClick={onToggleSidebar}
        >
          <SidebarIcon />
        </button>
        {/* UI-INT-002: panel toggle sits between the hamburger and Back in the
            reference; no surface hosts an auxiliary pane yet (state =
            unavailable with runtime reason). */}
        <button
          type="button"
          className="icon-btn"
          aria-label="Toggle panel"
          disabled
          title="No auxiliary pane on this surface"
        >
          <PanelIcon />
        </button>
        <button
          type="button"
          className="icon-btn"
          aria-label="Back"
          title="Back"
          disabled={!canBack}
          onClick={() => window.history.back()}
        >
          <ArrowIcon dir="back" />
        </button>
        <button
          type="button"
          className="icon-btn"
          aria-label="Forward"
          title="Forward"
          disabled={!canForward}
          onClick={() => window.history.forward()}
        >
          <ArrowIcon dir="forward" />
        </button>
      </div>

      {/* Center is intentionally empty (drag region only). The Cowork|Code switch
          lives in the sidebar per docs/06 §4 / docs/screens/new-task.md. */}
      <div className="titlebar-center" data-tauri-drag-region />

      <div className="titlebar-right">
        {/* Single round close at the corner — the reference's own window-control
            arrangement (no min/max visible on this surface). Wired via the
            desktop/window bridge in a later phase; inert with a reason for now. */}
        <button type="button" className="win-close" aria-label="Close" disabled title="Close — via bridge later">
          <svg width="12" height="12" viewBox="0 0 10 10" aria-hidden="true">
            <path d="m2 2 6 6m0-6-6 6" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" />
          </svg>
        </button>
      </div>
    </header>
  );
}
