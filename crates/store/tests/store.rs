//! The store slice-1 battery — the `TEST-DATA-*` / `TEST-SESSION-*`
//! persistence-half gates (`crates/store`, roadmap Phase 4 slice 1; `docs/13`
//! Phase 4; `docs/11` §14). Every test runs against a real file-backed store in
//! a temp directory: file handling (perms, corruption, set-aside) is half of
//! what this slice pins, so the in-memory shortcut would prove less.

use std::path::{Path, PathBuf};

use myharness_store::{
    quarantined_records, NewSession, NewUsageEvent, Store, StoreError, TurnRole, TurnStatus,
};
use rusqlite::Connection;
use tempfile::TempDir;

struct TestStore {
    store: Store,
    db_path: PathBuf,
    files_dir: PathBuf,
    #[allow(dead_code)]
    dir: TempDir,
}

impl TestStore {
    fn new() -> TestStore {
        let dir = TempDir::new().expect("temp dir");
        let db_path = dir.path().join("sessions.db");
        let files_dir = dir.path().join("files-root");
        let store = Store::open(&db_path, &files_dir).expect("open store");
        TestStore {
            store,
            db_path,
            files_dir,
            dir,
        }
    }

    fn fresh_session(&self, title: &str) -> myharness_store::repos::SessionRow {
        self.store
            .create_session(NewSession {
                surface: "chat",
                title,
                ..Default::default()
            })
            .expect("create session")
    }
}

// ---------------------------------------------------------------------
// Migrations (`TEST-DATA-001`)
// ---------------------------------------------------------------------

/// Forward-only and idempotent: opening twice changes nothing, and there is no
/// sanctioned write-down path in the crate at all.
#[test]
fn migrations_are_forward_only_and_idempotent() {
    let probe = TestStore::new();
    let version = |p: &Path| {
        let c = Connection::open(p).unwrap();
        c.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap()
    };
    assert_eq!(version(&probe.db_path), myharness_store::LATEST_SCHEMA_VERSION);

    // Reopen: idempotent, same version.
    drop(probe.store);
    let reopened = Store::open(&probe.db_path, &probe.files_dir).expect("reopen");
    assert_eq!(version(&probe.db_path), myharness_store::LATEST_SCHEMA_VERSION);
    drop(reopened);

    // The crate exposes no down path: the supported height is a forward-only
    // constant, and the ledger keeps one row per applied revision.
    assert_eq!(myharness_store::LATEST_SCHEMA_VERSION, 2);
}

/// The `payload` passthrough preserves unknown keys across a close/reopen
/// (`TEST-DATA-001`): a writer newer than this binary may have written fields
/// these captures never saw, and a rebuild must not eat them.
#[test]
fn unknown_payload_keys_survive_reopen() {
    let probe = TestStore::new();
    let created = probe
        .store
        .create_session(NewSession {
            surface: "chat",
            title: "documented fields only",
            extra: serde_json::json!({
                "olderField": {"nested": [1, 2, 3]},
                "author-added": "value"
            }),
            ..Default::default()
        })
        .expect("create");
    drop(probe.store);
    let reopened = Store::open(&probe.db_path, &probe.files_dir).expect("reopen");
    let reread = reopened.read_session(&created.id).expect("reread");
    assert_eq!(
        reread.extra,
        serde_json::json!({
            "olderField": {"nested": [1, 2, 3]},
            "author-added": "value"
        })
    );
}

/// A database written by a newer build refuses to open, untouched
/// (`TEST-DATA-001`'s forward-only discipline taken to its conclusion: no
/// downgrade, no guess).
#[test]
fn refuses_a_database_written_by_a_newer_build() {
    let probe = TestStore::new();
    drop(probe.store);
    let c = Connection::open(&probe.db_path).unwrap();
    c.execute(
        "INSERT INTO schema_migrations (version, name, applied_at) VALUES (99, 'future-stub', 0)",
        [],
    )
    .unwrap();
    drop(c);
    match Store::open(&probe.db_path, &probe.files_dir).map(|_| ()) {
        Err(StoreError::SchemaAhead { found, supported }) => {
            assert_eq!(found, 99);
            assert_eq!(supported, myharness_store::LATEST_SCHEMA_VERSION);
        }
        other => panic!("expected SchemaAhead, got {other:?}"),
    }
}

