/**
 * Search palette — UI-INT-020..028 (docs/08 §2) and docs/screens/search-palette.md:
 * centered overlay ~726×470, strong shadow, scrim over the app while the sidebar
 * stays visible; 48px search row with a leading magnifier, placeholder
 * "Search chats and projects", and a clear button; result rows = surface icon +
 * title (+ right-side relative time when data exists). Selected row is a light-gray
 * rounded highlight across the row; "No results" keeps the query editable.
 *
 * Results here are shell route entries as a stand-in — the palette's real result set
 * is local session/project metadata (Phase 4). No "action rows" are claimed beyond
 * opening the matching surface (docs/screens/search-palette.md caution).
 */
import { useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { SCREENS } from "../routes";
import { useFocusTrap } from "./focusTrap";

interface Props {
  onClose: () => void;
}

function Magnifier() {
  return (
    <svg width="15" height="15" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <circle cx="7" cy="7" r="4.25" stroke="currentColor" strokeWidth="1.2" />
      <path d="m10.2 10.2 3 3" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
    </svg>
  );
}

function ClearGlyph() {
  return (
    <svg width="12" height="12" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path d="m4 4 8 8M12 4l-8 8" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" />
    </svg>
  );
}

function ListGlyph() {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path d="M2.5 4.5h11M2.5 8h11M2.5 11.5h7" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
    </svg>
  );
}

function CodeGlyph() {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path d="m5.5 4.5-3 3.5 3 3.5M10.5 4.5l3 3.5-3 3.5" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" />
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

function RowIcon({ surface }: { surface: string }) {
  if (surface === "code") return <CodeGlyph />;
  if (surface === "projects" || surface === "artifacts") return <FolderGlyph />;
  return <ListGlyph />;
}

export function SearchPalette({ onClose }: Props) {
  const navigate = useNavigate();
  const inputRef = useRef<HTMLInputElement>(null);
  const trapRef = useRef<HTMLDivElement>(null);
  const [query, setQuery] = useState("");
  const [index, setIndex] = useState(0);
  // UI-INT-026 + docs/06 §5: the palette traps focus and restores it to the trigger.
  useFocusTrap(trapRef, true);

  const queryTrimmed = query.trim();
  // UI-INT-021: empty query shows the recent set; trim the live-filter input.
  const results = SCREENS.filter((screen) =>
    queryTrimmed ? screen.label.toLowerCase().includes(queryTrimmed.toLowerCase()) : true,
  );

  const onKeyDown = (event: React.KeyboardEvent) => {
    // UI-INT-022: arrows move highlight and keep it visible; UI-INT-023: Enter opens.
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setIndex((i) => Math.min(i + 1, Math.max(results.length - 1, 0)));
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setIndex((i) => Math.max(i - 1, 0));
    } else if (event.key === "Enter") {
      event.preventDefault();
      const selected = results[index];
      if (selected) {
        navigate(selected.path);
        onClose();
      }
    } else if (event.key === "Escape") {
      event.preventDefault();
      // docs/08 §13: Escape closes the TOPMOST layer only — keep the event from
      // reaching underneath (e.g. the settings modal when both are open).
      event.stopPropagation();
      onClose();
    }
  };

  return (
    <div
      className="overlay-backdrop"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div className="palette" role="dialog" aria-label="Search" aria-modal="true" onKeyDown={onKeyDown} ref={trapRef}>
        <div className="palette-search-row">
          <span className="palette-glyph">
            <Magnifier />
          </span>
          <input
            ref={inputRef}
            className="palette-input"
            placeholder="Search chats and projects"
            value={query}
            aria-label="Search chats and projects"
            onChange={(event) => {
              setQuery(event.target.value);
              setIndex(0);
            }}
          />
          {/* Clear keeps the palette open (search-palette.md behavior table). */}
          {query ? (
            <button
              type="button"
              className="icon-btn icon-btn-sm"
              aria-label="Clear search"
              onClick={() => {
                setQuery("");
                setIndex(0);
                inputRef.current?.focus();
              }}
            >
              <ClearGlyph />
            </button>
          ) : null}
        </div>
        <div className="palette-results">
          {results.length === 0 ? (
            <div className="palette-empty" title={queryTrimmed}>
              No results
            </div>
          ) : (
            results.map((screen, i) => (
              <button
                key={screen.path}
                type="button"
                className={`palette-row${i === index ? " highlighted" : ""}`}
                onMouseEnter={() => setIndex(i)}
                onClick={() => {
                  navigate(screen.path);
                  onClose();
                }}
              >
                <span className="palette-glyph">
                  <RowIcon surface={screen.paletteSurface} />
                </span>
                <span className="palette-title">{screen.label}</span>
                {/* Right-side relative time renders from session metadata (Phase 4). */}
              </button>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
