# MEMORY.md — Durable Project Memory

**Facts that are true, would be expensive to rediscover, and should survive across sessions.**
Everything here is sourced. If you cannot cite a source for an entry, it does not belong here.

Last verified against the repository: **2026-10-24**.

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
- **Workspace** — 27 members; root `Cargo.toml` carries `[workspace.package]` (version 0.1.0,
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

## 7k. Phase 6 slice 1 — Code dashboard + folder-backed session (2026-10-25)

The `phase-6-code` lane (cut from `dev` @ `d3d2d81`) implements `docs/13` Phase 6
Slice 1. Merged to `dev` and pushed 2026-10-26 with the owner's approval (merge
`f3fdc70`). Projects, Artifacts, Scheduled, and Search are slices 2–5 (§7l).

- **Policy (`crates/bridge/src/policy.rs`):** `CodePolicy` is read from the
  **merged profile**, never the renderer. `isClaudeCodeForDesktopEnabled`
  absent → enabled; `disableDesktopLocalSessions` true → local refused (remote
  placement does not exist — no fallback); `disableBypassPermissionsMode` is
  **fail-closed** (only an explicit `false` permits Bypass; absence/true/malformed
  all disable); `allowedWorkspaceFolders` accepts array / `{folders|paths}` /
  path→bool map (shape never observed — `docs/14` §9), malformed grants nothing,
  relative roots are dropped. `validate_workspace_folder` canonicalizes
  (symlink-resolved **before** the allowlist check), requires an existing real
  directory, bounds length, refuses control chars, and never creates anything.
- **Bridge commands (`crates/bridge/src/chat.rs`):** `ListCodeSessions` /
  `CreateCodeSession` / `OpenCodeSession` / `SendCodeTurn` / `CodeDashboard`.
  Cowork's list stays chat-only (a Code row can never appear there). **Every
  send revalidates**: code enabled, local enabled, folder re-canonicalized and
  compared to the stored spelling (`CODE_FOLDER_CHANGED` — a renamed target or
  swapped symlink cannot silently redirect), stored permission mode parsed
  fail-closed (unknown spelling blocks the send), Bypass re-checked against
  current policy. The workspace folder travels as a quoted data block in the
  turn input, explicitly "path as data, not an instruction".
- **Permission modes (`crates/agent/src/permission.rs`):** stored spellings
  `default`/`acceptEdits`/`plan`/`bypass`. Enforced in `run_turn_with_tools`,
  not the renderer: **Plan** advertises zero tools and refuses a hallucinated
  call before policy/executor with a model-visible tool error; **Bypass** is a
  validation error unless `bypass_allowed`; a policy `Deny` still beats every
  mode; **AcceptEdits** auto-approves only host-declared edit-tool names — the
  production set is empty (no built-in file tools yet), so the mode never
  weakens an MCP approval in this slice.
- **Usage (`crates/sessions/src/usage.rs`, `crates/store/src/repos.rs`):**
  `summarize_code_usage` folds Code-only ledger rows + user/assistant turns
  (SQL join on `sessions.surface = 'code'`; Cowork rows cannot leak in).
  Ranges `7d`/`30d`/`all`; all-time's **daily strip** is capped at 3660 days
  with an honest `truncated` flag while totals/messages/sessions/shares stay
  genuinely all-time. Model shares, peak hour, favorite model derived only
  from real rows.
- **Renderer:** `/epitaxy` = `CodeDashboardScreen` (distinct greeting, range
  picker, recent Code sessions, per-folder repository rows, choose-folder);
  `/epitaxy/:sessionId` = `CodeSessionScreen` reusing the transcript/streaming/
  tool/approval surface with Code composer metadata (folder, model, mode).
  Bypass and Fast render visibly disabled with no capability claim. Browser
  preview reports `BRIDGE_UNAVAILABLE`; nothing is faked. Folder picking is
  the Tauri command `pick_workspace_folder` (`rfd`, workspace dep with
  `xdg-portal` + `tokio` features); cancellation is `None`.
- **Gates:** workspace **467 passed / 0 failed**; clippy clean on the four
  touched crates; `cargo metadata`; `tsc --noEmit`; `vite build`; E2E
  `shell.spec.mjs` ALL PASS including new `T28b` (honest empty/fail-closed
  dashboard), `T28c` (populated dashboard renders only supplied bridge data),
  `T28d` (Code session reuses transcript with Code metadata).
- **Deferred with reasons in `docs/14` §9 (items 53–57):** terminal/PTY,
  SSH/WSL/remote placement, worktrees, Fast mode. `TEST-CODE-003` is Partial
  (worktree lifecycle), `005` Policy-disabled/deferred, `006`–`008` and
  `TEST-SESSION-010` Unavailable/deferred. The native folder picker and a real
  Tauri window are compile/E2E-mock verified only, not exercised end-to-end.
