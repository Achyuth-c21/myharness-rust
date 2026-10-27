# 04 — Investigation Progress Tracker

**Snapshot:** 2026-10-20. This tracks investigation work, not product development. The rebuild
roadmap is in [13-implementation-roadmap.md](13-implementation-roadmap.md).

## 1. Overall status

| Workstream | Status | Notes |
|---|---|---|
| Environment and access | Complete | App version, platform, user-data, CDP, asar trees recorded |
| Static source extraction | Complete | Pristine/current app.asar extracted (current in private archive); no source maps |
| Renderer identification | Complete | Local SPA from `resources/ion-dist`, not remote claude.ai |
| Asar diff / patch isolation | Complete | Three changes isolated; adapter characterized |
| Main navigation and shell | Complete | Cowork/Code, sidebar, account, update banner |
| Chat/new task | Complete for default state | Composer, menus, model picker, search, filter |
| Task detail/transcript | Complete for observed task | Tool blocks and message layout captured |
| Projects | Partial | Empty state, choice dialog, and form; existing-project detail not created |
| Artifacts | Partial | Library and new-artifact flow; pane unavailable |
| Scheduled tasks | Complete for page/templates | Create/edit and history not exercised |
| Customize | Complete for lists | Skills/Connectors/Plugins; no plugin install |
| Settings | Complete for all visible sections | Cowork memory contents intentionally not retained |
| 3P setup window | Complete for forms | All tabs/providers; changes discarded |
| Provider/model architecture | Complete for app-side contracts | Server implementation unknown |
| IPC/preload surface | Complete at namespace/contract level | 131 flags, 17 settings + 60 web namespaces |
| Storage/data model | Complete at schema level | Values not copied; personal content redacted |
| UI design system | Complete for light/dark tokens and core controls | Responsive/OS-width variants limited |
| Keyboard shortcuts | Complete for menu accelerators | Some in-app shortcuts may be undocumented |
| Rebuild documentation | Complete | Docs 00–20, 15 feature specs, 14 screen specs, traceability/test catalog |
| Evidence redaction | Complete | Structured subset in `evidence/redacted/`; no screenshots approved |
| Test task cleanup | Complete | “New live artifact” task deleted; no matching metadata or active generation |

## 2. Completed evidence work

### 2.1 Application and source

- Identified Claude Desktop `2.42304.0`, Electron `44.7.2`, Chromium `152`, Node `24.21.0`.
- Extracted pristine `app.asar.orig` and live `app.asar` into separate evidence trees.
- Found the actual renderer: 201 MB, 4,224-file `ion-dist` SPA with `assets/v1/index-D3kGySMO.js`.
- Located 35,441 `en-US` strings.
- Traced `app://localhost` static serving, CSP injection, custom-3P API stubs, and origin checks.
- Extracted 36 native menu accelerators and labels.
- Extracted 186 `flatKey` configuration descriptors and provider schemas.
- Extracted the 131 live feature gates and their statuses.
- Captured all 8 contextBridge namespaces; 17 settings and 60 web namespaces are documented.

### 2.2 Runtime UI

- New task/Cowork home (clean and with history).
- Code dashboard.
- Projects empty state; “Create new project”; “Use existing folder”.
- Artifacts empty state; new-artifact task.
- Scheduled empty state and six templates.
- Customize Skills, Connectors, Plugins.
- Account menu and Language dialog.
- Settings: General, Privacy, Usage, Claude Code, Cowork, Import & export, System, Developer.
- Search palette and filter menu.
- Task detail/transcript.
- Composer: attachments, Skills, Connectors, slash commands, project/folder selector.
- Model picker, model browser, “needs API key”.
- 3P setup: Connection, Capabilities, Connectors, Telemetry & updates, Limits, Appearance, Plugins,
  Egress, Source.
- Provider forms: Gateway, Claude API, Bedrock, Bedrock Mantle, Vertex AI, Foundry.
- Light and dark theme tokens, computed component styles, fonts.

### 2.3 Storage and configuration

