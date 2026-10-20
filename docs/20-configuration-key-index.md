# 20 — Configuration Key Index

**Evidence grade:** key names and categories are **verified** from the pristine main bundle; UI labels
are verified where the descriptor carries an inline label. Value classes marked *inferred* are for
navigation only. The Zod schema in the bundle is authoritative.

**Sources:** `evidence/app-asar-orig/.vite/build/index.chunk-Z7WxBjyE.js`, the setup-window capture,
and [09](09-settings-and-configuration.md). The live `configLibrary` was read for shape only; no
secrets or personal values are reproduced here.

## 1. How to read this index

- **Profile key** — a `flatKey` in a third-party inference config profile. 186 unique keys exist;
  provider-specific keys are grouped under Connection.
- **Value class** — broad shape, not a TypeScript type. `secret` means the key must never reach the
  renderer or a log. `array/object` requires schema validation per provider.
- **Undocumented** — key had `visibility: "public-undocumented"` in the bundle. It is still
  schema-validated but is not presented as a supported end-user control.
- **Scope** — nearly all profile keys are 3P-only unless noted; the active profile always applies
  through the setup window, not a renderer preference.

Precedence (verified/inferred from lifecycle): managed/bootstrap constraints > active profile >
local app preferences > defaults. Read-only managed keys cannot be overridden by the setup window.
A failed apply leaves the previously applied profile active.


## 2. Profile keys by category

### 2.1 Connection (75 keys)

