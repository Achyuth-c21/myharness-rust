# Chat and Composer

**Features:** `FEAT-CHAT-*`, `FEAT-SHELL-04`
**Primary docs:** [05](../05-screen-inventory.md), [06](../06-navigation-map.md), [08](../08-interaction-catalog.md)

## Purpose

The composer is the entry point for every task: a chat question, a Cowork job, or a Code session.
It is a multiline prompt with attachments and context chips, not a single-line search box.

The observed build presents a unified Cowork home. A dedicated Chat tab is unavailable
(`chatTab` gate); the same composer still accepts quick questions.

## Entry and exit

```text
/new (or Ctrl+N) → type/attach/select context → Enter
  → /cowork/<session-id> → streamed transcript
  → stop, complete, error, or permission prompt
```

- Sending creates a session immediately.
- The task appears in the sidebar and in the home “Active” list.
- The composer remains at the bottom of the task view.
- “New” clears the current context and returns to `/new`.
- Closing the window preserves drafts where local storage supports them; exact scope is **unknown**.

## Layout

The composer is a rounded card, ~720 px wide on the new-task screen. It has two regions:

1. **Input** — auto-growing multiline text with a placeholder (“How can I help you today?”).
2. **Toolbar** — Add (`+`), Skills/Connectors via the Add menu, project/folder chip, permission
   selector when applicable, model label, and send/stop.

The card uses a white surface, hairline border, large radius, and a transparent textarea. The send
button is circular and low-contrast when empty.

## Controls

| Control | Behavior |
|---|---|
| `+` | Add files/photos, Skills, Connectors, plugins |
| Folder chip | Choose a project or working folder |
| Permission selector | Default/Accept edits/Plan/Bypass, subject to policy |
| Model label | Opens the model picker; displays provider/model |
| Send | Enabled when there is text or a valid attachment; `Enter` sends |
| Stop | Replaces Send while streaming; cancels the turn |
| Slash | Typing `/` opens commands; arrows + Enter select |
| Remove attachment | Chip/thumbnail remove control; does not send |
| Info icon | Explains where data goes / privacy |

## Keyboard

- `Enter` sends; `Shift+Enter` inserts a newline.
- `Ctrl+U` opens the attachment picker.
- `Ctrl+N` new task.
- `Arrows` navigate slash/menus; `Enter` selects; `Escape` closes.
- Standard cut/copy/paste/undo/redo shortcuts apply inside the textarea.

Observed/expected; exact behavior when an IME composition is active is **unknown** and must be
tested.

## Composer states

| State | Treatment |
|---|---|
| Empty | Placeholder, neutral send, no error |
| Typing | Send becomes primary; card grows; max height then scrolls |
| Attachment uploading | Progress/chip state; send may be disabled |
| Ready | Attachments and context chips visible |
| Streaming | Stop visible; transcript streams; composer may stay editable for steering |
| Stopped | Partial transcript retained; status says stopped |
| Error | Inline error, draft retained; retry available |
| Permission required | Approval surface; send may be blocked until resolved |
| Missing provider key | Model row says “needs API key”; send blocked or configure prompt |
| Unsupported modality | Inline validation; remove/replace the file |
| Offline | Send fails visibly; no fake success |

## Attachments and context

- Multiple files and photos are supported.
- Files are referenced by local path/content id; they are not pasted into the prompt text.
- Project/folder selection changes the working context and may affect available tools.
- Skills/Connectors insert a structured mention or otherwise restrict context; exact token syntax
  is renderer-internal and not a public API.
- Attachment limits are provider/model-specific. Do not hard-code a universal limit; show the
  effective limit at send time.

See [multimodal-attachments.md](multimodal-attachments.md).

## Slash commands and menus

The observed slash menu (typed `/`) lists **Add files**, **Usage**, and skill/command rows with names
and icons only; no descriptions, argument hints, or source grouping were visible. The Add menu contains:

- Add files or photos
- Skills (with Manage/Browse)
- Connectors (with Manage connectors)
- Plugins where installed

The observed skill list included bundled document/design/memory/scheduling skills and partner
templates. Connector names are profile-dependent and redacted; do not treat either list as fixed
product content.

## Task list interactions

- Task rows show title, relative time, status, unread dot, and pin state.
- Overflow menu: Pin/Unpin (`P`), Mark unread (`U`), Rename (`R`), Archive (`A`), Delete (`D`).
- Rename is inline/dialog; empty names are rejected.
- Delete requires confirmation and is irreversible after confirmation.
- Archive removes the task from the active list without deleting files immediately.

## Copy, retry, feedback, rewind

Transcript-level actions are only partially verified. The session API supports branching/rewind and
feedback-like operations, but the exact menu was not exercised. The rebuild should expose:

- Copy message/code.
- Retry/regenerate with the **same** model, or explicitly choose another.
- Edit/rewind a prior user turn with confirmation about downstream loss.
- Thumbs feedback only if it is actually wired to a documented endpoint; in 3P mode it must not
  silently fail.

Do not invent server behavior for feedback.

## Errors and recovery

- A provider failure preserves the draft and displays the provider/model plus a retry action.
- An unauthorized/missing model blocks send; it does not switch models.
- A failed attachment upload leaves the file removable and does not send a partial prompt.
- A crash/restart restores the session metadata; unfinished streams are marked interrupted, not
  replayed.
- MCP/tool errors appear as tool-call errors inside the transcript and, where startup-wide, as a
  persistent toast.

## Requirements

| ID | Requirement |
|---|---|
| `REQ-COMPOSER-01` | Composer supports multiline text, attachments, context chips, model, and permission controls. |
| `REQ-COMPOSER-02` | Enter sends, Shift+Enter inserts a newline, IME composition is safe. |
| `REQ-COMPOSER-03` | Send is disabled without text/valid attachment; stop is available during streaming. |
| `REQ-COMPOSER-04` | Draft survives recoverable errors/navigation; unsent text is never silently discarded. |
| `REQ-COMPOSER-05` | Provider/model is always visible; missing credentials block send with a clear action. |
| `REQ-COMPOSER-06` | Slash/Add menus are keyboard-navigable and dismissible. |
| `REQ-COMPOSER-07` | A failed send never fabricates a response or substitutes a model. |
| `REQ-COMPOSER-08` | Attachments are validated against the selected model’s capabilities. |
| `REQ-COMPOSER-09` | Task actions (pin/unread/rename/archive/delete) are discoverable and confirmed where destructive. |

**Tests:** `TEST-UX-001`, `TEST-UX-002`, `TEST-SESSION-001`, `TEST-SESSION-009`, `TEST-AI-008`,
`TEST-AI-016`.
