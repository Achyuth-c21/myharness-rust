//! The `sessions` battery — the semantic half of `TEST-SESSION-001`
//! (create/archive/delete), the chain gates `TEST-SESSION-006` /
//! `TEST-DATA-006`, the legal-hold gate `TEST-DATA-005`, live-turn crash
//! recovery (`TEST-SESSION-002`), local-day usage aggregation
//! (`TEST-SESSION-008` / `TEST-DATA-007`), and retention
//! (`TEST-DATA-004` / `TEST-SESSION-005`). The store crate's own battery pins
//! the SQL/file mechanics; these tests pin the service's orchestration:
//! row + key + chain travel together.

use std::path::PathBuf;
use std::sync::Arc;

use chrono::{FixedOffset, TimeZone};
use myharness_sessions::{
    summarize_usage, AuditKeyStore, InMemoryAuditKeys, RetentionPolicy, SessionError,
    SessionService,
};
use myharness_store::{
    NewSession, NewUsageEvent, Store, StoreError, TurnRole, TurnStatus, UsageEventRow,
};
use rusqlite::Connection;
use tempfile::TempDir;

struct Fixture {
    #[allow(dead_code)]
    dir: TempDir,
    db_path: PathBuf,
    files_dir: PathBuf,
    service: SessionService<InMemoryAuditKeys>,
}

impl Fixture {
    fn new() -> Fixture {
        let dir = TempDir::new().expect("temp dir");
        let db_path = dir.path().join("sessions.db");
        let files_dir = dir.path().join("files-root");
        let store = Store::open(&db_path, &files_dir).expect("open store");
        Fixture {
            dir,
            db_path,
            files_dir,
            service: SessionService::new(store, InMemoryAuditKeys::new()),
        }
    }

    fn start(&self, title: &str) -> myharness_store::SessionRow {
        self.service
            .start(NewSession {
                project_id: None,
                surface: "chat",
                title,
                ..Default::default()
            })
            .expect("start session")
    }

    /// A second service over the same files with a fresh (empty) key store —
    /// the "keys are not in the database" proof.
    fn service_with_fresh_keys(&self) -> SessionService<InMemoryAuditKeys> {
        let store = Store::open(&self.db_path, &self.files_dir).expect("reopen store");
        SessionService::new(store, InMemoryAuditKeys::new())
    }
}

// ---------------------------------------------------------------------
// Lifecycle (`TEST-SESSION-001` persistence half)
// ---------------------------------------------------------------------

/// Starting a session writes the row, mints a per-session key, and opens the
/// chain with exactly one link.
#[test]
fn start_writes_the_row_the_key_and_the_genesis_link() {
    let fixture = Fixture::new();
    let started = fixture.start("first light");

    let reread = fixture
        .service
        .store()
        .read_session(&started.id)
        .expect("reread");
    assert_eq!(reread.title, "first light");
    assert_eq!(reread.surface, "chat");
    assert_eq!(fixture.service.verify(&started.id).unwrap(), 1);
    assert!(fixture
        .service
        .store()
        .list_turns(&started.id)
        .unwrap()
        .is_empty());
}

/// Each turn extends the transcript and the chain, and bumps activity.
#[test]
fn a_turn_extends_the_chain_and_touches_activity() {
    let fixture = Fixture::new();
    let started = fixture.start("turns");
    let before = fixture
        .service
        .store()
        .read_session(&started.id)
        .unwrap()
        .last_activity_at;
    std::thread::sleep(std::time::Duration::from_millis(3));

    fixture
        .service
        .append_turn(
            &started.id,
            0,
            TurnRole::User,
            TurnStatus::Complete,
            b"hello",
            None,
            None,
            None,
        )
        .unwrap();
    fixture
        .service
        .append_turn(
            &started.id,
            1,
            TurnRole::Assistant,
            TurnStatus::Partial,
            b"partial reply",
            Some(serde_json::json!({ "inputTokens": 4, "outputTokens": 2 })),
            Some("cancelled by the user"),
            None,
        )
        .unwrap();

    assert_eq!(fixture.service.verify(&started.id).unwrap(), 3);
    let turns = fixture.service.store().list_turns(&started.id).unwrap();
    assert_eq!(turns.len(), 2);
    assert_eq!(turns[1].status, "partial");
    // The partial turn's content is still readable by reference.
    let content = fixture
        .service
        .store()
        .files
        .read(&turns[1].content_ref)
        .unwrap();
    assert_eq!(content.as_deref(), Some(&b"partial reply"[..]));

    let after = fixture
        .service
        .store()
        .read_session(&started.id)
        .unwrap()
        .last_activity_at;
    assert!(after > before, "a turn must bump last activity");
}