| Key | UI label / meaning | Value class | Notes |
|---|---|---|---|
| `allowOptionalClientAuth` | Allow optional client-certificate fallback | boolean | Undocumented. |
| `alwaysStartWithDefaultModel` | Always start with the default model | boolean |  |
| `coworkVmIpv6Enabled` | Enable IPv6 in the workspace VM | boolean |  |
| `defaultModelEffort` | Default model effort | enum/string |  |
| `dictationEnabled` | Dictation | boolean | Undocumented. |
| `disableMultiAccount` | Single account only | boolean | Undocumented. |
| `egressProxyPacUrl` | Proxy auto-config (PAC) URL | URL string |  |
| `egressProxyUrl` | Proxy server URL | URL string |  |
| `entraAzureCloud` | Azure cloud | enum/string |  |
| `entraClientId` | Entra application (client) ID | string |  |
| `entraScope` | Delegated scope | string |  |
| `forceLoginOrgUUID` | Required organization | string (UUID) |  |
| `importThirdPartyHistoryIntoOrgUUID` | Import earlier third-party sessions into this organization | string (UUID) | Undocumented. |
| `inferenceAnthropicApiKey` | Claude API key | secret string |  |
| `inferenceBedrockAwsCliPath` | AWS CLI path | string |  |
| `inferenceBedrockAwsDir` | AWS config directory | string |  |
| `inferenceBedrockBaseUrl` | Bedrock base URL | URL string |  |
| `inferenceBedrockBearerToken` | AWS bearer token | secret string |  |
| `inferenceBedrockProfile` | AWS profile name | string |  |
| `inferenceBedrockRegion` | AWS region | string |  |
| `inferenceBedrockServiceTier` | Bedrock service tier | enum/string |  |
| `inferenceBedrockSsoAccountId` | AWS SSO account ID | string |  |
| `inferenceBedrockSsoRegion` | AWS SSO region | string |  |
| `inferenceBedrockSsoRoleName` | AWS SSO role name | string |  |
| `inferenceBedrockSsoStartUrl` | AWS SSO start URL | URL string |  |
| `inferenceBedrockWebIdentityRoleArn` | IAM role for the Microsoft 365 add-in | string |  |
| `inferenceCredentialHelper` | Helper script | string |  |
| `inferenceCredentialHelperArgs` | Helper script arguments | string |  |
| `inferenceCredentialHelperSilentRefreshEnabled` | Re-run helper for silent refresh | boolean |  |
| `inferenceCredentialHelperTimeoutSec` | Credential helper timeout | number |  |
| `inferenceCredentialHelperTtlSec` | Helper script TTL | number |  |
| `inferenceCredentialHelperWindows` | Helper script (Windows) | string |  |
| `inferenceCredentialKind` | Credential kind | enum/string |  |
| `inferenceCustomHeaders` | Custom inference headers | object |  |
| `inferenceFoundryApiKey` | Azure AI Foundry API key | secret string |  |
| `inferenceFoundryAuthFlow` | Entra ID sign-in flow | enum/string |  |
| `inferenceFoundryBaseUrl` | Azure AI Foundry base URL | URL string |  |
| `inferenceFoundryClientId` | Entra ID client ID | string |  |
| `inferenceFoundryResource` | Azure AI Foundry resource name | string |  |
| `inferenceFoundryTenantId` | Entra ID tenant ID | string |  |
| `inferenceGatewayApiKey` | Gateway API key | secret string |  |
| `inferenceGatewayAuthScheme` | Gateway auth scheme | enum/string |  |
| `inferenceGatewayBaseUrl` | Gateway base URL | URL string |  |
| `inferenceGatewayManagedClaudeCode` | Gateway-managed Claude Code | boolean | Undocumented. |
| `inferenceGatewayOidc` | Gateway SSO IdP (OIDC) | object |  |
| `inferenceGatewayOidcAuthFlow` | Gateway sign-in flow | enum/string |  |
| `inferenceIdpAuthFlow` | Identity provider sign-in flow | enum/string |  |
| `inferenceIdpOidc` | Identity provider (OIDC) | object |  |
| `inferenceModelPricing` | Model pricing | object |  |
| `inferenceModelPricingEnabled` | Show estimated cost | boolean |  |
| `inferenceModelPricingMultiplier` | Price multiplier | number |  |
| `inferenceModels` | Model list | array/object |  |
| `inferenceProvider` | Inference provider | enum/string |  |
| `inferenceSessionLifetimeSec` | Sign-in session lifetime | number |  |
| `inferenceStreamIdleTimeoutSec` | Stream idle timeout | number |  |
| `inferenceVertexBaseUrl` | Vertex AI base URL | URL string |  |
| `inferenceVertexCredentialsFile` | GCP credentials file path | string |  |
| `inferenceVertexOAuthClientId` | Vertex OAuth client ID | string |  |
| `inferenceVertexOAuthClientSecret` | Vertex OAuth client secret | secret string |  |
| `inferenceVertexOAuthLoginHint` | Vertex OAuth login hint | string |  |
| `inferenceVertexOAuthScopes` | Vertex OAuth scopes | array/object |  |
| `inferenceVertexProjectId` | GCP project ID | string |  |
| `inferenceVertexRegion` | GCP region | string |  |
| `inferenceVertexWorkforceAudience` | Workforce Identity audience | string |  |
| `inferenceVertexWorkforceAuthFlow` | Workforce Identity sign-in flow | enum/string |  |
| `inferenceVertexWorkforceOidc` | Workforce Identity IdP (OIDC) | object |  |
| `inferenceVertexWorkforceUserProject` | Workforce Identity billing project | string |  |
| `loginSsoOrgDomain` | SSO login domain | string |  |
| `modelCatalogEnabled` | Model catalog metadata | boolean |  |
| `modelCatalogUrl` | Model catalog URL | URL string |  |
| `modelDiscoveryEnabled` | Model discovery | boolean |  |
| `modelPrefer1mContext` | Use 1M wherever it\u2019s available | string |  |
| `selfHostedToken` | Self-hosted bearer token | secret string | Undocumented. |
| `selfHostedUrl` | Self-hosted execution URL | URL string | Undocumented. |
| `userContentRendererUrl` | Artifact preview iframe origin | URL string |  |

### 2.2 Capabilities and sandbox (48 keys)

