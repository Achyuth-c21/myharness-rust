//! SQLite WAL storage layer: migrations, repos, content-addressed files (`docs/11` §14).
//!
//! Phase 4 slice 1 — the persistence core. The crate owns the database file, its
//! migrations, the normalized tables the rebuild model defines, and the
//! content-addressed blob store that holds large transcript content by reference
//! (`docs/11` §14: "Store large content ... as content-addressed files with
//! references"). Session *semantics* — typed metadata, audit-key policy, delete
//! orchestration — live in `crates/sessions`; this crate stays storage-generic
//! (`docs/10` §9).
//!
//! Governing rows for this slice: `TEST-DATA-001` (migrations forward-only,
//! idempotent, unknown fields preserved), `TEST-DATA-002` (multi-row writes are
//! transactional), `TEST-DATA-003` / `TEST-SESSION-003` (a corrupt record is
//! quarantined with an error, never silently deleted), `TEST-SESSION-004`
//! (delete removes metadata and content), `TEST-DATA-005` / `TEST-SESSION-006`
//! (legal hold blocks deletion until released; audit HMAC detects tampering).
//!
//! Deliberate non-goals (later slices): the retention sweep (`TEST-DATA-004`),
//! backup/export (`TEST-DATA-008`), the live agent-loop write path
//! (`TEST-SESSION-001`'s send/stream/stop clauses), FTS search, and concurrency
//! beyond one desktop core.
//!
//! Log hygiene: no payload, message, or credential value ever appears in an error
//! from this crate — identifiers and variants only (`docs/09` §4; `docs/18` §10).

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::Connection;

pub mod backup;
pub mod blobs;
pub mod migrate;
pub mod quarantine;
pub mod repos;
pub mod repos_projects;
pub mod repos_scheduler;

pub use blobs::BlobStore;
pub use migrate::LATEST_SCHEMA_VERSION;
pub use quarantine::quarantined_records;
pub use repos::{
    AuditEventRow, CodeTurnActivity, NewSession, NewUsageEvent, SessionRow, TurnRole, TurnRow,
    TurnStatus, UsageEventRow,
};
pub use repos_projects::{NewProject, ProjectRow};
pub use repos_scheduler::{NewScheduledTask, ScheduledRunRow, ScheduledTaskRow};

pub(crate) mod secure {
    //! Owner-only file modes (`TEST-CFG-106` posture carried into storage). The
    //! WAL/SHM sidecars that may not exist yet at open time are also swept.
    use std::path::Path;

    pub fn file(path: &Path) -> std::io::Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let perms = std::fs::Permissions::from_mode(0o600);
            std::fs::set_permissions(path, perms)?;
        }
        #[cfg(not(unix))]
        let _ = path;
        Ok(())
    }

    pub fn dir(path: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let perms = std::fs::Permissions::from_mode(0o700);
            std::fs::set_permissions(path, perms)?;
        }
        #[cfg(not(unix))]
        let _ = path;
        Ok(())
    }
}