/// A refused duplicate sequence leaves the chain exactly as long as it was.
#[test]
fn a_refused_turn_does_not_grow_the_chain() {
    let fixture = Fixture::new();
    let started = fixture.start("slots");
    fixture
        .service
        .append_turn(
            &started.id,
            0,
            TurnRole::User,
            TurnStatus::Complete,
            b"one",
            None,
            None,
            None,
        )
        .unwrap();
    let refused = fixture.service.append_turn(
        &started.id,
        0,
        TurnRole::User,
        TurnStatus::Complete,
        b"two",
        None,
        None,
        None,
    );
    assert!(matches!(refused, Err(SessionError::Store(_))));
    assert_eq!(fixture.service.verify(&started.id).unwrap(), 2);
    assert_eq!(
        fixture.service.store().list_turns(&started.id).unwrap().len(),
        1
    );
}

/// Archiving and holds are chain events; the hold refuses deletion until it is
/// released, and deletion takes rows, blobs, and the key with it.
#[test]
fn archive_and_hold_chain_and_delete_honors_the_hold() {
    let fixture = Fixture::new();
    let started = fixture.start("lifecycle");
    fixture
        .service
        .append_turn(
            &started.id,
            0,
            TurnRole::User,
            TurnStatus::Complete,
            b"content to sweep",
            None,
            None,
            None,
        )
        .unwrap();

    fixture.service.archive(&started.id, true).unwrap();
    assert_eq!(fixture.service.verify(&started.id).unwrap(), 3);
    assert!(fixture
        .service
        .store()
        .read_session(&started.id)
        .unwrap()
        .archived);

    fixture.service.set_legal_hold(&started.id, true).unwrap();
    assert!(matches!(
        fixture.service.delete(&started.id),
        Err(SessionError::Store(StoreError::LegalHold))
    ));
    assert_eq!(fixture.service.verify(&started.id).unwrap(), 4);

    fixture.service.set_legal_hold(&started.id, false).unwrap();
    assert_eq!(fixture.service.verify(&started.id).unwrap(), 5);

    let removed = fixture.service.delete(&started.id).unwrap();
    assert_eq!(removed.len(), 1);
    assert!(matches!(
        fixture.service.store().read_session(&started.id),
        Err(StoreError::NotFound)
    ));
    // The key went with the session; the chain is unreachable, not "verified".
    assert!(matches!(
        fixture.service.verify(&started.id),
        Err(SessionError::MissingAuditKey)
    ));
    assert_eq!(fixture.service.store().list_turns(&started.id).unwrap().len(), 0);
}

// ---------------------------------------------------------------------
// Key lifecycle (`TEST-DATA-006`)
// ---------------------------------------------------------------------

/// Keys live outside the database: a fresh key store can read the rows but
/// cannot verify (or extend) the chain. That failure is explicit.
#[test]
fn keys_are_not_in_the_database() {
    let fixture = Fixture::new();
    let started = fixture.start("keyless reopening");
    let other = fixture.service_with_fresh_keys();
    assert!(matches!(
        other.verify(&started.id),
        Err(SessionError::MissingAuditKey)
    ));
    // The row itself is fine — only the chain operations are refused.
    assert_eq!(
        other.store().read_session(&started.id).unwrap().title,
        "keyless reopening"
    );
}

