//! The projects repository (roadmap Phase 6 slice 2; `docs/features/projects.md`,
//! `REQ-PROJ-01..06`).
//!
//! The shipped schema was never observed (`docs/11` does not capture a project
//! table); this is the rebuild model, recorded as such in `docs/14` §9: one row
//! per project with name, optional instructions, the canonical working folder,
//! and the project-scoped memory flag (`REQ-PROJ-04`).
//!
//! Deletion removes **only** the project row. Sessions keep their own `cwd`
//! and their `project_id` keeps pointing at the deleted id until it is read —
//! reads of a session whose project no longer exists resolve to "project
//! missing" at the service layer, never a silent retarget (`REQ-PROJ-06`,
//! `TEST-SESSION-005`'s "retention does not delete projects" spirit). There is
//! no audit chain here: `audit_events` is session-scoped by design, and no
//! project-level audit was captured; the omission is recorded, not hidden.
//!
//! Corrupt rows follow the store-wide quarantine-then-fail-read policy
//! (`TEST-SESSION-003`): the row moves to `quarantined_records` and the read
//! fails naming it — never a silent skip.

use rusqlite::{params, Connection, OptionalExtension};

use crate::quarantine;
use crate::{is_uuid, now_ms, Store, StoreError};

/// One project's persisted row.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProjectRow {
    pub id: String,
    pub name: String,
    pub instructions: Option<String>,
    pub cwd: String,
    pub memory_enabled: bool,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    #[serde(rename = "lastActivityAt")]
    pub last_activity_at: i64,
    #[serde(rename = "extra")]
    pub extra: serde_json::Value,
}

/// The fields a caller supplies at creation. The folder arrives already
/// canonicalized (the bridge validates and canonicalizes it against policy
/// before this layer sees it); this layer only checks shape.
#[derive(Debug, Clone, Default)]
pub struct NewProject<'a> {
    pub name: &'a str,
    pub instructions: Option<&'a str>,
    pub cwd: &'a str,
    pub memory_enabled: bool,
    pub extra: serde_json::Value,
}

fn project_columns() -> &'static str {
    "id, name, instructions, cwd, memory_enabled, created_at, last_activity_at, payload"
}

fn project_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProjectRow> {
    Ok(ProjectRow {
        id: row.get(0)?,
        name: row.get(1)?,
        instructions: row.get(2)?,
        cwd: row.get(3)?,
        memory_enabled: row.get::<_, i64>(4)? != 0,
        created_at: row.get(5)?,
        last_activity_at: row.get(6)?,
        extra: serde_json::from_str(&row.get::<_, String>(7)?).unwrap_or_default(),
    })
}

/// Validate one raw project row; `Err(reason)` names the field only.
fn valid_project(row: &ProjectRow) -> Result<(), String> {
    if !is_uuid(&row.id) {
        return Err("id is not a uuid".into());
    }
    if row.name.trim().is_empty() {
        return Err("name is empty".into());
    }
    if row.cwd.trim().is_empty() {
        return Err("cwd is empty".into());
    }
    if row.created_at <= 0 || row.last_activity_at <= 0 {
        return Err("timestamps are out of shape".into());
    }
    Ok(())
}