- 267 localStorage keys normalized to 104 patterns (no values retained).
- IndexedDB `keyval-store` v1 schema.
- Configuration library: active profile, 7 stored profiles, `_meta.json`.
- Local session store: org/account root, session metadata, usage ledger, scheduled tasks, plugin
  cache, memory roots, audit files.
- Model catalog: signed download with sidecar; schema and surfaces.
- Logs: startup, protocol installation, model discovery, plugin sync, session restore, shutdown,
  VM/mount lifecycle.

## 3. Work state

All planned investigation and documentation work is complete for the observed surface. Remaining items
are intentional limitations or optional deeper captures, listed in §4 and §7.

### 3.1 Rebuild state (this file tracks phase advances, not products)

| Phase | Slice | Status | Where |
|---|---|---|---|
| Phase 2 | 1–6 | Implemented (not exercised end-to-end; §4.9 records the live-path gap) | `docs/17` §4.7–4.9 |
| Phase 3 | 1 | Implemented on the `phase-3` lane: `crates/provider-sdk` (the `docs/12` §8 adapter contract), the native `crates/providers/gateway` adapter (discovery, `/v1/messages` SSE streaming + non-streaming, model-counting, probe), the `TEST-AI` wire battery (23 fixtures over a real-HTTP mock gateway), `crates/bridge::helper` discovery delegation, and the setup `testConnection` command whose **Test connection** performs a real one-token inference | `docs/17` §4.10, `docs/14` §9 items 21–23 |
| Phase 3 | 2 | Implemented, same lane (`9829197`): the native `crates/providers/anthropic` adapter — the Claude API family's own Messages client (`x-api-key`, first-party host + optional base URL, `/v1/models` discovery with no injected limit param, native count_tokens, sk-ant-oat refusal), a 21-fixture wire battery + 10 unit tests, the setup probe lane split per family (helper kind refused for this family), and the renderer's **Test connection** unlocked for `gateway` and `anthropic`. Both adapters now preserve every reported usage count including web-search input tokens (`AI-USAGE-01` repair) | `docs/17` §4.11, `docs/14` §9 items 24–26 |
| Phase 3 | 3 | Implemented on the `phase-3` lane: the native `crates/providers/bedrock` adapter — the Bedrock Runtime surface whose payload grammar is the Anthropic Messages one (`anthropic_version` body field, **no `model` field — the path carries the model or ARN**, ARN `/` percent-encoded), binary EventStream framing with verified CRC-32C decode/encode, `nextToken` control-plane discovery with no invented parameters, no native token count (refused, never estimated), conditional model echo, identity credential kinds (`sso`/`external-idp`/`vendor-profile`) refused fail-closed, and a beta-carrying request refused visibly (no recorded beta transport — `docs/14` §9 item 27). 22 wire fixtures + 12 unit tests (incl. the published CRC-32C vector and a hand-assembled frame), the `test_connection_bedrock` bridge lane (region gate, discovery fallback, helper kind), and the renderer's probe unlocked for `gateway`/`anthropic`/`bedrock` | `docs/17` §4.12, `docs/14` §9 item 27 |
| Phase 3 | 4 | **Removed (owner decision, 2026-10-23).** The Vertex AI family is out of rebuild scope. The implemented slice — the native `crates/providers/vertex` adapter, its data-only-SSE raw-prediction surface, 21 wire fixtures + 10 unit tests, the `test_connection_vertex` bridge lane, and the renderer unlock — was deleted together with the family's config surface (form fields, `validate_vertex_model_id`, `inferenceVertex*` keys, `ProviderKind::Vertex`). The name the owner put to it: the wire-test build (reqwest + tokio + mock SSE server, debug-linked) crashed the laptop; the family is simply out. Pre-removal snapshot: `7b84886` on the `phase-3` lane. The disposed wire reading remains an unknown, `docs/14` §9 item 28 (annotated); the shipped app keeps the family (`docs/09` §2.2 stands) | `docs/17` §4.13, `docs/14` §9 item 28 |
| Phase 3 | 5+ | Not started: the remaining provider adapters (`mantle`, `foundry`), MCP tool policy, catalog verification | `docs/13` §5, `docs/17` §4.10 deferred rows |
| Phase 3 — open model layer | C (owner pivot, 2026-10-23) | Implemented on the `phase-3` lane (`6e7a738`, `8a3d1b8`): **owner-approved widening to an open multi-provider harness** — any model, any provider, plus a user-defined OpenAI-compatible endpoint. All family-level model-ID gates removed (`validate_open_model_id` checks shape only; provider's own wire error is the refusal, flip tests in all three adapters + the bridge's first-draft-entry lane test); the native `crates/providers/openai` wall for the OpenAI Chat Completions wire serving first-party `openai` (static key required) and `custom` (base required, credential OPTIONAL → keyless send with no `authorization` header) with a 21-fixture battery; the shared bridge lane `test_connection_openai_compat` with the optional-credential resolver; the additive discovery command `setup_discover_openai_models`; the renderer's provider-family picker and family-aware Test connection / Discover models. `mantle`/`foundry` remain stubbed and locked. Workspace now 26 members | `docs/13` Appendix C, `docs/17` §4.14, `docs/14` §9 items 29–32, `docs/19` IMP-18 (status `8422880`) |
| Phase 4 | 1 | Implemented on the `phase-4` lane (`fc2e62a`, `bdc472d`): the **persistence core**. `crates/store`: SQLite WAL schema v1 (`sessions`, `turns`, `usage_events`, `audit_events`, `quarantined_records`), forward-only idempotent migrations that refuse a newer-schema database untouched, content-addressed blobs with hash-validation path confinement, quarantine-then-fail reads for corrupt rows, corrupt-database set-aside (renamed `.corrupt-*`, never deleted), owner-only file modes, and a per-session HMAC audit chain. `crates/sessions`: the session service — per-session audit-key lifecycle behind the `AuditKeyStore` seam (in-memory in this slice; keychain lands with the secrets work), chained lifecycle events (create/turn/archive/hold), delete that honors legal hold and sweeps transcript blobs + key. 21 + 8 tests green. Slice 2 adds the turn loop; retention/drafts/usage aggregation (005, 008, 009, `TEST-DATA-004/007`) and the renderer surface remain later slices | `docs/17` §4.15 |
| Phase 4 | 2 | Implemented on the `phase-4` lane: the **turn loop** (`crates/agent`) — transcript replay (empty turns skipped, consecutive same-role turns merged), user turn persisted before the send, adapter `authenticate` + `create_turn`, typed-stream accumulation (text/reasoning/signature/citations/tool blocks), native stop reason preserved (schema **v2** adds `turns.stop_reason`, threaded through `put_turn`/`append_turn`), cancellation via `CancellationToken` → `partial` turn with delivered content + ledger row marked cancelled, provider failure → `error` turn + ledger row carrying the provider's own message, model-provider mismatch refused, tools refused before contact, no retries. 13 agent + 21 store + 8 sessions tests green; zero clippy warnings on the three crates. Slice 3 adds recovery/usage/retention; slice 4 the profile→adapter factory and bridge; the renderer follows (`TEST-SESSION-002` is now covered at the data level) | `docs/17` §4.15 |
| Phase 4 | 3 | Implemented on the `phase-4` lane (`e1ad4bd`): **live-turn crash recovery, local-day usage, retention sweep**. Store: `finish_turn` validates session ownership and is one-shot (only a live `partial`/no-error row transitions; the content blob is written only after that check), `list_usage_between` bounds the ledger window, delete removes only blobs no surviving turn references. Sessions: `begin_assistant_turn`/`finish_turn`/`recover_interrupted`/`rename`, local-day `summarize_usage` (cached tokens separate; 1–366 days; explicit `FixedOffset`), and `sweep_retention` (surface + age filter, global and per-session holds, `deleted`/`held`/`kept`). Agent: the assistant placeholder is the durable start marker, recovery turns leftovers into `error` with the restart text, cancelled turns always finish as `partial` with the cancellation text. Review hardening: opening a session mid-stream does not recover the live turn; archive/delete refuse with `TURN_IN_PROGRESS`; `CancelledBeforeSend` keeps `userTurnRecorded` truthful; audit keys resolve before row transitions and a failed key creation rolls the session row back. 24 store + 15 sessions + 15 agent tests green; clippy clean | `docs/17` §4.15 |
| Phase 4 | 4 | Implemented on the `phase-4` lane (`955f47d`): **chat commands, profile→adapter factory, keychain audit keys**. `crates/bridge/src/chat.rs`: `ChatCommand`/`ChatResponse`, `ChatService<K,R>`, `ChatTargetResolver`; production `ProfileTargets` resolves only the **applied** profile (no model discovery at send), reuses setup's credential/header/base-URL rules, refuses `CONFIG_MISSING`/`CONFIG_VALIDATION`/`ADAPTER_NOT_AVAILABLE` (locked `mantle`/`foundry`), and refuses a send whose session provider/model no longer matches the applied profile (`MODEL_UNAVAILABLE`, no substitution). Two-step create-then-send; Tauri `chat_command` + `chat://turn-event`; `KeyringAuditKeys` (keychain account `session-audit/<uuid>`). 40 setup + 9 chat tests green; desktop `cargo check`; clippy clean | `docs/17` §4.15 |
| Phase 4 | 5 | Implemented on the `phase-4` lane (renderer commit): **renderer surface**. `apps/desktop/src/chat/` (typed commands/events, drafts, shared session list, provider-qualified label), New-task send → `/cowork/:sessionId` handoff with the draft moved before the first await, transcript surface (streaming deltas, Stop, provider error verbatim, non-`end_turn` line, reasoning behind `<details>`, copy, auto-follow, rename/archive/delete with inline confirm), sidebar task rows, Active list, `Ctrl+N` draft clear, 7/30/90-day usage panel in Settings with the device-local/billing caveat. Drafts survive any failure (including provider failure, per the composer spec) and clear only on completion. Browser E2E 35/35 (T27/T28 updated; T31–T33 added; old T31/T32 renumbered T34/T35), `tsc --noEmit`, `vite build` green. Honest gaps: the browser has no Tauri host, so a live stream/usage read is not E2E-exercised; no system prompt, retry, picker, attachments, retention timer/UI, or per-surface usage split (`docs/14` §9 items 33–40) | `docs/17` §4.15, `docs/14` §9 |
| Phase 5 | 1 | Implemented on the `phase-5` lane (`f45e71e`): **MCP lifecycle foundation** (`crates/mcp`). `rmcp` 3.7 with `default-features = false`; real stdio (`TokioChildProcess`) and Streamable HTTP; legacy HTTP+SSE is modelled but returns `UnsupportedTransport`; `McpManager` start/start_all/shutdown with non-fatal per-server failure, optional start timeout, reconnect after a closed session; `<server>:<tool>` registry, per-session frozen snapshots with missing names, snapshot-gated `call_tool` with caller-supplied timeout, content preservation (text/image/audio/embedded resources/resource links/structured), redacting `Debug`/errors, and rebuild-defined schema bounds (4,096 tools, 256 KiB/schema). Feature-gated stdio fixture; 2 unit + 15 manager + 2 real-stdio tests; clippy clean. Not wired to providers, agent, bridge, renderer, or boot | `docs/17` §4.16, `docs/14` §9 items 41–47 |
| Phase 5 | 2 | Implemented on the `phase-5` lane (`f054f65`): **policy, approvals, egress, execution gate**. `crates/agent/src/tool_policy.rs` — allow/ask/deny, argument-scoped rules, fail-closed `deny > ask > allow > default(Ask)`, `Once`/`Session`/`Persistent` scopes, persistent gated by `mcpPersistentAlwaysAllowEnabled`, scheduled lifetime with `0` disabling lasting approval, in-memory `ApprovalStore`, canonical SHA-256 argument fingerprint, and `visible_tool_names` (hides only unconditional denies). `crates/agent/src/tool_exec.rs` — `execute_tool_call` returns `Completed`/`Denied`/`NeedsApproval`; denied calls never reach the policy-blind `ToolExecutor`; executor failures become model-visible error results; text/image content preserved. `crates/mcp/src/egress.rs` — fail-closed exact-host allowlist, `http`/`https` only, manager checks before connect (zero connector calls on denial); stdio is explicitly not inspected. 13 policy + 8 execution + 15 turn agent tests; 8 unit + 18 manager + 2 stdio MCP tests; clippy clean. Still not wired into `run_turn`; `TEST-AI-019` is a primitive only | `docs/17` §4.16, `docs/14` §9 items 41–47 |
| Phase 5 | 3 | Implemented on the `phase-5` lane (`03d5102`, hardening `12d508a`, `b8391ae`): **tool turns, approval bridge, catalog, app integration**. `crates/agent` — bounded multi-round tool loop, provider-side visibility filter, provider-safe wire-name mapping and back, JSON Schema argument validation, tool-row replay, `TurnEvent::ToolResult`, data-only output sanitization (1 MiB; audio explicit error). `crates/bridge` — `mcp.rs` (config, egress-checked keychain OAuth + refresh, approval prompt, `ToolRuntime`, frozen snapshots, durable `mcp/approvals.json`, audit-chain `mcp.tool.approved`, usage aggregation, retry/forget) and `catalog.rs` (skills/plugins, plugin MCP servers, next-start enablement). `crates/mcp` — per-server OAuth, atomic schema cache, deterministic tool search, bounded shutdown, duplicate-name refusal. Tauri boot starts MCP off the boot path and emits approval/status events; renderer adds the approval modal, session tool picker, skill picker, and Developer/Skills/Plugins panes; browser preview reports `BRIDGE_UNAVAILABLE`. Gates: 55 bridge unit + 11 chat + 4 tool-turn integration, 25 MCP unit + 22 manager + 2 real-stdio (`--features fixture-server`), 57 agent, 7 skills, 7 plugins; provider wire batteries unchanged; `cargo check -p myharness-desktop`; `tsc --noEmit`; `vite build`; browser E2E 38/38; clippy clean on affected crates. Remaining gaps: no live HTTP MCP fixture, no live OAuth/keychain exercise, no marketplace/signature verification, no persistent failure toast, no model-driven tool search, prompt loss on navigation, global approval lock, no default MCP timeout, no DNS/IP egress check, legacy SSE and audio unsupported | `docs/17` §4.16, `docs/14` §9 items 44–52 |
| Phase 6 | 1 | Implemented on the `phase-6-code` lane (uncommitted at report time): **Code dashboard and Code session**. `crates/bridge` adds strict `ListCodeSessions`/`CreateCodeSession`/`OpenCodeSession`/`SendCodeTurn`/`CodeDashboard`; Code routes cannot open or send into Cowork sessions, and a stored Code session revalidates its canonical folder and permission policy on every send. `crates/bridge::policy` derives a fail-closed `CodePolicy` from capabilities config (explicit `disableBypassPermissionsMode: false` is the only Bypass grant; absent/malformed disables it), canonicalizes folders, and enforces configured allowed roots. `crates/agent::permission` implements Default/Accept-edits/Plan/Bypass enforcement in the tool gate; Plan advertises no tools and denies stale calls; Bypass never overrides a policy Deny; Accept-edits only auto-approves a host-declared edit-tool set (empty in this slice). `crates/sessions` adds Code-only usage aggregation (7d/30d/all, local days, model share, all-time totals plus a capped all-time activity strip with `truncated`). The renderer adds `/epitaxy` (greeting, real local stats, Overview/Models, activity strip, recent sessions, real folder rows, native folder picker, permission selector, disabled Fast) and `/epitaxy/:sessionId` reusing the transcript/streaming/tools/approvals screen with Code metadata. Gates: 60 bridge unit + 16 chat + 4 tool-turn integration; 16 agent tool tests; 25 store; 19 sessions (incl. 4 Code usage); 25 MCP unit + 22 manager + 2 real-stdio; `tsc --noEmit`; `vite build`; browser E2E 41/41; clippy clean. Deferred: terminal/PTY, SSH/WSL/remote, worktrees/GitHub, Fast, Projects, Artifacts, Scheduled, cross-session Search; live native folder picker and native populated dashboard not exercised; renderer populated states are covered by mocked E2E | `docs/17` §4.4.1, `docs/14` §9 items 53–58 |

