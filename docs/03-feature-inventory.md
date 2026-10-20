# 03 — Feature Inventory

This is the product-level inventory for the observed `2.42304.0` 3P build. Status describes **this
installation and platform**, not the entire product line.

Status vocabulary:

- **Available** — reached and exercised.
- **Available, unexercised** — entry point present; deeper flow not completed.
- **Policy-disabled** — intentionally disabled by the active third-party configuration.
- **Unavailable** — runtime feature gate says unavailable in this environment.
- **Unsupported** — runtime gate returns an explicit unsupported reason.
- **Partial** — some states/flows observed, others not.
- **Unknown** — not reached or no reliable evidence.
- **Noncompliant** — present in the inspected install but rejected by this blueprint.

The dynamic status vocabulary is intentionally small. Nuance (for example “available by policy” or
“supported gate but unexercised”) is recorded in the evidence/notes column, not as a new status.

Every row has a stable feature ID used by [17-traceability.md](17-traceability.md).

## 1. Shell and navigation

| ID | Feature | Status | Evidence / notes | Spec |
|---|---|---|---|---|
| FEAT-SHELL-01 | Main window with custom title bar | Available | Window controls, back/forward, sidebar toggle; native menu may also exist | [05](05-screen-inventory.md) |
| FEAT-SHELL-02 | Sidebar navigation | Available | New, Projects, Artifacts, Scheduled, Customize, task list, account, update banner | [06](06-navigation-map.md) |
| FEAT-SHELL-03 | Cowork/Code mode switch | Available | Segmented control; Code loads `/epitaxy` | [features/cowork.md](features/cowork.md), [features/code.md](features/code.md) |
| FEAT-SHELL-04 | Task history and pin/unread/rename/archive/delete | Available | Task overflow menu, shortcut letters P/U/R/A/D | [features/chat-and-composer.md](features/chat-and-composer.md) |
| FEAT-SHELL-05 | Global search palette | Available | Session/project rows, keyboard navigation, relative time; actions/snippets not observed | [features/search-and-filter.md](features/search-and-filter.md) |
| FEAT-SHELL-06 | Account menu | Available | Settings, Language, Inference configuration, changelog, learn more, sign out | [screens/account-menu-and-language.md](screens/account-menu-and-language.md) |
| FEAT-SHELL-07 | Language selection | Available | 11 locales; updates string catalog and native locale | [screens/account-menu-and-language.md](screens/account-menu-and-language.md) |
| FEAT-SHELL-08 | Quick Entry overlay | Unavailable | `nativeQuickEntry` unavailable; shortcut setting still visible | [14](14-gaps-and-unknowns.md) |
| FEAT-SHELL-09 | Tray icon | Partial | UI says closing quits because desktop has no tray icons | [09](09-settings-and-configuration.md) |
| FEAT-SHELL-10 | Update banner and update check | Available | Update to `2.42304.2` offered, not applied | [features/updates-telemetry-privacy.md](features/updates-telemetry-privacy.md) |
| FEAT-SHELL-11 | Native OS menu / keyboard shortcuts | Available | Menu accelerators extracted from main bundle | [08](08-interaction-catalog.md) |
| FEAT-SHELL-12 | Popout/split view | Available, unexercised | Feature gates and window code; not exercised in this pass | [14](14-gaps-and-unknowns.md) |

## 2. Conversation surfaces

