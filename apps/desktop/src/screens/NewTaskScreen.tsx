/**
 * New task / Cowork home (UI-SCREEN-02) — matched to the user-provided reference
 * capture of the shipped surface (2026-10-21, 1920×1080):
 * - greeting "You're here!" with the orange starburst directly beside it
 *   (serif display type; the section sits lower in the viewport than earlier
 *   builds, per the reference);
 * - composer: a rounded input card (transparent textarea; plain "+" lower-left;
 *   plain "claude" model label and a rounded-square outlined send with an
 *   upward arrow at the right), with a DISTINCT section strip directly beneath
 *   it holding the "Project or folder" selector and the info icon;
 * - "Ideas for you" suggestion rows under the composer (the shipped empty-home
 *   state; the Active list appears with session data — Phase 4).
 *
 * Phase 1 boundary: session creation (Enter send → /cowork/<local_uuid>) belongs to
 * the sessions module (docs/13 §Phase 4). Send is visually per reference but stays
 * disabled with a runtime reason; suggestions only pre-fill the existing draft.
 */
import { useEffect, useRef, useState } from "react";

interface Props {
  /** UI-INT-005: the composer focuses on the explicit New invocation (the Ctrl+N
   *  remount driven by newKey in App). Boot/plain navigation do not steal focus —
   *  boot focus visuals are unobserved (docs/14 §9 decision note, report Run 4). */
  focusOnMount?: boolean;
}

function Starburst() {
  // Orange starburst mark (docs/07 §1.5: starburst uses the orange accent).
  const rays = Array.from({ length: 8 }, (_, i) => {
    const a = (i * Math.PI * 2) / 8;
    const x1 = 12 + Math.cos(a) * 3.4;
    const y1 = 12 + Math.sin(a) * 3.4;
    const x2 = 12 + Math.cos(a) * 10.6;
    const y2 = 12 + Math.sin(a) * 10.6;
    return <line key={i} x1={x1} y1={y1} x2={x2} y2={y2} stroke="currentColor" strokeWidth="2.6" strokeLinecap="round" />;
  });
  return (
    <svg width="40" height="40" viewBox="0 0 24 24" fill="none" className="starburst" aria-hidden="true">
      {rays}
    </svg>
  );
}

function FolderGlyph() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path
        d="M1.75 4.5A1.5 1.5 0 0 1 3.25 3h3l1.5 1.75h4.5a1.5 1.5 0 0 1 1.5 1.5v5.25a1.5 1.5 0 0 1-1.5 1.5H3.25a1.5 1.5 0 0 1-1.5-1.5V4.5Z"
        stroke="currentColor"
        strokeWidth="1.2"
      />
    </svg>
  );
}

