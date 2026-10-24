//! The repositories: sessions, turns, usage events, and the audit chain.
//!
//! Attendance rows for this module: `TEST-SESSION-001` (create/reopen/archive/
//! delete — the persistence half; send/stream/stop arrive with the agent loop),
//! `TEST-SESSION-003` (corrupt rows quarantine, never delete), `TEST-SESSION-
//! 004` (delete removes rows *and* content files), `TEST-SESSION-007` /
//! `TEST-DATA-007` (usage events keep every count verbatim), `TEST-SESSION-006`
//! / `TEST-DATA-006` (the per-session HMAC chain), and `TEST-DATA-005` (legal
//! hold refuses deletion until released).
//!
//! Ids are UUIDv4 strings; timestamps are epoch-millisecond UTC; large content
//! lives only as blob references (`crate::blobs`). Every statement is a static
//! string with bound parameters — no value ever touches SQL text. Errors carry
//! identifiers, never content (`docs/09` §4).
//!
//! Corrupt-row policy: a stored row that fails validation is moved into
//! `quarantined_records`, removed from its table, and the read **fails naming
//! it** (`StoreError::QuarantinedRecord`). The retry after the error shows the
//! surviving rows. Never a silent skip: a transcript with a quietly-missing
//! turn would be a lie about what happened (`TEST-SESSION-003`).

use hmac::{Hmac, Mac};
use rusqlite::{params, Connection, OptionalExtension};
use sha2::Sha256;

use crate::quarantine;
use crate::{is_blob_ref, is_uuid, now_ms, Store, StoreError};

type HmacSha256 = Hmac<Sha256>;

// ---------------------------------------------------------------------
// Shapes
// ---------------------------------------------------------------------

/// One session's persisted row. `extra` is the verbatim passthrough of every
/// field the captures did not document — unknown keys survive round trips
/// (`TEST-DATA-001`); the documented fields above are the typed half
/// (`docs/11` §4.2's observed groups carried into `docs/11` §14's table).
#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionRow {
    pub id: String,
    pub surface: String,
    pub title: String,
    pub cwd: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub permission_mode: Option<String>,
    pub archived: bool,
    pub legal_hold: bool,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    #[serde(rename = "lastActivityAt")]
    pub last_activity_at: i64,
    #[serde(rename = "extra")]
    pub extra: serde_json::Value,
}

/// The fields a caller supplies at creation. `extra` is stored verbatim and
/// returned verbatim.
#[derive(Debug, Clone, Default)]
pub struct NewSession<'a> {
    pub surface: &'a str,
    pub title: &'a str,
    pub cwd: Option<&'a str>,
    pub model: Option<&'a str>,
    pub effort: Option<&'a str>,
    pub permission_mode: Option<&'a str>,
    pub extra: serde_json::Value,
}

/// The turn's role — the stored spelling is the lowercase enum name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TurnRole {
    User,
    Assistant,
    System,
    Tool,
}

impl TurnRole {
    pub fn as_stored(self) -> &'static str {
        match self {
            TurnRole::User => "user",
            TurnRole::Assistant => "assistant",
            TurnRole::System => "system",
            TurnRole::Tool => "tool",
        }
    }
    fn parse_stored(s: &str) -> Option<TurnRole> {
        match s {
            "user" => Some(TurnRole::User),
            "assistant" => Some(TurnRole::Assistant),
            "system" => Some(TurnRole::System),
            "tool" => Some(TurnRole::Tool),
            _ => None,
        }
    }
}

/// The turn's status. `partial` is what a cancel/interrupt leaves in place —
/// the transcript records what happened; it does not pretend a partial turn
/// completed (`docs/13` Phase 4 "error/partial/stop states").
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TurnStatus {
    Complete,
    Partial,
    Error,
}

impl TurnStatus {
    pub fn as_stored(self) -> &'static str {
        match self {
            TurnStatus::Complete => "complete",
            TurnStatus::Partial => "partial",
            TurnStatus::Error => "error",
        }
    }
    fn parse_stored(s: &str) -> Option<TurnStatus> {
        match s {
            "complete" => Some(TurnStatus::Complete),
            "partial" => Some(TurnStatus::Partial),
            "error" => Some(TurnStatus::Error),
            _ => None,
        }
    }
}

