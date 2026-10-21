/**
 * Loading/empty/error shells — Phase 1 build list item (docs/13 §4 Phase 1).
 * Placeholder copy is English strings for the scaffold; the shipped product must draw
 * from the locale catalog instead (docs/05/07 note; React Intl). Swapped per feature.
 */
import React from "react";

export function LoadingShell({ label = "Loading…" }: { label?: string }) {
  return (
    <div className="shell loading" role="status" aria-live="polite">
      <span className="spinner" aria-hidden="true" />
      <span>{label}</span>
    </div>
  );
}

export function EmptyShell({ title, hint }: { title: string; hint?: string }) {
  return (
    <div className="shell empty">
      <h2>{title}</h2>
      {hint ? <p>{hint}</p> : null}
    </div>
  );
}

/** Error shell. Internal error text is NOT echoed into the UI
 *  (report-bug-hunt C6: information exposure) — the copy is generic; details
 *  stay in the console for diagnostics, with redacted telemetry in Phase 7
 *  (docs/10 §8). */
export function ErrorShell() {
  return (
    <div className="shell error" role="alert">
      <h2>Something went wrong</h2>
      <p>The screen hit an unexpected error. Details are logged for diagnostics.</p>
    </div>
  );
}

type BoundaryProps = { children: React.ReactNode };
type BoundaryState = { error: Error | null };

/** Crash overlay shell (docs/02 §3.1: crash/error overlay behavior). */
export class ErrorBoundary extends React.Component<BoundaryProps, BoundaryState> {
  state: BoundaryState = { error: null };

  static getDerivedStateFromError(error: Error): BoundaryState {
    return { error };
  }

  componentDidCatch(error: Error, info: unknown) {
    // Phase 7 wires telemetry here (redaction-first, docs/10 §8).
    console.error("Renderer crash:", error, info);
  }

  render() {
    if (this.state.error) return <ErrorShell />;
    return this.props.children;
  }
}