- **Traps carried forward:** `docs/**` stays uncommitted (code-only commits);
  `apps/desktop/src/components/Untitled-1` remains untracked and untouched
  (owner decision pending); E2E needs the Vite dev server on 5173 **and** a
  served-title check (trap #13); use `apps/desktop/node_modules/.bin/tsc`.

## 7l. Phase 6 slices 2–5 — Projects, Artifacts, Scheduled, Search (2026-10-26)

The `phase-6-code` lane completes `docs/13` Phase 6's build areas on top of
slice 1 (§7k): Projects (`35958b8`), Artifacts (`8aa7987`), Scheduled
(`48eb6ac`), Search (`424eabc`). **Merged to `dev` and pushed 2026-10-26**
with the owner's approval (`f3fdc70`); a follow-up fix made the run-row
ordering and `runScheduledTaskNow` response deterministic (`3229372` —
the `running`/terminal rows of one occurrence share `started_at`, so a
random-uuid tie-break made the "latest run row" a coin flip; ties now
order by `rowid` and the response reads the exact terminal row). Gates on
merged `dev`: workspace 485 cargo tests / 0 failed, clippy clean on the
touched crates, `cargo metadata`, `tsc --noEmit`, `vite build`, E2E
`shell.spec.mjs` 47/47 (T01–T47), `setup-window.spec.mjs` 16/16.

- **Projects** — schema v3 (`projects`, soft `sessions.project_id` — no
  FK, deleting a project never deletes or retargets sessions and the
  deletion answer reports the orphan count). `crates/projects::ProjectService`
  borrows `&Store` per call. The bridge reuses the ONE folder validator
  (`policy::validate_workspace_folder`) for project folders; `CreateSession`
  gained `projectId`/`cwd` binding and folder-bound Cowork turns carry the
  Code-style quoted path-as-data system block. Folder "availability" =
  `canonicalize(stored) == stored`. Project instructions are stored/displayed
  but NOT injected into turns (no captured bootstrap assembly; item 61).
- **Artifacts** — `CreateArtifactTask` = one normal Cowork session with
  `extra.artifactTask: true`; no auto-send (the shipped generated prompt was
  never captured — recorded deviation). The empty-state suggestion sends its
  own visible copy as the first turn. No content model, no viewer — stated
  Unavailable (artifactsPane gate was never available).
- **Scheduled** — `crates/scheduler` is pure math (cron subset: `*`, lists,
  ranges `a-b`, `*/step` on minute/hour; dom/mon must be `*`; DST gap skips
  the minute, fall-back takes the earlier occurrence; `decide()` grace 60 s).
  Schema v4 (`scheduled_tasks` + `scheduled_runs`; every occurrence gets a
  run row; skips never stamp `last_run_at`). Bridge arms on create/enable/
  update, disarms on disable, `scheduled_tick(now)` runs due occurrences on
  the interactive send path with `scheduled=true` (the scheduled approval
  lifetime), records skips with the truthful reason, and rearms strictly
  forward; the shell spawns a 30 s tick loop with arm-on-boot. Keep-awake
  ships disabled.
- **Search** — palette combines `listSessions` + `listCodeSessions` +
  `listProjects`; case-insensitive title/name match; recent set on empty
  query; a labelled "Go to" route tail (full fallback in the browser
  preview). Sidebar filter menu composes AND with immediate apply, badge,
  reset, and a distinct filtered-empty state; Unread is disabled (no unread
  tracking); pinning is not built; filters persist in
  `localStorage["tasks.filters"]`.

Traps carried forward from this batch (all hit for real):

- **`i64::saturating_sub` is not `max(0, a-b)`** — `1000.saturating_sub(2000)`
  is `-1000`, and a "how late is it" check built on it misclassifies.
- **chrono 0.4 `TimeZone` for a hand-rolled zone needs `from_offset`,
  `offset_from_local_date`, and `offset_from_utc_date` too, and
  `offset_from_utc_datetime` takes `&NaiveDateTime`** (not `DateTime<Utc>`).
  `Local.timestamp_millis_opt` no longer exists — use
  `DateTime::from_timestamp_millis(ms).with_timezone(&Local)`.
  `impl Offset for FixedOffset` is an orphan violation (it already
  implements it).
- **Associated fns on generic types defeat `map_err` inference** (E0283):
  error mappers like `project_failure` are free functions, not
  `ChatService::<K, R>::` methods.
- **`#[serde(flatten)]` does not inherit the outer `rename_all`** —
  `ProjectRow`/`ScheduledTaskRow` spell their own fields (`memory_enabled`
  snake_case next to `createdAt` camelCase, like `SessionRow`'s stored-row
  convention; documented in `client.ts`).
- **E2E: `.switch input` has `pointer-events: none`** — click the `.switch`
  label. **`hasText` matches row VALUES too** — the filter menu rows carry
  `data-filter-row` for exact targeting; applying an option closes the menu.
  `waitForSelector("#composer")` before Ctrl+K, or the keypress races
  hydration.
- Store row views returned to the wire must include the new `project_id`
  everywhere `NewSession` is constructed — the compiler catches struct
  literals, but the E2E mocks had to add `projectId` to their fixtures.

## 7m. Phase 7 complete on the lane — memory, usage, privacy, updates, retention (2026-10-26)

The `phase-7` lane (cut from `dev` @ `178cb09`) implements all six build
areas of `docs/13` Phase 7 ("Memory, usage, privacy, updates") in five
commits: `4d18d87` memory · `7aa5dfe` usage split view · `0f76676`
privacy/diagnostics · `6b3fe47` retention + tombstone GC · `aa1337e` update
lifecycle. **Nothing is merged to `dev` or pushed — owner approval required,
per the standing pattern.** The Phase 7 exit criteria hold: legal hold
prevents deletion (boot-enforced), the diagnostic bundle enumerates and
redacts, and an update failure leaves the old version runnable (the
verification gate refuses every bundle).

- **Memory (`crates/memory`, `4d18d87`):** Markdown entries are the truth;
  `index.json` is a derived cache that heals on open; a corrupt (non-UTF-8)
  file is renamed aside (`.corrupt-<ms>`) and named in `events.jsonl`, never
  served or deleted; content bounds are refused, not truncated; the sha256
  baseline hash refreshes only on app-managed mutations so external edits stay
  detectable via `baseline_changed`; reset is explicit and audited. The bridge
  reads `memoryEnabled` fail-closed from the merged profile **per command**
  (absent = on, the captured default; explicit false/malformed = off); a broken
  memory directory degrades memory to `UNSUPPORTED`, never chat.
- **The usage split view:** store `list_surface_usage_between(surface, from,
  to)` joins `sessions` for the surface scoping (the ledger itself still has
  no surface column; the FK is `ON DELETE CASCADE`, so the shares always sum
  to the combined totals for app-written rows). Sessions'
  `usage_summary_split` returns combined + per-surface `SurfaceUsage
  { surface, totals }` — **totals only**: an earlier draft's `messages` field
  actually carried `totals.events` (a labeling fudge) and `sessions` was
  unused by any consumer; both were cut. Bridge `ChatResponse::Usage` carries
  `surfaces: Vec<SurfaceUsage>` alongside `summary` (a renderer that only
  reads `summary` is unaffected). The Settings usage panel renders Cowork/Code
  cards between "Tokens" and "Daily"; the old "no surface column" caveat is
  gone.
- **Traps hit in this slice:** the other model's edit had spliced the new
  types into the middle of `summarize_usage`'s doc comment — the doc text
  silently attached to the wrong item; check comment seams when inserting
  above a `pub fn`. The `Usage` response gained a field, so every
  `let ChatResponse::Usage { summary } = ...` destructure had to widen
  (the compiler catches struct literals, **not** pattern destructures in
  tests — grep for them). `surface` has no CHECK constraint on spellings
  (only `NOT NULL`); the split covers `chat`/`code` and combined folds
  everything — recorded as `docs/14` §9 item 68.
- **Diagnostics (`0f76676`):** `crates/telemetry` ships **no emitter** — no
  crash reporter, no analytics, no OTel exporter; the policy reads the
  merged `telemetry` section fail-closed (each block switch independent;
  malformed → blocked; the `disableNonessentialServices` list fails closed
  to the three known services; `otlpContentCapture` is refused at apply and
  surfaced anyway). Redaction is additive (Bearer/Basic schemes, token
  prefixes `sk-ant-/sk-/ghp_/github_pat_/xoxb-/xoxa-/AKIA/eyJ`, URL
  userinfo/query/fragment) — prompt-like text passes through unchanged. The
  bundle needs explicit consent (`CONSENT_REQUIRED`), passes the export
  redaction then the free-text pass, and logs `bundle-built` to
  `diagnostics/log.jsonl` with no contents (2 MiB bound). No upload exists
  and the Privacy pane says so.
- **Retention/GC (`6b3fe47`):** schema v5 `deleted_sessions` tombstones —
  the delete records its blob refs in the delete transaction;
  `sweep_tombstones` finishes crash-interrupted sweeps (reference-checked,
  so a shared blob survives until its last referent dies), prunes markers,
  idempotent across reopens. `RetentionSettings`: absent/malformed days =
  **no automatic deletion** (a guessed default for a destructive sweep is
  the worst invention); malformed hold = hold; `coworkSessionRetentionDays`
  governs the single chat surface and the shipped `chatSessionRetentionDays`
  is NOT silently mapped. Boot runs retention + tombstone sweeps; no timer.
- **Updates (`aa1337e`):** `crates/updates` — `UpdatePolicy` (72 h/24 h
  defaults, `0` = immediate, malformed keeps the default, never shortens),
  the lifecycle state machine (staged-only install; failed health check →
  `RolledBack`; a refused verification returns to `Running`), the
  enforcement window, and the streaming-task session guard. The production
  verifier `NoTrustAnchor` refuses EVERY bundle — the feed's wire format and
  public key were never captured, so the old version stays runnable by
  construction. The System pane states the no-feed posture; E2E T49.
- **Gates at phase end:** workspace 527 cargo tests / 0 failed; clippy `-D
  warnings` on store/sessions/bridge/telemetry/updates/desktop;
  `cargo metadata`; `tsc --noEmit`; `vite build`; E2E `shell.spec.mjs` ALL
  PASS (T01–T49; T48 privacy, T49 system/update posture).
- **Pre-Phase-8 verification sweep (2026-10-26, merged `dev` @ `db0116e`):**
  `cargo metadata`; workspace **527 cargo tests / 0 failed**; clippy
  `--workspace --all-targets -D warnings` clean; `tsc --noEmit`; `vite build`;
  E2E `shell.spec.mjs` ALL PASS (T01–T49), `setup-window.spec.mjs` 16/16;
  native `shell-runtime.sh` **19/19 after a harness fix** — the script's
  hardcoded `LOG=/tmp/opencode/...` pointed into a directory that no longer
  existed, so every primary launch's redirect failed silently and the cases
  cascaded into timeouts with a stray app instance holding the single-instance
  lock; the script now `mkdir -p`s its log directory. Toolchain note: a
  background `tail` pipe buffers the fixture's PASS lines until it exits —
  do not read emptiness mid-run as failure.
- **Docs on disk:** `docs/04` §3.1 rows Phase 7/1–5; `docs/17` §4.17 (the
  full status table); `docs/18` diagnostics contract + the retention note +
  the memory table + usage `surfaces`; `docs/14` §9 items 66–70.
- **Docs:** `docs/17` §4.18, `docs/14` §9 items 71–72, `docs/04` Phase 8 row.
  `apps/desktop/src/components/Untitled-1` remains untracked and untouched
  (owner decision pending). Do not run `cargo fmt --all` (it churns unrelated baseline files).

## 7n. Phase 8 slice 1 — the Responses extension (2026-10-26)

The `phase-8` lane (cut from `dev` @ `d1c4498`) implements `docs/13` Phase 8's
first reference extension: `crates/providers/openai-responses`
(`myharness-provider-openai-responses`), the native wall for the OpenAI
**Responses** wire — `ProtocolFamily::OpenAiResponses` in the sdk; the
workspace is 27 members. **Committed on the lane; not merged to `dev` and not
pushed — owner approval required.** 17-fixture wire battery green (stable
across repeated runs, parallel and serial), clippy `-D warnings` clean on the
crate + sdk, workspace **544 tests / 0 failed** (527 + 17), `cargo metadata`
green. The renderer was untouched, so no tsc/vite/E2E gates are owed. The
setup family, bridge probe lane, and renderer unlock are the next slice — no
UI path can select the family yet.

The wire readings are all rebuild-defined (`docs/14` §9 item 71 — the wire
was never observed): first-party OpenAI only (static key required, no
`custom` variant), `store: false` explicit, system blocks join into
`instructions`, documents native as `input_file`, media inside tool results
refused, replayed assistant reasoning omitted, prefix model echo, exact
usage (`reasoning_tokens` dropped — a subset with no app channel),
`completed`→`end_turn` (or `tool_use` from emitted function-call items),
**no `[DONE]` sentinel** — `response.completed`/`response.failed`/`error`
terminate, an undocumented event type errors, a stream ending without a
terminal event is malformed — annotations surface as citations, count_tokens
refused.

Traps hit for real in this slice (recorded as `docs/14` §9 item 72, all
fixed in `tests/wire.rs`):

- The mock listener must accept **non-blocking** with the stop flag
  re-checked between connections — a blocking `incoming()` hangs `Drop`'s
  `join()` forever in the zero-request cases (the flag is only checked
  after the next connection arrives).
- A chunked SSE response needs the terminating `0\r\n\r\n` chunk — without
  it hyper reads the close as a truncated body and errors the stream
  ("error decoding response body") instead of ending cleanly.
- A mock server must **read the request before answering** — a response
  head arriving before the request body finished sending is a hyper send
  error ("error sending request").
- Never wait for the client to close (keep-alive tail reads): reqwest
  returns the completed connection to its pool and the close is async work
  on the test's runtime, which is blocked in `join` — the wait stalls ~90 s
  until the pool's idle eviction. Force the accepted socket non-blocking
  explicitly rather than trusting the listener's flag to propagate.
- The Responses wire's announcement events carry the item id **inside the
  item object** (`item.id`); the top-level `item_id` belongs to the delta
  events. The error envelope classifies by `type` then `code` spellings —
  the stream's bare `error` event carries only a `code` (its `type` is the
  event name).

