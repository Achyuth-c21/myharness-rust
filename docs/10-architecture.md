# 10 — Application Architecture

This document describes the shipped desktop architecture and the boundaries a rebuild must preserve.
It does **not** treat the local inference patch as shipped behavior. The patch is separated and
critiqued in [12-ai-architecture.md](12-ai-architecture.md).

## 1. High-level shape

```text
┌─────────────────────────────────────────────────────────────────────┐
│ Electron main                                                       │
│  lifecycle · windows · config · credentials · provider clients      │
│  app:// protocol · sessions · Claude Code/agent · MCP · updates     │
│  telemetry · schedulers · file index · PTY · previews               │
└───────────────▲─────────────────────────────────────────────────────┘
                │ generated IPC envelope, sender/frame checks
┌───────────────┴─────────────────────────────────────────────────────┐
│ Preload (contextIsolation, sandbox)                                 │
│  claude.settings.* · claude.web.* · claude.hybrid · claude.skills   │
│  claude.simulator · claude.officeAddin · claude.buddy               │
│  claude.engineSurface                                               │
└───────────────▲─────────────────────────────────────────────────────┘
                │ structured values/events only
┌───────────────┴─────────────────────────────────────────────────────┐
│ Main renderer — local SPA at app://localhost                         │
│  React · React Intl · Tailwind · routes · composer · settings       │
│  calls /api/* through the app:// protocol layer                     │
└─────────────────────────────────────────────────────────────────────┘

Separate renderer: app://localhost/setup-desktop-3p
Separate processes: agent/CLI, Cowork VM/helper, MCP servers,
                    file-index/transcript-search/heavy-work workers, PTY host
```

The main renderer is an **app-local SPA**, not a remote site. This is crucial: the product’s UI is
bundled in `resources/ion-dist`, but its data APIs are a mixture of local stubs/overlays and a small
set of forwarded Anthropic endpoints. In third-party mode, most Anthropic-hosted endpoints are
unavailable by design.

## 2. Technology evidence

`package.json` and the build tree show:

| Area | Technology |
|---|---|
| Shell | Electron 44.7.2 |
| Build | Electron Forge 8 alpha, Vite, React, Tailwind CSS v4 |
| Localization | React Intl, 35,441 `en-US` strings |
| Validation | Zod, AJV |
| Storage | `electron-store`, `conf`, JSON/JSONL, IndexedDB, localStorage |
| Logging | Winston with custom transports |
| Errors | `@sentry/electron`, Crashpad |
| Agent | `@anthropic-ai/claude-agent-sdk` 0.3.299 |
| MCP | `@modelcontextprotocol/sdk` and client/core packages |
| Terminal | `node-pty`, `@xterm/headless` |
| Remote | `ssh2` |
| Browser automation | `playwright-core` |
| Images | `sharp` |
| Packaging | Electron Forge makers (deb/rpm/dmg/msix/pkg/squirrel/zip) |
| Testing/dev | Vitest, Playwright, oxlint |

Some packages are dev/build-time only. Resolved versions are in the lockfile, which was not shipped.
Do not assume a dependency is used in the main runtime without verifying its call site.

**Verified** from `package.json`; **strong inference** for usage based on build files and behavior.

## 3. Main-process responsibilities

### 3.1 App lifecycle and windows

- Single-instance lock, second-instance/deep-link routing.
- Privileged scheme registration.
- Main window, setup window, popouts, about/preview/simulator windows.
- Theme and native chrome.
- Crash/error overlay and lifecycle cleanup.
- Startup/login item and tray/quick-entry integration.

### 3.2 Configuration and credentials

- Local profile library and merge with managed bootstrap.
- Provider-specific credential helpers and OAuth/device-code flows.
- Policy/feature resolution.
- Config health/recheck and tainted-state handling.
- Redaction and safe export boundaries.

### 3.3 Inference

- Provider clients for Claude API, Bedrock, Bedrock Mantle, Vertex AI, Foundry, and Gateway.
- Model discovery and validation.
- Streaming, cancellation, retries, idle timeouts.
- Token counting, usage ledger, limits.
- Capability metadata passed to the renderer.