| Key | UI label / meaning | Value class | Notes |
|---|---|---|---|
| `allowedWorkspaceFolders` | Allowed workspace folders | array/object |  |
| `autoModeEnabled` | Allow Auto mode | boolean |  |
| `blockReadsOutsideWorkingDirectories` | Block reads outside working directories | boolean |  |
| `builtinBrowserAllowedDomains` | Allowed sites in the built-in browser | array/object |  |
| `builtinBrowserBlockedDomains` | Blocked sites in the built-in browser | array/object |  |
| `builtinBrowserDefaultDomainPolicy` | Default site policy in the built-in browser | enum/string |  |
| `builtinBrowserEnabled` | Allow the built-in browser | boolean |  |
| `builtinToolPolicy` | Built-in tool policy | string |  |
| `chatAdvancedFileAnalysisEnabled` | Advanced file analysis | boolean |  |
| `chatTabEnabled` | Allow Chat | boolean |  |
| `claudeCodeProcessWrapperEnabled` | Use Claude Code\u2019s corporate launcher | boolean | Undocumented. |
| `claudeCodeSettings` | Claude Code Settings | object | Undocumented. |
| `codeAllowedRepositories` | Allowed repositories | array/object | Undocumented. |
| `coworkEDRIntegration` | EDR Integration package path | string (path) | Undocumented. |
| `coworkEDRIntegrationVendor` | EDR Integration vendor ID | string | Undocumented. |
| `coworkEgressAllowedHosts` | Allowed egress hosts | array/object |  |
| `coworkFileReadCheck` | File read check | string | Undocumented. |
| `coworkTabEnabled` | Allow Cowork | boolean |  |
| `desktopHome` | Desktop home | string |  |
| `developerModeEnabled` | Allow Developer Mode | boolean |  |
| `disableBundledSkills` | Disable bundled skills and workflows | boolean |  |
| `disableBypassPermissionsMode` | Disable bypass permissions mode | boolean |  |
| `disabledBuiltinTools` | Disabled built-in tools | array/object |  |
| `disableDeepLinkRegistration` | Disable claude:// deep-link handling | boolean |  |
| `disableDeploymentModeChooser` | Disable Claude.ai sign-in | boolean |  |
| `disableDesktopLocalSessions` | Disable local Code sessions | boolean | Undocumented. |
| `disableWslSessions` | Block WSL sessions | boolean | Undocumented. |
| `isClaudeCodeForDesktopEnabled` | Allow Code | boolean |  |
| `keepAwakeEnabled` | Allow keep awake | boolean |  |
| `officeAccessPolicies` | Document access policies | object |  |
| `officeDisabledFeatures` | Disabled add-in features | string |  |
| `officeDisabledHosts` | Disabled Office applications | array/object |  |
| `organizationInstructions` | Organization instructions | string |  |
| `quickWriteEnabled` | Allow Quick Write | boolean | Undocumented. |
| `relocateUncUserData` | Move app data off network shares | boolean |  |
| `requireCoworkFullVmSandbox` | Require full VM sandbox | boolean |  |
| `scheduledTasksEnabled` | Allow scheduled tasks | boolean |  |
| `secureVmFeaturesEnabled` | Secure VM features | boolean |  |
| `skillCreationEnabled` | Allow user-created skills | boolean |  |
| `skipWebFetchPreflight` | Skip WebFetch domain check | boolean |  |
| `sshClientPath` | SSH client program | string |  |
| `sshConfigs` | SSH connections | array/object | Undocumented. |
| `sshHostAllowlist` | SSH host allowlist | string |  |
| `sshTransport` | SSH connection engine | enum/string |  |
| `toolPolicyEndpoint` | Tool policy endpoint | URL string | Undocumented. |
| `toolSearchEnabled` | Enable tool search | array/object |  |
| `userPluginMarketplacesEnabled` | Allow user-added plugin marketplaces | array/object |  |
| `userPluginUploadsEnabled` | Allow user-added plugins | boolean |  |

### 2.3 Connectors (17 keys)

| Key | UI label / meaning | Value class | Notes |
|---|---|---|---|
| `allowedPluginMcpServers` | Allowed plugin MCP servers | array/object |  |
| `claudeAiImport` | Claude.ai data import | object |  |
| `claudeInChromeEnabled` | Enable Claude in Chrome | boolean | Undocumented. |
| `claudeInChromePasswordManagersEnabled` | Enable password managers in Claude in Chrome | secret string | Undocumented. |
| `deniedPluginMcpServers` | Blocked plugin MCP servers | array/object |  |
| `disableConnectorSignInInApp` | Keep connector sign-in in the browser | boolean | Undocumented. |
| `hardwareBuddyEnabled` | Allow Hardware Buddy devices | boolean |  |
| `isDesktopExtensionDirectoryEnabled` | Show extension directory | boolean |  |
| `isDesktopExtensionEnabled` | Allow desktop extensions | boolean |  |
| `isDesktopExtensionSignatureRequired` | Require signed extensions | boolean |  |
| `isLocalDevMcpEnabled` | Allow user-added MCP servers | boolean |  |
| `managedMcpServers` | Managed MCP servers | array/object |  |
| `mcpPersistentAlwaysAllowEnabled` | Allow persistent tool approvals | boolean |  |
| `mcpScheduledTaskApprovalLifetimeDays` | Scheduled-task tool approval lifetime | number |  |
| `mcpToolTimeoutSec` | MCP tool call timeout | number |  |
| `microsoftAuthBroker` | Microsoft 365 native sign-in broker | enum/string |  |
| `microsoftAuthDefaultAccount` | Microsoft 365 sign-in with the Windows account | string |  |

