# 02 — Application Lifecycle and Startup

This document describes the desktop shell’s observable lifecycle. The renderer is a local SPA; the
main process owns windows, protocols, credentials, sessions, tools, and updates. Server-side
Anthropic lifecycle is out of scope.

## 1. Process model

| Process | Responsibility | Evidence |
|---|---|---|
| Electron main | app lifecycle, windows, protocols, config library, provider requests, sessions, MCP, updates | main bundle | **V** |
| Preload | context-isolated bridge; exposes `claude.*` APIs and event subscriptions | `mainView.js` | **V** |
| Renderer (main) | local Claude SPA at `app://localhost` | `resources/ion-dist` | **V** |
| Renderer (setup) | separate configuration window at `app://localhost/setup-desktop-3p` | runtime | **V** |
| Claude Code CLI / agent | local session execution, tool use, transcripts, MCP | session files, logs | **V** |
| Cowork VM/helper | sandboxed execution and mounts | `coworkd` logs | **V** on Linux |
| Utility/worker processes | file index, heavy work, previews, find-in-page | build files | **V** |

Security defaults for renderer windows are frozen in the main bundle:

```text
sandbox: true
contextIsolation: true
nodeIntegration: false
nodeIntegrationInSubFrames: false
nodeIntegrationInWorker: false
webviewTag: false
navigateOnDragDrop: false
spellcheck: false
backgroundThrottling: true
```

The renderer therefore has **no direct Node access**. It gets a large, explicitly enumerated
`contextBridge` API. This is a capability boundary, not a conventional untrusted-web boundary: the
exposed API includes filesystem, process, MCP, session, and browser controls. Any rebuild must keep
the enumerated, origin-checked bridge and must not expose raw `ipcRenderer`.

**Verified.** `aKn` webPreferences object and `mainView.js` contextBridge wrappers.

## 2. Launch sequence

The observed sequence is:

1. **Entry.** Electron loads `.vite/build/index.pre.js`.
2. **Single-instance lock.** The primary instance acquires the lock; a second launch is routed through
   `second-instance` and deep-link/argv handling. If the lock is unavailable, the new process quits.
3. **App metadata.** Sentry release/debug IDs and `__claudeMainJsStartMs` are set.
4. **Privileged schemes.** `cowork-artifact`, `cowork-file`, `claude-media`, and `claude-simulator`
   are registered as standard/secure schemes (plus additional internal schemes).
5. **Managed configuration.** Local and bootstrap configuration are read and merged; the active
   profile and deployment mode are resolved.
6. **Theme.** The persisted `userThemeMode` (`system`, `light`, `dark`) is applied to
   `nativeTheme.themeSource` before the window is shown.
7. **Protocol handler.** `app://localhost` is installed. It serves `ion-dist`, injects CSP and
   optional public config, and installs the custom-3P API layer.
8. **Window creation.** The main window is created hidden, with persisted size/layout, then shown on
   `ready-to-show`.
9. **Preload.** `mainView.js` installs `claude.settings`, `claude.web`, `claude.hybrid`,
   `claude.simulator`, `claude.skills`, `claude.officeAddin`, `claude.buddy`, and
   `claude.engineSurface`.
10. **Renderer boot.** The SPA loads `index.html`, assets, locale, and `/api/bootstrap`.
11. **Session restore.** Local sessions are restored, scheduled-task listeners start, and the
    sidebar/model picker are hydrated.
12. **Background services.** Plugin marketplace sync, model catalog refresh, MCP startup, update
    check, usage/telemetry, and wake scheduler continue asynchronously.

**Verified** from bundles and logs. Boot timings are environment-specific; one observed Linux run
reported app-ready at about 0.5 s, window creation at 0.7 s, renderer DOM-ready at 1.4 s, and input
ready at 2.9 s. These are evidence samples, not service-level objectives.

## 3. Window map

| Window/surface | Purpose | Status |
|---|---|---|
| Main window | Chat/Cowork/Code SPA | Verified |
| `setup-desktop-3p` | third-party inference configuration | Verified |
| Quick Entry overlay | global shortcut entry | Feature `nativeQuickEntry` unavailable here |
| Popout/session windows | side-by-side sessions, artifact popouts | Partially supported; not exercised |
| Artifact preview window | isolated artifact/file renderer | Strong inference from protocol/window code |
| About window | about/version | Not opened |
| Device-code/OIDC window | interactive credential flows | Not exercised |
| Computer-use teaching/preview | guided permission surface | Platform-gated |
| Simulator native views | iOS simulator panes | Unsupported here |

