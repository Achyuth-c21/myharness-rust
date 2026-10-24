//! Session metadata, transcripts, restore, archive/delete, retention and legal
//! hold (roadmap Phase 4; `docs/11` §4).
//!
//! Phase 4 slice 1 — the semantic layer over [`myharness_store`]. The store owns
//! rows, blobs, and the HMAC machinery; this crate owns what a *session* means:
//!
//! - **Audit-key lifecycle.** `docs/11` §4.3 records a per-session `.audit-key`
//!   beside `audit.jsonl` and recommends the rebuild keep those keys in the OS
//!   keychain, rotated per session. The key never lives in the database; it is
//!   reached through [`AuditKeyStore`]. The in-memory implementation is the
//!   honest slice-1 surface for tests; the keyring-backed implementation lands
//!   with the secrets work (`TEST-CFG-006` is compile-verified only today), and
//!   the trait is the seam so nothing else changes when it does.
//! - **Chained lifecycle events.** Creating, archiving, holding, and appending
//!   a turn each append one link to the session's audit chain (`TEST-
//!   SESSION-006` / `TEST-DATA-006`), so the chain answers "what happened to
//!   this session" without reading the transcript.
//! - **Delete orchestration.** Deletion removes rows, content blobs, and the
//!   audit key together (`TEST-SESSION-004`). The chain dies with the session,
//!   matching the shipped layout where audit data lives inside the session's
//!   own directory (`docs/11` §4.1); legal hold refuses the operation until
//!   explicitly released (`TEST-DATA-005`).
//!
//! Slice 3 adds the phase-exit semantics on top of the same service:
//!
//! - **Live turns and crash recovery.** An assistant turn is opened as a
//!   `partial` row *before* the provider is contacted; the agent finishes that
//!   row exactly once. A row still live on the next open means the process
//!   died mid-stream, and [`SessionService::recover_interrupted`] marks it
//!   `error` ("interrupted by an app restart") instead of replaying or
//!   duplicating it (`TEST-SESSION-002`; docs/features/chat-and-composer.md:
//!   "unfinished streams are marked interrupted, not replayed").
//! - **Usage aggregation** ([`usage`]) over local-day windows, so 7/30/90-day
//!   views do not shift at midnight (`TEST-SESSION-008`, `TEST-DATA-007`).
//! - **Retention** ([`retention`]) for one surface, honoring both the
//!   per-session legal hold and the profile-level hold switch
//!   (`TEST-DATA-004`, `TEST-SESSION-005`).
//!
//! Drafts (`TEST-SESSION-009`) are renderer-owned in the shipped app
//! (`docs/11` §10.1) and stay renderer-owned in the rebuild; they never cross
//! this crate. Content bytes are never copied into the chain — a link
//! authenticates a serialized summary, while the transcript content lives in
//! the content-addressed store and is referenced by hash.

pub mod retention;
pub mod usage;

pub use retention::{RetentionPolicy, RetentionReport};
pub use usage::{summarize_usage, DailyUsage, UsageSummary, UsageTotals};

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use myharness_store::{
    NewSession, NewUsageEvent, SessionRow, Store, StoreError, TurnRole, TurnRow, TurnStatus,
    UsageEventRow,
};
use rand::{rngs::OsRng, RngCore};

/// Failures at the session-semantic layer.
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error(transparent)]
    Store(#[from] StoreError),
    /// The session's audit key is not available — the chain cannot be verified
    /// or extended. Never synthesized from anything else.
    #[error("the audit key for the session is not available")]
    MissingAuditKey,
    /// The key store (keychain seam) refused the operation.
    #[error("the audit key store failed")]
    AuditKeyStore,
}

/// Where per-session audit keys live. One 32-byte key per session, generated at
/// session creation, never written to the database (`docs/11` §4.3).
pub trait AuditKeyStore {
    fn create(&self, session_id: &str) -> Result<[u8; 32], SessionError>;
    fn get(&self, session_id: &str) -> Result<Option<[u8; 32]>, SessionError>;
    fn remove(&self, session_id: &str) -> Result<(), SessionError>;
}

/// The slice-1 key store: process memory, random keys from the OS.
///
/// Deliberately not persistent — a rebuild of a key from storage would defeat
/// the per-session rotation the captures record. The keychain implementation
/// (wrapping `myharness-secrets`) replaces this type without touching callers.
#[derive(Debug, Default)]
pub struct InMemoryAuditKeys {
    keys: Mutex<HashMap<String, [u8; 32]>>,
}

impl InMemoryAuditKeys {
    pub fn new() -> InMemoryAuditKeys {
        InMemoryAuditKeys::default()
    }