/// A corrupt database file is set aside (renamed `*.corrupt-*`) and reported —
/// never deleted, and no migration ever "repairs" it (`TEST-DATA-003`).
#[test]
fn a_corrupt_database_is_set_aside_never_deleted() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("sessions.db");
    std::fs::write(&db_path, b"this is not sqlite \xff garbage").unwrap();
    match Store::open(&db_path, &dir.path().join("f")).map(|_| ()) {
        Err(StoreError::CorruptDatabase) => {}
        other => panic!("expected CorruptDatabase, got {other:?}"),
    }
    let set_aside: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains(".corrupt-"))
        .collect();
    assert_eq!(
        set_aside.len(),
        1,
        "the corrupt bytes must survive at set-aside"
    );
}

// ---------------------------------------------------------------------
// Posture
// ---------------------------------------------------------------------

/// WAL must actually be on (the store refuses to run without its assumptions),
/// and foreign-key enforcement must be real, not just a pragma typed somewhere.
#[test]
fn wal_is_on_and_foreign_keys_enforce() {
    let probe = TestStore::new();
    // journal_mode is file-level: a second connection sees it.
    let c = Connection::open(&probe.db_path).unwrap();
    let mode: String = c
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .unwrap();
    assert_eq!(mode.to_ascii_lowercase(), "wal");

    // FK enforcement is connection-local, so the probe enables the same posture
    // the store runs with; an orphan turn must then fail.
    c.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
    let fake_blob = probe.store.files.put(b"orphan-content").unwrap();
    let err = c.execute(
        "INSERT INTO turns (id, session_id, seq, role, status, content_ref, created_at)
         VALUES ('11111111-2222-4333-8444-555555555555', '22222222-3333-4444-8555-666666666666',
                 0, 'user', 'complete', ?1, 0)",
        [fake_blob],
    );
    assert!(err.is_err(), "foreign keys must refuse an orphan turn");
}

#[cfg(unix)]
#[test]
fn storage_files_are_owner_only() {
    use std::os::unix::fs::PermissionsExt;

    let probe = TestStore::new();
    let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&probe.db_path), 0o600);
    assert_eq!(mode(&probe.files_dir), 0o700);

    let hash = probe.store.files.put(b"perm-check").unwrap();
    let blob_path = probe
        .files_dir
        .join("files")
        .join(&hash[..2])
        .join(&hash);
    assert_eq!(mode(&blob_path), 0o600);
}

// ---------------------------------------------------------------------
// Sessions
// ---------------------------------------------------------------------

/// Create/reopen round trip, listing order by activity (`TEST-SESSION-001`'s
/// create/reopen clauses).
#[test]
fn sessions_round_trip_and_order_by_activity() {
    let probe = TestStore::new();
    let first = probe.fresh_session("first");
    std::thread::sleep(std::time::Duration::from_millis(3));
    let second = probe.fresh_session("second");
    // `touch_session` records millisecond resolution; create and touch in the
    // same millisecond ties, and the tie-breaker (`id ASC`) would make this
    // assertion order-dependent rather than activity-dependent.
    std::thread::sleep(std::time::Duration::from_millis(3));
    probe.store.touch_session(&first.id).unwrap();

    let all = probe.store.list_sessions(None).unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].id, first.id, "touched last = listed first");
    assert_eq!(all[1].id, second.id);

    let reread = probe.store.read_session(&first.id).unwrap();
    assert_eq!(reread.id, first.id);
    assert_eq!(reread.surface, "chat");
    assert_eq!(reread.title, "first");
    assert!(!reread.archived && !reread.legal_hold);
}

/// Archiving flips exactly the flag and the unarchived listing stops showing
/// the row (`TEST-SESSION-001`'s archive clause).
#[test]
fn archive_flips_only_the_flag() {
    let probe = TestStore::new();
    let created = probe.fresh_session("to-archive");
    let before = probe.store.read_session(&created.id).unwrap();
    probe.store.set_archived(&created.id, true).unwrap();
    let after = probe.store.read_session(&created.id).unwrap();
    assert!(after.archived);
    assert_eq!(after.title, before.title);
    assert_eq!(after.created_at, before.created_at);
    assert_eq!(after.extra, before.extra);

    assert_eq!(probe.store.list_sessions(Some(false)).unwrap().len(), 0);
    assert_eq!(probe.store.list_sessions(Some(true)).unwrap().len(), 1);
    probe.store.set_archived(&created.id, false).unwrap();
    assert_eq!(probe.store.list_sessions(Some(false)).unwrap().len(), 1);
}

