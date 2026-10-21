# Bug hunt — phase-1 lane (2026-10-21)

Scope: **implemented code only.** The 24 six-line stub crates (`agent`, `artifacts`, `config`, `mcp`,
`plugins`, `projects`, `protocol-core`, `provider-sdk`, `scheduler`, `search`, `secrets`, `sessions`,
`shared`, `skills`, `store`, `telemetry`, `updates`, `providers/*`) contain no behavior and are
excluded — per instruction *"leave it if its not has code (not completed)"*.

Examined: `crates/app-shell` (`protocol.rs`, `csp.rs`, `lifecycle.rs`, `asset.rs`), `crates/bridge`,
`apps/desktop/src` (renderer), `apps/desktop/src-tauri` (Tauri shell), `tests/e2e/shell.spec.mjs`.

Verdict vocabulary per finding: **malfunction** (UI/behavior defect) vs **more serious** (spec
violation and/or security-relevant). Evidence: `Verified` / `Strong inference` / `Unknown`.
Status vocabulary per `docs/17`.

## Environment and test state

| Check | Result |
|---|---|
| `cargo test -p myharness-app-shell` | 13/13 pass (12 unit + 1 integration; `apps/desktop/dist` present, so the integration test is not a silent skip) |
| `npx tsc --noEmit` (apps/desktop) | clean, exit 0 |
| `npm run e2e` (`tests/e2e/shell.spec.mjs`) | 30/30 pass (T01–T30) on the final run; one earlier run showed a single T30 timeout (`a.nav-item` after `page.reload()`) that did not reproduce. The served document title on the dev port was checked first (`<title>myharness</title>`), so the failure was **not** the dev-server-port-squatter trap of `docs/14` §7 item 13 — it is a flake of unknown cause, not an observed regression |
| `myharness-desktop` (`src-tauri`) compile | **Unavailable** — `webkit2gtk-4.1`, `gtk+-3.0`, `soup-3.0`, `dbus-1` missing on this host, no sudo in session. Unblock: `sudo dnf install -y dbus-devel webkit2gtk4.1-devel gtk3-devel librsvg2-devel pkgconf-pkg-config` |

The full suite is green and still misses every finding below. No raw capture, log, screenshot, key,
token, host, or path is reproduced here; findings are published as prose only.

---

## A. Confirmed and empirically verified

### A1 — Forward button is permanently disabled; Back never re-greys at history ends
**`apps/desktop/src/components/TitleBar.tsx:20–33` (tracker), `:96–114` (buttons)** · **malfunction
AND spec violation**

```ts
let histDepth = 0; let histMax = 0;          // module scope, never decremented
histDepth += 1; histMax = Math.max(histMax, histDepth);
return { canBack: depth >= 2, canForward: depth < max };
```

`histDepth` only ever increments and `histMax` is its running maximum, so `depth === max` holds at
all times and `canForward` is permanently `false`. `canBack` is `depth >= 2` and never returns to
`false`, so Back stays enabled after the user is back at the first entry. There is no `popstate`
listener and no decrement on back/forward.

Spec: `docs/08:13` UI-INT-003 — *"Back / forward | Navigates SPA history; disabled/greyed at history
ends"*; `docs/06:196` ARCH-NAV-04 — *"Back/forward must map to SPA history, not reload the app"*;
`docs/06` §2 — *"Back/forward | Title bar arrows | SPA history"*.

Observed (Playwright against the dev server, SPA navigation `/` → `/new` → `/projects`):

```
D1 url: /projects    back disabled: false   forward disabled: true
D2 url after Back: /new    back disabled: false   forward disabled: true   ← should be false
D3 forward click: element is not enabled → forward navigation unreachable
```

Back is also still enabled at `/new`, the first history entry.

Why the suite misses it: `tests/e2e/shell.spec.mjs` T08 exercises **Back only**; no case clicks
Forward or asserts that Back greys out at the start of history.

### A2 — Closing settings leaves a zombie history entry; Back re-opens the settings modal
**`apps/desktop/src/App.tsx:60–64`** · **malfunction** (mild spec deviation)

```ts
const closeSettings = useCallback(() => {
  window.history.pushState(null, "", window.location.pathname);   // push, not replace
  setSettingsSection(null);
}, []);
```

`pushState` adds a history entry instead of replacing the settings entry, so the settings hash stays
in the history stack after the modal is dismissed. `docs/06` §1 — *"Closing settings removes the hash
and returns to the underlying route without losing the task/session state."*

