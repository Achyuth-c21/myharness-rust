# 17 — Traceability, Modules, and Test Catalog

This document closes the loop from **feature → documentation → target module → acceptance test**. It
is a rebuild plan, not a claim that the original source has these modules. The original is minified; the
module names below are the proposed layout from [13](13-implementation-roadmap.md).

## 1. Status vocabulary

| Status | Meaning |
|---|---|
| Available | Reached and exercised in this environment |
| Available, unexercised | Entry point present, deeper flow not completed |
| Partial | Some states/flows observed, others not |
| Policy-disabled | Disabled by the active profile |
| Unavailable | Runtime gate says unavailable |
| Unsupported | Explicitly unsupported platform/feature |
| Unknown | Not reached or no reliable evidence |
| Noncompliant | Present locally but rejected by this blueprint |

“Available” describes the inspected installation, not all platforms or deployments.

## 2. Module map

| Short name | Target location |
|---|---|
| `renderer/*` | `apps/desktop/src/renderer/**` |
| `main/*` | `apps/desktop/src/main/**` |
| `preload` | `apps/desktop/src/preload/**` |
| `packages/provider-sdk` | Adapter interface, capability registry, conformance kit |
| `packages/protocol-core` | Internal turn/event/session types |
| `main/providers/*` | One native adapter package per provider |
| `main/mcp` | MCP lifecycle, auth, tool registry, policy |
| `main/sessions` | Session index, transcript, restore, retention |
| `main/agent` | Turn loop, tools, approvals, cancellation |
| `main/config` | Profiles, validation, secrets, setup IPC |
| `main/artifacts` | Artifact store, versions, isolation, export |
| `main/scheduler` | Cron/timezone, wake, runs, approvals |
| `main/telemetry` | Sentry/OTLP/usage with redaction |
| `main/updates` | Feed, signature, enforcement, rollback |
| `main/security` | Origin/frame checks, path/egress policy, audit |

## 3. Feature traceability matrix

Tests are IDs; ranges are inclusive. A feature with `?` status maps to the gap document and must not
be implemented from imagination. “Spec” is the primary behavior document; supporting docs are linked
from it.

| Feature ID | Status | Spec | Target module(s) | Tests |
|---|---|---|---|---|

| `FEAT-SHELL-01` | Available | [screens/new-task.md](screens/new-task.md) | `renderer/shell` | `ARCH-001/009, UX-014` |
| `FEAT-SHELL-02` | Available | [06-navigation-map.md](06-navigation-map.md) | `renderer/shell` | `UX-001/003/014` |
| `FEAT-SHELL-03` | Available | [06-navigation-map.md](06-navigation-map.md) | `renderer/shell` | `UX-001, CODE-001` |
| `FEAT-SHELL-04` | Available | [features/chat-and-composer.md](features/chat-and-composer.md) | `main/sessions` | `SESSION-001, UX-011` |
| `FEAT-SHELL-05` | Available | [features/search-and-filter.md](features/search-and-filter.md) | `main/search + renderer` | `UX-003, DATA-007` |
| `FEAT-SHELL-06` | Available | [screens/account-menu-and-language.md](screens/account-menu-and-language.md) | `renderer/settings` | `AUTH-001/005, UX-010` |
| `FEAT-SHELL-07` | Available | [screens/account-menu-and-language.md](screens/account-menu-and-language.md) | `renderer/i18n` | `I18N-001..004, UX-010` |
| `FEAT-SHELL-08` | Unavailable | [14-gaps-and-unknowns.md](14-gaps-and-unknowns.md) | `main/quick-entry` | `UX-013, ARCH-001` |
| `FEAT-SHELL-09` | Partial | [09-settings-and-configuration.md](09-settings-and-configuration.md) | `main/lifecycle` | `ARCH-009, UX-013` |
| `FEAT-SHELL-10` | Available | [features/updates-telemetry-privacy.md](features/updates-telemetry-privacy.md) | `main/updates` | `UPDATE-001/002` |
| `FEAT-SHELL-11` | Available | [08-interaction-catalog.md](08-interaction-catalog.md) | `main/menu + renderer` | `ARCH-009, UX-013` |
| `FEAT-SHELL-12` | Available, unexercised | [14-gaps-and-unknowns.md](14-gaps-and-unknowns.md) | `renderer/layout` | `UX-013, ARCH-008` |
| `FEAT-CHAT-01` | Available | [screens/new-task.md](screens/new-task.md) | `renderer/new + main/sessions` | `UX-001, SESSION-001` |
| `FEAT-CHAT-02` | Available | [features/chat-and-composer.md](features/chat-and-composer.md) | `renderer/composer + main/agent` | `UX-001/012, AI-016` |
| `FEAT-CHAT-03` | Available | [features/multimodal-attachments.md](features/multimodal-attachments.md) | `renderer/composer + main/files` | `UX-001, AI-012, SESSION-004/009` |
| `FEAT-CHAT-04` | Available | [screens/composer-menus.md](screens/composer-menus.md) | `renderer/composer + main/skills` | `UX-007, MCP-002/008` |
| `FEAT-CHAT-05` | Available | [screens/composer-menus.md](screens/composer-menus.md) | `renderer/composer + main/mcp` | `UX-007, MCP-001/002` |
| `FEAT-CHAT-06` | Available | [screens/composer-menus.md](screens/composer-menus.md) | `renderer/composer` | `UX-001, MCP-008` |
| `FEAT-CHAT-07` | Available | [screens/model-picker.md](screens/model-picker.md) | `renderer/model-picker + main/providers` | `UX-002, AI-001..010` |
| `FEAT-CHAT-08` | Available | [features/projects.md](features/projects.md) | `renderer/composer + main/projects` | `UX-004, SEC-002` |
| `FEAT-CHAT-09` | Available | [features/code.md](features/code.md) | `renderer/composer + main/policy` | `CODE-004, UX-001` |
| `FEAT-CHAT-10` | Available | [screens/task-detail.md](screens/task-detail.md) | `renderer/transcript + main/sessions` | `SESSION-001..004, AI-011/015` |
| `FEAT-CHAT-11` | Available | [features/chat-and-composer.md](features/chat-and-composer.md) | `renderer/composer + main/agent` | `AI-015, SESSION-001` |
| `FEAT-CHAT-12` | Partial | [screens/task-detail.md](screens/task-detail.md) | `renderer/transcript` | `SESSION-001, AI-014/015` |
| `FEAT-CHAT-13` | Unknown | [14-gaps-and-unknowns.md](14-gaps-and-unknowns.md) | `main/sessions` | `PRIV-001, SESSION-009` |
| `FEAT-CHAT-14` | Unavailable | [14-gaps-and-unknowns.md](14-gaps-and-unknowns.md) | `renderer/shell` | `UX-013, ARCH-001` |
| `FEAT-COWORK-01` | Available | [screens/new-task.md](screens/new-task.md) | `renderer/new` | `UX-001` |
| `FEAT-COWORK-02` | Available | [features/cowork.md](features/cowork.md) | `main/agent + sandbox` | `SESSION-002, ARCH-008, SEC-004` |
| `FEAT-COWORK-03` | Available | [screens/task-detail.md](screens/task-detail.md) | `renderer/transcript + main/sessions` | `SESSION-001..004, AI-015` |
| `FEAT-COWORK-04` | Available | [screens/scheduled-tasks.md](screens/scheduled-tasks.md) | `main/scheduler + renderer` | `UX-006, SCHED-001..006` |
| `FEAT-COWORK-05` | Available | [screens/projects.md](screens/projects.md) | `main/projects + renderer` | `UX-004, SESSION-005` |
| `FEAT-COWORK-06` | Available | [screens/artifacts.md](screens/artifacts.md) | `main/artifacts + renderer` | `UX-005, ART-001` |
| `FEAT-COWORK-07` | Partial | [screens/new-artifact.md](screens/new-artifact.md) | `main/artifacts + main/sessions` | `ART-002/003/005` |
| `FEAT-COWORK-08` | Available | [features/cowork.md](features/cowork.md) | `main/memory + renderer` | `PRIV-001, DATA-004/005` |
| `FEAT-COWORK-09` | Available | [features/cowork.md](features/cowork.md) | `main/files + policy` | `SEC-002, CFG-009` |
| `FEAT-COWORK-10` | Available, unexercised | [features/cowork.md](features/cowork.md) | `main/spaces` | `DATA-002/004, SEC-002` |
| `FEAT-COWORK-11` | Unsupported | [14-gaps-and-unknowns.md](14-gaps-and-unknowns.md) | `main/browser` | `UX-013, SEC-001` |
| `FEAT-COWORK-12` | Unavailable | [14-gaps-and-unknowns.md](14-gaps-and-unknowns.md) | `main/computer-use` | `UX-013, PRIV-001` |
| `FEAT-COWORK-13` | Unsupported | [14-gaps-and-unknowns.md](14-gaps-and-unknowns.md) | `main/simulator` | `UX-013` |
| `FEAT-COWORK-14` | Partial | [14-gaps-and-unknowns.md](14-gaps-and-unknowns.md) | `main/sessions` | `CODE-006, SEC-003` |
| `FEAT-COWORK-15` | Available, unexercised | [14-gaps-and-unknowns.md](14-gaps-and-unknowns.md) | `main/sessions` | `PRIV-001/003, SEC-001` |
| `FEAT-CODE-01` | Available | [screens/code-dashboard.md](screens/code-dashboard.md) | `renderer/code + main/local-sessions` | `CODE-001` |
| `FEAT-CODE-02` | Available | [screens/code-dashboard.md](screens/code-dashboard.md) | `main/files + renderer` | `CODE-002, SEC-002` |
| `FEAT-CODE-03` | Available, unexercised | [features/code.md](features/code.md) | `main/local-sessions` | `CODE-003, SESSION-010` |
| `FEAT-CODE-04` | Available | [features/code.md](features/code.md) | `main/policy + renderer` | `CODE-004, SEC-001` |
| `FEAT-CODE-05` | Available | [features/code.md](features/code.md) | `renderer/code + main/providers` | `CODE-005, AI-008` |
| `FEAT-CODE-06` | Available, unexercised | [14-gaps-and-unknowns.md](14-gaps-and-unknowns.md) | `main/ssh` | `CODE-006, SEC-003` |
| `FEAT-CODE-07` | Unsupported | [14-gaps-and-unknowns.md](14-gaps-and-unknowns.md) | `main/wsl` | `CODE-006, UX-013` |
| `FEAT-CODE-08` | Available | [09-settings-and-configuration.md](09-settings-and-configuration.md) | `main/sandbox + policy` | `SEC-004, CFG-009` |
| `FEAT-CODE-09` | Available, unexercised | [features/code.md](features/code.md) | `main/browser + mcp` | `MCP-007, SEC-001` |
| `FEAT-CODE-10` | Available, unexercised | [features/code.md](features/code.md) | `main/pty + renderer` | `CODE-007, ARCH-008` |
| `FEAT-CODE-11` | Partial | [18-api-contracts.md](18-api-contracts.md) | `main/github` | `ARCH-004, CODE-001` |
| `FEAT-PROJ-01` | Available | [screens/projects.md](screens/projects.md) | `renderer/projects` | `UX-004` |
| `FEAT-PROJ-02` | Available | [screens/projects.md](screens/projects.md) | `main/projects + renderer` | `UX-004, SEC-002` |
| `FEAT-PROJ-03` | Available | [features/projects.md](features/projects.md) | `main/projects + main/files` | `UX-004, SEC-002` |
| `FEAT-PROJ-04` | Unknown | [14-gaps-and-unknowns.md](14-gaps-and-unknowns.md) | `renderer/project-detail` | `UX-004, DATA-004` |
| `FEAT-PROJ-05` | Available | [features/projects.md](features/projects.md) | `renderer/composer + main/projects` | `UX-001/004` |
| `FEAT-PROJ-06` | Available, unexercised | [features/scheduled-tasks.md](features/scheduled-tasks.md) | `main/scheduler + projects` | `SCHED-001/006, CFG-009` |
| `FEAT-ART-01` | Available | [screens/artifacts.md](screens/artifacts.md) | `renderer/artifacts + main/artifacts` | `ART-001` |
| `FEAT-ART-02` | Available | [screens/new-artifact.md](screens/new-artifact.md) | `main/artifacts + sessions` | `ART-002, UX-005` |
| `FEAT-ART-03` | Partial | [14-gaps-and-unknowns.md](14-gaps-and-unknowns.md) | `renderer/artifact-viewer` | `ART-005, SEC-001` |
| `FEAT-ART-04` | Available, unexercised | [features/artifacts.md](features/artifacts.md) | `main/artifacts` | `ART-004` |
| `FEAT-ART-05` | Available | [features/artifacts.md](features/artifacts.md) | `renderer/artifact-render` | `ART-005, SEC-001` |
| `FEAT-ART-06` | Available, unexercised | [features/artifacts.md](features/artifacts.md) | `main/protocol + renderer` | `ART-005, ARCH-005` |
| `FEAT-SCHED-01` | Available | [screens/scheduled-tasks.md](screens/scheduled-tasks.md) | `renderer/scheduled` | `UX-006, SCHED-001` |
| `FEAT-SCHED-02` | Available | [screens/scheduled-tasks.md](screens/scheduled-tasks.md) | `renderer/scheduled` | `UX-006, SCHED-001` |
| `FEAT-SCHED-03` | Available, unexercised | [features/scheduled-tasks.md](features/scheduled-tasks.md) | `main/scheduler + renderer` | `SCHED-001/002` |
| `FEAT-SCHED-04` | Available | [screens/scheduled-tasks.md](screens/scheduled-tasks.md) | `main/power + renderer` | `SCHED-005` |
| `FEAT-SCHED-05` | Available, unexercised | [09-settings-and-configuration.md](09-settings-and-configuration.md) | `main/policy` | `SCHED-006` |
| `FEAT-SCHED-06` | Unknown | [14-gaps-and-unknowns.md](14-gaps-and-unknowns.md) | `main/scheduler` | `SCHED-003/004` |
| `FEAT-CUST-01` | Available | [screens/customize.md](screens/customize.md) | `renderer/customize + main/skills` | `UX-007` |
| `FEAT-CUST-02` | Available | [screens/customize.md](screens/customize.md) | `renderer/customize + main/mcp` | `UX-007, MCP-001` |
| `FEAT-CUST-03` | Available | [screens/customize.md](screens/customize.md) | `renderer/customize + main/plugins` | `UX-007, EXT-001/002` |
| `FEAT-CUST-04` | Available | [screens/customize.md](screens/customize.md) | `main/mcp + renderer` | `MCP-001/009` |
| `FEAT-CUST-05` | Available | [features/customize.md](features/customize.md) | `main/skills + policy` | `UX-007, MCP-008` |
| `FEAT-CUST-06` | Available, unexercised | [09-settings-and-configuration.md](09-settings-and-configuration.md) | `main/plugins + policy` | `EXT-001, CFG-006` |
| `FEAT-SET-01` | Available | [screens/settings.md](screens/settings.md) | `renderer/settings` | `UX-008` |
| `FEAT-SET-02` | Available | [features/settings.md](features/settings.md) | `renderer/settings + main/config` | `UX-008/014` |
| `FEAT-SET-03` | Available | [features/updates-telemetry-privacy.md](features/updates-telemetry-privacy.md) | `renderer/settings + main/telemetry` | `PRIV-001/002` |
| `FEAT-SET-04` | Available | [features/settings.md](features/settings.md) | `renderer/usage + main/usage` | `SESSION-008, DATA-007` |
| `FEAT-SET-05` | Available | [features/cowork.md](features/cowork.md) | `renderer/settings + main/memory/files` | `UX-008, DATA-004` |
| `FEAT-SET-06` | Available | [features/settings.md](features/settings.md) | `renderer/settings + main/lifecycle` | `UX-008, ARCH-009` |
| `FEAT-SET-07` | Available | [features/settings.md](features/settings.md) | `renderer/settings + main/mcp` | `UX-008, MCP-001/009` |
| `FEAT-SET-08` | Policy-disabled | [features/updates-telemetry-privacy.md](features/updates-telemetry-privacy.md) | `renderer/settings + main/import` | `UX-008, PRIV-002` |
| `FEAT-SET-09` | Available | [screens/setup-desktop-3p.md](screens/setup-desktop-3p.md) | `renderer/setup + main/config` | `UX-009, CFG-001..012` |
| `FEAT-SET-10` | Available | [screens/account-menu-and-language.md](screens/account-menu-and-language.md) | `renderer/i18n` | `UX-010, I18N-001..004` |
| `FEAT-MOD-01` | Available | [screens/model-picker.md](screens/model-picker.md) | `renderer/model-picker` | `UX-002` |
| `FEAT-MOD-02` | Available, unexercised | [features/model-selection.md](features/model-selection.md) | `renderer/composer + main/agent` | `UX-002, AI-009` |
| `FEAT-MOD-03` | Available | [features/model-selection.md](features/model-selection.md) | `main/agent + renderer/transcript` | `AI-009, UX-012` |
| `FEAT-MOD-04` | Available | [features/model-selection.md](features/model-selection.md) | `main/providers/gateway` | `CFG-011, AI-018` |
| `FEAT-MOD-05` | Available | [features/model-selection.md](features/model-selection.md) | `main/config` | `CFG-004/011` |
| `FEAT-MOD-06` | Policy-disabled | [features/model-selection.md](features/model-selection.md) | `renderer/model-picker` | `CFG-004` |
| `FEAT-MOD-07` | Available, unexercised | [features/model-selection.md](features/model-selection.md) | `main/agent + providers` | `UX-002, AI-001` |
| `FEAT-MOD-08` | Available, unexercised | [features/third-party-setup.md](features/third-party-setup.md) | `main/providers native` | `UX-009, CFG-004, AI-002..005` |
| `FEAT-MOD-09` | Noncompliant | [12-ai-architecture.md](12-ai-architecture.md) | `negative/local-adapter` | `AI-022/023/024` |
| `FEAT-TOOL-01` | Available | [features/mcp-skills-tools.md](features/mcp-skills-tools.md) | `main/mcp` | `MCP-001/006/009` |
| `FEAT-TOOL-02` | Available | [features/mcp-skills-tools.md](features/mcp-skills-tools.md) | `main/mcp` | `MCP-001/009` |
| `FEAT-TOOL-03` | Available, unexercised | [features/mcp-skills-tools.md](features/mcp-skills-tools.md) | `main/mcp + policy` | `MCP-003/005` |
| `FEAT-TOOL-04` | Available, unexercised | [features/mcp-skills-tools.md](features/mcp-skills-tools.md) | `main/mcp` | `MCP-002` |
| `FEAT-TOOL-05` | Available, unexercised | [09-settings-and-configuration.md](09-settings-and-configuration.md) | `main/policy` | `MCP-004/005` |
| `FEAT-TOOL-06` | Available | [features/mcp-skills-tools.md](features/mcp-skills-tools.md) | `main/skills` | `MCP-002/008` |
| `FEAT-TOOL-07` | Available | [features/customize.md](features/customize.md) | `main/skills + renderer` | `UX-007, MCP-008` |
| `FEAT-TOOL-08` | Available | [features/customize.md](features/customize.md) | `main/plugins` | `EXT-001/002/006/008` |
| `FEAT-TOOL-09` | Available, unexercised | [features/mcp-skills-tools.md](features/mcp-skills-tools.md) | `main/extensions` | `EXT-001/002/005` |
| `FEAT-TOOL-10` | Available, unexercised | [features/mcp-skills-tools.md](features/mcp-skills-tools.md) | `resources/mcp` | `MCP-001/006` |
| `FEAT-SEC-01` | Available | [features/cowork.md](features/cowork.md) | `main/files + policy` | `SEC-002, UX-004` |
| `FEAT-SEC-02` | Available | [09-settings-and-configuration.md](09-settings-and-configuration.md) | `main/egress` | `MCP-007, SEC-003` |
| `FEAT-SEC-03` | Available | [features/code.md](features/code.md) | `main/policy` | `CODE-004, SEC-001` |
| `FEAT-SEC-04` | Partial | [features/cowork.md](features/cowork.md) | `main/sandbox` | `SEC-004, ARCH-008` |
| `FEAT-SEC-05` | Available | [features/third-party-setup.md](features/third-party-setup.md) | `main/config + redaction` | `CFG-006, SEC-004` |
| `FEAT-SEC-06` | Available | [10-architecture.md](10-architecture.md) | `preload + protocol` | `ARCH-001/003, SEC-001` |
| `FEAT-SEC-07` | Available | [11-data-and-storage.md](11-data-and-storage.md) | `main/audit` | `DATA-006, SESSION-006` |
| `FEAT-SEC-08` | Available | [14-gaps-and-unknowns.md](14-gaps-and-unknowns.md) | `main/computer-use` | `UX-013, PRIV-001` |
| `FEAT-ACCT-01` | Available | [features/auth-and-accounts.md](features/auth-and-accounts.md) | `main/config + renderer` | `AUTH-001` |
| `FEAT-ACCT-02` | Policy-disabled | [features/auth-and-accounts.md](features/auth-and-accounts.md) | `main/auth` | `AUTH-002, CFG-003` |
| `FEAT-ACCT-03` | Available | [features/third-party-setup.md](features/third-party-setup.md) | `main/config` | `AUTH-006, CFG-001/002` |
| `FEAT-PRIV-01` | Available | [features/updates-telemetry-privacy.md](features/updates-telemetry-privacy.md) | `renderer/privacy` | `PRIV-001/002` |
| `FEAT-PRIV-02` | Available | [features/updates-telemetry-privacy.md](features/updates-telemetry-privacy.md) | `main/telemetry` | `PRIV-001..003` |
| `FEAT-PRIV-03` | Available | [09-settings-and-configuration.md](09-settings-and-configuration.md) | `main/retention` | `DATA-004/005, CFG-009` |
| `FEAT-PRIV-04` | Available | [09-settings-and-configuration.md](09-settings-and-configuration.md) | `main/usage + policy` | `CFG-009, DATA-007` |
| `FEAT-PRIV-05` | Policy-disabled | [features/updates-telemetry-privacy.md](features/updates-telemetry-privacy.md) | `main/import` | `UX-008, PRIV-002` |
| `FEAT-PRIV-06` | Available, unexercised | [features/updates-telemetry-privacy.md](features/updates-telemetry-privacy.md) | `main/support-bundle` | `PRIV-003, SEC-004` |

