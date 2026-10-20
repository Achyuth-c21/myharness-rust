# 09 — Settings and Configuration

Configuration has two layers:

1. **End-user Settings** — preferences and local app behavior, opened from the account menu or
   `Ctrl+,`.
2. **Third-party inference setup** — the separate `setup-desktop-3p` window where an administrator or
   user defines provider credentials, capabilities, policy, telemetry, limits, egress, and updates.

A rebuild must keep these surfaces separate. Inference configuration is not a tab in Settings; it is
a separate window with its own profile lifecycle and apply/relaunch semantics.

## 1. End-user Settings

### 1.1 Modal structure

- Centered modal over the current route, ~1400×960 at 1920×1080.
- Left column: search, section navigation, Customize shortcuts.
- Right column: scrollable section content.
- Close via X or Escape; there is no global Save for preference toggles.
- Section is reflected in the URL hash (`#settings/general`, `#settings/cowork`, …).
- A persistent MCP error toast may appear above the footer and links directly to Developer settings.

### 1.2 General

| Group | Controls | Behavior |
|---|---|---|
| Profile | Avatar, display identity, “What best describes your work?” select | Local profile metadata; provider identity shown separately |
| Instructions for Claude | Multiline global instructions | Applies across chats/Cowork for associated accounts, within policy |
| Appearance | Theme: System/Light/Dark; Chat font: Anthropic Serif or system | Applied live; theme persisted |
| Motion | Reduce animation switch; System/Reduced choice | Reduces streaming/interface animation |
| Notifications | Response completions switch | Notify when a long response finishes |

The General footer shows the MCP startup error when present, with **Open developer settings**.

**Verified.**

### 1.3 Privacy

The Privacy section is a disclosure page, not a local telemetry dashboard. It states:

- Inference requests go to the configured third-party provider, not Anthropic.
- Anthropic may still receive crash/performance reports, usage analytics without message content,
  update checks, and explicitly uploaded diagnostic bundles.
- Connector favicons, artifact/MCP-App iframes, and the Linux spellcheck dictionary are
  “nonessential services” and may be blocked by policy.
- A diagnostic-report action is available and clearly says what will be sent.

The exact provider hostname is deployment-specific and must not be hard-coded in the rebuild.

**Verified** for copy and available action.

### 1.4 Usage

- Period selector: 7d / 30d / 90d.
- Cowork: input + output tokens, web searches.
- Code: input + output tokens, web searches.
- “Tokens per day” chart with Cowork/Code series and date axis.
- Disclosure: provider bill covers the whole organization and charges cached tokens, so local totals
  will not match; costs are hidden unless pricing is enabled.

Usage is **device-local** and derived from a usage ledger. It is not an organization billing console.
When `inferenceModelPricingEnabled` is true, an estimated cost and multiplier/per-model rates may be
shown at Anthropic list price.

**Verified.**

### 1.5 Claude Code

Observed groups and controls:

| Group | Controls |
|---|---|
| Appearance | Code appearance (Claude Light/Dark), Code font |
| Typography | Interface font (Anthropic Sans/System/OpenDyslexic), transcript size/width |
| Conversation | Default transcript view (Normal/Thinking/Verbose) |
| Sessions | Archive inactive sessions, session retention, keep awake |
| Worktrees | Default location; branch prefix |
| Model behavior | “Switch models when a message is flagged” |
| Output | Output style |
| Sandbox | Local sandbox, strict sandbox, bypass permissions policy |
| Browser | Browser tools, open links in browser, cookie policy, allowed sites |
| Notifications | Task/response notifications |

Some controls are policy-locked or hidden by the active capability profile. Labels such as “Use
Claude Code for desktop” and “Enable local MCP servers” are policy/feature-driven.

**Verified** for visible layout and labels; exact defaults are profile-dependent.

### 1.6 Cowork

| Group | Controls |
|---|---|
| Files | Cowork working-folder path; **Change**; trusted folders list and **Manage** |
| Browser | Preferred browser, built-in browser toggle, link-handling mode, allowed/blocked sites |
| Instructions | Global Cowork instructions |
| Memory | Global memory toggle; list of memory entries; add/edit/delete actions |

Memory contents are personal data and are not reproduced. The folder path is shown as `~/Claude` in
this documentation; the actual path is machine-specific.

The browser controls can be visible even when the runtime `coworkBrowser` gate is unsupported. Treat
the setting and the runtime capability as separate: a policy can configure a browser that the current
build cannot render.