### 7n (slice 2). The Responses family's setup surface (2026-10-26)

Slice 2, same lane, committed after slice 1's merge: `ProviderKind::OpenAiResponses`
(serde `"openai-responses"`) with the family's OWN key spellings
`inferenceOpenAiResponsesApiKey` / `inferenceOpenAiResponsesBaseUrl`
(owner-defined rebuild keys — the Chat Completions slot never leaks across;
`changeFamily` resets family-specific keys). Form: static key required (the
wire has no keyless variant), base URL optional (empty = first-party).
Egress: override or `api.openai.com`. Bridge: `test_connection_openai_responses`
runs the STRICT resolver (`allow_helper: false`) — an absent credential is a
`CONFIG_VALIDATION` refusal at the BRIDGE with zero requests, NOT the
adapter's `PROVIDER_ERROR` (that class only appears on the keyless-optional
openai lane); the chat factory arm sets `provider_id` = the wall's own
`id()` spelling `openai-responses` — the agent loop refuses a model whose
provider id is not the adapter's, so this must match exactly. Discovery is
the additive command `setup_discover_openai_responses_models` (do not touch
the existing two); the wall's discovery outcome (`{models, warnings}`) maps
onto the shared setup shape (`pages: 1, partial: false, first warning`) in
`crates/bridge::helper::discover_responses_models`, which required a public
request-facing `discovery::discover_models` + `DiscoveryRequest` in the
responses crate (its `discover_with` is `pub(crate)`). Renderer: family
option "OpenAI (Responses)", probe unlock, discovery arm.

