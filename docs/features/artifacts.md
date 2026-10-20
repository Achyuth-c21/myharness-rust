# Artifacts

**Features:** `FEAT-ART-*`
**Primary docs:** [05](../05-screen-inventory.md), [10](../10-architecture.md), [11](../11-data-and-storage.md)

## Purpose

Artifacts are rendered documents/apps produced by tasks: HTML, Markdown, charts, diagrams, and
files. They can be previewed, versioned, starred, hidden, shared, exported, and linked to MCP tools.

## Artifacts library

- Route: `/cowork-artifact`.
- Empty state: document/folder illustration, **“Create your first artifact”**, and a single suggestion
  button **“What needs my attention”** (glasses icon). The suggestion starts a live-artifact task with
  a generated prompt; it is not a generic create button.
- Header: **Artifacts** title, search, view toggle, and black **New artifact**.
- Populated library cards/previews were not observed; do not invent card geometry.

## New artifact behavior

**Important:** “New artifact” does **not** open a creation modal. It immediately creates a
live-artifact task and starts generation.

```text
/cowork-artifact → New artifact → live-artifact session → streaming
```

This is a destructive/expensive side effect. A rebuild must preserve the behavior if it is required,
but must make the button’s consequence clear. At minimum:

- create the task only after an explicit click;
- navigate to the task immediately;
- show a stop control;
- if the user cancels before capture, leave the stopped task but do **not** show a completed/phantom
  artifact in the library.

The observed accidental test task was stopped but remained in history. Cleanup requires deleting the
task.

## Live artifacts

A live artifact is backed by a session. It can be regenerated, updated by the agent, and linked to
MCP tools. The `artifactsPane` gate is unavailable here, so the side pane/live preview was not
exercised.

## Viewer and actions

The API surface (`claude.web.CoworkArtifacts`, 25 methods) evidences:

| Group | Methods |
|---|---|
| List/metadata | `getAllArtifacts`, `getArtifactMetadata` |
| Content | `getArtifactHtml`, `getArtifactThumbnail`, `getArtifactRepublishContent` |
| Naming | `setArtifactName` |
| Versions | `restoreArtifactVersion` (version history UI unobserved) |
| Star/hide/show | `setArtifactStarred`, `hideArtifact`, `showArtifact` |
| Share | `shareArtifact`, `unshareArtifact`, `isSharingEnabled`, `isSharedContentCurrent` |
| Import/refresh | `importArtifact`, `refreshImportedArtifact` |
| Publish | `isAutoPublishEnabled`, `setArtifactAutoPublish` |
| MCP tools | `setArtifactMcpTools` |
| Session link | `setArtifactLastModifiedSession`, `parkAndCaptureArtifact` |
| Export | `printArtifactToPdf` |
| Change events | `onOnArtifactsChanged` |
| Viewer | `reloadArtifactView` |

Exact viewer layout, version selector, share dialog, and export flow are **unknown**. Do not invent
them; design them against this API and the security requirements below.

## Rendering

Bundled artifact libraries:

- Mermaid `11.17.2` — diagrams.
- Chart.js `4.5.0` — charts.
- Grid.js `5.0.2` — tables (plus Mermaid theme CSS).

The artifact HTML renderer is isolated from the main app. The config exposes
`userContentRendererUrl` / artifact-preview iframe origin. In 3P deployments this may be a dedicated
HTTPS origin; otherwise it defaults to the commercial host.

Security requirements:

- Serve artifact content from a dedicated origin/partition, never `app://localhost`.
- No preload, no `contextBridge`, no Node integration in the artifact frame.
- Strict CSP; do not allow arbitrary scripts in the parent origin.
- Treat artifact HTML/SVG/Markdown as untrusted.
- Network access follows egress policy; do not auto-load third-party assets.
- Sanitize Markdown/HTML; block `javascript:` URLs and dangerous attributes.
- Thumbnails/previews must also be isolated.
- Sharing must not leak local paths, tool output, or credentials.

## Storage model

The evidence does not expose the artifact content schema. A rebuild should use:

- artifact id, session/project id, name, kind, visibility;
- version records with content hash, created time, author/turn;
- current version pointer;
- thumbnail/preview cache;
- share state with explicit scope/expiry;
- MCP tool bindings.

Large binary content is stored as content-addressed files; metadata in SQLite. Deleting an artifact
must remove versions and previews, or retain them per retention/share policy.

## States

| State | Behavior |
|---|---|
| Empty | “Create your first artifact” + suggestion CTA |
| Creating | Immediate task/stream; stop available |
| Rendering | Dedicated preview origin; loading state |
| Render error | Show error + source/retry; never inject raw error into parent |
| Versions | Unknown UI; restore requires explicit choice |
| Shared | Show scope/expiry; stale content warning if not current |
| Hidden | Removed from library but retained per policy |
| Deleted | Confirmation; versions/previews removed or retained per policy |
| Pane unavailable | Library still works; do not fake a side pane |

## Requirements

| ID | Requirement |
|---|---|
| `REQ-ART-01` | Library empty state matches the observed copy; creation is explicit. |
| `REQ-ART-02` | New artifact starts a task only after explicit click and exposes stop. |
| `REQ-ART-03` | Artifact rendering is isolated from the app bridge and main storage. |
| `REQ-ART-04` | Versions are immutable and restorable; current pointer is explicit. |
| `REQ-ART-05` | Sharing is explicit, scoped, and does not leak local data. |
| `REQ-ART-06` | Unsupported pane/viewer is shown honestly, not emulated. |
| `REQ-ART-07` | Delete removes content/previews or honors retention/share policy. |

**Tests:** `TEST-UX-005`, `TEST-ARCH-006`, `TEST-SEC-001`, `TEST-SESSION-004`.
