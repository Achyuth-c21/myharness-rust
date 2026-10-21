# Phase 1 Test Run Report — 2026-10-20

Branch under test: `phase-1` (workspace scaffold `9186853` + uncommitted Phase 1 slice).
Scope: security core (`app-shell`), bridge envelope, Tauri desktop wiring (compile-level), renderer shell (behavioral, browser E2E).

> **Second pass — docs-fidelity audit (same day).** The first pass proved behavior only; it did NOT
> prove spec fidelity, and the preview indeed diverged from the docs. Run 2 audited every Phase-1
> shell requirement against `docs/06`/`docs/07`/`docs/05`/`docs/screens/*` and corrected 9 deviations
> (see "Run 2" below).

> **Third pass — adversarial UI battery (same day).** Extended the E2E suite to **30 cases**
> (T18–T30 below): focus-trap races, stacked overlays, hash deep links + history-back, theme live
> follow, persistence garbage, composer keyboard, rapid-spam sidebar toggling. It caught **3 real
> product bugs** (all fixed, all regression-locked) plus 4 test-side determinism defects and one
> environment trap. Final state: **Rust 16/16, E2E 30/30 ×3 consecutive greens, tsc clean,
> build OK**, light/dark screenshots re-verified. See "Run 3" below.

## Environment

| Component | State |
|---|---|
| Rust toolchain (stable rustc, Fedora) | OK |
| Node 22.23.1 / Vite 5.4.21 / tsc | OK |
| Playwright chromium (fallback build, ubuntu24.04-x64) | OK |
| System GTK/WebKit/dbus dev libs | **MISSING** (no sudo in this session) — see Gaps |

## Results

### Layer 1 — Rust unit + integration (`cargo test -p myharness-bridge -p myharness-app-shell`)
**16/16 PASS**

- `app-shell::protocol` (7): wrong host→404 (not 403), bad Origin→403, traversal→403, CSP injected on `index.html`, SPA fallback + asset MIME, API paths→typed 503, empty dist still answers.
- `app-shell::csp` (3): tight default policy, script hashes never relax to `unsafe-inline`, app origins only when well-formed.
- `app-shell::lifecycle` (2): allowlisted routes resolve; unknown schemes / non-allowlisted routes rejected.
- integration `protocol_dist` (1): **real built `apps/desktop/dist`** served through `serve()` — `index.html` boots React with CSP, client route falls back, real bundled JS asset serves with JS MIME, `/api/*` returns typed 503, host/origin guards hold on the real root.
- `bridge` (3): envelope serde roundtrip, API path classification, 503 body shape (`custom_3p_not_available`).

### Layer 2 — Renderer E2E (headless chromium against Vite dev server @ :5173)
**30/30 PASS** (`node tests/e2e/shell.spec.mjs`) — three consecutive full-grid greens.

| Case | Verifies |
|---|---|
| T01 | `/` → `/new` redirect (`docs/06` §1) |
| T02 | title `myharness`; `html[data-theme=claude]`; system+dark-emulation → `data-mode=dark` |
| T03 | **no Node surface** in renderer (`typeof process === 'undefined'`) — TEST-ARCH-001 spirit |
| T04 | sidebar = New/Projects/Artifacts/Scheduled exactly; Customize disabled |
| T05 | Ctrl+N from deep route → `/new` (UI-INT-005) |
| T06 | nav click → `/cowork-artifact` with `.active` fill (UI-INT-006) |
| T07 | segmented Cowork\|Code → `/epitaxy`, `aria-selected` (UI-INT-004) |
| T08 | Back arrow walks real history (UI-INT-003) |
| T09 | Ctrl+K palette, filtering, Enter navigates (UI-INT-020/023) |
| T10 | "No results" + Escape closes, focus restored (UI-INT-026/027) |
| T11 | Ctrl+, → `#settings/general` hash over preserved route; section click moves hash; Escape clears hash only |
| T12 | `theme` key persists; explicit light beats OS-dark emulation (`docs/20` §4) |
| T13 | Ctrl+B collapses sidebar + `sidebar.collapsed` persistence (UI-INT-001) |

Run 2 additions (docs-fidelity audit):