Gates at slice end: config 64, bridge 64+32+5, workspace **548 / 0 failed**
(544 + 4), clippy `-D warnings` on config/bridge/responses, `cargo metadata`,
`tsc --noEmit`, `vite build`, E2E `shell.spec.mjs` ALL PASS (T01–T49),
`setup-window.spec.mjs` 16/16. The chat-send path through the family is
compile/unit-verified only — no E2E drives a live Responses stream (no
Tauri host in the browser; same posture as every family).

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
  keychain implementation landed in the bridge slice (`KeyringAuditKeys`,
  §7g); `TEST-CFG-006` remains compile-verified only. The chain adds a `prev_hmac` link the captures do not
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

The next slice — profile→adapter factory + bridge commands + renderer chat surface, plus
usage aggregation/drafts/retention — is recorded in §7g.

## 7g. Phase 4 slices 3–4 + renderer (2026-10-24)

Slices 3 (`e1ad4bd`) and 4 (`955f47d`) plus the renderer slice complete Phase 4's chat path.
The full lane is `phase-4`; `dev` stays untouched until the owner says merge/push.

- **Crash recovery is marker-based, not replay-based.** `run_turn` writes an assistant
  placeholder (`partial`, no error) at send start; `finish_turn` may only transition a live
  `partial`/no-error row, validates session ownership, resolves the audit key **before** the
  row transition, and writes the content blob only after the row is known live. Recovery
  (`recover_interrupted`) turns leftovers into `error` with `"the stream was interrupted by
  an app restart"`; it runs at `run_turn` start and on session open, but **skips a turn this
  process is running** (opening a task mid-stream must not kill it). Cancelled turns are
  always finished as `partial` + `"the turn was cancelled"`, so a cancel never looks like a
  crash. A token cancelled before anything persisted returns `AgentError::CancelledBeforeSend`
  → `userTurnRecorded: false`; a post-persistence cancel is `TURN_CANCELLED` with `true`.
- **Archive/delete refuse while a turn runs** (`TURN_IN_PROGRESS`); otherwise the stream would
  finish against a deleted session. The renderer disables those menu items while streaming.
- **Audit-key ordering**: `start` rolls the session row back if key creation fails;
  `append_turn`/`begin_assistant_turn` resolve the key before inserting; `finish_turn` resolves
  it before transitioning. A missing key must leave a live turn recoverable, not finish it
  with no chain link.
- **Usage is local-day over raw ledger rows.** `summarize_usage(events, days, FixedOffset,
  now)` builds 1..=366 day buckets; cached tokens stay separate; the renderer passes
  `utcOffsetMinutes` (east-positive, clamped ±840), absent → process local offset. The offset
  is applied to the whole window, so a DST transition inside it is not re-bucketed (recorded
  unknown). The ledger has **no surface column**, so the panel combines Cowork+Code.
- **Retention is an engine only.** `sweep_retention` filters surface + `older_than_days`,
  honors a global hold and per-session legal hold, reports `deleted`/`held`/`kept`. No timer
  calls it and Settings has no control; the shipped schedule/UI was not observed.
- **The factory resolves only the applied profile.** `ProfileTargets` over `ConfigBackend`
  refuses `CONFIG_MISSING`/`CONFIG_VALIDATION`, refuses locked `mantle`/`foundry`
  (`ADAPTER_NOT_AVAILABLE`), reuses setup's `resolve_probe_credential` (static key or helper
  script) and base-URL/header rules, and runs no discovery at send. A session stores
  provider/profile/model; a send whose applied target no longer matches refuses
  `MODEL_UNAVAILABLE` ("No model was substituted") instead of switching. The profile document
  has no revision field, so a same-provider/model profile swap is not detected — recorded gap
  (`docs/14` §9 item 39).
- **No system prompt is sent.** The shipped bootstrap prompt/context assembly was never
  captured; `TurnInput::text` carries an empty system. This is an explicit deviation, not a
  claim (`docs/14` §9 item 33). No retries, picker, attachments, tools, or per-model token
  capability validation either.
- **Bridge + renderer surface.** `chat_command` (create/list/open/send/stop/archive/rename/
  delete/usage) and `chat://turn-event` (content deltas only; final response carries usage/
  stop/error). App audit keys are keychain `session-audit/<uuid>`; tests use in-memory.
  Renderer: `/cowork/:sessionId`, New→session handoff with the draft moved before the first
  await, Active list replacing "Ideas for you" when sessions exist, sidebar task rows,
  transcript with Stop/verbatim provider error/`Stopped`/reasoning `<details>`/copy,
  rename/archive/delete inline, 7/30/90 usage panel. The session's model label is
  provider-qualified; the New screen's is neutral ("model") because the applied profile is
  unknown until a session exists.
- **Draft rule (spec wins over the earlier plan):** drafts are retained on **any** failure,
  including a provider failure after the user turn is recorded (`docs/features/
  chat-and-composer.md` §Errors), and cleared only when a turn completes. The New draft is
  moved into the session draft before the first await. Drafts are plaintext `localStorage`,
  not encrypted — a recorded limitation.
- **Tests/gates**: store 24, sessions 15, agent 15, bridge 40 setup + 10 chat; desktop
  `cargo check`; clippy `-D warnings` on the four crates; `cargo metadata`; renderer
  `tsc --noEmit` + `vite build`; E2E **35/35** (T27/T28 updated, T31–T33 added, old T31/T32
  renumbered T34/T35). The E2E browser has **no Tauri host**: it proves the desktop-only
  error, draft persistence, and no faux rows/totals — not a live stream. No native-runtime UI
  stream test yet (`docs/14` §9 item 40).
- **Traps**: `docs/**` stays uncommitted; never touch
  `apps/desktop/src/components/Untitled-1`; use `apps/desktop/node_modules/.bin/tsc`, not
  root `npx tsc`; `Usage` serializes camelCase (`cacheRead`), `ContentPart` keeps snake_case
  fields, `SessionRow` keeps `legal_hold`/`permission_mode`, `TurnSummary` is camelCase.

## 7h. Phase 5 slice 1 — MCP lifecycle foundation (2026-10-24)

`crates/mcp` is no longer a scaffold. It owns the transport-independent MCP
lifecycle (`docs/13` Phase 5; `docs/12` §5.1; `TEST-MCP-001/002/009/010`):

- **SDK:** `rmcp` 3.7 with `client`, `transport-child-process`,
  `transport-streamable-http-client-reqwest`; `default-features = false` so the
  production crate does not compile the server/macro surface. The test-only
  `fixture-server` feature adds `server`, `macros`, `transport-io`, `schemars`
  and builds `mcp-echo-server` from `tests/fixtures/echo_server.rs`; a normal
  `cargo test -p myharness-mcp` does not build it.
- **Transports:** stdio (`TokioChildProcess`) and Streamable HTTP
  (`StreamableHttpClientTransport`) are real. Legacy HTTP+SSE is *modelled* in
  `ServerConfig::Sse` but returns `UnsupportedTransport` — rmcp 3.7 carries SSE
  inside Streamable HTTP, and the legacy client was not available. This is an
  explicit limitation, not a silent fallback.
- **Manager:** `McpManager` over an injectable `McpConnector`/`McpSession`.
  `start_all` records per-server failures and continues; an optional
  `with_start_timeout` bounds connect/list-tools so a hanging server cannot
  block the rest. A dead transport (`McpSession::is_closed`) is reconnected on
  the next `start`, and the tool cache is rebuilt. `shutdown` collects
  per-server failures and is idempotent.