### 1.7 Import & export

On this deployment the section explains that import is unavailable because it is not enabled for
third-party inference deployments. Export settings exist in the configuration schema but are
disabled by the active `claudeAiImport` policy:

```json
{
  "enabled": false,
  "automatic3pImport": false,
  "exportEnabled": false
}
```

Import/export is **policy-disabled**, not a broken menu.

### 1.8 System

Observed controls:

- Current version (`2.42304.0` here) and update check/state.
- Run on startup.
- Quick Entry global shortcut (feature-gated; panel unavailable here).
- Tray icon preference; this Linux desktop reports no tray support.
- Keep computer awake for scheduled tasks.
- Possibly zoom/window reset actions; exact list is build-dependent.

### 1.9 Developer

- List of local MCP servers with enable/disable, status, command, arguments, environment/log access.
- **Edit config** opens the MCP JSON configuration file.
- **View logs** opens the MCP server log.
- Add/remove local server flows.
- The section is visible when developer mode is enabled; local MCP may be separately policy-gated.

The observed local developer servers were scientific/utility MCPs; names are redacted.
Two managed MCP servers were configured in the 3P profile. Do not treat this list as universal.

### 1.10 Customize shortcuts

Settings links to Skills, Connectors, and Plugins. These are also available from the main sidebar’s
**Customize** surface. They are management views, not settings toggles; their detailed interaction is
in [features/customize.md](features/customize.md).

## 2. Third-party inference setup window

### 2.1 Shell

- Separate Electron window at `app://localhost/setup-desktop-3p`.
- Header: “Configure third-party inference”, active profile selector, Export menu, close.
- Tabs: **Connection**, **Capabilities**, **Connectors**, **Telemetry & updates**, **Limits**,
  **Appearance**, **Plugins**, **Egress**, **Source**.
- Footer/action: **Apply Changes**. Edits are draft until applied.
- The window can be opened from the main account menu or directly by URL.
- The active profile is a named configuration in `configLibrary`; `_meta.json` records selection.

**Verified.**

### 2.2 Connection

The provider selector is a card-style list. Selecting a provider changes credentials, endpoint
fields, and model validation.

#### Shared fields

- Credential kind. The help text is explicit: “When set, only that source is used (no fallback).”
- Custom inference headers. Intended for routing/tenant headers (organization IDs, guardrails), not
  credentials. Credential-like header names/values are warned about.
- Stream idle timeout (gateway only; extra seconds while only keep-alives arrive; default 300).
- Model discovery and explicit model list.
- Default effort, 1M preference, pricing, model catalog.
- Network proxy, PAC URL, workspace VM IPv6.

#### Gateway

| Field | Notes |
|---|---|
| Gateway base URL * | Full URL of the inference gateway endpoint |
| Gateway API key * | Static credential for bearer/x-api-key auth |
| Credential kind | Static key, interactive sign-in, or helper script |
| Auth scheme | `bearer` or `x-api-key`; UI shows `bearer` default |
| Artifact preview iframe origin | HTTPS origin of user-content renderer; defaults to commercial host |
| Custom headers | Routing/tenant only; no credentials |
| Stream idle timeout | Default 300 s |
| Gateway-managed Claude Code | Run Claude Code in gateway mode for gateway sign-in |
| Model discovery | Autopopulate from `<base>/v1/models` |
| Model list | Override discovered list; first entry is default |
| Default model effort | low / medium / high / xhigh / max |
| Model catalog | Published Claude Code catalog for names/descriptions/effort |
| Proxy/PAC | Network egress |
| VM IPv6 | Cowork VM network |

The gateway contract is Anthropic-compatible: `/v1/models` is called with
`anthropic-version: 2023-06-01`; inference is `/v1/messages`; SSE is streamed. There is no
protocol-translation layer in the shipped architecture. See [12-ai-architecture.md](12-ai-architecture.md).

#### Claude API

- API key or interactive Anthropic sign-in; optional base URL; model list; custom headers.
- OAuth access tokens beginning with `sk-ant-oat` receive an OAuth beta header.
- Anthropic-host delivery is policy-controlled and can be disabled for 3P deployments.

#### Bedrock

- Region, profile/SSO/web-identity/static bearer/helper script.
- Optional base URL, service tier.
- Model IDs must be Bedrock Anthropic IDs (`arn:...`/`anthropic.*`) or application-inference /
  provisioned-model ARNs. The UI warns that the underlying model is not verified for profiles/ARNs.