| ID | Feature | Status | Evidence / notes | Spec |
|---|---|---|---|---|
| FEAT-CHAT-01 | New task / new conversation | Available | `/new`; greeting, composer, active tasks | [screens/new-task.md](screens/new-task.md) |
| FEAT-CHAT-02 | Composer with multiline text and send/stop | Available | 720 px max composer, send arrow, stop during generation | [features/chat-and-composer.md](features/chat-and-composer.md) |
| FEAT-CHAT-03 | Attachments | Available | Multiple files/photos; `Ctrl+U`; file input | [features/multimodal-attachments.md](features/multimodal-attachments.md) |
| FEAT-CHAT-04 | Skills picker | Available | 13 bundled/user skills in menu; manage/browse | [features/mcp-skills-tools.md](features/mcp-skills-tools.md) |
| FEAT-CHAT-05 | Connectors picker | Available | Built-in and user connectors | [features/mcp-skills-tools.md](features/mcp-skills-tools.md) |
| FEAT-CHAT-06 | Slash commands | Available | `/` menu with Add files, Usage, and skill rows; no descriptions observed | [features/chat-and-composer.md](features/chat-and-composer.md) |
| FEAT-CHAT-07 | Model picker and model browser | Available | Flat popover + More models flyout; “needs API key” label; current model; search not observed | [features/model-selection.md](features/model-selection.md) |
| FEAT-CHAT-08 | Project/folder selector | Available | Choose project or working folder for the task | [features/projects.md](features/projects.md) |
| FEAT-CHAT-09 | Permission-mode selector | Available | Default / accept edits / plan / bypass (policy-dependent) | [features/code.md](features/code.md) |
| FEAT-CHAT-10 | Task transcript and tool calls | Available | Message list, tool blocks, task detail; expand/collapse | [screens/task-detail.md](screens/task-detail.md) |
| FEAT-CHAT-11 | Stop generation | Available | Stop control replaces send while streaming | [features/chat-and-composer.md](features/chat-and-composer.md) |
| FEAT-CHAT-12 | Copy/retry/feedback/rewind | Partial | Transcript actions seen or present in API; not all exercised | [screens/task-detail.md](screens/task-detail.md) |
| FEAT-CHAT-13 | Incognito mode | Unknown | `setIncognitoMode` bridge; no UI evidence captured | [14](14-gaps-and-unknowns.md) |
| FEAT-CHAT-14 | Dedicated Chat tab | Unavailable | `chatTab` gate unavailable; unified Cowork view is shown | [14](14-gaps-and-unknowns.md) |

## 3. Cowork

| ID | Feature | Status | Evidence / notes | Spec |
|---|---|---|---|---|
| FEAT-COWORK-01 | Cowork task home | Available | Greeting, active tasks, composer | [features/cowork.md](features/cowork.md) |
| FEAT-COWORK-02 | Long-running task execution in a sandbox | Available | Local agent session, VM/helper mounts, PTY | [features/cowork.md](features/cowork.md) |
| FEAT-COWORK-03 | Task detail / transcript | Available | Tool calls, status, permission prompts | [screens/task-detail.md](screens/task-detail.md) |
| FEAT-COWORK-04 | Scheduled tasks | Available | `/scheduled-task`; templates; keep-awake | [features/scheduled-tasks.md](features/scheduled-tasks.md) |
| FEAT-COWORK-05 | Projects | Available | Projects list, new project from scratch/existing folder | [features/projects.md](features/projects.md) |
| FEAT-COWORK-06 | Artifacts library | Available | `/cowork-artifact`; empty state; live artifact creation | [features/artifacts.md](features/artifacts.md) |
| FEAT-COWORK-07 | Live artifact view/pane | Partial | “New artifact” starts a live-artifact session; `artifactsPane` unavailable | [features/artifacts.md](features/artifacts.md) |
| FEAT-COWORK-08 | Memory | Available | Global memory toggle, account memories, memory entries | [features/cowork.md](features/cowork.md) |
| FEAT-COWORK-09 | Trusted folders and file access | Available | Trusted folder manage/add; folder grants | [features/cowork.md](features/cowork.md) |
| FEAT-COWORK-10 | Spaces | Available, unexercised | CoworkSpaces API (folders, links, projects, memory copy) | [features/cowork.md](features/cowork.md) |
| FEAT-COWORK-11 | Built-in browser / Cowork browser | Unsupported | `coworkBrowser` → `cowork_browser_disabled` | [14](14-gaps-and-unknowns.md) |
| FEAT-COWORK-12 | Computer use | Unavailable | Feature gate plus platform/consent requirements | [14](14-gaps-and-unknowns.md) |
| FEAT-COWORK-13 | iOS Simulator | Unsupported | Feature gate/platform support absent | [14](14-gaps-and-unknowns.md) |
| FEAT-COWORK-14 | Remote/SSH/cloud session placement | Partial | SSH gates supported; cloud not configured; self-hosted unavailable | [14](14-gaps-and-unknowns.md) |
| FEAT-COWORK-15 | Session sharing | Available, unexercised | Share session API and UI entry | [14](14-gaps-and-unknowns.md) |

