# Evidence Handling

This directory is **not** a self-contained publication bundle. Raw captures have been moved to a
private archive outside the repository; only the pristine source tree, structured inventories, tools,
and the approved redacted subset remain here.

## Tiers

| Tier | Location | Status |
|---|---|---|
| Pristine shipped code | `app-asar-orig/` (repository) | Product code. No personal data, but redistribution is subject to the app’s license/notice terms. |
| Current patched code | `<private-archive>/evidence/app-asar-current/` | Diff/current-state evidence only. Not in this repository. |
| Raw runtime evidence | `<private-archive>/evidence/{screens,screenshots,logs}/` | **Private.** May contain task titles, prompts, memory, paths, hosts, usage, or account context. |
| Publish-safe derivatives | `redacted/` (repository) | Only files listed in `redacted/manifest.json`. |
| Tools | `tools/` | Inspection driver. No secrets. |

## Rules

1. Treat every raw capture as personal until reviewed.
2. Never copy a screenshot into `redacted/` unless the sidebar, composer, task title, account, file
   paths, usage figures, and overlays were checked. No screenshot is approved at this time.
3. Never add API keys, OAuth tokens, cookies, helper-script output, auth headers, private MCP hosts,
   VPC/Tailscale names, or absolute home paths.
4. Structured files may be copied only if they contain names/schema/measurements with no live values.
5. Add every approved file to `redacted/manifest.json` with its SHA-256 and redaction note.
6. Run a secret/personal-data scan before publishing.
7. The private archive must not be packaged, zipped, linked, or committed with the repository.
8. If in doubt, publish the finding in Markdown without the raw artifact.

## Approved subset

`redacted/` contains API namespaces, feature-flag names, storage key types, IndexedDB schema, UI
tokens/styles/measurements, the shipped English string list, and a sanitized live-patch summary. It
contains **no screenshots**, transcripts, prompt/memory content, usage values, account identifiers,
credentials, private hosts, or application source bodies.

`manifest.json` records the exact files, sizes, hashes, app version, and redaction statement.
