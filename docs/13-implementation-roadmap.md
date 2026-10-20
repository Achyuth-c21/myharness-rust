# 13 — Rebuild Roadmap, Modules, and Acceptance Criteria

This roadmap turns the evidence into a rebuildable plan. It is a **rebuild design**, not a claim
about Anthropic’s internal source or backend. Where the original implementation is unknown, the plan
chooses an explicit, testable strategy and labels it as a decision.

## 1. Scope and non-goals

### In scope

- Electron desktop shell with main/preload/renderer separation.
- The observed routes and screens: Cowork, Code, Projects, Artifacts, Scheduled, Customize, Settings,
  Search/Filter, third-party setup.
- Local session persistence, transcripts, tool approvals, artifacts, memory, usage, retention.
- Native provider adapters for Claude API, Bedrock, Bedrock Mantle, Vertex AI, Foundry, and an
  Anthropic-compatible Gateway.
- MCP servers, skills, plugins, desktop extensions, and document skills.
- Third-party capability/policy/telemetry/egress configuration with a separate setup window.
- An extension layer for additional native providers/SDKs/protocols.
- Security, accessibility, tests, packaging, update/rollback.

### Out of scope

- Reimplementing Anthropic’s hosted account, billing, telemetry, or model-serving backend.
- A universal protocol translator, proxy, router, or gateway.
- Bypassing provider model policy or synthesizing model responses.
- Platform features that were unavailable and unobserved: computer use, iOS simulator, WSL,
  self-hosted/cloud sessions, Cowork browser, artifacts pane. They may be specified later from a
  platform where they are available.
- Copying personal data, prompts, or credentials from the inspected install.

## 2. Recommended stack

| Layer | Choice | Rationale |
|---|---|---|
| Shell | Electron 44.x (match observed major) | Preserves window/protocol/security model and observed behavior |
| Language | TypeScript everywhere | Safer IPC/contracts; original was TS before minification |
| Main | Node 24.x | Matches runtime; native SDK support |
| UI | React + Tailwind CSS v4 | Matches observed stack and tokens |
| Routing | Client-side router with typed route table | Preserves `/new`, `/epitaxy`, etc. |
| State | Query cache + small local stores | Original uses React Query/local stores; avoid global mutable singleton |
| Persistence | SQLite (WAL) + content-addressed files | Better atomicity/retention than many JSON files |
| Validation | Zod at every boundary; AJV for JSON Schema where needed | Provider/tool/config validation |
| Logging | Structured JSON logs with redaction | Auditable, supportable |
| Errors | Sentry-compatible crash/error queue | Matches product behavior |
| Terminal | `node-pty` + xterm | Code/terminal sessions |
| MCP | Official MCP SDK | Protocol fidelity |
| Packaging | Electron Forge (deb/rpm/dmg/msix) | Matches observed makers |
| Tests | Vitest + Playwright Electron + mock native SDK servers | Unit, UI, wire conformance |

**Rebuild decisions.** The original shipped React/Tailwind/Electron and used JSON/NDJSON storage.
SQLite is an improvement for atomicity and retention; it is not claimed as original. Preserve the
observable storage semantics and migration behavior.

## 3. Target module layout

```text
apps/desktop/
  src/main/
    lifecycle/          windows, protocols, single instance, deep links
    config/             profiles, merge, validation, credentials, setup IPC
    providers/          native adapters (one package per provider)
    agent/              turn loop, streaming, tools, approvals, cancellation
    mcp/                servers, transports, OAuth, tool registry, policy
    sessions/           metadata, transcripts, restore, retention
    artifacts/          versions, content store, preview isolation
    projects/           project folders/instructions/memory
    scheduler/          cron, wake, runs, approvals
    search/             session/file/transcript indexing
    skills/             bundled/user skills
    plugins/            marketplaces, installs, org bundles
    telemetry/          crash/usage/OTel, redaction
    updates/            feed, verify, enforce, rollback
    bridge/             generated typed IPC
  src/preload/
  src/renderer/
    routes/             new, cowork, epitaxy, projects, artifacts, scheduled
    screens/
    components/         design system
    composer/
    settings/
    setup/              setup-desktop-3p window
  packages/
    protocol-core/      internal TurnEvent/ModelDescriptor types
    provider-sdk/       adapter interface + conformance kit
    shared/             schemas, redaction, policy
  tests/
    unit/ integration/ e2e/ wire/ fixtures/
```