- **Names and snapshots:** server names are `[A-Za-z0-9._-]`, at most 64 bytes,
  and may not contain `:`; tool names may contain `:` and are split on the
  first colon. The registry key is `<server>:<tool>`. A session snapshots the
  enabled subset; `missing` names stay visible; duplicate requested names are
  collapsed. Empty `enabled` means an empty snapshot, not "all".
- **Calls:** `McpManager::call_tool(snapshot, name, arguments, timeout)` refuses
  a tool absent from the frozen snapshot *before* touching the session. The
  caller supplies the timeout because the shipped `mcpToolTimeoutSec` default
  was not captured. Text, images, audio, embedded text/blob resources, resource
  links, and structured content are preserved; an unknown content block is a
  visible `UnsupportedContent` error, never a silent drop.
- **Trust:** `ServerConfig`'s `Debug` prints env/header **keys only**, redacts
  URL userinfo and query, and `redact()` removes env/header values, URL
  passwords, and query strings from stored errors. Schemas are bounded at
  4,096 tools/server and 256 KiB/schema — rebuild-defined resource bounds
  (`docs/14` §9), not observed shipped caps.
- **Tests/gates:** 2 unit + 15 manager + 2 real-stdio = 19 passing with
  `--features fixture-server`; `cargo clippy -p myharness-mcp --all-targets
  --features fixture-server -- -D warnings`; `cargo check --workspace`;
  `cargo metadata`. The stdio test spawns the fixture, lists `echo:echo`, calls
  it, and shuts the child down; a broken command does not stop the healthy
  server.
- **Traps:** in rmcp 3.7 `RunningService::peer_info()` exposes
  `server_info: Option<Implementation>` (not a bare `Implementation`);
  `StreamableHttpClientTransportConfig` is `#[non_exhaustive]`, so use
  `with_uri(...)` and assign `custom_headers`; `ContentBlock` and
  `ResourceContents` are `#[non_exhaustive]`, so a wildcard arm is mandatory.
  Do not add `reqwest`/`tracing` directly to `crates/mcp` unless used; the
  transport's reqwest type is inferred by `from_config`.
- **Not done in this slice:** OAuth/token storage; tool policy/approvals
  (allow/ask/deny, argument scopes, persistent approvals); egress policy; tool
  search/on-demand schemas; skills; plugins; bridge commands; renderer/UI;
  app-boot wiring; durable schema cache; tool-call audit-chain integration;
  health probing beyond `is_closed`; legacy SSE. `TEST-AI-019/020` are not
  satisfied by this crate alone.

## 7i. Phase 5 slice 2 — policy, approvals, egress, execution gate (2026-10-24)

The policy layer is now real, still **not wired into `run_turn`**. Tools are
refused by `TurnInput::validate` until the loop slice exists.

- **Policy (`crates/agent/src/tool_policy.rs`):** allow/ask/deny with
  argument-scoped rules. The rule serialization, fail-closed precedence
  (`deny > ask > allow > default`), subset argument match, default `Ask`, and
  SHA-256 canonical argument fingerprint are **rebuild-defined** because the
  shipped shapes were not captured (`docs/14` §9). Deny wins regardless of
  rule order; an approval can turn `Ask` into `Allow` but can never override
  `Deny`.
- **Scopes:** `Once` is never stored; `Session` and `Persistent` are stored.
  `Persistent` is refused when the context's `persistent_allowed` is false
  (resolved from `mcpPersistentAlwaysAllowEnabled`). A scheduled task with
  `scheduled_lifetime_days == 0` offers only `Once`; otherwise session and
  persistent grants expire at `now + days*86_400`. `ApprovalStore` is
  **in-memory** in this slice; durable persistence and the session-metadata
  audit write are later.
- **Visibility:** `ToolPolicy::hides_tool` / `visible_tool_names` remove only
  **unconditional** denies. An argument-scoped deny leaves the tool declared
  (otherwise the allowed argument sets would be unreachable) and is enforced
  at execution. This is the `TEST-AI-019` provider-side filter primitive, but
  the provider request is not wired yet.
- **Execution gate (`crates/agent/src/tool_exec.rs`):**
  `execute_tool_call(policy, store, context, call, executor, now)` returns
  `Completed`, `Denied`, or `NeedsApproval { allowed_scopes }`. The executor
  is policy-blind; a denied call never reaches it. An executor error becomes a
  model-visible `ToolOutput { is_error: true }`, never an empty success.
  `ToolOutput` uses the provider SDK's `ContentPart`, so text/images/documents
  survive without a `myharness-mcp` dependency in the agent crate.
- **Egress (`crates/mcp/src/egress.rs`):** fail-closed **when configured**;
  exact case-insensitive host match; only `http`/`https`; empty allowlist
  denies all; `allow_all` is explicit. `McpManager::with_egress` checks before
  `connect`, so a denied HTTP/SSE host is recorded as a failure with **zero**
  connector calls (`TEST-AI-020` at the manager boundary). **stdio is not
  inspected** — a child can open its own sockets; that is an OS/proxy
  boundary, not this layer. A denied host is not echoed in `Display`.
- **Tests/gates:** agent 13 policy + 8 execution + 15 turn = 36;
  mcp 8 unit + 18 manager + 2 stdio = 28; clippy `-D warnings` on both crates
  with the fixture feature for mcp; `cargo check --workspace`.
- **Traps:** `ToolPattern`/`ArgumentMatch` serialize as
  `{"kind":"exact","value":...}` / `{"kind":"server","value":...}`; the
  approval fingerprint is over **exact arguments**, so an approval for
  `{"repo":"prod"}` does not cover `{"repo":"dev"}`; `ApprovalStore` never
  sees policy, so a grant for a denied tool is possible but cannot unlock it
  at the gate; `ToolOutput` has no audio variant (provider SDK gap) — an MCP
  audio result must become an explicit error until the SDK carries it.
- **Not done:** wiring the gate into `run_turn` (multi-round tool loop and
  tool-row replay), renderer approval prompt/events, durable approvals and
  audit-chain records, tool-search/on-demand schemas, OAuth, skills, plugins,
  bridge commands, app-boot MCP initialization. `TEST-AI-019` is only a
  primitive until the provider request uses `visible_tool_names`.

Next: Phase 5 slice 3 (agent tool loop + MCP executor adapter + bridge
approval events) or skills/plugin discovery. The deferred retention timer/UI,
retry, picker, attachments, and per-surface usage remain unknown/deferred, not
invented.

## 7j. Phase 5 slice 3 — tool turns, approvals, catalog, app integration (2026-10-25)

Slice 3 (`03d5102`, hardening `12d508a`, `b8391ae`) completes Phase 5's local/stdio path. The lane is
`phase-5`; **nothing has been merged or pushed** — owner approval is required.