The shipped gateway contract is Anthropic-compatible. Shipped model policy deliberately rejects
non-Anthropic model IDs (`minimax`, `gpt`, `gemini`, `llama`, etc.) unless the ID satisfies the
Anthropic family checks. This is not a bug; it is a product constraint that the local patch bypasses.

### 3.4 Sessions and agents

- Local session metadata and transcripts.
- Claude Agent SDK/CLI process control.
- Cowork VM/helper sandbox and mounts.
- Tool approval and permission modes.
- SSH/remote/cloud placement where enabled.
- Archive/delete/retention/legal hold.
- Scheduled task execution and wake scheduling.

### 3.5 Tools and extensions

- Managed and developer MCP servers over stdio/HTTP/SSE.
- MCP OAuth and tool discovery.
- Skills, plugins, desktop extensions (`.dxt`, `.mcpb`).
- Built-in tools and tool policy.
- Tool search/on-demand schema loading.
- Audit and usage events.

### 3.6 Protocol and API layer

- `app://localhost` static serving with SPA fallback.
- CSP and optional public-config injection.
- Synthetic bootstrap/account/org/plugin responses for 3P.
- Endpoint table for forwarded Anthropic requests.
- 503 for unavailable APIs.
- Dedicated schemes for artifacts/files/media/simulator.

### 3.7 Observability and updates

- Winston logs per subsystem.
- Sentry/Crashpad and optional OTel.
- Usage metrics without message content.
- Update feed, download, enforcement, relaunch.
- Diagnostic/support bundles with explicit upload.

## 4. Preload and IPC boundary

`mainView.js` exposes eight namespaces:

| Namespace | Purpose |
|---|---|
| `claude.settings` | App preferences, startup, CLI install, shortcuts, MCP, extensions, file pickers, 3P setup, import, support |
| `claude.web` | Window controls, account, navigation, menu events, deep links, storage, toasts, auth, Cowork, artifacts, Code, sessions, usage |
| `claude.hybrid` | Hybrid/remote transition and org switching |
| `claude.simulator` | iOS simulator integration (unsupported here) |
| `claude.skills` | Skill management |
| `claude.officeAddin` | Microsoft 365 add-in integration |
| `claude.buddy` | Hardware buddy/companion integration |
| `claude.engineSurface` | Engine/agent control surface |

The live API inventory contains 17 `claude.settings` namespaces and 60 `claude.web` namespaces.
The largest are local-agent/session APIs, not generic CRUD.

### 4.1 Channel pattern

Channels are generated with a build token:

```text
$eipc_message$_<random-build-token>_$_<Namespace>_$_<method>
```

That token rotates between builds. The rebuild must not rely on the literal value; use a generated,
allowlisted channel map.

Each method is a thin wrapper around `ipcRenderer.invoke(channel, ...args)` and returns a Promise.
Event subscriptions register a listener and return an unsubscribe function. The preload keeps the
bridge object frozen/enumerated by namespace.

### 4.2 Sender validation

The preload refuses to expose a namespace unless:

- the call comes from the top frame (frame token/routing id match); and
- the origin is `https://claude.ai`, `https://preview.claude.ai`, `https://claude.com`,
  `https://preview.claude.com`, an `app://localhost` origin, `localhost`, or `*.ant.dev`.

The protocol handler separately enforces `Origin: app://localhost` for non-navigation requests.
Main-process handlers must validate the sender too; preload gating alone is not a security boundary.
Whether every generated main handler performs a sender check was not exhaustively proven — **unknown**.

### 4.3 API inventory

- 17 settings namespaces, 145 methods.
- 60 web namespaces, 1,017 methods (LocalSessions alone exposes 339).
- 131 renderer feature gates.

The inventory is evidence, not a stable public API. The rebuild should define its own smaller,
versioned contracts and implement only documented capabilities.

`evidence/runtime-api-surface.json` is the machine-readable inventory. [18-api-contracts.md](18-api-contracts.md)
describes the important contracts.

