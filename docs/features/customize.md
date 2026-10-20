# Customize — Skills, Connectors, Plugins

**Features:** `FEAT-CUST-*`, `FEAT-TOOL-06..09`
**Primary docs:** [09](../09-settings-and-configuration.md), [12](../12-ai-architecture.md)

## Purpose

Customize is the user-facing management surface for extending Claude’s capabilities. It is separate
from Settings and from the provider setup window.

Tabs: **Skills**, **Connectors**, **Plugins**.

## Skills

Observed:

- Search, filter, sort, and **Add**.
- Sections **Created by you** and **From Anthropic & Partners**.
- Rows (not cards) with icon, name, author, description, date, overflow.
- Skills are instructions/assets/tools, not models.

Observed bundled/user skill names included:

- `consolidate-memory`
- `context`
- `docx`
- `explain-usage`
- `find-skills`
- `frontend-design`
- `pdf`
- `pdf-reading`
- `pptx`
- `schedule`
- `setup-claude`
- `vercel-composition-patterns`
- `xlsx`

The bundled manifest ships `docx`, `pdf`, `pdf-reading`, `pptx`, `xlsx`, and `frontend-design`;
other entries are user/org skills or installed bundles. Do not hard-code the list.

Add menu:

- Create a skill (if `skillCreationEnabled`).
- Upload/import a skill package.
- Browse/manage skills.
- Policy may hide upload/create.

Skill actions:

- Open/edit details.
- Enable/disable in a session.
- Delete with confirmation.
- Inspect source/author/version.

The skill editor/detail was **not opened**; layout is unknown.

## Connectors

Observed:

- Filter: All / Connected / Not connected.
- Table columns: Connector, Type, Status.
- **Connect** action.
- Search.
- Error toast for failed MCP startup.

Connectors are MCP servers plus built-in integrations. They can be:

- managed by the organization;
- added by the user (local stdio) if policy allows;
- provided by plugins;
- built-in (e.g. registry/browser tools where supported).

Connector lifecycle:

```text
discover → configure/auth → connect → list tools → approve → use
         → health/error → reconnect/remove
```

OAuth/credential state:

- OAuth token per server in main/keychain.
- `forgetMcpOAuth` can clear it.
- Auth failures are actionable; do not silently disable a connector.
- Tool schemas are cached but may be stale; refresh on connect.

## Plugins

Observed empty state:

- **Add your first plugins** empty panel with puzzle illustration.
- **Discover** and **Add** actions.
- No plugins installed in this profile. Marketplaces are configured, but the Directory/install flow
  was not opened.

Plugin content:

- skills;
- MCP servers;
- commands;
- policies.

Marketplaces:

- Git repositories or hosted `marketplace.json` URLs.
- Organization marketplaces are re-fetched periodically.
- Org bundles can be mounted read-only under `/etc/claude/org-plugins`.
- User uploads/marketplaces are policy-controlled.

Install flow (unknown layout):

1. Browse marketplace.
2. Inspect plugin contents/permissions.
3. Confirm install.
4. Fetch/verify/signature-check.
5. Install manifest/cache.
6. Register skills/MCP with policy.
7. Enable/disable per session.

Uninstall must remove installed files and MCP registrations while preserving audit/history.

## MCP error state

The observed error:

```text
MCP <server>: Couldn't start for Cowork and Code sessions.
Error: mcp-remote: fetch failed
```

Presentation:

- Persistent bottom-right toast.
- Close control.
- **Open developer settings** action.
- Does not block the rest of the app.

The toast must not include tokens, local paths, or full command lines.

## States

| State | Behavior |
|---|---|
| Empty | Explanatory empty state + Add/Discover |
| Loading | Skeleton/spinner; no fake cards |
| Populated | Cards/table with hover/overflow/status |
| Connected | Status chip + manage/disconnect |
| Error | Status + error detail + retry |
| Needs auth | Connect/sign-in action |
| Policy-disabled | Explanation; Add/Upload hidden |
| Unsupported | Gate reason, no fake control |
| Install pending | Progress + cancel where safe |

## Requirements

| ID | Requirement |
|---|---|
| `REQ-CUST-01` | Skills have search/filter/sort, author/source metadata, and safe enable/delete. |
| `REQ-CUST-02` | Connectors show connection/auth/error state and a working connect path. |
| `REQ-CUST-03` | Plugins have an explicit install review and permission summary. |
| `REQ-CUST-04` | Org policy can disable user-added MCP/plugins/skills without hiding the reason. |
| `REQ-CUST-05` | MCP failure is non-blocking, actionable, and redacted. |
| `REQ-CUST-06` | Uninstall removes registrations/cache without deleting unrelated user data. |

**Tests:** `TEST-UX-007`, `TEST-MCP-001..010`, `TEST-CFG-006`.