## 4. Test catalog

Core suites are defined with their criteria in [13](13-implementation-roadmap.md), except `TEST-AI`,
which is defined in [12](12-ai-architecture.md). This table is the index; the additional suites below
close the previously referenced but undefined groups.

| Prefix | Defined in | Count / range | Scope |
|---|---|---|---|
| `TEST-ARCH-*` | [13](13-implementation-roadmap.md) §6.2 | 001–010 | Shell, protocol, security defaults |
| `TEST-CFG-*` | [13](13-implementation-roadmap.md) §6.2 | 001–012 | Config profiles, setup, credentials |
| `TEST-AI-*` | [12](12-ai-architecture.md) §10 | 001–024 | Native provider adapters |
| `TEST-SESSION-*` | [13](13-implementation-roadmap.md) §6.2 | 001–010 | Session/transcript/usage |
| `TEST-MCP-*` | [13](13-implementation-roadmap.md) §6.2 | 001–010 | MCP/tools/policy |
| `TEST-UX-*` | [13](13-implementation-roadmap.md) §6.2 | 001–014 | Screen interaction/visual |
| `TEST-PRIV-*` | [13](13-implementation-roadmap.md) §6.2 | 001–004 | Telemetry/privacy |
| `TEST-UPDATE-*` | [13](13-implementation-roadmap.md) §6.2 | 001–002 | Update verification/rollback |
| `TEST-SEC-*` | [13](13-implementation-roadmap.md) §6.2 | 001–004 | Origin, paths, deep links, secrets |
| `TEST-DATA-*` | this document | 001–010 | Persistence, retention, audit, export |
| `TEST-SCHED-*` | this document | 001–006 | Scheduler/wake/runs/approvals |
| `TEST-ART-*` | this document | 001–006 | Artifacts library/capture/isolation |
| `TEST-CODE-*` | this document | 001–008 | Code dashboard/sessions/PTY |
| `TEST-EXT-*` | this document | 001–008 | Extension/provider packages |
| `TEST-AUTH-*` | this document | 001–006 | Local identity/credentials/sign-out |
| `TEST-I18N-*` | this document | 001–004 | Locale catalog/application/formatting |

### 4.1 Data and retention (`TEST-DATA`)

| ID | Criterion |
|---|---|
| `TEST-DATA-001` | Schema migrations are forward-only, idempotent, and preserve unknown fields where safe |
| `TEST-DATA-002` | Session/usage/config writes are atomic; a crash leaves a recoverable previous state |
| `TEST-DATA-003` | Corrupt/partial records are quarantined with an error, never silently deleted |
| `TEST-DATA-004` | Retention deletes only expired session artifacts/transcripts and honors legal hold |
| `TEST-DATA-005` | Legal hold survives restart and blocks deletion/GC until explicitly released |
| `TEST-DATA-006` | Audit HMAC validates; tampering is detected and reported |
| `TEST-DATA-007` | Usage events preserve input/output/cache/search/error and aggregate 7/30/90 correctly |
| `TEST-DATA-008` | Backup/restore round-trips config and sessions without secrets in the backup by default |
| `TEST-DATA-009` | Export/support bundle redacts credentials, prompts, paths, hosts, and account identifiers |
| `TEST-DATA-010` | Concurrent writers do not lose updates; WAL/transaction behavior is tested |

### 4.2 Scheduled tasks (`TEST-SCHED`)

| ID | Criterion |
|---|---|
| `TEST-SCHED-001` | Create/edit validates prompt, schedule, model, context, and permissions; cancel creates nothing |
| `TEST-SCHED-002` | Cron/timezone/DST produce the correct next run for each template |
| `TEST-SCHED-003` | A skipped run records asleep/offline/disabled reason and is not counted as success |
| `TEST-SCHED-004` | A run creates exactly one normal session and usage record; failure is visible and retryable |
| `TEST-SCHED-005` | Keep-awake prevents idle sleep only; it cannot claim cloud execution or survive power-off |
| `TEST-SCHED-006` | Approval lifetime expires per policy; stale approvals are not reused; revocation works |

### 4.3 Artifacts (`TEST-ART`)

| ID | Criterion |
|---|---|
| `TEST-ART-001` | Library empty/populated/search/no-results states match; New artifact is explicit |
| `TEST-ART-002` | New artifact creates exactly one task and starts one generation; double-click does not duplicate |
| `TEST-ART-003` | Stop/failure leaves a task, not a phantom completed artifact; capture is idempotent |
| `TEST-ART-004` | Versions/star/hide/share/export/delete update metadata and content consistently |
| `TEST-ART-005` | Viewer renders untrusted HTML/SVG in an isolated origin with CSP and no main bridge |
| `TEST-ART-006` | Session deletion/retention removes artifacts and previews per policy, honoring legal hold |

### 4.4 Code (`TEST-CODE`)

| ID | Criterion |
|---|---|
| `TEST-CODE-001` | Dashboard stats derive from the local ledger; range/tab states and empty state are correct |
| `TEST-CODE-002` | Folder/worktree selection is canonical, policy-checked, and reflected in metadata |
| `TEST-CODE-003` | Code session/worktree metadata survives restart; deletion does not delete uncommitted work |
| `TEST-CODE-004` | Permission modes honor policy; bypass is hidden/blocked when disabled |
| `TEST-CODE-005` | Fast mode is enabled only for capable models; unsupported selection is blocked visibly |
| `TEST-CODE-006` | Remote/SSH/WSL placement never silently falls back to local |
| `TEST-CODE-007` | PTY start/resize/stop and app quit drain correctly without orphan processes |
| `TEST-CODE-008` | Code retention/cleanup previews and deletes only eligible files; progress is accurate |

#### 4.4.1 Phase 6 Slice 1 status (2026-10-25)

| ID | Status | Where / limitation |
|---|---|---|
| `TEST-CODE-001` | Available (unit + bridge integration + mocked renderer) | `crates/sessions/tests/code_usage.rs` (4 tests), `crates/bridge/tests/chat.rs::code_dashboard_derives_only_from_code_rows`, E2E `T28b`/`T28c`. The populated renderer is exercised with a mocked bridge; a running Tauri window has not been exercised. |
| `TEST-CODE-002` | Available (unit + bridge integration); native picker compile-verified | `crates/bridge/src/policy.rs` (canonicalization, allowlist, control/relative/missing paths), `crates/bridge/tests/chat.rs::code_creation_persists_surface_folder_model_and_permission` and `code_creation_rejects_bad_folders_and_policy_disabled_modes`, `pick_workspace_folder` in `apps/desktop/src-tauri/src/lib.rs`. `rfd` cancellation returns `None`; a browser preview reports `BRIDGE_UNAVAILABLE`. |
| `TEST-CODE-003` | Partial | Code session `cwd`, model, permission mode, and `extra.originCwd` persist and reopen. Worktree lifecycle, worktree metadata, and “deletion does not delete uncommitted work” are **deferred** (no worktree implementation). |
| `TEST-CODE-004` | Available (unit + bridge integration); Accept-edits limited | `crates/agent/tests/tools.rs` Plan/Bypass/Accept-edits tests; `crates/bridge/tests/chat.rs` rejects stored Bypass when policy disables it and unknown stored modes. Bypass is permitted only by explicit policy `false`; a policy Deny still wins. Accept-edits has no production edit-tool set yet. |
| `TEST-CODE-005` | Policy-disabled / deferred | Fast is rendered visibly disabled with no capability claim; no model fast-mode capability exists in this slice. |
| `TEST-CODE-006` | Unavailable / deferred | No SSH/WSL/remote placement path exists; local is explicit and the only implemented placement. |
| `TEST-CODE-007` | Unavailable / deferred | No PTY or terminal integration in this slice. |
| `TEST-CODE-008` | Unavailable / deferred | No Code file-retention/cleanup path in this slice. |
| `TEST-SESSION-010` | Unavailable / deferred | Code worktree lifecycle was not started; screen-level Code navigation uses `TEST-CODE-001..003`. |
| `TEST-UX-001` | Partial | E2E `T28`/`T28b`/`T28c`/`T28d` cover the distinct greeting, honest desktop-only state, disabled Bypass/Fast, the cancelled/error folder-picker path, a populated dashboard with real-shaped bridge data, and a populated Code transcript with folder/permission/model metadata. The native picker and a real Tauri window are not exercised end-to-end. |

#### 4.4.2 Phase 6 slices 2–5 status (2026-10-26)

Projects, Artifacts, Scheduled, and Search are implemented on the
`phase-6-code` lane (commits `35958b8`, `8aa7987`, `48eb6ac`, `424eabc`).
Gates at completion: workspace 485 cargo tests, clippy clean on the touched
crates, `cargo metadata`, `tsc --noEmit`, `vite build`, E2E `shell.spec.mjs`
47/47 (T01–T47), `setup-window.spec.mjs` 16/16. Not exercised end-to-end
against a live Tauri window (the standing renderer limitation): the native
folder picker, reveal-in-file-manager, and a real scheduled firing (the tick
loop is covered by bridge tests, not a live process restart).