| Case | Verifies |
|---|---|
| T14 | mode switch is IN the sidebar, not the title bar (`docs/06` §4) |
| T15 | `/new` greeting, starburst, composer copy, "Active"/"Clear active" header (new-task.md) |
| T16 | palette geometry 726×470, search row, placeholder, X (search-palette.md) |
| T17 | documented empty-state copy verbatim (`docs/05` §4.4–4.6) |

Run 3 additions (adversarial battery):

| Case | Verifies |
|---|---|
| T18 | settings focus trap cycles all focusables + visible box-shadow ring (UI-DS-04/05) |
| T19 | Ctrl+K over settings closes TOPMOST layer only; second Esc needed (`docs/08` §13) |
| T20 | scrim outside-click closes both settings and palette |
| T21 | palette trap keeps Tab inside; Esc restores focus to trigger button (UI-INT-026) |
| T22 | clear-X empties query but keeps palette open; subsequent Esc closes |
| T23 | settings search filter "pri"→Privacy only; "qqqq"→"No results"; Esc consumed first (clears query, keeps open), second Esc closes |
| T24 | theme segmented: live apply, `theme` persistence, System follows OS **live** on media flip |
| T25 | `/#settings/usage` deep load opens Usage; history back closes (hash-redirect fix) |
| T26 | garbage localStorage values tolerated; app stays usable (resilience) |
| T27 | Shift+Enter newline / Enter inert / Ctrl+N clears draft and refocuses composer |
| T28 | `/cowork-local-abc` announces task; Code greeting distinct; unknown routes redirect |
| T29 | rapid Ctrl+B spam ends in coherent state; final reload matches persisted value |
| T30 | focus ring is box-shadow, never outline (`docs/07` §3.1) |

### Layer 3 — Frontend build gates
`tsc --noEmit` clean; `vite build` succeeds (`dist/` regenerated with the settings-hash fix).

## Bugs found & fixed during this run

1. **Deep-link classifier route loss (product bug)** — `myharness://cowork/local_abc` was parsed as `host=cowork`, so the route never matched; hostless URIs fell through to `New`. Fixed to strict `scheme://<route>[/<arg>]`; locked by `lifecycle` tests (`crates/app-shell/src/lifecycle.rs`).
2. **Settings section hash desync (product bug)** — clicking a section in the settings modal updated state but not the URL hash. `onSection` now goes through the same hash-pushing path (`apps/desktop/src/App.tsx`).
3. **Test-side locator/query mistakes (test bugs)** — active-fill asserted on the inner span (fixed to `a.nav-item.active`); palette query string matched nothing (fixed to `cow`).

Run 3 battery bugs:

4. **Focus trap loses restore target (product bug)** — `autoFocus` on the dialog raced the trap's
   `previous` capture, so Esc restored focus to `<body>` and Tab walked the page behind the overlay.
   Fixed: `useFocusTrap` is ref-scoped, registers capture-phase listeners only while mounted, and
   performs the trap's own initial focus; `autoFocus` removed from dialogs
   (`apps/desktop/src/components/focusTrap.ts`, T18/T21 lock it).
5. **Hash dropped on unknown-route redirect (product bug)** — `/#settings/usage` hit the catch-all
   route which redirected to `/new` **without the hash**, so deep-linked settings sections never
   opened and T25 history-back behavior was wrong. Fixed: both `/` and `*` routes preserve
   `window.location.hash` into the redirect target (`apps/desktop/src/App.tsx`, T25 locks it).
6. **Ctrl+N did not reset the /new draft (product bug)** — route-level keys don't remount a matched
   subtree, so the composer kept prior context. Fixed: the remount key moved onto the
   `<NewTaskScreen key={newKey}/>` element (`apps/desktop/src/App.tsx`, T27 locks it).
7. **Test determinism defects (test bugs)** — keypresses right after `goto`/dialogs raced React
   mount/unmount: added `waitForSelector("#composer")` before key sequences (T18/T23/T24), a
   palette-closed settle assert in T22, a guard Esc for a lingering overlay layer in T23, a
   `waitForTimeout(120)` after the OS-emulation flip in T24 (matchMedia listeners fire async), and
   T30 now re-persists the sidebar open + focuses the nav anchor itself (T29 leaves it collapsed).

## Environment-caused gaps (not product failures)

