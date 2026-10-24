//! The `sessions` slice-1 battery — the semantic half of `TEST-SESSION-001`
//! (create/archive/delete), the chain gates `TEST-SESSION-006` /
//! `TEST-DATA-006`, and the legal-hold gate `TEST-DATA-005`. The store crate's
//! own battery pins the SQL/file mechanics; these tests pin the service's
//! orchestration: row + key + chain travel together.

use std::path::PathBuf;

use myharness_sessions::{AuditKeyStore, InMemoryAuditKeys, SessionError, SessionService};
use myharness_store::{NewSession, Store, StoreError, TurnRole, TurnStatus};
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
