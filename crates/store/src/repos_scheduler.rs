//! The scheduled-task repository (roadmap Phase 6 slice 4;
//! `docs/features/scheduled-tasks.md`, `docs/screens/scheduled-tasks.md` §Data,
//! `REQ-SCHED-03/05`).
//!
//! The shipped `scheduled-tasks.json` layout was captured only at the schema
//! level; this is the rebuild's normalized model, recorded as such in
//! `docs/14` §9. The behavioral contract honored:
//!
//! - **Every due occurrence gets a run row** — completed, failed, or skipped
//!   — so a missed run is recorded with its reason and never silently
//!   backfilled (`TEST-SCHED-003`).
//! - **`armed_at` is the single pending occurrence.** The engine arms the
//!   next instant, decides run-or-skip, then rearms strictly past it. There
//!   is no queue and no backfill.
//! - **`project_id` is a soft association** (no foreign key), like
//!   `sessions.project_id`: deleting a project must never retarget a task
//!   (`REQ-SCHED-05`); the bridge resolves "project missing" at read time.
//! - **Deleting the task removes the schedule and its run rows**; the
//!   sessions a run created stay and follow session retention.
//!
//! Corrupt rows follow the store-wide quarantine-then-fail-read policy
//! (`TEST-SESSION-003`).

use rusqlite::{params, Connection, OptionalExtension};

use crate::quarantine;
use crate::{is_uuid, now_ms, Store, StoreError};

/// The run statuses this schema knows. Anything else quarantines on read.
pub const RUN_STATUSES: [&str; 4] = ["running", "completed", "failed", "skipped"];

/// One scheduled task's persisted row. `armed_at` is the pending occurrence;
/// `None` while disabled or before the first arm.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ScheduledTaskRow {
    pub id: String,
    pub name: String,
    pub prompt: String,
    pub cron: String,
    pub enabled: bool,
    #[serde(rename = "projectId")]
    pub project_id: Option<String>,
    pub cwd: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    #[serde(rename = "armedAt")]
    pub armed_at: Option<i64>,
    #[serde(rename = "lastRunAt")]
    pub last_run_at: Option<i64>,
    #[serde(rename = "extra")]
    pub extra: serde_json::Value,
}

/// The fields a caller supplies at creation. The cron string is validated by
/// the scheduler crate at the bridge; the store checks shape only.
#[derive(Debug, Clone, Default)]
pub struct NewScheduledTask<'a> {
    pub name: &'a str,
    pub prompt: &'a str,
    pub cron: &'a str,
    pub enabled: bool,
    pub project_id: Option<&'a str>,
    pub cwd: Option<&'a str>,
    pub extra: serde_json::Value,
}

/// One recorded run (or skip) of a task.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ScheduledRunRow {
    pub id: String,
    #[serde(rename = "taskId")]
    pub task_id: String,
    /// The occurrence instant this row accounts for.
    #[serde(rename = "startedAt")]
    pub started_at: i64,
    pub status: String,
    /// Skip reason or failure message; `None` for a clean run.
    pub reason: Option<String>,
    /// The session a completed/failed run created (auditable history).
    #[serde(rename = "sessionId")]
    pub session_id: Option<String>,
    #[serde(rename = "finishedAt")]
    pub finished_at: Option<i64>,
}

fn task_columns() -> &'static str {
    "id, name, prompt, cron, enabled, project_id, cwd, created_at, armed_at, last_run_at, payload"
}

fn task_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ScheduledTaskRow> {
    Ok(ScheduledTaskRow {
        id: row.get(0)?,
        name: row.get(1)?,
        prompt: row.get(2)?,
        cron: row.get(3)?,
        enabled: row.get::<_, i64>(4)? != 0,
        project_id: row.get(5)?,
        cwd: row.get(6)?,
        created_at: row.get(7)?,
        armed_at: row.get(8)?,
        last_run_at: row.get(9)?,
        extra: serde_json::from_str(&row.get::<_, String>(10)?).unwrap_or_default(),
    })
}