| ID | Status | Where / notes |
|---|---|---|
| `TEST-UX-004` (Projects screens) | Available (unit + bridge + mocked renderer) | `crates/projects` (5 service tests), `crates/bridge/tests/chat.rs` project battery (4 tests: CRUD + availability, folder/allowlist refusals, session binding + path-as-data block, deletion orphans), E2E `T39`–`T41`. Empty state, choice dialog, and form copy are captured verbatim; the populated list and detail are the unobserved rebuild design (`docs/14` §9 item 59). |
| `TEST-SESSION-005` (project-adjacent) | Partial | Deleting a project removes only the project row; sessions keep their cwd and stale association (`crates/bridge/tests/chat.rs::deleting_a_project_leaves_its_sessions_and_reports_the_count`). Legal-hold interplay with project deletion is not applicable (projects are not sessions). |
| `TEST-UX-005` (Artifacts screens) | Available (bridge + mocked renderer) | `crates/bridge/tests/chat.rs::an_artifact_task_is_one_marked_session_the_library_can_filter`; E2E `T42`/`T43` (honest preview, one click = one task, search/no-results). Populated rows are a rebuild design; the viewer is Unavailable (`docs/14` §9 item 62). |
| `TEST-ART-001` | Available (mocked renderer) | E2E `T42`/`T43` cover empty/populated/search/no-results; `New artifact` is explicit. Native populated state not exercised. |
| `TEST-ART-002` | Available (mocked renderer) | One `createArtifactTask` per click; double-click is guarded in the renderer and E2E asserts exactly one task (`T43`). The shipped auto-send is not reproduced (no captured prompt) — recorded deviation, `docs/14` §9 item 62. |
| `TEST-ART-003` | Available by construction | The library derives rows from real session rows only; nothing can show a completed artifact because no content capture exists. Stop/failure leaves the task row with its real status. |
| `TEST-ART-004` | Unavailable | No artifact content model, versions, star/hide/share/export in this slice. |
| `TEST-ART-005` | Unavailable | The artifactsPane gate was unavailable during inspection; the rebuild states the viewer unavailable instead of emulating it (`REQ-ART-06`). |
| `TEST-ART-006` | Unavailable | No artifact content exists to remove; session deletion follows the Phase 4 delete path. |
| `TEST-UX-006` (Scheduled screens) | Available (unit + bridge + mocked renderer) | `crates/scheduler` (5 unit tests incl. a hand-rolled DST gap zone), `crates/store/tests/store.rs` (schema v4 pin), `crates/bridge/tests/chat.rs` scheduled battery (3 tests), E2E `T44`/`T45`. Captured copy: banner, empty state, six template cards, /schedule hint. Editor and populated rows are the unobserved rebuild design (`docs/14` §9 item 63). |
| `TEST-SCHED-001` | Available (bridge) | Create/edit validates name/prompt bounds and the cron subset before storage; templates only prefill (`crates/bridge/tests/chat.rs::scheduled_tasks_crud_arms_previews_and_refuses_bad_schedules`, E2E `T45`). Model/context/permission editing arrives with the model picker; the context is project/folder with the same validation. |
| `TEST-SCHED-002` | Available (unit + bridge) | `crates/scheduler` next-run arithmetic over any `chrono::TimeZone` (zone-correctness, weekday sets, Sunday alias, ranges); spring-forward gap skips the missing minute, fall-back takes the earlier occurrence; the editor preview uses the same arithmetic (`previewSchedule`). |
| `TEST-SCHED-003` | Available (bridge) | An occurrence missed past the 60 s grace is a recorded skip with the truthful reason, never executed late; a skip does not stamp `last_run_at` (`crates/bridge/tests/chat.rs::a_missed_occurrence_is_a_recorded_skip_and_a_due_one_runs`). |
| `TEST-SCHED-004` | Available (bridge) | A run creates exactly one normal session (`extra.scheduledTaskId`/`scheduledRunId`) + the prompt as the user turn + the terminal run row; a failure records the provider's message and stays visible/retryable via Run now. |
| `TEST-SCHED-005` | Unavailable | Keep-awake ships disabled with the truthful explanation (`docs/14` §9 item 64). |
| `TEST-SCHED-006` | Available (mechanism) | Scheduled runs execute with the scheduled approval context (`mcpScheduledTaskApprovalLifetimeDays`; `0` offers only Once) via `run_turn_with_permission(..., scheduled=true, ...)` — the Phase 5 mechanism exercised on the scheduled path. Revocation/expiry tests remain the Phase 5 battery's. |
| `TEST-UX-003` (Search/filter) | Available (mocked renderer) | E2E `T46` (palette over real local data: recent set, live narrowing across sessions+projects, keyboard open) and `T47` (filter menu composes AND, badge, reset, filtered-empty vs true empty, disabled Unread). Metadata-only search; transcript-content search and pinning are not built (`docs/14` §9 item 65). |

### 4.5 Extensions/providers (`TEST-EXT`)

| ID | Criterion |
|---|---|
| `TEST-EXT-001` | Manifest schema/signature/integrity/min-version are verified; unsigned packages are rejected |
| `TEST-EXT-002` | Lazy loading isolates a broken adapter; app boot and other providers still work |
| `TEST-EXT-003` | Each adapter passes its native-wire fixture and has no cross-protocol translation |
| `TEST-EXT-004` | Static checks forbid global fetch patching and arbitrary translated URLs |
| `TEST-EXT-005` | Credentials are scoped per provider/profile and inaccessible to other adapters |
| `TEST-EXT-006` | Egress is declared in the manifest and enforced at request time |
| `TEST-EXT-007` | Registration adds setup fields/UI without hard-coding the provider in the agent/shell |
| `TEST-EXT-008` | A policy-blocked model fails closed; no substitution or policy bypass |

### 4.6 Auth/accounts (`TEST-AUTH`)

| ID | Criterion |
|---|---|
| `TEST-AUTH-001` | Local deployment identity is displayed without claiming an Anthropic account |
| `TEST-AUTH-002` | Anthropic sign-in is hidden/disabled when policy disables it; no hidden auth call |
| `TEST-AUTH-003` | Credential kinds are exclusive; invalid combinations are rejected at apply |
| `TEST-AUTH-004` | Helper-script timeout/failure does not fall back to a stored key; error is actionable |
| `TEST-AUTH-005` | Sign-out clears the documented credential/session scope and does not pretend success |
| `TEST-AUTH-006` | Profile switch/relaunch guard preserves running work or explains why it must stop |

### 4.7 Internationalization (`TEST-I18N`)

| ID | Criterion |
|---|---|
| `TEST-I18N-001` | Exactly the 11 observed locales are offered with autonym + English label |
| `TEST-I18N-002` | Selection applies immediately and persists; restart requirement is explained if needed |
| `TEST-I18N-003` | Missing strings fall back to English, never render raw message IDs |
| `TEST-I18N-004` | Dates/numbers/plurals use the selected locale; untranslated content is labeled |

### 4.8 Configuration (`TEST-CFG`)

Two ID blocks live in the `TEST-CFG` area, and they must not be confused:

- **`TEST-CFG-001..012`** — the canonical acceptance criteria, fixed by `docs/13` §6.2.
  The table below is the status of those criteria. It is the contract; it is not
  renumbered by implementation slices.
- **`TEST-CFG-101..112`** — the `crates/config` library's own mechanics (draft
  lifecycle, atomicity, unknown-key preservation, permissions). These were numbered
  `001..012` in an earlier working draft, which collided with the `docs/13` §6.2
  criteria above; the collision was resolved on 2026-10-22 by moving the library
  rows into their own block rather than redefining the acceptance criteria.

Fixture: unit tests in `crates/config/src/lib.rs` and `crates/config/src/forms.rs`
(`cargo test -p myharness-config`).

#### 4.8.1 `docs/13` §6.2 acceptance criteria

| ID | Criterion | Status | Where |
|---|---|---|---|
| `TEST-CFG-001` | Profile apply is atomic; invalid config leaves old profile intact | Available (unit) | `crates/config` — `invalid_apply_is_rejected_and_previous_profile_stays_active`, `apply_refuses_an_incomplete_profile_and_keeps_the_previous_one`, `failed_write_leaves_previous_document_and_active_untouched` |
| `TEST-CFG-002` | Draft discard restores applied values exactly | Available, unexercised (contract) | Renderer-side draft session; the library guarantees the applied document is never mutated by a draft write, but the discard path is a UI behavior that has not been exercised |
| `TEST-CFG-003` | Bootstrap override is read-only and visibly labeled | Partial | The read-only enforcement and the per-field source/lock state are Available (unit — `crates/config::overlay`, `crates/config::health`); the "visibly labeled" half is a renderer behavior and the bootstrap **fetch** is not implemented |
| `TEST-CFG-004` | Every provider family validates its required fields, and model ids validate against the open-model shape rule only (a non-empty id — no family gate since 2026-10-23, §4.14; the Vertex family is out of rebuild scope — §4.13) | Available (unit) | `crates/config::forms` — `every_provider_requires_its_own_fields`, `open_model_id_rule`, `openai_family_validation`, `custom_family_validation` |
| `TEST-CFG-005` | Credential kind is exclusive; helper failure does not fall back | Available (unit) | Exclusivity is structural (enum, `credential_kind_subset_is_enforced`); the helper runner fails closed with no fallback (`crates/secrets::helper`). The provider-side use of the resolved credential is Phase 3 |
| `TEST-CFG-006` | Secrets are keychain-backed and absent from renderer storage/logs | Partial | `crates/secrets` ships the keychain store and handle layer (`KeyringStore` runtime is compile-verified only — `docs/14` §9 item 8); the renderer-storage half is a bridge concern and is not yet implemented |
| `TEST-CFG-007` | Export/support bundle warns and redacts by default | Available (unit) | `crates/config::export` — the bundle enumerates its contents, requires explicit consent, and redacts secrets, credential-shaped values, URL query strings and private hosts (`docs/09` §4) |
| `TEST-CFG-008` | Egress derivation excludes private hosts from published docs/fixtures | Available (unit) | `crates/config::egress` derives the allowlist from the active config; `crates/config::export` redacts private hosts out of the exported list, and no private host appears in the crate's own fixtures |
| `TEST-CFG-009` | Retention/legal-hold controls persist and enforce | Unknown | Limits tab + `crates/store`; no implementation |
| `TEST-CFG-010` | Managed config recheck/relaunch windows behave as configured | Partial | The re-check interval, the healthy/tainted state machine and the local-fallback decision are Available (unit — `crates/config::health`); the fetch transport and the relaunch window are not implemented |
| `TEST-CFG-011` | Model discovery 3-page/10 s/partial behavior matches contract | Available (unit) | `crates/bridge::helper` — the shipped protocol (`GET <base>/v1/models?limit=1000`, `after_id` cursor, at most three pages, 10 s per page, a later-page failure keeps what was read and warns, a first-page failure is an error, ids de-duplicated). The probe is exercised against a mock server |
| `TEST-CFG-012` | Catalog signature/expiry/floor failure uses bundled fallback | Partial | The stored envelope, the staleness/expiry decision and the bundled fallback with a non-blocking warning are implemented (`crates/bridge::helper::catalog_state`). The **signature check** is not: the shipped public key was never captured, so a stored catalog is treated as unverified and the documented failure path is taken (`docs/14` §9 item 20) |

#### 4.8.2 `crates/config` library mechanics

| ID | Criterion | Status |
|---|---|---|
| `TEST-CFG-101` | A created profile is a draft: `appliedId` stays unset until `apply` | Available (unit) |
| `TEST-CFG-102` | `apply` flips `appliedId` atomically; `active` reports the applied entry | Available (unit) |
| `TEST-CFG-103` | Unknown profile keys (top-level and in sections, e.g. legacy `claudeAiImport` tails) survive read-modify-write | Available (unit) |
| `TEST-CFG-104` | Apply validates: unknown `schemaVersion` (own or foreign-writer file) refuses with the prior profile still active | Available (unit) |
| `TEST-CFG-105` | A disk-level write failure leaves the prior document and active selection untouched (temp+rename atomicity) | Available (unit) |
| `TEST-CFG-106` | Profile and `_meta.json` files are owner-only (`0600`, POSIX) | Available (unit) |
| `TEST-CFG-107` | Credential kind is exclusive by construction (enum, no fallback pair), kebab-case on the wire | Available (unit) |
| `TEST-CFG-108` | `duplicate` yields an independent draft (fresh id, name explicit) and never moves the active selection | Available (unit) |
| `TEST-CFG-109` | `rename` moves document + meta entry; duplicate names are refused | Available (unit) |
| `TEST-CFG-110` | `delete` refuses the applied profile (`CONFIG_IN_USE`); inactive profiles delete cleanly | Available (unit) |
| `TEST-CFG-111` | Profile switch moves only the pointer; neither document mutates | Available (unit) |
| `TEST-CFG-112` | Wire format matches the observed camelCase (`appliedId`, `entries`, `schemaVersion`); unknown `_meta.json`/entry keys and entry order are preserved | Available (unit) |

#### 4.8.3 Provider form schemas (`crates/config::forms`)

Shipped by the Phase-2 forms slice. The provider families each carry
their own field set; since the open model layer (owner decision, 2026-10-23) the
model-ID rule is one shared shape-only predicate, `validate_open_model_id`
(§4.14) — the per-family name gates are gone. The shipped sixth family,
Vertex, was removed from rebuild scope — §4.13); the shared connection fields
(custom headers, model discovery/list, effort, catalog, proxy) are common to all
families. Five field key names are documented in `docs/09` §2.2 but were never
captured in the key index (`docs/20` §2.1) — they are recorded in `docs/14` §9 and
listed in `crates/config::forms::UNVERIFIED_KEYS`.

| ID | Criterion | Status |
|---|---|---|
| `TEST-CFG-121` | Each provider family refuses its own missing required fields and accepts its own complete form | Available (unit) |
| `TEST-CFG-122` | The five supported families do not share a key set (a Gateway key never appears in a Foundry/Bedrock form) | Available (unit) |
| `TEST-CFG-123` | The model-ID predicate is the open-model shape rule (a non-empty id) for every family — the former family-name gates were removed with the 2026-10-23 open model layer (§4.14) | Available (unit) |
| `TEST-CFG-124` | Bedrock ids validate under the open-model shape rule only; the former `anthropic.*`/ARN gate was removed with the open model layer (§4.14; the Vertex `claude-` policy was withdrawn with the family — §4.13) | Available (unit) |
| `TEST-CFG-125` | Custom inference headers carry routing metadata only — credential-shaped names and values are refused (`TEST-SEC-004` adjacency) | Available (unit) |
| `TEST-CFG-126` | Model discovery is gated per credential kind where the docs state it (mantle static/helper; the Vertex combination was withdrawn with the family — §4.13) | Available (unit) |

#### 4.8.4 Managed configuration (`crates/config::overlay`, `crates/config::health`)

The managed (bootstrap) overlay is merged over the applied local profile. Precedence
is a rebuild decision — the shipped precedence was never proven (`docs/09` §3.5):
`managed overlay > active local profile > runtime defaults`. Managed fields are
read-only, and that is enforced on write server-side, not only by disabling the
control in the UI (`docs/18` §10).

| ID | Criterion | Status |
|---|---|---|
| `TEST-CFG-113` | The merged view reports the effective source (local / bootstrap / organization) and the lock state per field (`DATA-CONFIG-04`) | Available (unit) |
| `TEST-CFG-114` | A managed field's effective value is the overlay's, and the field is read-only | Available (unit) |
| `TEST-CFG-115` | A local write that changes a managed field is refused (`CONFIG_MANAGED`); a write that leaves it alone is accepted | Available (unit) |
| `TEST-CFG-116` | A failed health check marks a landed managed configuration tainted and **retains** the landed overlay | Available (unit) |
| `TEST-CFG-117` | A successful re-check repairs the taint and replaces the overlay | Available (unit) |
| `TEST-CFG-118` | The re-check interval is `configRecheckIntervalMinutes` (default 10 minutes) and drives the schedule (`DATA-CONFIG-09`) | Available (unit) |
| `TEST-CFG-119` | `disableLocalConfigCache` forbids the stale-local-copy fallback when the managed source is unreachable | Available (unit) |

#### 4.8.5 Credential helper script (`crates/secrets::helper`)

The helper-script credential kind (`docs/09` §2.2): an external command returns
the credential. The helper's output is treated as data — a credential value, never
an instruction (`docs/12` §5.2) — and a failure never falls back to another
credential source (`TEST-CFG-005`).

| ID | Criterion | Status |
|---|---|---|
| `TEST-CFG-127` | A successful helper's stdout is the credential, and it is written to the keychain rather than to a profile | Available (unit) |
| `TEST-CFG-128` | A failing helper is an error with no fallback, and the error carries neither stdout nor stderr | Available (unit) |
| `TEST-CFG-129` | The configured timeout is enforced and a hanging helper is killed | Available (unit) |
| `TEST-CFG-130` | The TTL and silent-refresh policy is enforced: a cached credential inside its TTL is reused, a stale one without silent refresh is an error | Available (unit) |
| `TEST-CFG-131` | An empty helper output is a failure, not an empty credential | Available (unit) |

#### 4.8.6 Egress derivation and export redaction (`crates/config::egress`, `crates/config::export`)

The Egress tab is a read-only allowlist recomputed from the active configuration
(`docs/09` §2.9; `docs/20` §6). The export bundle is redacted by default and
enumerates its contents (`docs/09` §4; `TEST-CFG-007`).

| ID | Criterion | Status |
|---|---|---|
| `TEST-CFG-141` | The derived allowlist contains the documented groups and the hosts the docs name verbatim | Available (unit) |
| `TEST-CFG-142` | A group whose hostnames were never captured is emitted with `hostsUnobserved`, and no hostname is invented | Available (unit) |
| `TEST-CFG-143` | The provider endpoint follows the `docs/12` §1 default host shape, and the Bedrock control-plane hosts are derived from the region | Available (unit) |
| `TEST-CFG-144` | A managed overlay value wins in the derivation — the list describes what the app will actually reach | Available (unit) |
| `TEST-CFG-145` | A secret-valued key, a credential-shaped header entry and a URL query string are redacted in the export | Available (unit) |
| `TEST-CFG-146` | Private, loopback, link-local and metadata hosts are redacted from the exported egress list | Available (unit) |
| `TEST-CFG-147` | The bundle enumerates every file it contains and requires explicit consent | Available (unit) |

#### 4.8.7 Setup-tab schemas and the setup bridge namespace

The setup window is a separate window with nine tabs (`docs/09` §2.1–§2.10).
`crates/config::tabs` holds the form schema of every tab except Connection
(which varies per provider family and lives in `crates/config::forms`), and
`crates/bridge::setup` is the `settings.Custom3pSetup` namespace
(`docs/18` §3.4) with argument validation in the bridge and redaction on every
response.

