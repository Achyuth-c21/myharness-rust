/**
 * Settings → Privacy (`docs/09` §1.3, `docs/screens/settings.md` §Privacy;
 * `TEST-PRIV-001/002/003`, `REQ-PRIV-01/02/03`).
 *
 * The Privacy section is a disclosure page, not a telemetry dashboard. The
 * four disclosure statements are the captured copy (`docs/09` §1.3) — they
 * are the trust boundary and are not reworded. The rebuild's own telemetry
 * posture is stated honestly: no crash reporter, no analytics, and no
 * OpenTelemetry exporter ship in this build, so nothing leaves the device
 * except inference requests to the provider the profile configures.
 *
 * The diagnostic action follows the support-bundle states (`docs/features/
 * updates-telemetry-privacy.md` §7–§8): preview the enumerated contents,
 * require explicit consent, and state plainly that there is no upload in
 * this build — the bundle is written under the app's diagnostics directory
 * and stays on this device.
 */
import { useEffect, useState } from "react";
import { chat, type BridgeError } from "../chat/client";

function PrivacyPanel() {
  const [entries, setEntries] = useState<string[] | null>(null);
  const [consent, setConsent] = useState(false);
  const [built, setBuilt] = useState<string | null>(null);
  const [error, setError] = useState<BridgeError | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      const response = await chat({ type: "diagnosticsPreview" });
      if (cancelled) return;
      if (response.type === "diagnosticsPreview") {
        setEntries(response.entries);
      } else if (response.type === "error") {
        setError(response.error);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const build = async () => {
    if (busy || !consent) return;
    setBusy(true);
    const response = await chat({ type: "diagnosticsBuild", consent: true });
    setBusy(false);
    if (response.type === "diagnosticsBundle") {
      setBuilt(response.file);
      setError(null);
    } else if (response.type === "error") {
      setBuilt(null);
      setError(response.error);
    }
  };

  return (
    <div className="privacy-panel">
      <h3>What leaves this device</h3>
      <ul className="privacy-disclosure">
        <li>
          Inference requests go to the configured third-party provider, not
          Anthropic.
        </li>
        <li>
          Anthropic may still receive crash and performance reports,
          product-usage analytics without message content, update checks, and
          diagnostic bundles you explicitly build and upload.
        </li>
        <li>
          Connector favicons, artifact and MCP-App iframes, and the Linux
          spellcheck dictionary are nonessential services and may be blocked
          by policy.
        </li>
        <li>
          Provider billing and usage are handled by the configured provider,
          not Anthropic.
        </li>
      </ul>
      <p className="settings-note">
        This build ships no crash reporter, no analytics pipeline, and no
        OpenTelemetry exporter. Prompts and completions are never telemetry:
        the local ledger stores token counts only. The telemetry switches in
        the provider settings are recorded and displayed here; there is no
        emitter for them to gate yet.
      </p>

      <h3>Diagnostic report</h3>
      {error ? (
        <p className="settings-note" role="alert">
          {error.code === "BRIDGE_UNAVAILABLE"
            ? "Diagnostics are available in the desktop app. This browser preview has no local data to describe."
            : error.message}
        </p>
      ) : null}
      {error === null && entries ? (
        <>
          <p className="settings-note">A diagnostic bundle would include:</p>
          <ul className="privacy-contents" data-testid="diagnostics-contents">
            {entries.map((entry) => (
              <li key={entry}>{entry}</li>
            ))}
          </ul>
          <label className="privacy-consent">
            <input
              type="checkbox"
              checked={consent}
              onChange={(event) => {
                setConsent(event.target.checked);
                setBuilt(null);
              }}
            />
            <span>I understand what this bundle includes. Build it.</span>
          </label>
          <button
            type="button"
            className="btn-primary"
            disabled={!consent || busy}
            onClick={() => void build()}
          >
            Build diagnostic bundle
          </button>
          {built ? (
            <p className="settings-note" role="status">
              Written to the app&apos;s diagnostics directory as{" "}
              <code>{built}</code>. This build has no upload: the bundle stays
              on this device until you share it yourself.
            </p>
          ) : null}
        </>
      ) : null}
    </div>
  );
}

export { PrivacyPanel };