/// One turn: a per-session sequence number, a blob reference for the content,
/// and the status/usage/error tail.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TurnRow {
    pub id: String,
    #[serde(rename = "sessionId")]
    pub session_id: String,
    pub seq: i64,
    pub role: String,
    pub status: String,
    #[serde(rename = "contentRef")]
    pub content_ref: String,
    #[serde(rename = "usageJson")]
    pub usage_json: Option<String>,
    pub error: Option<String>,
    /// The provider's own stop spelling (`docs/12` §4.1) — never collapsed into
    /// a generic "done".
    #[serde(rename = "stopReason")]
    pub stop_reason: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
}

/// A usage event as preserved verbatim (`TEST-SESSION-007`): every count the
/// adapter reported is stored, including which channel errored. Aggregation
/// (`TEST-DATA-007`'s 7/30/90 reading) is a later slice; this row is the truth
/// the aggregation reads.
#[derive(Debug, Clone, serde::Serialize)]
pub struct UsageEventRow {
    pub id: String,
    #[serde(rename = "sessionId")]
    pub session_id: String,
    pub model: String,
    #[serde(rename = "inputTokens")]
    pub input_tokens: i64,
    #[serde(rename = "outputTokens")]
    pub output_tokens: i64,
    #[serde(rename = "cacheReadTokens")]
    pub cache_read_tokens: i64,
    #[serde(rename = "cacheWriteTokens")]
    pub cache_write_tokens: i64,
    #[serde(rename = "searchRequests")]
    pub search_requests: i64,
    pub error: Option<String>,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Default)]
pub struct NewUsageEvent<'a> {
    pub model: &'a str,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_write_tokens: i64,
    pub search_requests: i64,
    pub error: Option<&'a str>,
}

/// One audit event. The chain fields (`prev_hmac`, `hmac`, `payload_hash`) are
/// the tamper evidence; the audit *key* never lives in the database
/// (`docs/11` §4.3: "store audit keys in the OS keychain") — it rides the call.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AuditEventRow {
    pub id: String,
    #[serde(rename = "sessionId")]
    pub session_id: String,
    pub actor: String,
    pub action: String,
    pub message: Option<String>,
    #[serde(rename = "payloadHash")]
    pub payload_hash: String,
    #[serde(rename = "prevHmac")]
    pub prev_hmac: String,
    pub hmac: String,
    pub timestamp: i64,
}

// ---------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------

fn session_columns() -> &'static str {
    "id, surface, title, cwd, model, effort, permission_mode,
     archived, legal_hold, created_at, last_activity_at, payload"
}

fn turn_columns() -> &'static str {
    "id, session_id, seq, role, status, content_ref, usage_json, error, created_at, stop_reason"
}

fn usage_columns() -> &'static str {
    "id, session_id, model, input_tokens, output_tokens, cache_read_tokens,
     cache_write_tokens, search_requests, error, timestamp"
}

fn audit_columns() -> &'static str {
    "id, session_id, actor, action, message, payload_hash, prev_hmac, hmac, timestamp"
}

fn turn_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TurnRow> {
    Ok(TurnRow {
        id: row.get(0)?,
        session_id: row.get(1)?,
        seq: row.get(2)?,
        role: row.get(3)?,
        status: row.get(4)?,
        content_ref: row.get(5)?,
        usage_json: row.get(6)?,
        error: row.get(7)?,
        created_at: row.get(8)?,
        stop_reason: row.get(9)?,
    })
}

fn usage_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<UsageEventRow> {
    Ok(UsageEventRow {
        id: row.get(0)?,
        session_id: row.get(1)?,
        model: row.get(2)?,
        input_tokens: row.get(3)?,
        output_tokens: row.get(4)?,
        cache_read_tokens: row.get(5)?,
        cache_write_tokens: row.get(6)?,
        search_requests: row.get(7)?,
        error: row.get(8)?,
        timestamp: row.get(9)?,
    })
}

/// The raw session row before validation.
#[derive(Debug, serde::Serialize)]
struct SessionRowParsed {
    id: String,
    surface: String,
    title: String,
    cwd: Option<String>,
    model: Option<String>,
    effort: Option<String>,
    permission_mode: Option<String>,
    archived: bool,
    legal_hold: bool,
    created_at: i64,
    last_activity_at: i64,
    payload: String,
}