/// Storage errors. Variants carry identifiers, never payload or message content
/// (`docs/09` §4).
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// The database file failed SQLite's integrity check on open. The file is
    /// renamed aside as `*.corrupt-<epoch-ms>` (`TEST-DATA-003`: put aside,
    /// never deleted) and the caller decides about recovery.
    #[error("the database failed its integrity check and was set aside; existing sessions are untouched")]
    CorruptDatabase,
    /// The database's schema version is ahead of the binary (a newer writer
    /// opened it). The open refuses without mutating anything — a silent
    /// downgrade would be a lie about the data.
    #[error("the database was written by a newer build (schema {found} > supported {supported}); it is left untouched")]
    SchemaAhead { found: i64, supported: i64 },
    /// A stored row failed validation while being read. The row is moved to
    /// `quarantined_records` (never deleted); the failed read names it.
    #[error("a stored record failed validation and was quarantined (id {source_id})")]
    QuarantinedRecord {
        source_table: &'static str,
        source_id: String,
    },
    /// The session is under legal hold (`TEST-DATA-005`): deletion is refused
    /// until the hold is explicitly released.
    #[error("the session is under legal hold and cannot be deleted")]
    LegalHold,
    #[error("the requested record does not exist")]
    NotFound,
    /// The audit chain failed verification at the named event
    /// (`TEST-SESSION-006` / `TEST-DATA-006`). The chain is left exactly as it
    /// was found — tamper evidence is evidence, not a cleanup trigger.
    #[error("the audit chain failed verification at event {event_id}")]
    AuditTampered { event_id: String },
    /// A caller passed a value the row shape refuses (uuid/hash shape, role or
    /// status spelling), or a blob hash that would escape its directory.
    #[error("{0}")]
    Validation(&'static str),
    #[error("the underlying database failed: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("the underlying filesystem failed: {0}")]
    Io(#[from] std::io::Error),
}

/// One epoch-millisecond wall-clock notch — every table timestamps in UTC.
pub(crate) fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// The SQLite connection, mutex-guarded (single desktop core, sync API).
pub(crate) type Conn = Mutex<Connection>;

/// True when `s` is a lowercase SHA-256 hex digest — the blob-ref shape.
pub(crate) fn is_blob_ref(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// True when `s` parses as a UUID — the id shape every table keys on.
pub(crate) fn is_uuid(s: &str) -> bool {
    uuid::Uuid::parse_str(s).is_ok()
}

/// The storage layer: one mutex-guarded connection plus the content-addressed
/// file store rooted beside it.
pub struct Store {
    pub(crate) conn: Conn,
    pub files: BlobStore,
    db_path: PathBuf,
}

impl Store {
    /// Open (creating or migrating as needed) the database at `path`, with the
    /// content-addressed files beneath `files_dir`.
    pub fn open(path: &Path, files_dir: &Path) -> Result<Store, StoreError> {
        if let Some(parent) = path.parent() {
            secure::dir(parent)?;
        }
        secure::dir(files_dir)?;
        if path.exists() {
            secure::file(path)?;
        }

        // Preparation touches the file before any mutation. A database-
        // corruption signal here (a file that is not SQLite, a failed quick
        // check) puts the bytes aside and reports — never a silent delete,
        // never an in-place "repair" (`TEST-DATA-003`).
        let mut conn = match prepare_connection(path) {
            Ok(conn) => conn,
            Err(err) if is_db_corruption(&err) => {
                set_aside_corrupt(path)?;
                return Err(StoreError::CorruptDatabase);
            }
            Err(err) => return Err(err),
        };

        migrate::run(&mut conn)?;

        // The migration transaction created the file and its sidecars; pin the
        // owner-only posture on what now exists (`TEST-CFG-106` carried into
        // storage).
        secure::file(path)?;
        let base = path.to_string_lossy().into_owned();
        for suffix in ["-wal", "-shm"] {
            let _ = secure::file(Path::new(&format!("{base}{suffix}")));
        }

        Ok(Store {
            conn: Mutex::new(conn),
            files: BlobStore::new(files_dir.join("files"))?,
            db_path: path.to_path_buf(),
        })
    }

    /// The database path — support-bundle and health plumbing.
    pub fn path(&self) -> &Path {
        &self.db_path
    }
}

/// Open the file and put it into the store's operating posture: WAL, foreign
/// keys, NORMAL sync, a busy timeout, and an integrity check **before** any
/// migration writes (`TEST-DATA-003`).
fn prepare_connection(path: &Path) -> Result<Connection, StoreError> {
    let conn = Connection::open(path)?;

    // The desktop posture (`docs/11` §14 + the SQLite skill's WAL blocking
    // notes): WAL for concurrent reads, NORMAL sync, foreign keys ON because
    // the schema relies on them, a busy timeout because the agent loop
    // (slice 2) writes from a streaming context. Set WAL first, then verify it
    // took — a filesystem that refuses WAL must not silently fall back.
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.busy_timeout(std::time::Duration::from_millis(5000))?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    let journal: String = conn.pragma_query_value(None, "journal_mode", |row| row.get(0))?;
    if !journal.eq_ignore_ascii_case("wal") {
        // WAL refused means the filesystem cannot support it; the store must
        // not silently fall back to a journal mode whose semantics its callers
        // assume (`TEST-DATA-002`'s atomicity posture).
        return Err(StoreError::Validation(
            "the database location refused WAL mode",
        ));
    }

    let check: String = conn.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
    if check != "ok" {
        return Err(StoreError::CorruptDatabase);
    }
    Ok(conn)
}

/// True when the error says the *file* is not a healthy SQLite database (as
/// opposed to an I/O or permission failure, which must stay distinguishable in
/// the error surface).
pub(crate) fn is_db_corruption(err: &StoreError) -> bool {
    match err {
        StoreError::CorruptDatabase => true,
        StoreError::Sql(rusqlite::Error::SqliteFailure(e, _)) => {
            e.code == rusqlite::ErrorCode::DatabaseCorrupt
                || e.extended_code == rusqlite::ffi::SQLITE_NOTADB
        }
        _ => false,
    }
}

/// Rename a corrupt database aside as `<name>.corrupt-<epoch-ms>` — the bytes
/// survive for recovery; nothing is deleted (`TEST-DATA-003`).
fn set_aside_corrupt(path: &Path) -> Result<(), StoreError> {
    let aside = PathBuf::from(format!(
        "{}.corrupt-{}",
        path.display(),
        chrono::Utc::now().timestamp_millis()
    ));
    std::fs::rename(path, &aside)?;
    Ok(())
}
