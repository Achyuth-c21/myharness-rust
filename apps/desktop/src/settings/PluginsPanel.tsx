/**
 * Plugins settings — Phase 5 (`docs/features/mcp-skills-tools.md` §7).
 *
 * Local install only. The marketplace, `.dxt`/`.mcpb` installer, and
 * signature verification were not captured; required-signature mode therefore
 * refuses every install rather than pretending a signature string is trusted.
 * Org bundles are read-only. A plugin contributes MCP servers and skills only
 * while enabled; server enablement takes effect on the next app start because
 * a session snapshot is never mutated underneath a turn.
 */
import { useCallback, useEffect, useState } from "react";
import {
  chat,
  type BridgeError,
  type PluginsView,
  type SignaturePolicy,
} from "../chat/client";

export function PluginsPanel() {
  const [view, setView] = useState<PluginsView | null>(null);
  const [error, setError] = useState<BridgeError | null>(null);
  const [path, setPath] = useState("");
  const [busy, setBusy] = useState<string | null>(null);

  const load = useCallback(async () => {
    const response = await chat({ type: "listPlugins" });
    if (response.type === "plugins") {
      setView(response.view);
      setError(null);
    } else if (response.type === "error") {
      setError(response.error);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const install = async () => {
    if (!path.trim()) return;
    setBusy("install");
    const response = await chat({ type: "installPlugin", path: path.trim() });
    if (response.type === "pluginInstalled") {
      setPath("");
      await load();
    } else if (response.type === "error") {
      setError(response.error);
    }
    setBusy(null);
  };

  const setPolicy = async (policy: SignaturePolicy) => {
    const response = await chat({ type: "setPluginSignaturePolicy", policy });
    if (response.type === "plugins") setView(response.view);
    else if (response.type === "error") setError(response.error);
  };

  if (error) {
    return (
      <div className="settings-note" role="alert">
        <span className="error-code">{error.code}</span> {error.message}
        <button type="button" className="text-btn" onClick={() => void load()}>
          Reload
        </button>
      </div>
    );
  }
  if (!view) return <div className="settings-note">Loading plugins…</div>;

  return (
    <div className="settings-panel">
      <p className="settings-note">
        This pane is rebuild-defined: the marketplace and desktop-extension installer were
        not captured. Install a local directory containing <code>plugin.json</code>.
        Plugin-contributed MCP servers join the normal policy/approval path on the next
        app start.
      </p>
      <div className="settings-inline">
        <input
          className="settings-search settings-path"
          placeholder="/absolute/path/to/plugin"
          aria-label="Plugin directory"
          value={path}
          onChange={(event) => setPath(event.target.value)}
        />
        <button
          type="button"
          className="text-btn"
          disabled={!path.trim() || busy === "install"}
          onClick={() => void install()}
        >
          Install
        </button>
      </div>
      <label className="settings-check">
        Signature policy{" "}
        <select
          value={view.signaturePolicy}
          onChange={(event) => void setPolicy(event.target.value as SignaturePolicy)}
        >
          <option value="optional">Optional (unsigned allowed, unverified)</option>
          <option value="required">Required (refuses all installs)</option>
        </select>
      </label>
      <div className="settings-list">
        {view.plugins.length === 0 ? (
          <div className="settings-note">No plugins installed.</div>
        ) : (
          view.plugins.map((plugin) => {
            const enabled = !view.disabled.includes(plugin.name);
            return (
              <div key={plugin.name} className="settings-row">
                <div>
                  <div className="settings-row-title">
                    {plugin.displayName}{" "}
                    <span className="settings-tag">{plugin.source}</span>
                    <span className="settings-tag">{plugin.signature}</span>
                    {plugin.readOnly ? <span className="settings-tag">read-only</span> : null}
                  </div>
                  <div className="settings-row-sub">
                    {plugin.description || "No description."}
                  </div>
                  <div className="settings-row-sub">
                    Servers: {plugin.servers.join(", ") || "none"} · Skills:{" "}
                    {plugin.skills.join(", ") || "none"}
                  </div>
                  {plugin.permissions.length > 0 ? (
                    <div className="settings-row-sub">
                      Permissions: {plugin.permissions.join(", ")}
                    </div>
                  ) : null}
                </div>
                <div className="settings-row-actions">
                  <label className="settings-check">
                    <input
                      type="checkbox"
                      checked={enabled}
                      disabled={busy === plugin.name}
                      onChange={async (event) => {
                        setBusy(plugin.name);
                        const response = await chat({
                          type: "setPluginEnabled",
                          name: plugin.name,
                          enabled: event.target.checked,
                        });
                        if (response.type === "plugins") setView(response.view);
                        else if (response.type === "error") setError(response.error);
                        setBusy(null);
                      }}
                    />
                    Enabled
                  </label>
                  <button
                    type="button"
                    className="text-btn"
                    disabled={plugin.readOnly || busy === plugin.name}
                    title={
                      plugin.readOnly
                        ? "An org bundle is mounted read-only"
                        : "Remove the installed plugin"
                    }
                    onClick={async () => {
                      setBusy(plugin.name);
                      const response = await chat({
                        type: "uninstallPlugin",
                        name: plugin.name,
                      });
                      if (response.type === "plugins") setView(response.view);
                      else if (response.type === "error") setError(response.error);
                      setBusy(null);
                    }}
                  >
                    Uninstall
                  </button>
                </div>
              </div>
            );
          })
        )}
      </div>
      {view.warnings.length > 0 ? (
        <details className="settings-warnings">
          <summary>{view.warnings.length} plugin warning(s)</summary>
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