- **Agent tool loop (`crates/agent`):** `run_turn_with_tools` is a bounded
  multi-round executor (`DEFAULT_MAX_TOOL_ROUNDS = 16`). It replays `tool` rows
  as provider tool-result messages (`Role::User` + `ContentPart::ToolResult`),
  filters unconditional denies before the request, validates arguments against
  the tool schema, maps internal `<server>:<tool>` names to provider-safe
  `mcp_<sanitized-prefix>_<16-hex-sha256>` names and back, persists each tool
  result, and emits `TurnEvent::ToolResult`. A policy-denied call becomes a
  model-visible tool error so the transcript stays grammatically valid; this
  denial-result gate is **rebuild-defined** because the shipped behavior was
  not observed. Output is sanitized to text/images/documents, capped at
  `MAX_TOOL_OUTPUT_BYTES = 1 MiB`; audio is an explicit error (no provider SDK
  audio part). A tool-enabled send calls `list_models`, then
  `adapter.capabilities`; absent `tools_supported` fails `MODEL_UNSUPPORTED`
  and no tools are sent.
- **Policy/approvals (`crates/agent/src/tool_policy.rs`):** `Once` is never
  stored; `Session`/`Persistent` are durable. `Persistent` needs
  `context.persistent_allowed` (`mcpPersistentAlwaysAllowEnabled`); a scheduled
  task with lifetime `0` offers only `Once` and never inherits an interactive
  grant with no expiry. `ApprovalStore::restore` skips expired/`Once` records.
  Fingerprints are canonical SHA-256 over exact arguments.
- **Bridge (`crates/bridge/src/mcp.rs`):** `McpConfig::load(root)` reads
  `root/servers.json` (bounded at 2 MiB); `McpConfig::build` returns
  `BuiltMcp { manager, config, warnings }`. `ToolRuntime` holds the manager
  behind `Arc<tokio::sync::Mutex<_>>`, the policy, approvals, prompt, frozen
  snapshots, OAuth store, and warnings. `set_session_tools` validates live
  names, persists `extra.mcpTools`, then installs the snapshot; a missing name
  fails before storage/send; a running turn refuses changes
  (`TURN_IN_PROGRESS`). `snapshot_for` freezes on first send. `clear_session`
  drops the snapshot and session grants but keeps persistent grants.
  Approvals persist to `mcp/approvals.json` (0600, bounded 2 MiB); new grants
  are chained as `mcp.tool.approved` with tool name + fingerprint only.
- **OAuth/schema/search (`crates/mcp`):** per-server token store; production
  keychain account `mcp-oauth/<server>`; refresh is rebuild-defined OAuth 2.0
  (`grant_type=refresh_token`, form-encoded) behind an injectable `OAuthHttp`;
  the production refresh client applies the egress allowlist to the token URL
  and disables redirects; an explicit configured `Authorization` header wins
  over OAuth. No authorization-code/PKCE flow — a server needing it surfaces
  `NeedsAuth`.
  Schema cache is `mcp/schemas.json` (atomic, 0600); cached tools are
  metadata-only. Tool search is deterministic; `toolSearchEnabled` defaults
  `false`; `McpTools` bypasses the flag for the session picker but filters
  disabled and policy-hidden tools.
- **Skills (`crates/skills`):** directory + `manifest.json`; precedence
  `Org > User > Plugin > Bundled`; bundled placeholders are non-runnable;
  scripts never execute; assets are declared-only, path-confined, capped;
  symlinked directories rejected; truncation is UTF-8-safe.
- **Plugins (`crates/plugins`):** local `plugin.json` install into a managed
  root; org bundles read-only; no network marketplace. Signature verification
  is **not implemented**: `Required` refuses every install; `Optional` accepts
  unsigned/unverified and reports `Unverified`. Plugin MCP servers are
  namespaced `<plugin>.<server>` and use normal MCP policy/approval; a server
  name collision is skipped with a warning; disabled plugins contribute
  nothing. Enablement changes apply on next app start, never to a frozen
  session snapshot.
- **Catalog (`crates/bridge/src/catalog.rs`):** root `app_data/catalog`;
  `skills.json`/`plugins.json` plus `skills/`, `plugins/`, `org/skills/`,
  `org/plugins/`. Lock ordering matters: catalog settings first, then MCP
  config; never hold both across an await.
- **App boot / renderer:** `apps/desktop/src-tauri/src/lib.rs` builds the
  catalog, appends plugin servers, builds MCP, starts `start_all` in the
  background, emits `chat://mcp-status` and `chat://tool-approval`, and wires
  `ToolRuntime`/`CatalogRuntime`. A broken MCP directory degrades MCP, never
  chat. Renderer: approval modal, live tool status, session tool picker, skill
  picker, `SkillsPanel`/`PluginsPanel`/`DeveloperPanel`. `#settings/developer`
  is a rebuild-defined placeholder key; `skills`/`plugins` stand in for the
  uncaptured Customize route; `connectors` stays disabled. Browser preview
  reports `BRIDGE_UNAVAILABLE` and never fakes rows/totals.
- **Gates:** bridge 55 unit + 11 chat + 4 `mcp_tools`; MCP 25 unit + 22
  manager + 2 real-stdio (`--features fixture-server`); agent 57; skills 7;
  plugins 7; provider wire batteries unchanged; `cargo check -p
  myharness-desktop`; `tsc --noEmit`; `vite build`; browser E2E 38/38; clippy
  `-D warnings` on affected crates.
- **Traps:** `run_turn` holds one global approval mutex across a turn, so tool
  turns serialize across sessions; a pending approval can be lost if the
  renderer navigates away; no default MCP start/tool-call timeout
  (`None` unless configured); the pinned `rmcp` transport follows only
  same-origin 307/308 redirects and refuses cross-origin hops, and the
  production OAuth refresh client applies the allowlist and disables redirects;
  stdio is not inspected; DNS/IP egress is not checked; legacy SSE is explicitly
  unsupported; tool schema validation is a bounded subset (`pattern`,
  `format`, external `$ref` not enforced); no live HTTP MCP fixture or live
  OAuth/keychain exercise; no marketplace/signature verification; no
  persistent actionable failure toast; no model-driven on-demand tool search.
- **Docs:** `docs/17` §4.16, `docs/14` §9 items 44–52, `docs/18` §4.5, `docs/19`
  IMP-31..37, `docs/04` Phase 5 row 3. `docs/**` is modified on disk only and is
  **never committed**. `apps/desktop/src/components/Untitled-1` remains untracked and
  untouched. Do not run `cargo fmt --all` (it churns unrelated baseline files).

## 7o. The final sprint — local extension, extension framework, backup (2026-10-26/27)

The `feature-final-sprint` lane (cut from `dev` @ `706f2ec`) carries four
commits (`8965209`…`57de540`); merged to `dev` and pushed with the owner's
approval. Gates at merge: workspace **596 cargo tests / 0 failed**, clippy
`--workspace --all-targets -D warnings` clean, `cargo metadata`, tsc, vite
build, E2E `shell.spec.mjs` ALL PASS (T01–T49), `setup-window.spec.mjs`
16/16, and the release desktop binary builds under `--locked` (37.7 MB).