- **System libraries missing** (Fedora, no sudo in session): `webkit2gtk-4.1`, `gtk+-3.0`, `soup-3.0`, `dbus-1` — all absent. Consequence:
  - `myharness-desktop` (src-tauri) crate cannot compile here; it is **feature-gated**: default `single-instance` on (docs/02 §2 / TEST-ARCH-002 behavior), `--no-default-features` builds without dbus.
  - The real windowed `cargo run` session is **pending** until the arch packages are installed.
  - Unblock command (todo once sudo is available):
    `sudo dnf install -y dbus-devel webkit2gtk4.1-devel gtk3-devel librsvg2-devel pkgconf-pkg-config`
  - E2E covers the renderer logic in a real browser; the webview-specific surface (native window, accelerators via OS, protocol inside wry) remains covered only by Layer 1 tests + Tauri APIs until then.

## Verdict

Run 1: GO on all automatable layers (16/16 Rust, 13/13 E2E, clean tsc + build) — **but behavior-only**.
Run 2 (docs-fidelity audit): 9 spec deviations found and fixed; 17/17 E2E — **GO**.
Run 3 (adversarial battery): 3 product bugs found and fixed; 30/30 E2E ×3 consecutive greens,
16/16 Rust, clean tsc/build, light+dark screenshots verified — **GO**.
Run 4 (user-attested UI corrections): title-bar topology/glyphs, composer focus + model pill,
greeting alignment corrected against the shipped app; 30/30 E2E, clean tsc/build — **GO**.
Environment gap for the true windowed run remains (GTK/WebKit/dbus dev libs need sudo).

## Run 3 — adversarial UI battery (same day)

Scope: try to break the Run-2 shell with races, stacking, persistence, and history abuse.
Method: 13 new cases (T18–T30), each run inside the full 30-case sequence (one browser context,
one shared page — cross-test state is the adversary, not per-test isolation), plus isolated
debug scripts for any failure before classifying it.

Environment trap recorded (before trusting ANY red suite run):

- A **stale foreign dev server** was holding `:5173`; the fresh `vite` failed with
  "Port 5173 is already in use" and the suite tested the wrong app entirely (title `Claude`,
  server-rendered theme attrs — not ours). Whole-grid reds under a port conflict are
  harness failures, not product regressions. Rule: verify `curl :5173/` serves **title
  `myharness`** before interpreting results (recorded as trap 13 in `docs/14` §7).

What the battery caught (all fixed in this slice; see Bugs 4–7):

- Product: focus-trap trigger restore (T18/T21), hash-preserving redirect (T25),
  Ctrl+N draft reset remount key placement (T27).
- Test-side: missing determinism gates listed above.

Evidence honesty note: `docs/*` never spec a **document** title (screens spec only the custom
title *bar*); `title="myharness"` in `index.html`/`tauri.conf.json` is the project's own label,
not a captured spec value. T02 asserts it as a plumbing/regression check, not as evidence.

Evidence of stability: *three consecutive* full-suite greens after the fix batch
(pass_count 30, exit 0 each), then Rust (16/16), `tsc --noEmit`, `vite build` all clean,
and light/dark screenshots re-captured with no visual regression.

## Run 4 — user-attested UI corrections (2026-10-21)

Source: the user compared the preview side-by-side with the shipped app and flagged the
title bar, composer, and model pill. Each change cites its basis; where the docs are
silent, the change is labeled user-attested and handled per `docs/14` §9:

1. **Title bar topology** — left: hamburger sidebar toggle, back, forward; right: globe
   (web/browser context, `task-detail.md` §Layout order `[globe] [pane]`), panel toggle,
   then platform window controls. `docs/05` lists the controls without sides; the burger
   glyph and the right-side cluster placement are **user-attested** from the shipped app.
2. **Sidebar toggle glyph** = 3-line hamburger (user-attested; `docs/07` §1.5 leaves exact
   codepoints to the icon set).
3. **Window controls** render as real SVG glyphs (– □ ×), full-bar hit areas, flush to the
   corner; still inert until the desktop/window bridge lands.
4. **Composer focus** — the composite card carries the keyboard-focus ring via
   `:has(:focus-visible)`; the boot-time ring on the textarea was a rebuild artifact the
   shipped app does not show. Boot no longer autofocuses: focus follows the explicit
   Ctrl+N invocation (UI-INT-005, `focusOnMount` prop).
5. **Model pill** = selected-pill treatment (raised surface + darker text, `docs/07` §3.6);
   label text stays "Model" until provider wiring (`model-picker.md` display labels come
   from provider config, `docs/13` §Phase 3).