/// Tampering detected on one session never implicates another: keys are
/// per session.
#[test]
fn one_tampered_chain_does_not_touch_its_neighbor() {
    let fixture = Fixture::new();
    let a = fixture.start("alpha");
    let b = fixture.start("beta");

    let conn = Connection::open(&fixture.db_path).unwrap();
    conn.execute(
        "UPDATE audit_events SET action = 'forged' WHERE session_id = ?1",
        rusqlite::params![a.id],
    )
    .unwrap();
    drop(conn);

    assert!(matches!(
        fixture.service.verify(&a.id),
        Err(SessionError::Store(StoreError::AuditTampered { .. }))
    ));
    assert_eq!(fixture.service.verify(&b.id).unwrap(), 1);
}

/// Deleting a session removes exactly its key: verification then fails on the
/// key before it ever reaches the (now absent) chain.
#[test]
fn delete_removes_the_key() {
    let fixture = Fixture::new();
    let started = fixture.start("ephemeral");
    fixture.service.delete(&started.id).unwrap();
    assert!(matches!(
        fixture.service.verify(&started.id),
        Err(SessionError::MissingAuditKey)
    ));
    assert_eq!(
        fixture.service.store().list_sessions(None).unwrap().len(),
        0
    );
}

/// The in-memory key store itself: create/get/remove semantics.
#[test]
fn the_in_memory_key_store_round_trips() {
    let keys = InMemoryAuditKeys::new();
    let id = "11111111-2222-4333-8444-555555555555";
    let created = keys.create(id).unwrap();
    assert_eq!(keys.get(id).unwrap(), Some(created));
    assert_eq!(keys.len(), 1);
    keys.remove(id).unwrap();
    assert_eq!(keys.get(id).unwrap(), None);
    assert!(keys.is_empty());
}


// ---------------------------------------------------------------------
// Live turns and crash recovery (`TEST-SESSION-002`)
// ---------------------------------------------------------------------

/// Opening a live turn and finishing it is a one-way transition; a leftover
/// live row is recovered as an explicit interruption, idempotently.
#[test]
fn a_live_turn_finishes_once_and_recovery_marks_the_interruption() {
    let fixture = Fixture::new();
    let started = fixture.start("live");
    fixture
        .service
        .append_turn(
            &started.id,
            0,
            TurnRole::User,
            TurnStatus::Complete,
            b"{\"parts\":[{\"type\":\"text\",\"text\":\"hi\"}]}",
            None,
            None,
            None,
        )
        .unwrap();
    let live = fixture
        .service
        .begin_assistant_turn(&started.id, 1, b"{\"parts\":[]}")
        .unwrap();
    assert_eq!(live.status, "partial");
    assert_eq!(live.error, None);
    assert_eq!(fixture.service.verify(&started.id).unwrap(), 3);

    assert_eq!(
        fixture
            .service
            .recover_interrupted(&started.id)
            .expect("recover"),
        1
    );
    let turns = fixture
        .service
        .store()
        .list_turns(&started.id)
        .expect("turns");
    assert_eq!(turns.len(), 2);
    assert_eq!(turns[1].id, live.id);
    assert_eq!(turns[1].status, "error");
    assert_eq!(
        turns[1].error.as_deref(),
        Some("the stream was interrupted by an app restart")
    );
    assert_eq!(fixture.service.verify(&started.id).unwrap(), 4);

    // Idempotent: the recovered row is no longer live.
    assert_eq!(
        fixture
            .service
            .recover_interrupted(&started.id)
            .expect("recover again"),
        0
    );
    assert_eq!(fixture.service.verify(&started.id).unwrap(), 4);

    // A finished live turn is terminal: the store refuses a second finish.
    let next = fixture
        .service
        .begin_assistant_turn(&started.id, 2, b"{\"parts\":[]}")
        .unwrap();
    fixture
        .service
        .finish_turn(
            &started.id,
            &next.id,
            TurnStatus::Complete,
            b"{\"parts\":[{\"type\":\"text\",\"text\":\"done\"}]}",
            None,
            None,
            Some("end_turn"),
        )
        .unwrap();
    assert!(matches!(
        fixture.service.finish_turn(
            &started.id,
            &next.id,
            TurnStatus::Error,
            b"{}",
            None,
            None,
            None,
        ),
        Err(SessionError::Store(StoreError::Validation(_)))
    ));

    // A cancellation-finished partial carries an error and is not a crash
    // marker.
    let cancelled = fixture
        .service
        .append_turn(
            &started.id,
            3,
            TurnRole::Assistant,
            TurnStatus::Partial,
            b"{\"parts\":[]}",
            None,
            Some("the turn was cancelled"),
            None,
        )
        .unwrap();
    assert_eq!(
        fixture
            .service
            .recover_interrupted(&started.id)
            .expect("recover"),
        0
    );
    assert_eq!(
        fixture
            .service
            .store()
            .list_turns(&started.id)
            .expect("turns")
            .last()
            .map(|turn| turn.id.clone()),
        Some(cancelled.id)
    );
}