    /// How many session keys this store currently holds (diagnostics/tests).
    pub fn len(&self) -> usize {
        self.keys.lock().expect("key mutex poisoned").len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl AuditKeyStore for InMemoryAuditKeys {
    fn create(&self, session_id: &str) -> Result<[u8; 32], SessionError> {
        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);
        self.keys
            .lock()
            .expect("key mutex poisoned")
            .insert(session_id.to_string(), key);
        Ok(key)
    }

    fn get(&self, session_id: &str) -> Result<Option<[u8; 32]>, SessionError> {
        Ok(self
            .keys
            .lock()
            .expect("key mutex poisoned")
            .get(session_id)
            .copied())
    }

    fn remove(&self, session_id: &str) -> Result<(), SessionError> {
        self.keys
            .lock()
            .expect("key mutex poisoned")
            .remove(session_id);
        Ok(())
    }
}

/// Sharing one key store across service handles is how a test (or a
/// re-opened app service) keeps keys available across a store reopen. The app
/// uses the keyring-backed store; the trait stays the only seam.
impl<T: AuditKeyStore + ?Sized> AuditKeyStore for Arc<T> {
    fn create(&self, session_id: &str) -> Result<[u8; 32], SessionError> {
        (**self).create(session_id)
    }

    fn get(&self, session_id: &str) -> Result<Option<[u8; 32]>, SessionError> {
        (**self).get(session_id)
    }

    fn remove(&self, session_id: &str) -> Result<(), SessionError> {
        (**self).remove(session_id)
    }
}

/// The session service: every session-semantic operation goes through here so
/// the transcript, the chain, and the key lifecycle cannot drift apart.
pub struct SessionService<K: AuditKeyStore> {
    store: Store,
    keys: K,
}

impl<K: AuditKeyStore> SessionService<K> {
    pub fn new(store: Store, keys: K) -> SessionService<K> {
        SessionService { store, keys }
    }

    /// The underlying store — transcript content reads (by blob reference) and
    /// usage queries use it directly.
    pub fn store(&self) -> &Store {
        &self.store
    }

    /// Create a session: row, per-session audit key, genesis chain link. If the
    /// genesis link cannot be written, the just-created row and key are removed
    /// again rather than leaving a session whose chain begins mid-story.
    pub fn start(&self, session: NewSession<'_>) -> Result<SessionRow, SessionError> {
        let row = self.store.create_session(session)?;
        // A session without an audit key could never be verified; roll the
        // row back rather than leave an unauditable session behind.
        let key = match self.keys.create(&row.id) {
            Ok(key) => key,
            Err(error) => {
                let _ = self.store.delete_session(&row.id);
                return Err(error);
            }
        };
        let payload = serde_json::json!({ "event": "session.created" }).to_string();
        if let Err(err) =
            self.store
                .append_audit(&row.id, "core", "session.created", None, payload.as_bytes(), &key)
        {
            let _ = self.store.delete_session(&row.id);
            let _ = self.keys.remove(&row.id);
            return Err(err.into());
        }
        Ok(row)
    }

    /// Append one transcript turn and its chain link. The link authenticates a
    /// structural summary (sequence, role, status, content hash) — never the
    /// content bytes, which live in the content-addressed store.
    #[allow(clippy::too_many_arguments)]
    pub fn append_turn(
        &self,
        session_id: &str,
        seq: i64,
        role: TurnRole,
        status: TurnStatus,
        content: &[u8],
        usage_json: Option<serde_json::Value>,
        error: Option<&str>,
        stop_reason: Option<&str>,
    ) -> Result<TurnRow, SessionError> {
        // The key is resolved before the row: a missing key must not leave a
        // turn that no chain link authenticates.
        let key = self.key(session_id)?;
        let turn = self.store.put_turn(
            session_id,
            seq,
            role,
            status,
            content,
            usage_json,
            error,
            stop_reason,
        )?;
        let payload = serde_json::json!({
            "event": "turn.appended",
            "seq": turn.seq,
            "role": turn.role,
            "status": turn.status,
            "contentRef": turn.content_ref,
            "stopReason": turn.stop_reason,
        })
        .to_string();
        self.store.append_audit(
            session_id,
            "agent",
            "turn.appended",
            None,
            payload.as_bytes(),
            &key,
        )?;
        self.store.touch_session(session_id)?;
        Ok(turn)
    }

    /// Open a live assistant turn: a `partial` row with no error, plus a
    /// `turn.started` chain link. The agent finishes the row exactly once
    /// ([`Self::finish_turn`]); a row left in this state is the durable marker
    /// of an interrupted stream that [`Self::recover_interrupted`] turns into
    /// an explicit error (`TEST-SESSION-002`).
    pub fn begin_assistant_turn(
        &self,
        session_id: &str,
        seq: i64,
        content: &[u8],
    ) -> Result<TurnRow, SessionError> {
        let key = self.key(session_id)?;
        let turn = self.store.put_turn(
            session_id,
            seq,
            TurnRole::Assistant,
            TurnStatus::Partial,
            content,
            None,
            None,
            None,
        )?;
        let payload = serde_json::json!({
            "event": "turn.started",
            "seq": turn.seq,
            "role": turn.role,
            "contentRef": turn.content_ref,
        })
        .to_string();
        self.store.append_audit(
            session_id,
            "agent",
            "turn.started",
            None,
            payload.as_bytes(),
            &key,
        )?;
        self.store.touch_session(session_id)?;
        Ok(turn)
    }