| ID | Criterion | Status |
|---|---|---|
| `TEST-CFG-151` | Every tab has a schema, owns a distinct profile section, and the Egress tab is read-only | Available (unit) |
| `TEST-CFG-152` | Egress host entries must be parseable hosts, and `*` is special-cased (`docs/09` §3.4) | Available (unit) |
| `TEST-CFG-153` | `0` keeps its explicit meaning for the approval lifetime and the relaunch window; it is not "unset" | Available (unit) |
| `TEST-CFG-154` | `claudeAiImport` is refused — it is policy-disabled in third-party mode (`docs/09` §1.7; `RULES.md` §10) | Available (unit) |
| `TEST-CFG-155` | `otlpContentCapture` is refused — attachment content is never uploaded for telemetry (`RULES.md` §2) | Available (unit) |
| `TEST-CFG-156` | The bootstrap config URL must be https, and trusting bootstrap delivery warns that the profile becomes organization-managed | Available (unit) |
| `TEST-CFG-157` | A profile the renderer reads carries no secret value; a renderer write carrying an inline secret is refused (`docs/18` §10) | Available (unit) |
| `TEST-CFG-158` | Bridge arguments are validated before the library is touched, and an unknown tab or provider is refused rather than guessed | Available (unit) |
| `TEST-CFG-159` | A draft can be validated without being persisted | Available (unit) |
| `TEST-CFG-160` | The tab and provider form schemas are addressable from the bridge | Available (unit) |

#### 4.8.8 Model discovery and the published catalog (`crates/bridge::helper`)

The `settings.Custom3pHelperRun` namespace (`docs/18` §3.4): the setup-time probes.
The discovery client speaks the gateway's own `/v1/models` contract — it is a
probe of a provider-native endpoint, not a translation layer (`RULES.md` §2).

| ID | Criterion | Status |
|---|---|---|
| `TEST-CFG-171` | Discovery reads `GET <base>/v1/models?limit=1000` with `anthropic-version: 2023-06-01`, the configured auth scheme and the custom headers | Available (unit) |
| `TEST-CFG-172` | The cursor is followed, and the page cap is three | Available (unit) |
| `TEST-CFG-173` | A later-page failure keeps the rows already read and warns; a first-page failure is an error | Available (unit) |
| `TEST-CFG-174` | Duplicate ids collapse — the list is de-duplicated by stable id | Available (unit) |
| `TEST-CFG-175` | The catalog's expiry and staleness are decided, and a catalog that cannot be trusted falls back to the bundled copy with a non-blocking warning | Available (unit) |
| `TEST-CFG-176` | A gateway error body never reaches an error message | Available (unit) |

Rows owed by later Phase-2 slices: managed/read-only field enforcement, whole-profile renderer
redaction and export redaction (`crates/secrets`, slice 2 shipped the handle/resolution layer:
`SecretRef`/`SecretSlot::resolve`, fail-closed; `KeyringStore` runtime is compile-verified only —
`docs/14` §9 item 8), `getMerged`/health/recheck, profile-delete secret orchestration, the
helper-script runner, and the eight non-Connection tab schemas — tracked per-slice in
`docs/14` §9 supplements.

### 4.9 Setup window (`TEST-UX-009`)

The third-party inference setup window is a **separate window**, not a Settings tab and not a
modal (`docs/09` §2.1; `docs/screens/setup-desktop-3p.md`). It is the surface the
`settings.Custom3pSetup` bridge namespace (§4.8.7) drives.

| ID | Criterion | Status | Where |
|---|---|---|---|
| `TEST-UX-009` | Setup tabs, provider switch, apply and discard behave as specified | Partial | The window renders (header with the title, profile selector and Export; a left navigation with a search box and nine entries; a scrolling content pane; a fixed Apply Changes footer), the draft is in-memory until Apply, Discard restores the applied values, and the provider switch is edit-only. The renderer behavior is exercised by `tests/e2e/setup-window.spec.mjs` (S01–S16) against a **stubbed bridge**, the window was driven in Chromium and checked against the captured layout, and the native fixture proves the dedicated window is really created. **Not exercised**: the window has never been driven against a *live* configuration library and the OS keychain, so the credential path is proven at the bridge seam only. The spec row itself lives in `docs/13` §6.2 |
| `TEST-CFG-161` | The setup route renders only inside the setup window; the main window redirects away from it | Available (unit) | `apps/desktop/src/App.tsx` (`SetupRoute` checks the window label) |
| `TEST-CFG-162` | A credential the user types is stored in the keychain and the document carries the handle; the value never returns to the renderer | Available (unit) | `crates/bridge::setup` (`apply_secrets`, `set_secret`) |
| `TEST-CFG-163` | A credential the user did not re-enter keeps the stored value — the renderer never received it, so it cannot send it back | Available (unit) | `crates/bridge::setup` |
| `TEST-CFG-164` | A renderer write that carries an inline secret is refused before anything is written | Available (unit) | `crates/bridge::setup` (`reject_inline_secrets`) |
| `TEST-CFG-165` | The Egress tab is read-only and shows the derived allowlist; a group whose hostnames were never captured says so rather than listing guesses | Available (unit) | `crates/config::egress`, `apps/desktop/src/setup/SetupWindow.tsx` |
| `TEST-CFG-166` | The deep link `setup-desktop-3p` opens the dedicated window and never navigates the main renderer (ARCH-NAV-03) | Available (unit + native) | `apps/desktop/src-tauri/src/lib.rs`; the native fixture asserts the window was created (log proof) and that the main renderer was not navigated |
| `TEST-CFG-167` | The setup window renders the captured layout: title, profile selector, Export, nine nav entries, a scrolling content pane and a fixed Apply footer | Available (E2E) | `tests/e2e/setup-window.spec.mjs` S01–S03 |
| `TEST-CFG-168` | A clean draft leaves Apply disabled; an edit makes it dirty; Discard restores the applied values | Available (E2E) | `tests/e2e/setup-window.spec.mjs` S05–S07 |
| `TEST-CFG-169` | Apply writes the draft and then applies the profile, and the written document carries a credential **handle**, never a value | Available (E2E) | `tests/e2e/setup-window.spec.mjs` S08–S09 |
| `TEST-CFG-170` | The Egress tab is read-only, shows the derived list, and says which group had no captured hostnames | Available (E2E) | `tests/e2e/setup-window.spec.mjs` S10 |
| `TEST-CFG-177` | The main window never renders the setup surface — the route redirects away from it | Available (E2E) | `tests/e2e/setup-window.spec.mjs` S12 |
| `TEST-CFG-178` | The setup window shows none of the main app's chrome — the captured window is standalone | Available (E2E) | `tests/e2e/setup-window.spec.mjs` S13 |
| `TEST-CFG-179` | A select shows the stored value with its label (the credential kind is stored as `static`, shown as "Static API key") | Available (E2E) | `tests/e2e/setup-window.spec.mjs` S14 |
| `TEST-CFG-180` | The export bundle is a modal over a scrim and does not cover the footer's Apply action | Available (E2E) | `tests/e2e/setup-window.spec.mjs` S15 |
| `TEST-CFG-181` | The layout does not overflow a narrow window, and the credential reveal control stays reachable | Available (E2E) | `tests/e2e/setup-window.spec.mjs` S16 |

Available since Phase 3 slice 1: the Connection tab's **Test connection** performs a real
one-token inference through the native adapter of the draft's family (`crates/provider-gateway`
for the `gateway` family, `crates/provider-anthropic` for the `anthropic` family — slice 2; the
`settings.Custom3pSetup::testConnection` command resolves the draft's typed credential or
stored slot, picks the target model, and reports usage, latency and the native stop reason —
§4.10/§4.11/§4.12/§4.14 for the wire rows). The button is enabled for the `gateway`, `anthropic`,
`bedrock`, `openai`, and `custom` families; the `mantle`/`foundry` families keep it disabled
rather than faked until their adapters land (`docs/14` §9 item 29). The family itself is chosen
on the Connection tab's provider-family picker (net-new with the open model layer; blank stays
unset; switching keeps the shared fields, resets the family-specific ones, and reloads the
connection schema — §4.14). The Egress tab's **Test connectivity** button remains
disabled (no live-egress probe has been decided). The credential-helper flow is implemented in
`crates/secrets::helper` and is used by the probe whenever a draft of a family that offers it
selects the helper credential kind; the model-discovery action is family-aware: the gateway
family walks the shipped `crates/bridge::helper` probe, and the open model layer's
`openai`/`custom` families call the additive `setup_discover_openai_models` through
`discoverOpenAiModels` (`docs/14` §9 item 32); the Claude API family's setup card offers a
model list, and its discovery lane is reached through the probe's fallback, not a button.

### 4.10 Native provider adapters — gateway (`TEST-AI`)

The gateway adapter is Phase 3 slice 1 (`docs/13` §5): the **native protocol wall** of `docs/12`
§8 for the `gateway` family. The wire battery runs against a mock gateway that speaks the
real HTTP contract (chunked SSE, status codes, auth headers) — `crates/providers/gateway/tests/wire.rs`
— and against the adapter's own unit tests (`crates/providers/gateway/src/tests.rs`). Every row
also carries the `AI-ADAPTER-*` regression assertions it forbids (`docs/12` §7).

| ID | Criterion | Status | Where |
|---|---|---|---|
| `TEST-AI-005` | The gateway contract: `POST /v1/messages` SSE + `GET /v1/models?limit=1000` cursor pagination (3 pages, 10 s), `anthropic-version` header, bearer or `x-api-key` auth, custom headers forwarded | Available (unit + wire) | `crates/providers/gateway/tests/wire.rs` (`a_streamed_turn_speaks_the_native_wire`, `an_x_api_key_turn_authenticates_natively`, `discovery_follows_the_cursor_and_retains_partial_results`) |
| `TEST-AI-006` | Credential resolution is exclusive: a missing or empty credential blocks the send with zero requests; nothing is substituted | Available (unit) | `crates/providers/gateway` (`a_missing_credential_blocks_the_send`); the setup-path rule is `crates/bridge::setup::resolve_probe_credential` + `test_connection_blocks_without_a_credential_and_without_a_request` |
| `TEST-AI-007` | No model-ID family policy: a non-Anthropic id travels verbatim to the provider — the open model layer's owner decision (§4.14); the refusal, where one exists, is the provider's own wire error | Available (wire) | `crates/providers/gateway/tests/wire.rs` (`a_non_anthropic_id_travels_verbatim`); the draft-selection application is `test_connection_sends_the_first_draft_entry_verbatim` (`crates/bridge::setup`) |
| `TEST-AI-008` | A missing/unknown model blocks the send; no alternate model is used; the model echo mismatch terminates visible (`AI-MODEL-01`, `AI-ADAPTER-11`) | Available (unit) | `crates/providers/gateway` (`a_missing_credential…` echo check + `a_model_echo_mismatch_stops_the_stream`) |
| `TEST-AI-009` | Thinking / redacted-thinking blocks survive the stream whole (`AI-ADAPTER-04`) | Available (unit) | `redacted_thinking_survives_the_stream` |
| `TEST-AI-010` | Prompt-caching controls and cache usage round-trip exactly (`AI-USAGE-01`, `AI-ADAPTER-05`) | Available (unit) | `cache_usage_is_exact` |
| `TEST-AI-011` | Tool calls preserve ids, name, streamed argument deltas and the assembled arguments (`AI-TOOL-01`) | Available (unit) | `tool_arguments_stream_and_close` |
| `TEST-AI-012` | Tool-result images reach the provider; unsupported block kinds are named, never dropped (`AI-MM-01/02`, `AI-ADAPTER-06`) | Available (unit) | `tool_result_images_reach_the_provider`, `an_unsubscribed_assistant_block_is_named` |
| `TEST-AI-013` | Token counting is native (`POST /v1/messages/count_tokens`); unavailable counting is an error, never an estimate (`AI-ADAPTER-09`) | Available (unit) | `token_counting_is_native_or_refused` |
| `TEST-AI-014` | Stop reasons are preserved verbatim, including refusal / content-filter / unknown spellings | Available (unit) | `native_stop_reasons_are_preserved` |
| `TEST-AI-015` | Dropping the stream cancels the request; the partial output already emitted is what the caller keeps (`AI-STREAM-02`) | Available (unit) | `dropping_the_stream_cancels_it` |
| `TEST-AI-016` | A stream that only sends keep-alives times out at the configured idle window; `ping` never resets it | Available (unit) | `a_pigeonhole_stream_times_out_at_the_idle_window`, `a_hung_stream_is_bounded` |
| `TEST-AI-018` | Discovery: first-page failure is fatal; later-page failure retains the rows already read and warns; the page cap stays 3 | Available (unit) | `discovery_follows_the_cursor_and_retains_partial_results` |
| `TEST-AI-021` | Provider error classes stay distinguishable, and no raw body ever lands in a message (`docs/18` §11) | Available (unit) | `provider_error_classes_are_distinguishable` |
| `TEST-AI-022` | No fetch interception and no routed URL: two adapters configured to different endpoints share no state (`AI-ADAPTER-01/02/13`) | Available (unit) | `requests_travel_only_to_the_configured_endpoint` |
| `TEST-AI-023` | The native-wire conformance fixture runs against the real mock gateway (this table) | Available (unit) | the whole `crates/providers/gateway/tests/wire.rs` battery |
| `TEST-SETUP-CFG-001` (working id) | The setup path resolves the credential exactly once — typed value wins over the stored slot; a helper credentials runs once and is never stored; a stored slot resolves fail-closed (`SECRET_MISSING`); an interactive credential is refused, not emulated | Available (unit) | `crates/bridge::setup` (`test_connection_uses_the_typed_value_over_the_stored_slot`, `test_connection_resolves_the_stored_slot_fail_closed`, `test_connection_refuses_interactive_without_a_request`, `test_connection_runs_the_helper_kind_without_persisting_it`) |
| `TEST-SETUP-CFG-002` (working id) | The probe performs a real inference: usage, latency and the native stop reason reach the renderer; a draft of a family whose adapter has not landed yet is refused before anything is sent | Available (unit) | `crates/bridge::setup` (`test_connection_sends_a_real_inference`, `test_connection_refuses_an_unreleased_family_draft`) |

**Working ids:** `TEST-SETUP-CFG-001/002` name the setup-path rows until `docs/13` renumbers
the `TEST-CFG` extension range; they are not self-assigned numbers in a released suite.

**Deferred rows** (`Available, unexercised` until their adapters exist): `TEST-AI-004`
(Foundry), `TEST-AI-017` (catalog signature — `docs/14` §9 item 20),
`TEST-AI-019/020` (MCP approval/egress — Phase 3 slice 2+), `TEST-AI-024` (MiniMax).
`TEST-AI-002` (Bedrock) left this list in Phase 3 slice 3 — §4.12.
`TEST-AI-003` (Vertex) left this list in Phase 3 slice 4 — §4.13 — and on 2026-10-23
the family was removed from rebuild scope at the owner's direction; its rows are
**Unsupported**, not deferred.

### 4.11 Native provider adapters — Claude API (`TEST-AI`)

The `anthropic` family's adapter is Phase 3 slice 2 (`docs/13` §5): the same **native protocol
wall** of `docs/12` §8, now for the first-party Anthropic Messages host. The wire battery runs
against a mock provider that speaks the real HTTP contract (`crates/providers/anthropic/tests/wire.rs`)
and against the adapter's own unit tests (`crates/providers/anthropic/src/tests.rs`). The crate
shares no implementation with the gateway crate — only the `myharness-provider-sdk` contract —
so every row below is exercised independently (`AI-PROVIDER-01`: each adapter owns its own
native client).