The `providers/` directory is not a router. Each adapter is a separate native integration. The agent
selects exactly one adapter instance for a turn.

## 4. Build phases

### Phase 0 — Evidence and guardrails

**Goal:** freeze the behaviors that must be preserved and the behaviors that must not be copied.

Deliverables:

- Evidence index and trust levels ([15](15-evidence-index.md)).
- Feature/screen traceability ([17](17-traceability.md)).
- Explicit rejection of the local adapter ([12](12-ai-architecture.md)).
- Fixtures: model catalog, MCP schemas, sample sessions with synthetic content, golden UI tokens.
- Security threat model and redaction policy.

Exit criteria:

- No credential or personal data in fixtures.
- Every “must preserve” row has an owner/module/test.
- A lint rule/test forbids global `fetch` monkey-patching and arbitrary provider URL translation.

### Phase 1 — Shell, protocols, and design system

Build:

- Single-instance lifecycle, deep links, privileged schemes.
- Main/setup window creation with the observed security flags.
- `app://localhost` static serving, CSP, SPA fallback, 503 gate.
- Title bar, sidebar, mode switch, routes, back/forward, theme tokens.
- Loading/empty/error shells, error overlay, native menu accelerators.

Tests: `TEST-ARCH-001..010`, `TEST-UX-001..014`.

Exit: route smoke tests pass; renderer has no Node access; CSP blocks inline script except the
documented bootstrap injection; a corrupted provider config still opens the setup window.

### Phase 2 — Configuration, profiles, and setup window

Build:

- Profile library, draft/apply/discard, active-profile metadata.
- Managed bootstrap overlay, read-only state, health/recheck, tainted handling.
- Credential abstraction + keychain + helper-script runner.
- Setup tabs: Connection, Capabilities, Connectors, Telemetry & updates, Limits, Appearance, Plugins,
  Egress, Source.
- Provider form schemas and validation; model list/discovery/catalog controls.
- Egress derivation and export/support redaction.

Tests: `TEST-CFG-001..012`, `TEST-UX-009`.

Exit: applying an invalid profile does not change the live config; secrets never appear in renderer
storage/logs; all six provider forms validate their model IDs; profile switch/relaunch is observable.

### Phase 3 — Native provider adapters

Order:

1. Anthropic-compatible Gateway (the current deployment path).
2. Claude API.
3. Bedrock.
4. Vertex AI.
5. Bedrock Mantle.
6. Foundry.

For each adapter:

- Native SDK/protocol client with pinned version.
- Auth + credential rotation.
- Model discovery/static list + validation.
- Capability descriptor.
- Streaming, cancellation, errors, usage, token counting.
- Native tool calling and multimodal content.
- Mock-server wire tests.

Tests: all `TEST-AI-*` applicable to that provider.

Exit: every adapter passes its native-wire fixture; no adapter imports the local translation shim;
missing credential/model blocks send without substitution.

### Phase 4 — Agent, sessions, and transcript

Build:

- Session/turn state machine.
- Agent loop with tool calls, approvals, cancellation, retries.
- Transcript persistence + restore.
- System prompt/context assembly with redaction and snapshots.
- Usage ledger and token limits.
- Error/partial/stop states.
- Code/Claude Code session bridge where applicable.

Tests: `TEST-SESSION-001..010`, `TEST-AI-001..024`, `TEST-DATA-001..008`.

Exit: kill/restart during a stream leaves a recoverable, non-duplicated session; cancellation is
idempotent; stop reasons are preserved; usage matches provider reports.

### Phase 5 — MCP, tools, skills, plugins

Build:

- Managed/developer/plugin MCP lifecycle (stdio/HTTP/SSE).
- OAuth, token storage, tool discovery, schema cache.
- Namespaced tool registry, enablement snapshots, tool search.
- Permission/approval model, argument-scoped policy, egress policy.
- Bundled skills + user/org skills.
- Plugin marketplace, install manifests, org bundle loading.
- Prompt-injection and tool-output data handling.

Tests: `TEST-MCP-001..010`, `TEST-AI-019/020`.

Exit: an unavailable MCP server cannot block app boot; tool calls are exact and auditable; a denied
tool never reaches the provider; tool-result images survive.

### Phase 6 — Cowork, Code, Projects, Artifacts, Scheduled

Build:

