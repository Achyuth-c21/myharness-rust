/**
 * "Trust this workspace?" — the gate before a folder-executing Code session
 * (UI-SCREEN-32; docs/screens/trust-workspace.md; FEAT-CODE-13; REQ-CODE-09;
 * IMP-41; TEST-CODE-009).
 *
 * Copy is verbatim from the live audit (docs/23 §3.6 item 32, capture x26).
 * The semantics are binding, not decorative: Cancel aborts — no session is
 * created and nothing touches the folder; Trust records a stored per-folder
 * decision in the bridge and proceeds. Trust is stored, never inferred
 * (docs/10 §8): the bridge refuses an untrusted folder at create and send,
 * so this modal is the path, not the gate itself.
 */
import { useEffect, useRef } from "react";
import { useFocusTrap } from "./focusTrap";

interface Props {
  /** The canonical folder spelling the bridge verified. */
  folder: string;
  onCancel: () => void;
  onTrust: () => void;
  busy?: boolean;
  error?: string | null;
}

export function TrustWorkspaceModal({ folder, onCancel, onTrust, busy, error }: Props) {
  const dialogRef = useRef<HTMLDivElement>(null);
  useFocusTrap(dialogRef, true);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onCancel();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onCancel]);

  return (
    <div className="trust-scrim">
      <div
        className="trust-dialog"
        role="dialog"
        aria-modal="true"
        aria-label="Trust this workspace?"
        ref={dialogRef}
      >
        <h2 className="trust-title">Trust this workspace?</h2>
        <p className="trust-body">
          Claude Code may read, write, or execute files in this folder. Only proceed if you trust
          this workspace.
        </p>
        <p className="trust-folder">{folder}</p>
        <p className="trust-note">Read our security guide for more information.</p>
        <p className="trust-source">
          Execution allowed by: <code>.claude/settings.local.json</code>
        </p>
        {error ? (
          <p className="trust-error" role="alert">
            {error}
          </p>
        ) : null}
        <div className="trust-actions">
          <button type="button" className="trust-cancel" onClick={onCancel} disabled={busy}>
            Cancel
          </button>
          <button type="button" className="trust-confirm" onClick={onTrust} disabled={busy}>
            {busy ? "Recording…" : "Trust workspace"}
          </button>
        </div>
      </div>
    </div>
  );
}
