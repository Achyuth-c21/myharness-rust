//! Backup and restore (`docs/13` Phase 9 "Crash/restart, migration,
//! backup/restore"): the store-level machinery, verified by tests.
//!
//! - **`Store::backup_into`** produces a consistent snapshot of the live
//!   database with SQLite's `VACUUM INTO` — a single statement the engine
//!   executes under its own lock, so the snapshot is transactionally
//!   consistent however busy the live store is. The target must not exist
//!   (the caller picks a fresh path; nothing is overwritten) and the
//!   snapshot inherits the store's owner-only posture.
//! - **`restore_backup`** is an *offline* operation used before a store
//!   opens: it verifies the candidate (integrity check, schema version —
//!   the same forward-only gate live opens enforce), then swaps it into
//!   place atomically (write to `<target>.restore-<ms>`, fsync, rename) so
//!   a failed restore never leaves a half-written database and never
//!   touches the displaced file's bytes (the previous database is renamed
//!   aside as `<target>.pre-restore-<ms>` — recovery keeps everything,
//!   like every corruption path in this crate).
//!
//! Deliberately NOT here: an in-app "restore now" flow. Restoring while
//! the app holds live connections cannot be done safely under this schema
//! (the file cannot move under an open SQLite connection), so the bridge
//! surface is deferred and recorded (`docs/14` §9) — the machinery is the
//! part that must be right.

use std::path::{Path, PathBuf};

use crate::{secure, StoreError, LATEST_SCHEMA_VERSION};

/// The restore report: what moved, where the previous database went.
#[derive(Debug, Clone, PartialEq)]
pub struct RestoreReport {
    /// The schema version the restored backup carried.
    pub schema_version: i64,
    /// The displaced live database's aside path, when one existed.
    pub previous_database_aside: Option<PathBuf>,
}

/// Create a consistent snapshot of the live store at `path` using SQLite's
/// `VACUUM INTO`. The target must not exist; the snapshot is a plain
/// database file (no WAL sidecars needed — VACUUM INTO writes a clean,
/// self-contained file) with the store's owner-only mode.
pub fn backup_into(live: &rusqlite::Connection, target: &Path) -> Result<(), StoreError> {
    if target.exists() {
        return Err(StoreError::Validation(
            "the backup target already exists; choose a fresh path (nothing is overwritten)",
        ));
    }
    if let Some(parent) = target.parent() {
        secure::dir(parent).map_err(StoreError::Io)?;
    }
    let target_text = target.to_string_lossy().into_owned();
    live.execute("VACUUM INTO ?1", [target_text.as_str()])
        .map_err(|error| {
            // A failed VACUUM INTO may leave a partial file; remove it —
            // the target did not exist before this call, so removing what
            // this call created is not destroying user data.
            let _ = std::fs::remove_file(target);
            StoreError::Sql(error)
        })?;
    secure::file(target)?;
    Ok(())
}

/// The store-facing wrapper: snapshot this store's live connection to a
/// fresh file (the store method the bridge and tests call).
impl crate::Store {
    pub fn backup_into(&self, target: &Path) -> Result<(), StoreError> {
        let guard = self.conn.lock().expect("the store lock");
        backup_into(&guard, target)
    }
}

