/**
 * Title bar — 36px high (docs/07 §1.4), frameless custom bar (docs/05 §2).
 * Left: sidebar hamburger toggle (UI-INT-001), back/forward (UI-INT-003).
 * Right: globe (web/browser context, task-detail.md §Layout) and panel toggle
 * (UI-INT-002) in the task-detail order [globe] [pane], then platform window
 * controls. Placement attested from the shipped app by the user (docs/14 §9
 * decision note in tests/report-phase1.md); docs/05 viewport table lists the
 * controls without a side. The center is a drag region.
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

function GlobeIcon() {
  // Globe = web/browser context (docs/screens/task-detail.md §Layout).
  return (
    <svg width="18" height="18" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <circle cx="8" cy="8" r="6.25" stroke="currentColor" strokeWidth="1.2" />
      <ellipse cx="8" cy="8" rx="2.85" ry="6.25" stroke="currentColor" strokeWidth="1.2" />
      <path d="M2 8h12M2.9 5h10.2M2.9 11h10.2" stroke="currentColor" strokeWidth="1.2" />
    </svg>
  );
}

function ArrowIcon({ dir }: { dir: "back" | "forward" }) {
  const d = dir === "back" ? "M10.5 3 5.5 8l5 5" : "M5.5 3l5 5-5 5";
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
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
        {/* Globe: web/browser-related context (task-detail.md §Layout). The context
            pane/tooling arrives in a later phase; disabled with a runtime reason. */}
        <button
          type="button"
          className="icon-btn"
          aria-label="Web context"
          disabled
          title="Web/browser context arrives with the web tooling phase"
        >
          <GlobeIcon />
        </button>
        {/* UI-INT-002: panel toggle; no surface hosts an auxiliary pane yet
            (state = unavailable with runtime reason). */}
        <button
          type="button"
          className="icon-btn"
          aria-label="Toggle panel"
          disabled
          title="No auxiliary pane on this surface"
        >
          <PanelIcon />
        </button>
        {/* Platform window controls (docs/05 §2: frameless custom bar on Linux).
            Wired via crates/bridge (core invoke) in a later phase; inert for now
            but rendered as real glyphs, not text approximations. */}
        <button type="button" className="win-btn" aria-label="Minimize" disabled title="Minimize — via bridge later">
          <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
            <path d="M1 5h8" stroke="currentColor" strokeWidth="1.1" />
          </svg>
        </button>
        <button type="button" className="win-btn" aria-label="Maximize" disabled title="Maximize — via bridge later">
          <svg width="10" height="10" viewBox="0 0 10 10" fill="none" aria-hidden="true">
            <rect x="1" y="1" width="8" height="8" rx="1.2" stroke="currentColor" strokeWidth="1.1" />
          </svg>
        </button>
        <button type="button" className="win-btn" aria-label="Close" disabled title="Close — via bridge later">
          <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
            <path d="m1.5 1.5 7 7m0-7-7 7" stroke="currentColor" strokeWidth="1.1" strokeLinecap="round" />
          </svg>
        </button>
      </div>
    </header>
  );
}
