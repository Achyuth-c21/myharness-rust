//! The quarantine table and its reader (`TEST-DATA-003` / `TEST-SESSION-003`).
//!
//! A record that fails validation on read is **moved** here — never deleted,
//! never "fixed" in place. The quarantine is the visible record that something
//! was wrong; recovery (re-import, manual repair, retention) is a caller
//! decision, not a store one.

use rusqlite::{params, Connection};

use crate::{now_ms, StoreError};

/// Move a row's raw payload into the quarantine. `payload` is the row's
/// serialized representation (JSON object of its columns); idempotent on
/// (source_table, source_id) so a repeated failed read does not stack rows.
pub(crate) fn quarantine(
    conn: &Connection,
    source_table: &'static str,
    source_id: &str,
    reason: &str,
    payload: &str,
) -> Result<(), StoreError> {
    conn.execute(
        "INSERT INTO quarantined_records (source_table, source_id, reason, payload, quarantined_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT (source_table, source_id) DO UPDATE SET
             reason = excluded.reason,
             quarantined_at = excluded.quarantined_at",
        params![
            source_table,
            source_id,
            reason,
            payload,
            now_ms()
        ],
    )?;
    Ok(())
}

/// Delete the surviving (validated) copy of a row that was just quarantined.
/// Callers that quarantine-then-fail the read call this so the corrupt row
/// exists exactly once — in the quarantine, not in its table.
pub(crate) fn remove(
    conn: &Connection,
    source_table: &'static str,
    source_id: &str,
) -> Result<(), StoreError> {
    conn.execute(
        &format!(
            // Interpolation is safe: the table name is one of this crate's own
            // constants (the whitelist posture), the id is a bound parameter.
            "DELETE FROM {source_table} WHERE id = ?1"
        ),
        params![source_id],
    )?;
    Ok(())
}

#[derive(Debug, serde::Serialize)]
pub struct QuarantinedRecord {
    pub source_table: String,
    pub source_id: String,
    pub reason: String,
    pub payload: String,
    #[serde(rename = "quarantinedAt")]
    pub quarantined_at: i64,
}

/// Everything currently quarantined, oldest first — the diagnostic read.
pub fn quarantined_records(
    store: &crate::Store,
) -> Result<Vec<QuarantinedRecord>, StoreError> {
    let conn = store.conn.lock().expect("store mutex poisoned");
    let mut stmt = conn.prepare(
        "SELECT source_table, source_id, reason, payload, quarantined_at
         FROM quarantined_records
         ORDER BY quarantined_at ASC",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(QuarantinedRecord {
                source_table: row.get(0)?,
                source_id: row.get(1)?,
                reason: row.get(2)?,
                payload: row.get(3)?,
                quarantined_at: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}
