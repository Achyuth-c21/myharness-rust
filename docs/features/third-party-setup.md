# Third-Party Inference Setup

**Features:** `FEAT-SET-09`, `FEAT-MOD-08`, `FEAT-ACCT-03`
**Primary docs:** [09 — Settings and Configuration](../09-settings-and-configuration.md), [12 — AI Architecture](../12-ai-architecture.md)

## Purpose

The separate `setup-desktop-3p` window is the authoritative control plane for provider credentials,
models, surfaces, tools, telemetry, retention, egress, and managed configuration. It can open even
when the main app cannot initialize a provider.

It is **not** a Settings tab and does not share the Settings modal state.

## Shell

- Separate Electron window at `app://localhost/setup-desktop-3p`.
- Header: “Configure third-party inference”, profile selector, Export, close.
- Tabs: Connection, Capabilities, Connectors, Telemetry & updates, Limits, Appearance, Plugins,
  Egress, Source.
- Footer action: **Apply Changes**.
- Draft editing; **Discard Changes** restores applied values.
- Apply validates and may relaunch; active-session guard may warn.

## Profile model

A profile is a named JSON document in the config library. The header selector can switch profiles;
each profile has its own provider/capability/policy. `_meta.json` tracks the applied profile.

Workflow:

```text
select/create profile → edit draft → validate → Apply Changes
  → write atomically → set active → relaunch if required
```

A remote bootstrap overlay can make fields read-only. Do not edit/publish a real profile export.

## Connection tab

Provider cards:

- **Gateway** — Anthropic-compatible endpoint; static/interactive/helper credential; auth scheme
  bearer/x-api-key; artifact preview origin; custom routing headers; stream idle timeout; gateway-
  managed Claude Code; model discovery/list/effort/1M/pricing/catalog; proxy/PAC; VM IPv6.
- **Claude API** — API key/interactive; optional base URL; model list; custom headers.
- **Bedrock** — region, profile/SSO/web identity/static bearer/helper; base URL; service tier;
  Anthropic model ID/ARN validation.
- **Bedrock Mantle** — region/base URL; credential; model routing.
- **Vertex AI** — project/region; Google/Workforce/helper/vendor profile; credentials file;
  `claude-` model validation.
- **Foundry** — resource/base URL; API key/interactive/helper; deployment-name validation.

Credential kind is exclusive: “When set, only that source is used (no fallback).” Custom headers are
for routing/tenant metadata, not secrets; credential-shaped values are warned about.

## Capabilities tab

- Allowed surfaces: Chat, Cowork, Code; unified Chat/Cowork beta.
- Allowed egress hosts.
- Allowed workspace folders / allow-none.
- Block reads outside working directories.
- Auto mode / bypass permissions.
- Built-in tool policy and disabled tools.
- Browser enable/policy/allowed/blocked domains.
- Claude in Chrome/password managers.
- Dictation, feature announcements, organization instructions.
- Local/SSH/cloud placement and related restrictions.

Controls must be enforced at the tool boundary, not only hidden.

## Connectors tab

- Managed MCP servers (HTTP/SSE or stdio), including embedded bearer tokens warning.
- Persistent tool approvals.
- Scheduled-task approval lifetime.
- User-added local MCP allow.
- Allowed/denied plugin MCP servers.
- MCP tool timeout.
- Desktop extensions/directory/signature/local-dev.
- Third-party history import.

## Telemetry & updates tab

- Organization UUID (telemetry namespace, not auth).
- Block essential/nonessential telemetry.
- Block nonessential services.
- OpenTelemetry endpoint.
- Block auto-updates; enforcement window (default 72 h).
- Update feed via `releases.claude.com`.
- Configuration relaunch/recheck windows.

## Limits tab

- Chat/Cowork/Code retention days.
- Legal hold/suspend deletion.
- Max tokens per window/device; cached tokens excluded.

## Appearance tab

- Hide feature announcements.
- End-user attribution and OTel `enduser.id`.
- Deployment display name/subtitle.
- Hide configuration deprecation warnings.
- Other branding/theme options as configured.

## Plugins tab

- Organization marketplaces (Git/hosted `marketplace.json`), periodic refresh.
- Org plugin bundles under `/etc/claude/org-plugins` (read-only, loaded at launch).
- Per-server tool policies.
- User marketplace/upload policy.

## Egress tab

Read-only derived firewall allowlist grouped by downloads, updates, essential telemetry,
nonessential services, gateway, marketplaces, managed MCP, user MCP/Python, SSH/identity. Actions:
test connectivity, copy hostnames, download `.txt`. The list can reveal private infrastructure;
treat it as sensitive.

## Source tab

Bootstrap config URL: HTTPS endpoint returning a per-user overlay. Response values override local
settings and become read-only. Trust requires HTTPS/authentication; exact trust/signature policy is
unknown.

## Validation and error states

- Required fields marked `*`; Apply blocks/shows errors.
- Provider/model validation per family.
- Gateway discovery: 3 pages, 10 s, partial retention after first page.
- Health check can mark a managed config tainted; re-check/repair.
- Invalid credential/helper output is an error; no fallback credential kind.
- Test connection/probe actions must not persist a draft.
- Apply is atomic; failed apply leaves the previous profile.

## Security

- Secrets in keychain, never renderer storage/logs.
- Export can contain secrets; require warning and redaction.
- Never publish private hostnames, MCP URLs, or Tailscale/VPC endpoints.
- Do not use Apply/Test during evidence collection unless required; use Discard.

## Requirements

| ID | Requirement |
|---|---|
| `REQ-SETUP-01` | Setup is a separate window and survives provider boot failure. |
| `REQ-SETUP-02` | Six providers with native credential/model validation. |
| `REQ-SETUP-03` | Apply is atomic; Discard restores exactly. |
| `REQ-SETUP-04` | Managed overlays are read-only and visibly sourced. |
| `REQ-SETUP-05` | Credential kinds are exclusive; no fallback. |
| `REQ-SETUP-06` | Policy/egress/telemetry fields are enforced, not cosmetic. |
| `REQ-SETUP-07` | Export/support bundle is redacted and explicit. |

**Tests:** `TEST-CFG-001..012`, `TEST-AI-001..007`, `TEST-SEC-004`.