Observed:

```
after close:        url /new          settings dialogs: 0
after browser Back: url /new#settings/general   settings dialogs: 1   ← modal re-opens
```

The same `pushState` also drops any query string on the current route (only `location.pathname` is
kept). `docs/08` T11/T25 cover settings via hash deep-load and history-back, but not
close-via-X-then-Back.

---

## B. Confirmed by code reading — not exercisable in this environment

### B1 — `docs/02` §4 host gate fails open when the `Host` header is absent
**`apps/desktop/src-tauri/src/lib.rs:41–46`** · **more serious (security gate, fail-open)**

```rust
let host = request.headers().get("host").and_then(|v| v.to_str().ok())
    .map(str::to_owned)
    .unwrap_or_else(|| shell::protocol::HOST.to_string());   // absent Host ⇒ the ALLOWED value
```

`docs/02` §4 gate 1 — *"Hostname must be `localhost`; other hosts return 404."* The request URI's own
host is never inspected; only the `Host` header is, and a request that omits it is treated as if it
targeted `localhost`. The gate therefore cannot reject anything for a hostless request.

The check is also case-insensitive on the *header* but the URI is not consulted at all, so a
`Host: localhost` header on a request for a different URI host passes.

Status: `Partial` — the gate exists and is exercised by `crates/app-shell`'s unit test
`wrong_host_is_404_not_403` (which calls `serve()` directly with an explicit host), but the Tauri
adapter around it fails open.

Impact: `Strong inference` — Tauri custom-protocol requests normally carry a `Host` header, so
practical exposure in the shipped shell is unconfirmed, and the Tauri shell cannot be built here
(environment gap above). The defect is in the fail-open direction, which is what the gate exists to
prevent.

### B2 — CSP builder permits directive injection (latent — no production caller)
**`crates/app-shell/src/csp.rs`** (`valid_origin`, `allowed_script_hash` handling) · **more serious
in principle, currently unreachable**

```rust
fn valid_origin(origin: &str) -> bool {
    origin.starts_with("https://") || origin.eq_ignore_ascii_case(ALLOWED_ORIGIN)
}
// script-src:
if hash.starts_with("'sha256-") { script.push(' '); script.push_str(hash); }
```

Any `https://`-prefixed string is accepted as a connect-src origin, so a value such as
`https://evil; script-src 'unsafe-inline'` injects a second CSP directive. Likewise a hash string
that merely *starts* with `'sha256-` can carry a trailing `'unsafe-inline'`, which lands inside
`script-src`. The existing test only covers the exact value `"'unsafe-inline'"`, which fails the
prefix test — so the suite passes while the injection path is open.

Reachability: a repo-wide search for `build_csp` / `CspConfig` finds no call site outside `csp.rs`.
The only consumer is `apps/desktop/src-tauri/src/lib.rs:58` → `shell::default_policy()`, an empty
config. Status: `Available, unexercised` for the builder, `Unknown` for exploitability. Reported as
latent hardening, not an active vulnerability.

---

## C. Real but lower severity

