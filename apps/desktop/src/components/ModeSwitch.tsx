/**
 * Cowork|Code segmented switch (UI-INT-004, docs/08 §1) — lives at the TOP OF THE
 * SIDEBAR, not the title bar: docs/screens/new-task.md layout shows it in the sidebar
 * column, and docs/06 §4: "The Cowork/Code switch is persistent in the sidebar."
 * Selected mode is visually raised with darkened text (UI-INT-004 verified states).
 * Tab icons (shuffle for Cowork, `</>` for Code) match the reference capture.
 */
import { useLocation, useNavigate } from "react-router-dom";

function CoworkIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      {/* shuffle-arrows glyph */}
      <path
        d="M2 4.5h2.2c1 0 1.9.5 2.4 1.3L9 10a2.9 2.9 0 0 0 2.5 1.4H14M2 11.5h2.2c1 0 1.9-.5 2.4-1.3l.6-.9M9 5.9A2.9 2.9 0 0 1 11.5 4.5H14"
        stroke="currentColor"
        strokeWidth="1.2"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <path d="M12.2 2.8 14 4.5l-1.8 1.7M12.2 9.6l1.8 1.8-1.8 1.7" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

function CodeIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      {/* </> glyph */}
      <path d="m5 4.25-3 3.75 3 3.75M11 4.25l3 3.75-3 3.75" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

export function ModeSwitch() {
  const navigate = useNavigate();
  const { pathname } = useLocation();
  const mode = pathname.startsWith("/epitaxy") ? "code" : "cowork";

  return (
    <div className="segmented" role="tablist" aria-label="Mode">
      <button
        type="button"
        role="tab"
        aria-selected={mode === "cowork"}
        className={`seg${mode === "cowork" ? " seg-active" : ""}`}
        onClick={() => navigate("/new")}
      >
        <CoworkIcon />
        Cowork
      </button>
      <button
        type="button"
        role="tab"
        aria-selected={mode === "code"}
        className={`seg${mode === "code" ? " seg-active" : ""}`}
        onClick={() => navigate("/epitaxy")}
      >
        <CodeIcon />
        Code
      </button>
    </div>
  );
}