/// Recovery across a real store reopen: the live row is durable, and a shared
/// key store stands in for the app's keyring across the process boundary.
#[test]
fn recovery_works_across_a_store_reopen() {
    let dir = TempDir::new().expect("temp dir");
    let db = dir.path().join("sessions.db");
    let files = dir.path().join("files-root");
    let keys = Arc::new(InMemoryAuditKeys::new());
    let session;
    {
        let store = Store::open(&db, &files).expect("open");
        let service = SessionService::new(store, Arc::clone(&keys));
        session = service
            .start(NewSession {
                project_id: None,
                surface: "chat",
                title: "crash",
                ..Default::default()
            })
            .expect("start")
            .id;
        service
            .append_turn(
                &session,
                0,
                TurnRole::User,
                TurnStatus::Complete,
                b"{}",
                None,
                None,
                None,
            )
            .unwrap();
        service
            .begin_assistant_turn(&session, 1, b"{\"parts\":[]}")
            .unwrap();
    }
    let store = Store::open(&db, &files).expect("reopen");
    let service = SessionService::new(store, Arc::clone(&keys));
    assert_eq!(service.recover_interrupted(&session).expect("recover"), 1);
    let turns = service.store().list_turns(&session).expect("turns");
    assert_eq!(turns.len(), 2);
    assert_eq!(turns[1].status, "error");
    assert_eq!(service.verify(&session).expect("verify"), 4);
}

// ---------------------------------------------------------------------
// Usage aggregation (`TEST-SESSION-008`, `TEST-DATA-007`)
// ---------------------------------------------------------------------

fn usage_row(timestamp: i64, input: i64, output: i64, error: Option<&str>) -> UsageEventRow {
    UsageEventRow {
        id: format!("event-{timestamp}"),
        session_id: "session".into(),
        model: "model".into(),
        input_tokens: input,
        output_tokens: output,
        cache_read_tokens: 2,
        cache_write_tokens: 1,
        search_requests: 0,
        error: error.map(str::to_string),
        timestamp,
    }
}

