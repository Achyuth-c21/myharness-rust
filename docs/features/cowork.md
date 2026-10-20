# Cowork

**Features:** `FEAT-COWORK-*`, `FEAT-SHELL-03`
**Primary docs:** [09](../09-settings-and-configuration.md), [11](../11-data-and-storage.md), [12](../12-ai-architecture.md)

## Purpose

Cowork is the long-running work surface: Claude can read granted folders, use tools/MCP, create
files/artifacts, and keep working across turns. The observed build merges the Chat and Cowork entry
experience; the dedicated Chat tab is unavailable.

## Home

- Cowork/Code segmented switch, with Cowork selected.
- Greeting, Claude starburst, and the main composer.
- Project/folder context and model selection.
- “Active” tasks with relative timestamps and a clear-active control.
- Sidebar task history grouped by time, with search/filter and pinned/unread states.

Sending starts a local Cowork session. If the app cannot create a sandbox/session, it must fail
before model generation and show the placement/permission error.

## Session placement

A session can be local-only, SSH/remote, VM-backed, or cloud-backed depending on policy. The observed
3P launch selected local-only (device allowed, SSH host allowed, cloud not configured). Placement
must be explicit and must not silently move a task.

Local Cowork execution uses:

- a working directory and granted folders;
- a sandbox/VM helper where available;
- an agent process with a transcript and audit log;
- MCP servers/tools;
- egress and file-read policy;
- memory and skills/plugins settings.

On Linux, the log showed a Cowork helper/VM process, session mount handling, and a “full VM sandbox”
requirement flag. Full VM virtualization (`yukonSilver`) was unavailable here, so the complete VM
UX was not exercised.

## Files and folders

- Cowork has a working folder (shown in Settings → Cowork).
- Trusted folders can be managed; sessions can receive additional folder grants.
- File tools must refuse reads outside working directories when policy requires it.
- Uploaded files live in the session’s `uploads/`; generated files in `outputs/`.
- Folder mounts are recorded in session metadata.
- Symlinks/canonical paths must be resolved before granting access.

The UI distinguishes “folder” from “project”; a project can carry instructions and memory, while a
folder is a working location.

## Memory

- Global memory toggle.
- Account memory list with add/edit/delete.
- Per-session `memoryEnabled` and a global-memory baseline hash.
- A bundled `consolidate-memory` skill can rewrite/summarize memory.
- Retention of chats/Cowork does **not** automatically delete memory.

Memory content is personal and must never be included in fixtures, logs, telemetry, or published
evidence.

## Spaces

`CoworkSpaces` APIs expose spaces/folders/links/projects and memory-copy operations. The visible UI
for spaces was not fully exercised. Treat spaces as a higher-level organization layer above
sessions; exact layout/behavior is **unknown**.

## Projects and artifacts

Cowork can start from a project or folder and can produce artifacts/files. The artifact library is a
separate route; “New artifact” starts a live-artifact task immediately. See
[projects.md](projects.md) and [artifacts.md](artifacts.md).

## Scheduled work

Scheduled tasks can run Cowork jobs while the computer is awake and online. Templates include daily
briefing, inbox triage, meeting prep, weekly review, content ideas, and topic monitoring. See
[scheduled-tasks.md](scheduled-tasks.md).

## Browser

Cowork settings expose a preferred browser, built-in browser toggle, link-handling mode, and
allowed/blocked sites. The runtime `coworkBrowser` gate is unsupported here
(`cowork_browser_disabled`), so the in-app browser surface could not be exercised.

This is an important distinction:

- The **policy/configuration** exists and must be preserved.
- The **runtime browser** is not available in this build/platform.

Do not render a fake browser when the gate is unsupported.

## Tools and MCP

Cowork sessions can use managed and local MCP servers. The observed profile had two managed servers
and several developer servers. Tool availability is snapshotted per session; approvals can be
per-call/session/persistent per policy.

MCP startup errors are non-fatal. Observed error:

```text
MCP <server>: Couldn't start for Cowork and Code sessions.
Error: mcp-remote: fetch failed
```

The UI shows a persistent toast and a Developer settings action. Never dump raw tool output or
credentials into the toast.

## Permissions and safety

- Tool calls may require approval.
- File access is scoped to granted folders.
- Egress allowlist applies to tools/browser.
- “Always allow” appears only if policy enables persistent approvals.
- Scheduled tasks reuse approvals only within the configured lifetime.
- computer use, iOS simulator, and Cowork browser are unavailable/unsupported here and must not be
  emulated.

## States

| State | Behavior |
|---|---|
| Empty | Home greeting/composer; no tasks |
| Active | Running task with stop; active list updates |
| Waiting for approval | Explicit prompt; no automatic tool execution |
| Streaming | Incremental response/tool progress |
| Completed | Final transcript, files, artifacts |
| Error | Session-level error banner; retry/duplicate; draft retained |
| Archived | Removed from active list; retained on disk |
| Deleted | Confirmation; files/metadata removed per retention |
| Placement unavailable | Explain device/SSH/cloud policy; do not start a local fallback silently |
| Sandbox unavailable | Explain platform/VM limitation; do not claim isolation |

## Requirements

| ID | Requirement |
|---|---|
| `REQ-COWORK-01` | Cowork creates a local/remote session with explicit placement and folder grants. |
| `REQ-COWORK-02` | File reads outside granted/working folders are blocked by policy. |
| `REQ-COWORK-03` | Memory has an explicit toggle, list, and deletion flow; no content leaves the device for telemetry. |
| `REQ-COWORK-04` | Tool approvals and egress are enforced at execution, not only hidden in UI. |
| `REQ-COWORK-05` | MCP startup failures are non-blocking and actionable. |
| `REQ-COWORK-06` | Unsupported browser/computer-use/simulator surfaces show an honest unavailable state. |
| `REQ-COWORK-07` | Archived/deleted sessions follow retention and legal-hold rules. |
| `REQ-COWORK-08` | Session placement never silently falls back to another host. |

**Tests:** `TEST-UX-001`, `TEST-SESSION-001..010`, `TEST-MCP-001..010`, `TEST-SEC-002`.
