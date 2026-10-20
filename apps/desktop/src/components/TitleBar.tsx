/**
 * Title bar — 36px high (docs/07 §1.4).
 * - sidebar toggle + Ctrl+B (UI-INT-001);
 * - back/forward arrows greyed at history ends (UI-INT-003);
 * - segmented Cowork|Code switch changing mode AND route (UI-INT-004);
 * - window controls are present but inert until the bridge wires desktop/window APIs.
 */
import { useEffect, useState } from "react";
import { useLocation, useNavigate } from "react-router-dom";

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

export function TitleBar({ sidebarOpen, onToggleSidebar }: Props) {
  const navigate = useNavigate();
  const location = useLocation();
  const { canBack, canForward } = useHistoryDepth();
  const mode = location.pathname.startsWith("/epitaxy") ? "code" : "cowork";

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
          ☰
        </button>
        <button
          type="button"
          className="icon-btn"
          aria-label="Back"
          title="Back"
          disabled={!canBack}
          onClick={() => window.history.back()}
        >
          ←
        </button>
        <button
          type="button"
          className="icon-btn"
          aria-label="Forward"
          title="Forward"
          disabled={!canForward}
          onClick={() => window.history.forward()}
        >
          →
        </button>
      </div>

      <div className="titlebar-center" data-tauri-drag-region>
        {/* UI-INT-004: segmented control; selected mode is raised, text darkened. */}
        <div className="segmented" role="tablist" aria-label="Mode">
          <button
            type="button"
            role="tab"
            aria-selected={mode === "cowork"}
            className={`seg${mode === "cowork" ? " seg-active" : ""}`}
            onClick={() => navigate("/new")}
          >
            Cowork
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={mode === "code"}
            className={`seg${mode === "code" ? " seg-active" : ""}`}
            onClick={() => navigate("/epitaxy")}
          >
            Code
          </button>
        </div>
      </div>

      <div className="titlebar-right">
        {/* Wired via crates/bridge (core invoke) in a later phase; inert for now. */}
        <button type="button" className="icon-btn" disabled title="Minimize — via bridge later">
          –
        </button>
        <button type="button" className="icon-btn" disabled title="Maximize — via bridge later">
          ▢
        </button>
        <button type="button" className="icon-btn" disabled title="Close — via bridge later">
          ✕
        </button>
      </div>
    </header>
  );
}
