/**
 * Search palette — UI-INT-020..028 (docs/08 §2), minimal Phase 1 subset:
 * open on Ctrl+K (App), live filter, arrow navigation, Enter opens, Escape/outside
 * closes with focus returning to the trigger, "No results" empty text, ellipsized rows.
 * Results include actions (route entries); sessions/tasks data arrives with the
 * sessions module (Phase 4) — scoped to local data per docs/08 §2 footnote.
 */
import { useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { SCREENS } from "../routes";

interface Props {
  onClose: () => void;
}

export function SearchPalette({ onClose }: Props) {
  const navigate = useNavigate();
  const inputRef = useRef<HTMLInputElement>(null);
  const triggerRef = useRef<Element | null>(null);
  const [query, setQuery] = useState("");
  const [index, setIndex] = useState(0);

  // UI-INT-026: Escape or outside click closes; focus returns to the trigger.
  useEffect(() => {
    triggerRef.current = document.activeElement;
    inputRef.current?.focus();
    return () => {
      if (triggerRef.current instanceof HTMLElement) triggerRef.current.focus();
    };
  }, []);

  const queryTrimmed = query.trim();
  // UI-INT-021: "empty query shows recent/actions" — route actions act as the recent set.
  const results = SCREENS.filter((screen) =>
    queryTrimmed ? screen.label.toLowerCase().includes(queryTrimmed.toLowerCase()) : true,
  );

  const onKeyDown = (event: React.KeyboardEvent) => {
    // UI-INT-022: arrows move highlight; UI-INT-023: Enter opens.
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
      onClose();
    } else if (event.key === "Tab") {
      // UI-INT-024: Tab moves focus (default behavior); nothing special.
    }
  };

  return (
    <div
      className="overlay-backdrop"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div className="palette" onKeyDown={onKeyDown}>
        <input
          ref={inputRef}
          className="palette-input"
          placeholder="Search locally…"
          value={query}
          onChange={(event) => {
            setQuery(event.target.value);
            setIndex(0);
          }}
        />
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
                <span className="palette-title">{screen.label}</span>
              </button>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
