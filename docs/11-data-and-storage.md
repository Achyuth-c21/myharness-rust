# 11 — Data and Storage Model

The app uses a mix of JSON/JSONL files, Chromium storage, and IndexedDB. It is a local-first desktop
application, not a thin client. This document records the schema families and ownership boundaries;
it deliberately omits values, personal content, and credentials.

## 1. User-data root

Observed layout under `~/.config/Claude-3p`:

| Path | Purpose |
|---|---|
| `config.json` | Global preferences, theme, locale, window boot state, OAuth caches |
| `configLibrary/` | Named inference profiles + `_meta.json` |
| `model-catalog/` | Signed hosted model catalog and floor version |
| `local-agent-mode-sessions/` | Cowork/local agent sessions, usage ledger, memory, plugins |
| `claude-code-sessions/` | Code session metadata and shadow/worktree state |
| `claude-code/`, `claude-code-vm/` | CLI/VM versions and per-session assets |
| `spaces-present/` | Cowork spaces presentation state |
| `document-baselines/` | Artifact/document baselines |
| `git-shadow/`, `git-worktrees.json` | Code worktree/shadow repository metadata |
| `pending-uploads/` | Upload queue |
| `preview-last-frames/`, `document-baselines/` | Preview artifacts |
| `inference-bridge/` | **Nonstandard local patch** (not shipped) |
| `logs/` | Per-subsystem logs |
| `Crashpad/`, `sentry/` | Crash/error queues |
| `Partitions/` | Chromium partitions for previews and browser surfaces |
| `Local Storage/`, `IndexedDB/`, `Session Storage/` | Renderer state |
| `window-state.json` | Window geometry and display bounds |
| `developer_settings.json` | Developer-mode preference |
| `claude_desktop_config.json` | Local MCP/developer configuration |
| `mcp-user-tool-toggles.json`, `custom3p-mcp-schemas.json` | MCP enablement/schema cache |

The account/org directory is pseudonymized. A gateway deployment may use an all-zero placeholder
account/org because there is no Anthropic account identity. The exact hash/salt must be treated as a
device identifier and never published.

**Verified** for paths and schema presence. **Unknown:** migration from older roots and the precise
meaning of every Chromium partition.

## 2. Global preferences (`config.json`)

Top-level fields observed:

```text
updaterLastSeenVersion
first_launch_at
version_first_launch { version, ... }
locale
lastSeenRequireCoworkFullVmSandbox
hasTrackedInitialActivation
bootFrameLayout
lastKnownAccountUuid          (sensitive)
bootSidebarPlaceholder
windowControlsZoomFactor
custom3pMcpOAuth              (sensitive)
oauth:tokenCache              (sensitive)
windowSizeWasSignedIn
hostGitConfinedInstall
userThemeMode
```

Rules:

- Do not copy token caches, OAuth state, or account IDs into logs or docs.
- `userThemeMode` is `system`, `light`, or `dark`.
- Boot layout/zoom are applied before first paint.
- `version_first_launch` supports migration and first-run analytics.
- Unknown keys must be preserved on rewrite to avoid destroying forward-compatible state.

## 3. Configuration library

`configLibrary/<uuid>.json` stores a profile. `_meta.json` contains:

```json
{
  "appliedId": "<profile-uuid>",
  "entries": [
    { "id": "<profile-uuid>", "name": "...", "...": "..." }
  ]
}
```

Observed profile documents range from empty objects (draft/partial) to complete provider/capability
configs. A complete active gateway profile contained:

- gateway base URL and API key;
- model discovery/1M/pricing/effort/default-model flags;
- `inferenceProvider` and `inferenceCredentialKind`;
- six plugin marketplaces;
- two managed MCP servers;
- egress allowlist;
- built-in browser flag;
- embedded `claudeAiImport` policy.

Profiles can contain secrets. Restrict file permissions and prefer a keychain reference. Apply is not
atomic in the file layer unless implemented with a temporary file + rename; the rebuild must add
that.

## 4. Cowork/local agent sessions

### 4.1 Layout

```text
local-agent-mode-sessions/<account>/<org>/
  scheduled-tasks.json
  usage-ledger/YYYY-MM-DD.ndjson
  memory/memory/*.md
  cowork_plugins/
  <short-session-dir>/
    audit.jsonl
    .audit-key
    uploads/
    uploads-tmp/
    outputs/
    .claude/
  local_<uuid>.json
```