/// The same UTC instant lands on different local dates in different zones;
/// 7/30/90 views must follow the device's day, not UTC.
#[test]
fn usage_buckets_follow_the_local_day() {
    let now = utc_2026_10_24_noon();
    let late_utc = utc_2026_10_23_1800(); // 23:30 on the 23rd at +05:30
    let early_utc = utc_2026_10_23_2000(); // 01:30 on the 24th at +05:30
    let events = vec![
        usage_row(late_utc, 3, 1, None),
        usage_row(early_utc, 5, 2, Some("provider said no")),
    ];

    let plus = FixedOffset::east_opt(5 * 3600 + 1800).expect("offset");
    let summary = summarize_usage(&events, 2, plus, now).expect("summary");
    assert_eq!(summary.timezone_offset_minutes, 330);
    assert_eq!(summary.daily.len(), 2);
    assert_eq!(summary.daily[0].date, "2026-10-23");
    assert_eq!(summary.daily[0].totals.events, 1);
    assert_eq!(summary.daily[0].totals.input_tokens, 3);
    assert_eq!(summary.daily[1].date, "2026-10-24");
    assert_eq!(summary.daily[1].totals.events, 1);
    assert_eq!(summary.daily[1].totals.input_tokens, 5);
    assert_eq!(summary.totals.events, 2);
    assert_eq!(summary.totals.error_events, 1);
    assert_eq!(summary.totals.input_tokens, 8);
    assert_eq!(summary.totals.output_tokens, 3);
    assert_eq!(summary.totals.cache_read_tokens, 4);
    assert_eq!(summary.totals.cache_write_tokens, 2);

    // At UTC-08:00 the "early UTC" event is still 2026-10-23 12:00 local.
    let west = FixedOffset::west_opt(8 * 3600).expect("offset");
    let summary = summarize_usage(&events, 2, west, now).expect("summary");
    assert_eq!(summary.timezone_offset_minutes, -480);
    assert_eq!(summary.daily[0].date, "2026-10-23");
    assert_eq!(summary.daily[0].totals.events, 2);
    assert_eq!(summary.daily[1].date, "2026-10-24");
    assert_eq!(summary.daily[1].totals.events, 0);
}

/// The window is closed and local: an event before the oldest local midnight
/// is excluded even when it falls on a date the bucket list contains.
#[test]
fn usage_excludes_events_before_the_local_window_start() {
    let now = utc_2026_10_24_noon();
    let plus = FixedOffset::east_opt(5 * 3600 + 1800).expect("offset");
    // 2026-10-22 18:00 UTC = 2026-10-22 23:30 local, before the 23rd's start.
    let before = utc_2026_10_22_1800();
    let summary = summarize_usage(&[usage_row(before, 9, 9, None)], 2, plus, now)
        .expect("summary");
    assert_eq!(summary.totals.events, 0);
    assert_eq!(summary.daily.len(), 2);
    assert!(summary.daily.iter().all(|day| day.totals.events == 0));
}

/// The service reads the ledger window from the store, device-wide or for one
/// session, and keeps the documented 7/30/90 bounds.
#[test]
fn usage_summary_reads_the_ledger_window() {
    let fixture = Fixture::new();
    let first = fixture.start("usage-a");
    let second = fixture.start("usage-b");
    fixture
        .service
        .record_usage(
            &first.id,
            NewUsageEvent {
                model: "model-a",
                input_tokens: 7,
                output_tokens: 3,
                cache_read_tokens: 2,
                ..Default::default()
            },
        )
        .unwrap();
    fixture
        .service
        .record_usage(
            &second.id,
            NewUsageEvent {
                model: "model-a",
                input_tokens: 11,
                output_tokens: 5,
                ..Default::default()
            },
        )
        .unwrap();

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_millis() as i64;
    let utc = FixedOffset::east_opt(0).expect("offset");
    let all = fixture
        .service
        .usage_summary(None, 7, utc, now)
        .expect("device summary");
    assert_eq!(all.days, 7);
    assert_eq!(all.daily.len(), 7);
    assert_eq!(all.totals.events, 2);
    assert_eq!(all.totals.input_tokens, 18);
    assert_eq!(all.totals.output_tokens, 8);

    let one = fixture
        .service
        .usage_summary(Some(&second.id), 30, utc, now)
        .expect("session summary");
    assert_eq!(one.totals.events, 1);
    assert_eq!(one.totals.input_tokens, 11);

    assert!(fixture.service.usage_summary(None, 0, utc, now).is_err());
    assert!(fixture.service.usage_summary(None, 367, utc, now).is_err());
}

