# Scheduled Tasks

**Features:** `FEAT-SCHED-*`, `FEAT-TOOL-03` (approval lifetime)
**Primary docs:** [09](../09-settings-and-configuration.md), [11](../11-data-and-storage.md)

## Purpose

Scheduled tasks run a saved prompt on a schedule while the desktop app is awake and online. They are
local desktop jobs, not a cloud scheduler.

## Scheduled page

Route: `/scheduled-task`.

Observed:

- Title + search + sort (“Next run”) + **New task**.
- Information banner: scheduled tasks run only while the computer is awake and online.
- **Keep awake** switch.
- Empty state: clock illustration, “No scheduled tasks yet.”
- Six template cards with schedules/descriptions:
  - Daily briefing
  - Inbox triage
  - Meeting prep
  - Weekly review
  - Content ideas
  - Monitor a topic

The populated task list/row layout and run history were **not observed**.

## Execution model

```text
cron/time trigger + saved prompt + project/folder/model/tools
    → app awake/online? → create run/session
    → execute with policy → record result/error/skip
```

- The app must be running (or resumed at the right time). `WakeScheduler`/keep-awake can prevent
  idle sleep but cannot override lid close or explicit power-off.
- A missed run because of sleep/offline is recorded as a skip (`recordedSkips`), not silently
  backfilled.
- Time-zone/daylight-saving behavior needs explicit tests. The task file includes
  `sundayAliasBoundaryStamped` / `dayFieldsOrBoundaryStamped` migration flags.
- A run should create a normal session so it is inspectable in history (strong inference).

## Create/edit

The editor was not exercised. Required design:

- Name and prompt.
- Schedule (time/day/cron-like UI) with a readable summary.
- Project/folder context.
- Model/effort or “use default.”
- Tool/permission policy and approval lifetime.
- Enabled toggle.
- Preview of next run times.
- Save/discard; validation for empty prompt/invalid schedule.

The exact shipped fields/order are **unknown**. Do not claim the layout.

## Templates

Templates prefill a task. They are not enabled automatically. Selecting a template should open the
editor with values populated, not create a job without review.

## Approvals

- Persistent “Always allow” options exist only if `mcpPersistentAlwaysAllowEnabled`.
- `mcpScheduledTaskApprovalLifetimeDays` controls how long a scheduled task may reuse an approval;
  `0` removes the lasting option.
- A scheduled run must never inherit an approval from an unrelated session beyond the configured
  scope/lifetime.
- Tool policy/egress still applies.

## Retention and deletion

- Scheduled task deletion removes the schedule; prior run sessions follow session retention.
- Deleting a project/folder referenced by a task must not silently retarget the task. Mark it
  unavailable and require user action.
- Legal hold overrides session deletion but does not prevent disabling a schedule.

## States

| State | Behavior |
|---|---|
| Empty | Explanation/templates |
| Creating | Editor; validation |
| Enabled/scheduled | Shows next run, last result |
| Running | Status; stop/cancel if supported |
| Skipped | Reason (asleep/offline); visible |
| Failed | Error + retry; no silent success |
| Disabled | No future runs; history retained |
| Deleted | Removed; runs per retention |
| Project missing | Explicit unavailable state |

## Requirements

| ID | Requirement |
|---|---|
| `REQ-SCHED-01` | Page has search/sort/new task and a truthful keep-awake explanation. |
| `REQ-SCHED-02` | Templates prefill an editor; they do not auto-create jobs. |
| `REQ-SCHED-03` | Scheduler records skips/misses and never silently backfills. |
| `REQ-SCHED-04` | Approval lifetime/persistent approval policy is enforced per run. |
| `REQ-SCHED-05` | Deleting a referenced project/folder does not silently retarget a task. |
| `REQ-SCHED-06` | Keep-awake does not claim to override lid close/power-off. |

**Tests:** `TEST-UX-006`, `TEST-MCP-003`, `TEST-SESSION-005`, `TEST-CFG-009`.
