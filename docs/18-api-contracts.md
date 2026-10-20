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
