/**
 * Routines — the Code-mode counterpart of Scheduled (docs/23 §7 addendum
 * item 6; IMP-54): serif title, black "New routine" button, the stopwatch
 * illustration, and the "No routines yet." empty state.
 *
 * The round-2 pass observed this screen's surface but not its editor or run
 * flow, and the rebuild has no routines store — the button is inert with a
 * reason and the screen carries the honest bridge states. The stopwatch art
 * is shared with the Scheduled screen (same observed illustration).
 */
import { StopwatchArt } from "./ScheduledScreen";


function NewRoutineGlyph() {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path d="M8 3.5v9M3.5 8h9" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
    </svg>
  );
}

export function RoutinesScreen() {
  return (
    <div className="routines">
      <header className="screen-head sched-head">
        <div>
          <h1 className="routines-title">Routines</h1>
        </div>
        <button
          type="button"
          className="btn-primary"
          disabled
          title="The routine editor was not observed and no routines store exists yet (docs/23 §7 addendum item 6)"
        >
          <NewRoutineGlyph /> New routine
        </button>
      </header>

      <div className="screen-empty">
        <StopwatchArt />
        <h2>No routines yet.</h2>
        <p />
      </div>
    </div>
  );
}
