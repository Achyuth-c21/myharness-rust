/**
 * Settings modal — docs/06 §1: NOT a separate route; lives on the current route as the
 * hash `#settings/<section>`. Sections follow the verified hash list. Layout follows
 * docs/07 §1.4: large modal, ~208px left nav, ~800px content column. Section contents
 * are specified in docs/09-settings-and-configuration.md and land phase by phase.
 */
import { useEffect } from "react";
import { SETTINGS_SECTIONS } from "../routes";

interface Props {
  section: string;
  onSection: (section: string) => void;
  onClose: () => void;
}

export function SettingsModal({ section, onSection, onClose }: Props) {
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onClose();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div
      className="overlay-backdrop"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div className="settings-modal" role="dialog" aria-modal="true" aria-label="Settings">
        <aside className="settings-nav" aria-label="Settings sections">
          {SETTINGS_SECTIONS.map((s) => (
            <button
              key={s}
              type="button"
              className={`settings-link${s === section ? " active" : ""}`}
              onClick={() => onSection(s)}
            >
              {s}
            </button>
          ))}
        </aside>
        <section className="settings-content">
          <h2>{section}</h2>
          <p>
            This section is specified in docs/09-settings-and-configuration.md and is wired in
            a later phase. Closing restores the underlying route without losing state
            (docs/06 §1).
          </p>
        </section>
      </div>
    </div>
  );
}

export function settingsSectionForSectionLabel(section: string): string {
  return section;
}