The setup window is a separate renderer and is the **authoritative control plane** for provider
configuration. It must be reachable even when the main renderer cannot start inference.

## 4. The `app://localhost` protocol

The handler is installed once and combines three layers:

1. **Static SPA** from `resources/ion-dist`, with an `index.html` fallback for client routes.
2. **Custom-3P layer** when a third-party configuration exists: synthesized bootstrap, account
   settings overlay, plugin/marketplace listings, gated imports, and API stubs.
3. **Anthropic-host forwarding** for the limited endpoint table that remains renderer-reachable.

Key gates:

- Hostname must be `localhost`; other hosts return 404.
- An `Origin` header, if present, must be `app://localhost`; otherwise 403.
- Path traversal outside the ion-dist root returns 403.
- `index.html` is served with an injected CSP.
- Optional `__DESKTOP_3P_PUBLIC_CONFIG__` is injected only when a user-content renderer origin is set.
- `/api/`, `/edge-api/`, `/v1/`, `/v2/`, `/mcp-registry/`, `/sandbox/`, and selected
  `/claudeai-rpc/` paths are treated as API paths.
- An API path that is neither stubbed nor forwarded returns HTTP 503 with:

```json
{"error":{"type":"custom_3p_not_available","message":"This functionality is not available in third-party mode."}}
```

This 503 is expected product behavior in 3P mode, not a bug. See
[18-api-contracts.md](18-api-contracts.md).

**Verified** from `y2r`/`l2r`/`v2r`, protocol tables, and runtime behavior.

## 5. Bootstrap and configuration lifecycle

The custom-3P layer synthesizes `/api/bootstrap` and related responses from the active configuration
and account-settings overlay. It does not expose an Anthropic account backend. The bootstrap payload
includes account/organization shapes, feature state, model selector state, model catalog, gated
messages, and system prompts as applicable.

Configuration can come from:

- The local configuration library (`configLibrary/*.json`, `_meta.json`).
- A managed bootstrap URL that returns a per-user JSON overlay. Remote values override local values
  and become read-only.
- Enterprise-managed sources (OIDC bootstrap, organization config, MDM-style files).
- The setup window’s profile editor.

The setup API exposes `getMergedConfig`, `getConfigHealth`, `recheckConfigHealth`, `readConfig`,
`writeConfig`, `setAppliedConfig`, `switchProfile`, `relaunchApp`, and
`relaunchAppWithSessionGuard`. This shows a **draft → validate → apply → relaunch** model.

**Verified** for the API and UI labels. **Unknown:** the exact precedence when local, bootstrap, and
organization overlays conflict; and whether every apply path relaunches.

## 6. Theme and appearance initialization

`userThemeMode` is read before window creation. The renderer also reports its computed mode to the
main process via `claude.web.WindowControl.setThemeMode`. The app supports `system`, `light`, and
`dark`; the settings UI exposes all three. Code appearance is a separate setting (`Claude Light`,
`Claude Dark`) independent of the overall UI theme.

**Verified.** The dark palette was captured by temporarily switching the setting and reverting it.
See [07-ui-design-system.md](07-ui-design-system.md).

## 7. Session restoration

On renderer boot the app:

- Loads persisted local sessions from the user-data tree.
- Skips sessions already live in memory.
- Restores window state and split/popout layout.
- Starts scheduled-task listeners and marks renderer readiness.
- Hydrates the session list, active task, and model picker.
- Restores MCP/tool availability and emits status changes.

A session can be local, SSH/remote, VM-backed, or cloud-backed depending on policy. The 3P launch log
shows a placement decision such as “local-only: device allowed, SSH host allowed, cloud not
configured.” The rebuild must make placement explicit rather than silently moving a session.

**Verified** for restore behavior and placement vocabulary. **Unknown:** migration behavior across
app versions and partial-corruption recovery.

## 8. Claude Code / agent lifecycle

A Cowork or Code task creates a local agent session with:

- a session id and optional CLI session id;
- a working directory and selected folders;
- model, effort, permission mode, and tool policy;
- enabled MCP tools and remote MCP configuration;
- skills/plugins/document-funnel flags;
- a transcript and audit log.