| Phase 6 | 2 | Implemented on the `phase-6-code` lane (`35958b8`): **Projects**. Schema v3 (`projects` + soft `sessions.project_id`), `crates/projects` service (bounds refused, folder never rewritten), bridge `ListProjects/CreateProject/OpenProject/UpdateProject/DeleteProject` reusing the one Code folder validator, `CreateSession` project/folder binding with the path-as-data block on folder-bound turns, `reveal_workspace_folder` shell command, renderer list + captured create dialog/form + unobserved-design detail, composer project chip. Gates: 24 bridge chat tests, 47 browser E2E (T39–T41 added); clippy clean; tsc/vite green | `docs/17` §4.4.2, `docs/14` §9 items 59–61 |
| Phase 6 | 3 | Implemented (`8aa7987`): **Artifacts**. `CreateArtifactTask` creates one marked Cowork session (no auto-send — the shipped generated prompt was never captured), the suggestion sends its own visible copy, the library derives rows from real marked sessions only, the viewer is stated unavailable. E2E T42/T43 | `docs/17` §4.4.2, `docs/14` §9 item 62 |
| Phase 6 | 4 | Implemented (`48eb6ac`): **Scheduled**. `crates/scheduler` (cron subset, DST gap/fall-back rules, arm/catch-up with 60 s grace), schema v4 (`scheduled_tasks`/`scheduled_runs`), bridge commands + run-now + the shell's 30 s tick with arm-on-boot, runs on the scheduled approval context, renderer screen with captured banner/empty/templates and an explicit editor; keep-awake disabled. 5 scheduler unit tests, 3 bridge tests, E2E T44/T45 | `docs/17` §4.4.2, `docs/14` §9 items 63–64 |
| Phase 6 | 5 | Implemented (`424eabc`): **Search**. Palette over real local data (sessions both surfaces + projects; recent set on empty query; labelled "Go to" tail), sidebar filter menu (Type/Status/Last activity/Group by/Sort by) composing with AND, immediate apply, badge, reset, distinct filtered-empty state, Unread disabled, filters persisted. Metadata-only; no transcript search, no pinning. E2E T46/T47 | `docs/17` §4.4.2, `docs/14` §9 item 65 |
| Phase 7 | 1 | Implemented on the `phase-7` lane (`4d18d87`): **Memory**. `crates/memory` (new) — the captured file-based model: Markdown entries are the truth, `index.json` is a derived cache that heals on open, a corrupt file is renamed aside (`.corrupt-<ms>`) and named in the `events.jsonl` audit trail, content bounds refused not truncated, the sha256 baseline hash is refreshed only by app-managed mutations so external edits stay detectable, reset is explicit and audited. `memoryEnabled` joins the Capabilities tab; the bridge reads the gate fail-closed from the merged profile per command (absent = on, the captured default); a broken memory directory degrades memory to `UNSUPPORTED`, never chat. Renderer: the Cowork settings section is the memory management panel. 10 memory unit tests + 3 bridge integration tests | `docs/17` §4.17, `docs/14` §9 items 66–67 |
| Phase 7 | 2 | Implemented on the `phase-7` lane (uncommitted at report time): **the usage split view**. Store `list_surface_usage_between` (surface-scoped window over the same bounds as the combined query), sessions `usage_summary_split` (combined window + Cowork/Code shares; a surface with no rows is a zero summary, never a missing field), bridge `usage` response carries `surfaces`; the Settings usage panel renders the per-surface cards beside the combined totals and daily chart (`docs/screens/settings.md` §Usage: Cowork and Code token/search summaries). New tests: 1 store + 1 sessions + 1 bridge; the panel's now-false "no surface column" caveat removed | `docs/17` §4.17, `docs/14` §9 item 68 |
| Phase 7 | 3 | Implemented on the `phase-7` lane (`0f76676`): **privacy and diagnostics**. `crates/telemetry` (implemented from the scaffold) — the effective telemetry policy read fail-closed from the merged profile (each block switch independent, malformed fails closed, the service block list fails closed to the three known services) and the write-time redaction (Bearer/Basic schemes, credential token prefixes, URL userinfo/query/fragment); the crate ships **no emitter** — no crash reporter, no analytics, no OTel exporter. Bridge `DiagnosticsPreview`/`DiagnosticsBuild`: the manifest enumerates exactly what a bundle includes, the build requires explicit consent (`CONSENT_REQUIRED`), the configuration passes the export redaction then the free-text pass, the bundle lands in the app-managed diagnostics directory, and `diagnostics/log.jsonl` records the action without contents; there is no upload in this build. Renderer: the Privacy pane carries the captured disclosure verbatim plus the stated no-emitter posture; E2E T48. 11 telemetry tests + 3 bridge tests | `docs/17` §4.17, `docs/14` §9 |
| Phase 7 | 4 | Implemented on the `phase-7` lane (`6b3fe47`): **retention and tombstone GC**. Schema v5 `deleted_sessions` tombstones — a delete records the blob refs it owned in the same transaction that removes the rows, and `sweep_tombstones` finishes crash-interrupted sweeps (reference-checked) and prunes markers, idempotent across reopens. `RetentionSettings` read fail-safe from the merged limits section (absent/malformed days = no automatic deletion; malformed hold = hold); `retention_sweep` at boot through the Phase-4 engine; the shipped `chatSessionRetentionDays` is not silently mapped (one chat surface in the rebuild). No timer — the shipped schedule was never observed. 1 store test + 1 bridge test | `docs/17` §4.17, `docs/14` §9 item 69 |
| Phase 7 | 5 | Implemented on the `phase-7` lane (`aa1337e`): **the update lifecycle**. `crates/updates` — `UpdatePolicy` (72 h enforcement window and 24 h relaunch window by default, `0` = immediate, malformed keeps the default), the Running/Available/Downloading/Staged/Updated/RolledBack/Failed state machine with the streaming-task session guard, and fail-closed signature verification: the shipped feed's wire format and public key were never captured, so the production verifier refuses every bundle and the old version stays runnable by construction. Renderer: the System pane reports the shell version (desktop-only boundary) and the recorded no-feed posture; E2E T49. 10 tests | `docs/17` §4.17, `docs/14` §9 item 70 |
| Phase 8 | 1 | Implemented on the `phase-8` lane (uncommitted at report time): **the Responses extension — Phase 8's first reference extension**. `crates/providers/openai-responses` (`myharness-provider-openai-responses`), the native wall for the OpenAI **Responses** wire (`POST {base}/responses`, its own input-item grammar and SSE stream; `ProtocolFamily::OpenAiResponses` in the sdk), sibling to the Chat Completions wall with a shared design and no shared code. First-party OpenAI only (static key required); `store: false` explicit; system blocks join into `instructions`; documents natively as `input_file`; media inside tool results refused upstream; reasoning omitted from replay; prefix model echo; exact usage (cached tokens in, `reasoning_tokens` dropped — no app channel); stop mapping with `tool_use` from emitted items; **no `[DONE]` sentinel** — `response.completed`/`response.failed`/`error` terminate, an undocumented event type errors, a stream ending without a terminal event is malformed; annotations surface as citations; count_tokens refused, never estimated. 17-fixture wire battery + workspace wiring (27 members). The setup family, bridge probe lane, and renderer unlock are unbuilt (a later Phase 8 slice) | `docs/17` §4.18, `docs/14` §9 items 71–72 |
| Phase 8 | 2 | Implemented on the `phase-8` lane (uncommitted at report time): **the Responses family's setup surface**. `ProviderKind::OpenAiResponses` (serde `"openai-responses"`) with the family's own key spellings (`inferenceOpenAiResponsesApiKey`/`inferenceOpenAiResponsesBaseUrl` — owner-defined rebuild keys), the form family (static key required — the wire has no keyless variant; base URL optional, empty = the first-party host), egress derivation (override or `api.openai.com`), the export label, the bridge probe lane `test_connection_openai_responses` (strict resolver: an absent credential is a `CONFIG_VALIDATION` refusal with zero requests), the chat factory arm (provider id = the wall's own `id()` spelling so the agent loop's identity check holds), the additive discovery command `setup_discover_openai_responses_models` (single page, tolerant parse, mapped onto the shared setup outcome), and the renderer's family option ("OpenAI (Responses)"), Test connection unlock, and Discover models arm. Gates: config 64, bridge 64+32+5, clippy `-D warnings` on config/bridge/responses, `cargo metadata`, `tsc --noEmit`, `vite build`, E2E `shell.spec.mjs` ALL PASS + `setup-window.spec.mjs` 16/16 | `docs/17` §4.18, `docs/14` §9 item 71 |
| Campaign | 1 — baseline and launch truth | Whole-application campaign phase 1 (`docs/17` §4.20): 596/0 Rust tests, 52/52 shell E2E, 16/16 setup E2E, 19/19 native checks, clippy/tsc/vite clean; the real release binary launched under Xvfb/X11 and rendered the embedded bundle. Findings: `WA-01` (P0, fixed — release builds were dev builds because `custom-protocol` was absent), `WA-02` (P1, open — 10 serious light-mode contrast failures on the preserved `--mh-text-3`, registered as `IMP-39`), `WA-03` (P3, open — the native fixture proves a process/webview, not a rendered app) | `docs/17` §4.20, `docs/19` IMP-39 |

