# Screen — New Live Artifact Task

**Screen:** `UI-SCREEN-11`
**Trigger:** **New artifact** or the empty-library suggestion
**Features:** `FEAT-ART-02`, `FEAT-COWORK-07`

## Purpose

The immediate task view created by the Artifacts library. It is not an artifact editor. The app
creates a Cowork/live-artifact session, inserts a generated prompt, and starts model generation.

## Observed sequence

```text
/cowork-artifact
  → click New artifact
  → new session titled “New live artifact”
  → navigate to /cowork/<session>
  → user-style prompt appears
  → assistant status “Working on it…” with starburst
  → generation continues until complete/stopped
```

The prompt asks the model to explain live artifacts, inspect connectors/MCP servers, and ask what
kind of artifact would be useful. It is a product-generated prompt, not a user message typed in the
composer. The exact text may change; do not make it a stable API.

The task appears in the sidebar immediately. The screenshot used for evidence contains a personal
connector error and task history; publish only a redacted crop if needed.

## Layout

- Standard Cowork task detail layout.
- Header: session title **New live artifact** with a dropdown chevron.
- Right pane: Progress / Working folder / Context.
- Transcript: generated user prompt bubble, assistant working indicator.
- Composer at bottom: empty placeholder, `+`, folder icon, model label, send/stop.
- MCP startup error toast at bottom-right when a configured server fails:
  “MCP `<server>`: Couldn’t start for Cowork and Code sessions…” with **Open developer settings**.
- The toast overlaps the lower-right area but not the composer.

## Behavior and safety

- **No creation modal.** Clicking New artifact is the confirmation.
- A session and model request start immediately.
- Stop must be visible and cancel the stream.
- If the app crashes or the user leaves, the task remains; it is not an artifact until content is
  captured.
- The library must not show a phantom artifact for a stopped/failed task.
- If no provider/model is available, do not start generation or fabricate a prompt; show the
  configuration error.
- MCP inspection is optional; a failed connector must not block the artifact task.
- Do not auto-share or auto-publish the result (`artifactAutoPublish` is explicit).

## States

| State | Behavior |
|---|---|
| Starting | Session created; navigating |
| Working | Starburst + status; stop available |
| Streaming content | Assistant text/tool calls; transcript updates |
| Stopped | Partial content; task retained |
| Failed | Error block; no artifact record |
| Completed | Artifact captured; library updates |
| MCP error | Toast; task continues |
| No provider | Configuration error before generation |

## Data

- Session metadata stores the generated initial message, model, cwd, capability switches, and MCP
  tool snapshot.
- Artifact capture (`parkAndCaptureArtifact`) links content to the session.
- `lastModifiedSession` allows the artifact to be updated by later turns.

These fields are evidenced by API/schema names; the exact capture timing is **unknown**.

## Accessibility

- The generated prompt is a normal message with a readable role.
- Working status is announced as a live status, not repeated every token.
- Stop is reachable by keyboard.
- The MCP toast is announced once and has an accessible action link.

## Requirements and tests

`FEAT-ART-02`, `FEAT-COWORK-07`; `TEST-UX-005`, `TEST-AI-015`, `TEST-MCP-009`, `TEST-SESSION-004`.

## Acceptance checks

1. One click creates one session; double-click does not duplicate.
2. Generation is stoppable and cancellation is idempotent.
3. A stopped task does not appear as a completed artifact.
4. Missing provider blocks with a configuration action.
5. MCP failure is non-blocking and redacted.
6. The generated prompt is not treated as trusted user content.
7. Publishing evidence masks all personal history/connectors.
