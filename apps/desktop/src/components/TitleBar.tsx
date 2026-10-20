/**
 * Title bar — 36px high (docs/07 §1.4). Control set per docs/05 §2 and
 * docs/screens/new-task.md §Title bar, in observed order:
 * sidebar toggle (UI-INT-001), panel toggle (UI-INT-002), back/forward (UI-INT-003).
 * The center is a drag region; window controls stay inert until the bridge wires
 * the desktop/window APIs (crates/bridge, later phase — docs/08 §1 UI-INT-013).
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
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <rect x="1.5" y="2.5" width="13" height="11" rx="2" stroke="currentColor" strokeWidth="1.2" />
      <line x1="6" y1="2.5" x2="6" y2="13.5" stroke="currentColor" strokeWidth="1.2" />
    </svg>
  );
}

function PanelIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <rect x="1.5" y="2.5" width="13" height="11" rx="2" stroke="currentColor" strokeWidth="1.2" />
      <line x1="10" y1="2.5" x2="10" y2="13.5" stroke="currentColor" strokeWidth="1.2" />
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
  const navigateBack = 0; // replaced by history below
  void navigateBack;
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
        {/* UI-INT-002: panel toggle is present on the bar but no surface hosts an
            auxiliary pane yet (state = unavailable with runtime reason). */}
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
        {/* Wired via crates/bridge (core invoke) in a later phase; inert for now. */}
        <button type="button" className="icon-btn" aria-label="Minimize" disabled title="Minimize — via bridge later">
          –
        </button>
        <button type="button" className="icon-btn" aria-label="Maximize" disabled title="Maximize — via bridge later">
          ▢
        </button>
        <button type="button" className="icon-btn" aria-label="Close" disabled title="Close — via bridge later">
          ✕
        </button>
      </div>
    </header>
  );
}