The short directory name is a truncated session id, not a separate identity. Session JSON files are
the metadata of record; the short directory holds runtime artifacts and audit data.

### 4.2 Session metadata fields

Observed top-level schema:

| Group | Fields |
|---|---|
| Identity | `sessionId`, `cliSessionId`, `processName`, `vmProcessName`, `createdAt`, `lastActivityAt` |
| Placement | `cwd`, `userSelectedFolders`, `resolvedFolderKinds`, `folderMountNames`, `hostLoopMode` |
| Model | `model` (string; can include effort/variant metadata) |
| Lifecycle | `isArchived`, `error`, `errorAt`, `errorVersion`, `title` |
| Prompt | `initialMessage`, `systemPrompt`, `systemPromptRendererAppends`, `globalMemoryBaselineHash` |
| Commands | `slashCommands` (snapshot) |
| Tools | `enabledMcpTools` map, `remoteMcpServersConfig` list, `orgCliExecPolicies` |
| Capabilities | `memoryEnabled`, `skillsEnabled`, `pluginsEnabled`, `documentFunnelEnabled`, `docxEditingCarveout`, `frameArtifactsEnabled` |
| Plugins | `pluginInstallPaths` |
| Policy | `egressAllowedDomains`, permission/sandbox fields |
| Account | `emailAddress` (sensitive, duplicated here) |

**Important:** session JSON is high-sensitivity. `systemPrompt` can be tens of kilobytes, and
`enabledMcpTools` can enumerate hundreds of tools, including third-party account context. A rebuild
should store only references/checksums where possible, encrypt metadata, and never log these objects.

The observed session file is ~1.5 MB for an inactive session because it embeds prompt and tool
snapshots. Startup restore must not load every file fully into memory.

### 4.3 Audit trail

Each session directory contains `audit.jsonl` and a 51-byte `.audit-key`. Audit records contain:

```text
type, uuid, session_id, parent_tool_use_id, client_platform,
timestamp, message, _audit_timestamp, _audit_hmac
```

`_audit_hmac` makes the log tamper-evident; the key is per session. Audit events cover tool calls and
agent actions. They are not a replacement for encryption or access control. A rebuild should:

- store audit keys in the OS keychain;
- rotate/rekey per session;
- define retention;
- redact message content in support exports unless explicitly requested.

### 4.4 Uploads/outputs

`uploads/`, `uploads-tmp/`, and `outputs/` hold user files. They are outside the metadata JSON.
Deletion must remove both metadata and files; retention must handle orphaned directories and failed
uploads. Symlinks must be resolved and confined to the granted folder.

## 5. Memory

`memory/memory/` contains Markdown memory files and an index. The UI exposes a global toggle and a
list of memory entries. Memory is injected into prompts and is therefore sensitive.

- Do not publish memory content or file names.
- Memory has a baseline hash used to detect changes.
- “Consolidate memory” can rewrite memory (a bundled skill exists).
- Retention for chats/Cowork explicitly **does not** delete memory automatically.
- “Reset/delete memory” must be explicit and auditable.

Exact memory indexing/embedding implementation is **unknown**; it may be file-based only.

## 6. Usage ledger

`usage-ledger/YYYY-MM-DD.ndjson` contains one JSON object per completed/attempted model call:

```json
{
  "v": 1,
  "ts": 0,
  "surface": "cowork|code|chat",
  "origin": "...",
  "sessionId": "local_...",
  "isError": false,
  "models": {
    "<model-id>": {
      "inputTokens": 0,
      "outputTokens": 0,
      "cacheReadTokens": 0,
      "cacheWriteTokens": 0,
      "webSearchRequests": 0
    }
  }
}
```

Properties:

- Append-only NDJSON, partitioned by local date.
- Multiple models per event supported.
- Cached tokens are recorded separately; the Usage page states they are excluded from token limits.
- No prompt/completion content is stored in the ledger.
- Aggregation for 7/30/90 days is computed locally.

Rebuild can use an append-only table or NDJSON, but must preserve error/attempt events to avoid
undercounting.

## 7. Scheduled tasks

`scheduled-tasks.json` schema:

```json
{
  "scheduledTasks": [],
  "recordedSkips": {},
  "sundayAliasBoundaryStamped": true,
  "dayFieldsOrBoundaryStamped": true
}
```

- Tasks include schedule, prompt, project/folder context, model, permissions, and last/next run in
  the full record (the observed file was empty).