- Cowork home/task/transcript/composer.
- Code dashboard/session, folder/worktree selection, permission modes, Fast mode, terminal.
- Projects empty/create/detail (detail requires a design pass if still unknown).
- Artifacts library/new/live task/viewer (viewer if platform gate permits; otherwise explicit
  unavailable).
- Scheduled tasks/templates/editor/runs/keep-awake.
- Search/filter.

Tests: `TEST-UX-*`, `TEST-SCHED-*`, `TEST-ART-*`, `TEST-CODE-*`.

Exit: the observed empty/create/navigation states are reproduced; every create action either opens a
form or explicitly starts a task; no accidental generation.

### Phase 7 — Memory, usage, privacy, updates

Build:

- Memory store/index, global toggle, management UI, baseline hash.
- Usage page 7/30/90 and chart; cache/search accounting.
- Privacy disclosure and diagnostic bundle.
- Sentry/OTel controls, redaction, no content capture.
- Update feed, enforcement window, relaunch, rollback, signature verification.
- Retention/legal hold/tombstone GC.

Tests: `TEST-PRIV-001..004`, `TEST-UPDATE-001..002`, `TEST-DATA-009..010`.

Exit: legal hold prevents deletion; diagnostic bundle enumerates and redacts; update failure leaves
the old version runnable.

### Phase 8 — Extension SDKs and additional providers

Build the adapter framework and at least one reference extension beyond the shipped six:

- OpenAI Responses.
- Google Gemini.
- MiniMax native.
- A local provider (Ollama or llama.cpp).

Each is a signed adapter package with a manifest, native SDK, conformance suite, and setup schema.
No shared translation layer. A provider that only speaks OpenAI Chat Completions is supported by an
explicit adapter for that native protocol, not by converting Anthropic requests.

Tests: `TEST-EXT-001..008`.

Exit: adding a provider requires no changes to the agent or UI beyond registration; a broken adapter
cannot prevent app boot; no extension can bypass model policy or credential isolation.

### Phase 9 — Packaging and hardening

- Reproducible builds, signing/notarization, fuses, ASAR integrity.
- Crash/restart, migration, backup/restore.
- Performance budgets and stall detection.
- Accessibility audit.
- Threat-model review and penetration tests.
- Documentation freeze + traceability audit.

Exit: signed installers for Linux/macOS/Windows (platform tests), clean install/upgrade/rollback,
no P0 security findings, all P0/P1 traceability rows green.

## 5. Feature preservation vs improvement

### Must preserve

- Local SPA routes and navigation model.
- Separate setup window.
- Provider-native credentials and Anthropic-compatible Gateway contract.
- Claude-only model policy for Claude provider families.
- Composer and model-picker behavior, including “needs API key.”
- Task/session restore, archive/delete, usage, retention semantics.
- MCP policy/approval and egress behavior.
- Empty/loading/error copy and destructive-action safeguards.
- No silent model substitution.
- Light/dark/system themes and design tokens.

### Improvements (separate register: [19](19-improvement-register.md))

- SQLite instead of many JSON files, with atomic writes/migrations.
- Keychain-only secrets.
- Explicit capability registry.
- Native adapters for non-Anthropic providers instead of a translation shim.
- Better sender validation and a smaller bridge API.
- Artifact sandbox with a dedicated origin and no shared partition.
- Signed adapter plugins, conformance tests, and update rollback.
- More complete accessibility semantics and focus restoration.
- Structured redaction at log-write time.
- Model availability UI that never silently swaps.

Every improvement must be tested and documented as an improvement, not presented as original.

## 6. Acceptance test plan

### 6.1 Test layers

| Layer | Tool | Scope |
|---|---|---|
| Unit | Vitest | schemas, policy, token/usage math, redaction, path validation |
| Wire conformance | mock HTTP/SSE servers + SDK fixtures | native provider request/response shapes |
| Integration | Vitest + temp user-data | config apply, DB migrations, MCP lifecycle, retention |
| E2E | Playwright Electron | routes, composer, setup, settings, menus, permissions |
| Security | targeted tests + static checks | origin/frame checks, traversal, secret leakage, fetch patch ban |
| Accessibility | axe + manual keyboard/screen-reader | dialogs, menus, composer, model picker |
| Performance | startup/stream/stall budgets | cold start, first token, long transcript |

### 6.2 Acceptance IDs

#### Shell and protocol