/// The Phase-7 split view (`docs/screens/settings.md` §Usage): the combined
/// window plus per-surface shares whose totals fold back into the combined
/// one, and a surface with no rows is a zero summary, never a missing field.
#[test]
fn usage_summary_split_reports_each_surface() {
    let fixture = Fixture::new();
    let chat = fixture.start("split-chat");
    let code = fixture
        .service
        .start(NewSession {
            project_id: None,
            surface: "code",
            title: "split-code",
            ..Default::default()
        })
        .expect("code session");
    fixture
        .service
        .record_usage(
            &chat.id,
            NewUsageEvent {
                model: "model-a",
                input_tokens: 7,
                output_tokens: 3,
                cache_read_tokens: 2,
                ..Default::default()
            },
        )
        .unwrap();
    fixture
        .service
        .record_usage(
            &code.id,
            NewUsageEvent {
                model: "model-a",
                input_tokens: 11,
                output_tokens: 5,
                search_requests: 2,
                ..Default::default()
            },
        )
        .unwrap();

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_millis() as i64;
    let utc = FixedOffset::east_opt(0).expect("offset");
    let split = fixture
        .service
        .usage_summary_split(7, utc, now)
        .expect("split summary");
    assert_eq!(split.combined.totals.events, 2);
    assert_eq!(split.combined.totals.input_tokens, 18);
    assert_eq!(split.combined.totals.output_tokens, 8);
    assert_eq!(split.surfaces.len(), 2);

    let chat_share = split
        .surfaces
        .iter()
        .find(|share| share.surface == "chat")
        .expect("chat share");
    assert_eq!(chat_share.totals.events, 1);
    assert_eq!(chat_share.totals.input_tokens, 7);
    assert_eq!(chat_share.totals.cache_read_tokens, 2);
    assert_eq!(chat_share.totals.search_requests, 0);

    let code_share = split
        .surfaces
        .iter()
        .find(|share| share.surface == "code")
        .expect("code share");
    assert_eq!(code_share.totals.events, 1);
    assert_eq!(code_share.totals.input_tokens, 11);
    assert_eq!(code_share.totals.search_requests, 2);

    // Every share folds back into the combined totals.
    let sum: i64 = split.surfaces.iter().map(|share| share.totals.events).sum();
    assert_eq!(sum, split.combined.totals.events);

    // A ledger with no rows at all still reports both surfaces as zeros.
    let empty = Fixture::new();
    let split = empty
        .service
        .usage_summary_split(30, utc, now)
        .expect("empty split");
    assert_eq!(split.combined.totals.events, 0);
    assert_eq!(split.surfaces.len(), 2);
    assert!(split.surfaces.iter().all(|share| share.totals.events == 0));
}

fn utc_2026_10_24_noon() -> i64 {
    FixedOffset::east_opt(0)
        .expect("offset")
        .with_ymd_and_hms(2026, 10, 24, 12, 0, 0)
        .single()
        .expect("valid time")
        .timestamp_millis()
}

fn utc_2026_10_23_1800() -> i64 {
    FixedOffset::east_opt(0)
        .expect("offset")
        .with_ymd_and_hms(2026, 10, 23, 18, 0, 0)
        .single()
        .expect("valid time")
        .timestamp_millis()
}

fn utc_2026_10_23_2000() -> i64 {
    FixedOffset::east_opt(0)
        .expect("offset")
        .with_ymd_and_hms(2026, 10, 23, 20, 0, 0)
        .single()
        .expect("valid time")
        .timestamp_millis()
}

fn utc_2026_10_22_1800() -> i64 {
    FixedOffset::east_opt(0)
        .expect("offset")
        .with_ymd_and_hms(2026, 10, 22, 18, 0, 0)
        .single()
        .expect("valid time")
        .timestamp_millis()
}

// ---------------------------------------------------------------------
// Retention (`TEST-DATA-004`, `TEST-SESSION-005`)
// ---------------------------------------------------------------------