## 5. The `app://` layer

### 5.1 Static server

- Resolves `resources/ion-dist`.
- Serves `index.html` for client routes.
- Prevents traversal outside the root.
- Rejects non-`localhost` hosts.
- Rejects cross-origin requests from anything other than `app://localhost`.
- Injects CSP and optional `__DESKTOP_3P_PUBLIC_CONFIG__`.
- Adds `Content-Security-Policy` to HTML responses.

CSP is configuration-derived. It can allow:

- the configured Anthropic host;
- artifact-preview iframe origin;
- dictation/media origins;
- configured connect sources.

The exact CSP string is configuration-dependent. The rebuild must generate, test, and log it without
weakening it to `unsafe-eval` by default.

### 5.2 API path classification

API-like paths include:

```text
/api/*
/edge-api/*
/v1/*
/v2/*
/mcp-registry/*
/sandbox/*
/claudeai-rpc/*   (selected)
```

They are routed through the custom-3P endpoint table. A path that is neither stubbed nor forwarded
returns:

```json
{"error":{"type":"custom_3p_not_available","message":"This functionality is not available in third-party mode."}}
```

This is a deliberate capability boundary.

### 5.3 Synthetic bootstrap

The custom-3P layer builds `/api/bootstrap` from local account settings and config. It includes
account/organization shapes, model selector state, model catalog, feature state, gated messages, and
system prompts where applicable. It stamps first-seen timestamps in local account settings.

Other stubs include:

- `/api/bootstrap/*/app_start`
- `/api/bootstrap/*/system_prompts`
- `/api/bootstrap/*/current_user_access`
- plugin/marketplace listings
- GitHub branch statuses (empty in 3P)
- account/organization lookups derived from local config

**Verified** from the main bundle and runtime.

## 6. Renderer architecture

The SPA is a React application with client-side routing. It renders:

- the main shell (title bar, sidebar, main pane);
- Cowork/Code homes, session transcripts, projects, artifacts, scheduled tasks, Customize;
- settings modal and language dialog;
- composer and its popovers;
- search/filter overlays;
- model picker/browser;
- error toasts, update banner, loading/empty states.

It does **not** own provider credentials or spawn processes. It calls the preload APIs and the
`/api/*` layer. This separation should be preserved.

The renderer is localized through React Intl. The locale catalog includes 35,441 strings. A rebuild
should use a message catalog rather than embedding English strings, especially for error and policy
copy.

### 6.1 State ownership

| State | Owner |
|---|---|
| Theme, font, motion, transcript preferences | renderer preference store + main config |
| Selected model/effort | session and global preference, depending on `alwaysStartWithDefaultModel` |
| Composer draft/attachments | renderer, persisted locally where supported |
| Task transcript | agent/session store; renderer caches a view |
| Provider credentials | main/keychain only |
| MCP definitions/tokens | main config/keychain |
| Window layout/zoom | main window state + renderer |
| Feature gates | main/config → renderer at boot |
| Update state | main updater → renderer banner |
| Usage ledger | main/session store → renderer Usage page |

## 7. Worker and helper processes

| Component | Purpose |
|---|---|
| `file-index-worker` | Index workspace files for search/context |
| `transcript-search-worker` | Search session transcripts |
| `heavy-work-worker` | CPU-heavy/background tasks |
| `stall-sampler-worker` | Detect renderer/main stalls |
| `shell-path-worker` | Resolve login-shell PATH |
| `pty-host` | Terminal sessions for Code |
| `mcp-runtime` | Isolated MCP runtime |
| Cowork helper/VM | Mounts, sandbox, file operations |
| Agent/CLI | Model/tool loop and transcript |

These processes must be treated as separate trust domains. Their output is untrusted content. File
paths and commands must be validated in main, not in the renderer.

## 8. Trust boundaries and threats