- CLI environment: `CLAUDE_CODE_USE_BEDROCK=1`, region/profile/credentials.

#### Bedrock Mantle

- Region/base URL; static, helper, or vendor credential.
- Model discovery is disabled for static/helper-script modes.

#### Vertex AI

- Project ID, region, Google sign-in/Workforce/helper/vendor profile.
- Credentials file and OAuth client fields.
- Model ID must start with `claude-` and contain no `/`, `\`, `?`, `#`, `%`, or whitespace.
- Model discovery is disabled for interactive/workforce/helper/vendor-profile credential kinds.

#### Foundry

- Resource/base URL; API key, interactive, or helper.
- Deployment name must resolve to an Anthropic model (validated with the same model policy).
- Model discovery is disabled for several credential kinds; static/helper omit bearer auth.

#### Credential kinds

| Kind | Meaning | Notes |
|---|---|---|
| `static` | Stored API key | Secret; file mode should be restricted |
| `helper-script` | External command returns credential | Arguments, timeout, TTL, silent refresh |
| `interactive` | OAuth/device-code/browser flow | Tokens cached separately |
| `vendor-profile` | AWS/GCP/Azure vendor identity | No static secret in config |
| `workforce` | Google Workforce identity | Vertex |
| `external-idp` | External identity provider | Bedrock/Vertex variants |

`_we(e)` detects `sk-ant-oat` tokens; the UI chooses the OAuth beta header from that.

### 2.3 Capabilities

Allowed surfaces:

- Chat, Cowork, Code. The “unified Chat and Cowork view” is a Beta gate.
- Disabling a surface removes its entry points, sessions UI, and related controls.

General restrictions:

- **Allowed egress hosts** — hosts agent tools may reach; `*` means unrestricted for tools but the
  built-in browser still needs concrete hosts. Empty acts as unset for tools and blocks browser
  navigation.
- **Allowed workspace folders** — folders where agents may work; unset = unrestricted, “Allow none”
  = deny all.
- **Block reads outside working directories** — file tools refuse outside reads; sandboxed shell
  loses home directory.
- **Allow Auto mode** — offer Auto permission mode in Code.
- **Bypass permissions** — admin toggle; disabled means the UI must not offer it.
- **Tool policy** — allow/ask/deny defaults and argument-scoped rules.
- **Built-in tools** — disable specific tools.
- **Browser** — enabled/default domain policy/allowed/blocked domains.
- **Claude in Chrome** and password-manager integration.
- **Dictation**.
- **Feature announcements**.
- **Organization instructions**.
- **Desktop local sessions / SSH / cloud** placement policy.

Every restriction is enforced on the app side and repeated at the agent/tool boundary. Hiding a UI
control alone is not sufficient.

### 2.4 Connectors

- **Managed MCP servers**: org-pushed remote HTTP/SSE or local stdio commands; may embed bearer
  tokens. Header “Authorization” is treated as a credential; prefer a headers helper so the secret
  is not stored in config.
- **Allow persistent tool approvals**: enables “Always allow” options.
- **Scheduled-task tool approval lifetime**: days before a lasting approval is re-requested; `0`
  removes the lasting option.
- **Allow user-added MCP servers**: local stdio servers via Developer settings.
- **Allowed/denied plugin MCP servers**: servers plugins may connect to beyond managed/org plugins.
- **MCP tool timeout**.
- **Desktop extensions**: enable directory, require signature, allow local development MCP.
- **Import third-party history** and org UUID.

### 2.5 Telemetry & updates

- Organization UUID (tags telemetry and namespaces local data; not authentication).
- Block essential telemetry (crash/performance).
- Block nonessential telemetry (usage analytics without content, diagnostic uploads).
- Block nonessential services (connector favicons, artifact/MCP-App iframes, Linux spellcheck).
- OpenTelemetry endpoint.
- Block auto-updates.
- Auto-update enforcement window (hours before forced install; default 72).
- Check updates at `releases.claude.com` so `api.anthropic.com` can remain blocked.
- Configuration relaunch window (hours; default 24; 0 = immediate).
- Configuration re-check interval (minutes; default 10).

### 2.6 Limits

- Chat retention days (projects and memory stay).
- Cowork retention days (spaces and memory stay).
- Code retention days (uncommitted work stays on disk).
- Suspend session deletion / legal hold.
- Max tokens per window per device. Cached tokens do not count, so this does not cap provider spend.

