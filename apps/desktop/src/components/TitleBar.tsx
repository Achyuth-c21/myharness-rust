/**
 * Title bar — 36px high (docs/07 §1.4), frameless custom bar (docs/05 §2).
 * Left cluster in the reference order: panel toggle (UI-INT-002), sidebar
 * toggle (UI-INT-001), back/forward (UI-INT-003) — matching the user-provided
 * capture of the shipped home surface. The two left controls keep their
 * positions and glyphs; the reference pairs the three-line "menu" glyph with
 * the panel toggle and the framed-pane glyph with the sidebar toggle, so the
 * control that collapses the whole left column is the one beside the three
 * lines, not the three lines themselves (the shipped app's top-left three-line
 * control is a *Menu* button; its sidebar is toggled by Cmd/Ctrl+B).
 * The home bar carries no globe; web/window
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

/* History-position tracker for greyed ends (UI-INT-003: "disabled/greyed at history
   ends"). React Router maintains `history.state.idx` (init 0, +1 per push), so read
   the browser's own position instead of guessing from visit counts — the earlier
   monotonic counter left Forward permanently dead and Back never re-greyed at the
   first entry (report-bug-hunt A1). maxIdx = the furthest idx seen this session;
   Forward is live only strictly before it. */
let maxIdx = 0;

function useHistoryButtons(): { canBack: boolean; canForward: boolean } {
  const { pathname, hash, key } = useLocation();
  const [buttons, setButtons] = useState({ canBack: false, canForward: false });
  useEffect(() => {
    const read = () => {
      const idx = (window.history.state as { idx?: number } | null)?.idx;
      if (typeof idx !== "number") return; // foreign/absent state: stay inert, fail closed
      if (idx > maxIdx) maxIdx = idx;
      setButtons({ canBack: idx > 0, canForward: idx < maxIdx });
    };
    read();
    window.addEventListener("popstate", read); // back/forward themselves recompute
    return () => window.removeEventListener("popstate", read);
  }, [pathname, hash, key]);
  return buttons;
}

/* Icon-only SVG glyphs: aria-labels live on the buttons; glyphs are decorative. */

function MenuLinesIcon() {
  // Three-line glyph (3 equal lines). Decorative; the button carries the
  // accessible label. It sits on the panel toggle — in the shipped app the
  // top-left three-line control is a *Menu* button, and the sidebar itself is
  // toggled by Cmd/Ctrl+B (app menu "Show/Hide Sidebar"), so the three lines
  // never stood for "collapse the whole side panel". docs/07 §1.5 leaves exact
  // codepoints to the icon set.
  return (
    <svg width="18" height="18" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path d="M2 4.25h12M2 8h12M2 11.75h12" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
    </svg>
  );
}

function PanelIcon() {
  // Framed pane with a divider — the glyph of the control that owns the whole
  // left column (the sidebar). docs/07 §1.5 leaves exact codepoints to the
  // icon set.
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
  const { canBack, canForward } = useHistoryButtons();

  return (
    <header className="titlebar" data-tauri-drag-region>
      <div className="titlebar-left">
        {/* The two left controls keep their positions and glyphs; their FUNCTIONS
            are the reference's: the three-line control is the panel toggle
            (UI-INT-002) and the framed-pane control beside it is the sidebar
            toggle (UI-INT-001) that collapses/expands the whole left column.
            The three lines are the shipped app's top-left *Menu* button; the
            sidebar is toggled by Cmd/Ctrl+B there, so the three lines never
            collapsed the side panel. See docs/14 §3 (title-bar glyph↔role
            pairing) and docs/19 IMP-28. */}
        <button
          type="button"
          className="icon-btn"
          aria-label="Toggle panel"
          aria-disabled="true"
          title="No auxiliary pane on this surface"
        >
          <MenuLinesIcon />
        </button>
        {/* UI-INT-002/UI-INT-001: no surface hosts an auxiliary pane yet, so the
            panel toggle is unavailable — but discoverable: a native `disabled`
            button is unfocusable and shows no tooltip, so its runtime reason was
            unreachable (report-bug-hunt C10). aria-disabled keeps interaction
            off while focus, hover, and assistive tech still reach the row. */}
        <button
          type="button"
          className="icon-btn"
          aria-label="Toggle sidebar"
          aria-pressed={sidebarOpen}
          title="Toggle sidebar (Ctrl+B)"
          onClick={onToggleSidebar}
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