/// The sweep deletes only expired sessions of the named surface, honors both
/// holds, and takes rows, blobs, and keys together.
#[test]
fn retention_sweeps_one_surface_and_honors_holds() {
    let fixture = Fixture::new();
    let expired = fixture.start("expired chat");
    let held = fixture.start("held chat");
    let cowork = fixture
        .service
        .start(NewSession {
            project_id: None,
            surface: "cowork",
            title: "cowork stays",
            ..Default::default()
        })
        .expect("cowork");
    let young = fixture.start("young chat");
    let turn = fixture
        .service
        .append_turn(
            &expired.id,
            0,
            TurnRole::User,
            TurnStatus::Complete,
            b"content that must go",
            None,
            None,
            None,
        )
        .unwrap();
    fixture.service.set_legal_hold(&held.id, true).unwrap();

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_millis() as i64;

    // A sweep at the present time leaves everything (30 days).
    let report = fixture
        .service
        .sweep_retention(&RetentionPolicy {
            surface: "chat",
            older_than_days: 30,
            hold: false,
            now_ms: now,
        })
        .expect("recent sweep");
    assert!(report.deleted.is_empty());
    assert_eq!(report.kept, 3);
    assert_eq!(report.held.len(), 0);

    // Forty days later the 30-day cutoff has passed every chat's last
    // activity; the hold keeps one.
    let future = now + 40 * 86_400_000;
    let report = fixture
        .service
        .sweep_retention(&RetentionPolicy {
            surface: "chat",
            older_than_days: 30,
            hold: false,
            now_ms: future,
        })
        .expect("sweep");
    let mut deleted = report.deleted.clone();
    deleted.sort();
    let mut expected = vec![expired.id.clone(), young.id.clone()];
    expected.sort();
    assert_eq!(deleted, expected);
    assert_eq!(report.held, vec![held.id.clone()]);
    assert_eq!(report.kept, 0);
    assert_eq!(report.cutoff_ms, future - 30 * 86_400_000);
    assert!(report.surface == "chat");

    // Rows, blob, and key of the deleted session are gone.
    assert!(matches!(
        fixture.service.store().read_session(&expired.id),
        Err(StoreError::NotFound)
    ));
    assert_eq!(
        fixture
            .service
            .store()
            .files
            .read(&turn.content_ref)
            .expect("read"),
        None
    );
    assert!(matches!(
        fixture.service.verify(&expired.id),
        Err(SessionError::MissingAuditKey)
    ));

    // The held session and the other surface survive with intact chains; the
    // young chat is old at this clock and was swept too.
    assert!(fixture.service.store().read_session(&held.id).is_ok());
    assert!(fixture.service.store().read_session(&cowork.id).is_ok());
    assert!(matches!(
        fixture.service.store().read_session(&young.id),
        Err(StoreError::NotFound)
    ));
    assert_eq!(fixture.service.verify(&held.id).unwrap(), 2);
    assert_eq!(fixture.service.verify(&cowork.id).unwrap(), 1);
}

/// The profile-level hold suspends every deletion even without per-session
/// holds; a different surface is never touched; zero days is refused.
#[test]
fn retention_global_hold_and_validation() {
    let fixture = Fixture::new();
    let chat = fixture.start("global hold");
    let future = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_millis() as i64
        + 10 * 86_400_000;

    let report = fixture
        .service
        .sweep_retention(&RetentionPolicy {
            surface: "chat",
            older_than_days: 1,
            hold: true,
            now_ms: future,
        })
        .expect("held sweep");
    assert!(report.deleted.is_empty());
    assert_eq!(report.held, vec![chat.id.clone()]);
    assert_eq!(fixture.service.verify(&chat.id).unwrap(), 1);

    let report = fixture
        .service
        .sweep_retention(&RetentionPolicy {
            surface: "code",
            older_than_days: 1,
            hold: false,
            now_ms: future,
        })
        .expect("other surface");
    assert!(report.deleted.is_empty());
    assert_eq!(report.kept, 0);

    assert!(fixture
        .service
        .sweep_retention(&RetentionPolicy {
            surface: "chat",
            older_than_days: 0,
            hold: false,
            now_ms: future,
        })
        .is_err());
}
