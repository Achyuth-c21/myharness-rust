# Screen — Task Detail / Transcript

**Screen:** `UI-SCREEN-03`
**Route:** `/cowork/<session-id>` (Cowork); Code sessions use `/epitaxy/<session-id>`
**Features:** `FEAT-CHAT-10`, `FEAT-CHAT-11`, `FEAT-COWORK-03`

## Purpose

The active transcript for a Cowork or Code task. It shows user/assistant turns, reasoning, tool
calls, files/artifacts, streaming state, and an optional context/progress pane. The composer stays
available at the bottom for follow-up turns.

## Layout

Observed at 1920×1080:

```text
┌──────────── 321 px ────────────┬──────────────── transcript ─────────────┬── 340 px ──┐
│ sidebar                        │ title  ⌄                 [globe] [pane]  │ Progress ⌄  │
│                                │                                          │ Working dir │
│                                │            user message (right bubble)   │ Context ⌄   │
│                                │                                          │             │
│                                │ assistant text / Thought process         │  illustration│
│                                │ copy icon                                │             │
│                                │ starburst  Working on it…                │             │
│                                │                                          │             │
│                                │ ┌──── composer 835 px ────────────────┐  │             │
│                                │ │ Write a message…                    │  │             │
│                                │ │ +  folder      model         send   │  │             │
│                                │ └─────────────────────────────────────┘  │             │
│                                │ disclaimer                               │             │
└────────────────────────────────┴──────────────────────────────────────────┴─────────────┘
```

- The transcript column is centered and readable-width, not full-bleed.
- The right pane can be toggled; when collapsed, the transcript uses the available width.
- The header title is a dropdown trigger for task actions.
- The globe icon opens web/browser-related context; the pane icon toggles the progress panel.
- The disclaimer (“Claude is AI and can make mistakes…”) is pinned below the composer.

## Transcript anatomy

| Element | Rendering |
|---|---|
| User turn | Right-aligned rounded bubble, neutral fill, plain text |
| Assistant turn | Left-aligned, no bubble, serif/body text with copy affordance |
| Reasoning | “Thought process” label plus body, shown in Thinking/Verbose view |
| Tool call | Tool-specific block/card; expandable args/result; status |
| File/artifact | Link/chip/preview; opens isolated viewer |
| Error | Inline error block; retry/continue action where safe |
| Working | Starburst + “Working on it…” / status text |
| Stopped | Partial output retained; explicit stopped status |
| Permission request | Inline/modal approval; exact placement unobserved |
| Divider | Between turns/context sections where needed |

The observed screenshot contains personal prompts, tool names, and task titles. Do not publish it.

## Header actions

- Title dropdown: rename and session actions (pin/unread/archive/delete are also in the sidebar
  overflow).
- Back/forward navigation.
- Right-pane toggle.
- Possibly copy/open/share actions; not all verified.

## Composer

Same composer as `/new`, but narrower and pinned. It supports:

- `+` add files/photos;
- folder context icon;
- model label / picker;
- send or stop;
- streaming status.

The model label must remain visible so a follow-up cannot silently use a different model.

## Right pane

Observed content:

- **Progress** section.
- **Working folder** section.
- **Context** section with an illustration and “Track tools and referenced files used in this\n  task.”
- Linked/expandable rows where data exists.

The pane is optional and should be keyboard-toggleable. It must not expose absolute paths in shared
screenshots/exports.

## States

| State | Behavior |
|---|---|
| Loading | Skeleton transcript; no empty-state flash |
| Empty | Should not occur for a real session; show a recoverable error |
| Streaming | Incremental text/tool events; stop control |
| Waiting on approval | Explicit prompt; no hidden execution |
| Completed | Final content, files/artifacts, copy/retry |
| Stopped | Partial content + stopped label |
| Error | Inline error with provider/model context; retry |
| Archived | Read-only? (unknown); history retained |
| Deleted | Route closes/returns to home; data removed per retention |
| Session missing | Recoverable not-found state; no fabricated transcript |

## Interactions

| Action | Result |
|---|---|
| Enter | Send follow-up |
| Shift+Enter | Newline |
| Stop | Cancel current turn |
| Copy | Copy message/code block |
| Title dropdown | Session actions |
| Pane toggle | Show/hide right panel |
| Scroll up | Load/virtualize older turns; do not jump |
| Link/artifact | Open isolated viewer |

## Data and APIs

- Session metadata (`local_<uuid>.json`) plus agent transcript/audit.
- Streaming events come through the agent/session bridge; the renderer never calls a provider
  directly.
- Tool blocks reflect audit/tool-call records, not reconstructed guesses.
- Artifacts/files are references; content lives in the session/artifact store.

## Evidence

- `65-task.png` (personal; private archive only).
- `70-new-artifact.png` (live artifact task; personal prompt; private archive only).
- Session schema and audit keys in [11](../11-data-and-storage.md).

## Test mapping

`TEST-SESSION-001..004`, `TEST-UX-012`, `TEST-AI-015`, `TEST-MCP-003/008`.

## Acceptance checks

1. Reopen restores the transcript and does not create a duplicate process.
2. Stop cancels and marks partial output; follow-up can continue.
3. Reasoning/tool/file blocks remain in the correct order.
4. Error and permission states are actionable.
5. Right pane toggles without losing scroll position.
6. No provider/model silently changes between turns.
7. Personal content is not used in visual regression fixtures.
