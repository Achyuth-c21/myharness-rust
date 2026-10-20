# MCP, Skills, and Tools

**Features:** `FEAT-TOOL-*`, `FEAT-CUST-05`, `FEAT-COWORK-10`
**Primary docs:** [12 — AI Architecture](../12-ai-architecture.md), [09](../09-settings-and-configuration.md)

## 1. MCP servers

MCP (Model Context Protocol) servers provide tools, resources, and prompts. The app supports:

- **Managed MCP servers** pushed by the organization (HTTP/SSE or stdio; may include bearer tokens).
- **Developer MCP servers** added locally via Settings → Developer (stdio).
- **Plugin MCP servers** bundled by installed plugins/orgs.
- **Built-in/local tools** such as registry search, browser, and file tools where enabled.

Transports:

- stdio (child process; command/args/env);
- HTTP;
- SSE.

Tool names are namespaced as `<server>:<tool>` in session metadata. A server can expose dozens or
hundreds of tools; the app supports tool search/on-demand schemas so it does not inject all of them
into every request.

## 2. Lifecycle

```text
config/policy → spawn/connect → initialize → list tools/resources
  → cache schemas → enable per session → approve/use → health → reconnect/shutdown
```

Rules:

- MCP failure is **non-fatal** to app boot.
- Startup failures produce a persistent, actionable toast (observed).
- Tool lists/schemas are cached but refreshed on reconnect.
- A server may require OAuth; tokens are per server and main/keychain-owned.
- The session snapshots enabled tools and tool surface, so a reconnect/mid-session change does not
  silently alter what a task is allowed to do.
- Shutdown must terminate child processes and clean up stdio pipes.

Observed failure:

```text
MCP <server>: Couldn't start for Cowork and Code sessions.
Error: mcp-remote: fetch failed
```

The toast includes **Open developer settings** and a close control. Do not include command lines,
tokens, or private paths.

## 3. Permissions and approvals

- Tool policy: allow / ask / deny, plus argument-scoped rules.
- Approval scopes: per call, session, and persistent “Always allow.”
- Persistent approvals are policy-controlled (`mcpPersistentAlwaysAllowEnabled`).
- Scheduled tasks have a separate approval lifetime
  (`mcpScheduledTaskApprovalLifetimeDays`; 0 disables lasting approval).
- Denied tools must never appear in the provider request.
- Argument-scoped policy must be checked against the actual arguments, not only the tool name.
- Approvals are recorded in audit/session metadata.
- A tool call cannot approve itself or change policy.

## 4. Egress and network

- `allowedMcpServers`/managed lists define which servers are allowed.
- Cowork egress allowlist applies to tool network calls and browser navigation.
- Local MCP/Python runtime may need PyPI/GitHub/package hosts; these are explicit egress categories.
- Proxy/PAC settings apply to app and agent traffic.
- Do not bypass policy by using an allowlisted host as a proxy to arbitrary destinations.
- Private/internal hosts must never be published in docs or egress exports.

## 5. Built-in tools and tool search

Configuration includes:

- `disabledBuiltinTools`
- `builtinToolPolicy`
- `toolPolicyEndpoint`
- `toolSearchEnabled`
- `toolSearch`/MCP registry search tools.

Tool search requirements:

- Search returns tool metadata/schema, not execution.
- Selecting/loading a tool does not grant approval.
- Schemas are validated and size-limited.
- Disabled tools remain unavailable even if search returns them.

## 6. Skills

Skills package instructions, assets, and sometimes scripts/tools. They are not providers/models.

Bundled skills (manifest):

- `docx` — create/edit Word documents.
- `pdf` — PDF generation/manipulation.
- `pdf-reading` — PDF extraction/reading.
- `pptx` — PowerPoint.
- `xlsx` — Excel.
- `frontend-design` — design/implementation guidance.

Other observed catalog entries include `consolidate-memory`, `context`, `explain-usage`,
`find-skills`, `schedule`, `setup-claude`, and Vercel composition patterns. These may be user/org/
bundled variants; do not hard-code.

Invocation:

- Add → Skills, or a slash command.
- Selecting a skill inserts a structured mention/instruction into the turn.
- The agent may load skill files/scripts; file access follows session folder grants.
- Skill output can create artifacts/documents.
- A skill cannot bypass tool approvals or egress.

Skill creation/upload is gated by `skillCreationEnabled` and org policy. The editor was not exercised.

## 7. Plugins and desktop extensions

Plugins bundle skills, MCP servers, and commands. Marketplaces are explicit sources; org bundles can
be mounted read-only. Desktop extensions (`.dxt`, `.mcpb`) have separate enable/directory/signature/
local-dev gates.

Requirements:

- Show contents/permissions before install.
- Verify signatures when required.
- Register MCP tools through the normal policy/approval path.
- Uninstall removes registrations/cache; don’t orphan processes.
- A disabled plugin’s tools disappear from new sessions (existing session behavior follows its
  snapshot/policy).

## 8. Prompt injection and untrusted output

MCP tool results, web pages, files, and model output are **data**, never instructions to the app or
agent runtime. Defenses:

- Treat tool output as untrusted; do not auto-execute commands found in it.
- Require approval for dangerous tools even if the model requests them.
- Apply egress/file policy at execution.
- Redact secrets from tool output before logging/showing where possible.
- Prevent tool output from altering system prompts/policy.
- Bound output size and handle malformed JSON/schema.

## 9. States

| State | Behavior |
|---|---|
| Not configured | Empty list + add/managed explanation |
| Starting | Connecting/initializing; non-blocking |
| Connected | Tools listed; status |
| Needs auth | Sign-in action |
| Failed | Error + retry + logs |
| Timeout | Cancel + error; no partial tool execution |
| Tool approval | Explicit prompt; scope choice |
| Disabled | Hidden/disabled with reason |
| Plugin removed | Tools unavailable; session marks change |

## 10. Requirements

| ID | Requirement |
|---|---|
| `REQ-TOOL-01` | MCP supports managed/developer/plugin servers over stdio/HTTP/SSE. |
| `REQ-TOOL-02` | Tool schemas are cached, namespaced, and snapshotted per session. |
| `REQ-TOOL-03` | Tool policy/approvals/argument scopes are enforced at execution. |
| `REQ-TOOL-04` | Egress/file policy applies to every tool network/file operation. |
| `REQ-TOOL-05` | MCP failure is non-blocking, actionable, and redacted. |
| `REQ-TOOL-06` | Skills are treated as instructions/assets, not model capabilities. |
| `REQ-TOOL-07` | Plugins/desktop extensions require explicit install and signature policy. |
| `REQ-TOOL-08` | Tool output cannot bypass policy or execute instructions. |
| `REQ-TOOL-09` | Tool-result images/files are preserved to the provider when supported. |

**Tests:** `TEST-MCP-001..010`, `TEST-AI-011/012/019/020`, `TEST-SEC-002/003`.