impl SessionRowParsed {
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SessionRowParsed> {
        Ok(SessionRowParsed {
            id: row.get(0)?,
            surface: row.get(1)?,
            title: row.get(2)?,
            cwd: row.get(3)?,
            model: row.get(4)?,
            effort: row.get(5)?,
            permission_mode: row.get(6)?,
            archived: row.get::<_, i64>(7)? != 0,
            legal_hold: row.get::<_, i64>(8)? != 0,
            created_at: row.get(9)?,
            last_activity_at: row.get(10)?,
            payload: row.get(11)?,
        })
    }

    fn into_row(mut self) -> SessionRow {
        // `payload` was validated as JSON before this point; the passthrough
        // is the caller's unknown fields, returned exactly as stored.
        let extra = serde_json::from_str::<serde_json::Value>(&self.payload).unwrap_or_default();
        SessionRow {
            id: self.id,
            surface: self.surface,
            title: self.title,
            cwd: self.cwd.take(),
            model: self.model.take(),
            effort: self.effort.take(),
            permission_mode: self.permission_mode.take(),
            archived: self.archived,
            legal_hold: self.legal_hold,
            created_at: self.created_at,
            last_activity_at: self.last_activity_at,
            extra,
        }
    }
}

/// Validate one raw session row; `Err(reason)` names the field only.
fn valid_session(row: &SessionRowParsed) -> Result<(), String> {
    if !is_uuid(&row.id) {
        return Err("id is not a uuid".into());
    }
    if row.surface.trim().is_empty() {
        return Err("surface is empty".into());
    }
    if row.title.trim().is_empty() {
        return Err("title is empty".into());
    }
    if row.created_at <= 0 || row.last_activity_at <= 0 {
        return Err("timestamps are out of shape".into());
    }
    if serde_json::from_str::<serde_json::Value>(&row.payload).is_err() {
        return Err("payload is not valid JSON".into());
    }
    Ok(())
}

fn valid_turn(row: &TurnRow) -> Result<(), String> {
    if !is_uuid(&row.id) {
        return Err("id is not a uuid".into());
    }
    if row.seq < 0 {
        return Err("seq is negative".into());
    }
    if TurnRole::parse_stored(&row.role).is_none() {
        return Err("role spelling is out of shape".into());
    }
    if TurnStatus::parse_stored(&row.status).is_none() {
        return Err("status spelling is out of shape".into());
    }
    if !is_blob_ref(&row.content_ref) {
        return Err("content_ref is not a blob hash".into());
    }
    if let Some(usage) = &row.usage_json {
        if serde_json::from_str::<serde_json::Value>(usage).is_err() {
            return Err("usage_json is not valid JSON".into());
        }
    }
    Ok(())
}

fn valid_usage(row: &UsageEventRow) -> Result<(), String> {
    if !is_uuid(&row.id) {
        return Err("id is not a uuid".into());
    }
    if row.model.trim().is_empty() {
        return Err("model is empty".into());
    }
    if row.input_tokens < 0
        || row.output_tokens < 0
        || row.cache_read_tokens < 0
        || row.cache_write_tokens < 0
        || row.search_requests < 0
    {
        return Err("a count is negative".into());
    }
    if row.timestamp <= 0 {
        return Err("timestamp is out of shape".into());
    }
    Ok(())
}

// ---------------------------------------------------------------------
// Sessions
// ---------------------------------------------------------------------