These are local policy controls; the provider’s own billing/limits are separate.

### 2.7 Appearance

- Hide feature announcements.
- End-user attribution: show identity-provider identity in sidebar/account and emit
  `enduser.id` as an OpenTelemetry resource attribute.
- Deployment display name and subtitle.
- Hide configuration deprecation warnings.
- Possibly theme/branding options; exact list is configuration-dependent.

### 2.8 Plugins

- Organization plugin marketplaces: Git repositories or hosted `marketplace.json` URLs, re-fetched
  periodically.
- Organization plugins: mount bundles under `/etc/claude/org-plugins`; loaded at launch; read-only.
- Per-server tool policies saved in the configuration.
- User plugin marketplaces/uploads are separate policy controls.

The observed profile had six marketplaces and no installed organization plugins.

### 2.9 Egress

Read-only derived allowlist, grouped as:

- **Core downloads:** `downloads.claude.ai`.
- **Auto-updates:** `claude.ai`, `api.anthropic.com` (unless update host is redirected).
- **Essential telemetry:** Sentry and Datadog intake hosts.
- **Nonessential services:** Google static/fonts, `claudeusercontent.com`, cdnjs, jsDelivr,
  `claudemcpcontent.com`, `assets.claude.ai`, update/redirect hosts.
- **Gateway base URL** (deployment-specific).
- **Plugin marketplaces** (e.g. GitHub).
- **Managed MCP servers** (deployment-specific).
- **User-added MCP/Python runtime** (PyPI, GitHub objects, Python package hosts).
- **SSH/identity endpoints** where applicable.

Actions: **Test connectivity**, **Copy hostnames**, **Download .txt**. These are sensitive: the list
can reveal private infrastructure. Documentation must never publish a private hostname.

### 2.10 Source

- **Bootstrap config URL**: HTTPS endpoint returning a per-user JSON overlay. Values from the
  response override local settings and become read-only.
- A warning explains that this makes the profile organization-managed.
- Source is the last tab; the actual JSON is not shown here, only the URL.

**Verified.**

## 3. Configuration model

### 3.1 Stores

| Store | Path | Purpose |
|---|---|---|
| App config | `~/.config/Claude-3p/config.json` | preferences, locale, theme, window boot state, OAuth caches |
| Profile library | `~/.config/Claude-3p/configLibrary/*.json` | provider/capability/policy profiles |
| Library metadata | `~/.config/Claude-3p/configLibrary/_meta.json` | active profile, ordering |
| MCP config | user-data MCP configuration file | local MCP servers |
| Bootstrap cache | managed config/cache paths | remote overlay and health |
| Model catalog | `~/.config/Claude-3p/model-catalog/published.json` | signed catalog + sidecar |
| Secrets | OS keychain where available; else profile/config fields with restricted permissions | credentials |

Secrets are not duplicated into documentation. The current profile stores some credentials in the
config file; that is an operational risk to fix in the rebuild.

### 3.2 Profile lifecycle

```text
create/duplicate profile → edit draft → validate → Apply Changes
    → write profile → setActive → (maybe) relaunch
    → main window reads merged config → new provider clients
```

- Switching profiles can require relaunch; a session guard warns if work is active.
- `setAppliedConfig`, `getMergedConfig`, and `getConfigHealth` support validation.
- A managed overlay can make fields read-only.
- Deprecated keys are surfaced unless warnings are hidden.
- A failed health check can mark a landed remote configuration as tainted; `recheckConfigHealth`
  repairs/retries.
- `disableLocalConfigCache` prevents using a stale local copy when managed config is unreachable.

**Verified** for API names and UI; exact precedence is **unknown**.

### 3.3 Key groups

All 186 `flatKey` descriptors are indexed in
[20-configuration-key-index.md](20-configuration-key-index.md). Major groups:

