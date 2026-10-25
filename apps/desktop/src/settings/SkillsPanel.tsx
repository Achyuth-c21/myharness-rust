/**
 * Skills settings — Phase 5 (`docs/features/mcp-skills-tools.md` §6).
 *
 * The shipped Customize → Skills surface was not captured (`docs/06` §1 lists
 * it only as a shortcut), so this pane is rebuild-defined. It lists bundled
 * placeholders honestly (`runnable: false` when the body was not captured),
 * user/org/plugin skills, and persists enablement. Selection happens in the
 * composer; this pane never executes a skill.
 */
import { useCallback, useEffect, useState } from "react";
import { chat, type BridgeError, type SkillsView } from "../chat/client";

export function SkillsPanel() {
  const [view, setView] = useState<SkillsView | null>(null);
  const [error, setError] = useState<BridgeError | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  const load = useCallback(async () => {
    const response = await chat({ type: "listSkills" });
    if (response.type === "skills") {
      setView(response.view);
      setError(null);
    } else if (response.type === "error") {
      setError(response.error);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const toggle = async (name: string, enabled: boolean) => {
    setBusy(name);
    const response = await chat({ type: "setSkillEnabled", name, enabled });
    if (response.type === "skills") {
      setView(response.view);
      setError(null);
    } else if (response.type === "error") {
      setError(response.error);
    }
    setBusy(null);
  };

  if (error) {
    return (
      <div className="settings-note" role="alert">
        <span className="error-code">{error.code}</span> {error.message}
      </div>
    );
  }
  if (!view) return <div className="settings-note">Loading skills…</div>;

  return (
    <div className="settings-panel">
      <p className="settings-note">
        This pane is rebuild-defined: the shipped Customize → Skills surface was not
        captured. Bundled entries whose bodies were not captured are listed but cannot be
        selected.
      </p>
      <label className="settings-check">
        <input
          type="checkbox"
          checked={view.creationEnabled}
          onChange={async (event) => {
            const response = await chat({
              type: "setSkillCreationEnabled",
              enabled: event.target.checked,
            });
            if (response.type === "skills") setView(response.view);
            else if (response.type === "error") setError(response.error);
          }}
        />
        Allow skill creation and upload
      </label>
      <p className="settings-note">
        The skill editor was not exercised; this flag is persisted only and does not add
        an editor.
      </p>
      <div className="settings-list">
        {view.skills.map((skill) => {
          const enabled = !view.disabled.includes(skill.name);
          return (
            <div key={skill.name} className="settings-row">
              <div>
                <div className="settings-row-title">
                  {skill.displayName}{" "}
                  <span className="settings-tag">{skill.source}</span>
                  {!skill.runnable ? (
                    <span className="settings-tag">not runnable</span>
                  ) : null}
                </div>
                <div className="settings-row-sub">{skill.description}</div>
              </div>
              <label className="settings-check">
                <input
                  type="checkbox"
                  checked={enabled}
                  disabled={busy === skill.name}
                  onChange={(event) => void toggle(skill.name, event.target.checked)}
                />
                Enabled
              </label>
            </div>
          );
        })}
      </div>
      {view.warnings.length > 0 ? (
        <details className="settings-warnings">
          <summary>{view.warnings.length} skill warning(s)</summary>
          <ul>
            {view.warnings.map((warning, index) => (
              <li key={index}>{warning}</li>
            ))}
          </ul>
        </details>
      ) : null}
    </div>
  );
}