6. **Greeting column** — starburst and composer card share the 720px column, so their
   left edges align exactly (`new-task.md` §Layout).

Gates after the batch: `tsc --noEmit` clean, `vite build` OK, E2E **30/30** (two consecutive
runs), light/dark screenshots re-verified; geometry audit shows the model pill within 2px of
the +/send midpoint (documented "model label centered").

## Repro commands

## Run 2 — docs-fidelity audit (behavior vs docs)

| # | Doc requirement (source) | Was | Now |
|---|---|---|---|
| 1 | Cowork\|Code switch lives at the top of the **sidebar** (docs/06 §4; docs/screens/new-task.md layout) | rendered in the title bar center | in the sidebar (`ModeSwitch.tsx`) |
| 2 | Title bar = sidebar toggle, **panel toggle**, back/forward in that order (docs/05 §2; UI-INT-002) | panel toggle missing | present, disabled w/ runtime reason (no pane yet) |
| 3 | Sidebar: `+ New`, Projects, Artifacts, Scheduled, Customize, then **Tasks header with search + filter icons**, account pinned bottom (new-task.md §Sidebar) | bare label list, "Account" generic | full structure; filter disabled until a list exists; no faux task rows |
| 4 | `/new`: serif greeting **“You’re here!”** ~36px with orange starburst left; 720px composer card, placeholder **“How can I help you today?”**, toolbar (+/model/send), second row **“Project or folder”** chip + info icon; **Active / Clear active** header (new-task.md §Layout/Regions) | generic centered placeholder | `NewTaskScreen.tsx` implements the documented layout; Enter-inert until sessions (Phase 4) — no faux navigation |
| 5 | Search palette ~**726×470**, 48px search row, magnifier, placeholder **“Search chats and projects”**, clear X, full-row rounded highlight, surface icons (search-palette.md) | 560px, “Search locally…”, no icons/clear | rebuilt to the measured spec (T16 asserts 726×470) |
| 6 | Settings: left nav light-gray ~208px with ~175px **“Search…”** field, groups **Settings / This computer / Customize**, close **X** top-right, display labels (settings.md §Modal geometry) | raw hash slugs, no search, no X | groups + labels from settings.md; heading search filters with “No results”; X closes (UI-INT-107) |
| 7 | Theme control: **System/Light/Dark segmented icon control** in General (settings.md §General; UI-INT-105; docs/20 §4 key `theme`) | not present | wired in General, persists `theme`, applies live |
| 8 | Empty states with captured copy — Projects **“Looking to start a project?”**, Artifacts **“Create your first artifact”**, Scheduled **“No scheduled tasks yet.”** + wake/online banner (docs/05 §4.4–4.6) | generic placeholders | implemented verbatim; header actions rendered but disabled until their flows |
| 9 | Ctrl+N clears the composer context **and focuses the composer** (UI-INT-005) | navigation only | remounts `/new` and focuses the composer |

Deliberate non-changes (evidence discipline — copy/geometry NOT in evidence is never invented):
- Send/model/folder/add controls render per spec but stay **disabled with runtime reasons** — session
  creation is docs/13 §Phase 4, model picker §Phase 3; UI-INT acceptance for those flows is phase-gated.
- Palette rows surface route entries only (stand-in, flagged in code); sessions come with Phase 4.
- Sidebar labels stay **New/Projects/Artifacts/Scheduled/Customize** exactly as docs/06 §2 writes them.
  Note for the locale pass: `evidence/i18n-en-US-values.txt` contains `New Chat` (keys `0aHdFu/l3q`,
  `hPVHFt5f9s`) — flagged for reconciliation, docs text wins until then.

New E2E coverage for the audit (all in `tests/e2e/shell.spec.mjs`): T14 switch-in-sidebar,
T15 greeting/composer/copy column, T16 palette geometry, T17 documented empty-state copy.

## Repro commands

```bash
# Rust
cargo test -p myharness-bridge -p myharness-app-shell
# Renderer E2E
npm install && npx playwright install chromium
nohup npm run dev --prefix apps/desktop > /tmp/vite.log 2>&1 &
node tests/e2e/shell.spec.mjs
# Frontend gates
cd apps/desktop && npx tsc --noEmit && npm run build
```