| Boundary | Risk | Required control |
|---|---|---|
| Renderer ↔ preload | XSS or malicious artifact could call powerful APIs | origin/top-frame checks, narrow methods, sender validation, no raw IPC |
| Artifact/content ↔ app | Untrusted HTML/Markdown could execute | dedicated sandboxed origin/window, no preload, strict CSP, no `allow-same-origin` with scripts |
| MCP server ↔ agent | Untrusted tool output/prompt injection | treat output as data, tool approval, egress policy, no automatic credential forwarding |
| Provider ↔ app | Malicious/incorrect model output | schema validation, no silent tool execution, visible errors |
| Config ↔ secrets | Export/log leakage | keychain, redaction, safe export, mode 0600 |
| Deep link ↔ app | Open arbitrary paths/commands | allowlisted route grammar, auth checks, path validation |
| Update feed ↔ app | Supply-chain compromise | signature verification, pinned channels, rollback |
| Remote bootstrap ↔ config | Policy override/tainting | authenticated HTTPS, signature/trust policy, health checks, read-only display |
| File picker ↔ filesystem | Reading outside granted folders | main-side canonicalization, grants, no symlink escape |

The shipped renderer exposes filesystem and process capabilities. That makes the origin check and
artifact isolation especially important. A rebuild should reduce the API surface and require
explicit capability tokens for sensitive operations.

## 9. Rebuild module map

| Module | Responsibility | Docs |
|---|---|---|
| `app-shell` | lifecycle, windows, menus, protocols, theme | [02](02-application-lifecycle.md) |
| `bridge` | generated, validated preload API | this doc, [18](18-api-contracts.md) |
| `config` | profiles, managed overlay, validation, credentials | [09](09-settings-and-configuration.md) |
| `providers` | native provider adapters, discovery, capabilities | [12](12-ai-architecture.md) |
| `sessions` | task metadata, transcripts, restore, retention | [11](11-data-and-storage.md) |
| `agent` | model/tool loop, permissions, streaming | [12](12-ai-architecture.md) |
| `mcp` | server lifecycle, tools, OAuth, policy | [features/mcp-skills-tools.md](features/mcp-skills-tools.md) |
| `skills` | bundled/user skills, invocation | same |
| `plugins` | marketplaces, install, org bundles | [features/customize.md](features/customize.md) |
| `artifacts` | renderer, versions, preview isolation | [features/artifacts.md](features/artifacts.md) |
| `scheduler` | cron, wake, approvals, history | [features/scheduled-tasks.md](features/scheduled-tasks.md) |
| `search` | sessions/files/transcript search | [features/search-and-filter.md](features/search-and-filter.md) |
| `telemetry` | crash/usage/OTel, redaction | [features/updates-telemetry-privacy.md](features/updates-telemetry-privacy.md) |
| `updates` | feed, enforcement, rollback | same |
| `ui` | design system, screens, composer | [05](05-screen-inventory.md), [07](07-ui-design-system.md), [08](08-interaction-catalog.md) |

## 10. Architecture requirements

| ID | Requirement |
|---|---|
| `ARCH-PROC-01` | Keep main, preload, renderer, agent, and MCP in separate trust domains. |
| `ARCH-PROC-02` | Renderer has no Node integration and no raw IPC. |
| `ARCH-PROC-03` | Every privileged bridge method validates sender, arguments, and policy. |
| `ARCH-PROC-04` | Artifact/content rendering uses an isolated, script-safe origin. |
| `ARCH-PROC-05` | Provider clients live in main; credentials never enter renderer storage. |
| `ARCH-PROC-06` | The setup window remains usable when main provider boot fails. |
| `ARCH-PROC-07` | Protocol paths are allowlisted; unavailable APIs return a typed 503. |
| `ARCH-PROC-08` | MCP/tool output is untrusted and never executed as instructions. |
| `ARCH-PROC-09` | Background workers have narrow, typed interfaces and timeouts. |
| `ARCH-PROC-10` | Logs, exports, and diagnostics redact secrets and content by default. |
| `ARCH-PROC-11` | Updates verify signatures and support rollback. |
| `ARCH-PROC-12` | The app never silently substitutes a provider or model. |
