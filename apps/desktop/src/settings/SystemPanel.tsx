/**
 * Settings → System (`docs/screens/settings.md` §System: "Version/update
 * state"; `REQ-UPDATE-01/02`).
 *
 * The shipped section shows the running version and the update state. This
 * build's update posture is stated honestly (`docs/features/updates-
 * telemetry-privacy.md` §4, `docs/14` §9): the shipped feed's wire format
 * and signing key were never captured, so the lifecycle machinery exists
 * (`crates/updates`) but no bundle can be offered, verified, or installed —
 * and the running version is never placed at risk by an unverified bundle.
 * The auto-update switches in the provider settings are recorded policy,
 * not dead controls: the enforcement/relaunch windows and the session guard
 * they configure are implemented and tested at the state-machine level.
 */
import { useEffect, useState } from "react";

function SystemPanel() {
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const { getVersion } = await import("@tauri-apps/api/app");
        const reported = await getVersion();
        if (!cancelled) setVersion(reported);
      } catch {
        // No Tauri host (browser preview): the boundary note below stands.
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <div className="system-panel">
      <div className="usage-total" data-testid="system-version">
        <span className="usage-label">Version</span>
        <span className="usage-value">{version ?? "—"}</span>
      </div>
      {version === null ? (
        <p className="settings-note" role="status">
          The running version is reported by the desktop app. This browser
          preview has no shell to ask.
        </p>
      ) : null}
      {/* The observed System rows (docs/23 §3.4 item 24, capture x16). Every
          control here needs a shell capability the Tauri side does not expose
          yet, so each renders disabled with its reason (docs/14 §9) — the
          observed anatomy with honest states, per IMP-43. */}
      <div className="settings-pref-row">
        <span className="settings-pref-label">
          Run on startup
          <span className="settings-note">
            Launch the app when you log in.
          </span>
        </span>
        <label className="switch">
          <input type="checkbox" disabled title="Startup registration is a shell capability that is not exposed yet (docs/14 §9)" />
          <span className="switch-track" />
        </label>
      </div>
      <div className="settings-pref-row">
        <span className="settings-pref-label">
          Quick Entry keyboard shortcut
          <span className="settings-note">
            Quick Entry is unavailable in this build (FEAT-SHELL-08) — the
            observed default binding is shown, not active.
          </span>
        </span>
        <span className="settings-value">
          <kbd className="settings-kbd">Ctrl+Alt+Space</kbd>
          <button
            type="button"
            className="text-btn"
            disabled
            title="The binding is not active in this build; there is nothing to clear yet"
          >
            Clear
          </button>
        </span>
      </div>
      <div className="settings-pref-row">
        <span className="settings-pref-label">
          System tray
          <span className="settings-note">
            Tray support is a shell capability that is not exposed yet; until
            it is, closing the window closes the app.
          </span>
        </span>
        <label className="switch">
          <input type="checkbox" disabled title="The tray is not exposed by this build's shell yet (docs/14 §9)" />
          <span className="switch-track" />
        </label>
      </div>
      <div className="settings-pref-row">
        <span className="settings-pref-label">
          Keep computer awake
          <span className="settings-note">
            The scheduler&apos;s keep-awake ships disabled with the recorded
            reason (docs/14 §9 item 64); the toggle cannot assert a capability
            the shell does not expose.
          </span>
        </span>
        <label className="switch">
          <input type="checkbox" disabled title="Keep-awake is not exposed by this build's shell yet (docs/14 §9)" />
          <span className="switch-track" />
        </label>
      </div>
      <div className="settings-pref-row">
        <span className="settings-pref-label">Storage</span>
        <button
          type="button"
          className="text-btn"
          disabled
          title="A storage manager needs store-usage reporting that is not built yet"
        >
          Manage
        </button>
      </div>
      <h3>Updates</h3>
      <p className="settings-note">
        This build has no update feed: the shipped feed&apos;s wire format and
        signing key were never captured, so no bundle can be offered or
        verified, and none is installed. Your running version is never
        replaced by an unverified bundle. Auto-update switches in the provider
        settings are recorded policy — the enforcement and relaunch windows
        and the streaming-task guard they configure are implemented, and a
        captured feed will move this statement aside rather than weaken the
        verification gate.
      </p>
    </div>
  );
}

export { SystemPanel };
