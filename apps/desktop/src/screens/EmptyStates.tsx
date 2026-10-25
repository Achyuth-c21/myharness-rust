/**
 * Documented empty states — the only screens with captured empty copy/images
 * (docs/05 §4.4–4.7). Every string here is copied verbatim from the docs; nothing
 * is invented. Header action buttons are rendered per docs but stay disabled until
 * their flows arrive (docs/05 §4.4 "The header also has New project", etc.).
 */
function ClockGlyph() {
  return (
    <svg width="40" height="40" viewBox="0 0 40 40" fill="none" aria-hidden="true">
      <circle cx="20" cy="21.5" r="13" stroke="currentColor" strokeWidth="1.6" />
      <path d="M20 14.5v7l4.5 3" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
      <path d="M13 5.5h14" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
    </svg>
  );
}

function DocGlyph() {
  return (
    <svg width="40" height="40" viewBox="0 0 40 40" fill="none" aria-hidden="true">
      <path d="M11 6h13l6 6v22H11V6Z" stroke="currentColor" strokeWidth="1.6" strokeLinejoin="round" />
      <path d="M24 6v6h6M16 18h9M16 23h9M16 28h6" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
    </svg>
  );
}

function ChevronRight() {
  return (
    <svg width="12" height="12" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path d="m6 4 4 4-4 4" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

/**
 * Shared empty-state primitive (docs/07 §4 Empty: centered icon, short headline,
 * one sentence, primary action; no faux data).
 */
export function EmptyState({
  icon,
  headline,
  copy,
  action,
}: {
  icon: React.ReactNode;
  headline: string;
  copy: string;
  action: string;
}) {
  return (
    <div className="screen-empty">
      {icon}
      <h2>{headline}</h2>
      <p>{copy}</p>
      <button type="button" className="btn-secondary" disabled title="Arrives in a later phase (docs/13)">
        {action}
      </button>
    </div>
  );
}

/** Screen header — title + trailing action (Projects: docs/05 §4.4; Artifacts: §4.5). */
export function ScreenHead({ label, action }: { label: string; action?: string }) {
  return (
    <header className="screen-head">
      <h1>{label}</h1>
      {action ? (
        <button type="button" className="btn-secondary" disabled title="Creation flow arrives in a later phase (docs/13)">
          {action} <ChevronRight />
        </button>
      ) : null}
    </header>
  );
}

/** /projects — the real screen (Phase 6 slice 2) lives in ProjectsScreen.tsx. */

/** /cowork-artifact — docs/05 §4.5 empty state, exact copy (no sentence captured). */
export function ArtifactsScreen() {
  return (
    <>
      <ScreenHead label="Artifacts" action="New artifact" />
      <EmptyState
        icon={<DocGlyph />}
        headline="Create your first artifact"
        copy=""
        action="What needs my attention"
      />
    </>
  );
}

/** /scheduled-task — docs/05 §4.6: banner, keep-awake switch, empty copy. */
export function ScheduledScreen() {
  return (
    <>
      <ScreenHead label="Scheduled tasks" action="New Task" />
      <div className="info-banner" role="status">
        <span>
          Scheduled tasks run only while the computer is awake and online.{" "}
          {/* banner semantics per docs/05 §4.6; exact sentence copy pending locale catalog */}
        </span>
        <label className="switch">
          <input type="checkbox" disabled title="Keep awake control arrives with scheduling (docs/08 UI-INT-084)" />
          <span className="switch-track" aria-hidden="true" />
          <span className="switch-label">Keep awake</span>
        </label>
      </div>
      <EmptyState
        icon={<ClockGlyph />}
        headline="No scheduled tasks yet."
        copy=""
        action="New Task"
      />
    </>
  );
}

/** Generic fallthrough for routed surfaces without captured copy yet. */