| # | Location | Issue | Verdict |
|---|---|---|---|
| C1 | `crates/app-shell/src/protocol.rs:239–249` | `"/."` normalizes to the root directory; `fs::read` on a directory returns EISDIR → `ResolveError::Io` → typed **503 `custom_3p_not_available`** instead of the SPA fallback a normal unknown path gets | malfunction (mislabeled error) |
| C2 | `crates/app-shell/src/protocol.rs:249` | Every non-`NotFound` I/O failure is reported with the third-party-mode envelope `custom_3p_not_available`, so a filesystem fault is reported as a product "not available in third-party mode" signal | malfunction (misleading signal) |
| C3 | `crates/app-shell/src/protocol.rs:232` | CSP is attached only when the path ends with lowercase `.html`/`.htm`; an uppercase-extension document would ship without CSP | malfunction (hardening gap, low) |
| C4 | `crates/app-shell/src/protocol.rs:207` + `crates/bridge/src/lib.rs` | `HttpRequest.kind` (`Navigation` vs `Fetch`) is never read by `serve()` — the field is dead, so the navigation/fetch distinction has no effect on any gate | dead code / `Improvement` |
| C5 | `crates/app-shell/src/protocol.rs` (`is_api_path` in `crates/bridge`) | `is_api_path` does not percent-decode, so an encoded prefix would not be classified. Latent only: the sole caller (`protocol.rs:220`) decodes first, so `/%61pi/x` is classified today | latent, `Unknown` |
| C6 | `apps/desktop/src/components/shells.tsx:26–30, 52` | `ErrorShell` renders the raw `error.message` in the UI and the boundary logs via `console.error` — internal error text surfaced verbatim | malfunction (low, information exposure) |
| C7 | `apps/desktop/src/screens/NewTaskScreen.tsx:135, 140–143` | `autoGrow` runs only from the textarea's `onChange`; `seedDraft` prefills programmatically without re-measuring, so the composer keeps its old height after an idea chip is clicked | malfunction (cosmetic) |
| C8 | `apps/desktop/src/components/focusTrap.ts` | The capture-phase `keydown` listener is attached to the *container*, so it fires only when focus is already inside the layer — the inline comment claims otherwise. T18/T21 show the trap working in the exercised flows | comment accuracy, low |
| C9 | `apps/desktop/src/App.tsx:66–89` | Global shortcuts do not check `event.target`, so `Ctrl+B` / `Ctrl+K` / `Ctrl+,` fire while the user is typing in the `#composer` textarea (e.g. `Ctrl+B` hides the sidebar mid-sentence) | `Improvement` — the global behavior itself matches `docs/08:11` UI-INT-001 / `:33` UI-INT-020 |
| C10 | `apps/desktop/src/components/TitleBar.tsx:93–101` | The panel toggle (now the **first** left control, the three-line one) is permanently inert **and its reason is undiscoverable**: it is a native `disabled` button, so it is not focusable (`element.focus()` does not reach it) and Chrome does not fire hover on it, so the `title="No auxiliary pane on this surface"` tooltip never renders. Verified by hover screenshot: no tooltip | malfunction (dead control, undiscoverable reason) — the *disabled state itself* is spec-consistent per `docs/08:12` UI-INT-002 ("when the surface supports it"); no surface in the rebuild supports an auxiliary pane (`docs/screens/task-detail.md:114` defines the pane toggle for the task-detail surface, which is not implemented) |

---

## D. Candidates investigated and **not** bugs

Recorded so they are not re-raised.

- **"The hamburger button is doing the work of the button next to it"** (user-reported) — *confirmed as
  a role/glyph mismatch, then corrected by the user.* What the code showed: the three-line control
  was wired to `onToggleSidebar` and was the only live control in the cluster, while the control
  beside it (`aria-label="Toggle panel"`) was inert. The three-line glyph therefore performed the
  whole-side-panel collapse, and the control that the reference pairs with the panel toggle was
  dead. This is not a hit-testing or overlap defect — `document.elementFromPoint` at the panel
  button's centre resolves to that button itself, the two glyphs are distinct, and the buttons sit
  8 px apart with no CSS overlap. It is a **role↔glyph mismatch**: the glyph a user reads as "menu"
  carried the sidebar-collapse action.
  **Adjudication and fix (2026-10-21):** the user confirmed the intent — *"it was supposed to close
  the whole side panel....the three lines is doing that.....which shouldnt"* — and the shipped app's
  own chrome supports it: its top-left three-line control is a *Menu* button and the sidebar is
  toggled by `Cmd/Ctrl+B` (app menu "Show/Hide Sidebar"). The two left controls keep their positions
  and their glyphs; **only the functions were swapped**: the three-line control is now the panel
  toggle (UI-INT-002, disabled with a runtime reason) and the framed-pane control beside it is the
  live sidebar toggle (UI-INT-001) that collapses/expands the whole left column. Recorded in
  `docs/14` §3 (unknown: the reference capture itself is not in the repo) and proposed in
  `docs/19` IMP-28.
- **Ctrl+N wiping the composer draft** — *not a defect.* `docs/08:15` UI-INT-005 explicitly states
  Ctrl+N *"Opens `/new`, clears the active composer context, focuses the composer"*, and
  `apps/desktop/src/App.tsx:116` remounts `<NewTaskScreen key={newKey} …/>` to do exactly that. The
  un-scoped-shortcut aspect is listed as C9 above.
- **SearchPalette stale highlight index** — *disproved.* The index resets on query change; observed
  6 rows → highlight on "Scheduled", 2 rows after "co" → 1 highlighted, Enter navigated to
  `/cowork`, palette closed.
- **Fixed-position modal controls escaping the focus trap** — *disproved.*
  `apps/desktop/src/styles/global.css:416` makes `.overlay-backdrop` `position: fixed`, so overlay
  children keep a non-null `offsetParent` and stay inside `focusTrap`'s `focusables()` filter.
