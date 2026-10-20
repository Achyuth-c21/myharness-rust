# Search and Filter

**Features:** `FEAT-SHELL-05`
**Primary docs:** [06](../06-navigation-map.md), [08](../08-interaction-catalog.md)

## Purpose

The search palette finds local sessions/tasks and actions. The filter menu narrows the sidebar task
list. Neither surface searches Anthropic-hosted content in this deployment.

## Search palette

Open with `Ctrl+K` or the sidebar search icon. It is a centered overlay with a scrim.

Layout:

- Query field at the top, with leading search icon and clear control.
- Result list below; session rows show title/relative time/snippet and an action affordance.
- Action rows can appear for commands/create actions.
- Keyboard hints at the bottom.
- Filter control is available from the task-list context.

Observed search screenshot shows a query and a matching task; content is personal and not reproduced.

## Search behavior

- Query filters client-side over local metadata/transcript index.
- Empty query shows recent tasks/actions (strong inference; not fully captured).
- Matching is case-insensitive and token/substring based; exact ranking algorithm is **unknown**.
- Results update as you type; no submit required.
- `ArrowUp/ArrowDown` moves the highlight; `Enter` opens; `Escape` closes.
- Focus is trapped while open and restored to the trigger.
- Long titles truncate; hover/focus should expose full text.
- A transcript-search worker exists (`transcript-search-worker`), so content search is separate from
  title metadata search.

## Filter menu

Observed filter controls:

| Group | Values |
|---|---|
| Type | All types, Cowork, Code (surface-dependent) |
| Status | Active, Archived, Unread (and similar) |
| Last activity | Any time, Today, Last 7 days, Last 30 days |
| Group by | None, Date, Project, Status |
| Sort by | Last activity, Date created, Name |

Exact labels/combinations are locale-driven. The menu applies immediately; the list updates without
a separate Apply button. A reset/clear affordance is expected.

## Task-list grouping

The sidebar groups sessions by recency (Today/Yesterday/Previous 7 days/older) and supports pinning.
Unread, active, pinned, and archived states are independent. Pinned tasks should remain accessible
even when a filter would otherwise hide them — exact behavior is **unknown**.

## States

| State | Behavior |
|---|---|
| Closed | No overlay |
| Idle/empty query | Recent/actions; no blank panel |
| Typing | Loading indicator if indexing; results update |
| No results | “No results” + keep query editable |
| Error | Non-blocking error; retry; don’t close the palette |
| Many results | Virtualized/scrollable; highlight stays visible |
| Filtered empty | Explain active filters + reset |
| Keyboard focus | Visible highlight, focus trap |

## Scope and privacy

- Search is local; do not send queries to telemetry.
- Do not display matches from deleted/archived content unless the filter includes it.
- Search snippets are personal; never use them in fixtures/screenshots.
- File/workspace search belongs to the relevant project/folder surface; the command palette is
  primarily session/action search.

## Requirements

| ID | Requirement |
|---|---|
| `REQ-SEARCH-01` | Palette opens globally, traps focus, and restores focus on close. |
| `REQ-SEARCH-02` | Query/results update live with keyboard navigation and empty/error states. |
| `REQ-SEARCH-03` | Search is local and never sends query/content to telemetry. |
| `REQ-SEARCH-04` | Filtering/grouping/sorting compose and have a clear reset. |
| `REQ-SEARCH-05` | Deleted/archived content follows filter visibility rules. |
| `REQ-SEARCH-06` | Long titles and snippets are accessible, not clipped without alternative. |

**Tests:** `TEST-UX-003`, `TEST-PRIV-001`, `TEST-UX-012`.