impl Store {
    /// Create a session row. Ids are UUIDv4 (`docs/18` §10's id posture); both
    /// timestamps start at creation.
    pub fn create_session(&self, session: NewSession<'_>) -> Result<SessionRow, StoreError> {
        if session.surface.trim().is_empty() {
            return Err(StoreError::Validation("the session surface is required"));
        }
        if session.title.trim().is_empty() {
            return Err(StoreError::Validation("the session title is required"));
        }
        let id = uuid::Uuid::new_v4().to_string();
        let now = now_ms();
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO sessions (id, surface, title, cwd, model, effort, permission_mode,
                                   archived, legal_hold, created_at, last_activity_at, payload)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, 0, ?8, ?8, ?9)",
            params![
                id,
                session.surface,
                session.title,
                session.cwd,
                session.model,
                session.effort,
                session.permission_mode,
                now,
                session.extra.to_string(),
            ],
        )?;
        drop(conn);
        self.read_session(&id)
    }

    /// Re-read one session by id, validating its shape.
    pub fn read_session(&self, id: &str) -> Result<SessionRow, StoreError> {
        if !is_uuid(id) {
            return Err(StoreError::Validation("the session id is not a uuid"));
        }
        let conn = self.conn.lock().expect("store mutex poisoned");
        let raw = conn
            .query_row(
                &format!("SELECT {} FROM sessions WHERE id = ?1", session_columns()),
                params![id],
                SessionRowParsed::from_row,
            )
            .optional()?;
        drop(conn);
        match raw {
            Some(parsed) => match valid_session(&parsed) {
                Ok(()) => Ok(parsed.into_row()),
                Err(reason) => {
                    self.quarantine_session(&parsed, &reason)?;
                    Err(StoreError::QuarantinedRecord {
                        source_table: "sessions",
                        source_id: parsed.id,
                    })
                }
            },
            None => Err(StoreError::NotFound),
        }
    }

    /// All sessions, newest activity first. A corrupt row is quarantined and
    /// the read fails naming it; the retry lists the survivors
    /// (`TEST-SESSION-003` — explicit, never silent).
    pub fn list_sessions(&self, archived: Option<bool>) -> Result<Vec<SessionRow>, StoreError> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(&format!(
            "SELECT {} FROM sessions WHERE (?1 IS NULL OR archived = ?1)
             ORDER BY last_activity_at DESC, id ASC",
            session_columns()
        ))?;
        let raws: Vec<SessionRowParsed> = stmt
            .query_map(params![archived.map(|a| a as i64)], SessionRowParsed::from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        drop(stmt);

        let mut out = Vec::new();
        for parsed in raws {
            match valid_session(&parsed) {
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

    fn quarantine_session(
        &self,
        parsed: &SessionRowParsed,
        reason: &str,
    ) -> Result<(), StoreError> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        quarantine::quarantine(
            &conn,
            "sessions",
            &parsed.id,
            reason,
            &serde_json::json!(parsed).to_string(),
        )?;
        quarantine::remove(&conn, "sessions", &parsed.id)?;
        Ok(())
    }

    /// Flip the archived flag; every other field is untouched.
    pub fn set_archived(&self, id: &str, archived: bool) -> Result<(), StoreError> {
        if !is_uuid(id) {
            return Err(StoreError::Validation("the session id is not a uuid"));
        }
        let conn = self.conn.lock().expect("store mutex poisoned");
        let changed = conn.execute(
            "UPDATE sessions SET archived = ?2 WHERE id = ?1",
            params![id, archived as i64],
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    /// Rename a session.
    pub fn set_title(&self, id: &str, title: &str) -> Result<(), StoreError> {
        if !is_uuid(id) {
            return Err(StoreError::Validation("the session id is not a uuid"));
        }
        if title.trim().is_empty() {
            return Err(StoreError::Validation("the session title is required"));
        }
        let conn = self.conn.lock().expect("store mutex poisoned");
        let changed = conn.execute(
            "UPDATE sessions SET title = ?2 WHERE id = ?1",
            params![id, title],
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    /// Mark/unmark the legal hold. A held session refuses deletion until this
    /// is called with `false` (`TEST-DATA-005`).
    pub fn set_legal_hold(&self, id: &str, hold: bool) -> Result<(), StoreError> {
        if !is_uuid(id) {
            return Err(StoreError::Validation("the session id is not a uuid"));
        }
        let conn = self.conn.lock().expect("store mutex poisoned");
        let changed = conn.execute(
            "UPDATE sessions SET legal_hold = ?2 WHERE id = ?1",
            params![id, hold as i64],
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    /// Delete a session and everything it owns. The row deletion (and the
    /// cascade over turns/usage/audit) is one transaction (`TEST-DATA-002`);
    /// the content files are removed afterwards, and the removed references
    /// are returned so a caller can log exactly what left (`TEST-SESSION-004`).
    /// [`StoreError::LegalHold`] stands while the hold stands (`TEST-DATA-005`).
    pub fn delete_session(&self, id: &str) -> Result<Vec<String>, StoreError> {
        if !is_uuid(id) {
            return Err(StoreError::Validation("the session id is not a uuid"));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let held: Option<i64> = conn
            .query_row(
                "SELECT legal_hold FROM sessions WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .optional()?;
        match held {
            None => return Err(StoreError::NotFound),
            Some(v) if v != 0 => return Err(StoreError::LegalHold),
            Some(_) => {}
        }
        let refs: Vec<String> = {
            let mut stmt = conn.prepare("SELECT content_ref FROM turns WHERE session_id = ?1")?;
            let rows = stmt
                .query_map(params![id], |row| row.get(0))?
                .collect::<Result<Vec<_>, _>>()?;
            rows
        };
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM sessions WHERE id = ?1", params![id])?;
        tx.commit()?;
        // Content is addressed by hash, so two sessions can reference the same
        // blob. Delete only the refs no surviving turn still needs; removing a
        // shared blob would corrupt the neighbor (`TEST-SESSION-004`).
        let mut removed = Vec::new();
        for item in &refs {
            let still_referenced: Option<i64> = conn
                .query_row(
                    "SELECT 1 FROM turns WHERE content_ref = ?1 LIMIT 1",
                    params![item],
                    |row| row.get(0),
                )
                .optional()?;
            if still_referenced.is_none() {
                self.files.delete(item)?;
                removed.push(item.clone());
            }
        }
        drop(conn);
        Ok(removed)
    }

    /// Bump `last_activity_at` (the agent loop calls this per turn).
    pub fn touch_session(&self, id: &str) -> Result<(), StoreError> {
        if !is_uuid(id) {
            return Err(StoreError::Validation("the session id is not a uuid"));
        }
        let conn = self.conn.lock().expect("store mutex poisoned");
        let changed = conn.execute(
            "UPDATE sessions SET last_activity_at = ?2 WHERE id = ?1",
            params![id, now_ms()],
        )?;
        if changed == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------
// Turns
// ---------------------------------------------------------------------

impl Store {
    fn require_session(&self, conn: &Connection, id: &str) -> Result<(), StoreError> {
        let found: Option<i64> = conn
            .query_row("SELECT 1 FROM sessions WHERE id = ?1", params![id], |r| {
                r.get(0)
            })
            .optional()?;
        if found.is_none() {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    /// Write one turn: the content goes into the content-addressed store, the
    /// row references it. A reused `(session, seq)` pair is refused — appending
    /// twice to the same slot is a caller mistake, not a silent overwrite.
    #[allow(clippy::too_many_arguments)]
    pub fn put_turn(
        &self,
        session_id: &str,
        seq: i64,
        role: TurnRole,
        status: TurnStatus,
        content: &[u8],
        usage_json: Option<serde_json::Value>,
        error: Option<&str>,
        stop_reason: Option<&str>,
    ) -> Result<TurnRow, StoreError> {
        if !is_uuid(session_id) {
            return Err(StoreError::Validation("the session id is not a uuid"));
        }
        if seq < 0 {
            return Err(StoreError::Validation("the turn sequence is negative"));
        }
        let conn = self.conn.lock().expect("store mutex poisoned");
        self.require_session(&conn, session_id)?;

        let content_ref = self.files.put(content)?;
        let id = uuid::Uuid::new_v4().to_string();
        let created_at = now_ms();
        let usage_json_text = usage_json.as_ref().map(|v| v.to_string());
        if let Err(err) = conn.execute(
            "INSERT INTO turns (id, session_id, seq, role, status, content_ref, usage_json, error, created_at, stop_reason)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                id,
                session_id,
                seq,
                role.as_stored(),
                status.as_stored(),
                content_ref,
                usage_json_text,
                error,
                created_at,
                stop_reason,
            ],
        ) {
            return match err {
                rusqlite::Error::SqliteFailure(e, _)
                    if e.code == rusqlite::ErrorCode::ConstraintViolation =>
                {
                    // The blob was written; a turn slot's content is immutable
                    // anyway, so a stale blob for a refused insert is harmless
                    // until the unreferenced sweep runs.
                    Err(StoreError::Validation(
                        "a turn already occupies this sequence for the session",
                    ))
                }
                other => Err(other.into()),
            };
        }

        Ok(TurnRow {
            id,
            session_id: session_id.to_string(),
            seq,
            role: role.as_stored().to_string(),
            status: status.as_stored().to_string(),
            content_ref,
            usage_json: usage_json_text,
            error: error.map(|e| e.to_string()),
            stop_reason: stop_reason.map(|s| s.to_string()),
            created_at,
        })
    }

    /// Finish a live turn: the assistant placeholder written at send time is
    /// updated with its terminal status and final content. Only a `partial`
    /// row with no error is live — the agent's start marker; a finished row is
    /// refused rather than rewritten, so a retry cannot silently overwrite the
    /// record. The audit chain records the start and the finish as separate
    /// links (`SessionService`); this method moves the row only.
    #[allow(clippy::too_many_arguments)]
    pub fn finish_turn(
        &self,
        session_id: &str,
        turn_id: &str,
        status: TurnStatus,
        content: &[u8],
        usage_json: Option<serde_json::Value>,
        error: Option<&str>,
        stop_reason: Option<&str>,
    ) -> Result<TurnRow, StoreError> {
        if !is_uuid(session_id) {
            return Err(StoreError::Validation("the session id is not a uuid"));
        }
        if !is_uuid(turn_id) {
            return Err(StoreError::Validation("the turn id is not a uuid"));
        }
        let usage_json_text = usage_json.as_ref().map(|value| value.to_string());
        let content_ref = self.files.put(content)?;
        let conn = self.conn.lock().expect("store mutex poisoned");
        let current: Option<(String, String, Option<String>)> = conn
            .query_row(
                "SELECT session_id, status, error FROM turns WHERE id = ?1",
                params![turn_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let Some((row_session_id, current_status, current_error)) = current else {
            return Err(StoreError::NotFound);
        };
        if row_session_id != session_id {
            return Err(StoreError::Validation("the turn belongs to another session"));
        }
        if current_status != TurnStatus::Partial.as_stored() || current_error.is_some() {
            return Err(StoreError::Validation("the turn is already finished"));
        }
        conn.execute(
            "UPDATE turns SET status = ?2, content_ref = ?3, usage_json = ?4, error = ?5,
                            stop_reason = ?6
             WHERE id = ?1",
            params![
                turn_id,
                status.as_stored(),
                content_ref,
                usage_json_text,
                error,
                stop_reason
            ],
        )?;
        let turn = conn.query_row(
            &format!("SELECT {} FROM turns WHERE id = ?1", turn_columns()),
            params![turn_id],
            turn_from_row,
        )?;
        drop(conn);
        self.touch_session(session_id)?;
        Ok(turn)
    }

    /// The session's transcript, sequence order. Content bytes are NOT loaded
    /// here — callers resolve `content_ref` through `self.files` lazily (the
    /// shipped session files ballooned because everything loaded eagerly;
    /// `docs/11` §4.2: startup restore must not load every transcript fully).
    ///
    /// A corrupt turn is quarantined and the read fails naming it; the retry
    /// lists the surviving turns.
    pub fn list_turns(&self, session_id: &str) -> Result<Vec<TurnRow>, StoreError> {
        if !is_uuid(session_id) {
            return Err(StoreError::Validation("the session id is not a uuid"));
        }
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(&format!(
            "SELECT {} FROM turns WHERE session_id = ?1 ORDER BY seq ASC",
            turn_columns()
        ))?;
        let rows: Vec<TurnRow> = stmt
            .query_map(params![session_id], turn_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        drop(stmt);

        let mut out = Vec::new();
        for row in rows {
            match valid_turn(&row) {
                Ok(()) => out.push(row),
                Err(reason) => {
                    quarantine::quarantine(
                        &conn,
                        "turns",
                        &row.id,
                        &reason,
                        &serde_json::json!(&row).to_string(),
                    )?;
                    quarantine::remove(&conn, "turns", &row.id)?;
                    return Err(StoreError::QuarantinedRecord {
                        source_table: "turns",
                        source_id: row.id,
                    });
                }
            }
        }
        Ok(out)
    }
}

// ---------------------------------------------------------------------
// Usage events
// ---------------------------------------------------------------------

impl Store {
    /// Record one usage event verbatim (`TEST-SESSION-007`). Counts are what
    /// the adapter reported — this layer never computes them.
    pub fn record_usage(
        &self,
        session_id: &str,
        event: NewUsageEvent<'_>,
    ) -> Result<UsageEventRow, StoreError> {
        if !is_uuid(session_id) {
            return Err(StoreError::Validation("the session id is not a uuid"));
        }
        if event.model.trim().is_empty() {
            return Err(StoreError::Validation("the usage event needs a model"));
        }
        if event.input_tokens < 0
            || event.output_tokens < 0
            || event.cache_read_tokens < 0
            || event.cache_write_tokens < 0
            || event.search_requests < 0
        {
            return Err(StoreError::Validation("a usage count is negative"));
        }
        let id = uuid::Uuid::new_v4().to_string();
        let conn = self.conn.lock().expect("store mutex poisoned");
        self.require_session(&conn, session_id)?;
        let timestamp = now_ms();
        conn.execute(
            "INSERT INTO usage_events (id, session_id, model, input_tokens, output_tokens,
                                       cache_read_tokens, cache_write_tokens, search_requests,
                                       error, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                id,
                session_id,
                event.model,
                event.input_tokens,
                event.output_tokens,
                event.cache_read_tokens,
                event.cache_write_tokens,
                event.search_requests,
                event.error,
                timestamp,
            ],
        )?;
        Ok(UsageEventRow {
            id,
            session_id: session_id.to_string(),
            model: event.model.to_string(),
            input_tokens: event.input_tokens,
            output_tokens: event.output_tokens,
            cache_read_tokens: event.cache_read_tokens,
            cache_write_tokens: event.cache_write_tokens,
            search_requests: event.search_requests,
            error: event.error.map(|e| e.to_string()),
            timestamp,
        })
    }

    /// The session's usage events, chronological (`TEST-DATA-007`'s
    /// preservation half — the aggregation itself is a later slice).
    pub fn list_usage(&self, session_id: &str) -> Result<Vec<UsageEventRow>, StoreError> {
        if !is_uuid(session_id) {
            return Err(StoreError::Validation("the session id is not a uuid"));
        }
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(&format!(
            "SELECT {} FROM usage_events WHERE session_id = ?1 ORDER BY timestamp ASC, id ASC",
            usage_columns()
        ))?;
        let rows: Vec<UsageEventRow> = stmt
            .query_map(params![session_id], usage_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        drop(stmt);

        let mut out = Vec::new();
        for row in rows {
            match valid_usage(&row) {
                Ok(()) => out.push(row),
                Err(reason) => {
                    quarantine::quarantine(
                        &conn,
                        "usage_events",
                        &row.id,
                        &reason,
                        &serde_json::json!(&row).to_string(),
                    )?;
                    quarantine::remove(&conn, "usage_events", &row.id)?;
                    return Err(StoreError::QuarantinedRecord {
                        source_table: "usage_events",
                        source_id: row.id,
                    });
                }
            }
        }
        Ok(out)
    }

    /// Usage events in a closed time window (`TEST-SESSION-008` /
    /// `TEST-DATA-007`'s aggregation input): the caller decides the window
    /// boundaries from its own local-day arithmetic; this layer only selects
    /// and returns the raw rows, newest last. `session_id` narrows to one
    /// session; `None` is the device-wide ledger.
    pub fn list_usage_between(
        &self,
        from_ms: i64,
        to_ms: i64,
        session_id: Option<&str>,
    ) -> Result<Vec<UsageEventRow>, StoreError> {
        if to_ms < from_ms {
            return Err(StoreError::Validation(
                "the usage window ends before it starts",
            ));
        }
        if let Some(id) = session_id {
            if !is_uuid(id) {
                return Err(StoreError::Validation("the session id is not a uuid"));
            }
        }
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(&format!(
            "SELECT {} FROM usage_events
             WHERE timestamp >= ?1 AND timestamp <= ?2
               AND (?3 IS NULL OR session_id = ?3)
             ORDER BY timestamp ASC, id ASC",
            usage_columns()
        ))?;
        let rows: Vec<UsageEventRow> = stmt
            .query_map(params![from_ms, to_ms, session_id], usage_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        drop(stmt);
        Ok(rows)
    }
}

// ---------------------------------------------------------------------
// The audit chain
// ---------------------------------------------------------------------

/// The canonical bytes an HMAC covers: the previous chain link, the payload
/// hash, and every authenticated field, separated by NUL. Changing any stored
/// field (or the order) changes the bytes and breaks the chain.
fn audit_chain_bytes(row: &AuditEventRow) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(row.prev_hmac.as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(row.payload_hash.as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(row.actor.as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(row.action.as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(row.message.as_deref().unwrap_or("").as_bytes());
    bytes.push(0);
    bytes.extend_from_slice(&row.timestamp.to_le_bytes());
    bytes
}

fn hex_digest(bytes: &[u8]) -> String {
    use sha2::Digest;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The HMAC of one audit event, as lowercase hex.
fn audit_mac(row: &AuditEventRow, key: &[u8; 32]) -> Result<String, StoreError> {
    let mut mac = HmacSha256::new_from_slice(key)
        .map_err(|_| StoreError::Validation("the audit key is not 32 bytes"))?;
    mac.update(&audit_chain_bytes(row));
    Ok(mac
        .finalize()
        .into_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

impl Store {
    /// Append one audit event, chained onto the previous record for the
    /// session (`docs/11` §4.3's tamper-evident trail). The key is supplied per
    /// call and never stored here.
    pub fn append_audit(
        &self,
        session_id: &str,
        actor: &str,
        action: &str,
        message: Option<&str>,
        payload: &[u8],
        key: &[u8; 32],
    ) -> Result<AuditEventRow, StoreError> {
        if !is_uuid(session_id) {
            return Err(StoreError::Validation("the session id is not a uuid"));
        }
        if actor.trim().is_empty() || action.trim().is_empty() {
            return Err(StoreError::Validation(
                "the audit event needs an actor and an action",
            ));
        }
        let payload_hash = hex_digest(payload);
        let conn = self.conn.lock().expect("store mutex poisoned");
        self.require_session(&conn, session_id)?;
        let prev_hmac: String = conn
            .query_row(
                "SELECT hmac FROM audit_events WHERE session_id = ?1 ORDER BY rowid DESC LIMIT 1",
                params![session_id],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or_default();

        let id = uuid::Uuid::new_v4().to_string();
        let timestamp = now_ms();
        let mut row = AuditEventRow {
            id,
            session_id: session_id.to_string(),
            actor: actor.to_string(),
            action: action.to_string(),
            message: message.map(|m| m.to_string()),
            payload_hash,
            prev_hmac,
            hmac: String::new(),
            timestamp,
        };
        row.hmac = audit_mac(&row, key)?;

        conn.execute(
            "INSERT INTO audit_events (id, session_id, actor, action, message, payload_hash,
                                       prev_hmac, hmac, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                row.id,
                row.session_id,
                actor,
                action,
                message,
                row.payload_hash,
                row.prev_hmac,
                row.hmac,
                timestamp
            ],
        )?;
        Ok(row)
    }

    /// Verify the session's whole audit chain (`TEST-SESSION-006` /
    /// `TEST-DATA-006`): every link's HMAC must recompute from the stored
    /// fields and each link must name its predecessor. Verification modifies
    /// nothing — tamper evidence is evidence.
    pub fn verify_audit(&self, session_id: &str, key: &[u8; 32]) -> Result<usize, StoreError> {
        if !is_uuid(session_id) {
            return Err(StoreError::Validation("the session id is not a uuid"));
        }
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(&format!(
            "SELECT {} FROM audit_events WHERE session_id = ?1 ORDER BY rowid ASC",
            audit_columns()
        ))?;
        let rows: Vec<AuditEventRow> = stmt
            .query_map(params![session_id], |row| {
                Ok(AuditEventRow {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    actor: row.get(2)?,
                    action: row.get(3)?,
                    message: row.get(4)?,
                    payload_hash: row.get(5)?,
                    prev_hmac: row.get(6)?,
                    hmac: row.get(7)?,
                    timestamp: row.get(8)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(stmt);

        let mut prev: String = String::new();
        for row in &rows {
            if row.prev_hmac != prev || row.hmac != audit_mac(row, key)? {
                return Err(StoreError::AuditTampered {
                    event_id: row.id.clone(),
                });
            }
            prev = row.hmac.clone();
        }
        Ok(rows.len())
    }
}
