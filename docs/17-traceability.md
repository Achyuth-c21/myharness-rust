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
- `TEST-UX-009` is the **setup window**; composer-menu tests use `TEST-UX-013`, not UX-009.
- `TEST-UX-003` is search/filter; model-picker keyboard/layout tests use `TEST-UX-002/013`.
- `TEST-SESSION-010` is Code worktree lifecycle; screen-level Code navigation uses `TEST-CODE-001..003`.
- No test may be claimed as passing without a reproducible fixture in `tests/`.
- Phase-1 shell evidence: the 32-case E2E battery `tests/e2e/shell.spec.mjs` (T01–T32) exercises
  `FEAT-SHELL-01/02` routes/sidebar/theme, palette and settings overlays, focus traps, hash deep
  links and history — including history-end greying of Back/Forward and the no-zombie settings
  close (T31/T32, added with the bug-hunt fixes in `tests/report-bug-hunt-phase1.md` §F); its run
  record and bug ledger live in
  [tests/report-phase1.md](../tests/report-phase1.md). Runner: `node tests/e2e/shell.spec.mjs`
  against a dev server verified to serve title `myharness` (see `docs/14` §7 trap 13).
