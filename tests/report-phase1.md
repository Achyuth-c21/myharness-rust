# Phase 1 Test Run Report — 2026-10-20

Branch under test: `phase-1` (workspace scaffold `9186853` + uncommitted Phase 1 slice).
Scope: security core (`app-shell`), bridge envelope, Tauri desktop wiring (compile-level), renderer shell (behavioral, browser E2E).

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
**13/13 PASS** (`node tests/e2e/shell.spec.mjs`)

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

### Layer 3 — Frontend build gates
`tsc --noEmit` clean; `vite build` succeeds (`dist/` regenerated with the settings-hash fix).

## Bugs found & fixed during this run

1. **Deep-link classifier route loss (product bug)** — `myharness://cowork/local_abc` was parsed as `host=cowork`, so the route never matched; hostless URIs fell through to `New`. Fixed to strict `scheme://<route>[/<arg>]`; locked by `lifecycle` tests (`crates/app-shell/src/lifecycle.rs`).
2. **Settings section hash desync (product bug)** — clicking a section in the settings modal updated state but not the URL hash. `onSection` now goes through the same hash-pushing path (`apps/desktop/src/App.tsx`).
3. **Test-side locator/query mistakes (test bugs)** — active-fill asserted on the inner span (fixed to `a.nav-item.active`); palette query string matched nothing (fixed to `cow`).

## Environment-caused gaps (not product failures)

- **System libraries missing** (Fedora, no sudo in session): `webkit2gtk-4.1`, `gtk+-3.0`, `soup-3.0`, `dbus-1` — all absent. Consequence:
  - `myharness-desktop` (src-tauri) crate cannot compile here; it is **feature-gated**: default `single-instance` on (docs/02 §2 / TEST-ARCH-002 behavior), `--no-default-features` builds without dbus.
  - The real windowed `cargo run` session is **pending** until the arch packages are installed.
  - Unblock command (todo once sudo is available):
    `sudo dnf install -y dbus-devel webkit2gtk4.1-devel gtk3-devel librsvg2-devel pkgconf-pkg-config`
  - E2E covers the renderer logic in a real browser; the webview-specific surface (native window, accelerators via OS, protocol inside wry) remains covered only by Layer 1 tests + Tauri APIs until then.

## Verdict

Phase 1 shell slice: **GO on all automatable layers** (16/16 Rust, 13/13 E2E, clean tsc + build), with the documented environment gap for the true windowed run.

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