| ID | Criterion |
|---|---|
| `TEST-ARCH-001` | Renderer has no Node integration; only enumerated bridge APIs exist |
| `TEST-ARCH-002` | Second instance/deep link routes to primary; invalid deep links are rejected |
| `TEST-ARCH-003` | `app://` rejects non-localhost host, bad origin, and traversal |
| `TEST-ARCH-004` | Unknown API path returns typed 503, not a network call |
| `TEST-ARCH-005` | CSP is generated per config and blocks unapproved origins |
| `TEST-ARCH-006` | Artifact preview is isolated from the main bridge/origin |
| `TEST-ARCH-007` | Setup window opens when provider boot fails |
| `TEST-ARCH-008` | Quit drains agents/PTYs and unmounts sandbox resources |
| `TEST-ARCH-009` | Native menu accelerators match the documented table |
| `TEST-ARCH-010` | No global fetch monkey-patch or hidden translation entry point exists |

#### Configuration/setup

| ID | Criterion |
|---|---|
| `TEST-CFG-001` | Profile apply is atomic; invalid config leaves old profile intact |
| `TEST-CFG-002` | Draft discard restores applied values exactly |
| `TEST-CFG-003` | Bootstrap override is read-only and visibly labeled |
| `TEST-CFG-004` | All six providers validate required fields and model IDs |
| `TEST-CFG-005` | Credential kind is exclusive; helper failure does not fall back |
| `TEST-CFG-006` | Secrets are keychain-backed and absent from renderer storage/logs |
| `TEST-CFG-007` | Export/support bundle warns and redacts by default |
| `TEST-CFG-008` | Egress derivation excludes private hosts from published docs/fixtures |
| `TEST-CFG-009` | Retention/legal-hold controls persist and enforce |
| `TEST-CFG-010` | Managed config recheck/relaunch windows behave as configured |
| `TEST-CFG-011` | Model discovery 3-page/10 s/partial behavior matches contract |
| `TEST-CFG-012` | Catalog signature/expiry/floor failure uses bundled fallback |

#### AI/provider

Use `TEST-AI-001..024` from [12-ai-architecture.md](12-ai-architecture.md).

#### Sessions/data

| ID | Criterion |
|---|---|
| `TEST-SESSION-001` | Create, send, stream, stop, reopen, archive, delete |
| `TEST-SESSION-002` | Restart restores metadata without duplicate live process |
| `TEST-SESSION-003` | Corrupt/partial session is quarantined, not silently deleted |
| `TEST-SESSION-004` | Delete removes metadata/transcript/uploads/outputs/preview cache |
| `TEST-SESSION-005` | Retention does not delete projects/memory/uncommitted work |
| `TEST-SESSION-006` | Audit HMAC verification detects tampering |
| `TEST-SESSION-007` | Usage event preserves input/output/cache/search/error |
| `TEST-SESSION-008` | 7/30/90-day aggregation is correct across time zones |
| `TEST-SESSION-009` | Draft survives recoverable error and navigation |
| `TEST-SESSION-010` | Code worktree state survives session metadata deletion |

#### MCP/tools

| ID | Criterion |
|---|---|
| `TEST-MCP-001` | stdio/HTTP/SSE servers start, fail, reconnect, and shut down |
| `TEST-MCP-002` | Tool namespacing and enablement snapshots are stable |
| `TEST-MCP-003` | Approval modes per-call/session/persistent work; lifetime expiry works |
| `TEST-MCP-004` | Denied tools never reach provider requests |
| `TEST-MCP-005` | Argument-scoped policy is enforced |
| `TEST-MCP-006` | MCP OAuth token refresh is isolated per server |
| `TEST-MCP-007` | Egress allowlist blocks unapproved hosts |
| `TEST-MCP-008` | Tool output is treated as data; prompt injection does not execute actions |
| `TEST-MCP-009` | MCP failure is a non-blocking, actionable toast |
| `TEST-MCP-010` | Tool timeout cancels and reports |

#### UX