### 2.4 Telemetry and updates (23 keys)

| Key | UI label / meaning | Value class | Notes |
|---|---|---|---|
| `autoUpdaterEnforcementHours` | Auto-update enforcement window | number |  |
| `configRecheckIntervalMinutes` | Configuration re-check interval | number |  |
| `dangerousMaxVersion` | Maximum version | boolean | Undocumented. |
| `deploymentOrganizationUuid` | Organization UUID | string (UUID) |  |
| `disableAutoUpdates` | Block auto-updates | boolean |  |
| `disableEssentialTelemetry` | Block essential telemetry | boolean |  |
| `disableLocalConfigCache` | Keep only your organization ID and restrictions on disk | boolean | Undocumented. |
| `disableNonessentialServices` | Block nonessential services | array/object |  |
| `disableNonessentialTelemetry` | Block nonessential telemetry | boolean |  |
| `otlpAttrMaxChars` | Telemetry attribute length limit | number |  |
| `otlpAuthMode` | Collector authentication | enum/string |  |
| `otlpContentCapture` | Content capture categories | string |  |
| `otlpDesktopLogLevel` | Desktop telemetry export level | enum/string |  |
| `otlpEndpoint` | OpenTelemetry collector endpoint | URL string |  |
| `otlpHeaders` | OpenTelemetry exporter headers | object |  |
| `otlpHeadersHelper` | OpenTelemetry headers helper script | array/object |  |
| `otlpProtocol` | OpenTelemetry exporter protocol | enum/string |  |
| `otlpResourceAttributes` | OpenTelemetry resource attributes | object |  |
| `otlpTracesEnabled` | Export traces | boolean |  |
| `relaunchEnforcementHours` | Configuration relaunch window | number |  |
| `releaseChannel` | Release channel | enum/string |  |
| `updateViaUpdatesHost` | Check for updates on releases.claude.com | string |  |
| `usageMetricsEnabled` | Report usage metrics to your Claude.ai organization | boolean | Undocumented. |

### 2.5 Limits (6 keys)

| Key | UI label / meaning | Value class | Notes |
|---|---|---|---|
| `chatSessionRetentionDays` | Chat retention period | number |  |
| `codeSessionRetentionDays` | Code retention period | number |  |
| `coworkSessionRetentionDays` | Cowork retention period | number |  |
| `inferenceMaxTokensPerWindow` | Max tokens per window | secret string |  |
| `inferenceTokenWindowHours` | Token cap window | secret string |  |
| `sessionRetentionHold` | Suspend session deletion | boolean |  |

### 2.6 Appearance (6 keys)

| Key | UI label / meaning | Value class | Notes |
|---|---|---|---|
| `banner` | Organization banner | object |  |
| `deploymentDisplayName` | Deployment display name | string |  |
| `deploymentDisplaySubtitle` | Deployment display subtitle | string |  |
| `disableConfigDeprecationWarnings` | Hide configuration deprecation warnings | boolean | Undocumented. |
| `disableFeatureDiscovery` | Hide feature announcements | boolean |  |
| `endUserAttribution` | End User Attribution | enum/string |  |

### 2.7 Plugins (5 keys)

| Key | UI label / meaning | Value class | Notes |
|---|---|---|---|
| `allowedPluginMarketplaces` | Plugin marketplaces | array/object |  |
| `organizationPluginsUrl` | Organization plugins endpoint | array/object | Undocumented. |
| `orgPluginSettings` | Organization plugin settings | object |  |
| `sharingEndpoint` | Organization sharing endpoint | URL string | Undocumented. |
| `skillBundles` | Skill bundles | array/object |  |