- **The local provider extension (`crates/providers/local`,
  `myharness-provider-local`)** — Phase 8's second reference extension: a
  native wall for Ollama/llama.cpp over their documented OpenAI-compatible
  surfaces (`POST {base}/v1/chat/completions`; discovery reads `/api/tags`
  first — Ollama's keyed listing — then `/v1/models` on a 404, the
  llama.cpp posture). Its core is local-network security: the destination
  gate pins the base URL to loopback at construction (loopback IP literals
  or `localhost` only; a name resolving off-loopback — DNS/IP rebinding —
  refuses before any bytes; the resolver is injected for tests), requests
  pin to the resolved address, the wall builds its own per-instance client
  with redirects disabled (a redirect errors, never follows — the one
  deliberate deviation from client injection, recorded), and keyless is the
  documented default (no `authorization` header; keyless never widens the
  gate). Tool fragments join by the wire's `tool_calls` index (the id may
  travel only on the first fragment); usage parses before the
  finish-reason close flushes the outbox; `data: {}` pings never re-arm
  the idle window; the `[DONE]` sentinel is required. 20 wire fixtures +
  4 destination-gate unit tests + 3 bridge lane tests (keyless probe,
  typed secret, non-loopback refusal). `ProviderKind::Local` (serde
  `"local"`), keys `inferenceLocalBaseUrl`/`inferenceLocalApiKey`, both
  OPTIONAL (keyless localhost is the default). Traps: the destination gate
  must be SYNCHRONOUS (std `ToSocketAddrs`, never a nested tokio runtime —
  construction happens inside sync and async contexts alike); accepted
  socket inheritance of O_NONBLOCK is not guaranteed — force it.
- **The extension-package machinery (`crates/extensions`)** — manifest
  schema with structural gates (name charset/bounds, version triples,
  shallow relative payload paths, digest shape, 256 KiB bound), SHA-256
  payload integrity, the `TrustAnchor` seam whose production anchor
  (`NoTrustAnchor`) refuses every signature fail-closed (no credential
  exists — the recorded blocker), a test-only keyed-digest anchor, and a
  loader that isolates broken packages (per-package errors; payloads
  verify lazily on first read; a missing root is an empty set). 13 tests.
  Trap: hand-rolled base64 decode must shift by the CHUNK's bit width
  (the 24-bit layout only holds for full 4-char groups).
- **The family catalog (`TEST-EXT-007`'s data half)** — `crates/config::
  forms::FamilyDescriptor` + `family_catalog()` served through the new
  `familyCatalog` setup command; the setup UI renders the picker, unlocks
  probe/discovery, and dispatches discovery on the catalog's STYLE + keys —
  no family ids are hard-coded in the UI. The boundary: the catalog
  DESCRIBES; the bridge CONSTRUCTS (adapter factories stay per-family).
  `FamilyDescriptor` carries owned `String`s (`&'static str` fields cannot
  Deserialize across the IPC).
- **Backup/restore (`crates/store::backup`)** — `Store::backup_into`
  snapshots via `VACUUM INTO` (consistent, self-contained, refuses an
  existing target); `restore_backup` is the offline pre-open path: FULL
  integrity check read-only, the schema version read from
  `schema_migrations` (NOT `user_version` — VACUUM INTO does not carry
  it), atomic staging+fsync+rename swap, the displaced database renamed
  aside. An in-app restore flow is deferred (the file cannot move under a
  live connection). 6 tests.
- **Performance budgets, measured** (`performance_budgets_over_a_local_
  workload`): create-500 32.5 ms, ledger-2000 79.8 ms, list-500 2.0 ms,
  backup 3.2 ms — ceilings pinned by the test.
- **Honest negatives recorded** (`docs/14` §9 items 73–74 and
  `docs/final-sprint-status.md`, on disk): NOT bit-for-bit reproducible
  (two independent release builds hash differently); production signing
  and the extension trust anchor are authorization-pending (the fail-closed
  gates hold by construction); gemini/minimax deliberately deferred;
  mantle/foundry stay locked; Ollama's native `/api/chat` grammar is an
  unobserved reading. The stale `docs/13` §2 stack table is fixed to the
  verified Tauri/Rust architecture (the shipped Electron stack is a
  labeled preservation note).

## 7p. Whole-app campaign phase 2 — the feature-surface sweep (2026-10-27)

Phase 2 (`docs/17` §4.21) is complete: WA-04/05 (setup credential flow) were
already fixed; this session swept every surface **hands-on in the release
binary** against a fresh install and found `WA-06` (P1, fixed) and `WA-07`
(P2, unreproduced ×2, monitored). Merged lanes: `feature-whole-app-test-phase-2`
(campaign), `fix-wa-06-setup-first-run` (the fix). Durable facts:

- **WA-06 was two bugs stacked.** (1) The setup boot's no-profile early return
  skipped `loadFamilyCatalog()`/`loadSchemas()` — empty picker, blank panes.
  (2) The setup namespace had **no first-profile creation path at all**
  (`library.write` refuses unknown ids; `duplicate` needs a source). The
  rebuild defines the **id-less-draft upsert**: `writeProfile` with no id
  creates the entry (refuses fail-closed without a provider family), stamps
  the generated id, answers with the stored document; the renderer's first-run
  draft is a blank local document (always dirty, no Discard), and Apply uses
  the returned id. Rebuild decision, `docs/14` §9 item 75 — the shipped
  first-run was never observed.
- **Playwright `addInitScript` drops functions from its argument.** The setup
  suite's `seed.tabSchema` never existed in the page; every tabSchema response
  errored to BRIDGE_UNAVAILABLE and every tab rendered schema-less in all
  cases, hidden because the with-profile flow overrides the connection schema
  from the object-typed `seed.connectionSchema`. Stub logic that is a function
  must live INSIDE the init script; the seed carries plain data only.