## 4. Code

| ID | Feature | Status | Evidence / notes | Spec |
|---|---|---|---|---|
| FEAT-CODE-01 | Code dashboard (`/epitaxy`) | Available | Overview stats, activity, model summary, composer | [features/code.md](features/code.md) |
| FEAT-CODE-02 | Local repository/folder selection | Available | Path picker and create session | [features/code.md](features/code.md) |
| FEAT-CODE-03 | Code sessions and worktrees | Available, unexercised | Session APIs, worktree settings, folder worktree gates | [features/code.md](features/code.md) |
| FEAT-CODE-04 | Permission modes | Available | Bypass mode toggle and policy warning | [features/code.md](features/code.md) |
| FEAT-CODE-05 | Fast mode | Available | Composer toggle | [features/code.md](features/code.md) |
| FEAT-CODE-06 | SSH/remote sessions | Available, unexercised | SSH host allowlist, client path, transport settings | [14](14-gaps-and-unknowns.md) |
| FEAT-CODE-07 | WSL sessions | Unsupported | `disableWslSessions`; Linux host | [14](14-gaps-and-unknowns.md) |
| FEAT-CODE-08 | Local sandbox and strict mode | Available | Not exercised | [09](09-settings-and-configuration.md) |
| FEAT-CODE-09 | Browser tools and dev-server preview | Available, unexercised | Code settings; browser gate | [features/code.md](features/code.md) |
| FEAT-CODE-10 | Terminal/PTY | Available, unexercised | Shell PTY lifecycle in logs | [features/code.md](features/code.md) |
| FEAT-CODE-11 | GitHub/PR integration | Partial | `/v1/code/github/` returns empty branch statuses in 3P | [18](18-api-contracts.md) |

## 5. Projects

| ID | Feature | Status | Evidence / notes | Spec |
|---|---|---|---|---|
| FEAT-PROJ-01 | Projects empty state | Available | Single New project entry; two paths inside dialog | [screens/projects.md](screens/projects.md) |
| FEAT-PROJ-02 | Create from scratch | Available | Name, instructions, folder location, Memory is on | [screens/projects.md](screens/projects.md) |
| FEAT-PROJ-03 | Use existing folder | Available | Dialog card; folder picker; project instructions with project | [features/projects.md](features/projects.md) |
| FEAT-PROJ-04 | Project detail and files | Unknown | No existing project was created | [14](14-gaps-and-unknowns.md) |
| FEAT-PROJ-05 | Project context in composer | Available | “Project or folder” selector | [features/projects.md](features/projects.md) |
| FEAT-PROJ-06 | Scheduled-task projects | Available, unexercised | `coworkScheduledTaskProjects` | [features/scheduled-tasks.md](features/scheduled-tasks.md) |

## 6. Artifacts

| ID | Feature | Status | Evidence / notes | Spec |
|---|---|---|---|---|
| FEAT-ART-01 | Artifacts library page | Available | Empty state + suggestion CTA; header “New artifact” | [features/artifacts.md](features/artifacts.md) |
| FEAT-ART-02 | Create live artifact | Available | Creates a task and begins generation immediately | [screens/new-artifact.md](screens/new-artifact.md) |
| FEAT-ART-03 | Artifact viewer/pane | Partial | `artifactsPane` unavailable; task/detail view still possible | [14](14-gaps-and-unknowns.md) |
| FEAT-ART-04 | Artifact versions, star, hide, share, export | Available, unexercised | `CoworkArtifacts` 25 methods | [features/artifacts.md](features/artifacts.md) |
| FEAT-ART-05 | Artifact rendering libraries | Available | Mermaid 11.17.2, Chart.js 4.5.0, Grid.js 5.0.2 | [features/artifacts.md](features/artifacts.md) |
| FEAT-ART-06 | Artifact preview origin / iframe | Available, unexercised | `userContentRendererUrl`, artifact iframe host | [09](09-settings-and-configuration.md) |

