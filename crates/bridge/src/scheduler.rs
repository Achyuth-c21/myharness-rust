//! Scheduled tasks (roadmap Phase 6 slice 4; `docs/features/scheduled-tasks.md`,
//! `docs/screens/scheduled-tasks.md`; `REQ-SCHED-01..06`).
//!
//! The state machine lives here; execution stays in the chat send path so a
//! scheduled run is byte-for-byte the same path an interactive send uses. The
//! shipped editor was never exercised and the shipped task file was captured
//! only at the schema level, so the wire names, bounds, and template prompts
//! are **rebuild-defined** and recorded in `docs/14` §9 — except the template
//! names, descriptions, and schedule labels, which are the captured copy.
//!
//! Keep-awake is deliberately absent: it prevents idle sleep only, cannot
//! override lid close or power-off (`REQ-SCHED-06`), and no platform
//! implementation exists in this build — the renderer ships the switch
//! disabled with a truthful explanation (`TEST-SCHED-005`).

use myharness_scheduler::{CronError, CronSchedule};

pub use myharness_scheduler::{decide, skip_reason, Occurrence};
use myharness_store::Store;

/// One captured template card (docs/screens/scheduled-tasks.md §Templates).
/// `name` and `schedule_label` are the captured copy; `prompt` is the
/// rebuild-defined prefill (the shipped prompt text was never captured).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledTemplate {
    pub name: &'static str,
    pub prompt: &'static str,
    pub cron: &'static str,
    pub schedule_label: &'static str,
}

/// The six captured templates, in the captured order.
pub const TEMPLATES: [ScheduledTemplate; 6] = [
    ScheduledTemplate {
        name: "Daily briefing",
        prompt: "What needs my attention today across calendar, email, and messages?",
        cron: "0 8 * * 1-5",
        schedule_label: "Weekdays at 8:00 AM",
    },
    ScheduledTemplate {
        name: "Inbox triage",
        prompt: "Categorize my inbox and draft replies to urgent items.",
        cron: "0 8 * * 1-5",
        schedule_label: "Weekdays at 8:00 AM",
    },
    ScheduledTemplate {
        name: "Meeting prep",
        prompt: "Write a short brief before each meeting with attendees, context, and agenda.",
        cron: "0 8 * * 1-5",
        schedule_label: "Weekdays at 8:00 AM",
    },
    ScheduledTemplate {
        name: "Weekly review",
        prompt: "Summarize what happened this week.",
        cron: "0 16 * * 5",
        schedule_label: "Every Friday at 4:00 PM",
    },
    ScheduledTemplate {
        name: "Content ideas",
        prompt: "Draft a few post ideas from recent news.",
        cron: "0 9 * * 1",
        schedule_label: "Every Monday at 9:00 AM",
    },
    ScheduledTemplate {
        name: "Monitor a topic",
        prompt: "Watch for news or mentions of a topic and report what changed.",
        cron: "0 9 * * *",
        schedule_label: "Daily at 9:00 AM",
    },
];

/// Resource bounds this rebuild accepts (refused, never truncated — the same
/// posture as project names and instructions).
pub const MAX_TASK_NAME_CHARS: usize = 200;
pub const MAX_TASK_PROMPT_BYTES: usize = 16 * 1024;

/// The next occurrence of `cron` strictly after `after_ms`, in the device
/// zone (`chrono::Local`), as epoch millis. `None` means the schedule never
/// fires within the lookahead window.
pub fn next_occurrence_after(cron: &str, after_ms: i64) -> Result<Option<i64>, CronError> {
    let schedule = CronSchedule::parse(cron)?;
    let after = match chrono::DateTime::from_timestamp_millis(after_ms) {
        Some(instant) => instant.with_timezone(&chrono::Local),
        None => return Err(CronError::FieldCount),
    };
    Ok(schedule
        .next_after(&chrono::Local, &after)
        .map(|next| next.timestamp_millis()))
}

/// The readable summary the renderer shows next to a task.
pub fn describe(cron: &str) -> Result<String, CronError> {
    Ok(CronSchedule::parse(cron)?.describe())
}

/// Validate the editable fields; `Err` carries the wire-facing reason.
pub fn validate_fields(name: &str, prompt: &str) -> Result<(), &'static str> {
    if name.trim().is_empty() {
        return Err("the task name is required");
    }
    if name.trim().chars().count() > MAX_TASK_NAME_CHARS {
        return Err("the task name is too long");
    }
    if prompt.trim().is_empty() {
        return Err("the task prompt is required");
    }
    if prompt.trim().len() > MAX_TASK_PROMPT_BYTES {
        return Err("the task prompt is too long");
    }
    Ok(())
}

/// The tasks due right now (enabled with an armed occurrence at or before
/// `now`), oldest arm first.
pub fn due(store: &Store, now: i64) -> Result<Vec<myharness_store::ScheduledTaskRow>, myharness_store::StoreError> {
    store.due_scheduled_tasks(now)
}