- `recordedSkips` tracks missed runs because the machine slept/offline.
- Boundary stamps support cron/day-field migration.
- Run history may live elsewhere; exact location is **unknown**.

## 8. Code sessions

Directory:

```text
claude-code-sessions/<account>/<org>/
  <uuid>.profile-origin.json
  local_<uuid>.json
  deleted_<uuid>            (tombstone)
```

Code session metadata fields:

```text
sessionId, cliSessionId, cwd, originCwd, worktreePath, worktreeName,
sourceBranch, branch, createdAt, lastActivityAt, lastFocusedAt,
model, isArchived, title, titleSource, permissionMode,
enabledMcpTools, remoteMcpServersConfig, completedTurns, titleTurn,
lastSpawnRootDetected, remoteControlAutoEligible, latestUserFrameAt,
alwaysAllowedReasons, sessionPermissionUpdates,
classifierSummaryEnabled, reportFindingsCard,
spawnSeed, cliBinaryPin, promptAppendSnapshot, toolSurfaceSnapshot
```

Code worktrees are tracked separately in `git-worktrees.json` and a shadow Git repository under
`git-shadow/`. Uncommitted work must survive session deletion/retention, as the Limits copy states.

## 9. Plugins

`cowork_plugins/` contains:

- `installed_plugins.json`
- `known_marketplaces.json`
- `marketplaces/`
- `cache/`
- `.install-manifests/`
- `.last_inuse_sweep`

Plugin installation is per account/org, with a periodic in-use sweep. Tool policy is stored in the
configuration, not only in the plugin directory. Removing a plugin must remove its cache and
manifest while preserving audit records.

## 10. Renderer storage

### 10.1 localStorage

267 keys normalize to 104 patterns. Families:

| Family | Examples |
|---|---|
| UI layout | `dframe-store`, `desktop-frame.paneStore.v1`, `epitaxy.sidePaneStore.v1` |
| Mode/navigation | `LSS-sidebar-selected-mode`, `code-nav:*`, `wco-*` |
| Drafts | `composer-draft:epitaxy-local_<uuid>`, `chat-draft-write-signal` |
| Read/unread/star | `persisted.cowork-read-state.<uuid>`, `epitaxy-unread-v1`, `starred-*` |
| Permissions | `epitaxy-perm-mode-acks.<uuid>` |
| Onboarding | `seenInferenceSetupHint`, `has_started_cowork_conversation` |
| Caches | `rq-cache-confirmed-account`, `code-session-list:metadata-v2` |
| Terminal | `epitaxy.terminalTabs.v1`, `terminalPromptLines.v1` |
| Feature latches | `LSS-*`, `banner_dismissed:*` |

These are local UI state, not authoritative session data. A corrupted cache must not delete sessions.
A rebuild should version and migrate keys, and clear retired keys deliberately.

### 10.2 IndexedDB

Database `app_localhost_0`, version 1, object store `keyval`:

- `store:chat-draft:<session-id>`
- `store:pin-state:dframe-starred-code`

Drafts and pin state are renderer-owned. They are not encrypted at rest by Chromium. Do not store
secrets or personal long-term data here.

### 10.3 Chromium partitions

Observed:

- `cowork-file-preview`
- `launch-preview-cowork-shared`
- `launch-preview-static`
- default browser partition

Partitions isolate cookies/cache for previews. Artifact/file previews must not share the main
partition or preload. Partition lifetime and clearing rules are **unknown**; define them in the
rebuild.

## 11. Model catalog

`model-catalog/published.json`:

```json
{
  "format": 1,
  "locationHash": "…",
  "url": "https://downloads.claude.ai/model-catalog/v1/catalog.json",
  "fetchedAt": 0,
  "staleAt": 0,
  "etag": "…",
  "documentBytes": "<base64>",
  "sidecar": {
    "schema": 1,
    "algorithm": "RSASSA-PKCS1-v1_5-SHA512",
    "signature": "…",
    "publicKeySha256": "…"
  },
  "version": 2707
}
```

Decoded document:

```text
$schema, schema_version, version, issued_at, expires_at, key_id, surfaces
```

Surfaces: `cc`, `ccd`, `ccr`, `chat`, `cowork`. Each has a model selector state/config and a model
list. Model metadata keys include:

```text
id, name, short_name, description, section, capabilities, thinking,
fast_mode, hard_limit, voice_model, min_claude_code_version,
quick_select, runtime, offered_on
```