## 7. Scheduled tasks

| ID | Feature | Status | Evidence / notes | Spec |
|---|---|---|---|---|
| FEAT-SCHED-01 | Scheduled page | Available | Empty state, search, sort | [features/scheduled-tasks.md](features/scheduled-tasks.md) |
| FEAT-SCHED-02 | Templates | Available | Daily briefing, inbox triage, meeting prep, weekly review, content ideas, monitor topic | [features/scheduled-tasks.md](features/scheduled-tasks.md) |
| FEAT-SCHED-03 | Create/edit task | Available, unexercised | New task menu; code/scheduled APIs | [features/scheduled-tasks.md](features/scheduled-tasks.md) |
| FEAT-SCHED-04 | Keep awake | Available | Banner and toggles; `keepAwakeEnabled` | [features/scheduled-tasks.md](features/scheduled-tasks.md) |
| FEAT-SCHED-05 | Approval lifetime | Available, unexercised | `mcpScheduledTaskApprovalLifetimeDays` | [09](09-settings-and-configuration.md) |
| FEAT-SCHED-06 | Run history/status | Unknown | Not observed | [14](14-gaps-and-unknowns.md) |

## 8. Customize

| ID | Feature | Status | Evidence / notes | Spec |
|---|---|---|---|---|
| FEAT-CUST-01 | Skills list/search/filter/add | Available | Row list; Created-by-you + Anthropic/partner groups | [features/customize.md](features/customize.md) |
| FEAT-CUST-02 | Connectors list/filter/status | Available | All/Connected/Not connected; Connect action | [features/customize.md](features/customize.md) |
| FEAT-CUST-03 | Plugins list/add/discover | Available | No installed plugins; marketplace configured | [features/customize.md](features/customize.md) |
| FEAT-CUST-04 | MCP server health/error states | Available | Persistent MCP startup error observed (server name redacted) | [features/customize.md](features/customize.md) |
| FEAT-CUST-05 | Skill creation/upload | Available | `skillCreationEnabled`; user-created skills seen | [features/mcp-skills-tools.md](features/mcp-skills-tools.md) |
| FEAT-CUST-06 | Plugin marketplace management | Available, unexercised | Six configured marketplaces | [09](09-settings-and-configuration.md) |

## 9. Settings and control plane

| ID | Feature | Status | Evidence / notes | Spec |
|---|---|---|---|---|
| FEAT-SET-01 | Settings modal and section search | Available | General, Privacy, Usage, Claude Code, Cowork, Import & export, System, Developer, Customize | [features/settings.md](features/settings.md) |
| FEAT-SET-02 | Profile/instructions/appearance/notifications | Available | Theme, font, motion, response notifications | [features/settings.md](features/settings.md) |
| FEAT-SET-03 | Privacy disclosure | Available | 3P provider disclosure; crash/usage/update data | [features/updates-telemetry-privacy.md](features/updates-telemetry-privacy.md) |
| FEAT-SET-04 | Device usage | Available | Token totals, web searches, daily chart | [features/settings.md](features/settings.md) |
| FEAT-SET-05 | Cowork settings | Available | Files path, trusted folders, browser, instructions, memory | [features/cowork.md](features/cowork.md) |
| FEAT-SET-06 | System settings | Available | Version, startup, Quick Entry shortcut, tray, keep awake | [features/settings.md](features/settings.md) |
| FEAT-SET-07 | Developer settings | Available | Local MCP servers, command/args, logs | [features/settings.md](features/settings.md) |
| FEAT-SET-08 | Import/export | Policy-disabled | Import not enabled for deployment | [features/updates-telemetry-privacy.md](features/updates-telemetry-privacy.md) |
| FEAT-SET-09 | Third-party setup window | Available | Nine tabs, profiles, validation, apply | [features/third-party-setup.md](features/third-party-setup.md) |
| FEAT-SET-10 | Language dialog | Available | 11 locales | [screens/account-menu-and-language.md](screens/account-menu-and-language.md) |