### 2.8 Source (6 keys)

| Key | UI label / meaning | Value class | Notes |
|---|---|---|---|
| `bootstrapEnabled` | Use bootstrap config | boolean |  |
| `bootstrapHeaders` | Bootstrap request headers | object |  |
| `bootstrapHeadersHelper` | Bootstrap headers helper script | array/object |  |
| `bootstrapOidc` | Bootstrap OIDC parameters | object |  |
| `bootstrapUrl` | Bootstrap config URL | URL string |  |
| `trustBootstrapDelivery` | Trust bootstrap-delivered settings | boolean |  |

## 3. Key-group notes

### 3.1 Connection

- Provider selectors: `inferenceProvider`, `inferenceCredentialKind`.
- Gateway: base URL, auth scheme, static key, OIDC/IdP, managed Claude Code, custom headers, stream
  idle timeout, session lifetime.
- Bedrock: profile, AWS dir/CLI, region, base URL, bearer token, SSO fields, service tier, web-identity
  role ARN.
- Vertex: project, region, base URL, OAuth client/secret/scopes/login hint, Workforce Identity fields,
  credentials file.
- Foundry: resource, base URL, API key, tenant/client, auth flow.
- Models: discovery, static list, 1M preference, default effort, pricing, catalog metadata/URL.
- Self-hosted execution: URL/token (deployment-level; not a generic provider).
- Proxy/PAC and VM IPv6 also live here.

`inferenceBedrockProfile`, `inferenceBedrockAwsDir`, `inferenceBedrockAwsCliPath`, and
`inferenceBedrockRegion` appear once per provider schema at extraction time (four duplicate occurrences,
186 unique keys).

### 3.2 Capabilities and sandbox

This group mixes surface gates with enforcement policy. Important separations:

- **Availability**: `coworkTabEnabled`, `isClaudeCodeForDesktopEnabled`, `chatTabEnabled`.
- **Local/remote placement**: `disableDesktopLocalSessions`, `ssh*`, `disableWslSessions`.
- **Tools**: `disabledBuiltinTools`, `builtinToolPolicy`, `autoModeEnabled`,
  `disableBypassPermissionsMode`, `toolSearchEnabled`, `skipWebFetchPreflight`.
- **Files/egress**: `allowedWorkspaceFolders`, `blockReadsOutsideWorkingDirectories`,
  `coworkEgressAllowedHosts`, `coworkFileReadCheck`.
- **Sandbox**: `requireCoworkFullVmSandbox`, `secureVmFeaturesEnabled`, `coworkVmIpv6Enabled`,
  `coworkEDRIntegration*`.
- **Browser**: `builtinBrowserEnabled` and allow/block/default-domain policy.
- **Retention-adjacent**: `claudeCodeSettings`, `office*`, `quickWriteEnabled`,
  `hardwareBuddyEnabled`.

A flag's `enabled` status in this group is not proof the underlying platform/service exists. The
runtime gate in [16](16-feature-flag-matrix.md) remains authoritative.

### 3.3 Connectors

- Managed and local MCP definitions (`managedMcpServers`, `isLocalDevMcpEnabled`).
- Approval policy (`mcpPersistentAlwaysAllowEnabled`, lifetime days, tool timeout).
- Claude in Chrome and password-manager controls.
- Plugin MCP allow/deny lists.
- Desktop extensions and signature/directory policy.
- Browser sign-in/Entra broker and Claude.ai import.

MCP URLs and tokens are sensitive. `managedMcpServers` can carry OAuth configuration and must be
redacted by presence, not printed.

### 3.4 Telemetry and updates

- Essential/nonessential/usage toggles and nonessential services.
- OTLP endpoint/protocol/headers/auth/attributes/content capture/traces.
- Auto-update block, enforcement window, releases host, channel, maximum version.
- Relaunch enforcement, config recheck interval, local cache policy.
- Deployment org UUID.

`otlpHeaders` and `otlpHeadersHelper` can contain credentials. `disableLocalConfigCache` retains
organization identity/restrictions only; it is a privacy control, not a full offline mode.

### 3.5 Limits

- Token cap and window: `inferenceMaxTokensPerWindow`, `inferenceTokenWindowHours`.
- Retention per surface: chat/Cowork/Code days, plus `sessionRetentionHold`.
- Legal hold overrides deletion. Retention is not a backup guarantee.