- **XTEST/PRIMARY text entry into the release WebKitGTK webview is lossy in a
  WM-less Xvfb session** (typed chars dropped non-deterministically at any
  pacing; PRIMARY paste did not land). Clicks and key-combos work. The
  click-through first-run creation + live send through the real shell therefore
  remain browser-E2E-verified + unit-verified only (`docs/14` §9 item 75); a
  future native pass needs a WM (or `gtk_entry_set_text` via AT-SPI — at-spi
  activated on the private bus this time, unlike phase 1's note).
- **The sweep harness** (reusable for phase 3, all in /tmp/wa-sweep, never
  committed): `dbus-run-session -- inside.sh` with `XDG_CONFIG_HOME`/`XDG_DATA_HOME`
  redirected (throwaway state), `gnome-keyring-daemon --start
  --components=secrets` for the Secret Service (plain `--unlock` does NOT
  enable the component), a durable FIFO command channel (reopen per iteration —
  a one-shot FIFO read tears the session down), Xvfb :99 + `import` for
  captures, python-xlib XTEST for input, `WM_DELETE_WINDOW` ClientMessage to
  close Tauri windows (no decorations without a WM).
- **WA-07:** two silent release-shell exits under Xvfb (no panic/core/stderr),
  not reproducible on retry — environment-suspected (GPU-less EGL), monitored;
  a recurrence on a real desktop reopens it as P1 (`docs/14` §9 item 76).
- Gates at phase end: **599/0** workspace (79→81 suites), clippy clean, tsc
  clean, vite ok, shell E2E 52/52, setup E2E **18/18** (S17/S18 new), a11y
  18/18, native 20/20. `Untitled-1` untouched; no screenshots committed.

## 7q. Whole-app campaign phases 3–4 — adversarial pass and the live wire (2026-10-28)

Phases 3 (`docs/17` §4.22) and 4 (§4.23) are complete on
`feature-whole-app-test-phase-3` with fix lanes `fix-wa-08-*` and
`fix-wa-09-*`. Durable facts and traps:

- **The hostile-response matrix is the send path's real regression harness.**
  The mode-driven loopback mock (mock-mode file read per request: normal /
  garbage / wrongmodel / http500 / abrupt / slow) proved through the release
  shell: verbatim parse errors, model-substitution refusal, verbatim provider
  errors, the `[DONE]` sentinel rule, and the idle window surviving a 6 s
  stall. Any future wire change should re-run this matrix.
- **The click-only send loop**: the New screen's idea chips prefill the
  composer; once a session exists the chips are replaced by "Active" — but
  deleting the session (title menu → Delete… → confirm popover) restores the
  chips. This drove five hostile turns with zero typing (XTEST text entry into
  the release webview remains unusable; clicks and combos are reliable).
- **Seeding a profile directly into the isolated library works**: camelCase
  `$XDG_CONFIG_HOME/Claude-3p/configLibrary/{<uuid>.json,_meta.json}` with
  `appliedId` + `entries`; the setup window and the send path accept it. The
  library validates on WRITE only — a future `schemaVersion` reads fine and is
  refused at write/apply (`CONFIG_SCHEMA`, tested in the crate).
- **WA-08 decision (`docs/14` §9 item 77)**: host redaction is Export-only;
  the renderer read keeps the user's own hosts (secret handles, credential
  shapes, and query strings still redacted).
- **WA-09**: a list error must NOT fall into the first-run path —
  `loadProfiles` returns null and the boot keeps the error banner (S19).
- **WA-07 count is three** silent Xvfb exits (Projects click; setup
  interaction; setup open), never on retry, zero diagnostics — still
  environment-suspected (GPU-less WebKitGTK/EGL under Xvfb, no WM), item 76.
- **One FIFO, ONE reader**: a leftover `inside.sh` loop from a previous
  session interleaves reads on the same FIFO and mangles commands ("oCATED:
  command not found"). Kill all `dbus-run-session` instances before starting
  a new one.
- Gates at phase end: **600/0** workspace, clippy clean, tsc clean, vite ok,
  shell E2E 52/52, setup E2E **19/19** (S19 new), a11y 18/18, native 20/20.
  The `docs/14` §9 item 40 gap (no native-runtime UI stream test) is retired.

## 7r. Whole-app campaign phase 5 — the user-journey suite (2026-10-28)

`tests/e2e/user-journey.spec.mjs` (U01–U12) is the standing journey regression:
each case drives a multi-step flow against a **stateful fake bridge** (its
sessions/turns/projects/tasks mutate per command — later steps see earlier
steps' effects). Facts worth keeping:

- **The projects CREATE flow lands in the project DETAIL**, not back on the
  list (createProject → `{type:"project"}` → the screen opens the detail).
  Journeys and future tests must expect the detail.
- **The folder chip on the Code dashboard shows the basename** ("journey-code"),
  never the absolute path — assert on the basename.
- **The session-title menu race**: clicking `button.session-title` right after
  a send can be eaten by the post-send `load()` re-render — the journey suite
  retries once if `.session-menu` does not appear. Any new menu-driven test
  needs the same tolerance (the helper is `openSessionMenu` in the spec).
- Selector map the hard way: session menu items are `role=menuitem`
  (Tools…/Rename/Archive/Delete…), rename input is `[aria-label="Task title"]`
  + Save, delete confirm lives INSIDE the menu (`.session-confirm`), scheduled
  route is `/scheduled-task` (templates `.sched-template`, rows `.sched-row`,
  switch `.sched-row .switch`, delete confirm `.confirm-inline`), code
  dashboard rows `.code-repo-row`/`.code-session-row`, back is the toolbar
  "Back" icon-btn, session drafts restore from localStorage via the sidebar
  task row.
- Phase 5 found **no product defect** (all first-pass failures were the
  suite's assumptions). 12/12 stable ×3; other gates unchanged (600/0, shell
  52/52, setup 19/19, a11y 18/18, native 20/20). The four campaign phases are
  complete; follow-ups are owner-gated (WA-07 under a real WM, installer).

## 7s. Whole-app campaign phase 6 — interface parity (2026-10-28)

Screen-by-screen comparison against the original (`docs/17` §4.25). Durable facts:

- **The original's authenticated UI cannot be rendered here.** The installed
  app (`/usr/lib/claude-desktop-unofficial`) under Xvfb with a throwaway HOME
  + private dbus never maps a window; the shipped renderer
  (`resources/ion-dist`) served statically with `/edge-api/*` stubbed to `{}`
  boots to its REAL offline screen ("Can't reach Claude") and 404 page —
  usable anchors (serif display headings, starburst wordmark, pill buttons).
  Real-pixel diffing of authenticated screens needs the private archive
  captures (owner-gated).
- **ion-dist boot facts**: entry is `index.html` (`client-shell-v1.html` is a
  handshake shell waiting for a `boot` postMessage); it fetches
  `edge-api/bootstrap?statsig_hashing_algorithm=djb2...` before anything; a
  SPA-fallback server + API stub renders the shell. The 82 extracted CSS
  variables live in `evidence/ui-tokens.json` (`pick` holds the sidebar/main/
  composer metrics: 321 px / 13 px / 19.5 px — all already matched).
- **The parity fixes (IP-01..07)**: serif `.screen-head h1` (16 px/400
  display serif — the specs say every list title is serif), 30d/7d range
  labels, Projects/Artifacts header tool icons (sort and view-toggle stay
  disabled-honest: never observed), the account menu's identity header +
  separator + changelog `›`, the General pane's real controls (chat font
  Sans/Serif → `html[data-chat-font]` on the transcript; Motion System/
  Reduced → `html[data-motion]` kills animations; the notifications switch
  fires a real Notification on turn completion — prefs in
  `settings/prefs.ts`, keys `ui.chatFont`/`ui.motion`/`ui.notifyOnDone`,
  rebuild-defined), and the task-detail right pane (340 px, Progress/
  Working dir/Context, real bridge data only, open by default, globe
  disabled-honest).
- **Test collisions the new controls caused**: T24's `radio System` needed
  scoping to the Theme radiogroup (Motion now has a second System radio);
  T28c needed the documented 30d/7d labels. Anything new that adds
  same-named controls must expect role-name ambiguity in the shell suite.
- Gates: 600/0, shell 52/52, setup 19/19, journeys 12/12, a11y 18/18 (new
  pane/controls included), native 20/20, tsc/vite clean.
