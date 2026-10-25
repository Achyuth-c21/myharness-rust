/**
 * Settings modal — docs/06 §1 (hash `#settings/<section>` over the current route,
 * never a route) and docs/screens/settings.md geometry: ~1400×960 modal, light-gray
 * left nav ~208px with a ~175px search field at its top, groups (Settings / This
 * computer / Customize), close X at the top-right of the content (UI-INT-107),
 * content column ~800px. No footer Save; changes apply immediately.
 *
 * General content implements the documented Appearance → Theme segmented icon
 * control (System/Light/Dark per settings.md §General, semantics per docs/20 §4 and
 * UI-INT-105); remaining section bodies are specified in docs/09 and land later.
 */
import { useEffect, useMemo, useRef, useState } from "react";
import { SETTINGS_GROUPS } from "../routes";
import { applyMode, loadMode, type ThemeMode } from "../theme";
import { UsagePanel } from "../settings/UsagePanel";
import { PrivacyPanel } from "../settings/PrivacyPanel";
import { MemoryPanel } from "../settings/MemoryPanel";
import { DeveloperPanel } from "../settings/DeveloperPanel";
import { PluginsPanel } from "../settings/PluginsPanel";
import { SkillsPanel } from "../settings/SkillsPanel";
import { useFocusTrap } from "./focusTrap";

interface Props {
  section: string;
  onSection: (section: string) => void;
  onClose: () => void;
}

function MonitorGlyph() {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <rect x="2" y="3" width="12" height="8" rx="1.2" stroke="currentColor" strokeWidth="1.2" />
      <path d="M6 13.5h4" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
    </svg>
  );
}

function SunGlyph() {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <circle cx="8" cy="8" r="3" stroke="currentColor" strokeWidth="1.2" />
      <path
        d="M8 1.5v1.8M8 12.7v1.8M1.5 8h1.8M12.7 8h1.8M3.4 3.4l1.3 1.3M11.3 11.3l1.3 1.3M12.6 3.4l-1.3 1.3M4.7 11.3l-1.3 1.3"
        stroke="currentColor"
        strokeWidth="1.1"
        strokeLinecap="round"
      />
    </svg>
  );
}

function MoonGlyph() {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path d="M13 9.5A5.5 5.5 0 1 1 6.5 3a4.6 4.6 0 0 0 6.5 6.5Z" stroke="currentColor" strokeWidth="1.2" strokeLinejoin="round" />
    </svg>
  );
}

function CloseGlyph() {
  return (
    <svg width="13" height="13" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path d="m4 4 8 8M12 4l-8 8" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
    </svg>
  );
}

function ThemeControl() {
  const [mode, setMode] = useState<ThemeMode>(() => loadMode());
  const options: { key: ThemeMode; label: string; glyph: React.ReactNode }[] = [
    { key: "system", label: "System", glyph: <MonitorGlyph /> },
    { key: "light", label: "Light", glyph: <SunGlyph /> },
    { key: "dark", label: "Dark", glyph: <MoonGlyph /> },
  ];
  return (
    <div className="segmented theme-seg" role="radiogroup" aria-label="Theme">
      {options.map((opt) => (
        <button
          key={opt.key}
          type="button"
          role="radio"
          aria-checked={mode === opt.key}
          className={`seg theme-seg-option${mode === opt.key ? " seg-active" : ""}`}
          title={opt.label}
          onClick={() => {
            setMode(opt.key);
            applyMode(opt.key);
          }}
        >
          {opt.glyph}
          <span>{opt.label}</span>
        </button>
      ))}
    </div>
  );
}

export function SettingsModal({ section, onSection, onClose }: Props) {
  const searchRef = useRef<HTMLInputElement>(null);
  const trapRef = useRef<HTMLDivElement>(null);
  const [filter, setFilter] = useState("");
  useFocusTrap(trapRef, true);

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

  const groups = useMemo(
    () =>
      SETTINGS_GROUPS.map((group) => ({
        ...group,
        sections: group.sections.filter((s) =>
          filter ? s.label.toLowerCase().includes(filter.trim().toLowerCase()) : true,
        ),
      })).filter((group) => group.sections.length > 0),
    [filter],
  );

  return (
    <div
      className="overlay-backdrop"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div className="settings-modal" role="dialog" aria-modal="true" aria-label="Settings" ref={trapRef}>
        <aside className="settings-nav" aria-label="Settings sections">
          <input
            ref={searchRef}
            className="settings-search"
            placeholder="Search…"
            aria-label="Search settings"
            value={filter}
            onChange={(event) => setFilter(event.target.value)}
            onKeyDown={(event) => {
              // settings.md §Search: Escape closes Settings only when search isn't
              // consuming it — a non-empty query is cleared first (topmost-layer rule,
              // docs/08 §13), and only a second Escape reaches the modal close.
              if (event.key === "Escape" && filter) {
                event.preventDefault();
                event.stopPropagation();
                setFilter("");
              }
            }}
          />
          {filter && groups.length === 0 ? <div className="settings-no-match">No results</div> : null}
          {groups.map((group) => (
            <div key={group.group} className="settings-group">
              <div className="settings-group-label">{group.group}</div>
              {group.sections.map((s) => (
                <button
                  key={s.key}
                  type="button"
                  className={`settings-link${s.key === section ? " active" : ""}`}
                  disabled={s.disabled}
                  title={s.disabled ? "Route not captured in the evidence — docs/06 §1" : undefined}
                  onClick={() => onSection(s.key)}
                >
                  {s.label}
                </button>
              ))}
            </div>
          ))}
        </aside>
        <section className="settings-content-wrap">
          <button type="button" className="icon-btn settings-x" aria-label="Close settings" onClick={onClose}>
            <CloseGlyph />
          </button>
          <div className="settings-content">
            <h2>{SETTINGS_GROUPS.flatMap((g) => g.sections).find((s) => s.key === section)?.label ?? section}</h2>
            {section === "general" ? (
              <>
                <h3>Appearance</h3>
                <ThemeControl />
                <p className="settings-note">
                  Remaining General fields (profile, instructions, chat font, motion, notifications) and the
                  other sections are specified in docs/09-settings-and-configuration.md and land later.
                </p>
              </>
            ) : section === "usage" ? (
              <UsagePanel />
            ) : section === "data-privacy-controls" ? (
              <PrivacyPanel />
            ) : section === "cowork" ? (
              <MemoryPanel />
            ) : section === "developer" ? (
              <DeveloperPanel />
            ) : section === "skills" ? (
              <SkillsPanel />
            ) : section === "plugins" ? (
              <PluginsPanel />
            ) : (
              <p className="settings-note">
                This section is specified in docs/09-settings-and-configuration.md and is wired in a later
                phase. Closing restores the underlying route without losing state (docs/06 §1).
              </p>
            )}
          </div>
        </section>
      </div>
    </div>
  );
}