/// Verify and restore a backup file into `target` (the store's database
/// path), offline — before any connection opens. The candidate must pass
/// SQLite's integrity check and carry a schema version this build supports
/// (the same forward-only gate live opens enforce: a newer schema refuses
/// untouched). The displaced live database, if any, is renamed aside —
/// never deleted.
pub fn restore_backup(backup: &Path, target: &Path) -> Result<RestoreReport, StoreError> {
    // The candidate must be a real database this build can read: integrity
    // and schema version are checked on a read-only connection before any
    // bytes move.
    let candidate = open_verified_candidate(backup)?;
    let schema_version = read_schema_version(&candidate)?;
    if schema_version > LATEST_SCHEMA_VERSION {
        return Err(StoreError::SchemaAhead {
            found: schema_version,
            supported: LATEST_SCHEMA_VERSION,
        });
    }
    drop(candidate);

    if let Some(parent) = target.parent() {
        secure::dir(parent).map_err(StoreError::Io)?;
    }
    let aside = if target.exists() {
        let aside = PathBuf::from(format!(
            "{}.pre-restore-{}",
            target.display(),
            chrono::Utc::now().timestamp_millis()
        ));
        std::fs::rename(target, &aside).map_err(StoreError::Io)?;
        Some(aside)
    } else {
        None
    };
    // Copy through a sibling temp file, fsync, then rename — the rename is
    // atomic on the same filesystem, so the target is either the old bytes
    // or the verified backup, never a partial write.
    let staging = PathBuf::from(format!(
        "{}.restore-{}",
        target.display(),
        chrono::Utc::now().timestamp_millis()
    ));
    let copy = std::fs::copy(backup, &staging);
    if let Err(error) = copy {
        let _ = std::fs::remove_file(&staging);
        return Err(StoreError::Io(error));
    }
    if let Err(error) = secure::file(&staging) {
        let _ = std::fs::remove_file(&staging);
        return Err(StoreError::Io(error));
    }
    if let Err(error) = fsync_file(&staging) {
        let _ = std::fs::remove_file(&staging);
        return Err(StoreError::Io(error));
    }
    std::fs::rename(&staging, target).map_err(StoreError::Io)?;
    Ok(RestoreReport {
        schema_version,
        previous_database_aside: aside,
    })
}

/// Open the candidate read-only and run the full integrity check — the
/// strict one, not the quick one: a restore is the one moment a silent
/// corruption would become permanent.
fn open_verified_candidate(backup: &Path) -> Result<rusqlite::Connection, StoreError> {
    if !backup.is_file() {
        return Err(StoreError::Validation(
            "the backup path is not a file; nothing was restored",
        ));
    }
    let conn =
        rusqlite::Connection::open_with_flags(backup, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|error| {
            let classified = StoreError::Sql(error);
            if crate::is_db_corruption(&classified) {
                StoreError::CorruptDatabase
            } else {
                classified
            }
        })?;
    let check: String = match conn.query_row("PRAGMA integrity_check", [], |row| row.get(0)) {
        Ok(check) => check,
        Err(error) => {
            let classified = StoreError::Sql(error);
            if crate::is_db_corruption(&classified) {
                return Err(StoreError::CorruptDatabase);
            }
            return Err(classified);
        }
    };
    if check != "ok" {
        return Err(StoreError::CorruptDatabase);
    }
    Ok(conn)
}

/// The store's schema version lives in `schema_migrations` (the
/// migrations' own ledger — `crate::migrate`), not in `user_version`.
fn read_schema_version(conn: &rusqlite::Connection) -> Result<i64, StoreError> {
    let present: bool = conn
        .query_row(
            "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'schema_migrations')",
            [],
            |row| row.get(0),
        )
        .map_err(StoreError::Sql)?;
    if !present {
        return Err(StoreError::Validation(
            "the backup carries no schema_migrations table; it is not this app's database",
        ));
    }
    conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |row| row.get(0),
    )
    .map_err(StoreError::Sql)
}