fn valid_task(row: &ScheduledTaskRow) -> Result<(), String> {
    if !is_uuid(&row.id) {
        return Err("id is not a uuid".into());
    }
    if row.name.trim().is_empty() {
        return Err("name is empty".into());
    }
    if row.prompt.trim().is_empty() {
        return Err("prompt is empty".into());
    }
    if row.cron.trim().is_empty() {
        return Err("cron is empty".into());
    }
    if row.created_at <= 0 {
        return Err("timestamps are out of shape".into());
    }
    Ok(())
}

fn run_columns() -> &'static str {
    "id, task_id, started_at, status, reason, session_id, finished_at, payload"
}

fn run_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ScheduledRunRow> {
    Ok(ScheduledRunRow {
        id: row.get(0)?,
        task_id: row.get(1)?,
        started_at: row.get(2)?,
        status: row.get(3)?,
        reason: row.get(4)?,
        session_id: row.get(5)?,
        finished_at: row.get(6)?,
    })
}

fn valid_run(row: &ScheduledRunRow) -> Result<(), String> {
    if !is_uuid(&row.id) {
        return Err("id is not a uuid".into());
    }
    if !is_uuid(&row.task_id) {
        return Err("task id is not a uuid".into());
    }
    if !RUN_STATUSES.contains(&row.status.as_str()) {
        return Err("status is not a run status".into());
    }
    if row.started_at <= 0 {
        return Err("timestamps are out of shape".into());
    }
    Ok(())
}

