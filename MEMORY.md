# MEMORY.md — Durable Project Memory

**Facts that are true, would be expensive to rediscover, and should survive across sessions.**
Everything here is sourced. If you cannot cite a source for an entry, it does not belong here.

Last verified against the repository: **2026-10-20**.

---

## 1. Orientation

- **Repo:** `myharness-rust` — a blueprint for rebuilding Claude Desktop 3P as Tauri 2 + Rust.
  It is a documentation/evidence repository, not a working clone. Status per `STACK.md`:
  *structure only — no application code yet* (the scaffold has since advanced to a Phase-1
  slice; see §6).
- **Inspected product:** Claude Desktop `2.42304.0`, Electron `44.7.2`, Chromium `152`,
  Node `24.21.0`, on Fedora Linux.
- **The renderer is local, not remote.** The SPA is `resources/ion-dist` (4,224 files, 201 MB),
  entry `assets/v1/index-D3kGySMO.js`; there are **no source maps** (`docs/04`, `docs/15`).
- **Doc map:** `docs/00`–`docs/22` plus `docs/features/` (15 files) and `docs/screens/`
  (10 files). `README.md` holds the map and the conventions.
- **The main and setup windows share an origin.** CDP target selection therefore needs a URL
  substring (`/new`, `epitaxy`, `cowork/`) — a known trap (`docs/14` §7 trap #4).

## 2. Known doc conflicts (do not silently pick one)

| Conflict | Resolution |
|---|---|
| `docs/13` §2 recommended-stack table still says **Electron/TypeScript**; `STACK.md`, `docs/21`, the Cargo workspace, and the code are **Tauri 2/Rust** | Follow **Tauri/Rust**. The `docs/13` §2 row is stale. |
| `docs/04` calls itself an *investigation* tracker, not a product tracker; `docs/13` is the rebuild roadmap | Keep the two separate. `docs/04` = what we learned about the app. `docs/13` = what we build. |
| `docs/19` proposals vs `docs/03` feature inventory | Proposals are not shipped behavior. Never merge their vocabularies. |

If you fix one of these, fix it in the doc — do not leave a new inconsistency behind.

## 3. The known traps (`docs/14` §7) — read before trusting an observation

1. Current state ≠ shipped state.
2. Model count ≠ selectable models.
3. Feature visible ≠ feature usable.
4. The setup window shares an origin with the main window.
5. Menus remain mounted (close by toggling the trigger).
6. "New artifact" is destructive.
7. Screenshot contents are personal.
8. Logs include private infrastructure.
9. Session JSON is huge.
10. Catalog model names are not entitlements.
11. The advisory update banner does not mean an update was applied.
12. No source maps.
13. A stale foreign process on the dev port answers E2E with another app; verify the served
    document title before interpreting any red suite run.

## 4. The noncompliant local adapter — the most important trap in the repo

The live install contains `native-adapter.cjs`: an **in-process Anthropic→OpenAI shim**, roughly
1,001 lines, whose policy path evaluates `So → true` — a policy bypass.

- It is documented as the 14 violations `AI-ADAPTER-01..14` (`docs/12` §7).
- **It is negative evidence.** It is the specimen of what not to build.
- It may be retained **only as a negative test fixture**.
- Never call it "native". Never copy from it. Never grow it. The compliant architecture is
  `docs/12` §8: Provider → protocol family → SDK plugin → Adapter → capability registry, with
  `ProviderAdapter`, `TurnEvent`, `ModelDescriptor` interfaces, and "unknown capability =
  unsupported".

## 5. Architecture facts worth not rediscovering

- **Trust boundaries** (`docs/10` §8): renderer↔preload, artifact/content↔app, MCP server↔agent,
  provider↔app, config↔secrets, deep link↔app, update feed↔app, remote bootstrap↔config,
  file picker↔filesystem.
- **Renderer privilege rule:** the renderer crate never links `crates/secrets` or
  `crates/providers/*`; everything privileged crosses `crates/bridge` (`ARCH-PROC-02/03/05`).
- **The `app://` protocol contract** (implemented in `crates/app-shell`): wrong host → **404**
  (not 403); bad `Origin` → 403; path traversal → 403; CSP injected into `index.html`;
  SPA fallback plus correct asset MIME; API paths → typed 503. API path prefixes:
  `["/api", "/edge-api", "/v1", "/v2", "/mcp-registry", "/sandbox", "/claudeai-rpc"]`.
- **The 3P-unavailable body** is fixed:
  `{"error":{"type":"custom_3p_not_available","message":"This functionality is not available in third-party mode."}}`
- **The shell serves a real built `apps/desktop/dist`** through `serve()`; `MYHARNESS_DIST`
  overrides the dist path; `shell::protocol::HOST` and `shell::default_policy()` are the
  entry points (`apps/desktop/src-tauri/src/lib.rs`).
- **Protocol classification is central**: `shell::RequestKind::{Fetch, Navigation}`.

## 6. Code conventions in this repo

- **Reference implementation style** — `crates/bridge/src/lib.rs`. Doc comments cite
  `docs/18`, `docs/10` §8, `ARCH-PROC-03`, `TEST-ARCH-001`, `ARCH-PROC-07`. Enums use
  `#[serde(tag = "type", rename_all = "camelCase")]`. `is_api_path()` holds the prefix list.
  `pub const NOT_AVAILABLE_BODY: &str = ...`. `#[cfg(test)] mod tests` with unit tests.
- **Stub convention** — `crates/provider-sdk/src/lib.rs`:
  `//! Scaffolded structure only - implementation arrives in a later roadmap phase.` and
  `//! TODO(phase): implement this module per its roadmap phase; wire deps via the root
  Cargo.toml workspace.dependencies.`
- **Workspace** — 26 members; root `Cargo.toml` carries `[workspace.package]` (version 0.1.0,
  edition 2021, repository `github.com/Achyuth-c21/myharness-rust`) and
  `workspace.dependencies` (tokio, serde, thiserror, anyhow, reqwest+rustls, keyring 3,
  secrecy 0.10, zeroize, rusqlite 0.32 bundled, portable-pty 0.8, tauri 2).
- **Root `package.json` is a tooling workspace only** — it exists for `tests/e2e`
  (playwright ^1.50.0, `npm run e2e` → `node tests/e2e/shell.spec.mjs`).
- **Test layers** (`tests/README.md`, `docs/13` §6.1): `unit/` (schemas, policy, token/usage
  math, redaction, path validation), `integration/` (config apply, DB migrations, MCP
  lifecycle, retention, temp profiles), `e2e/` (routes, composer, setup, settings, menus,
  permissions), `wire/` (native provider conformance against mock servers), `fixtures/`
  (synthetic only).
- **Skills library** lives in `.agents/skills/` (playwright-best-practices, rust-best-practices,
  rust-testing, rust-mcp-server-generator, rust-rmcp-server-development, security-review,
  sqlite-database-expert, several tauri-*, typescript-advanced-types,
  vercel-react-best-practices). **`.claude/` was empty** before this file set was added.

## 7. Roadmap phase order

- **Phase 0** — foundations; exit criterion includes a lint rule/test that forbids global
  `fetch` monkey-patching and arbitrary provider URL translation.
- **Phase 1** — app shell + bridge (this is what exists today).
- **Phase 3** — adapter order: **gateway → anthropic → bedrock → vertex → mantle → foundry**.
- `docs/19` priorities: IMP-01 native adapters (P0), IMP-02 per-provider model policy (P0),
  IMP-03 capability-first UI (P0), IMP-04 signed lazy extension packages (P0), IMP-05 native
  protocol families (P1), IMP-06 MiniMax M3.1 Flash as transparent reconstruction default (P1).

## 8. Status vocabulary (`docs/17`)

`Available` · `Available, unexercised` · `Partial` · `Policy-disabled` · `Unavailable` ·
`Unsupported` · `Unknown` · `Noncompliant`.

Use these words exactly. Do not invent near-synonyms. "Available" describes the *inspected
installation*, not all platforms or deployments.

Test-ID areas: ARCH, UX, SESSION, AI, CFG, MCP, SEC, ART, CODE, SCHED, DATA, PRIV, UPDATE, EXT,
AUTH, I18N. Format: `TEST-<AREA>-<NNN>`.

Other ID prefixes: `UI-*`, `FEAT-*`, `ARCH-*`, `API-*`, `DATA-*`, `AI-*`, `REQ-*`, `IMP-*`,
`AI-ADAPTER-NN`.

## 9. Git model

`main` frozen, maintainer-only · `dev` is the integration branch · `phase-*` / `feature-*` are
disposable lanes cut from `dev`. Merge back with `--no-ff`. Agents must never push to `main`,
never `git checkout main` for edits, never merge into `main`, never force-push a protected
branch. `evidence/app-asar-orig/` is git-ignored by design.

## 10. Where the answer lives — quick index

| Question | Go to |
|---|---|
| How do I start a task? | `AGENTS.md` §4 |
| What must I not do? | `RULES.md` |
| Am I done? | `CHECKLIST.md` |
| What does the app do on screen X? | `docs/screens/*` |
| What is feature Y? | `docs/features/*` |
| What is the spec ID for my change? | `docs/17-traceability.md` |
| Where is the shape of the data? | `docs/11`, `docs/18`, `docs/20` |
| What are the provider rules? | `docs/12` |
| How do I handle an unknown? | `docs/14` §9 |
| Which phase am I in? | `docs/13` §4, `docs/04` §1 |
| What is behind a claim? | `docs/15`, `evidence/` |
| Why is this a trap? | `docs/14` §7, and §3/§4 above |
| What is proposed but not built? | `docs/19` |

## 11. Open items and known gaps

- `docs/13` §2 stack table still contradicts `STACK.md` — a one-line doc fix is owed.
- Projects, Artifacts, and Scheduled tasks are only **Partial** in the investigation
  (`docs/04` §1): existing-project detail was never created, the artifact pane was never
  available, create/edit and history for scheduled tasks were never exercised.
- Phase-1 initially lacked system GTK/WebKit/dbus dev libraries. **Installed 2026-10-21**
  (`webkit2gtk4.1-devel`, `gtk3-devel`, `libsoup3-devel`, `dbus-devel`, `librsvg2-devel`,
  `pkgconf-pkg-config`); `cargo check` on `apps/desktop/src-tauri` passes and the first real windowed
  run launched (`MYHARNESS_DIST=apps/desktop/dist cargo run` in `src-tauri`) — full details and the
  two latent first-compile blockers it surfaced are in `tests/report-bug-hunt-phase1.md` §F and
  `docs/14` §9 supplement. Longer-running runtime behaviors (deep-link routes, updater, window-state)
  have still not been exercised end-to-end.
- `npm install` has since been run for the test toolchain; dev-server icons were never generated
  (`docs/21` state notes).
- No screenshot is approved for publication; the private archive stays outside the repo.