### 3.6 Appearance

- Organization banner, feature-announcement suppression, deployment display name/subtitle.
- Artifact preview iframe origin (`userContentRendererUrl`).
- Attribution (`endUserAttribution`) and deprecation-warning suppression.

`userContentRendererUrl` changes a security boundary. It must be HTTPS/loopback per policy and
isolated from the main origin.

### 3.7 Plugins

- Skill bundles, organization plugin endpoint/settings.
- Allowed marketplaces, user uploads/marketplaces, sharing endpoint.

Marketplace and plugin bundles are code supply chain; verify signatures and never auto-enable
newly synced org plugins without policy.

### 3.8 Source

- Bootstrap enabled/URL/OIDC/headers/helper, trust flag.
- These are deployment controls, not a general remote-config backdoor.
- Bootstrap delivery must be authenticated; `trustBootstrapDelivery` is a security-sensitive opt-in.

## 4. End-user application preferences

These are not `flatKey` profile fields; they live in app config/local storage. Names are verified
where quoted, otherwise descriptive.

| Area | Keys / state | Notes |
|---|---|---|
| Theme | `theme` (`system`/`light`/`dark`) | Applies live |
| Chat font | `chatFont` (`anthropic-serif`/etc.) | Select in General |
| Motion | `reducedMotion`/system | Respects OS |
| Language | locale / `language` | 11 offered locales |
| Notifications | response-completion notification flag | OS permission separate |
| Sidebar | width/collapsed, task sections | Renderer state |
| Composer | draft, model, effort, folder, attachments | Session-scoped; draft local |
| Code | font, transcript width/view, worktree location/branch, retention/archive, sandbox/browser, switch-model behavior, output style | [09](09-settings-and-configuration.md) |
| Cowork | working folder, trusted folders, browser/sites, instructions, memory toggle | Memory contents sensitive |
| System | startup on login, menu bar/tray preference, quick-entry shortcut, keep awake | Platform-dependent |
| Developer | MCP server config/logs, extension settings | Command lines/tokens sensitive |
| Usage | range, chart preferences | Local ledger only |

## 5. Storage key families

| Store | Prefix / file | Authority |
|---|---|---|
| localStorage | 267 keys → 104 normalized patterns | Renderer UI state; not security boundary |
| IndexedDB | `keyval-store` database, `keyval` store | Renderer cache |
| App config | `config.json` | App preferences |
| Profile library | `configLibrary/*.json`, `_meta.json` | Active third-party profile |
| Session metadata | `local-agent-mode-sessions/**` | Authoritative session index |
| Audit | per-session `.audit-key` + HMAC log | Tamper evidence |
| Usage | `usage-ledger/*.ndjson` | Local accounting |
| Scheduled | `scheduled-tasks.json` | Jobs/skips/migrations |
| Catalog | `model-catalog/` | Signed metadata |
| Logs | `logs/` | Lifecycle/diagnostics |

Exact schemas and retention are in [11](11-data-and-storage.md). Never publish raw values.

## 6. Change and validation rules

| Rule | Requirement |
|---|---|
| Unknown key | Reject on apply with a field path; do not silently retain |
| Deprecated key | Warn unless `disableConfigDeprecationWarnings`; migrate if safe |
| Secret value | Validate presence/length only; never echo/log |
| URL | Enforce scheme/host policy; reject embedded credentials |
| Model list | Validate provider-specific IDs; max 200 static entries |
| Egress | Recompute from active config; block unapproved at request time |
| Retention/hold | Enforce at delete/GC; legal hold is not bypassable by UI |
| Apply | Atomic; health check; rollback on failure |
| Concurrency | One draft per profile; conflict detection on apply |
| Read-only | Managed fields cannot be overridden by local draft |

## 7. Rebuild guidance

- Keep a typed schema per provider and generate forms from descriptors; do not copy minified Zod code.
- Store secrets in the OS keychain, not JSON, even if the original allows profile fields.
- Version the profile schema and migrate forward; keep a redacted backup.
- Expose every security-sensitive key with a warning and a policy owner.
- Do not expose undocumented keys in the normal UI unless needed for support.
- Log key names and validation outcomes, never values.