fn fsync_file(path: &Path) -> std::io::Result<()> {
    use std::os::unix::io::AsRawFd;
    let file = std::fs::File::open(path)?;
    file.sync_all()?;
    let _ = file.as_raw_fd();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NewSession, Store};

    fn scratch(name: &str) -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = dir.path().join(name);
        let files = dir.path().join("files");
        (dir, db, files)
    }

    fn session<'a>(title: &'a str) -> NewSession<'a> {
        NewSession {
            surface: "chat",
            title,
            cwd: None,
            model: None,
            effort: None,
            permission_mode: None,
            project_id: None,
            extra: serde_json::json!({ "marker": title }),
        }
    }

    /// A backup of a live store is a self-contained database whose rows
    /// read back after the live one is gone (`docs/13` Phase 9: backup
    /// creation and integrity).
    #[test]
    fn a_backup_snapshots_the_live_rows() {
        let (_dir, db, files) = scratch("store.db");
        let store = Store::open(&db, &files).expect("open");
        let created = store
            .create_session(session("backup marker"))
            .expect("the session");
        let backup_path = db.with_extension("backup");
        backup_into(&store.conn.lock().unwrap(), &backup_path).expect("the backup");
        drop(store);

        // The snapshot opens standalone and carries the row.
        let restored = Store::open(&backup_path, &files).expect("the backup opens");
        let reread = restored.read_session(&created.id).expect("read");
        assert_eq!(reread.title, "backup marker");
        assert_eq!(
            reread.extra["marker"], "backup marker",
            "unknown fields survive the snapshot"
        );
    }

    /// A second backup to the same path refuses — nothing is overwritten.
    #[test]
    fn a_backup_refuses_an_existing_target() {
        let (_dir, db, files) = scratch("store.db");
        let store = Store::open(&db, &files).expect("open");
        let backup_path = db.with_extension("backup");
        backup_into(&store.conn.lock().unwrap(), &backup_path).expect("the first");
        assert!(matches!(
            backup_into(&store.conn.lock().unwrap(), &backup_path),
            Err(StoreError::Validation(_))
        ));
    }

    /// A restore verifies, swaps atomically, and moves the previous
    /// database aside — never deletes it.
    #[test]
    fn a_restore_swaps_atomically_and_keeps_the_previous_bytes() {
        let (_dir, db, files) = scratch("store.db");
        let store = Store::open(&db, &files).expect("open");
        let kept = store
            .create_session(session("live before restore"))
            .expect("the session");
        let backup_path = db.with_extension("backup");
        backup_into(&store.conn.lock().unwrap(), &backup_path).expect("the backup");

        // Mutate the live store after the snapshot; the restore brings the
        // snapshot's rows back.
        let dropped = store
            .create_session(session("written after the backup"))
            .expect("the session");
        drop(store);

        let report = restore_backup(&backup_path, &db).expect("the restore");
        assert_eq!(report.schema_version, LATEST_SCHEMA_VERSION);
        assert!(report.previous_database_aside.is_some());
        assert!(
            report.previous_database_aside.as_ref().unwrap().is_file(),
            "the previous bytes survive"
        );

        let reopened = Store::open(&db, &files).expect("reopens");
        assert!(
            reopened.read_session(&kept.id).is_ok(),
            "the backup's row is back"
        );
        assert!(
            reopened.read_session(&dropped.id).is_err(),
            "the post-backup row is gone"
        );
    }

    /// A corrupt backup refuses with the corruption class; the live
    /// database's bytes are untouched.
    #[test]
    fn a_corrupt_backup_refuses_and_touches_nothing() {
        let (_dir, db, files) = scratch("store.db");
        let store = Store::open(&db, &files).expect("open");
        let backup_path = db.with_extension("backup");
        std::fs::write(&backup_path, b"this is not a database").expect("the junk");
        assert!(
            matches!(
                restore_backup(&backup_path, &db),
                Err(StoreError::CorruptDatabase)
            ),
            "junk is corruption, not a validation miss"
        );
        assert!(db.is_file(), "the live bytes are untouched");
        drop(store);
        // The live store still opens.
        Store::open(&db, &files).expect("the live store opens");
    }

    /// A backup whose schema is ahead refuses untouched (the forward-only
    /// gate live opens enforce applies to restores too).
    #[test]
    fn a_newer_schema_backup_refuses() {
        let (_dir, db, files) = scratch("store.db");
        let store = Store::open(&db, &files).expect("open");
        drop(store);
        // Forge a snapshot whose migration ledger claims one version past
        // the supported constant, via a plain SQLite connection.
        let forged = db.with_extension("forged");
        {
            let conn = rusqlite::Connection::open(&forged).expect("forge");
            conn.execute_batch(&format!(
                "CREATE TABLE schema_migrations (version INTEGER, name TEXT, applied_at INTEGER);
                 INSERT INTO schema_migrations (version, name, applied_at) VALUES ({}, 'forged', 0);",
                LATEST_SCHEMA_VERSION + 1
            ))
            .unwrap();
        }
        assert!(matches!(
            restore_backup(&forged, &db),
            Err(StoreError::SchemaAhead { .. })
        ));
        assert!(db.is_file(), "the live bytes are untouched");
    }
}