## 10. Models and inference

| ID | Feature | Status | Evidence / notes | Spec |
|---|---|---|---|---|
| FEAT-MOD-01 | Model picker with provider metadata | Available | Provider, model id, “needs API key” | [features/model-selection.md](features/model-selection.md) |
| FEAT-MOD-02 | Effort selector | Available, unexercised | `low`, `medium`, `high`, `xhigh`, `max` in source; default medium | [features/model-selection.md](features/model-selection.md) |
| FEAT-MOD-03 | Extended thinking | Available | `setExtendedThinking`; transcript “Thinking” view | [features/model-selection.md](features/model-selection.md) |
| FEAT-MOD-04 | Gateway model discovery | Available | `/v1/models`, pagination, 10 s timeout, 3 pages | [features/model-selection.md](features/model-selection.md) |
| FEAT-MOD-05 | Static model list | Available | `inferenceModels` | [features/model-selection.md](features/model-selection.md) |
| FEAT-MOD-06 | Model pricing display | Policy-disabled | `inferenceModelPricingEnabled=false` | [features/model-selection.md](features/model-selection.md) |
| FEAT-MOD-07 | 1M context variant | Available, unexercised | `supports_1m` / `prefer_1m` and `[1m]` suffix | [features/model-selection.md](features/model-selection.md) |
| FEAT-MOD-08 | Provider-native Claude API/Bedrock/Vertex/Foundry | Available, unexercised | Forms and validators present; active profile is gateway | [features/third-party-setup.md](features/third-party-setup.md) |
| FEAT-MOD-09 | Current local adapter | Noncompliant | In-process translation shim, not native | [12](12-ai-architecture.md) |

## 11. Tools, MCP, skills, plugins

| ID | Feature | Status | Evidence / notes | Spec |
|---|---|---|---|---|
| FEAT-TOOL-01 | Managed MCP servers | Available | Two managed MCP servers configured (names redacted) | [features/mcp-skills-tools.md](features/mcp-skills-tools.md) |
| FEAT-TOOL-02 | Local developer MCP servers | Available | Several local developer MCP servers configured (names redacted) | [features/mcp-skills-tools.md](features/mcp-skills-tools.md) |
| FEAT-TOOL-03 | MCP tool permissions/approvals | Available, unexercised | Per-call, session, persistent “Always allow” policy | [features/mcp-skills-tools.md](features/mcp-skills-tools.md) |
| FEAT-TOOL-04 | Tool search / on-demand schemas | Available, unexercised | `toolSearchEnabled` | [features/mcp-skills-tools.md](features/mcp-skills-tools.md) |
| FEAT-TOOL-05 | Built-in tool policy | Available, unexercised | Allow/ask/deny, argument-scoped rules | [09](09-settings-and-configuration.md) |
| FEAT-TOOL-06 | Bundled skills | Available | docx, pdf, pdf-reading, pptx, xlsx, frontend-design | [features/mcp-skills-tools.md](features/mcp-skills-tools.md) |
| FEAT-TOOL-07 | User skills | Available | Created/managed in Customize → Skills | [features/customize.md](features/customize.md) |
| FEAT-TOOL-08 | Plugins | Available | Marketplace and org plugin loading | [features/customize.md](features/customize.md) |
| FEAT-TOOL-09 | Desktop extensions (`.dxt`, `.mcpb`) | Available, unexercised | Not installed during inspection | [features/mcp-skills-tools.md](features/mcp-skills-tools.md) |
| FEAT-TOOL-10 | Office365/GitHub bundled MCP binaries | Available, unexercised | Not configured in this profile | [features/mcp-skills-tools.md](features/mcp-skills-tools.md) |

## 12. Security and permissions

