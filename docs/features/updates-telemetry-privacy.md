# Updates, Telemetry, Privacy, and Import/Export

**Features:** `FEAT-PRIV-*`, `FEAT-SHELL-10`, `FEAT-SET-03/08`
**Primary docs:** [09](../09-settings-and-configuration.md), [11](../11-data-and-storage.md)

## 1. Third-party privacy disclosure

In 3P mode the Privacy page states:

- Prompts, completions, and user data are **not sent to Anthropic** for inference.
- Anthropic may still receive crash/performance reports, product-usage analytics without message
  content, update checks, and diagnostic bundles explicitly uploaded by the user.
- Connector favicons, artifact/MCP-App iframes, and the Linux spellcheck dictionary are
  “nonessential services” and may be blocked by policy.
- Provider billing/usage is handled by the configured provider, not Anthropic.

This copy is a critical trust boundary. Do not weaken it, and do not add hidden telemetry.

## 2. Telemetry categories

| Category | Examples | Content |
|---|---|---|
| Essential | Crash/performance reports | Error stack, app version, platform; may include paths |
| Nonessential | Product-usage analytics | Feature events, counts; no message content |
| Nonessential services | favicons, preview iframes, spellcheck | Network fetches; can be blocked |
| OpenTelemetry | logs/metrics to org collector | Configurable; admin-controlled |
| Usage metrics | token counts/surfaces | No prompt/completion |
| Support bundle | diagnostics | Explicit user action; must enumerate contents |

Observed hosts include Sentry/Datadog intake domains and optional OTel endpoint. Do not publish
private collector URLs.

## 3. Redaction

- Never log prompts/completions, attachment contents, tool args/results, API keys, OAuth tokens,
  cookies, authorization headers, or query-string secrets.
- Session IDs may be logged only when needed; account/org IDs should be hashed/redacted.
- Tool/server stderr may contain secrets; filter before writing/uploading.
- Diagnostic bundles must redact or exclude message content by default.
- Screenshots/docs must be redacted separately; telemetry settings do not make them safe.

## 4. Updates

Observed:

- Update banner offered `2.42304.2`; not applied.
- Update feed can be routed via `releases.claude.com` so `api.anthropic.com` remains blocked.
- Auto-update can be disabled; enforcement window defaults to 72 hours.
- Configuration relaunch window defaults to 24 hours; re-check interval defaults to 10 minutes.
- 9-method AutoUpdater bridge; version/config keys; pending update UI.

Lifecycle requirements:

```text
check → verify metadata/signature → download → verify → stage
  → prompt/enforce → restart → health check → rollback on failure
```

- Never install without signature verification.
- Preserve a runnable previous version for rollback.
- Do not force-install while a task is streaming; use the configured window/session guard.
- Update failure must leave the app usable and explain the error.
- On Linux packaging may delegate updates to the package manager; detect and label this.

## 5. Retention and limits

- Chat/Cowork/Code retention days are separate.
- Legal hold suspends automatic deletion.
- Deletion of chats does not delete projects/memory; Cowork does not delete spaces/memory; Code does
  not delete uncommitted work.
- Max tokens per window is per device and excludes cached tokens; it does not cap provider spend.
- Usage page is device-local; provider bill includes the whole org and cache charges.

## 6. Import and export

This deployment has `claudeAiImport` disabled:

```json
{"enabled": false, "automatic3pImport": false, "exportEnabled": false}
```

The Import & export settings section explains that the administrator must enable it. This is a policy
state, not a bug.

If enabled (not observed):

- Import must validate source/schema and avoid overwriting live sessions.
- Export must include a manifest, redact secrets by default, and warn about personal data.
- Export/download actions must require explicit user action.
- Artifacts/files use dedicated schemes/isolated viewers.

## 7. Support bundle

The app has `SupportBundle` IPC and an explicit “send to Anthropic” action. Rebuild requirements:

- Enumerate exactly what will be included.
- Redact secrets/content by default.
- Let the user inspect the archive before upload.
- Upload only with explicit consent and a visible destination.
- Log the action in audit/telemetry without including contents.

## 8. States

| State | Behavior |
|---|---|
| Update available | Banner with version/action; dismissible |
| Downloading | Progress; cancel where possible |
| Ready to install | Restart prompt; session guard |
| Enforced | Countdown/window; explicit copy |
| Failed | Error + retry; old version remains |
| Telemetry blocked | Controls disabled/explained |
| Import/export disabled | Explanation, not a dead button |
| Support bundle | Preview/consent before upload |

## 9. Requirements

| ID | Requirement |
|---|---|
| `REQ-PRIV-01` | Prompts/completions are never included in telemetry. |
| `REQ-PRIV-02` | Essential/nonessential/OTel controls are independent and enforced. |
| `REQ-PRIV-03` | Diagnostics require explicit consent and redaction. |
| `REQ-PRIV-04` | Retention/legal hold semantics are visible and enforced. |
| `REQ-UPDATE-01` | Updates are signed, staged, rollback-capable, and session-aware. |
| `REQ-UPDATE-02` | Update/config relaunch windows are honored. |
| `REQ-IMPORT-01` | Import/export is policy-gated and never silently overwrites data. |

**Tests:** `TEST-PRIV-001..004`, `TEST-UPDATE-001/002`, `TEST-CFG-007/010/012`.