| Group | Examples |
|---|---|
| Provider/inference | `inferenceProvider`, `inferenceCredentialKind`, `inferenceGatewayBaseUrl`, `inferenceModels`, `inferenceCustomHeaders` |
| Model behavior | `defaultModelEffort`, `alwaysStartWithDefaultModel`, `modelPrefer1mContext`, `modelDiscoveryEnabled`, `modelCatalogEnabled` |
| Credentials | `inferenceGatewayApiKey`, `inferenceBedrock*`, `inferenceVertex*`, `inferenceFoundry*`, helper TTL/timeout |
| Surfaces | `coworkTabEnabled`, `chatTabEnabled`, `isClaudeCodeForDesktopEnabled`, `scheduledTasksEnabled` |
| Tools/MCP | `managedMcpServers`, `mcpPersistentAlwaysAllowEnabled`, `toolSearchEnabled`, `disabledBuiltinTools`, `builtinToolPolicy` |
| Files/browser/egress | `allowedWorkspaceFolders`, `blockReadsOutsideWorkingDirectories`, `coworkEgressAllowedHosts`, `builtinBrowser*` |
| Plugins | `allowedPluginMarketplaces`, `orgPluginSettings`, `userPluginUploadsEnabled`, `isDesktopExtension*` |
| Telemetry | `disableEssentialTelemetry`, `disableNonessentialTelemetry`, `otlp*`, `usageMetricsEnabled` |
| Updates | `disableAutoUpdates`, `autoUpdaterEnforcementHours`, `releaseChannel`, `updateViaUpdatesHost` |
| Retention/limits | `chatSessionRetentionDays`, `coworkSessionRetentionDays`, `codeSessionRetentionDays`, `inferenceMaxTokensPerWindow` |
| Identity/SSO | `loginSsoOrgDomain`, `entra*`, `inferenceIdp*`, `forceLoginOrgUUID`, `endUserAttribution` |
| Bootstrap | `bootstrapUrl`, `bootstrapEnabled`, `bootstrapOidc`, `bootstrapHeaders`, `trustBootstrapDelivery` |

### 3.4 Validation rules observed in source

- Gateway base URL must parse and be reachable for model discovery; trailing slashes normalized.
- Gateway model discovery: `GET <base>/v1/models?limit=1000`, following `after_id` for at most three
  pages, 10 s timeout per page, partial results retained on later-page failure.
- Model IDs must satisfy the provider’s policy:
  - Gateway/Claude API/Foundry: Anthropic model policy or provider alias.
  - Bedrock: Anthropic vendor prefix or approved ARN shape.
  - Vertex: starts with `claude-`; no `/`, `\`, `?`, `#`, `%`, whitespace.
- Credential header names are checked for secrets; the UI warns against storing tokens in
  `inferenceCustomHeaders`.
- Model discovery off + model list empty is invalid for providers that require a model.
- `0` has explicit meaning for approval lifetime and relaunch window; blank means default.
- Retention fields accept days; legal hold overrides them.
- Egress host entries must be parseable hosts; `*` is special-cased.

### 3.5 Precedence and read-only state

The UI discloses:

- Bootstrap values **override local settings** and become read-only.
- Managed organization policy can lock toggles.
- The provider credential kind, once set, is the only source used — no fallback.

The exact merge order among local profile, bootstrap overlay, organization MDM, and runtime defaults is
not fully proven. Treat precedence as a contract to define explicitly in the rebuild, with a visible
“managed by” indicator for every locked field.

## 4. Sensitive data and security

- Never log API keys, OAuth tokens, helper output, or custom headers.
- The setup Export action can contain secrets or embedded tokens; warn and require explicit action.
- Managed MCP server definitions may embed bearer tokens; prefer headers helpers.
- Egress lists and MCP URLs can disclose private infrastructure.
- Store secrets in the OS keychain where available; otherwise use mode `0600` files and a clear
  warning. Do not write them into renderer-accessible storage.
- Redact authorization headers and query strings from logs.
- The diagnostic bundle must enumerate its contents and require explicit consent before upload.

## 5. Configuration requirements

| ID | Requirement |
|---|---|
| `DATA-CONFIG-01` | Separate user preferences from managed inference profiles. |
| `DATA-CONFIG-02` | Store credentials in the OS keychain; never expose them to the renderer. |
| `DATA-CONFIG-03` | Validate provider/model combinations before applying. |
| `DATA-CONFIG-04` | Show the effective source (local, bootstrap, organization) and lock state per field. |
| `DATA-CONFIG-05` | Apply changes atomically; a failed apply must leave the previous profile intact. |
| `DATA-CONFIG-06` | Relaunch only when required, warning about active sessions. |
| `DATA-CONFIG-07` | Keep provider-native credentials/flows; no universal gateway translation layer. |
| `DATA-CONFIG-08` | Enforce policy at the tool boundary, not only by hiding controls. |
| `DATA-CONFIG-09` | Re-check managed configuration on the configured interval and surface health. |
| `DATA-CONFIG-10` | Redact secrets from logs, exports, screenshots, and diagnostic bundles. |