impl Store {
    /// Create a project row. Duplicate names are allowed; the id is the
    /// identity (strong inference recorded in `docs/features/projects.md`).
    pub fn create_project(&self, project: NewProject<'_>) -> Result<ProjectRow, StoreError> {
        if project.name.trim().is_empty() {
            return Err(StoreError::Validation("the project name is required"));
        }
        if project.cwd.trim().is_empty() {
            return Err(StoreError::Validation("the project folder is required"));
        }
        let id = uuid::Uuid::new_v4().to_string();
        let now = now_ms();
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO projects (id, name, instructions, cwd, memory_enabled,
                                   created_at, last_activity_at, payload)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, ?7)",
            params![
                id,
                project.name.trim(),
                project.instructions.map(str::trim).filter(|s| !s.is_empty()),
                project.cwd,
                project.memory_enabled as i64,
                now,
                project.extra.to_string(),
            ],
        )?;
        drop(conn);
        self.read_project(&id)
    }

    /// Re-read one project by id, validating its shape.
    pub fn read_project(&self, id: &str) -> Result<ProjectRow, StoreError> {
        if !is_uuid(id) {
            return Err(StoreError::Validation("the project id is not a uuid"));
        }
        let conn = self.conn.lock().expect("store mutex poisoned");
        let raw = conn
            .query_row(
                &format!("SELECT {} FROM projects WHERE id = ?1", project_columns()),
                params![id],
                project_from_row,
            )
            .optional()?;
        match raw {
            Some(row) => match valid_project(&row) {
                Ok(()) => Ok(row),
                Err(reason) => {
                    self.quarantine_project(&conn, &row, &reason)?;
                    Err(StoreError::QuarantinedRecord {
                        source_table: "projects",
                        source_id: row.id,
                    })
                }
            },
            None => Err(StoreError::NotFound),
        }
    }

    /// All projects, newest activity first, with the store-wide quarantine
    /// policy (`TEST-SESSION-003`).
    pub fn list_projects(&self) -> Result<Vec<ProjectRow>, StoreError> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(&format!(
            "SELECT {} FROM projects
             ORDER BY last_activity_at DESC, id ASC",
            project_columns()
        ))?;
        let rows: Vec<ProjectRow> = stmt
            .query_map([], project_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        drop(stmt);

        let mut out = Vec::new();
        for row in rows {
            match valid_project(&row) {
                Ok(()) => out.push(row),
                Err(reason) => {
                    let id = row.id.clone();
                    self.quarantine_project(&conn, &row, &reason)?;
                    return Err(StoreError::QuarantinedRecord {
                        source_table: "projects",
                        source_id: id,
                    });
                }
            }
        }
        Ok(out)
    }

    fn quarantine_project(
        &self,
        conn: &Connection,
        row: &ProjectRow,
        reason: &str,
    ) -> Result<(), StoreError> {
        quarantine::quarantine(
            conn,
            "projects",
            &row.id,
            reason,
            &serde_json::json!(row).to_string(),
        )?;
        quarantine::remove(conn, "projects", &row.id)?;
        Ok(())
    }

    /// Replace name/instructions/memory. The folder is **not** editable here:
    /// a moved project folder is a new project decision, not a silent rewrite
    /// of where past sessions ran (`docs/features/projects.md` "Missing
    /// folder" state). Callers that need a different folder create it through
    /// validation explicitly; this keeps the stored cwd the one sessions ran
    /// with.
    pub fn update_project(
        &self,
        id: &str,
        name: &str,
        instructions: Option<&str>,
        memory_enabled: bool,
    ) -> Result<(), StoreError> {
        if !is_uuid(id) {
            return Err(StoreError::Validation("the project id is not a uuid"));
        }
        if name.trim().is_empty() {
            return Err(StoreError::Validation("the project name is required"));
        }
        let conn = self.conn.lock().expect("store mutex poisoned");
        let changed = conn.execute(
            "UPDATE projects SET name = ?2, instructions = ?3, memory_enabled = ?4
             WHERE id = ?1",
            params![
                id,
                name.trim(),
                instructions.map(str::trim).filter(|s| !s.is_empty()),
                memory_enabled as i64,
            ],
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    /// Delete the project row. Sessions and their content are untouched
    /// (`REQ-PROJ-06`); the count of sessions that still reference the id is
    /// returned so the caller can say exactly what the deletion left behind.
    pub fn delete_project(&self, id: &str) -> Result<usize, StoreError> {
        if !is_uuid(id) {
            return Err(StoreError::Validation("the project id is not a uuid"));
        }
        let conn = self.conn.lock().expect("store mutex poisoned");
        let orphaned: usize = conn.query_row(
            "SELECT COUNT(*) FROM sessions WHERE project_id = ?1",
            params![id],
            |row| row.get(0),
        )?;
        let changed = conn.execute("DELETE FROM projects WHERE id = ?1", params![id])?;
        if changed == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(orphaned)
    }

    /// Bump `last_activity_at` (a session in the project turning).
    pub fn touch_project(&self, id: &str) -> Result<(), StoreError> {
        if !is_uuid(id) {
            return Err(StoreError::Validation("the project id is not a uuid"));
        }
        let conn = self.conn.lock().expect("store mutex poisoned");
        let changed = conn.execute(
            "UPDATE projects SET last_activity_at = ?2 WHERE id = ?1",
            params![id, now_ms()],
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    /// The sessions associated with one project, newest activity first.
    /// Includes both surfaces: the project list shows everything the project
    /// owns. A session row whose own shape is corrupt still quarantines and
    /// fails the read (same policy as `list_sessions`).
    pub fn list_sessions_for_project(
        &self,
        project_id: &str,
        archived: Option<bool>,
    ) -> Result<Vec<crate::repos::SessionRow>, StoreError> {
        if !is_uuid(project_id) {
            return Err(StoreError::Validation("the project id is not a uuid"));
        }
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(&format!(
            "SELECT {} FROM sessions
             WHERE project_id = ?1 AND (?2 IS NULL OR archived = ?2)
             ORDER BY last_activity_at DESC, id ASC",
            crate::repos::session_columns(),
        ))?;
        let raws: Vec<crate::repos::SessionRowParsed> = stmt
            .query_map(
                params![project_id, archived.map(|a| a as i64)],
                crate::repos::SessionRowParsed::from_row,
            )?
            .collect::<Result<Vec<_>, _>>()?;
        drop(stmt);

        let mut out = Vec::new();
        for parsed in raws {
            match crate::repos::valid_session(&parsed) {
                Ok(()) => out.push(parsed.into_row()),
                Err(reason) => {
                    let id = parsed.id.clone();
                    quarantine::quarantine(
                        &conn,
                        "sessions",
                        &id,
                        &reason,
                        &serde_json::json!(&parsed).to_string(),
                    )?;
                    quarantine::remove(&conn, "sessions", &id)?;
                    return Err(StoreError::QuarantinedRecord {
                        source_table: "sessions",
                        source_id: id,
                    });
                }
            }
        }
        Ok(out)
    }

    /// Attach or detach a session's project association. Detach (`None`) is
    /// what "move out of the project" means; deleting the project never calls
    /// this — the stale id is resolved as "project missing" at read time.
    pub fn set_session_project(
        &self,
        session_id: &str,
        project_id: Option<&str>,
    ) -> Result<(), StoreError> {
        if !is_uuid(session_id) {
            return Err(StoreError::Validation("the session id is not a uuid"));
        }
        if let Some(id) = project_id {
            if !is_uuid(id) {
                return Err(StoreError::Validation("the project id is not a uuid"));
            }
        }
        let conn = self.conn.lock().expect("store mutex poisoned");
        let changed = conn.execute(
            "UPDATE sessions SET project_id = ?2 WHERE id = ?1",
            params![session_id, project_id],
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }
}
