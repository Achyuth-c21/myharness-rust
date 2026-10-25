//! Forward-only, idempotent migrations (`TEST-DATA-001`).
//!
//! Every schema change is one [`Migration`]: an ordered, transactional `up` in
//! SQL recorded in `schema_migrations`. There is no `down`: the rebuild model
//! has no sanctioned rollback path — a rollback would be an unobserved claim
//! about data loss behavior (`docs/14` §9 discipline), and idempotency is
//! instead proven by re-running ([`run`] is a no-op when already applied). A
//! database written by a *newer* build is refused outright ([`StoreError::
//! SchemaAhead`]): never down-migrate, never guess.

use rusqlite::Connection;

use crate::{now_ms, StoreError};

/// One schema revision. `sql` runs inside a transaction together with the
/// version bookkeeping row.
#[derive(Debug)]
pub struct Migration {
    pub version: i64,
    pub name: &'static str,
    pub sql: &'static str,
}

/// Revision 1 — the rebuild model's slice-1 tables (`docs/11` §14). The
/// `sessions.payload` column is the safe unknowns passthrough: every documented
/// session field the docs capture lives as a typed column, and every field the
/// captures *did not* document — and everything newer writers might add —
/// survives a round trip through `payload` (`TEST-DATA-001`).
pub const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    name: "slice1_core_tables",
    sql: r#"
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    surface TEXT NOT NULL,
    title TEXT NOT NULL,
    cwd TEXT,
    model TEXT,
    effort TEXT,
    permission_mode TEXT,
    archived INTEGER NOT NULL DEFAULT 0,
    legal_hold INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    last_activity_at INTEGER NOT NULL,
    payload TEXT NOT NULL
);

CREATE INDEX idx_sessions_activity ON sessions(last_activity_at DESC);

CREATE TABLE turns (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    seq INTEGER NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('user', 'assistant', 'system', 'tool')),
    status TEXT NOT NULL CHECK (status IN ('complete', 'partial', 'error')),
    content_ref TEXT NOT NULL,
    usage_json TEXT,
    error TEXT,
    created_at INTEGER NOT NULL,
    UNIQUE (session_id, seq)
);

CREATE TABLE usage_events (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    model TEXT NOT NULL,
    input_tokens INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0,
    cache_read_tokens INTEGER NOT NULL DEFAULT 0,
    cache_write_tokens INTEGER NOT NULL DEFAULT 0,
    search_requests INTEGER NOT NULL DEFAULT 0,
    error TEXT,
    timestamp INTEGER NOT NULL
);

CREATE INDEX idx_usage_events_session ON usage_events(session_id, timestamp);

CREATE TABLE audit_events (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    actor TEXT NOT NULL,
    action TEXT NOT NULL,
    message TEXT,
    payload_hash TEXT NOT NULL,
    prev_hmac TEXT NOT NULL,
    hmac TEXT NOT NULL,
    timestamp INTEGER NOT NULL
);

CREATE INDEX idx_audit_events_session ON audit_events(session_id, timestamp);

CREATE TABLE quarantined_records (
    id INTEGER PRIMARY KEY,
    source_table TEXT NOT NULL,
    source_id TEXT NOT NULL,
    reason TEXT NOT NULL,
    payload TEXT NOT NULL,
    quarantined_at INTEGER NOT NULL,
    UNIQUE (source_table, source_id)
);
"#,
},
    // Revision 2 (Phase 4 slice 2): the native stop reason belongs to the turn.
    // `docs/12` §4.1 requires preserving native stop reasons rather than
    // collapsing them to "done"; without a column they died at the agent
    // boundary. Stored as the provider's own spelling (`NativeStopReason::wire`).
    Migration {
        version: 2,
        name: "turns_stop_reason",
        sql: "ALTER TABLE turns ADD COLUMN stop_reason TEXT;",
    },
    // Revision 3 (Phase 6 slice 2): projects. The shipped schema was never
    // observed (`docs/features/projects.md`, `docs/11`); this is the rebuild
    // model. `sessions.project_id` is a **soft** association on purpose — no
    // foreign key — because deleting a project must not delete or retarget its
    // sessions (`REQ-PROJ-06`); a session keeps its own `cwd` and stays open.
    Migration {
        version: 3,
        name: "phase6_projects",
        sql: r#"
CREATE TABLE projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    instructions TEXT,
    cwd TEXT NOT NULL,
    memory_enabled INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    last_activity_at INTEGER NOT NULL,
    payload TEXT NOT NULL
);

CREATE INDEX idx_projects_activity ON projects(last_activity_at DESC);

ALTER TABLE sessions ADD COLUMN project_id TEXT;

CREATE INDEX idx_sessions_project ON sessions(project_id);
"#,
    },
];

/// The newest schema revision this binary understands. Kept as a flat constant
/// (const-eval cannot iterate the table); the unit test below pins it to the
/// last migration so the two can never drift apart.
pub const LATEST_SCHEMA_VERSION: i64 = 3;

/// Apply every missing migration inside one transaction each, recording the
/// version row together with the schema it belongs to. Idempotent: an already
/// applied revision is skipped without touching it.
pub fn run(conn: &mut Connection) -> Result<(), StoreError> {
    // The bookkeeping table exists before the first read; creating it is its
    // own idempotent step, outside any revision's transaction.
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            applied_at INTEGER NOT NULL
        );",
    )?;

    let current: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |row| row.get(0),
    )?;

    if current > LATEST_SCHEMA_VERSION {
        return Err(StoreError::SchemaAhead {
            found: current,
            supported: LATEST_SCHEMA_VERSION,
        });
    }

    for migration in MIGRATIONS {
        if migration.version <= current {
            continue;
        }
        let tx = conn.transaction()?;
        tx.execute_batch(migration.sql)?;
        tx.execute(
            "INSERT INTO schema_migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![migration.version, migration.name, now_ms()],
        )?;
        tx.commit()?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The constant, the table's last revision, and ascending order must agree.
    #[test]
    fn the_supported_version_is_the_last_migration() {
        assert_eq!(
            Some(&LATEST_SCHEMA_VERSION),
            MIGRATIONS.last().map(|m| &m.version)
        );
        let mut previous = 0;
        for migration in MIGRATIONS {
            assert!(migration.version > previous, "migrations must ascend");
            previous = migration.version;
        }
    }
}
