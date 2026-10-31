/**
 * Error surfaces (UI-SCREEN-33; docs/screens/error-surfaces.md; FEAT-SHELL-13;
 * IMP-44; TEST-UX-016).
 *
 * The original shows two structured failure surfaces: a top-center banner for
 * a failed provider contact (title "Can't reach …", body, "Details" expander,
 * "Open Setup" + "Check again", close X) and bottom-right stacked toasts for
 * background failures with action buttons ("View logs" / "Open settings").
 * The rebuild had neither (docs/23 §3.8 item 34).
 *
 * Binding rules (spec §Binding rules for the rebuild):
 * - The renderer renders what the bridge reports — no invented
 *   provider/network logic, no local reachability probing. The only feed is
 *   `reportProviderContactFailure` / `reportBackgroundFailure`, called from
 *   real bridge-failure sites.
 * - `BRIDGE_UNAVAILABLE` is the browser preview's honest "available in the
 *   desktop app" state, NOT a failure — it never reaches these surfaces.
 * - "Check again" renders only when the reporting site supplies a real
 *   re-check; a button with no real target stays absent rather than faking.
 *   ("Open Setup" is real: the setup window exists.)
 * - Toast timing was never observed, so toasts stay until dismissed instead
 *   of inventing an auto-dismiss interval.
 * - The banner's title/body use the observed original copy for the same
 *   failure class (a provider contact that failed); the bridge's verbatim
 *   code + message stay one "Details" click away, so nothing is hidden.
 */
import { useEffect, useState } from "react";
import { openSetupWindow } from "../setup/client";
import type { BridgeError } from "../chat/client";

/** Failure classes the renderer itself can attest to: a contact was attempted. */
export const PROVIDER_CONTACT_CODES = new Set(["PROVIDER_ERROR", "BRIDGE_ERROR"]);

interface BackgroundNotice {
  id: number;
  code: string;
  message: string;
  action: { label: string; run: () => void } | null;
}

interface NoticeState {
  /** The newest provider-contact failure (the banner is singular). */
  banner: BridgeError | null;
  toasts: BackgroundNotice[];
}

let state: NoticeState = { banner: null, toasts: [] };
const listeners = new Set<() => void>();

function emit() {
  for (const listener of listeners) listener();
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

function useNotices(): NoticeState {
  const [, setTick] = useState(0);
  useEffect(() => subscribe(() => setTick((tick) => tick + 1)), []);
  return state;
}

let nextToastId = 1;

/**
 * Report a failed provider contact (banner). Only codes that mean the bridge
 * actually attempted and failed belong here; call sites pass the bridge's
 * error verbatim.
 */
export function reportProviderContactFailure(error: BridgeError): void {
  if (!PROVIDER_CONTACT_CODES.has(error.code)) return;
  state = { ...state, banner: error };
  emit();
}

/**
 * Report a background failure (toast) with an optional real action. The
 * message is shown verbatim; nothing is reworded into a fake success.
 * An identical visible toast (same code + message) is not duplicated — two
 * mounted list hooks must not stack the same failure twice.
 */
export function reportBackgroundFailure(input: {
  code: string;
  message: string;
  action?: { label: string; run: () => void };
}): void {
  if (
    state.toasts.some(
      (toast) =>
        toast.code === input.code &&
        toast.message === input.message &&
        toast.action?.label === input.action?.label,
    )
  ) {
    return;
  }
  state = {
    ...state,
    toasts: [
      ...state.toasts,
      { id: nextToastId++, code: input.code, message: input.message, action: input.action ?? null },
    ],
  };
  emit();
}

function dismissBanner() {
  state = { ...state, banner: null };
  emit();
}

function dismissToast(id: number) {
  state = { ...state, toasts: state.toasts.filter((toast) => toast.id !== id) };
  emit();
}

/**
 * The two surfaces, mounted once in the main shell. Renders nothing until a
 * real failure is reported; an honest empty render is the default.
 */
export function NoticeSurfaces() {
  const { banner, toasts } = useNotices();
  const [detailsOpen, setDetailsOpen] = useState(false);

  return (
    <>
      {banner ? (
        <div className="notice-banner" role="alert" aria-label="Provider error">
          <div className="notice-banner-head">
            <span className="notice-banner-title">Can’t reach the configured provider</span>
            <button
              type="button"
              className="notice-close"
              aria-label="Dismiss error"
              onClick={dismissBanner}
            >
              ✕
            </button>
          </div>
          <p className="notice-banner-body">
            The provider didn’t respond. Check your network or VPN, then try again.
          </p>
          <details
            className="notice-details"
            open={detailsOpen}
            onToggle={(event) => setDetailsOpen((event.target as HTMLDetailsElement).open)}
          >
            <summary>Details</summary>
            <pre className="notice-details-body">
              {banner.code}: {banner.message}
            </pre>
          </details>
          <div className="notice-actions">
            <button type="button" className="notice-btn" onClick={() => void openSetupWindow()}>
              Open Setup
            </button>
            {banner.retryable ? (
              <>
                {/* IMP-28: a tooltip on a disabled button never renders, so the
                    reason is a real described-by note. Re-checking the provider
                    needs a bridge command that does not exist yet; a fake
                    re-check would be invented behavior. */}
                <button
                  type="button"
                  className="notice-btn notice-btn-secondary"
                  disabled
                  aria-describedby="notice-checkagain-reason"
                >
                  Check again
                </button>
                <span id="notice-checkagain-reason" className="visually-hidden">
                  Provider re-checking is not available in this build yet.
                </span>
              </>
            ) : null}
          </div>
        </div>
      ) : null}
      {toasts.length > 0 ? (
        <div className="notice-toasts" role="status" aria-label="Notifications">
          {toasts.map((toast) => (
            <div key={toast.id} className="notice-toast">
              <p className="notice-toast-body">{toast.message}</p>
              <div className="notice-toast-row">
                {toast.action ? (
                  <button
                    type="button"
                    className="notice-btn"
                    onClick={() => {
                      // The action is a re-check: dismiss this toast first so
                      // a repeated failure re-reports instead of deduping
                      // against the stale one.
                      dismissToast(toast.id);
                      toast.action?.run();
                    }}
                  >
                    {toast.action.label}
                  </button>
                ) : null}
                <button
                  type="button"
                  className="notice-close"
                  aria-label="Dismiss notification"
                  onClick={() => dismissToast(toast.id)}
                >
                  ✕
                </button>
              </div>
            </div>
          ))}
        </div>
      ) : null}
    </>
  );
}
