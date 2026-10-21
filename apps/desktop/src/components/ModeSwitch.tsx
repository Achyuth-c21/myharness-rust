/**
 * Cowork|Code segmented switch (UI-INT-004, docs/08 §1) — lives at the TOP OF THE
 * SIDEBAR, not the title bar: docs/screens/new-task.md layout shows it in the sidebar
 * column, and docs/06 §4: "The Cowork/Code switch is persistent in the sidebar."
 * Selected mode is visually raised with darkened text (UI-INT-004 verified states).
 * Tab icons: task-list (checklist) for Cowork, `</>` for Code — per the reference
 * capture and the palette surface glyphs (docs/screens/search-palette.md).
 */
import { useLocation, useNavigate } from "react-router-dom";

function CoworkIcon() {
  // Task-list (checklist) glyph — matches the user-provided reference tab icon;
  // docs/screens/search-palette.md also documents the Cowork surface icon as a
  // task-list glyph. 16px, 1.2 stroke to match the nav icon family.
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path
        d="m2.5 4.4 1.3 1.3 2-2.5M8.4 4.7h5M2.5 10.9l1.3 1.3 2-2.5M8.4 11.2h5"
        stroke="currentColor"
        strokeWidth="1.2"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
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
