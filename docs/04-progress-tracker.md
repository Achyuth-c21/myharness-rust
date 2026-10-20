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
