# Code

**Features:** `FEAT-CODE-*`, `FEAT-SHELL-03`
**Primary docs:** [05](../05-screen-inventory.md), [09](../09-settings-and-configuration.md), [12](../12-ai-architecture.md)

## Purpose

Code is the repository/terminal surface. It runs Claude Code-style sessions against local folders or
remote SSH hosts, with worktrees, permission modes, a terminal, diff/files panes, and usage
analytics.

## Entry

The Cowork/Code segmented switch navigates to `/epitaxy`. The dashboard is shown when no session is
open. A session route is `/epitaxy/<session-id>` (**strong inference** from the route pattern; the
session view was not exercised here).

## Dashboard

Observed:

- Greeting (“What’s up next?”) and Code composer.
- Folder/repository path, choose-folder action.
- Model, permission mode, Fast mode, send.
- Overview card: Sessions, Messages, Total tokens, Active days, Peak hour, Favorite model.
- Tabs: Overview / Models.
- Time range: All / 30 days / 7 days.
- Activity heatmap and comparative copy.
- Sidebar: repositories, sessions, presets, starred items.

The dashboard is local analytics, not a provider dashboard.

## Session creation

1. Choose a local folder or configured remote host.
2. Select model/effort.
3. Select permission mode (Default/Accept edits/Plan/Bypass if policy allows).
4. Optionally enable Fast mode.
5. Send the first request.

The app creates a Code session metadata record and a CLI/agent session. A folder can be opened as a
worktree; worktree metadata is stored separately and uncommitted work must survive session deletion.

## Worktrees and Git

Settings include:

- default worktree location;
- branch prefix;
- whether worktree features are enabled;
- folder worktree/remote worktree gates;
- GitHub/PR integration.

The observed 3P protocol layer returns empty GitHub branch statuses. A real GitHub integration needs
its own provider/auth contract; do not assume the 3P stub is functional.

## Permission modes

| Mode | Intent |
|---|---|
| Default | Ask for risky tools/edits |
| Accept edits | Auto-approve file edits, still gate higher-risk actions |
| Plan | Read/plan without executing changes |
| Bypass | Broad auto-approval; admin can disable (`disableBypassPermissionsMode`) |

Bypass is visually distinct and carries a warning. “Auto mode” is offered only when policy allows.
Permission changes are recorded in session metadata (`permissionMode`,
`sessionPermissionUpdates`, `alwaysAllowedReasons`).

## Fast mode

A composer toggle for lower-latency model behavior. It is shown only when the selected model/provider
declares fast-mode support. It must not silently switch the model.

## Terminal and files

`node-pty` and `@xterm/headless` are shipped dependencies; a user-visible terminal is part of Code.
Observed build files include `pty-host`, shell-path resolution, find-in-page, file preview, and
worktree helpers.

Expected session panes (not all exercised):

- transcript/chat;
- terminal;
- files/diff;
- preview/browser;
- side pane for context/progress.

The exact split/popout layout is **unknown**; do not invent a fixed three-pane layout.

## Remote sessions

- `sshConfigs`, `sshHostAllowlist`, `sshClientPath`, `sshTransport`.
- Remote worktree/file-preview gates are supported.
- Remote sessions must be explicitly selected; no silent fallback to local.
- SSH host keys/credentials stay in main; prompts must be origin-safe.
- `ssh.log` exists for diagnostics.
- Self-hosted/cloud execution is unavailable/unconfigured here.

## WSL

`disableWslSessions` and the Linux host mean WSL sessions were not available. Do not assume Windows
WSL behavior from this evidence.

## Sandbox and browser

Settings expose local sandbox, strict sandbox, browser tools, open-links behavior, cookie handling,
and allowed sites. Code sessions can use the built-in browser/dev-server preview when the gate
permits. The runtime `coworkBrowser` gate is separate; do not conflate Code browser tools with the
Cowork browser surface.

## Local usage analytics

The dashboard’s totals come from the device usage ledger. They may differ from the Usage settings
page due to time range/surface; both are local and exclude provider billing. Token counts include
cache fields separately.

## States

| State | Behavior |
|---|---|
| No repositories | Empty dashboard + “Choose folder” |
| Session running | Streaming + terminal activity; stop |
| Permission required | Prompt with allow/deny/session scope |
| Worktree conflict | Explain branch/path conflict; no destructive reset |
| Remote disconnected | Reconnect/error; no local fallback |
| Model unavailable | Block send; choose another explicitly |
| Session archived | Removed from active list; worktree retained |
| Session deleted | Metadata removed; uncommitted work retained on disk |
| Git/GitHub unavailable | Show integration unavailable; do not fake PR data |

## Requirements

| ID | Requirement |
|---|---|
| `REQ-CODE-01` | Code sessions have explicit folder/host, model, permission mode, and worktree context. |
| `REQ-CODE-02` | Permission modes and bypass policy are enforced and auditable. |
| `REQ-CODE-03` | Worktree/remote placement never silently falls back. |
| `REQ-CODE-04` | Terminal/PTY lifecycle is cleaned up on stop/quit. |
| `REQ-CODE-05` | Fast mode is capability-gated and does not change model identity. |
| `REQ-CODE-06` | GitHub/PR integration is only shown when a real backend is configured. |
| `REQ-CODE-07` | Uncommitted work survives session metadata deletion/retention. |
| `REQ-CODE-08` | Dashboard usage is local and labeled as such. |

**Tests:** `TEST-UX-001`, `TEST-SESSION-010`, `TEST-SEC-002`, `TEST-ARCH-008`, `TEST-MCP-003/004`.