/// Rename refuses a blank title (a session name is a required field in the
/// shipped form; the store is not the place to invent a default).
#[test]
fn rename_refuses_a_blank_title() {
    let probe = TestStore::new();
    let created = probe.fresh_session("named");
    assert!(matches!(
        probe.store.set_title(&created.id, "   "),
        Err(StoreError::Validation(_))
    ));
    probe.store.set_title(&created.id, "renamed").unwrap();
    assert_eq!(
        probe.store.read_session(&created.id).unwrap().title,
        "renamed"
    );
}

/// Every value is data: an injection-shaped title stores and reads back as
/// text, and the table stays intact (parameterized-statements posture).
#[test]
fn an_injection_shaped_title_stores_as_data() {
    let probe = TestStore::new();
    let payload = "'); DROP TABLE sessions; --";
    probe
        .store
        .create_session(NewSession {
            surface: "chat",
            title: payload,
            ..Default::default()
        })
        .unwrap();
    let listed = probe.store.list_sessions(None).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].title, payload);
    assert!(probe.store.read_session(&listed[0].id).is_ok());
}

/// Legal hold: deletion refuses while the hold stands, the hold survives a
/// restart, and it releases only explicitly (`TEST-DATA-005`).
#[test]
fn legal_hold_survives_restart_and_blocks_delete_until_released() {
    let probe = TestStore::new();
    let created = probe.fresh_session("held");
    probe.store.set_legal_hold(&created.id, true).unwrap();

    drop(probe.store);
    let reopened = Store::open(&probe.db_path, &probe.files_dir).expect("reopen");
    assert!(matches!(
        reopened.delete_session(&created.id),
        Err(StoreError::LegalHold)
    ));
    assert!(reopened.read_session(&created.id).unwrap().legal_hold);

    reopened.set_legal_hold(&created.id, false).unwrap();
    reopened.delete_session(&created.id).unwrap();
    assert!(matches!(
        reopened.read_session(&created.id),
        Err(StoreError::NotFound)
    ));
}

/// Delete removes the session, its turns, their blobs — and nothing of its
/// neighbor's (`TEST-SESSION-004`'s metadata+content clause; upload
/// directories are Phase-5 artifact scope).
#[test]
fn delete_removes_rows_and_content_blobs_of_one_session() {
    let probe = TestStore::new();
    let gone = probe.fresh_session("doomed");
    let neighbor = probe.fresh_session("neighbor");

    let turn = probe
        .store
        .put_turn(
            &gone.id,
            0,
            TurnRole::User,
            TurnStatus::Complete,
            b"content of the doomed session",
            None,
            None,
            None,
        )
        .unwrap();
    let neighbor_turn = probe
        .store
        .put_turn(
            &neighbor.id,
            0,
            TurnRole::User,
            TurnStatus::Complete,
            b"neighbor content",
            None,
            None,
            None,
        )
        .unwrap();

    let removed = probe.store.delete_session(&gone.id).unwrap();
    assert_eq!(removed, vec![turn.content_ref.clone()]);
    assert!(probe.store.files.read(&turn.content_ref).unwrap().is_none());
    assert!(probe
        .store
        .files
        .read(&neighbor_turn.content_ref)
        .unwrap()
        .is_some());

    assert!(matches!(
        probe.store.read_session(&gone.id),
        Err(StoreError::NotFound)
    ));
    assert_eq!(probe.store.list_turns(&neighbor.id).unwrap().len(), 1);
    assert_eq!(probe.store.list_sessions(None).unwrap().len(), 1);
}

/// A turn sequence is a slot, not an overwrite target: double-appending is a
/// caller mistake and is refused.
#[test]
fn a_turn_sequence_is_a_slot_not_an_overwrite() {
    let probe = TestStore::new();
    let created = probe.fresh_session("slots");
    probe
        .store
        .put_turn(
            &created.id,
            0,
            TurnRole::User,
            TurnStatus::Complete,
            b"one",
            None,
            None,
            None,
        )
        .unwrap();
    let second = probe.store.put_turn(
        &created.id,
        0,
        TurnRole::User,
        TurnStatus::Complete,
        b"two",
        None,
        None,
        None,
    );
    assert!(matches!(second, Err(StoreError::Validation(_))));
    assert_eq!(probe.store.list_turns(&created.id).unwrap().len(), 1);
}

