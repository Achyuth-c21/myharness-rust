# 18 — API and Interface Contracts

This document defines the contracts a rebuild must implement between renderer, preload, main process,
agent, providers, MCP, and storage. It separates **verified shipped behavior** from **rebuild design**.

Verified sources: `mainView.js` preload surface, the pristine main bundle, live IPC/DOM evidence, and
the runtime inventory. Payload schemas below are **rebuild designs** unless explicitly marked verified;
the shipped code is minified and most payloads were not exhaustively observed.

## 1. Contract principles

1. **One owner per boundary.** Renderer owns UI state; main owns credentials, processes, files,
   network, sessions, and policy.
2. **No generic provider router.** Each adapter speaks its provider’s native SDK/protocol.
3. **No silent substitution.** A request names one provider/model; failure is surfaced, not retried on
   a different model/provider.
4. **Versioned messages.** IPC and persisted records carry a schema version; unknown versions fail
   closed with a recoverable error.
5. **Least privilege.** Preload exposes only allowlisted methods; main validates the sender again.
6. **Data is not instruction.** Transcript/tool/MCP/artifact content never becomes executable policy.
7. **Secrets never cross into renderer state**, logs, telemetry, exports, or docs.

## 2. Process and trust boundaries

```text
┌ renderer (sandboxed, contextIsolation) ──────────────┐
│ React SPA + local UI state                           │
│           │ window.claude.* (typed bridge)           │
└───────────┼──────────────────────────────────────────┘
            │ validated IPC (invoke/event)
┌───────────▼──────────── main process ────────────────┐
│ config/policy · sessions · agent · providers · MCP    │
│ files/pty · scheduler · artifacts · telemetry/update  │
└───────┬───────────────┬───────────────┬──────────────┘
        │ native SDK     │ MCP transport │ local files/PTY
   provider APIs     MCP servers       workspace/VM
```

- Renderer has no Node integration (`sandbox: true`, `contextIsolation: true`,
  `nodeIntegration: false`, `webviewTag: false`).
- `app://localhost` is privileged. `app://` requests require `Origin: app://localhost` for
  non-navigation calls and reject non-localhost hosts/traversal.
- Artifact/document/simulator previews use separate schemes/origins and must not receive the main
  preload bridge.
- The separate `/setup-desktop-3p` window is still privileged; it uses the same origin checks.

## 3. IPC contract

### 3.1 Channel shape (verified)

```text
$eipc_message$_<random-build-token>_$_<Namespace>_$_<method>
```

The token rotates per build. Do not hard-code it. Generate a typed allowlist at build time.

### 3.2 Invocation and events

```ts
type IpcResult<T> =
  | { ok: true; value: T }
  | { ok: false; error: AppError };

interface BridgeMethod<Req, Res> {
  (req: Req): Promise<Res>;
}

interface BridgeEvent<T> {
  (listener: (event: T) => void): () => void; // returns unsubscribe
}
```

The shipped preload methods mostly return the value directly and reject on error. The rebuild may use
a typed result envelope, but must preserve atomicity and reject unknown/invalid calls. Event
subscriptions must return an unsubscribe function and must not leak listeners on window close.

### 3.3 Validation

- Preload: top-frame check, origin allowlist, method allowlist, argument shape check.
- Main: **re-check** `event.senderFrame`/origin and ownership; preload gating is not a security boundary.
- Reject unknown channels/methods and extra fields.
- Cap payload size; reject prototype-pollution keys (`__proto__`, `constructor`, `prototype`).
- Never accept a raw filesystem path, URL, command, or header from the renderer without policy checks.
- Redact errors before returning; include a stable code and user-safe message, not a stack/credential.

### 3.4 Namespace inventory

The live build exposes 17 `claude.settings` namespaces (145 methods), 60 `claude.web` namespaces
(1,017 methods; `LocalSessions` alone has 339), six top-level integration namespaces, and five app
bindings. The machine-readable list is
`evidence/runtime-api-surface.json`. The rebuild should implement a smaller set; the groups below are
the minimum contract.