function ChevronDown() {
  return (
    <svg width="12" height="12" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path d="m4 6.5 4 4 4-4" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

function ArrowUpIcon() {
  // Rounded-square send: upward arrow (reference capture).
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path d="M8 12.5V4M8 4 4.6 7.4M8 4l3.4 3.4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

function InfoIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <circle cx="8" cy="8" r="5.9" stroke="currentColor" strokeWidth="1.2" />
      <path d="M8 7.2v3.6" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" />
      <circle cx="8" cy="5" r="0.8" fill="currentColor" stroke="none" />
    </svg>
  );
}

function BriefingIcon() {
  return (
    <svg width="26" height="26" viewBox="0 0 26 26" fill="none" aria-hidden="true">
      <rect x="3.5" y="3.5" width="19" height="19" rx="4" stroke="currentColor" strokeWidth="1.2" />
      <path d="M8.5 10.5h9M8.5 15.5h5.5" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" />
    </svg>
  );
}

function InboxIcon() {
  return (
    <svg width="26" height="26" viewBox="0 0 26 26" fill="none" aria-hidden="true">
      {/* two overlapping message glyphs */}
      <path d="M4 7h11a2 2 0 0 1 2 2v4.5a2 2 0 0 1-2 2H8.5L5 18v-2.5H4a2 2 0 0 1-2-2V9a2 2 0 0 1 2-2Z" stroke="currentColor" strokeWidth="1.2" strokeLinejoin="round" />
      <path d="M18.5 10.5H21a2 2 0 0 1 2 2v3.5a2 2 0 0 1-2 2h-.5v2.5l-3.5-2.5h-2.2" stroke="currentColor" strokeWidth="1.2" strokeLinejoin="round" />
    </svg>
  );
}

function CustomizeSuggestionIcon() {
  return (
    <svg width="26" height="26" viewBox="0 0 26 26" fill="none" aria-hidden="true">
      {/* document + slider glyph */}
      <path d="M7 2.5h8l4 4V19a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V4.5a2 2 0 0 1 2-2Z" stroke="currentColor" strokeWidth="1.2" strokeLinejoin="round" />
      <path d="M4.5 21.5c2 .5 3.5-.5 4-2l4-7.5 3 1.8-4.2 7.4c-.7 1.2-1.8 1.6-3.2 1.4" stroke="currentColor" strokeWidth="1.2" strokeLinejoin="round" />
    </svg>
  );
}

/** Suggestion rows of the shipped empty home ("Ideas for you", reference capture). */
const IDEAS = [
  { label: "Send me a daily briefing", Icon: BriefingIcon },
  { label: "Organize my inbox", Icon: InboxIcon },
  { label: "Customize Cowork for me", Icon: CustomizeSuggestionIcon },
];

export function NewTaskScreen({ focusOnMount = false }: Props) {
  const [draft, setDraft] = useState("");
  const taRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    if (focusOnMount) taRef.current?.focus(); // UI-INT-005 (Ctrl+N remount only)
  }, [focusOnMount]);

  const hasText = draft.trim().length > 0;

  const onKeyDown = (event: React.KeyboardEvent) => {
    // UI-INT-041: Enter sends, Shift+Enter newlines. Send itself arrives with the
    // sessions module (docs/13 §Phase 4) — Enter stays inert until then.
    if (event.key === "Enter" && !event.shiftKey) event.preventDefault();
  };

  const autoGrow = (el: HTMLTextAreaElement) => {
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, 200)}px`; // grows to a max, then scrolls (UI-INT-040)
  };

  const seedDraft = (label: string) => {
    // Reuse the existing composer draft: a suggestion prefills the input. No
    // session starts until the send flow exists (docs/13 §4). C7
    // (report-bug-hunt): the programmatic prefill bypasses onChange, so the
    // textarea must re-measure after the value lands or the card keeps its
    // old height.
    setDraft(label);
    requestAnimationFrame(() => {
      const el = taRef.current;
      if (el) autoGrow(el);
      el?.focus();
    });
  };

  return (
    <div className="new-task">
      <h1 className="greeting">
        <Starburst />
        You’re here!
      </h1>

      <div className="composer-stack">
        <div className="composer-card">
          <textarea
            ref={taRef}
            id="composer"
            className="composer-input"
            placeholder="How can I help you today?"
            value={draft}
            rows={1}
            aria-label="Message"
            onChange={(event) => {
              setDraft(event.target.value);
              autoGrow(event.target);
            }}
            onKeyDown={onKeyDown}
          />
          <div className="composer-toolbar">
            <button
              type="button"
              className="plus-btn"
              aria-label="Add"
              disabled
              title="Add menu arrives with attachments (docs/13 §later phases)"
            >
              +
            </button>
            <span className="composer-spacer" />
            <button
              type="button"
              className="composer-model"
              disabled
              title="Model wiring arrives with provider adapters (docs/13 §Phase 3); the shipped home shows this label"
            >
              claude
            </button>
            <button
              type="button"
              className={`send-btn${hasText ? " primary" : ""}`}
              aria-label="Send"
              disabled
              title="Sending arrives with the sessions module (docs/13 §Phase 4)"
            >
              <ArrowUpIcon />
            </button>
          </div>
        </div>

        {/* Distinct selector section directly beneath the input (reference:
            same width, its own rounded strip). Selection arrives later
            (docs/08 §UI-INT-045); the info affordance is an inert pointer. */}
        <div className="composer-strip">
          <button
            type="button"
            className="project-chip"
            disabled
            title="Project/folder selection arrives in a later phase (docs/08 §UI-INT-045)"
          >
            <FolderGlyph />
            Project or folder
            <ChevronDown />
          </button>
          <span className="composer-spacer" />
          <button
            type="button"
            className="icon-btn icon-btn-sm"
            aria-label="Info"
            disabled
            title="Info affordance arrives in a later phase"
          >
            <InfoIcon />
          </button>
        </div>
      </div>

      <section className="ideas" aria-label="Ideas for you">
        <h2 className="ideas-title">Ideas for you</h2>
        {IDEAS.map(({ label, Icon }) => (
          <button key={label} type="button" className="idea-row" onClick={() => seedDraft(label)}>
            <Icon />
            <span>{label}</span>
          </button>
        ))}
        {/* The Active list (surface icon, title, relative time, chevron —
            docs/screens/new-task.md §Active list) replaces this block when
            session metadata exists — Phase 4; the empty home shows Ideas. */}
      </section>
    </div>
  );
}
