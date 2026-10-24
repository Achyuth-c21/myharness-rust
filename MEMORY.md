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
- **Phase 3** — adapter order: **gateway → anthropic → bedrock → mantle → foundry**. (The
  Vertex family was **removed from rebuild scope** at the owner's direction, 2026-10-23 — §7c.)
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
(cut from `dev` @ `2d2a840`), stacked commits, not merged and not pushed. Slices 1–3
(gateway, Claude API, Bedrock) are done. **The Vertex slice (4) was removed from the
rebuild on 2026-10-23 at the owner's direction** — crate, wire battery, bridge probe
lane, config form family/keys, egress/export keys, and renderer unlock were deleted
outright; the Vertex family is out of rebuild scope (the shipped app keeps it,
`docs/09` §2.2 stays). The pre-removal state is preserved at commit `7b84886` on the
lane. `docs/17` (§4.10–§4.13 rows) and `docs/14` §9 items 21–28 are updated on disk
per the standing policy, never committed.

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
| 3 | `crates/providers/bedrock` — the Bedrock Runtime surface's native adapter (own crate):
      `POST /model/{id-or-arn}/invoke[-with-response-stream]` with the Anthropic Messages
      grammar minus the `model` field plus `"anthropic_version": "bedrock-2023-05-31"`,
      AWS EventStream binary framing (CRC-32C Castagnoli, encode + incremental decoder;
      1 MiB / <16-byte frame refusals), `nextToken` control-plane discovery on
      `/foundation-models` with no invented parameters, no native token count (explicit
      refusal, never an estimate), conditional model echo (an answer without a `model`
      field is not a refusal), 22 wire fixtures + 12 unit tests (published CRC-32C
      vector + hand-assembled frames). Setup lane `test_connection_bedrock`
      (`inferenceBedrockBearerToken` slot, helper kind allowed, `sso`/`external-idp`/
      `vendor-profile` refused fail-closed; region required; one base override answers
      both surfaces). `validate_bedrock_model_id` became `pub` (the family policy gate).
      Renderer Test connection unlocked for `"gateway" | "anthropic" | "bedrock"`. |
| 4 | **Removed (owner decision, 2026-10-23).** The `crates/providers/vertex` adapter, its 21-fixture wire battery and 10 unit tests, the `test_connection_vertex` bridge lane, the Vertex family in `crates/config` (form fields, `validate_vertex_model_id`, egress/export keys, `ProviderKind::Vertex`), and the renderer unlock were deleted. Root cause named by the owner: the wire-test build (reqwest + tokio + mock SSE server, debug-linked) crashed the laptop. Pre-removal snapshot: `7b84886`. |

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
- **Slice 4 (Vertex): withdrawn with the family on 2026-10-23.** The decisions that
  slice had recorded lived in `docs/14` §9 item 28, which is retained as the
  unknown/research record with a removal annotation; the code, fixtures, and the
  on-disk doc rows they produced sit at `7b84886` on the `phase-3` lane.
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
- **Slice 3 (Bedrock) decisions recorded in `docs/14` §9 item 27, summarized:** the
  family's wire was never observed, so the adapter's contract is one pinned reading —
  runtime host `bedrock-runtime.{region}.amazonaws.com` + control host
  `bedrock.{region}.amazonaws.com` (the single `inferenceBedrockBaseUrl` override
  replaces both); the body is the Anthropic grammar **without `model`** (the path
  carries the model or ARN, `/` pct-encoded, `:` kept); auth is a plain
  `Authorization: Bearer`; streaming is AWS EventStream binary frames (CRC-32C) whose
  payloads are Anthropic event JSON; empty-payload frames are keep-alives that never
  reset the idle window; exception envelopes map by name under
  `__type`/`reason`/wrapped `Output`; **a beta-carrying request is refused visibly**
  (no recorded beta transport — dropping silently would be the `AI-ADAPTER-07` mode in
  reverse). `NativeStopReason::parse("guardrail_intervened")`-class unknown stop
  spellings stay `Other` verbatim — preserving the native wording beats reclassifying.
- **rustfmt drift is pre-existing and repo-wide (~61 files at `e37e08a`):** the
  2026-10-13 stable toolchain formats committed code differently than the toolchain the
  earlier gates ran on (config/secrets/app-shell/src-tauri all diff; verified via a HEAD
  worktree). Slices 2–3 formatted only their own files; reconciling the repo is a
  separate, explicit task — do not run bare `cargo fmt --check` as a slice gate until
  it happens. (`bridge/src/lib.rs` and `config/src/lib.rs` still diff under
  `cargo fmt --check`; their slice-3 edits were kept format-clean by hand.)

## 7d. The open model layer (owner pivot, 2026-10-23)

On 2026-10-23 the owner approved widening the rebuild from an Anthropic-first
product into an **open multi-provider harness**: any model, any provider, plus a
user-defined OpenAI-compatible endpoint (design reference: the DeepSeek
harness). This rides alongside the in-flight Phase 3 work (§7c), not instead of
it. The decision record lives in `docs/14` §9 and `docs/13` §C on disk (docs are
never agent-committed).

What changed, and why it is not the negative fixture's mode:

- **All model-ID policy gates are removed by owner decision.**
  `validate_open_model_id` (`crates/config`) checks shape (a non-empty id),
  never family; the per-family validators no longer reject foreign families.
  Every id travels verbatim; a rejection is the provider's own wire error,
  mapped to `PROVIDER_ERROR` with the provider's message. This is a uniform
  policy change with honest error surfacing — the opposite of
  `AI-ADAPTER-01..14`'s per-call circumvention. The five families' test flips
  prove it: `a_non_anthropic_id_travels_verbatim` (gateway + anthropic) and
  `a_non_bedrock_id_travels_verbatim` (bedrock, which posts
  `/model/{id}/invoke-with-response-stream`, not converse-stream).
- **Wire families, not per-vendor crates.** The app-internal format stays
  Anthropic Messages; each provider ships its own adapter translating to its
  native wire. `crates/providers/openai` (`myharness-provider-openai`) is one
  native wall for the whole OpenAI Chat Completions wire: the first-party
  `openai` family (base optional, default `https://api.openai.com/v1`, static
  key required — empty credential refuses before send) and the `custom` family
  (base required at lane level, credential OPTIONAL → a keyless send with no
  `authorization` header, `docs/14` §9 owner decision). `AdapterKind`
  kebab-spells `"openai"` / `"custom"`.
- **The OpenAI wire's shapes, locked by the 21-fixture battery**
  (`crates/providers/openai/tests/wire.rs`): system blocks join `\n\n` into one
  `role:"system"` message; `max_tokens` (not `max_completion_tokens`); no
  `stream_options`; `top_k`/thinking/betas/`cache_control` dropped; images →
  `image_url` data URI; documents and tool-result media → explicit
  `UnsupportedModality` (never silently dropped); assistant Reasoning /
  RedactedThinking omitted from replay; reasoning streamed via
  `delta.reasoning_content` (fallback `delta.reasoning`); tool args stream via
  `delta.tool_calls` with synthesized id `chunk.id-{index}` (`call-{index}`
  when the chunk carries no completion id); close-assembling happens on
  `finish_reason`; stop map stop→end_turn, length→max_tokens,
  tool_calls/function_call→tool_use, content_filter→ContentFilter, unknown→
  `Other` verbatim; echo = **prefix match** (`gpt-4o-2024-11-20` is legal for
  `gpt-4o`; real substitution → `ModelNotFound`); usage
  prompt/completion_tokens + `prompt_tokens_details.cached_tokens`; a
  `data: {}` empty-object frame is a keep-alive Ping; `data: [DONE]` sentinel
  required else error; **count_tokens → explicit refusal** (never an estimate).
- **Bridge:** one shared lane `test_connection_openai_compat` with
  `OpenAiCompatFamily::{first_party, custom}`; `resolve_optional_probe_credential`
  returns `Ok(None)` for an absent/blank slot so the keyless send can exist
  (strict resolvers of the three original lanes unchanged). Probe target:
  the draft's first `inferenceModels` id, falling back to the endpoint's own
  `/models` listing first row. Transitional arm reduced to
  `BedrockMantle | Foundry` (message unchanged, tests updated).
- **Discovery:** `setup_discover_openai_models` is an ADDITIVE Tauri command —
  do not touch `setup_discover_models` (`apps/desktop/src-tauri/src/lib.rs`
  ~line 253; e2e fixtures reference it). Single page, 10 s, tolerant parse of
  three listing shapes (data array / bare top-level array / keyed `models`
  object; bare-string rows; id from `id` / `model` / `name`).
- **Renderer:** `ProviderKind` gained `"openai" | "custom"`
  (`apps/desktop/src/setup/client.ts`); net-new provider-family picker on the
  connection tab (blank stays unset; family switch keeps the shared keys —
  models, idle window, custom headers, credential-kind trio — and resets the
  family-specific ones); Test connection and discovery unlocked for
  `openai`/`custom`; `pendingSecrets` keyed `connection/<fieldKey>` as always.
- **State of the batteries at `8a3d1b8`:** gateway 23, anthropic 21 + 10 unit,
  bedrock 34, openai 21, bridge 40. `cargo metadata` green; renderer gates and
  the E2E spec green on this slice.
- **`mantle` / `foundry` stay stubbed:** their forms exist, but the probe lane
  refuses with the transitional message until their native adapters land —
  locked, not faked.

Traps: crate names are `myharness-provider-<family>`; `ProtocolFamily::OpenAiChat`
is the sdk's id for the wire; reqwest lowercases header names in mock
recordings (the openai tests' `Recorded::header` is case-insensitive).

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

## 7e. Phase 4 slice 1 — the persistence core (2026-10-24)

The `phase-4` lane (cut from `dev` @ `77b6bb3`) implemented the first slice of
`docs/13` Phase 4: `crates/store` (`fc2e62a`) and `crates/sessions` (`bdc472d`).
What is durable about it, so a later agent does not re-derive or contradict it:

- **The store is the rebuild's data truth, not a port of the shipped layout.**
  `docs/11` §4 (per-session `local_<uuid>.json` + `audit.jsonl` +
  `.audit-key`) stays the preservation baseline; the schema is `docs/11` §14's
  normalized model. 1:1 session-JSON parity is deliberately not attempted.
  Turn content lives in a content-addressed blob store (`sha256` names, reads
  and deletes validate the hex shape before touching a path); rows reference
  blobs by hash, `list_turns` never loads content bytes.
- **Migrations are forward-only with no down path.** A database written by a
  newer build refuses to open (`SchemaAhead`) untouched. Unknown session fields
  survive round-trips in `sessions.payload` (`TEST-DATA-001`). The supported
  version constant is pinned to the last migration by a unit test.
- **Corrupt-row policy is quarantine-then-fail-read.** The row moves to
  `quarantined_records`, the read returns `QuarantinedRecord { source_table,
  source_id }`, and the retry lists survivors. Never a silent skip; never an
  in-place repair. A corrupt database *file* is renamed `<name>.corrupt-<ms>`
  (never deleted) and `CorruptDatabase` is returned. Only real corruption
  signals trigger that path (`DatabaseCorrupt` / `SQLITE_NOTADB`); I/O and
  permission errors stay distinguishable.
- **Audit keys never live in the database.** `AuditKeyStore` is the seam; the
  slice-1 implementation is process memory with OS-random 32-byte keys. The
  keychain implementation lands with the secrets work (`TEST-CFG-006` remains
  compile-verified only). The chain adds a `prev_hmac` link the captures do not
  show — an implementation refinement, recorded as such in `docs/17` §4.15.
- **Delete honors legal hold and sweeps what it owns**: rows (one transaction,
  cascade), transcript blobs, and the audit key. Uploads/outputs/preview are
  artifact-phase scope, not this slice's.
- **Statuses are tracked in `docs/17` §4.15** (18 rows): Available =
  quarantine, chain, migrations, legal hold; Partial = `TEST-SESSION-001`
  (persistence half), 004, 007, `TEST-DATA-002`/`007`/`010`; Unknown =
  retention, aggregation, drafts, backup/export, Code worktrees.
- **`crates/protocol-core` stays a scaffold on purpose.** The turn types live
  in `myharness-provider-sdk` (Phase 3 put them there); protocol-core becomes a
  re-export facade only if the agent layer wants an app-side name. Do not
  create a second truth for `TurnEvent`.

Next slice was the turn loop — recorded in §7f.

## 7f. Phase 4 slice 2 — the turn loop (2026-10-24)

`crates/agent` implements one turn end-to-end (`run_turn(TurnRun)`): replay → user turn →
adapter `authenticate`/`create_turn` → typed-stream accumulation → assistant turn. It takes
`&dyn ProviderAdapter`; **adapter construction stays outside** (the profile→adapter factory
is slice-3 bridge work, not agent work).

- **The transcript record shape is `TurnContent` JSON**: `{ parts: [ContentPart…],
  citations: [...] }`. Citations are response-only and have no request-side home in the sdk
  model; they stay in the record and are **excluded from replay** on purpose. Anything that
  decodes transcript blobs must handle this shape (a bare `Vec<ContentPart>` will not read
  back).
- **Replay rules**: only `user`/`assistant` turns; empty turns skipped (an error turn that
  never received content); **consecutive same-role turns merged** — a failed send leaves
  `user, user`, which the Messages grammar rejects as separate messages. A missing/undecodable
  blob fails the replay (`Transcript`), never a shortened send.
- **Ordering**: validate (model-provider mismatch, empty model id, zero max_tokens, empty
  user, non-empty tools) → pre-cancel check → replay → **persist the user turn** → auth →
  send. Refusals write nothing and contact nobody; a pre-cancelled turn writes nothing.
- **Terminal states**: complete (stop received) / partial (cancelled mid-stream, content
  non-empty; no assistant row when nothing arrived) / error (provider error or stream ending
  without a stop reason). A stop/cancel race resolves as **complete** (the model did finish).
  Dropping the stream is the cancellation (`TEST-AI-015`).
- **Ledger**: one `usage_events` row per attempt that contacted (or tried to contact) the
  provider; failure rows carry the provider's message in `error`. Absent cache/web-search
  channels land as `0` in the ledger (numbers for aggregation) while the turn's `usage_json`
  keeps the exact `Option` shape. Counts are capped at `i64::MAX`, never wrapped.
- **`ToolSpec` declarations are refused before contact** (`AgentError::Unsupported`) until
  the MCP phase can execute them; the loop also refuses a model whose `provider_id` is not
  the adapter's `id()`. No retries: retry policy is unobserved and must not be invented.
- **Schema v2** (`migrate.rs` revision 2) adds `turns.stop_reason`; `Store::put_turn` and
  `SessionService::append_turn` grew a trailing `stop_reason: Option<&str>` parameter, stored
  as `NativeStopReason::wire()`. The v1→v2 upgrade runs as `ALTER TABLE` during fresh opens,
  which exercises the migration sequence in every test.
- **Tests at this slice**: agent 13 (`crates/agent/tests/turn.rs`, scripted adapter with
  per-call error injection and a pending tail for cancellation), store 21 (+ stop-reason
  reopen), sessions 8; clippy clean on the three crates. Pre-existing flake fixed: the store's
  activity-order test touched a session in the same millisecond it created its neighbor, and
  the `id ASC` tie-breaker made the assertion a coin flip.
- **Docs (disk-only)**: `docs/17` §4.15 now covers slices 1–2 (`TEST-SESSION-001` →
  Available (unit)); `docs/04` §3.1 has the slice-2 row.

Next slice (not started): profile→adapter factory + bridge commands + renderer chat surface
(`TEST-SESSION-002` app-restart check belongs there), then usage aggregation/drafts/retention
(`TEST-SESSION-005/008/009`, `TEST-DATA-004/007`).