impl Store {
    /// Create a task row. Duplicate names are allowed (the id is the
    /// identity); the caller owns cron validation and arming.
    pub fn create_scheduled_task(
        &self,
        task: NewScheduledTask<'_>,
    ) -> Result<ScheduledTaskRow, StoreError> {
        if task.name.trim().is_empty() {
            return Err(StoreError::Validation("the task name is required"));
        }
        if task.prompt.trim().is_empty() {
            return Err(StoreError::Validation("the task prompt is required"));
        }
        if task.cron.trim().is_empty() {
            return Err(StoreError::Validation("the schedule is required"));
        }
        let id = uuid::Uuid::new_v4().to_string();
        let now = now_ms();
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO scheduled_tasks (id, name, prompt, cron, enabled, project_id, cwd,
                                          created_at, armed_at, last_run_at, payload)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, NULL, ?9)",
            params![
                id,
                task.name.trim(),
                task.prompt.trim(),
                task.cron.trim(),
                task.enabled as i64,
                task.project_id,
                task.cwd,
                now,
                task.extra.to_string(),
            ],
        )?;
        drop(conn);
        self.read_scheduled_task(&id)
    }

    /// Re-read one task by id, validating its shape.
    pub fn read_scheduled_task(&self, id: &str) -> Result<ScheduledTaskRow, StoreError> {
        if !is_uuid(id) {
            return Err(StoreError::Validation("the task id is not a uuid"));
        }
        let conn = self.conn.lock().expect("store mutex poisoned");
        let raw = conn
            .query_row(
                &format!("SELECT {} FROM scheduled_tasks WHERE id = ?1", task_columns()),
                params![id],
                task_from_row,
            )
            .optional()?;
        match raw {
            Some(row) => match valid_task(&row) {
                Ok(()) => Ok(row),
                Err(reason) => {
                    self.quarantine_task(&conn, &row, &reason)?;
                    Err(StoreError::QuarantinedRecord {
                        source_table: "scheduled_tasks",
                        source_id: row.id,
                    })
                }
            },
            None => Err(StoreError::NotFound),
        }
    }

    /// All tasks, creation order descending (the renderer sorts).
    pub fn list_scheduled_tasks(&self) -> Result<Vec<ScheduledTaskRow>, StoreError> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(&format!(
            "SELECT {} FROM scheduled_tasks ORDER BY created_at DESC, id ASC",
            task_columns()
        ))?;
        let rows: Vec<ScheduledTaskRow> = stmt
            .query_map([], task_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        drop(stmt);

        let mut out = Vec::new();
        for row in rows {
            match valid_task(&row) {
                Ok(()) => out.push(row),
                Err(reason) => {
                    let id = row.id.clone();
                    self.quarantine_task(&conn, &row, &reason)?;
                    return Err(StoreError::QuarantinedRecord {
                        source_table: "scheduled_tasks",
                        source_id: id,
                    });
                }
            }
        }
        Ok(out)
    }

    /// One run row by id (the terminal-row response of `runScheduledTaskNow`
    /// reads exactly the row it recorded, never "the latest").
    pub fn read_scheduled_run(&self, run_id: &str) -> Result<ScheduledRunRow, StoreError> {
        if !is_uuid(run_id) {
            return Err(StoreError::Validation("the run id is not a uuid"));
        }
        let conn = self.conn.lock().expect("store mutex poisoned");
        let raw = conn
            .query_row(
                &format!("SELECT {} FROM scheduled_runs WHERE id = ?1", run_columns()),
                params![run_id],
                run_from_row,
            )
            .optional()?;
        match raw {
            Some(row) => {
                valid_run(&row).map_err(|_| StoreError::QuarantinedRecord {
                    source_table: "scheduled_runs",
                    source_id: row.id.clone(),
                })?;
                Ok(row)
            }
            None => Err(StoreError::NotFound),
        }
    }

    /// Enabled tasks whose armed occurrence is due (`armed_at <= now`). The
    /// engine's whole queue is this query — there is no second bookkeeping.
    pub fn due_scheduled_tasks(&self, now: i64) -> Result<Vec<ScheduledTaskRow>, StoreError> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(&format!(
            "SELECT {} FROM scheduled_tasks
             WHERE enabled = 1 AND armed_at IS NOT NULL AND armed_at <= ?1
             ORDER BY armed_at ASC, id ASC",
            task_columns()
        ))?;
        let rows: Vec<ScheduledTaskRow> = stmt
            .query_map(params![now], task_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        drop(stmt);
        for row in &rows {
            // The same shape guard the read paths enforce: a corrupt row in
            // the due queue must never dispatch.
            valid_task(row).map_err(|_| StoreError::QuarantinedRecord {
                source_table: "scheduled_tasks",
                source_id: row.id.clone(),
            })?;
        }
        Ok(rows)
    }

    fn quarantine_task(
        &self,
        conn: &Connection,
        row: &ScheduledTaskRow,
        reason: &str,
    ) -> Result<(), StoreError> {
        quarantine::quarantine(
            conn,
            "scheduled_tasks",
            &row.id,
            reason,
            &serde_json::json!(row).to_string(),
        )?;
        quarantine::remove(conn, "scheduled_tasks", &row.id)?;
        Ok(())
    }

    /// Replace name/prompt/cron and enabled. An enabled change or a schedule
    /// change clears the armed occurrence — the caller rearms with the new
    /// schedule. The project/folder context is intentionally not editable
    /// here (a moved task target is a new decision, not a silent retarget).
    #[allow(clippy::too_many_arguments)]
    pub fn update_scheduled_task(
        &self,
        id: &str,
        name: &str,
        prompt: &str,
        cron: &str,
        enabled: bool,
    ) -> Result<(), StoreError> {
        if !is_uuid(id) {
            return Err(StoreError::Validation("the task id is not a uuid"));
        }
        if name.trim().is_empty() {
            return Err(StoreError::Validation("the task name is required"));
        }
        if prompt.trim().is_empty() {
            return Err(StoreError::Validation("the task prompt is required"));
        }
        if cron.trim().is_empty() {
            return Err(StoreError::Validation("the schedule is required"));
        }
        let conn = self.conn.lock().expect("store mutex poisoned");
        let changed = conn.execute(
            "UPDATE scheduled_tasks
             SET name = ?2, prompt = ?3, cron = ?4, enabled = ?5, armed_at = NULL
             WHERE id = ?1",
            params![id, name.trim(), prompt.trim(), cron.trim(), enabled as i64],
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    /// Enable or disable. Disabling clears the armed occurrence and stops
    /// future runs; history is retained (REQ-SCHED states table).
    pub fn set_scheduled_task_enabled(&self, id: &str, enabled: bool) -> Result<(), StoreError> {
        if !is_uuid(id) {
            return Err(StoreError::Validation("the task id is not a uuid"));
        }
        let conn = self.conn.lock().expect("store mutex poisoned");
        let changed = conn.execute(
            "UPDATE scheduled_tasks SET enabled = ?2, armed_at = NULL WHERE id = ?1",
            params![id, enabled as i64],
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    /// Delete the task and its run rows. Run-created sessions are untouched.
    pub fn delete_scheduled_task(&self, id: &str) -> Result<(), StoreError> {
        if !is_uuid(id) {
            return Err(StoreError::Validation("the task id is not a uuid"));
        }
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute("DELETE FROM scheduled_runs WHERE task_id = ?1", params![id])?;
        let changed = conn.execute("DELETE FROM scheduled_tasks WHERE id = ?1", params![id])?;
        if changed == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    /// Arm the next pending occurrence (`armed_at`). Pass `None` to disarm.
    pub fn arm_scheduled_task(&self, id: &str, armed_at: Option<i64>) -> Result<(), StoreError> {
        if !is_uuid(id) {
            return Err(StoreError::Validation("the task id is not a uuid"));
        }
        let conn = self.conn.lock().expect("store mutex poisoned");
        let changed = conn.execute(
            "UPDATE scheduled_tasks SET armed_at = ?2 WHERE id = ?1",
            params![id, armed_at],
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    /// Record one run row for an occurrence and stamp the task's
    /// `last_run_at` for a run that actually executed (a skip is not a run,
    /// so it must never advance the last-run stamp — `TEST-SCHED-003`).
    pub fn record_scheduled_run(
        &self,
        task_id: &str,
        started_at: i64,
        status: &str,
        reason: Option<&str>,
        session_id: Option<&str>,
        finished_at: Option<i64>,
    ) -> Result<String, StoreError> {
        if !is_uuid(task_id) {
            return Err(StoreError::Validation("the task id is not a uuid"));
        }
        if !RUN_STATUSES.contains(&status) {
            return Err(StoreError::Validation("the run status is not known"));
        }
        let id = uuid::Uuid::new_v4().to_string();
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO scheduled_runs (id, task_id, started_at, status, reason, session_id,
                                         finished_at, payload)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, '{}')",
            params![id, task_id, started_at, status, reason, session_id, finished_at],
        )?;
        if status == "completed" || status == "failed" {
            conn.execute(
                "UPDATE scheduled_tasks SET last_run_at = ?2 WHERE id = ?1",
                params![task_id, started_at],
            )?;
        }
        Ok(id)
    }

    /// The most recent run rows for one task, newest first. Ties on
    /// `started_at` (the `running` and terminal rows of one occurrence share
    /// it) order by insert time (`rowid`), so the terminal row of an
    /// occurrence is always the newest — a random-uuid tie-break would make
    /// `lastRun` a coin flip (the same trap `MEMORY.md` §7f fixed for
    /// sessions).
    pub fn list_scheduled_runs(
        &self,
        task_id: &str,
        limit: usize,
    ) -> Result<Vec<ScheduledRunRow>, StoreError> {
        if !is_uuid(task_id) {
            return Err(StoreError::Validation("the task id is not a uuid"));
        }
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(&format!(
            "SELECT {} FROM scheduled_runs WHERE task_id = ?1
             ORDER BY started_at DESC, rowid DESC LIMIT ?2",
            run_columns()
        ))?;
        let rows: Vec<ScheduledRunRow> = stmt
            .query_map(params![task_id, limit as i64], run_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        drop(stmt);
        for row in &rows {
            valid_run(row).map_err(|_| StoreError::QuarantinedRecord {
                source_table: "scheduled_runs",
                source_id: row.id.clone(),
            })?;
        }
        Ok(rows)
    }
}