| Namespace | Minimum methods / responsibility |
|---|---|
| `settings.AppConfig` | get/set app config, feature switches, DXT/node flags |
| `settings.Startup` | startup-on-login, menu bar state |
| `settings.GlobalShortcut` | get/set/clear global shortcut |
| `settings.MCP` | list/config/status, reveal logs/config, config-change events |
| `settings.Extensions` | install/uninstall/list/settings/signature/directory |
| `settings.FilePickers` | directory/file picker |
| `settings.AppPreferences` | get/set preferences + change event |
| `settings.AppFeatures` | supported features + change event |
| `settings.DesktopInfo` | system info, logs, default browser |
| `settings.Custom3pSetup` | profiles, config read/write/apply, health, probes, relaunch, auth |
| `settings.Custom3pHelperRun` | model discovery, inference probe, credential helper |
| `settings.ClaudeAiImport` | import auth/export/merge (policy-disabled here) |
| `settings.SupportBundle` | support-bundle state/action |
| `settings.WakeScheduler` | keep-awake status/settings |
| `settings.Custom3pUsage` | local usage summary |
| `web.WindowControl` | theme, close/focus/show, screenshot, split/modal state |
| `web.Navigation` / `BrowserNavigation` | navigate, back/forward, state |
| `web.MenuEvents` | menu commands/accelerators/context menus |
| `web.WindowState` | focus, fullscreen, visibility, zoom |
| `web.DeviceStorage` | usage, cleanup preview/run, cache clear |
| `web.Auth` | browser auth, signed-in preparation |
| `web.DesktopNotifications` | permission, show, actions |
| `web.AutoUpdater` | check, state, restart/guard |
| `web.CoworkArtifacts` | list/metadata/html/versions/star/share/export/capture |
| `web.LocalAgentModeSessions` | Cowork sessions, transcript, send/stop, permission, tools, folders, memory |
| `web.LocalSessions` | Code sessions, PTY, git/PR, trust, transcript |
| `web.CoworkMemory` / `CoworkSpaces` | memory and spaces |
| `web.CoworkScheduledTasks` | CRUD, events, watcher history, permissions |
| `web.CustomPlugins` / `LocalPlugins` / `PluginBridgeMcp` | marketplaces, installs, OAuth, tool bridge |
| `web.CoworkFilePreview` / `Launch` | file/preview/browser surfaces |
| `web.CoworkStorage` | counts, export, cleanup |
| `web.SupportBundle` | diagnostics |
| `claude.skills` | skill management |
| `claude.hybrid` | profile/org transition |
| `claude.buddy`, `claude.officeAddin`, `claude.simulator`, `claude.engineSurface` | platform/integration gates |

## 4. Session and agent contracts

### 4.1 Session record (rebuild design; fields verified from metadata)

```ts
interface SessionRecord {
  schemaVersion: 1;
  id: string;                     // local_<uuid> or cli session id
  surface: 'cowork' | 'chat' | 'code' | 'artifact';
  title: string;
  createdAt: string;
  updatedAt: string;
  model: ModelKey;
  effort?: 'low'|'medium'|'high'|'xhigh'|'max';
  cwd?: string;                   // canonical, policy-checked
  originCwd?: string;
  worktree?: { path: string; branch?: string; base?: string };
  permissionMode: 'default'|'acceptEdits'|'plan'|'bypass';
  fastMode?: boolean;
  enabledMcpTools: string[];      // immutable snapshot per turn
  folderGrants?: string[];
  projectId?: string;
  incognito?: boolean;
  parentId?: string;              // fork/side session
  status: 'idle'|'running'|'waiting'|'stopped'|'error'|'archived';
  stopReason?: string;
}
```

Personal prompt/memory snapshots may exist in session metadata in the shipped app; a rebuild should
store them in the protected transcript store, not in list/index files.

### 4.2 Turn request

```ts
interface TurnRequest {
  sessionId: string;
  turnId: string;
  providerId: string;
  modelId: string;
  messages: NativeMessage[];      // adapter-native
  system?: ContentPart[];
  tools: ToolDefinition[];
  toolChoice?: 'auto'|'none'|'required'|{ name: string };
  thinking?: { enabled: boolean; budget?: number; effort?: string };
  cache?: { strategy: 'none'|'automatic'|'explicit'; breakpoints?: number[] };
  maxOutputTokens?: number;
  metadata?: { requestId: string; userIdHash?: string };
  signal: AbortSignal;
}
```

The renderer never supplies credentials or arbitrary endpoint URLs. Main resolves the active profile,
checks policy/capabilities, and passes a provider-native request to exactly one adapter.

### 4.3 Stream events

```ts
type TurnEvent =
  | { type:'start'; turnId:string }
  | { type:'text_delta'; text:string }
  | { type:'reasoning_delta'; text:string; signature?:string }
  | { type:'tool_call'; id:string; name:string; argumentsDelta?:string; arguments?:unknown }
  | { type:'tool_result'; id:string; parts:ContentPart[] }
  | { type:'usage'; input:number; output:number; cacheRead?:number; cacheWrite?:number; webSearch?:number }
  | { type:'stop'; reason:string }
  | { type:'error'; error:ProviderError }
  | { type:'done' };
```

Rules:

- Events are ordered; `done` is emitted exactly once after `stop`/`error`/cancel.
- Deltas may be coalesced by the renderer, but persistence records the final content.
- Tool-call arguments are accumulated and validated before execution.
- A provider error maps to a stable code; raw provider bodies are redacted and capped.
- Cancellation aborts the provider stream and marks partial output `stopped`.
- Keep-alive-only streams time out at the configured idle timeout.

### 4.4 Permission request

```ts
interface ToolPermissionRequest {
  id: string;
  sessionId: string;
  turnId: string;
  toolName: string;
  displayName: string;
  argumentsPreview: unknown;      // redacted/truncated
  scope: 'once'|'session'|'persistent';
  policyDecision?: 'allow'|'ask'|'deny';
  expiresAt?: string;
}
```

Response: `allow`/`deny`, optional `always` and scope. Denied tools never reach the provider as
executed results; the model receives a denial result only if policy permits. Persistent approvals
expire per `mcpScheduledTaskApprovalLifetimeDays` or the session policy.

> **Rebuild deviation (Phase 5).** The rebuild's `chat://tool-approval` event (below) is a
> rebuild-defined shape, and a policy-denied call is always returned to the model as a tool error
> so the transcript remains grammatically valid. The shipped denial-result gate was not observed
> (`docs/17` §4.16, `docs/14` §9 item 44).

### 4.5 Rebuild chat bridge (Phase 4 + Phase 5 subset)

This is the **rebuild's** transport for the chat half of
`web.LocalAgentModeSessions` (`crates/bridge/src/chat.rs`; `docs/17` §4.15–4.16). It is a
deliberately smaller contract than the shipped namespace — folders, memory, and attachments are
not part of it — and the names below are rebuild names, not captured wire names.

One Tauri command, `chat_command`, carries a tagged `ChatCommand` (camelCase tags and fields).
The Phase 4 commands are:

