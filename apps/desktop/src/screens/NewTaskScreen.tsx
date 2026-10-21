/**
 * New task / Cowork home (UI-SCREEN-02) — layout per docs/screens/new-task.md:
 * greeting ("You’re here!" serif ~36px with the orange starburst mark to its left),
 * 720px composer card (placeholder "How can I help you today?", toolbar with +  /
 * model label centered / send arrow, second row "Project or folder" chip + info
 * icon), and the Active header with "Clear active" (empty list keeps the header,
 * no rows — docs/screens/new-task.md §Active list).
 *
 * Phase 1 boundary: session creation (Enter send → /cowork/<local_uuid>) belongs to
 * the sessions module (docs/13 §Phase 4). Send is visually per spec (UI-INT-042) but
 * stays disabled with a runtime reason; no faux navigation.
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
    return <line key={i} x1={x1} y1={y1} x2={x2} y2={y2} stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" />;
  });
  return (
    <svg width="28" height="28" viewBox="0 0 24 24" fill="none" className="starburst" aria-hidden="true">
      {rays}
    </svg>
  );
}

function FolderGlyph() {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
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

function SendGlyph() {
  return (
    <svg width="15" height="15" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path d="M8 13V3.5M8 3.5 4 7.4M8 3.5 12 7.4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

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

  return (
    <div className="new-task">
      <h1 className="greeting">
        <Starburst />
        You’re here!
      </h1>

      <div className="composer">
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
            className="icon-btn"
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
            title="Model picker arrives with provider adapters (docs/13 §Phase 3)"
          >
            Model <ChevronDown />
          </button>
          <span className="composer-spacer" />
          <button
            type="button"
            className={`send-btn${hasText ? " primary" : ""}`}
            aria-label="Send"
            disabled
            title="Sending arrives with the sessions module (docs/13 §Phase 4)"
          >
            <SendGlyph />
          </button>
        </div>
        <div className="composer-row2">
          <button
            type="button"
            className="chip"
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
            i
          </button>
        </div>
      </div>

      <div className="active-list">
        <div className="active-head">
          <span>Active</span>
          <button type="button" className="link-btn" disabled title="No active tasks">
            Clear active
          </button>
        </div>
        {/* Active rows (surface icon, title, relative time, chevron per
            docs/screens/new-task.md §Active list) render from session metadata
            — Phase 4. Empty list keeps the header only; no faux data. */}
      </div>
    </div>
  );
}