- **`theme.ts` `matchMedia` listener leak** — *withdrawn.* `apps/desktop/src/main.tsx:11` calls
  `initTheme()` exactly once, so the listener is registered once for the app lifetime.

---

## E. Notes and unknowns

- `docs/06` §6 describes the shipped app as registering a `claude://` scheme; the rebuild implements
  `myharness://` (`crates/app-shell/src/lifecycle.rs:45`). `docs/06` §6 also marks the exact deep-link
  route grammar as **unknown**, so this is recorded, not judged.
- `crates/app-shell/src/lifecycle.rs:49–64` rejects any deep link carrying a fragment
  (`myharness://cowork#frag` → `RouteNotAllowed`) because the route segment is never split on `#`.
  With the grammar marked `Unknown` in `docs/06` §6 this is an `Improvement`/design question, not a
  confirmed defect.
- The unhandled-API-path 503 envelope `{"error":{"type":"custom_3p_not_available", …}}` is expected
  third-party-mode product behavior per `docs/10` §5.2 and `docs/18`, not a bug.
- `docs/17` status for each item: A1 `Noncompliant` (shipped behavior contradicts `docs/08` UI-INT-003
  and `docs/06` ARCH-NAV-04); A2 `Partial`; B1 `Partial`; B2 `Available, unexercised`; C1–C10
  `Partial` / `Improvement` as marked per row.
- The title-bar role↔glyph swap (section D) is **implemented and verified**: the two left controls keep
  their positions and glyphs; the three-line control is the panel toggle (disabled with a runtime
  reason) and the framed-pane control beside it is the live sidebar toggle (UI-INT-001). Spec lines
  updated in `docs/screens/new-task.md:46` and `docs/05-screen-inventory.md:48`; the reference capture
  itself is absent from the repo, so the pairing is recorded as an unknown in `docs/14` §3 and
  proposed as `docs/19` IMP-28 (a proposal, not shipped behavior).

---

## F. Fixes applied (2026-10-21, phase-1 lane — no merge)

All finding files list commit-fresh at fix time. Verdicts resolved per `docs/17` vocabulary.