| Command | Fields | Response |
|---|---|---|
| `createSession` | `title?` | `session` (row + empty transcript) |
| `listSessions` | `archived?` (absent = both) | `sessions` (Cowork/chat only; Code uses `listCodeSessions`) |
| `openSession` | `id` | `session` (recovers a crashed live turn first) |
| `sendTurn` | `sessionId`, `text`, `maxTokens?` | `turn` or `toolTurn` (user + assistant/tool rows + usage + stop reason; `toolTurn` also carries `warnings`) |
| `stopTurn` | `sessionId` | `stopped` (idempotent) |
| `archiveSession` | `id`, `archived` | `ack` |
| `renameSession` | `id`, `title` | `ack` |
| `deleteSession` | `id` | `ack` (honors legal hold; clears the frozen snapshot and session grants) |
| `usageSummary` | `days` (1–366), `utcOffsetMinutes?` | `usage` (since Phase 7 the response also carries `surfaces`: the Cowork/Code shares of the same window, each folding only its own surface's ledger rows; a surface with no rows is a zero summary, never a missing field — `docs/14` §9 item 68) |

The Phase 5 additions are:

| Command | Fields | Response |
|---|---|---|
| `mcpStatus` | — | `mcpStatus` (redacted servers + warnings) |
| `mcpTools` | — | `mcpTools` (live executable tools for the session picker) |
| `mcpSearchTools` | `query`, `limit?` | `toolSearch` (live/cached hits; cached hits are metadata only) |
| `setSessionTools` | `sessionId`, `tools` | `ack` (validates live names, persists `mcpTools`, installs the frozen snapshot) |
| `respondToolApproval` | `approvalId`, `decision` (`allow`/`deny`), `scope?` (`once`/`session`/`persistent`) | `ack` |
| `retryMcpServer` | `server` | `mcpStatus` |
| `forgetMcpOAuth` | `server` | `ack` |
| `listSkills` | — | `skills` |
| `selectSkill` | `name` | `skillSelection` (instructions only; no execution, no approval grant) |
| `setSkillEnabled` | `name`, `enabled` | `skills` |
| `setSkillCreationEnabled` | `enabled` | `skills` |
| `listPlugins` | — | `plugins` |
| `installPlugin` | `path` (absolute local directory) | `pluginInstalled` |
| `uninstallPlugin` | `name` | `plugins` |
| `setPluginEnabled` | `name`, `enabled` | `plugins` (server enablement applies on next app start) |
| `setPluginSignaturePolicy` | `policy` (`optional`/`required`) | `plugins` |

A failure is `ChatResponse::Error { error, userTurnRecorded }`, where `error` is the
`docs/18` §11 envelope (`code`, `message`, `retryable`) and `userTurnRecorded` says whether
the user's turn reached the store. A provider failure carries the provider's own message
verbatim; a pre-persistence failure is `userTurnRecorded: false` and leaves the renderer's
draft intact. A command that needs an unavailable runtime returns `UNSUPPORTED` rather than
pretending the feature exists.

Content deltas stream on the rebuild-only event `chat://turn-event`:

```ts
interface ChatTurnEvent {
  sessionId: string;
  event: TurnEvent; // docs/18 §4.3, tagged by `type` in snake_case
}
```

Only content events (text, reasoning, citations, tool calls) are emitted live; usage, the
native stop reason, and the terminal error are carried by the command response. A turn that
executed at least one tool returns `toolTurn` instead of `turn`, with every assistant/tool row
in order and non-fatal audit/persistence `warnings`. Tool results also arrive as
`TurnEvent::ToolResult`.

Approvals arrive on `chat://tool-approval`:

```ts
interface ToolApprovalEvent {
  id: string;
  sessionId: string;
  callId: string;
  toolName: string;       // internal `<server>:<tool>`
  displayName: string;
  argumentsPreview: unknown; // redacted and bounded
  allowedScopes: ("once" | "session" | "persistent")[];
}
```

Background MCP boot/retry state arrives on `chat://mcp-status` with redacted
`servers` and `warnings`. A browser preview has no Tauri host, so the renderer reports
`BRIDGE_UNAVAILABLE` instead of inventing a transcript, session row, tool, or total.

Rules enforced at this seam: the renderer is untrusted (UUID ids, bounded title/text/tool
names, timezone clamped to ±840 minutes); credentials, server env/headers, OAuth tokens, and
raw tool arguments never cross back; the session's provider/model is fixed and a profile that
now serves another target refuses the send (`MODEL_UNAVAILABLE`, no substitution); a
tool-enabled send first capability-gates the model and fails `MODEL_UNSUPPORTED` rather than
sending tools to a model that did not advertise them; missing selected tools fail before the
selection is stored or sent; changing tools during a running turn is `TURN_IN_PROGRESS`; stop
is idempotent; archive/delete refuse while a turn runs; recovery on open skips a turn this
process is running. Cowork sends no system prompt in this phase (the shipped bootstrap prompt
was not captured); Code adds only the workspace-folder block described in §4.6. No retry,
model picker, attachment, or general system-prompt assembly exists yet (`docs/14` §9 items
33–40).

### 4.6 Rebuild Code bridge (Phase 6 Slice 1)

Code uses the same `chat_command` transport and the same `chat://turn-event` stream as Cowork,
but a separate, strict command set so a Code route can never open or send into a Cowork session
(and vice versa). Names below are rebuild names (`crates/bridge/src/chat.rs`); the shipped Code
namespace was not fully captured.

| Command | Fields | Response |
|---|---|---|
| `listCodeSessions` | `archived?` | `sessions` (only rows with `surface: "code"`) |
| `createCodeSession` | `title?`, `cwd`, `permissionMode` (`default`/`acceptEdits`/`plan`/`bypass`) | `session` (row + empty transcript) |
| `openCodeSession` | `id` | `session` (refuses a Cowork id with `SESSION_SURFACE_MISMATCH`) |
| `sendCodeTurn` | `sessionId`, `text`, `maxTokens?` | `turn` or `toolTurn`; refuses a Cowork id, a missing/changed folder, or a disabled mode before contacting the provider |
| `codeDashboard` | `range?` (`7d`/`30d`/`all`), `utcOffsetMinutes?` | `codeDashboard` |

`createCodeSession` validates and canonicalizes `cwd` first, then resolves the applied profile
(no session is created without a model), and stores the canonical `cwd`, the resolved `model`,
the stored permission spelling, and
`extra.providerId`/`profileId`/`placement: "local"`/`originCwd`. `cwd` is absolute, bounded,
control-free, existing, a directory, canonicalized, and checked against configured allowed roots;
a symlink cannot escape an allowed root. The renderer never supplies `surface`, `model`, or
`providerId`.

`sendCodeTurn` revalidates the stored canonical folder and the current `CodePolicy` on every
send. A deleted/renamed folder is `FOLDER_UNAVAILABLE`; a path that no longer canonicalizes to the
stored spelling is `CODE_FOLDER_CHANGED`; a policy-disabled Bypass or malformed stored mode is
refused. The canonical folder is also added to the model's system context. That is not a
filesystem sandbox: this slice has no built-in file tools, and an MCP server's process cwd is its
own configuration. Plan mode filters every tool from the provider request; when tools are
selected it uses an empty tool snapshot instead of resolving the MCP servers, so an offline
server cannot block a Plan turn, and a stale/hallucinated call is denied by the agent gate.
Accept-edits auto-approves only a host-declared edit-tool set (empty in this slice); Bypass
auto-approves only policy `Ask` decisions and never a `Deny`.

`codeDashboard` returns:

```ts
interface CodeDashboardResponse {
  summary: {
    range: "7d" | "30d" | "all";
    days: number;
    truncated: boolean;          // daily strip cap (3,660 days); totals stay all-time
    timezoneOffsetMinutes: number;
    from: string;
    to: string;
    totals: UsageTotals;         // input/output/cache/search/errors/events
    daily: DailyUsage[];
    messages: number;            // Code user+assistant turns in range
    sessions: number;            // distinct Code sessions with usage events
    activeDays: number;
    peakHour: number | null;     // device-local hour
    favoriteModel: string | null;
    models: { model: string; totals: UsageTotals; sessions: number; sharePercent: number }[];
  };
  sessions: SessionRecord[];     // recent non-archived Code rows
  repositories: { cwd: string; sessionCount: number; lastActivityAt: number }[];
  policy: { codeEnabled: boolean; localSessionsEnabled: boolean; bypassAllowed: boolean; workspaceRestricted: boolean };
}
```

`policy.bypassAllowed` is true only when `capabilities/disableBypassPermissionsMode` is
explicitly `false`; the renderer must keep Bypass visible but disabled otherwise. The response
never includes allowed folder roots, credentials, or provider secrets.

The native folder picker is a separate Tauri command:

```ts
pick_workspace_folder(): Promise<string | null>
```

`null` means the user cancelled; a UTF-8/path failure rejects with the standard `BridgeError`.
`rfd` does not expose a platform-picker failure separately from cancellation, so a failed portal
is also observed as `null`; the bridge still revalidates any path. A browser preview has no Tauri
host and reports `BRIDGE_UNAVAILABLE` instead of inventing a folder. The chosen path is still
untrusted and is revalidated by `createCodeSession`.

### 4.7 Rebuild Projects/Artifacts/Scheduled/Search bridge (Phase 6 slices 2–5)

All commands ride the same `chat_command` transport. Names below are rebuild
names (`crates/bridge/src/chat.rs`, `crates/bridge/src/scheduler.rs`).

Projects:

| Command | Fields | Response |
|---|---|---|
| `listProjects` | — | `projects` (rows + bridge-computed `folderAvailable`) |
| `createProject` | `name`, `instructions?`, `cwd`, `memoryEnabled?` | `project`; the folder is canonicalized against the same workspace policy as Code before storage |
| `openProject` | `id`, `archived?` | `projectDetail` (project + its sessions, both surfaces); a deleted id is `PROJECT_MISSING` |
| `updateProject` | `id`, `name`, `instructions?`, `memoryEnabled?` | `project` (the folder is not editable) |
| `deleteProject` | `id` | `projectDeleted` (`orphanedSessions` counts sessions still referencing the id) |

`createSession` gained optional `projectId`/`cwd`: the bridge resolves the
folder (never the renderer), refuses a project whose folder no longer
resolves (`FOLDER_UNAVAILABLE`), and stores `project_id` + `cwd` on the
session. A folder-bound Cowork turn carries the same quoted path-as-data
system block Code uses. A turn in a project bumps the project's activity
stamp (best-effort; a failure is a warning on tool turns only).

The Phase 7 additions so far (memory, `docs/11` §5; `docs/14` §9 items 66–67):

| Command | Fields | Response |
|---|---|---|
| `memoryOverview` | — | `memoryOverview` (`policy` effective state, `baselineHash?`, `baselineChanged`, `count`); always answers so the panel can explain a disabled state |
| `listMemoryEntries` | — | `memoryEntries` |
| `createMemoryEntry` | `content` | `memoryEntry` (bounds refused, never truncated) |
| `updateMemoryEntry` | `id`, `content` | `memoryEntry`; a missing id is `MEMORY_MISSING` |
| `deleteMemoryEntry` | `id` | `ack` |
| `resetMemory` | — | `memoryReset` (`deleted` count); explicit and audited |

Every memory command except `memoryOverview` is gated by the effective
`memoryEnabled` capability read fail-closed from the merged profile
(disabled → the typed refusal, never a silent no-op). Error codes:
`MEMORY_MISSING`, `MEMORY_CORRUPT` (a corrupt file is renamed aside and
named in the audit trail, `docs/14` §9 item 66), `CONFIG_IO`,
`CONFIG_VALIDATION`, `UNSUPPORTED` (broken memory directory).

The Phase 7 diagnostics commands (`docs/features/updates-telemetry-privacy.md`
§7; `docs/14` §9):

| Command | Fields | Response |
|---|---|---|
| `diagnosticsPreview` | — | `diagnosticsPreview` (`entries`: the manifest of exactly what a bundle would include, including the never-included list); writes nothing |
| `diagnosticsBuild` | `consent` | `diagnosticsBundle` (`file`, `entries`); `consent: false` (or absent) is the typed `CONSENT_REQUIRED` refusal and writes nothing |

The bundle is a redacted markdown document under the app-managed
`diagnostics/` directory: version/platform/time, the effective configuration
through the export redaction and then the free-text redaction, local counts
(sessions by surface, turns, usage events, memory entries), and the effective
telemetry policy. **There is no upload in this build** — the bundle stays on
the device; the action is appended to `diagnostics/log.jsonl` with no
contents (bounded at 2 MiB). A diagnostics command without an attached
directory answers `UNSUPPORTED` — diagnostics degrade, never chat.

Retention (Phase 7, `TEST-CFG-009`) has no bridge command: the sweeps run at
boot through the Phase-4 engine, governed by the merged limits section —
`coworkSessionRetentionDays` → the `chat` surface, `codeSessionRetentionDays`
→ `code`, `sessionRetentionHold` suspends everything, and per-session legal
holds are honored throughout. Nothing deletes unless a profile explicitly
configures days (`docs/14` §9 item 69).

Artifacts:

| Command | Fields | Response |
|---|---|---|
| `createArtifactTask` | — | `session` (a normal Cowork session with `extra.artifactTask: true`; no turn is generated on the task's behalf) |

The library filters `listSessions` rows on that marker; there is no artifact
content model, so nothing in the response can claim a completed artifact.

Scheduled tasks:

| Command | Fields | Response |
|---|---|---|
| `listScheduledTasks` | — | `scheduledTasks` (rows + `scheduleSummary` + `lastRun` + `contextUnavailable`) |
| `scheduledTemplates` | — | `templates` (the six captured cards; name/schedule label are the captured copy, prompts are rebuild prefill text) |
| `createScheduledTask` | `name`, `prompt`, `cron`, `projectId?`/`cwd?` (mutually exclusive), `enabled?` | `scheduledTask`; an enabled task arms immediately |
| `updateScheduledTask` | `id`, `name`, `prompt`, `cron` | `scheduledTask` (rearms if enabled) |
| `setScheduledTaskEnabled` | `id`, `enabled` | `scheduledTask` (disable disarms; enable rearms) |
| `deleteScheduledTask` | `id` | `ack` (run rows go with the task; run-created sessions stay) |
| `runScheduledTaskNow` | `id` | `scheduledRun` (the terminal run row; the run used the scheduled approval context) |
| `previewSchedule` | `cron`, `count?` (1–10) | `schedulePreview` (`times`: epoch millis in the device zone) |

`cron` is the 5-field subset: `*`, values, comma lists, `a-b` ranges, and
`*/step` on minute/hour; day-of-month and month must be `*`
(`CONFIG_VALIDATION` otherwise). The shell ticks every 30 s: due armed
occurrences run on the interactive send path (`scheduled=true`, so only the
scheduled approval lifetime applies); an occurrence missed past the 60 s
grace is recorded as `skipped` with the reason and never executed late. A
run creates a normal Cowork session carrying
`extra.scheduledTaskId`/`scheduledRunId`.

Search/filter is renderer-local: the palette combines `listSessions`,
`listCodeSessions`, and `listProjects` (all existing commands); the filter
menu composes client-side over the same rows. No new search command exists,
and no query leaves the device.

Two shell commands accompany the slices: `reveal_workspace_folder(path)`
(project detail's "Open folder") refuses non-absolute, non-existent,
non-canonical paths before opening the OS file manager, and
`pick_workspace_folder` is shared by Code and Projects.

## 5. HTTP contracts inside `app://`

### 5.1 Static server

- Serves `resources/ion-dist`, SPA fallback to `index.html`, canonical path checks.
- Rejects non-`localhost` host and traversal; only `app://localhost` origin for API calls.
- Injects config-derived CSP and `__DESKTOP_3P_PUBLIC_CONFIG__` into HTML.
- CSP must not use `unsafe-eval`/wildcards by default; preview origins are explicitly allowlisted.
- Missing assets return 404, not SPA HTML; API paths are not swallowed by the fallback.

### 5.2 API classification (verified)

```text
/api/*  /edge-api/*  /v1/*  /v2/*  /mcp-registry/*  /sandbox/*  /claudeai-rpc/* (selected)
```

Known/forwarded vs local is configuration- and build-dependent. A path that is neither implemented
nor explicitly forwarded returns HTTP 503:

```json
{
  "error": {
    "type": "custom_3p_not_available",
    "message": "This functionality is not available in third-party mode."
  }
}
```

This is a capability boundary, not an invitation to proxy arbitrary Anthropic traffic.

### 5.3 Bootstrap stubs

Verified synthetic responses include:

- `/api/bootstrap` — account/org, model selector state, catalog, feature state, gated messages.
- `/api/bootstrap/*/app_start`
- `/api/bootstrap/*/system_prompts`
- `/api/bootstrap/*/current_user_access`
- plugin/marketplace listings
- GitHub branch statuses (empty in 3P)
- account/organization lookups derived from local config.

Contract rules:

- Never include the configured API key or OAuth token in a bootstrap response.
- Model selector state must match the active provider profile; do not advertise disabled providers.
- Feature gates are explicit (`supported`/`unavailable`/`unsupported` + reason).
- Stubs are versioned with the renderer; unknown fields ignored, missing required fields fail the
  screen gracefully.

### 5.4 Forwarding

If a path is in the explicit endpoint table, forward it to the configured Anthropic host with:

- the configured auth scheme;
- `anthropic-version` and required beta headers;
- a strict allowlist of headers (drop cookies, origin, credentials not owned by the app);
- timeouts, redirect refusal, response-size caps;
- no wildcard host or path concatenation that permits SSRF;
- redacted logging.

Exact endpoint coverage was not exhaustively enumerated; treat it as **unknown** and default to 503.

## 6. Provider contracts

Exact host mappings and credential kinds are in [12](12-ai-architecture.md). Summary:

| Provider | Native endpoint / auth | Model list |
|---|---|---|
| Claude API | `api.anthropic.com/v1/messages`, `x-api-key`/OAuth | `/v1/models` or static |
| Bedrock | `bedrock-runtime[-fips].<region>.amazonaws.com`, SigV4/profile/SSO | Bedrock model APIs or static |
| Bedrock Mantle | `bedrock-mantle.<region>.api.aws`, bearer/credential helper | provider discovery/static |
| Vertex | `<region>-aiplatform.googleapis.com` (or rep host), Google OAuth/Workforce | publisher models/static |
| Foundry | `<resource>.services.ai.azure.com`, Entra/API key | deployments/static |
| Gateway | configured base, bearer/x-api-key | `GET /v1/models?limit=1000`, 3 pages, 10 s/page |
| OpenAI Responses (Phase 8 slice 1) | `{base}/responses` (default `https://api.openai.com/v1`), Bearer (static key required — first-party only) | `GET {base}/models` (single page, 10 s) |

Provider errors must be mapped to:

```ts
type ProviderErrorCode =
  | 'auth' | 'permission' | 'quota' | 'rate_limit' | 'region'
  | 'model_not_found' | 'model_unsupported' | 'context_length'
  | 'invalid_request' | 'unsupported_modality' | 'content_filter'
  | 'timeout' | 'network' | 'server' | 'cancelled' | 'unknown';
```

Do not retry non-idempotent calls automatically. Do not retry a request on a different model.

## 7. MCP/tool contracts

```ts
interface McpServerConfig {
  id: string;
  name: string;
  transport: 'stdio'|'http'|'sse';
  command?: string; args?: string[]; env?: Record<string,string>;
  url?: string;
  oauth?: { mode:'dcr'|'manual'|'none'; scopes?: string[] };
  enabled: boolean;
  source: 'managed'|'local'|'plugin';
  policy?: { allow?: string[]; deny?: string[]; ask?: string[] };
}

interface ToolDefinition {
  name: string;                  // namespaced: mcp__<server>__<tool>
  description: string;
  inputSchema: JSONSchema;
  annotations?: { readOnly?: boolean; destructive?: boolean; openWorld?: boolean };
  source: 'builtin'|'mcp'|'plugin';
}
```

Rules:

- Namespace all MCP tools; collision always errors.
- Cache the tool list per server/session; snapshot enablement per turn.
- Validate arguments against the schema; invalid JSON never executes.
- Enforce egress before dialing; block private/link-local/metadata hosts unless policy allows.
- MCP startup failure is non-fatal and produces an actionable toast.
- Tool output is untrusted content. Never auto-execute commands found in it.
- Tool-result images/documents are preserved as content parts; unsupported parts produce a visible error.

## 8. Artifact contract

```ts
interface ArtifactRecord {
  id: string;
  sessionId: string;
  name: string;
  kind: 'html'|'svg'|'markdown'|'other';
  createdAt: string;
  updatedAt: string;
  lastModifiedSession?: string;
  starred?: boolean;
  hidden?: boolean;
  shared?: boolean;
  version: number;
  thumbnailRef?: string;
  contentRef: string;
  mcpTools?: string[];
}
```

- Content is stored separately from metadata; versions are immutable blobs.
- Share/export/print are explicit actions.
- HTML/SVG renders only in an isolated origin/partition with CSP and no bridge.
- `parkAndCaptureArtifact` links the generated content to the session; a failed/stopped task creates
  no phantom completed artifact.
- Deleting a session removes its artifacts per retention/legal hold.

## 9. Scheduled-task contract

```ts
interface ScheduledTaskRecord {
  id: string;
  title: string;
  prompt: string;
  schedule: { cron?: string; timezone: string; every?: string };
  enabled: boolean;
  model: ModelKey;
  projectId?: string;
  cwd?: string;
  permissionMode: string;
  approvalLifetimeDays?: number;
  keepAwake?: boolean;
  lastRun?: string;
  nextRun?: string;
  lastStatus?: 'ok'|'failed'|'skipped';
  skipReason?: string;
}
```

- The scheduler is local; a job runs only while the machine is awake/online.
- A skipped run records a reason; it is not silently counted as success.
- Every run creates a normal session and usage record.
- Disabling/deleting a task does not delete its history unless policy says so.

## 10. Configuration/control-plane contract

```ts
interface ConfigProfile {
  schemaVersion: number;
  id: string;
  name: string;
  provider: ProviderKind;
  connection: Record<string, unknown>;  // typed per provider
  capabilities: Record<string, unknown>;
  connectors: Record<string, unknown>;
  telemetry: Record<string, unknown>;
  limits: Record<string, unknown>;
  appearance: Record<string, unknown>;
  plugins: Record<string, unknown>;
  egress: Record<string, unknown>;
  source: Record<string, unknown>;
}
```

Operations: `list/read/create/write/duplicate/rename/delete/apply/getMerged/health/recheck/export`.

- `apply` is atomic and validates the merged config.
- Draft edits never mutate the active profile.
- Managed/read-only fields are enforced server-side.
- Secrets are opaque handles, not values, in renderer messages.
- Failed health/apply leaves the prior profile active.
- Profile switch may require relaunch; the UI must explain and guard running sessions.

## 11. Error envelope

```json
{
  "code": "PROVIDER_AUTH_FAILED",
  "message": "The gateway rejected the credential.",
  "retryable": false,
  "action": { "kind": "open_setup", "section": "connection" },
  "details": { "provider": "gateway", "status": 401 }
}
```

- User-safe message; no raw token/body/stack.
- Stable machine code for tests/telemetry.
- Optional action must be safe to invoke; no arbitrary URL or shell command.
- Cancellation is not an error; use `cancelled` stop state.

## 12. Extension manifest

```json
{
  "schemaVersion": 1,
  "id": "com.example.provider.minimax",
  "type": "provider",
  "protocol": "minimax-native",
  "minAppVersion": "2.42304.0",
  "platforms": ["linux", "darwin", "win32"],
  "entry": "dist/index.js",
  "integrity": "sha512-...",
  "capabilities": ["text", "tools", "reasoning", "usage"],
  "credentialSchema": { "...": "..." },
  "egress": ["api.example.com"],
  "signature": "..."
}
```

Rules: signed, version-pinned, lazy-loaded, no global fetch patch, no undeclared egress, no access to
other providers’ credentials, disableable without data loss.

## 13. Contract tests

- IPC: allowlist, origin/frame rejection, argument validation, unsubscribe, no listener leak.
- Streaming: order, exactly-one done, cancel, idle timeout, malformed events, oversized payload.
- Provider: native-wire fixtures per SDK, auth header, model id, tools, thinking, cache, usage, errors.
- MCP: stdio/HTTP/SSE lifecycle, OAuth refresh, tool schema, approval, egress, injection.
- Artifact: isolated render, no bridge, version/share/delete, retention.
- Config: atomic apply, discard, managed fields, secret redaction, profile switch.
- Scheduler: cron/timezone, skip reasons, run session/usage, keep-awake boundaries.
- Security: SSRF, traversal, symlink escape, prototype pollution, header injection.

See [17](17-traceability.md) for the test-ID catalog and module mapping.

## 14. Requirements

| ID | Requirement |
|---|---|
| `API-IPC-01` | Only enumerated bridge methods exist; sender/origin is revalidated in main |
| `API-IPC-02` | Events are ordered, unsubscribe cleanly, and stop at window close |
| `API-AI-01` | Exactly one native adapter handles a turn; no translation/router layer |
| `API-AI-02` | Provider errors/stop reasons/usage survive without loss |
| `API-MCP-01` | Tools are namespaced, schema-validated, policy-checked, and auditable |
| `API-ART-01` | Untrusted artifact content cannot access the main bridge or origin |
| `API-CFG-01` | Profile apply is atomic and secrets never reach the renderer |
| `API-STORE-01` | Records are versioned, corrupt data is quarantined, retention is enforced |
| `API-SEC-01` | No API accepts an arbitrary URL/command/path without policy validation |