| ID | Criterion | Status | Where |
|---|---|---|---|
| `TEST-AI-001` | The Claude API family's native contract: `POST /v1/messages` (streaming and not) on the first-party host (or the configured base URL — the field's spelling stays a `docs/14` §9 unknown), `x-api-key` auth and `anthropic-version: 2023-06-01`, betas forwarded, custom headers forwarded | Available (unit + wire) | `crates/providers/anthropic/tests/wire.rs` (`a_streamed_turn_speaks_the_native_wire`, `a_non_streamed_turn_returns_typed_events`, `the_probe_inference_is_real`) |
| `TEST-AI-006` | Credential resolution is exclusive for this family too: an empty credential blocks the send with zero requests (`AI-ADAPTER-12`) | Available (wire) | `crates/providers/anthropic/tests/wire.rs` (`a_missing_credential_blocks_the_send`) |
| `TEST-AI-007` | No model-ID family policy for this family either: a non-Anthropic id travels verbatim — the open model layer's owner decision (§4.14) | Available (wire) | `crates/providers/anthropic/tests/wire.rs` (`a_non_anthropic_id_travels_verbatim`) |
| `TEST-AI-008` | The model echo mismatch terminates the stream visibly for this family (`AI-MODEL-01`, `AI-ADAPTER-11`) | Available (wire) | `crates/providers/anthropic/tests/wire.rs` (`a_model_echo_mismatch_stops_the_stream`) |
| `TEST-AI-009` | Redacted thinking is preserved whole (`AI-ADAPTER-04`) | Available (wire) | `crates/providers/anthropic/tests/wire.rs` (`redacted_thinking_survives_the_stream`) |
| `TEST-AI-010` | Usage is exact and separate per channel — input, output, cache read/write, web search — and never estimated (`AI-USAGE-01`, `AI-ADAPTER-05`) | Available (wire) | `crates/providers/anthropic/tests/wire.rs` (`cache_usage_is_exact`) |
| `TEST-AI-011` | Tool calls: ids, names, streamed argument fragments, assembled arguments (`AI-TOOL-01`) | Available (wire) | `crates/providers/anthropic/tests/wire.rs` (`tool_arguments_stream_and_close`) |
| `TEST-AI-012` | Tool-result images and documents travel as such on the wire; unmodeled block kinds are named (`AI-MM-01/02`, `AI-ADAPTER-06`) | Available (wire) | `crates/providers/anthropic/tests/wire.rs` (`tool_result_images_reach_the_provider`, `an_unsubscribed_assistant_block_is_named`) |
| `TEST-AI-013` | Counting is native (`POST /v1/messages/count_tokens`); a missing endpoint is an error, never an estimate (`AI-ADAPTER-09`) | Available (wire) | `crates/providers/anthropic/tests/wire.rs` (`token_counting_is_native_or_refused`) |
| `TEST-AI-014` | Native stop reasons preserved 1:1, including refusal / pause_turn / unknown spellings | Available (wire) | `crates/providers/anthropic/tests/wire.rs` (`native_stop_reasons_are_preserved`) |
| `TEST-AI-015` | Dropping the stream cancels the request (`AI-STREAM-02`) | Available (wire) | `crates/providers/anthropic/tests/wire.rs` (`dropping_the_stream_cancels_it`) |
| `TEST-AI-016` | `ping`-only streams time out at the idle window; content events reset it (`docs/14` §9 items 22, 25) | Available (wire) | `crates/providers/anthropic/tests/wire.rs` (`a_pigeonhole_stream_times_out_at_the_idle_window`) |
| `TEST-AI-018` | Discovery (`GET /v1/models`, **no injected limit param** — `docs/14` §9 item 24): first-page failure fatal, later-page failure retains rows, the cap stays 3 pages | Available (wire) | `crates/providers/anthropic/tests/wire.rs` (`discovery_follows_the_cursor_and_retains_partial_results`) |
| `TEST-AI-021` | Provider error envelopes map onto distinguishable classes; provider wording is kept, raw bodies are not | Available (wire) | `crates/providers/anthropic/tests/wire.rs` (`provider_error_classes_are_distinguishable`) |
| `TEST-AI-022` | Requests travel only to the configured endpoint; two adapters share nothing global (`AI-ADAPTER-01/02/13`) | Available (wire) | `crates/providers/anthropic/tests/wire.rs` (`requests_travel_only_to_the_configured_endpoint`) |
| `TEST-AI-023` | The native-wire conformance fixture runs against the real mock provider (this table) | Available (wire) | the whole `crates/providers/anthropic/tests/wire.rs` battery |
| `TEST-SETUP-CFG-001` (working id) | The setup path resolves the Claude API credential strictly in the family's own slot (`inferenceAnthropicApiKey`): typed value wins, a stored handle resolves fail-closed, and the `helper-script` kind is refused outright — the family offers static/interactive only (`docs/12` §1) | Available (unit) | `crates/bridge::setup` (`claude_api_probe_uses_the_family_slot`, `claude_api_probe_refuses_helper_and_interactive_without_a_request`) |
| `TEST-SETUP-CFG-002` (working id) | The Claude API probe performs a real inference against the family's host, with a discovery fallback when the draft names no model; probes of families without adapters are refused | Available (unit) | `crates/bridge::setup` (`test_connection_probes_the_claude_api_family`, `claude_api_probe_discovers_when_the_draft_has_no_models`, `test_connection_refuses_an_unreleased_family_draft`) |

**Not implemented for this family, recorded as unknowns** (`docs/14` §9 item 24): the
first-party listing's page size (no `limit` is invented), the exact response-row grammar
(the parse is shape-tolerant), and interactive-OAuth probing (`sk-ant-oat…` credentials are
refused — the beta header value was never captured, `docs/14` §9 item 23).

### 4.12 Native provider adapters — Bedrock (`TEST-AI`)

The `bedrock` family's adapter is Phase 3 slice 3 (`docs/13` §5): the **native protocol
wall** of `docs/12` §8 for the Bedrock Runtime surface — the family whose payload grammar
is the Anthropic Messages one carried over the cloud's binary event-stream transport.
The wire battery runs against a mock provider that serves real EventStream frames with
verified CRCs (`crates/providers/bedrock/tests/wire.rs`) and against the adapter's own
unit tests (`crates/providers/bedrock/src/tests.rs`, including the published CRC-32C
check value and a hand-assembled frame). Sharing stays exactly the
`myharness-provider-sdk` contract — no code shared with the gateway or Claude API
crates (`AI-PROVIDER-01`). Every wire choice is a reconstructed reading recorded in
`docs/14` §9 item 27; nothing from this family's wire was ever observed.

| ID | Criterion | Status | Where |
|---|---|---|---|
| `TEST-AI-001` / `TEST-AI-002` | The Bedrock family's native contract: `POST /model/{id-or-arn}/invoke[-with-response-stream]` on `bedrock-runtime.{region}.amazonaws.com` (or the configured override — one override answers both surfaces), `Authorization: Bearer` auth, custom headers forwarded, the `anthropic_version` body declaration, **no `model` field in the body** (the path carries it) | Available (unit + wire) | `crates/providers/bedrock/tests/wire.rs` (`a_streamed_turn_speaks_the_native_wire`, `the_probe_inference_is_real`, `an_arn_model_travels_encoded_in_the_path`) |
| `TEST-AI-006` | Credential resolution is exclusive for this family too: an empty credential blocks the send with zero requests (`AI-ADAPTER-12`) | Available (wire) | `crates/providers/bedrock/tests/wire.rs` (`a_missing_credential_blocks_the_send`) |
| `TEST-AI-007` | No model-ID family policy for this family either: any id travels verbatim (an ARN still travels percent-encoded in the path) — the open model layer's owner decision (§4.14) | Available (wire) | `crates/providers/bedrock/tests/wire.rs` (`a_non_bedrock_id_travels_verbatim`, `an_arn_model_travels_encoded_in_the_path`) |
| `TEST-AI-008` | The model echo mismatch terminates the stream visibly when the answer names a model; an answer without one is not a refusal (`AI-MODEL-01`; `docs/14` §9 item 27) | Available (wire) | `crates/providers/bedrock/tests/wire.rs` (`a_model_echo_mismatch_stops_the_stream`, `the_probe_inference_is_real`) |
| `TEST-AI-009` | Redacted thinking is preserved whole; thinking deltas and signatures ride the stream (`AI-ADAPTER-04`) | Available (wire) | `crates/providers/bedrock/tests/wire.rs` (`redacted_thinking_survives_the_stream`) |
| `TEST-AI-010` | Usage is exact and separate per channel across `message_start`/`message_delta` — input, output, cache read/write, web search (`AI-USAGE-01`, `AI-ADAPTER-05`) | Available (wire) | `crates/providers/bedrock/tests/wire.rs` (`cache_usage_is_exact`) |
| `TEST-AI-011` | Tool calls: ids, names, streamed `input_json_delta` fragments, assembled arguments close the call (`AI-TOOL-01`) | Available (wire) | `crates/providers/bedrock/tests/wire.rs` (`tool_arguments_stream_and_close`) |
| `TEST-AI-012` | Tool-result images travel as images on the wire; unmodeled block kinds are named (`AI-MM-01/02`, `AI-ADAPTER-06`) | Available (wire) | `crates/providers/bedrock/tests/wire.rs` (`tool_result_images_reach_the_provider`, `an_unsubscribed_assistant_block_is_named`) |
| `TEST-AI-013` | The family surface has **no native token-count endpoint**: the refusal is explicit, zero requests travel, no estimate is ever produced (`AI-ADAPTER-09`) | Available (wire) | `crates/providers/bedrock/tests/wire.rs` (`token_counting_has_no_native_endpoint`) |
| `TEST-AI-014` | Native stop reasons preserved 1:1, including `pause_turn` and unknown spellings (`AI-ADAPTER-08`) | Available (wire) | `crates/providers/bedrock/tests/wire.rs` (`native_stop_reasons_are_preserved`) |
| `TEST-AI-015` | Dropping the stream cancels the request (`AI-STREAM-02`) | Available (wire) | `crates/providers/bedrock/tests/wire.rs` (`dropping_the_stream_cancels_it`) |
| `TEST-AI-016` | Empty-payload frames are keep-alives that never reset the idle window; a stream of only keep-alives times out (`docs/14` §9 item 27) | Available (wire) | `crates/providers/bedrock/tests/wire.rs` (`a_pigeonhole_stream_times_out_at_the_idle_window`) |
| `TEST-AI-018` | Discovery: `GET {control}/foundation-models` with **no invented parameters**, `nextToken` cursor, first-page failure fatal, later-page failure retains rows, the cap stays 3 pages | Available (wire) | `crates/providers/bedrock/tests/wire.rs` (`discovery_walks_the_control_plane_and_keeps_partial_rows`) |
| `TEST-AI-021` | Bedrock exception envelopes (`__type`/`reason`/wrapped `Output`) map onto distinguishable classes; provider wording is kept, raw bodies are not | Available (wire) | `crates/providers/bedrock/tests/wire.rs` (`provider_error_classes_are_distinguishable`) |
| `TEST-AI-022` | Requests travel only to the configured endpoints — the runtime invoke paths and the control plane's listing, nothing else (`AI-ADAPTER-01/02/13`) | Available (wire) | `crates/providers/bedrock/tests/wire.rs` (`requests_travel_only_to_the_configured_endpoints`) |
| `TEST-AI-023` | The native-wire conformance fixture runs against the real mock provider (this table) | Available (wire) | the whole `crates/providers/bedrock/tests/wire.rs` battery |
| `TEST-SETUP-CFG-001` (working id) | The setup path resolves the Bedrock credential strictly in the family's own slot (`inferenceBedrockBearerToken`): typed value wins, a stored handle resolves fail-closed, the helper-script kind runs once and is never stored, and the identity kinds (`sso`, `external-idp`, `vendor-profile`) are refused, not emulated (`docs/14` §9 item 27) | Available (unit) | `crates/bridge::setup` (`test_connection_probes_the_bedrock_family`, `test_connection_bedrock_refuses_the_identity_kinds`, `test_connection_bedrock_runs_the_helper_kind`) |
| `TEST-SETUP-CFG-002` (working id) | The Bedrock probe forms the endpoints from the required region (absent region → `CONFIG_VALIDATION`, zero requests), with a discovery fallback when the draft names no model; probes of families without adapters are refused | Available (unit) | `crates/bridge::setup` (`test_connection_probes_the_bedrock_family`, `test_connection_bedrock_requires_a_region`, `test_connection_bedrock_discovers_when_the_draft_has_no_models`) |

**Not implemented for this family, recorded as unknowns** (`docs/14` §9 item 27): the
native wire itself (paths, payload spellings, exception grammar are reconstructions), the
`SigV4`/profile credential kinds the form offers but whose sign-in flows were never
captured (refused, not emulated), a beta transport (a beta-carrying request is refused
visibly rather than silently dropped), and a native token-count endpoint (refused, never
estimated).

### 4.13 Native provider adapters — Vertex (`TEST-AI`)

**Removed from rebuild scope (owner decision, 2026-10-23).** The Phase-3 slice-4
adapter was implemented and then deleted from the codebase at the owner's direction:
the `crates/providers/vertex` crate (21 wire fixtures, 10 unit tests), the
`test_connection_vertex` bridge lane and its tests, the Vertex family in
`crates/config` (form fields, `validate_vertex_model_id`, egress/export keys,
`ProviderKind::Vertex`), and the renderer's probe unlock.

Status per the `docs/17` vocabulary: the family's rebuild rows
(`TEST-AI-003`, `TEST-AI-006..016`, `TEST-AI-021..023`, and the two
`TEST-SETUP-CFG` rows scoped to this family) are **Unsupported** — a deliberate
scope drop for the *rebuild*, not a failure. The shipped app does carry the family
(`docs/09` §2.2, `docs/12` §1 stay as observations). The reconstructed native-wire
reading the slice had pinned remains recorded as an unknown in `docs/14` §9 item 28,
which carries a removal annotation. Nothing of the family is wired anywhere: the
workspace has 25 members (`docs/21`), the probe refuses the family up front
(`crates/bridge::setup` — `test_connection_refuses_an_unreleased_family_draft`),
and no `inferenceVertex*` key exists in config, export, or egress. The open model layer
adds `crates/providers/openai` afterward — the workspace is now 26 members
(owner pivot, 2026-10-23 — §4.14).

Pre-removal state preserved at commit `7b84886` on the `phase-3` lane. Not pushable
under the standing docs policy; the code side is carried in commits only.

### 4.14 Native provider adapters — the open model layer (`TEST-AI`)

The owner decision of 2026-10-23 (`docs/14` §9 items 29–32): the harness speaks any
model any provider serves, plus a user-defined OpenAI-compatible endpoint. Wire-wise
this added one **native wall** for the whole OpenAI Chat Completions wire —
`crates/providers/openai` (`myharness-provider-openai`) — serving two families off
one wire: first-party `openai` (base URL optional, default `https://api.openai.com/v1`;
static key required) and `custom` (base URL required; credential OPTIONAL, and a
keyless send carries no `authorization` header at all — `docs/14` §9 item 30). The
crate shares nothing with the gateway/Claude API/Bedrock crates beyond the
`myharness-provider-sdk` contract (`AI-PROVIDER-01`). The **model-ID family gates
were removed everywhere** (`docs/14` §9 item 29): the shared `TEST-AI-007` rows in
§4.10–§4.12 now pin "travels verbatim" instead of a policy refusal, and
`crates/config`'s validators are shape-only (`TEST-CFG-004`, `TEST-CFG-123`,
`TEST-CFG-124`). The `mantle`/`foundry` families remain stubbed: their forms exist;
their probe lane refuses with the transitional message until their adapters land.

Bridge lane: `test_connection_openai_compat` (`OpenAiCompatFamily::{first_party,
custom}`) with `resolve_optional_probe_credential`, which turns an absent or blank
slot into `Ok(None)` and lets the keyless send exist; the strict resolvers of the
three original families are unchanged. Discovery is the additive Tauri command
`setup_discover_openai_models` (`docs/14` §9 item 32). The renderer's unlocks and
the net-new provider-family picker are described in §4.9.