`published-floor.json` records the minimum catalog version. The catalog is signed; a mismatch or
expired payload must be rejected or fall back to a bundled copy. Do not treat catalog model lists as
provider entitlements — credentials and model policy still apply.

## 12. Logs and diagnostics

- `main.log`, `main1.log` — app lifecycle and subsystem events.
- `custom3p-setup.log` — setup/config health.
- `mcp.log`, `mcp-server-*.log` — server lifecycle and stderr.
- `cli-diagnostics*.jsonl` — agent diagnostics.
- `coworkd-console.log`, `cowork_vm_node.log` — sandbox/VM.
- `in-app-browser.log`, `ssh.log` — remote/browser.
- `inference-bridge.log`, `native-adapter.log` — **only exists because of the local patch**.

Logs rotate and can grow large (multi-megabyte). They contain paths, session IDs, provider errors,
and possibly prompt fragments. Redaction must happen at write time for credentials and at
export/upload time for content. `support-bundle` must enumerate exactly what is included.

## 13. Retention and deletion

Policy fields:

- `chatSessionRetentionDays`
- `coworkSessionRetentionDays`
- `codeSessionRetentionDays`
- `sessionRetentionHold` (legal hold)
- `relaunchEnforcementHours`, `configRecheckIntervalMinutes` (config lifecycle)

Deletion semantics:

- Chat retention deletes chats and files; projects and memory remain.
- Cowork retention deletes tasks, uploads, outputs; spaces and memory remain.
- Code retention deletes sessions/conversation; uncommitted work on disk remains.
- Legal hold suspends all automatic deletion.
- Manual delete should remove metadata, transcript, uploads, outputs, and preview cache; audit
  records may be retained according to policy.

Orphan cleanup is required for crash-interrupted uploads/deletes. The rebuild should use tombstones
and a garbage collector rather than recursive deletes during UI interaction.

## 14. Recommended rebuild model

A normalized local store is easier to audit than many JSON files:

| Table | Key fields |
|---|---|
| `profiles` | id, name, provider, credential_ref, policy_json, revision |
| `sessions` | id, surface, title, cwd, model, effort, permission_mode, archived, timestamps |
| `turns` | id, session_id, role, content_ref, status, usage |
| `tool_calls` | id, turn_id, server, tool, args_redacted, result_ref, approval |
| `artifacts` | id, session_id, kind, current_version, visibility |
| `artifact_versions` | id, artifact_id, content_ref, created_at |
| `projects` | id, name, instructions, folder, memory_enabled |
| `scheduled_tasks` | id, cron, prompt, context, enabled, last_run, next_run |
| `scheduled_runs` | id, task_id, status, started_at, ended_at, error |
| `memory_entries` | id, scope, content_ref, updated_at |
| `usage_events` | id, session, model, tokens/cache/search, error, timestamp |
| `audit_events` | id, session, actor, action, payload_hash, hmac |
| `plugins` | id, marketplace, version, enabled, install_path |

Store large content (attachments, artifacts, transcripts, logs) as content-addressed files with
references. Encrypt sensitive metadata at rest where the OS supports it. Keep secrets only in the
keychain.

## 15. Data requirements

| ID | Requirement |
|---|---|
| `DATA-SESSION-01` | Session metadata, transcript, uploads, outputs, and audit are deleted atomically or via tombstones/GC. |
| `DATA-SESSION-02` | Restore is idempotent and never starts a duplicate live process for one session. |
| `DATA-SESSION-03` | Session JSON never exposes credentials; account identifiers are minimized. |
| `DATA-SESSION-04` | Audit records are tamper-evident and key-protected. |
| `DATA-CATALOG-01` | Verify model-catalog signature, expiry, version floor, and schema. |
| `DATA-CATALOG-02` | Fall back to a bundled catalog on verification failure; surface staleness. |
| `DATA-USAGE-01` | Record input/output/cache-read/cache-write/web-search separately and preserve errors. |
| `DATA-USAGE-02` | Usage aggregation is device-local and does not claim to be provider billing. |
| `DATA-STORAGE-01` | localStorage/IndexedDB are treated as rebuildable caches, not sources of truth. |
| `DATA-STORAGE-02` | Previews use isolated partitions and never share the main renderer session. |
| `DATA-RETENTION-01` | Legal hold overrides all automatic deletion. |
| `DATA-RETENTION-02` | Retention never deletes uncommitted Code work or memory unless explicitly stated. |
| `DATA-PRIVACY-01` | Exports/diagnostics exclude secrets and content by default. |