The agent process receives user messages over stdin, emits control events and API usage diagnostics,
and can request tool permissions. On shutdown it drains active work, emits a shutdown signal, and
cleans up the PTY and sandbox mounts.

The app also launches the Claude Code CLI for terminal/Code use, with provider environment flags
(`CLAUDE_CODE_USE_BEDROCK`, `_MANTLE`, `_VERTEX`, `_FOUNDRY`, `_GATEWAY`) derived from the active
profile. This is a shipped integration; the current local adapter additionally injects itself into
the CLI entry point.

**Verified** for session metadata, logs, environment flags, and cleanup. **Unknown:** exact
process-supervision retry policy and all platform-specific shell behavior.

## 9. Quick Entry, tray, and wake behavior

- Quick Entry is gated by `nativeQuickEntry`; **unavailable** here. The global shortcut setting exists
  but cannot produce the native panel.
- The tray icon gate is supported, but Linux reports that the desktop does not show tray icons;
  closing the window quits.
- `keepAwakeEnabled` and `WakeScheduler` prevent idle sleep so scheduled tasks can run. This does not
  override lid close or explicit sleep.
- “Run on startup” is exposed through `Startup` IPC and the OS login-item mechanism.

**Verified** for UI labels and gates; native OS behavior is platform-specific and not fully
reproduced.

## 10. Update lifecycle

The app has an `AutoUpdater` bridge (9 methods) and a visible “Update available” banner. Update
configuration includes:

- `releaseChannel`
- `disableAutoUpdates`
- `autoUpdaterEnforcementHours`
- `updateViaUpdatesHost`
- `relaunchEnforcementHours`
- `configRecheckIntervalMinutes`
- `updaterLastSeenVersion`

The observed install offered `2.42304.2`. It was **not** applied. The update feed can be routed to
`releases.claude.com` so `api.anthropic.com` can remain blocked. On Linux the log also reported an
“in-app updater off (updates via apt)” mode for a different install; update behavior therefore
depends on packaging.

**Verified** for UI, config keys, feed host, and pending version. **Unknown:** the exact download,
signature verification, and rollback flow in this package.

## 11. Shutdown and cleanup

Quit is asynchronous. `before-quit` and `will-quit` run cleanup handlers, including:

- persist main-window state;
- clean up popout windows;
- close web contents views and artifact/file-preview views;
- stop shell PTYs and sandbox processes;
- unmount Cowork volumes;
- flush event logs;
- mark the app ready for quit.

`window-all-closed` triggers the platform quit path. On Linux without tray support, closing the main
window exits the app.

**Verified** from lifecycle logs and source.

## 12. Recovery and failure surfaces

- Crashes are captured by Crashpad and Sentry queues under the user-data tree.
- Renderer load failures have an error overlay with retry/quit actions.
- Session restore is tolerant of already-live sessions and missing sessions.
- MCP startup errors are surfaced as persistent toasts and in connector status.
- Provider health errors are surfaced in the setup window and in connection-error banners.
- A config-health resolver can mark a landed remote configuration as tainted and require re-check.

**Verified** for error strings/APIs. **Unknown:** full recovery UX for corrupted databases, failed
migrations, and half-applied provider profiles.

## 13. Lifecycle requirements for the rebuild

| ID | Requirement |
|---|---|
| `ARCH-LIFE-01` | Acquire a single-instance lock and route second launches/deep links to the primary instance. |
| `ARCH-LIFE-02` | Register privileged protocols before app-ready and validate origins. |
| `ARCH-LIFE-03` | Apply theme and persisted window state before showing the window. |
| `ARCH-LIFE-04` | Load and validate configuration before creating provider clients. |
| `ARCH-LIFE-05` | Keep the setup window usable when provider initialization fails. |
| `ARCH-LIFE-06` | Expose only enumerated, origin-checked bridge APIs; never expose `ipcRenderer`. |
| `ARCH-LIFE-07` | Restore sessions idempotently and never start a second live process for the same session. |
| `ARCH-LIFE-08` | Drain active work and clean up PTYs/mounts/popouts on quit. |
| `ARCH-LIFE-09` | Make update checks, download, apply, and rollback observable and policy-controlled. |
| `ARCH-LIFE-10` | Report startup and shutdown failures to logs without leaking secrets. |