| ID | Criterion | Status | Where |
|---|---|---|---|
| `TEST-AI-001` | The OpenAI-compat wire contract: `POST {base}/chat/completions` (+ streaming) and `GET {base}/models`; `Authorization: Bearer` only when a credential exists; the families differ only in base-URL defaulting and credential shape | Available (wire) | `crates/providers/openai/tests/wire.rs` (`a_streamed_turn_speaks_the_native_wire`, `the_families_own_their_base_urls`, `a_keyless_custom_endpoint_sends_no_authorization`) |
| `TEST-AI-006` | Credential resolution is exclusive: the first-party family's empty credential blocks the send with zero requests; nothing is substituted (`AI-ADAPTER-12`) | Available (wire) | `crates/providers/openai/tests/wire.rs` (`a_missing_credential_blocks_the_openai_send`); the bridge-level form is `crates/bridge::setup::test_connection_blocks_an_openai_send_without_a_credential` |
| `TEST-AI-007` | No model-ID family policy on this wire either: every id travels verbatim (the refusal, where one exists, is the provider's own wire error) | Available (wire) | by construction — the adapter sends the id untouched; the family flip tests live in §4.10–§4.12, and a non-echoing probe model is refused by the echo check, not by policy (`the_probe_refuses_a_non_echoing_model`) |
| `TEST-AI-008` | Model echo is a PREFIX match (`gpt-4o-2024-11-20` is a legal echo for `gpt-4o`); a real substitution is `ModelNotFound` (`AI-MODEL-01`) | Available (wire) | `crates/providers/openai/tests/wire.rs` (`a_model_echo_mismatch_stops_the_stream`, `a_dated_revision_tolerates_the_echo`, `the_probe_refuses_a_non_echoing_model`) |
| `TEST-AI-009` | Native reasoning reaches the caller under both recorded spellings (`delta.reasoning_content`, fallback `delta.reasoning`); replayed cross-wire reasoning blocks are omitted (the one sanctioned deviation — `docs/14` §9 item 31) | Available (wire) | `crates/providers/openai/tests/wire.rs` (`reasoning_deltas_arrive_under_both_spellings`, `history_replays_with_tool_messages_and_omits_reasoning`) |
| `TEST-AI-010` | Usage is exact — prompt/completion tokens plus the cached-tokens detail — and never estimated (`AI-USAGE-01`) | Available (wire) | `crates/providers/openai/tests/wire.rs` (`usage_maps_off_the_wire_names`) |
| `TEST-AI-011` | Tool calls: streamed `delta.tool_calls` fragments assemble with the recorded synthetic ids (`chunk.id-{index}`, `call-{index}` when id-less); the close happens at `finish_reason` (`AI-TOOL-01`) | Available (wire) | `crates/providers/openai/tests/wire.rs` (`tool_calls_stream_and_close`, `continuation_fragments_without_ids_keep_the_recorded_synthetic_id`) |
| `TEST-AI-012` | Documents and tool-result media are refused explicitly (`UnsupportedModality`, zero requests) — nothing multimodal is silently dropped (`AI-MM-01/02`) | Available (wire) | `crates/providers/openai/tests/wire.rs` (`documents_are_refused_upstream_of_the_send` — both refusal classes assert in this fixture) |
| `TEST-AI-013` | No native token count exists on this wire: explicit refusal, zero requests, never an estimate (`AI-ADAPTER-09`) | Available (wire) | `crates/providers/openai/tests/wire.rs` (`token_counting_is_refused_and_never_estimated`) |
| `TEST-AI-014` | Native stop reasons map by the recorded table (stop→`end_turn`, length→`max_tokens`, tool_calls/function_call→`tool_use`, content_filter→`ContentFilter`); unknown spellings stay `Other` verbatim (`AI-ADAPTER-08`) | Available (wire) | `crates/providers/openai/tests/wire.rs` (`stop_reasons_map_onto_the_preserved_set`) |
| `TEST-AI-015` | Dropping the stream cancels the request (`AI-STREAM-02`) | Available (wire) | `crates/providers/openai/tests/wire.rs` (`dropping_the_stream_cancels_it`) |
| `TEST-AI-016` | A keep-alive-only stream (`data: {}` empty-object pings) times out at the idle window and never resets it; `data: [DONE]` is required, its absence is an error | Available (wire) | `crates/providers/openai/tests/wire.rs` (`a_pigeonhole_stream_times_out_at_the_idle_window`) |
| `TEST-AI-021` | Provider error classes stay distinguishable, including a mid-stream error chunk; provider wording is kept, raw bodies are not | Available (wire) | `crates/providers/openai/tests/wire.rs` (`provider_error_classes_are_distinguishable`) |
| `TEST-AI-022` | Requests travel only to the configured endpoint; the keyless custom send carries no `authorization` header (`AI-ADAPTER-01/02/13`) | Available (wire) | `crates/providers/openai/tests/wire.rs` (`a_keyless_custom_endpoint_sends_no_authorization`, the endpoint-asserting fixtures) |
| `TEST-AI-023` | The native-wire conformance fixture runs against the real-HTTP mock provider (this table) | Available (wire) | the whole `crates/providers/openai/tests/wire.rs` battery (21 fixtures) |
| `TEST-SETUP-CFG-001` (working id) | The setup lane `test_connection_openai_compat` resolves the probe credential per family: first-party static key required (empty → adapter refusal, zero requests), `custom` keyless-allowed with the helper kind offered, typed secrets ride `connection/<fieldKey>` and are never persisted | Available (unit) | `crates/bridge::setup` (`test_connection_probes_the_openai_family`, `test_connection_uses_the_typed_secret_for_the_openai_family`, `test_connection_runs_the_custom_family_keyless`, `test_connection_blocks_an_openai_send_without_a_credential`) |
| `TEST-SETUP-CFG-002` (working id) | The open-layer probe runs the draft's first `inferenceModels` id and nothing else sends (`test_connection_sends_the_first_draft_entry_verbatim`); when the draft names none, the lane falls back to the endpoint's own listing — the fallback choice is exercised through the listing reader's battery, with no dedicated bridge fixture yet | Available (unit) | `crates/bridge::setup` + `crates/providers/openai/tests/wire.rs` (`discovery_reads_the_listings_the_wire_sends`) |

**Still unknown or unexercised for this wire** (`docs/14` §9 item 31): the wire
readings are reconstructions pinned by fixtures, not captures — a real provider that
deviates from them (e.g. a system prompt sent per-message, a different keep-alive
shape) will surface as provider behavior to be re-read. `TEST-AI-017` (catalog
signature) and `TEST-AI-019/020` (MCP) stay the family-agnostic deferred rows of
§4.10.

### 4.15 Sessions and persistence — Phase 4 slices 1–4 (`TEST-SESSION`, `TEST-DATA`)

Phase 4's first slice is the persistence core (`docs/13` Phase 4): `crates/store` is the
SQLite WAL layer of the `docs/11` §14 rebuild model (schema v1: `sessions`, `turns`,
`usage_events`, `audit_events`, `quarantined_records`, plus a content-addressed file
store); `crates/sessions` is the semantic layer over it. The shipped layout (`docs/11` §4:
per-session JSON/JSONL files with a session-local `.audit-key`) remains the **preservation
baseline**; the normalized tables are a recorded rebuild decision, not an observation, and
1:1 session-JSON parity is deliberately not attempted. Two refinements are visible against
the captures and are called out so they are not mistaken for captured behavior: the chain
adds a `prev_hmac` link between records (the captured record shape shows a per-record
`_audit_hmac` but no linkage), and the audit key is reached through an `AuditKeyStore` seam
(production: `KeyringAuditKeys`, keychain account `session-audit/<uuid>`; tests: in-memory;
`TEST-CFG-006`).

Read policy for corrupt rows: a stored row that fails validation is moved to
`quarantined_records`, removed from its table, and **the read fails naming it**; the retry
lists the survivors. Never a silent skip — a transcript with a quietly-missing turn would
be a lie (`TEST-SESSION-003` / `TEST-DATA-003`). A database file that fails integrity
checks or is not SQLite at all is renamed `<name>.corrupt-<epoch-ms>` (never deleted) and
the open reports it; a database written by a newer build refuses to open untouched
(`SchemaAhead`).

Phase 4 slice 2 adds the turn loop (`crates/agent`). A send replays the stored transcript
(only `user`/`assistant` turns; empty turns skipped; consecutive same-role turns merged so a
failed send does not produce a message sequence the provider grammar rejects), persists the
user turn first, asks the adapter for one native turn, and records the assistant turn as
`complete`, `partial` (cancelled, with what the stream delivered before the drop), or `error`
(the provider's own class and message travel unchanged; no retries and no substitution —
`docs/12` §4.2, `TEST-AI-021`). Dropping the stream is what cancels the native request
(`TEST-AI-015`). Schema v2 adds `turns.stop_reason`, so the provider's native stop spelling
survives instead of being collapsed (`docs/12` §4.1, `AI-ADAPTER-08`). Transcript content is
the agent's `TurnContent` JSON: request-replayable `ContentPart` blocks plus response-only
citation annotations, kept in the record and excluded from replay because no adapter accepts
a response annotation as a request block. Tool declarations are refused before any provider
contact until the MCP phase can execute them; tool execution, retries, and approvals remain
open.

Phase 4 slice 3 (`e1ad4bd`) adds the durable live-turn marker and the recovery rules.
`run_turn` writes an assistant placeholder (`partial`, no error) before the user turn as the
start marker; `finish_turn` may only transition a live `partial`/no-error row, validates
session ownership, resolves the audit key before the row transition, and writes the content
blob only after the row is known to be live (a refused finish leaves no orphan blob).
`recover_interrupted` turns leftovers into `error` with `"the stream was interrupted by an
app restart"`; it runs at `run_turn` start and on session open, but **not** when this process
owns a running turn (opening a task mid-stream must not turn a live turn into a crash).
Cancelled turns are always finished as `partial` with `"the turn was cancelled"`, so a
cancel never looks like a crash; a token cancelled before anything was persisted returns
`CancelledBeforeSend` and is not reported as a recorded user turn. Archive and delete refuse
while a turn is running (`TURN_IN_PROGRESS`). `delete_session` removes only blobs no
surviving turn references (content-addressed blobs can be shared). `sessions::usage` folds
the ledger into local-day buckets (`summarize_usage(events, days, FixedOffset, now)`), keeps
cached tokens separate, accepts 1..=366 days, and takes the device's UTC offset explicitly;
the offset is the current one for the whole window, so a DST transition inside the window is
not re-bucketed (unobserved, recorded in `docs/14` §9). `sessions::retention` filters by
surface and `older_than_days`, honors a global hold and a per-session legal hold, and reports
`deleted`/`held`/`kept`; there is no background timer and no UI control yet.

Phase 4 slice 4 (`955f47d`) adds the bridge composition seam. `crates/bridge/src/chat.rs`
defines `ChatCommand`/`ChatResponse`, `ChatService<K, R>`, and the `ChatTargetResolver` seam;
the production `ProfileTargets` resolves **only the applied profile** through
`ConfigBackend`, refuses `CONFIG_MISSING` (none applied) and `CONFIG_VALIDATION` (no
provider/model), and refuses locked families (`mantle`/`foundry`) with
`ADAPTER_NOT_AVAILABLE`. It reuses setup's credential resolution (static key, helper script;
the profile's credential kind is honored) and its custom-header/base-URL rules; no model
discovery runs at send time. A session stores provider id, profile id, and the native model
id; a send refuses if the applied profile now serves a different provider/model
(`MODEL_UNAVAILABLE`, "No model was substituted") instead of silently switching. The Tauri
command is `chat_command`; content deltas are emitted as `chat://turn-event` with the session
id. The app's audit-key store is the OS keychain (`session-audit/<uuid>`); the bridge battery
uses an in-memory store. Cowork sends no system prompt or context assembly: the shipped
bootstrap prompt was not captured, so `TurnInput::text` carries an empty system (explicit
deviation, `docs/14` §9 item 33). Phase 6 Code adds only the selected folder block (§4.4.1).

The renderer slice adds the chat client (`apps/desktop/src/chat/`), the New-task send/Active
list, the `/cowork/:sessionId` transcript surface (streaming text, Stop, provider error
verbatim, non-`end_turn` stop line, reasoning behind `<details>`, copy), the sidebar task
rows, renderer-owned drafts (`composer-draft:new`, `composer-draft:<sessionId>`), and the
7/30/90-day usage panel in Settings. Drafts are retained on **any** failure and cleared only
when a turn completes; the New draft is moved into the session draft before the first await.
The New screen's model label is neutral ("model") because the applied profile is unknown
until a session exists; the session surface shows the provider-qualified label. The browser
E2E battery has no Tauri host: it proves the desktop-only error states, draft persistence,
and the absence of faux rows/totals, **not** a live stream. There is no retry action, model
picker, attachment flow, or per-surface usage split in this phase; all are recorded as
Unknown/deferred rather than invented.

| ID | Criterion | Status | Where |
|---|---|---|---|
| `TEST-SESSION-001` | Create, send, stream, stop, reopen, archive, delete | Available (unit + bridge); renderer unexercised | store/sessions lifecycle (`sessions_round_trip_and_order_by_activity`, `archive_flips_only_the_flag`, `delete_removes_rows_and_content_blobs_of_one_session`), agent turn (`a_reply_streams_and_is_recorded_with_its_stop_reason_and_usage`, `cancelling_leaves_a_partial_turn_with_what_arrived`, `a_provider_refusal_is_recorded_and_returned_verbatim`), bridge chat (`create_send_and_open_restore_the_transcript`, `stop_is_idempotent_and_keeps_the_partial_reply`); E2E T28 covers the browser's desktop-only state, not streaming |
| `TEST-SESSION-002` | Restart restores metadata without duplicate live process | Available (unit + bridge) | `opening_a_session_recovers_an_interrupted_stream`; the stop test asserts an open mid-stream leaves the live row `partial`/no-error; no OS-level kill fixture (a live placeholder row simulates the crash) |
| `TEST-SESSION-003` | Corrupt/partial session is quarantined, not silently deleted | Available (unit) | `a_corrupt_turn_row_is_quarantined_never_deleted`, `a_corrupt_session_row_is_quarantined_never_deleted`, `a_corrupt_database_is_set_aside_never_deleted` |
| `TEST-SESSION-004` | Delete removes metadata/transcript/uploads/outputs/preview cache | Partial | rows, transcript blobs (including shared-blob safety), and the audit key (`delete_removes_rows_and_content_blobs_of_one_session`, `deleting_a_session_keeps_blobs_its_neighbor_still_references`, `delete_honors_the_legal_hold`); uploads/outputs/preview belong to the artifacts phase |
| `TEST-SESSION-005` | Retention does not delete projects/memory/uncommitted work | Partial | `retention_sweeps_one_surface_and_honors_holds`, `retention_global_hold_and_validation`; the sweep only sees sessions because no project/memory store exists yet |
| `TEST-SESSION-006` | Audit HMAC verification detects tampering | Available (unit) | `the_audit_chain_verifies_and_detects_tampering`, `a_wrong_audit_key_breaks_the_chain`; per-session isolation in `one_tampered_chain_does_not_touch_its_neighbor` |
| `TEST-SESSION-007` | Usage event preserves input/output/cache/search/error | Available (unit) | `usage_events_preserve_every_count_and_the_error`; the agent writes one ledger event per attempt (absent channels land as zero in the ledger while the turn's `usage_json` keeps the exact provider shape) |
| `TEST-SESSION-008` | 7/30/90-day aggregation is correct across time zones | Available (unit) | `usage_buckets_follow_the_local_day`, `usage_excludes_events_before_the_local_window_start`, `usage_summary_reads_the_ledger_window`, `usage_summary_split_reports_each_surface`; bridge `usage_summary_aggregates_the_ledger` and `usage_summary_splits_cowork_and_code`; the Settings panel is browser-only (desktop-only copy) |
| `TEST-SESSION-009` | Draft survives recoverable error and navigation | Available (unit + E2E) | E2E T31 (reload), `apps/desktop/src/chat/drafts.ts`; the session composer keeps the text on provider failure, which is code-reviewed but not E2E-exercised without a core |
| `TEST-SESSION-010` | Code worktree state survives session metadata deletion | Unknown | Code phase — worktrees are not built |
| `TEST-DATA-001` | Migrations forward-only, idempotent, preserve unknown fields where safe | Available (unit) | `migrations_are_forward_only_and_idempotent`, `unknown_payload_keys_survive_reopen`, `refuses_a_database_written_by_a_newer_build`; `migrate`'s unit test pins the supported version to the last migration |
| `TEST-DATA-002` | Writes atomic; a crash leaves a recoverable previous state | Partial | SQLite WAL transactions carry the atomicity (e.g. the delete cascade is one transaction); the live-placeholder recovery is tested, but no OS-level crash fixture was built |
| `TEST-DATA-003` | Corrupt/partial records are quarantined with an error, never deleted | Available (unit) | the quarantine trio above, plus the corrupt-database set-aside test |
| `TEST-DATA-004` | Retention deletes only expired session artifacts/transcripts and honors hold | Available (unit) | `retention_sweeps_one_surface_and_honors_holds`, `retention_global_hold_and_validation`; no background timer or UI (unobserved) |
| `TEST-DATA-005` | Legal hold survives restart and blocks deletion until released | Available (unit) | `legal_hold_survives_restart_and_blocks_delete_until_released`; retention honors the same hold |
| `TEST-DATA-006` | Audit HMAC validates; tampering is detected and reported | Available (unit) | as `TEST-SESSION-006` |
| `TEST-DATA-007` | Usage events preserve channels and aggregate 7/30/90 | Available (unit) | preservation tests above plus the usage aggregation tests; since the Phase-7 split view the panel shows the Cowork/Code shares beside the combined totals (the ledger itself still carries no surface column — the split joins the session's surface, `docs/14` §9 item 68) |
| `TEST-DATA-008` | Backup/restore round-trips config and sessions without secrets | Unknown | not started |
| `TEST-DATA-009` | Export/support bundle redacts credentials, prompts, paths, hosts, account ids | Unknown | support bundle not started |
| `TEST-DATA-010` | Concurrent writers do not lose updates; WAL/transaction behavior tested | Partial | WAL is verified (`wal_is_on_and_foreign_keys_enforce`); the store is one mutex-guarded connection by design (single desktop core), and no concurrent-writer fixture was built |

Known limits: quota/permission errors surface distinguishably from corruption (an inode/disk
error never masquerades as a corrupt database); the schema has no FTS index yet (`docs/19`
IMP-15 territory, unneeded until search lands); retention, backup, and export remain the
recorded unknowns above. Slice 2's own limits: no automatic retries (policy unobserved, not
invented), tools refused, citations recorded but not replayed. Phase-4 limits now explicit:
no Cowork system prompt/context assembly (bootstrap prompt unavailable; Code adds only its folder block), no retry/regenerate action,
no model picker or token-capability validation (no capability data), no attachments, no
retention timer/UI, no per-surface usage split, no stored profile revision (the profile
document has no revision field; a profile swap that keeps provider/model is not detected as a
substitution), and no native-runtime UI test of a live stream.

### 4.16 MCP, tools, skills, plugins — Phase 5 (`TEST-MCP`, `TEST-AI-019/020`)

Phase 5 is implemented on the `phase-5` lane in three commits. Slice 1 (`f45e71e`) built the
transport-independent MCP lifecycle in `crates/mcp`: `rmcp` 3.7 with real stdio and Streamable
HTTP, a namespaced `<server>:<tool>` registry, per-session frozen snapshots, snapshot-gated calls,
caller-supplied timeouts, non-fatal per-server startup failure, redacting config/errors, and
rebuild-defined schema bounds (4,096 tools/server, 256 KiB/schema). Legacy HTTP+SSE is modelled but
returns `UnsupportedTransport` because rmcp 3.7 carries SSE inside Streamable HTTP. Slice 2
(`f054f65`) added the pure policy/approval engine (`crates/agent/src/tool_policy.rs`), the execution
gate (`crates/agent/src/tool_exec.rs`), and the MCP egress gate (`crates/mcp/src/egress.rs`). The
rule serialization, fail-closed precedence, default `Ask`, argument subset match, and SHA-256
fingerprint are **rebuild-defined**; the shipped `builtinToolPolicy`/`toolPolicyEndpoint` shapes
were not captured.

Slice 3 (`03d5102`) closes the wiring gap:

- **Agent tool loop.** `run_turn_with_tools` runs bounded rounds (`DEFAULT_MAX_TOOL_ROUNDS = 16`),
  replays `tool` rows as provider tool-result messages, filters unconditional denies before the
  provider request, validates arguments against the tool schema, maps internal names to provider-safe
  wire names and back, persists every tool result, and emits `TurnEvent::ToolResult`. A denied call
  becomes a model-visible tool error so the transcript stays grammatically valid (rebuild decision;
  the shipped denial-result gate was not observed). Tool output is sanitized to text/images/documents
  and capped at 1 MiB; audio is an explicit error because the provider SDK has no audio part.
- **Bridge and boot.** `crates/bridge/src/mcp.rs` owns `mcp/servers.json`, the keychain-backed
  per-server OAuth store, the refresh HTTP client, the renderer approval prompt, the `ToolExecutor`
  adapter, `ToolRuntime`, frozen snapshots, approval persistence, and usage aggregation.
  `crates/bridge/src/catalog.rs` owns skills/plugins and plugin MCP servers. The Tauri shell starts
  MCP off the boot path, emits `chat://mcp-status`/`chat://tool-approval`, and exposes retry/forget
  plus the tool/skill/plugin commands. A broken MCP directory degrades MCP instead of failing chat.
- **Approvals and audit.** Scopes are `Once`/`Session`/`Persistent`; persistent is gated by
  `mcpPersistentAlwaysAllowEnabled`; scheduled lifetime `0` offers only `Once`. New grants are
  written to `mcp/approvals.json` (0600) and chained as `mcp.tool.approved` with the tool name and
  argument fingerprint only. `ApprovalStore::restore` skips expired and `Once` records.
- **OAuth, schema cache, search.** OAuth tokens are per server in the keychain
  (`mcp-oauth/<server>`); an explicit `Authorization` header wins over OAuth. The schema cache
  is `mcp/schemas.json` (atomic, 0600); cached tools are metadata/search-only and cannot execute
  without a live session. Tool search is deterministic and rebuild-defined; `McpTools` lists live
  executable tools for the session picker regardless of `toolSearchEnabled`, while still filtering
  disabled and policy-hidden tools. Follow-ups (`12d508a`, `b8391ae`) added the OAuth
  token-endpoint egress check and disabled refresh redirects; the `rmcp` 3.7 transport follows
  only same-origin 307/308 redirects.
- **Skills and plugins.** Skills use a rebuild-defined `manifest.json` directory with
  `Org > User > Plugin > Bundled` precedence; bundled names are non-runnable placeholders until
  bodies are captured; scripts never execute; assets are declared-only, path-confined, and capped.
  Plugins use local `plugin.json` manifests and a managed root; org bundles are read-only; no network
  marketplace exists. Signature verification is not implemented: `Required` refuses every install;
  `Optional` accepts unsigned/unverified and reports `Unverified`. Plugin MCP servers are namespaced
  `<plugin>.<server>` and go through normal MCP policy/approval. Enablement changes apply on the next
  app start, never to a frozen session snapshot.
- **Renderer.** A rebuild-defined approval modal, session tool picker, skill picker, and
  Developer/Skills/Plugins settings panes. `#settings/developer` is a placeholder key because the
  shipped Developer hash was not captured; `skills`/`plugins` stand in for the uncaptured Customize
  route; `connectors` remains disabled. Browser preview has no Tauri host and reports
  `BRIDGE_UNAVAILABLE`, never fake rows or totals.

| Test | Status | Evidence / gap |
|---|---|---|
| `TEST-MCP-001` | Partial | stdio + Streamable HTTP start/fail/reconnect/shutdown are covered by 22 manager tests and 2 real-stdio tests (`--features fixture-server`); legacy SSE is explicitly unsupported; no live HTTP integration fixture yet |
| `TEST-MCP-002` | Available | namespacing, duplicate-name refusal, frozen snapshots, session picker, and missing names failing the send are covered by MCP, agent, and bridge integration tests |
| `TEST-MCP-003` | Available | `Once`/`Session`/`Persistent`; persistent gated by context; scheduled lifetime expiry; durable `mcp/approvals.json`; renderer prompt; audit-chain record. Native UI prompt is not E2E-exercised; a prompt can be lost if the renderer navigates away |
| `TEST-MCP-004` | Available | `visible_tool_names` filters unconditional denies before the provider request; disabled/hidden tools are never offered; bridge integration covers the turn path |
| `TEST-MCP-005` | Available | argument-scoped deny matches the actual argument subset; the execution gate enforces it; rule shape is rebuild-defined |
| `TEST-MCP-006` | Partial | per-server token store, keychain backing, refresh primitive, header injection, `NeedsAuth`, and forget-one-server are implemented; the production refresh client applies the egress allowlist to the token URL and disables redirects; no live OAuth server/keychain exercise and no authorization-code/PKCE browser flow (unobserved, not emulated) |
| `TEST-MCP-007` | Available | exact-host egress allowlist blocks HTTP/SSE before `connect` with zero connector calls; `http`/`https` only; **stdio is not inspected**; the pinned transport follows only same-origin 307/308 redirects and refuses cross-origin hops; OAuth refresh applies the allowlist and disables redirects; DNS/IP checks are not implemented |
| `TEST-MCP-008` | Available | tool output is data-only, sanitized to text/images/documents, capped at 1 MiB, and an injection-shaped output cannot become a tool call; no fuzz corpus or UI-level test |
| `TEST-MCP-009` | Partial | startup failure is recorded per server, boot continues, status events and Developer retry exist; no persistent/actionable renderer toast |
| `TEST-MCP-010` | Available | caller timeout returns `ToolTimeout`; no default deadline is invented; dropping the future may not cancel the server-side request (recorded limitation) |
| `TEST-AI-019` | Available | approval decisions run through the turn loop, the renderer prompt, durable grants, and the audit chain; 4 bridge tool-turn tests + 12 agent tool-loop tests; native UI unexercised |
| `TEST-AI-020` | Available | egress denial occurs before the connector is called; stdio egress and DNS/IP checks are explicit limitations |

The Phase 5 exit criteria are met for the local/stdio path: an unavailable MCP server cannot block
app boot, tool calls are exact and auditable, and a denied tool never reaches the provider. Remaining
conformance gaps are recorded in `docs/14` §9 items 44–52: no live HTTP MCP fixture, no live OAuth
server/keychain exercise, no marketplace/signature verification, no persistent actionable failure
toast, no model-driven on-demand tool search, approval-prompt loss on navigation, a global approval
lock that serializes tool turns across sessions, no default MCP start/tool-call timeout, no
DNS/IP egress check, no audio tool-result path, legacy SSE unsupported, and a bounded JSON Schema
subset (`pattern`/`format`/external `$ref` not enforced).

### 4.17 Phase 7 — memory, usage, privacy, updates, retention (`TEST-SESSION`, `TEST-DATA`, `TEST-PRIV`, `TEST-UPDATE`)

Phase 7 (`docs/13` §"Phase 7") is implemented on the `phase-7` lane in five
increments, none merged to `dev` yet: memory (`4d18d87`), the usage split view
(`7aa5dfe`), privacy/diagnostics (`0f76676`), retention + tombstone GC
(`6b3fe47`), and the update lifecycle (`aa1337e`). The exit criteria hold:
legal hold prevents deletion (boot-enforced, `TEST-DATA-005`), the diagnostic
bundle enumerates and redacts (`TEST-PRIV-003`, `TEST-DATA-009`), and an
update failure leaves the old version runnable (the verification gate refuses
every bundle until a trust anchor exists, `TEST-UPDATE-001`).

| ID | Status | Where / limitation |
|---|---|---|
| `REQ-PRIV-04` (memory + retention halves) | Available (unit + bridge integration + mocked renderer) | Memory: `crates/memory` (10 unit tests), the bridge memory gate (3 integration tests), the Cowork settings panel. Retention: `RetentionSettings` read fail-safe from the merged limits section, `ChatService::retention_sweep` at boot through the Phase-4 engine, tombstone GC (`sweep_tombstones`), bridge `retention_deletes_only_what_a_profile_configures`. Nothing deletes unless a profile configures days (`docs/14` §9 item 69). |
| `TEST-SESSION-008` | Available (unit) | The 7/30/90 local-day battery plus the Phase-7 split view (`usage_summary_split_reports_each_surface`, bridge `usage_summary_splits_cowork_and_code`); the renderer renders the per-surface cards beside the combined totals. |
| `TEST-DATA-007` | Available (unit) | Channels preserved; the split view's store query is `list_surface_usage_between` (`surface_usage_events_are_scoped_to_their_surface`). The ledger itself still carries no surface column — the split joins the session's surface (`docs/14` §9 item 68). |
| `TEST-PRIV-001` | Available (by construction + unit) | No emitter ships (`crates/telemetry` module doc): no crash reporter, no analytics, no OTel exporter. The only usage data is the device-local ledger, whose rows carry counts and an error string — never message content. The redaction battery proves prompt-like text passes through unchanged (redaction is additive, not a content filter). |
| `TEST-PRIV-002` | Available (unit) | `crates/telemetry` — `the_block_switches_are_independent`, `a_malformed_switch_fails_closed`, `the_service_block_list_reads_arrays_and_fails_closed_on_malformed`. The switches gate no emitter (there is none) but are recorded and displayed; `otlpContentCapture` is refused at apply time and surfaced for defense in depth. |
| `TEST-PRIV-003` | Available (bridge + mocked renderer) | `a_diagnostic_bundle_requires_explicit_consent` (`CONSENT_REQUIRED`, nothing written); the Privacy pane's consent checkbox gates the build button; there is no upload in this build and the pane states it. |
| `TEST-PRIV-004` | Available (unit + bridge) | `crates/telemetry` redaction battery (Bearer/Basic schemes, token prefixes, URL userinfo/query/fragment); the bridge bundle passes the export redaction and then the free-text pass (`a_consented_bundle_is_written_redacted_and_logged` proves a credential value and a query token never appear). |
| `TEST-UPDATE-001` | Partial | The lifecycle state machine, the enforcement/relaunch windows, and the session guard are implemented and tested (`crates/updates`, 10 tests). The **feed** is Unavailable: the shipped wire format and public key were never captured, so the production verifier refuses every bundle (`NoTrustAnchor`) and no bundle installs — the old version stays runnable by construction (`docs/14` §9 item 70). A captured feed moves the verifier aside; it cannot weaken the gate. |
| `TEST-UPDATE-002` | Available (unit) | `crates/updates` — enforcement window (72 h default, `0` = immediate, malformed keeps the default), relaunch window (24 h default), streaming-task guard blocks even an expired window, blocked policy refuses outright. |
| `TEST-DATA-009` | Available (bridge) | `DiagnosticsPreview` enumerates exactly what a bundle includes (and says when the configuration is omitted); `DiagnosticsBuild` writes the redacted markdown under the app-managed diagnostics directory and logs the action to `diagnostics/log.jsonl` without contents. The config redacted-export half is §4.8.6. |
| `TEST-DATA-010` | Partial | Unchanged from §4.15 (WAL verified; single mutex-guarded connection by design). |
| `TEST-CFG-009` | Available (unit + bridge) | The Limits tab's keys validate and persist (§4.8.1's schema layer); enforcement is the boot sweep — nothing deletes unless a profile configures days, the hold suspends everything, and per-session legal holds are honored by the same engine (`TEST-DATA-005`). The shipped retention schedule was never observed, so there is no timer beyond boot (`docs/14` §9 item 69). |

## 5. Coverage rules

1. Every **Available** or **Available, unexercised** feature has at least one spec and one test suite.
2. Every **Partial/Unknown** feature links a gap and must not ship a speculative UI.
3. Every **Unavailable/Unsupported/Policy-disabled** feature has a gate test proving it is absent or
   explained, not a dead control.
4. Every **P0** path (create/send/stop/restore/apply/update/delete/export) has an integration or E2E
   test, a negative test, and a redaction test.
5. A test that depends on a real provider account is not an acceptance test; use a mock native server.
6. No fixture contains inspected personal data, credentials, memory, or private hosts.

## 6. Cross-reference audit

- `FEAT-ART-*`, `FEAT-SCHED-*`, `FEAT-CODE-*`, `FEAT-EXT-*`, `FEAT-AUTH-*`, and `FEAT-I18N-*` are
  referenced by the specs through the IDs above; the corresponding test prefixes are defined here.
- `TEST-UX-009` is the **setup window**; composer-menu tests use `TEST-UX-013`, not UX-009. Its
  status is tracked in §4.9.
- `TEST-UX-003` is search/filter; model-picker keyboard/layout tests use `TEST-UX-002/013`.
- `TEST-SESSION-010` is Code worktree lifecycle; screen-level Code navigation uses `TEST-CODE-001..003`.
- No test may be claimed as passing without a reproducible fixture in `tests/`.
- Phase-1 shell evidence: the **52-case** E2E battery `tests/e2e/shell.spec.mjs` (T01–T49, including
  T28b–T28d) exercises `FEAT-SHELL-01/02` routes/sidebar/theme, palette and settings overlays, focus
  traps, hash deep links and history — including history-end greying of Back/Forward and the
  no-zombie settings close (T34/T35, renumbered from T31/T32 when Phase 4 added T31–T33); its run
  record and bug ledger live in [tests/report-phase1.md](../tests/report-phase1.md). Runner:
  `node tests/e2e/shell.spec.mjs` against a dev server verified to serve title `myharness` (see
  `docs/14` §7 trap 13), plus the **19/19 native runtime fixture** `tests/native/shell-runtime.sh`
  (single-instance second-launch routing, cold-start/second-launch deep links fail-closed, Setup3p
  Phase-2 deferral — `docs/14` §9 supplement item 4) and the **16/16 setup-window battery**
  `tests/e2e/setup-window.spec.mjs`. The whole-application campaign's phase-1 run is §4.20.

### 4.18 Native provider adapters — the Responses extension, Phase 8 slice 1 (`TEST-EXT`, `TEST-AI`)

Phase 8's first reference extension (`docs/13` §"Phase 8"): the OpenAI
**Responses** wire as its own native wall — `crates/providers/openai-responses`
(`myharness-provider-openai-responses`), sibling to the Chat Completions wall
with a shared design and deliberately no shared code (`AI-PROVIDER-02`,
`AI-EXT-02`). `ProtocolFamily::OpenAiResponses` is the sdk's id for the wire.
The wire was never observed in the shipped app; every reading is
rebuild-defined and pinned by the crate's fixtures (`docs/14` §9 item 71).
The adapter and its 17-fixture battery are the whole of this slice — the
setup family, the bridge probe lane, and the renderer unlock arrive with a
later Phase 8 slice, so `mantle`/`foundry`-style locked status does not apply
(the family is not yet reachable from the UI at all).

| ID | Criterion | Status | Where |
|---|---|---|---|
| `TEST-EXT-003` | Each adapter passes its native-wire fixture and has no cross-protocol translation | Available (wire) | the whole `crates/providers/openai-responses/tests/wire.rs` battery (17 fixtures); the crate speaks only `POST {base}/responses` + `GET {base}/models`, and the app-internal Anthropic model maps to this wire inside the wall alone |
| `TEST-EXT-005` | Credentials are scoped per provider/profile and inaccessible to other adapters | Available (adapter level) | `an_empty_credential_refuses_before_any_contact` — the empty credential refuses with zero requests (`AI-ADAPTER-12`), and the credential exists only as request-header material (`AI-PROVIDER-03`); the profile/bridge-side scoping lands with the setup slice |
| `TEST-EXT-006` | Egress is declared in the manifest and enforced at request time | Partial | the signed-package/manifest machinery is Phase 8 scope not yet built; this slice's guarantee is narrower — requests travel only to the configured endpoint (`the_request_body_spells_the_responses_wire_and_drops_recorded_fields`, `discovery_and_health_read_the_listing`) |
| `TEST-EXT-008` | A policy-blocked model fails closed; no substitution or policy bypass | Available (wire) | `the_model_echo_gate_refuses_a_substitution` — a dated revision stays legal, a different model refuses with zero execution (`AI-MODEL-01`); ids travel verbatim (no family gate, per the open model layer) |
| `TEST-AI-006` | Credential resolution is exclusive: the first-party family's empty credential blocks the send with zero requests | Available (wire) | `an_empty_credential_refuses_before_any_contact` |
| `TEST-AI-008` | Model echo is a PREFIX match; a real substitution refuses (`AI-MODEL-01`) | Available (wire) | `the_model_echo_gate_refuses_a_substitution` (non-streaming) and the `response.created` echo gate in the stream state machine |
| `TEST-AI-009` | Reasoning reaches the caller; replayed reasoning is omitted where the wire cannot carry it | Available (wire) | `the_stream_maps_reasoning_and_survives_keep_alives` (summaries stream as `ReasoningDelta`); assistant `Reasoning`/`RedactedThinking` replay is omitted — the recorded v1 reading (`docs/14` §9 item 71), the sibling wall's one sanctioned deviation applied one wire over |
| `TEST-AI-010` | Usage is exact and never estimated (`AI-USAGE-01`) | Available (wire) | `a_completed_response_maps_text_usage_and_end_turn`, `the_stream_maps_deltas_and_terminates_on_response_completed` — `input_tokens`/`output_tokens`/`cached_tokens`; `reasoning_tokens` dropped (a subset with no app channel) |
| `TEST-AI-011` | Tool calls assemble across events; nothing executes half-parsed (`AI-TOOL-01`) | Available (wire) | `the_stream_assembles_function_calls_across_events` — `output_item.added` (item's own `id`) → argument fragments → the done item's assembled arguments (invalid JSON is a visible error) |
| `TEST-AI-012` | Multimodal handling is explicit, never a silent drop (`AI-MM-01/02`) | Available (wire) | documents map **natively** to `input_file` (the one wire that carries a PDF); media inside a tool result is refused upstream of any bytes (`media_inside_a_tool_result_is_refused_upstream`) |
| `TEST-AI-013` | No native token count: explicit refusal, zero requests, never an estimate (`AI-ADAPTER-09`) | Available (wire) | `token_counting_refuses_instead_of_estimating` |
| `TEST-AI-014` | Native stop reasons map by the recorded table; nothing collapses to "done" (`AI-ADAPTER-08`) | Available (wire) | `a_completed_response_maps_text_usage_and_end_turn`, `a_turn_that_emitted_function_calls_stops_as_tool_use` (emitted items are the `tool_use` fact), `an_incomplete_response_preserves_max_output_tokens` |
| `TEST-AI-016` | The idle window fires without progress and keep-alives never reset it; stream termination follows this wire's own grammar — no `[DONE]` sentinel, terminal events only | Available (wire) | `the_idle_window_fires_without_progress` (`TEST-AI-016`), `the_stream_maps_reasoning_and_survives_keep_alives` (keep-alive comments consumed as no-ops), `a_stream_without_a_terminal_event_is_malformed` (no stop reason invented), `an_undocumented_event_type_is_an_error` (`AI-MM-01`) |
| `TEST-AI-021` | Provider error classes stay distinguishable; provider wording is kept, raw bodies are not | Available (wire) | `stream_failures_surface_the_provider_error` (`response.failed` and the bare `error` event — classified by the envelope's `code` spelling) |
| `TEST-AI-023` | The native-wire conformance fixture runs against the real-HTTP mock provider | Available (wire) | the whole battery; the mock shares the sibling batteries' design, not code (`docs/14` §9 item 72 records the harness traps) |

**Slice 2 — the setup surface (2026-10-26).** The family is reachable from
the UI: `ProviderKind::OpenAiResponses` (serde `"openai-responses"`) with its
own key spellings (`inferenceOpenAiResponsesApiKey` /
`inferenceOpenAiResponsesBaseUrl` — owner-defined rebuild keys recorded like
every open-layer family's), the form family (static key required; base URL
optional), egress derivation (override or `api.openai.com`), the export
label, the bridge probe lane `test_connection_openai_responses` (the strict
resolver — this wire has no keyless variant, so an absent credential is a
`CONFIG_VALIDATION` refusal at the bridge, zero requests), the chat factory
arm (`provider_id` = the wall's own `id()` spelling, `openai-responses`), the
additive discovery command `setup_discover_openai_responses_models`, and the
renderer's family option, Test connection unlock, and Discover models arm.

| ID | Criterion | Status | Where |
|---|---|---|---|
| `TEST-CFG-004` (adjacency) | The family validates its required fields: the static key is required (no keyless variant), the base URL optional (empty = first-party), a nonsense host refused | Available (unit) | `crates/config::forms` (`openai_responses_family_validation`, `every_provider_requires_its_own_fields`); the family's keys never leak from/to the Chat Completions family (`connection_schemas_are_provider_specific`) |
| `TEST-CFG-005` (adjacency) | Credential kinds are exclusive: the family offers static only; a helper/interactive draft is refused | Available (unit) | `crates/config::forms` (`credential_kind_subset_is_enforced` covers the mechanism; the family's offered set is `schema_kinds`) |
| `TEST-SETUP-CFG-003` (working id) | The setup lane `test_connection_openai_responses` probes a real one-token Responses turn through the family's wall: the draft's first id runs it, the typed secret rides `connection/<fieldKey>` and is never persisted, and an absent credential refuses with zero requests | Available (unit) | `crates/bridge::setup` (`test_connection_probes_the_responses_family`, `test_connection_uses_the_typed_secret_for_the_responses_family`, `test_connection_blocks_a_responses_send_without_a_credential`) |
| `TEST-AI-018` (adjacency) | Discovery reads the platform's shared `GET {base}/models` listing (single page, 10 s), tolerant across the three listing shapes; the shared setup outcome reports one page, never partial | Available (unit, via the wire battery) | `crates/providers/openai-responses/tests/wire.rs` (`discovery_and_health_read_the_listing`); the bridge maps the wall's outcome shape in `crates/bridge::helper::discover_responses_models` |

**Still unknown or unexercised for this wire** (`docs/14` §9 item 71): the
readings are reconstructions pinned by fixtures, not captures — a real
Responses deployment that deviates (a different keep-alive shape, an
undocumented event, a tool-result media spelling) surfaces as provider
behavior to be re-read. The chat-send path through the Responses family is
compile- and unit-verified only; no E2E case drives a live Responses stream
(the browser E2E has no Tauri host — the same posture as every other
family).

### 4.19 Native provider adapters — the local extension, Phase 8 (`TEST-EXT`, `TEST-AI`, `AI-LOCAL`)

Phase 8's second reference extension: `crates/providers/local`
(`myharness-provider-local`), the native wall for locally running Ollama /
llama.cpp servers over their documented OpenAI-compatible surfaces. The
wall's core is **local-network security** (`AI-LOCAL-01..04`): the
destination gate pins the base URL to loopback at construction (loopback IP
literals or `localhost` only; a name resolving off-loopback — DNS/IP
rebinding — refuses before any bytes; the resolver is injected so the
rebinding case is fixture-tested), requests pin to the resolved loopback
address, and the wall builds its own per-instance client with redirects
disabled — a redirect to any other origin errors, never follows. Keyless is
the documented default (`AI-LOCAL-05`): no `authorization` header at all,
and keyless never widens the gate. Discovery reads `/api/tags` first
(Ollama's keyed listing) and `/v1/models` on a 404 (the llama.cpp posture)
(`AI-LOCAL-06`). The family surface (config forms/egress/export, bridge
probe lane with the optional-credential resolver, chat factory, the
additive discovery command) mirrors every other family.

| ID | Criterion | Status | Where |
|---|---|---|---|
| `TEST-EXT-003` (adjacency) | Native-wire fixtures, no cross-protocol translation | Available (wire) | the whole `crates/providers/local/tests/wire.rs` battery (20 fixtures) |
| `TEST-EXT-005` (adjacency) | Credentials scoped; keyless never a bypass | Available (wire) | `a_keyless_send_carries_no_authorization_header`, `a_credential_travels_as_bearer`, and the destination-gate tests (keyless refuses nothing but never widens where the wall may connect) |
| `AI-LOCAL-01` | Loopback-only destination | Available (wire) | `a_non_loopback_base_refuses_before_any_contact`; the destination unit tests |
| `AI-LOCAL-02` | DNS/IP rebinding refused | Available (wire) | `a_rebinding_localhost_refuses_before_any_contact` (injected resolver) |
| `AI-LOCAL-03` | Destination pinned at construction | Available (unit) | the destination tests (a clean `localhost` pins its resolved address) |
| `AI-LOCAL-04` | Redirects refused, never followed | Available (wire) | `a_redirect_is_refused_not_followed` (exactly one request travels) |
| `TEST-AI-006/008` | Exclusive credential; prefix echo | Available (wire) | `the_model_echo_gate_refuses_a_substitution` |
| `TEST-AI-011` | Tool fragments assemble with the recorded synthetic ids; close at `finish_reason` | Available (wire) | `the_stream_assembles_tool_calls_with_the_recorded_synthetic_ids`, `idless_fragments_synthesize_call_index` |
| `TEST-AI-012` | Multimodal explicit, never dropped | Available (wire) | documents refused on user turns; media refused inside tool results (`media_is_refused_upstream`) |
| `TEST-AI-013/016` | Count refused; idle window over `data: {}` pings; the `[DONE]` sentinel required | Available (wire) | `token_counting_refuses_instead_of_estimating`, `the_idle_window_fires_without_progress`, `a_stream_without_the_sentinel_is_malformed` |
| `TEST-AI-018` (adjacency) | Discovery reads the runtimes' listings tolerantly | Available (wire) | `discovery_reads_the_api_tags_listing_first`, `discovery_falls_back_to_the_openai_listing_on_404` |
| `TEST-SETUP-CFG-004` (working id) | The local probe lane: keyless by default, typed secret per slot, non-loopback base refused at the lane | Available (unit) | `crates/bridge::setup` (`test_connection_probes_the_local_family_keyless`, `test_connection_uses_the_typed_secret_for_the_local_family`, `test_connection_refuses_a_non_loopback_local_base`) |

**Still unknown or unexercised** (`docs/14` §9 item 73): Ollama's native
`/api/chat` NDJSON grammar is an unobserved reading (the compat surface is
the v1 scope); a LAN-hosted local server is outside the loopback-only v1
contract; the chat-send path is compile/unit-verified only (no Tauri host
in the browser E2E — the standing posture).

### 4.20 Whole-application test campaign — phase 1: baseline and launch truth (2026-10-27)

This is a **new campaign**, not Phase 1 acceptance of the roadmap. Its purpose is to exercise the
whole application as built today, layer by layer, and to leave each claim with a reproducible
fixture. Phase 1 establishes the executable baseline, launches the real desktop artifact, and
measures the accessibility baseline. Feature-surface sweeps, adversarial passes, and live provider
flows belong to later campaign phases.

Environment: Fedora 44, rustc/cargo 1.99.0, Node 22.23.1, Vite 5.4.21, Chromium via Playwright
1.64.1; GTK 3, WebKitGTK 4.1, libsoup 3, dbus present. The host session is Wayland; the real-window
runs below force `GDK_BACKEND=x11` under Xvfb so the window is capturable.

| Layer | Command | Phase-1 result |
|---|---|---|
| Workspace metadata | `cargo metadata` | 30 packages |
| Rust suite | `cargo test --workspace` | **596 passed / 0 failed** (79 suites) |
| Lints | `cargo clippy --workspace --all-targets -- -D warnings` | clean |
| Renderer types | `tsc --noEmit` (apps/desktop) | clean |
| Renderer bundle | `vite build` | 74 modules, built |
| Shell E2E | `node tests/e2e/shell.spec.mjs` | **52/52** (T01–T49 including T28b–T28d) |
| Setup E2E | `node tests/e2e/setup-window.spec.mjs` | **16/16** |
| Native runtime | `bash tests/native/shell-runtime.sh` | **19/19** (real binary, Xvfb/X11) |
| Release launch | release binary, dev server stopped | before fix: error page; after fix: embedded `app://` bundle renders |
| Accessibility | `node tests/e2e/a11y.spec.mjs` | 18 route/mode scans; **0 serious/critical** (clean after `IMP-39` option c) |

#### Findings

| ID | Severity | State | Summary |
|---|---|---|---|
| `WA-01` | P0 | **fixed in this phase** | The release artifact loaded `devUrl` and could not start without the Vite server |
| `WA-02` | P1 | **fixed in this phase** (`IMP-39` option c) | Light-mode tertiary text failed WCAG AA; token split keeps the observed value for icons and adds an AA text derivative |
| `WA-03` | P3 | open — test-infrastructure gap | The native cold-boot check proves a live process/webview, not a rendered app |

**`WA-01` — the release build was a dev build.** With the Vite server stopped, the
release binary opened a 1200×800 window showing Chromium's
`Could not connect to localhost: Connection refused`. Root cause: `tauri`'s build script
sets `dev = !custom-protocol`, and `apps/desktop/src-tauri/Cargo.toml` never enabled the
`custom-protocol` feature — so **every** build, release included, embedded `devUrl`.
Fix: `default = ["single-instance", "custom-protocol"]` plus
`custom-protocol = ["tauri/custom-protocol"]`. Verified after the fix: with no server on
5173 the release binary renders the embedded bundle over `app://`, and the workspace
suite, clippy, and the native fixture stay green (596/0, clean, 19/19). This corrects the
Phase 9 record's implicit reading: *the release binary builds* is not *the artifact runs*.
Traces `TEST-ARCH-001/003/004`, `ARCH-PROC`, `docs/02` §2, `docs/13` Phase 9.

**`WA-02` — preserved tertiary text failed AA in light mode; fixed by token split.** axe-core
(WCAG 2.0/2.1/2.2 A+AA) over `/new`, `/projects`, `/cowork-artifact`, `/scheduled-task`,
`/epitaxy`, and four settings sections × light/dark found 10 serious violation types across
33 nodes. Every light-mode finding was the same token: `--mh-text-3: #898781` (the observed
tertiary value, `docs/07` §1.2) on `#fcfcfb`/`#f3f3f3` surfaces — `.tasks-title`,
`.profile-provider`, `.settings-group-label`, `.error-code` at 3.23–3.59:1; `.code-placement`
was 4.1:1 on the dark `#282827` card. This was a **preservation vs accessibility conflict**,
not a rebuild defect. Decision (`IMP-39` option c): the observed sample remains the icon token
(`--mh-icon-3: #898781`, non-text 3:1), and text uses computed AA derivatives
(`--mh-text-3: #6f6e68` light, `#94928c` dark). Worst cases are 4.61:1 on `#f3f3f3` and
4.74:1 on `#282827`; icon-only selectors were switched to `--mh-icon-3` so the observed value
is still rendered where 3:1 governs. Re-run: **18/18 route/mode scans clean**, 0
serious/critical. Traces `TEST-UX-012/014`, `IMP-39`; the token extension is recorded in
`docs/07` §1.2.

**`WA-03` — the native fixture cannot see an error page.** Before `WA-01` was fixed, the
19-check native fixture was green while the window displayed the connection-refused error:
R1 asserts the process and `WebKitWebProcess` are alive, not that the document rendered.
This is the exact "green test that contradicts the docs" posture the repository forbids.
Proposal (not implemented in this phase): add a rendered-content assertion to R1 — e.g. the
`app://` response body or a captured document title — so cold boot proves the shell, not the
process tree. Traces `TEST-ARCH-001/003`, `docs/14` §9 supplement item 4.

#### Limits of this phase

- Provider statuses are unchanged: no live provider account or wire was exercised (the standing
  mock-native-server posture); the 596-test suite covers the adapters against fixtures.
- No installer/bundle was produced (the Tauri bundler CLI is not installed); the runnable artifact
  proved here is the release binary.
- The accessibility scan ran in Playwright Chromium against the Vite dev origin, not WebKitGTK
  against `app://`; the real window was visually inspected, not scripted. A screen-reader pass is
  blocked in the Xvfb session (`org.a11y.Bus` cannot start); contrast measurement is axe's.
- Window captures were inspected in-session and are **not** committed (`evidence/README.md`
  rules 3–8); this section records the result in prose.