## 4. Blocked or inaccessible

| Item | Why | Handling |
|---|---|---|
| Anthropic backend | 3P mode has no Anthropic account/session | Document app-side contracts only |
| Source maps / TypeScript | Not shipped | External behavior + minified source |
| Native menus | OS-rendered | Use source accelerators and screenshots |
| File pickers | Native dialogs | Document pre/post states; generic folder paths |
| Existing project detail | Requires creating a project with personal content | Mark unknown |
| Artifacts pane | `artifactsPane` unavailable | Mark unavailable; document library flow |
| Cowork browser | `coworkBrowser` unsupported | Mark unsupported |
| Computer use | Platform/consent gates | Mark unavailable |
| iOS Simulator | Platform gate | Mark unsupported |
| Self-hosted/cloud sessions | Not configured | Mark unknown |
| WSL | Disabled on Linux | Mark unsupported |
| macOS/Windows | Not available | Mark not inspected |
| Update behavior | Update not applied | Document offered state only |

## 5. Sensitive-data cleanup checklist

- [x] Do not include API keys, OAuth tokens, or cookies in docs.
- [x] Do not include account email, org UUID, device UUID, or user-data hashes.
- [x] Do not include personal task titles/transcripts in docs.
- [x] Do not include memory entries or file paths beyond generic roots.
- [x] Do not include usage totals or charts as personal data.
- [x] Keep screenshots in the private archive outside the repository; no screenshot is approved.
- [x] Do not publish saved setup text evidence; it remains in the private archive.
- [x] Scan the approved subset for keys, tokens, emails, and absolute paths.
- [x] Document raw-evidence handling in `evidence/README.md`.

## 6. Known anomalies

- The app is being run with a local inference patch. Current model names and model counts are not
  shipped behavior.
- Model discovery once reported 406 gateway models but only 2 selectable `inferenceModels`; these
  are different sources, not a contradiction.
- Multiple menus can remain mounted after closing. Runtime evidence must note whether a menu was
  toggled closed.
- The setup window and main window share an origin and can be confused by CDP target selection.
- The accidental “New live artifact” task was stopped and deleted; its metadata is absent and no
  background generation remains.
- The active provider profile points at a local adapter. It is **not** a native provider integration
  and must not be used as a reference architecture.

## 7. Remaining optional work

| Priority | Action | Status |
|---|---|---|
| P2 | Capture existing project detail, artifact viewer, scheduled editor/history | Blocked by personal-content and gate constraints |
| P2 | Exercise permission dialogs and native pickers | Not DOM-inspectable / destructive |
| P2 | Inspect macOS/Windows variants | Unavailable platform |
| P3 | Re-run against a future app version | Out of scope for `2.42304.0` |

No remaining P0/P1 documentation or cleanup item is open.