| # | Fix | Files | Evidence after fix |
|---|---|---|---|
| A1 | Forward is live and Back re-greys at history ends: the position is read from React Router's `history.state.idx` (init 0, +1 on push) instead of a monotonic visit counter; `maxIdx` (session-furthest idx) gates Forward; `popstate` recomputes; unknown states stay inert. New hook `useHistoryButtons` with `key` in deps so same-route pushes refresh | `apps/desktop/src/components/TitleBar.tsx` | **T31** (new E2E): Back disabled at first entry → after two pushes Back on + Forward off → Back → Forward enabled → Forward navigates → two Backs → Back re-greyed at first entry + Forward enabled. `Noncompliant → Available` |
| A2 | `closeSettings` now `replaceState`s the settings entry (no zombie Back entry), keeping pathname **and query**; `openSettings` pushes while carrying the existing history state so the position index survives | `apps/desktop/src/App.tsx` | **T32** (new E2E): two open/close cycles then one browser Back — no hash, modal does not re-open. T11/T25 unchanged green. `Partial → Available` |
| B1 | Host gate fails **closed**: an absent `Host` header consults the request URI's authority, else answers with an empty host (404 per gate 1) instead of defaulting to the allowed value | `apps/desktop/src-tauri/src/lib.rs` | **Compile-verified 2026-10-21** — after the system libraries were installed, `cargo check` in `apps/desktop/src-tauri` finishes clean (was impossible before: the crate had never type-checked on this host; §Environment). The first compile surfaced two latent blockers, fixed in the same train: the sync protocol callback must return `Response` directly (uniform fallback on adapter construction failure), and the four icons were 1×1 gray-alpha placeholders rejected by `generate_context!` (now RGBA PNGs of the documented orange starburst). `Partial → Partial (compile-verified; rejection path logic-only)` |
| B2 | CSP builders hardened: origins accept only the app origin (case-insensitive) or `https://<host>[:port]` over an authority-only charset; script hashes must be fully closed `'sha256-<digest>'` integrity literals (no internals with quotes/spaces/`;`) | `crates/app-shell/src/csp.rs` | New unit tests: `csp_values_cannot_inject_directives` (the report's exact poison strings produce single directives, no smuggled sources), `script_hash_must_be_closed_integrity_literal` (6 malformed forms rejected). `Available, unexercised → Available (unit-verified)` |
| C1 | Directory reads classify as `NotFound` (not EISDIR→503): `/.` and any directory path take the SPA fallback like other unknown routes | `crates/app-shell/src/protocol.rs` | New unit test `directory_paths_take_the_spa_fallback` (`/.` and `/assets/sub` → 200 shell body with CSP). `Partial → Available` |
| C2 | Residual documented, **not** code-changed: genuine filesystem faults still answer the only contractually defined envelope (`custom_3p_not_available`, docs/18); inventing a new kind would break `docs/18`. Directory reads (the real-world trigger) eliminated by C1 | `crates/app-shell/src/protocol.rs` (comment), `docs/14` §9 supplement | `docs/14` §§9-record. `Partial → Partial (documented unknown)` |
| C3 | CSP attaches case-insensitively (`.HTML` documents no longer ship without it) | `crates/app-shell/src/protocol.rs` | New unit test `csp_header_is_attached_case_insensitively`. `Partial → Available` |
| C4 | `HttpRequest.kind` is now consumed: only top-frame **navigations** get the SPA fallback; a fetch of an unknown extensionless path is a plain 404 (it must not receive the app shell HTML with a 200) | `crates/app-shell/src/protocol.rs` | New unit test `fetch_unknown_route_is_404_not_the_shell`; navigation behavior unchanged. `Improvement → Available` |
| C5 | Contract made explicit: `is_api_path` requires a single-decoded path (encoded prefixes already classified via the sole caller's decode; double-decode impossible) | `crates/bridge/src/lib.rs` (doc) | Bridge tests unchanged green. `Improvement → Available (documented)` |
| C6 | `ErrorShell` no longer echoes internal error text into the UI — generic copy; details remain in console for diagnostics (telemetry redaction lands Phase 7, docs/10 §8) | `apps/desktop/src/components/shells.tsx` | tsc clean; boundary renders generic shell. `Partial → Available` |
| C7 | Suggestion prefill re-measures the textarea after the value lands (`autoGrow` in the prefill path) — previously only `onChange` grew the card | `apps/desktop/src/screens/NewTaskScreen.tsx` | Live check: idea click prefills the composer (value landed, no console errors); one-line suggestions stay at the 64px min-height by design, growth applies to wrapping prefills. `Partial → Available` |
| C8 | Comment corrected: the container-scoped capture listener fires only while focus is already inside the layer (that is the case it polices; `inside===false` branches are the invoker edge) | `apps/desktop/src/components/focusTrap.ts` | T18/T21 still green. `Partial → Available (accuracy)` |
| C9 | Global shortcuts scoped while typing: with an editable target and no overlay open only Ctrl+N stays global (UI-INT-005 is unconditioned); overlays keep their shortcut flows (T19). Adopted as `docs/19` IMP-29 | `apps/desktop/src/App.tsx`, `docs/19-improvement-register.md` | Live check: Ctrl+B inert mid-sentence (sidebar stayed open), Ctrl+N still cleared + focused. T05/T13/T19/T27 green. `Improvement → Available ( IMP-29 adopted)` |
| C10 | Panel toggle is inert but **discoverable**: `aria-disabled` + tooltip instead of native `disabled` (which was unfocusable and showed no tooltip), styled to match; sidebar toggle next to it unchanged and live | `apps/desktop/src/components/TitleBar.tsx`, `apps/desktop/src/styles/global.css` | Live check: focus lands on the control, tooltip reason present, sidebar toggle still collapses. `Partial → Available` |
| D | Already implemented and verified before this section (role↔glyph swap per user adjudication); re-confirmed against current source during the fix pass | `apps/desktop/src/components/TitleBar.tsx` | T04/T14 and file review. `Partial → Available` |

**Rust gates:** `cargo test -p myharness-app-shell -p myharness-bridge` → 17 + 1 + 3 = **21 pass**
(13→17 app-shell via 4 new protocol/csp tests, +1 hash-literal suite split). `cargo metadata` OK.
**`cargo check` in `apps/desktop/src-tauri` now also passes clean** (system libraries installed and
two latent first-compile blockers fixed — see the §F B1 row and `docs/14` §9 supplement), so the
whole repo compiles on this host.

**E2E battery:** extended to **32 cases** (T31/T32 above). Full runs: the battery surfaced one
one-off T30 reload-timeout (the same unknown flake recorded in §Environment) that cascaded into
T31/T32 in that single run; two consecutive full runs after it are **32/32 ALL PASS**. tsc and
`vite build` clean.
