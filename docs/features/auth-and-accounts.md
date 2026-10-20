# Authentication, Accounts, and Profiles

**Features:** `FEAT-ACCT-*`, `FEAT-SET-09`
**Primary docs:** [09](../09-settings-and-configuration.md), [10](../10-architecture.md)

## 1. Deployment identities

This is a third-party deployment. The sidebar account row shows a local display name and the active
provider (for example, “Display name · Gateway”). It is **not** an Anthropic account identity.

There are several identity layers:

| Layer | Purpose | Owner |
|---|---|---|
| Local profile/display name | Sidebar/account label | Local config |
| Provider credential | Authenticate inference | Provider/keychain |
| Cloud identity (AWS/Google/Azure) | Bedrock/Vertex/Foundry | Vendor SDK/keychain |
| Organization bootstrap identity | Managed policy/org | Bootstrap/OIDC |
| Optional Anthropic account | Hosted features | Disabled in this 3P deployment |

Do not conflate the local display name with authentication. The UI must show which provider is
actually serving inference.

## 2. Provider authentication methods

### Static credential

- API key stored in keychain (preferred) or restricted config.
- Gateway: bearer or `x-api-key`.
- Claude API: `x-api-key`.
- Bedrock: optional bearer token.
- Foundry: API key.
- Never display the secret after entry; offer replace/clear, not reveal.

### Interactive/OAuth

- Browser/device-code flow in a dedicated window.
- Anthropic OAuth tokens may start with `sk-ant-oat`; these use an OAuth beta header.
- Gateway OIDC/device flow where configured.
- Google interactive/Workforce for Vertex.
- Azure/Entra for Foundry.
- Token cache is main-process only; refresh failures require re-auth, not fallback to a stale token.

### Helper script

- External command prints/returns a credential.
- Fields: command, args, timeout, TTL, silent refresh, platform variants.
- Helper output is a secret: never log it; trim whitespace; reject multi-line garbage unless the
  documented contract allows it.
- If the helper fails, the request fails. It must not fall back to a static key.

### Vendor profile / workload identity

- AWS profile/SSO/web identity/instance role.
- Google Application Default Credentials/Workforce.
- Azure managed identity/service principal.
- No long-lived secret in app config where the platform can provide one.

## 3. Profile lifecycle

Profiles are provider + policy bundles, not user accounts. They can be created, duplicated, renamed,
exported, switched, and deleted in the setup window.

```text
select profile → edit draft → validate → Apply
  → active profile updated → provider clients recreated → optional relaunch
```

- Switching while a session is active may warn via a session guard.
- An invalid profile is not applied.
- A profile can be managed by a bootstrap overlay; fields become read-only.
- Deleting the active profile requires selecting another first.
- Export can contain secrets; treat it as sensitive.

## 4. Multi-account and SSO

Configuration supports:

- `disableMultiAccount`
- `loginSsoOrgDomain`
- `forceLoginOrgUUID`
- `entraClientId`, `entraScope`, `entraAzureCloud`
- `inferenceIdpOidc`, `inferenceIdpAuthFlow`
- `allowOptionalClientAuth`
- `bootstrapOidc`
- `inferenceGatewayOidc`

Multi-account is disabled in this deployment. Exact account-switching UI/behavior is **unknown**.
SSO/Entra/OIDC flows were not completed.

Rules for the rebuild:

- Organization policy wins over a user’s preferred account.
- Do not silently switch organizations/accounts.
- Show the active org/identity before a request that uses org policy.
- A forced org that is unavailable is an error, not a fallback to a personal account.

## 5. Account menu

Observed actions:

- Settings.
- Language.
- Inference configuration (opens/focuses setup window).
- Changelog.
- Learn more.
- Sign out (destructive; not exercised).

The menu header shows the current local identity/provider. Sign out must clear the relevant tokens,
not merely hide the UI. Exact scope (all profiles vs active provider) is **unknown**; define it
explicitly.

## 6. Privacy boundary

The Privacy page states that prompts/completions are never sent to Anthropic in 3P mode. Anthropic
may still receive crash/performance reports, usage analytics without message content, update checks,
and explicitly uploaded diagnostics.

Authentication must not leak:

- provider keys/tokens into renderer storage or telemetry;
- organization UUIDs into public logs/exports;
- account email/identity into screenshots/docs;
- OAuth authorization codes or refresh tokens into URLs/logs.

## 7. Failure states

| Failure | Behavior |
|---|---|
| Missing credential | “Needs API key” / configure action; send blocked |
| Invalid credential | Provider 401/403; re-auth/replace; no fallback |
| Expired token | Refresh once; on failure re-auth |
| Helper failure | Visible error; no static-key fallback |
| Wrong org/tenant | Explain policy/tenant; no silent switch |
| Provider unavailable | Retry/backoff; no model substitution |
| Sign-out failure | Keep session state; report error; do not pretend success |
| Profile switch with active work | Session guard warning; explicit choice |

## 8. Requirements

| ID | Requirement |
|---|---|
| `REQ-AUTH-01` | Display identity and inference provider are distinct. |
| `REQ-AUTH-02` | Credentials are keychain-backed and never exposed to renderer/export by default. |
| `REQ-AUTH-03` | Credential kinds are exclusive; helper/OAuth failure never falls back. |
| `REQ-AUTH-04` | Organization policy cannot be bypassed by switching accounts/profiles. |
| `REQ-AUTH-05` | Sign-out clears the documented credential scope. |
| `REQ-AUTH-06` | Auth errors are provider-specific and actionable. |
| `REQ-AUTH-07` | Account/org identifiers are redacted from telemetry, logs, and docs. |

**Tests:** `TEST-CFG-005/006`, `TEST-AI-006`, `TEST-PRIV-001/004`, `TEST-SEC-004`.