| ID | Criterion |
|---|---|
| `TEST-UX-001` | New task → composer → task route; model/folder/attachments visible |
| `TEST-UX-002` | Model picker shows provider/model/needs-key and never auto-selects another |
| `TEST-UX-003` | Search/filter keyboard navigation and empty results |
| `TEST-UX-004` | Projects empty/create/dismiss/validation |
| `TEST-UX-005` | Artifacts new action starts a task only after explicit click |
| `TEST-UX-006` | Scheduled templates and empty state |
| `TEST-UX-007` | Customize Skills/Connectors/Plugins filters and status |
| `TEST-UX-008` | Settings sections/hash/close/theme/font/motion |
| `TEST-UX-009` | Setup tabs/provider switch/apply/discard |
| `TEST-UX-010` | Language dialog changes locale and persists |
| `TEST-UX-011` | Destructive actions require confirmation/undo |
| `TEST-UX-012` | Streaming/stop/error/empty states are announced accessibly |
| `TEST-UX-013` | Long model names/menus fit and scroll without overflow |
| `TEST-UX-014` | Light/dark/system visual regression within token tolerances |

#### Privacy/updates/security

| ID | Criterion |
|---|---|
| `TEST-PRIV-001` | Prompts/completions never appear in telemetry |
| `TEST-PRIV-002` | Essential/nonessential controls are independent and enforced |
| `TEST-PRIV-003` | Diagnostic upload requires explicit consent |
| `TEST-PRIV-004` | Logs redact credentials/headers/query tokens |
| `TEST-UPDATE-001` | Update feed/signature verification and rollback |
| `TEST-UPDATE-002` | Enforcement window/relaunch guard work |
| `TEST-SEC-001` | Renderer XSS cannot call bridge APIs from a non-app origin |
| `TEST-SEC-002` | Symlink/canonicalization prevents reads outside granted folders |
| `TEST-SEC-003` | Deep links cannot open arbitrary local files/commands |
| `TEST-SEC-004` | Custom headers reject credential-shaped values |

## 7. Test fixtures

- **Provider fixtures:** recorded/synthetic request and SSE streams for each native SDK; no real
  credentials. Assert exact request JSON/headers and event mapping.
- **MCP fixtures:** stdio echo server, HTTP server with OAuth, failing/slow server, malicious tool
  output, image tool result.
- **Session fixtures:** synthetic transcripts with text, image, PDF, tool calls, thinking, errors,
  cancellation, deleted/tombstoned records.
- **Catalog fixtures:** valid signed catalog, expired signature, bad floor, malformed model.
- **UI fixtures:** empty/populated/long/error/streaming, light/dark, reduced motion, OpenDyslexic.
- **Security fixtures:** traversal paths, symlink escape, prototype pollution in tool args, oversized
  payloads, HTML/JS artifact content.

No fixture may be copied from the inspected user’s real sessions or memory.

## 8. Performance and reliability targets

These are **rebuild targets**, not shipped guarantees. Observed startup on this machine was ~2.9 s
to input-ready; use that as a baseline, not a contract.

| Metric | Target |
|---|---|
| Cold start to interactive shell | ≤ 3 s on a warm SSD, 1080p |
| Route transition | ≤ 150 ms |
| First streamed token | network-bound; show a visible “thinking” state immediately |
| Long transcript scroll | 60 fps at 10k messages with virtualization |
| MCP failure to actionable toast | ≤ 2 s |
| Config apply | atomic; rollback on failure |
| Crash restart | no duplicate sessions; unsent draft preserved |
| Memory | no full-session 1.5 MB JSON load on boot; lazy-load transcripts |

## 9. Risks and mitigations

| Risk | Mitigation |
|---|---|
| Model-provider drift | Signed catalog + provider discovery + capability tests |
| SDK breaking changes | Pin versions and run wire conformance in CI |
| Prompt injection via MCP/tool output | Treat as data, enforce tool policy, no auto-execution |
| Secret leakage in logs/exports | Write-time redaction + keychain + test fixtures |
| Artifact XSS | Dedicated origin/partition, strict CSP, no bridge |
| Provider outage/quota | Native error mapping, visible retry, no fallback |
| Migration data loss | Backups, WAL, tombstones, dry-run migration |
| Platform divergence | Separate platform capability matrix; do not fake unavailable features |
| Catalog compromise | Signature + floor + expiry + bundled fallback |
| Extension supply chain | Signed adapter packages + manifest + review |

## 10. Definition of done

A feature is done when:

1. Its behavior matches [03](03-feature-inventory.md)/screen specs or an approved improvement.
2. Its module has unit + integration coverage.
3. Its provider/MCP path has a native-wire fixture.
4. It has loading/empty/error/permission states.
5. It passes accessibility checks.
6. It has no secrets/personal data in fixtures, logs, docs, or telemetry.
7. Its traceability row is complete in [17](17-traceability.md).
8. An improvement, if any, is recorded in [19](19-improvement-register.md).
