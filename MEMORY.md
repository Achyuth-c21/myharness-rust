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

## 7b. Phase 2 state (2026-10-22)

Phase 2 ("Configuration, profiles, and setup window", `docs/13` §4) is implemented on the
`phase-2` lane as nine commits (`623a019` … `cfadd90`), not yet pushed or merged.

| Slice | Shipped |
|---|---|
| 1 | `crates/config` — profile schema, atomic library (`d25005c`) |
| 2 | `crates/secrets` — keychain store, `SecretRef`/`SecretSlot` (`b66ccb2`) |
| 3 | `crates/config::forms` — per-provider schemas, model-ID policy, apply-time validation (`623a019`) |
| 4 | `crates/config::overlay` + `::health` — managed overlay, `getMerged`, health/recheck/taint (`0472ab5`) |
| 5 | `crates/secrets::helper` — credential helper script runner (`66aa883`) |
| 6a | `crates/config::tabs` + `crates/bridge::setup` — 8 tab schemas, the `settings.Custom3pSetup` namespace (`78e9c76`) |
| 6b | The setup window itself — renderer + Tauri (`06a44c8`) |
| 7 | `crates/config::egress` + `::export` — egress derivation, redacted export (`44b169e`) |
| 8 | `crates/bridge::helper` — model discovery, published catalog (`b29a3bb`) |

**Deliberately not implemented, with the reason recorded in `docs/14` §9:**

- The **bootstrap fetch** (the transport). The trust/signature policy for
  bootstrap delivery was never captured, and `trustBootstrapDelivery` is a
  security-sensitive opt-in — a transport on an unverified trust policy would be
  a defect. The *state machine* around it is implemented.
- The **catalog signature check**. The shipped public key was never captured, so
  there is nothing to verify against; a stored catalog is treated as unverified
  and the documented failure path (bundled catalog + staleness warning) is taken.
- The **inference probe** ("Test connection"). It is an inference request and
  belongs to the Phase-3 provider adapters. The button is shown disabled.
- The **keychain-less fallback** (mode-`0600` inline profile field). The bridge
  refuses a renderer write that carries an inline secret, so on a machine with no
  usable keychain the setup window cannot set a credential at all.

**Test-ID layout in `docs/17` §4.8 — the IDs are not contiguous, by design:**

- `TEST-CFG-001..012` — the canonical `docs/13` §6.2 acceptance criteria.
- `TEST-CFG-101..112` — the `crates/config` library mechanics (renumbered out of
  the 001..012 block on 2026-10-22 to break a collision).
- `TEST-CFG-121..126` — the provider form layer. `141..147` — egress + export.
  `151..160` — the tab schemas + the bridge namespace. `161..166` — the setup
  window. `171..176` — model discovery + the catalog.

**Two traps of this phase:**

- The setup window is served over `app://localhost/setup-desktop-3p`, so in
  development it needs `MYHARNESS_DIST=apps/desktop/dist` like any other
  `app://` load — the Vite dev server alone does not serve it.
- The E2E suite needs the Vite dev server on 5173 running (`npm run dev` in
  `apps/desktop`); without it every case fails with `ERR_CONNECTION_REFUSED`,
  which looks like a regression but is not (`MEMORY.md` §3 trap #13).

### 7c. Phase-2 verification sweep (2026-10-23, post-merge)

All gates re-run on merged `dev` (HEAD `2d2a840`): cargo workspace 118/118, tsc clean,
vite build OK, E2E `shell.spec.mjs` ALL PASS (T01-32), `setup-window.spec.mjs` 16/16
(S01-16), native fixture 19/19 (R1-7 incl. dedicated setup window). Phase 2 is complete
per `docs/13` §4 exit criteria with the four recorded deferrals (`docs/14` §9): bootstrap
transport, catalog signature, inference probe (Phase 3), keychain-less fallback.
Setup-window deep-link grammar for manual runs: second launch
`target/debug/myharness-desktop myharness://setup-desktop-3p` (needs `MYHARNESS_DIST`).
An untracked stray `apps/desktop/src/components/Untitled-1` (a C file, editor artifact)
sits in the tree, referenced by nothing - owner decision pending; do not delete on my own.

## 7c. Phase 3 state (2026-10-23)

Phase 3 ("Provider adapters", `docs/13` §5) is implemented on the `phase-3` lane
(cut from `dev` @ `2d2a840`), stacked commits, not merged and not pushed. Slices 1–2
(through the Claude API adapter) are done. `docs/17` (§4.10 rows) and `docs/14` §9 items
21–26 are updated on disk per the standing policy, never committed.

| Slice | Shipped |
|---|---|
| 1 | `crates/provider-sdk` (§8 adapter contract), `crates/providers/gateway` (native adapter:
      discovery + `/v1/messages` SSE + probe + count_tokens), the `TEST-AI` wire battery
      (23 fixtures), `crates/bridge::helper` discovery delegation, `settings.Custom3pSetup`
      `testConnection` command + renderer wiring (Test connection performs a real inference) |
| 2 | `crates/providers/anthropic` — the Claude API family's native adapter (own crate, no
      shared code with the gateway crate beyond the sdk contract): first-party
      `api.anthropic.com/v1/messages` + `/v1/models` discovery + native
      `/v1/messages/count_tokens`, `x-api-key` auth with no scheme field, 21-fixture wire
      battery + 10 unit tests. Setup probe lane widened to `ProviderKind::ClaudeApi`
      (family-aware credential slot `inferenceAnthropicApiKey`; helper kind refused — the
      family offers static/interactive only). Renderer Test connection unlocked for
      `"gateway" | "anthropic"`. |