/// A turn's native stop reason rides the row across a restart (schema v2) —
/// it is not collapsed at the agent boundary (`docs/12` §4.1;
/// `AI-ADAPTER-08`).
#[test]
fn a_turn_keeps_its_native_stop_reason_across_reopen() {
    let probe = TestStore::new();
    let created = probe.fresh_session("stop-reasons");
    probe
        .store
        .put_turn(
            &created.id,
            0,
            TurnRole::Assistant,
            TurnStatus::Complete,
            b"a truncated reply",
            None,
            None,
            Some("max_tokens"),
        )
        .unwrap();
    drop(probe.store);

    let reopened = Store::open(&probe.db_path, &probe.files_dir).expect("reopen");
    let turns = reopened.list_turns(&created.id).unwrap();
    assert_eq!(turns[0].stop_reason.as_deref(), Some("max_tokens"));
}

/// A live turn (partial, no error) finishes exactly once; a finished row or a
/// cancelled partial is refused rather than rewritten.
#[test]
fn a_live_turn_finishes_once() {
    let probe = TestStore::new();
    let created = probe.fresh_session("live-turn");
    let live = probe
        .store
        .put_turn(
            &created.id,
            0,
            TurnRole::Assistant,
            TurnStatus::Partial,
            b"{}",
            None,
            None,
            None,
        )
        .unwrap();

    let done = probe
        .store
        .finish_turn(
            &created.id,
            &live.id,
            TurnStatus::Complete,
            b"final",
            Some(serde_json::json!({ "input": 1 })),
            None,
            Some("end_turn"),
        )
        .unwrap();
    assert_eq!(done.id, live.id);
    assert_eq!(done.status, "complete");
    assert_eq!(done.stop_reason.as_deref(), Some("end_turn"));
    assert_eq!(
        probe.store.files.read(&done.content_ref).unwrap().as_deref(),
        Some(&b"final"[..])
    );

    let again = probe
        .store
        .finish_turn(&created.id, &live.id, TurnStatus::Complete, b"x", None, None, None);
    assert!(matches!(again, Err(StoreError::Validation(_))));

    let cancelled = probe
        .store
        .put_turn(
            &created.id,
            1,
            TurnRole::Assistant,
            TurnStatus::Partial,
            b"{}",
            None,
            Some("the turn was cancelled"),
            None,
        )
        .unwrap();
    let rewrite = probe.store.finish_turn(
        &created.id,
        &cancelled.id,
        TurnStatus::Error,
        b"x",
        None,
        None,
        None,
    );
    assert!(matches!(rewrite, Err(StoreError::Validation(_))));
}

/// Content is addressed by hash, so two sessions can share one blob; deleting
/// one session must not remove content the other still references
/// (`TEST-SESSION-004`).
#[test]
fn deleting_a_session_keeps_blobs_its_neighbor_still_references() {
    let probe = TestStore::new();
    let doomed = probe.fresh_session("shared-doomed");
    let keeper = probe.fresh_session("shared-keeper");
    let shared = b"identical turn content";
    let first = probe
        .store
        .put_turn(
            &doomed.id,
            0,
            TurnRole::User,
            TurnStatus::Complete,
            shared,
            None,
            None,
            None,
        )
        .unwrap();
    let second = probe
        .store
        .put_turn(
            &keeper.id,
            0,
            TurnRole::User,
            TurnStatus::Complete,
            shared,
            None,
            None,
            None,
        )
        .unwrap();
    assert_eq!(first.content_ref, second.content_ref);

    let removed = probe.store.delete_session(&doomed.id).unwrap();
    assert!(removed.is_empty(), "a shared blob must not be deleted");
    assert_eq!(
        probe.store.files.read(&second.content_ref).unwrap().as_deref(),
        Some(&shared[..])
    );
}

/// A turn written against a session that does not exist is refused.
#[test]
fn a_turn_needs_its_session() {
    let probe = TestStore::new();
    let err = probe.store.put_turn(
        "33333333-4444-4555-8666-777777777777",
        0,
        TurnRole::User,
        TurnStatus::Complete,
        b"orphan",
        None,
        None,
        None,
    );
    assert!(matches!(err, Err(StoreError::NotFound)));
}


