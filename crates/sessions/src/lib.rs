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
//! Not in this slice: send/stream/stop (the agent loop, slice 2), retention
//! (`TEST-DATA-004`), draft recovery (`TEST-SESSION-009`), and the renderer
//! surface (slice 3). Content bytes are never copied into the chain — a link
//! authenticates a serialized summary, while the transcript content lives in
//! the content-addressed store and is referenced by hash.

use std::collections::HashMap;
use std::sync::Mutex;

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
        let key = self.keys.create(&row.id)?;
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
    ) -> Result<TurnRow, SessionError> {
        let turn = self
            .store
            .put_turn(session_id, seq, role, status, content, usage_json, error)?;
        let key = self.key(session_id)?;
        let payload = serde_json::json!({
            "event": "turn.appended",
            "seq": turn.seq,
            "role": turn.role,
            "status": turn.status,
            "contentRef": turn.content_ref,
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
