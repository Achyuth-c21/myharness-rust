# Screen — Scheduled Tasks

**Screen:** `UI-SCREEN-13`
**Route:** `/scheduled-task`
**Features:** `FEAT-SCHED-01`–`FEAT-SCHED-04`

## Layout

```text
Scheduled tasks                                  [search] [Sort by Next run ⌄] [New task ⌄]
Run tasks on a schedule or whenever you need them. Type /schedule in any existing task to set one up.
──────────────────────────────────────────────────────────────────────────────────────────────
(i) Scheduled tasks only run while your computer is awake and online.     [☀ Keep awake  ○]
──────────────────────────────────────────────────────────────────────────────────────────────
                                  ┌──── clock illustration ────┐
                                       No scheduled tasks yet.
──────────────────────────────────────────────────────────────────────────────────────────────
  Daily briefing                          Inbox triage
  What needs your attention today …       Categorize your inbox and draft replies …
  Weekdays at 8:00 AM                     Weekdays at 8:00 AM

  Meeting prep                            Weekly review
  A short brief before each meeting …     A Friday summary of what happened this week.
  Weekdays at 8:00 AM                     Every Friday at 4:00 PM

  Content ideas                           Monitor a topic
  Draft a few post ideas each week …      Watch for news or mentions of a topic …
  Every Monday at 9:00 AM                 Daily at 9:00 AM
```

Observed at 1920×1080:

- Page title **Scheduled tasks** (serif display).
- Subtitle includes the slash-command hint: **Type `/schedule` in any existing task to set one up.**
- Top-right: search icon, **Sort by Next run** select, black **New task** split button with a chevron.
- Informational banner with clock icon and keep-awake switch at the right.
- Centered empty state with clock illustration and **No scheduled tasks yet.**
- Dotted divider above six template rows, arranged 2 columns × 3 rows. Each has an icon tile, title,
  description, and clock + schedule.

The populated task list, row actions, editor, and run history were not observed.

## Templates

| Template | Description | Default schedule |
|---|---|---|
| Daily briefing | Attention items across calendar, email, messages | Weekdays at 8:00 AM |
| Inbox triage | Categorize inbox; draft replies to urgent items | Weekdays at 8:00 AM |
| Meeting prep | Brief before meetings with attendees/context/agenda | Weekdays at 8:00 AM |
| Weekly review | Friday summary of the week | Every Friday at 4:00 PM |
| Content ideas | Draft post ideas from recent news | Every Monday at 9:00 AM |
| Monitor a topic | Watch news/mentions of a topic, competitor, keyword | Daily at 9:00 AM |

Clicking a template should prefill the editor; it must not silently create/enable a job. The editor
was not reached, so the exact behavior is **unknown**.

## New task

The split button likely offers blank task and template/import options. The main action opens an
editor (unobserved). Required fields: prompt, schedule, context, model, tools/permissions.

## Keep awake

- The switch reflects `keepAwakeEnabled`.
- It prevents idle sleep while kept awake; it cannot override lid close, power-off, or being offline.
- Enabling it must be explicit and locally persisted.
- The banner copy must remain truthful; do not claim cloud scheduling.

## States

| State | Behavior |
|---|---|
| Loading | Skeleton; no empty flash |
| Empty | Clock + templates |
| Search | Filters tasks/templates; no-results state |
| Sort | Next run / other orders; selection persists |
| Creating | Editor/dialog |
| Scheduled | Row with next run |
| Running | Status indicator |
| Skipped | Reason recorded (asleep/offline) |
| Failed | Error + retry |
| Disabled | No future runs |
| Keep awake unavailable | Disabled/explained |

## Interactions

| Action | Result |
|---|---|
| New task | Open editor |
| Template | Prefill editor |
| Search | Filter list |
| Sort | Reorder |
| Keep awake | Toggle local wake behavior |
| Task row | Open detail/editor (unobserved) |

## Data

`scheduled-tasks.json` stores tasks, recorded skips, and cron/day-field migration stamps. A run
should create a normal session/usage record so it is auditable. Approvals follow the configured
lifetime. See [11](../11-data-and-storage.md).

## Accessibility

- The banner is a status/notice, not a modal.
- Template cards are buttons with title + schedule in the accessible name.
- The switch has a visible label and state.
- Search/sort are labelled controls.
- Time schedules are readable text, not icon-only.

## Evidence

- `21-scheduled.png`.
- Text/feature docs for limits/approvals.
- Task schema in [11](../11-data-and-storage.md).

## Test mapping

`TEST-UX-006`, `TEST-MCP-003`, `TEST-SESSION-005`, `TEST-CFG-009`.

## Acceptance checks

1. Empty state and templates match the observed layout.
2. Keep-awake does not imply cloud execution.
3. Skip reasons are recorded and visible.
4. Templates prefill rather than auto-create.
5. Disabling a task stops future runs but retains history.
6. Missing project/folder blocks a run clearly.
7. No personal schedules/prompts in fixtures.