Durable traps and decisions worth not rediscovering:

- **Serde wire spellings are the contract's:** the adapter `AuthScheme` is
  `"bearer"` / `"x-api-key"` (kebab). A renderer or core literal `xApiKey` is dead on
  arrival — the earlier draft used it and the discovery probe silently broke. Test
  both spellings whenever a scheme resolves across the bridge.
- **The probe is two calls when the draft names no usable model** (discovery then a
  one-token `POST /v1/messages`), one call when it does (`docs/14` §9 item 21). The
  draft's model target comes from the first `inferenceModels` entry the Anthropic model
  policy accepts — `validate_anthropic_model_id` only rejects *non-Anthropic families*
  (`gpt-…`, `gemini-…`), not arbitrary words; a made-up benign word passes it and the
  adapter's model-echo check is what actually surfaces the mismatch.
- **`ConfigBackend::test_connection` is synchronous** and runs its own current-thread
  tokio runtime per call (the setup dispatch stays sync; the probe blocks the tauri
  command thread for its duration).
- **The helper credential path must use `run_helper`, not `HelperResolver`** for probe
  paths: the resolver writes fetched credentials to the keychain, and a probe must not
  persist anything.
- **`ProviderKind::ClaudeApi` serializes as `"anthropic"`** (docs/12 §1's family id), not
  `"claude-api"` — the renderer draft `provider` field and every serde round-trip key on
  that spelling.
- **The probe lane functions split per family** (`test_connection_gateway` /
  `test_connection_claude_api` as inherent `ConfigBackend` methods): a `match` arm lives
  in the trait impl, everything else must be inherent — extra fns inside a trait impl
  fail with E0407, not a borrow error.
- **The two ad-hoc local SSE mock servers (gateway wire.rs, anthropic wire.rs, bridge
  StubGateway) share a design, not code** — deliberately: adapters must not grow common
  transport plumbing that could drift into a shared gateway-shaped client.
- **Slice 2 decisions recorded in `docs/14` §9 items 24–26, summarized:** first-party
  discovery sends **no `limit` param** (the gateway's `limit=1000` is the gateway
  section's URL shape; the family's page size was never captured — a shape-tolerant page
  parse covers `data`/`models` + `id`/`model`/`name` + a cursor under
  `after_id`/`next_cursor`/`last_id`); the Claude API idle default applies the app's
  300-second reading (the family's own shipped default was not separately captured);
  health_check uses the plain first listing page (no injected `?limit=1` — a
  gateway-crate convenience reading not carried into the newer crate).
- **rustfmt drift is pre-existing and repo-wide (~61 files at `e37e08a`):** the
  2026-10-13 stable toolchain formats committed code differently than the toolchain the
  earlier gates ran on (config/secrets/app-shell/src-tauri all diff; verified via a HEAD
  worktree). Slice 2 formatted only its own files; reconciling the repo is a separate,
  explicit task — do not run bare `cargo fmt --check` as a slice gate until it happens.

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
  `docs/14` §9 supplement. Longer-running runtime behaviors are now partially exercised: deep links
  (argv channel: cold start + second-launch forwarding, `tests/native/shell-runtime.sh` 17/17);
  upstream OS scheme registration stays a Phase-9 packaging item; updater and window-state have
  still not been exercised end-to-end.
- `npm install` has since been run for the test toolchain; dev-server icons were never generated
  (`docs/21` state notes).
- No screenshot is approved for publication; the private archive stays outside the repo.
- Phase 2 is implemented but **not exercised end-to-end**: the setup window has not
  been driven in a real run against a live configuration library, and the
  renderer suite does not open it (`docs/17` §4.9, `docs/14` §9 item 19).
  `TEST-UX-009` is `Partial` for that reason.
- `TEST-CFG-009` (retention/legal-hold controls persist and enforce) is still
  `Unknown`: the Limits tab has a schema and validation, but "enforce" needs the
  store and the retention engine of Phases 4/7.
- **Push policy (owner decision, 2026-10-21): `docs/` and `evidence/` are never pushed again.**
  What is already on origin (dev/phase-1/`main` initial commit) will be handled by the owner later;
  do NOT rewrite remote history without an explicit fresh approval. Mechanism adopted:
  - commit code paths only (explicit pathspecs; never `git add -A` while docs are modified);
  - doc edits (docs/**, evidence/**) stay on disk uncommitted — they still must be *made* every
    slice per AGENTS §4.3, they just do not enter commits;
  - before every push: `git diff --name-only origin/dev..HEAD | grep -E '^(docs|evidence)/'`
    must print nothing; abort the push otherwise.
  - Slice-1 commit was re-cut to `d25005c` (code only) to honor this; the doc content lives in
    the worktree.
