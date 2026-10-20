/**
 * Cowork|Code segmented switch (UI-INT-004, docs/08 §1) — lives at the TOP OF THE
 * SIDEBAR, not the title bar: docs/screens/new-task.md layout shows it in the sidebar
 * column, and docs/06 §4: "The Cowork/Code switch is persistent in the sidebar."
 * Selected mode is visually raised with darkened text (UI-INT-004 verified states).
 */
import { useLocation, useNavigate } from "react-router-dom";

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
  );
}