    /// Finish a live assistant turn exactly once and chain the outcome. The
    /// store refuses a second finish, so a retried write cannot rewrite
    /// history; `partial` here is the deliberate cancellation state, with the
    /// error text naming the cancellation.
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
    ) -> Result<TurnRow, SessionError> {
        // Resolve the audit key BEFORE transitioning the row: a missing key
        // must leave the turn live (and therefore recoverable) rather than
        // finish it with no chain link.
        let key = self.key(session_id)?;
        let turn = self.store.finish_turn(
            session_id,
            turn_id,
            status,
            content,
            usage_json,
            error,
            stop_reason,
        )?;
        let payload = serde_json::json!({
            "event": "turn.finished",
            "seq": turn.seq,
            "status": turn.status,
            "contentRef": turn.content_ref,
            "stopReason": turn.stop_reason,
        })
        .to_string();
        self.store.append_audit(
            session_id,
            "agent",
            "turn.finished",
            None,
            payload.as_bytes(),
            &key,
        )?;
        Ok(turn)
    }

    /// Mark every live assistant turn as interrupted. Called before replay on
    /// a send and when a session is opened; idempotent because a finished turn
    /// is no longer live. Delivered-but-unpersisted deltas are not replayed —
    /// the doc records the interruption, never a fabricated reply.
    pub fn recover_interrupted(&self, session_id: &str) -> Result<usize, SessionError> {
        let mut recovered = 0;
        for turn in self.store.list_turns(session_id)? {
            if turn.role != TurnRole::Assistant.as_stored()
                || turn.status != TurnStatus::Partial.as_stored()
                || turn.error.is_some()
            {
                continue;
            }
            let content = self
                .store
                .files
                .read(&turn.content_ref)?
                .ok_or(StoreError::NotFound)?;
            self.finish_turn(
                session_id,
                &turn.id,
                TurnStatus::Error,
                &content,
                None,
                Some("the stream was interrupted by an app restart"),
                None,
            )?;
            recovered += 1;
        }
        Ok(recovered)
    }

    /// Move the title and chain the change. The sidebar is a view; the chain
    /// is the record of what the session was called.
    pub fn rename(&self, session_id: &str, title: &str) -> Result<(), SessionError> {
        self.store.set_title(session_id, title)?;
        let key = self.key(session_id)?;
        let payload = serde_json::json!({ "event": "session.renamed", "title": title }).to_string();
        self.store.append_audit(
            session_id,
            "core",
            "session.renamed",
            None,
            payload.as_bytes(),
            &key,
        )?;
        Ok(())
    }

    /// Flip the archived flag and record which way it went.
    pub fn archive(&self, session_id: &str, archived: bool) -> Result<(), SessionError> {
        self.store.set_archived(session_id, archived)?;
        let key = self.key(session_id)?;
        let action = if archived {
            "session.archived"
        } else {
            "session.unarchived"
        };
        self.store
            .append_audit(session_id, "core", action, None, b"", &key)?;
        Ok(())
    }

    /// Set/release the legal hold; both directions are chain events because the
    /// hold is what makes deletion refuse (`TEST-DATA-005`).
    pub fn set_legal_hold(&self, session_id: &str, hold: bool) -> Result<(), SessionError> {
        self.store.set_legal_hold(session_id, hold)?;
        let key = self.key(session_id)?;
        let action = if hold {
            "session.hold_set"
        } else {
            "session.hold_released"
        };
        self.store
            .append_audit(session_id, "core", action, None, b"", &key)?;
        Ok(())
    }

    /// Delete the session: rows, transcript blobs, and the audit key. The chain
    /// itself is removed with the session (it lives inside the session's data
    /// by design, `docs/11` §4.1); a legal hold refuses the call.
    pub fn delete(&self, session_id: &str) -> Result<Vec<String>, SessionError> {
        let removed = self.store.delete_session(session_id)?;
        self.keys.remove(session_id)?;
        Ok(removed)
    }

    /// Record one usage event verbatim (`TEST-SESSION-007`).
    pub fn record_usage(
        &self,
        session_id: &str,
        event: NewUsageEvent<'_>,
    ) -> Result<UsageEventRow, SessionError> {
        Ok(self.store.record_usage(session_id, event)?)
    }

    /// Verify the whole audit chain; returns the number of links. A missing key
    /// is an explicit error, never a "looks fine".
    pub fn verify(&self, session_id: &str) -> Result<usize, SessionError> {
        let key = self.key(session_id)?;
        Ok(self.store.verify_audit(session_id, &key)?)
    }

    fn key(&self, session_id: &str) -> Result<[u8; 32], SessionError> {
        self.keys
            .get(session_id)?
            .ok_or(SessionError::MissingAuditKey)
    }
}