| ID | Feature | Status | Evidence / notes | Spec |
|---|---|---|---|---|
| FEAT-SEC-01 | File/folder consent | Available | Trusted folders, session grants, project upload folder | [features/cowork.md](features/cowork.md) |
| FEAT-SEC-02 | Egress allowlist | Available | `coworkEgressAllowedHosts`; `*` in active profile | [09](09-settings-and-configuration.md) |
| FEAT-SEC-03 | Permission modes | Available | Default/accept edits/plan/bypass; admin can disable bypass | [features/code.md](features/code.md) |
| FEAT-SEC-04 | Sandbox/VM isolation | Partial | Cowork helper/mount logs; full VM requirement flag | [features/cowork.md](features/cowork.md) |
| FEAT-SEC-05 | Credential redaction | Available | Header-name/value heuristics detect secrets | [features/third-party-setup.md](features/third-party-setup.md) |
| FEAT-SEC-06 | Origin/frame checks | Available | `app://localhost`, top-frame checks | [10](10-architecture.md) |
| FEAT-SEC-07 | Audit logs | Available | Per-session audit files | [11](11-data-and-storage.md) |
| FEAT-SEC-08 | Computer-use emergency stop | Available | Not exercised | [14](14-gaps-and-unknowns.md) |

## 13. Account, privacy, lifecycle features

| ID | Feature | Status | Evidence / notes | Spec |
|---|---|---|---|---|
| FEAT-ACCT-01 | Gateway account identity | Available | Sidebar `user · Gateway`; no Anthropic account | [features/auth-and-accounts.md](features/auth-and-accounts.md) |
| FEAT-ACCT-02 | Anthropic sign-in | Policy-disabled | “Disable Claude.ai sign-in” | [features/auth-and-accounts.md](features/auth-and-accounts.md) |
| FEAT-ACCT-03 | Profile switching | Available | Setup profile selector; relaunch semantics | [features/third-party-setup.md](features/third-party-setup.md) |
| FEAT-PRIV-01 | Third-party disclosure | Available | Explicit “not sent to Anthropic” notice | [features/updates-telemetry-privacy.md](features/updates-telemetry-privacy.md) |
| FEAT-PRIV-02 | Telemetry controls | Available | Essential/nonessential/OTel; no prompt content | [features/updates-telemetry-privacy.md](features/updates-telemetry-privacy.md) |
| FEAT-PRIV-03 | Retention controls | Available | Chat/Cowork/Code retention, legal hold | [09](09-settings-and-configuration.md) |
| FEAT-PRIV-04 | Usage limits | Available | Max tokens per window; cached tokens excluded | [09](09-settings-and-configuration.md) |
| FEAT-PRIV-05 | Import/export | Policy-disabled | Import unavailable; export switch exists | [features/updates-telemetry-privacy.md](features/updates-telemetry-privacy.md) |
| FEAT-PRIV-06 | Diagnostic support bundle | Available, unexercised | “Send to Anthropic” is explicit | [features/updates-telemetry-privacy.md](features/updates-telemetry-privacy.md) |

## 14. Deliberate limitations in this build

The following were **not** reachable. Their existence is evidence-backed only where a flag or route
is named.

| Area | Status | Gate/reason |
|---|---|---|
| Artifacts side pane | Unavailable | `artifactsPane` |
| Native Quick Entry | Unavailable | `nativeQuickEntry`, `quickEntryDictation` |
| Cowork browser | Unsupported | `coworkBrowser` (`cowork_browser_disabled`) |
| Computer use | Unavailable | `computerUse`-family gates and platform consent |
| iOS Simulator | Unsupported | simulator gates/platform |
| WSL sessions | Disabled/unsupported | `disableWslSessions`, Linux |
| Self-hosted sessions | Unavailable | `selfHosted` gates |
| Dedicated Chat tab | Unavailable | `chatTab` |
| VM virtualization (`yukonSilver`) | Unavailable | platform/hypervisor check |
| Cloud sessions | Not configured | launch placement |
| Anthropic-hosted account/org | Not available | 3P mode |
| macOS/Windows-specific behavior | Not inspected | platform |

Full flag results are in [16-feature-flag-matrix.md](16-feature-flag-matrix.md). Full unknowns are in
[14-gaps-and-unknowns.md](14-gaps-and-unknowns.md).