/// The ledger can be read as a closed window, device-wide or per session
/// (`TEST-SESSION-008` / `TEST-DATA-007` aggregation input).
#[test]
fn usage_events_are_selectable_in_a_window() {
    let probe = TestStore::new();
    let first = probe.fresh_session("usage-window-a");
    let second = probe.fresh_session("usage-window-b");
    for (session, input) in [(&first.id, 3), (&second.id, 5)] {
        probe
            .store
            .record_usage(
                session,
                NewUsageEvent {
                    model: "model-a",
                    input_tokens: input,
                    output_tokens: 1,
                    ..Default::default()
                },
            )
            .unwrap();
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;

    let all = probe.store.list_usage_between(0, now + 1_000, None).unwrap();
    assert_eq!(all.len(), 2);
    let one = probe
        .store
        .list_usage_between(0, now + 1_000, Some(&second.id))
        .unwrap();
    assert_eq!(one.len(), 1);
    assert_eq!(one[0].input_tokens, 5);
    assert!(probe
        .store
        .list_usage_between(now + 1_000, now + 2_000, None)
        .unwrap()
        .is_empty());
    assert!(matches!(
        probe.store.list_usage_between(10, 5, None),
        Err(StoreError::Validation(_))
    ));
}

// ---------------------------------------------------------------------
// Quarantine (`TEST-SESSION-003` / `TEST-DATA-003`)
// ---------------------------------------------------------------------

/// A transcript row that fails validation is moved to the quarantine with a
/// reason, removed from its table, reported by the failing read — and the rest
/// of the transcript survives the retry.
#[test]
fn a_corrupt_turn_row_is_quarantined_never_deleted() {
    let probe = TestStore::new();
    let created = probe.fresh_session("corrupt-neighborhood");
    let good = probe
        .store
        .put_turn(
            &created.id,
            0,
            TurnRole::User,
            TurnStatus::Complete,
            b"good",
            None,
            None,
            None,
        )
        .unwrap();
    // Forge a corrupt row directly: a content reference that is not a SHA-256
    // digest (the column's CHECK constraints cannot express that shape).
    let conn = Connection::open(&probe.db_path).unwrap();
    conn.execute(
        "INSERT INTO turns (id, session_id, seq, role, status, content_ref, created_at)
         VALUES ('11111111-2222-4333-8444-555555555555', ?1, 1, 'user', 'complete', ?2, 0)",
        rusqlite::params![created.id, "z".repeat(64)],
    )
    .unwrap();
    drop(conn);

    assert!(matches!(
        probe.store.list_turns(&created.id),
        Err(StoreError::QuarantinedRecord {
            source_table: "turns",
            ..
        })
    ));

    // The corrupt row lived its whole remaining life in the quarantine.
    let quarantined = quarantined_records(&probe.store).unwrap();
    assert_eq!(quarantined.len(), 1);
    assert_eq!(quarantined[0].source_table, "turns");
    assert!(quarantined[0].reason.contains("content_ref"));

    // The valid transcript survives the surgery.
    let turns = probe.store.list_turns(&created.id).unwrap();
    assert_eq!(turns.len(), 1);
    assert_eq!(turns[0].id, good.id);
}

/// A session row that fails validation is quarantined the same way: the direct
/// read names it, the listing retry shows the survivors, and nothing was
/// silently deleted (`TEST-SESSION-003`).
#[test]
fn a_corrupt_session_row_is_quarantined_never_deleted() {
    let probe = TestStore::new();
    let created = probe.fresh_session("victim");
    let conn = Connection::open(&probe.db_path).unwrap();
    conn.execute(
        "UPDATE sessions SET surface = '' WHERE id = ?1",
        rusqlite::params![created.id],
    )
    .unwrap();
    drop(conn);

    assert!(matches!(
        probe.store.read_session(&created.id),
        Err(StoreError::QuarantinedRecord {
            source_table: "sessions",
            ..
        })
    ));
    assert_eq!(quarantined_records(&probe.store).unwrap().len(), 1);
    // The row moved out of `sessions` entirely — into the quarantine, whose
    // payload still holds it.
    assert!(matches!(
        probe.store.read_session(&created.id),
        Err(StoreError::NotFound)
    ));
    assert!(probe.store.list_sessions(None).unwrap().is_empty());
}

// ---------------------------------------------------------------------
// Usage (`TEST-SESSION-007`)
// ---------------------------------------------------------------------

/// Usage events preserve every count and the error channel verbatim — the
/// aggregation slices' truth (`TEST-DATA-007`'s preservation half).
#[test]
fn usage_events_preserve_every_count_and_the_error() {
    let probe = TestStore::new();
    let created = probe.fresh_session("usage");
    probe
        .store
        .record_usage(
            &created.id,
            NewUsageEvent {
                model: "claude-opus-4-1",
                input_tokens: 3,
                output_tokens: 1,
                cache_read_tokens: 2,
                cache_write_tokens: 0,
                search_requests: 1,
                error: Some("transient provider error"),
            },
        )
        .unwrap();
    let events = probe.store.list_usage(&created.id).unwrap();
    assert_eq!(events.len(), 1);
    let event = &events[0];
    assert_eq!(event.model, "claude-opus-4-1");
    assert_eq!(event.input_tokens, 3);
    assert_eq!(event.output_tokens, 1);
    assert_eq!(event.cache_read_tokens, 2);
    assert_eq!(event.cache_write_tokens, 0);
    assert_eq!(event.search_requests, 1);
    assert_eq!(event.error.as_deref(), Some("transient provider error"));
}

// ---------------------------------------------------------------------
// Audit chain (`TEST-SESSION-006` / `TEST-DATA-006`)
// ---------------------------------------------------------------------

/// The chain verifies when untouched and detects tampering at the exact event.
#[test]
fn the_audit_chain_verifies_and_detects_tampering() {
    let probe = TestStore::new();
    let created = probe.fresh_session("audited");
    let key = [7u8; 32];
    for action in ["session.created", "turn.appended", "session.archived"] {
        probe
            .store
            .append_audit(
                &created.id,
                "core",
                action,
                Some("fixture-note"),
                b"fixture-payload",
                &key,
            )
            .unwrap();
    }
    assert_eq!(probe.store.verify_audit(&created.id, &key).unwrap(), 3);

    // Tamper with the middle event's action: the chain breaks there.
    let conn = Connection::open(&probe.db_path).unwrap();
    conn.execute(
        "UPDATE audit_events SET action = 'forged' WHERE action = 'turn.appended'",
        [],
    )
    .unwrap();
    drop(conn);
    match probe.store.verify_audit(&created.id, &key) {
        Err(StoreError::AuditTampered { event_id }) => {
            assert_ne!(event_id, "");
        }
        other => panic!("expected AuditTampered, got {other:?}"),
    }
}

/// A wrong key verifies nothing: the key lives outside the store and belongs to
/// the session.
#[test]
fn a_wrong_audit_key_breaks_the_chain() {
    let probe = TestStore::new();
    let created = probe.fresh_session("wrong-key");
    probe
        .store
        .append_audit(&created.id, "core", "session.created", None, b"p", &[1u8; 32])
        .unwrap();
    let wrong = [2u8; 32];
    assert!(matches!(
        probe.store.verify_audit(&created.id, &wrong),
        Err(StoreError::AuditTampered { .. })
    ));
}

// ---------------------------------------------------------------------
// Blobs
// ---------------------------------------------------------------------

/// Content addressing: identical bytes hash identically, reads round-trip,
/// deletes are idempotent, and a traversal-shaped "hash" is refused at the
/// path-confinement check (`docs/11` §4.4).
#[test]
fn blobs_are_content_addressed_and_confining() {
    let probe = TestStore::new();
    let hash_a = probe.store.files.put(b"same").unwrap();
    let hash_b = probe.store.files.put(b"same").unwrap();
    assert_eq!(hash_a, hash_b);
    assert_eq!(probe.store.files.read(&hash_a).unwrap().unwrap(), b"same");
    assert!(probe.store.files.exists(&hash_a).unwrap());
    probe.store.files.delete(&hash_a).unwrap();
    probe.store.files.delete(&hash_a).unwrap();
    assert!(probe.store.files.read(&hash_a).unwrap().is_none());

    assert!(matches!(
        probe.store.files.read("../evil"),
        Err(StoreError::Validation(_))
    ));
    assert!(matches!(
        probe.store.files.read("uppercase-not-hex"),
        Err(StoreError::Validation(_))
    ));
}
