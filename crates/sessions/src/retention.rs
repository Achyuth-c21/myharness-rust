//! Retention for one session surface (`TEST-DATA-004`, `TEST-SESSION-005`).
//!
//! The Limits tab records per-surface retention days and a legal-hold switch
//! (`crates/config/src/tabs.rs`: `chatSessionRetentionDays`,
//! `coworkSessionRetentionDays`, `codeSessionRetentionDays`,
//! `sessionRetentionHold`). The sweep here is deliberately narrow:
//!
//! - one surface at a time, so chat retention can never touch Cowork or Code;
//! - only sessions whose **last activity** is older than the cutoff;
//! - a per-session legal hold or the profile-level hold switch keeps the
//!   session and reports it (`held`), never a silent skip;
//! - projects, memory, and uncommitted work are not sessions and are not
//!   reachable from this module.
//!
//! There is no background timer in this phase: the caller decides when to
//! sweep. That is honest — the shipped schedule is unobserved — while the
//! destructive behavior itself is implemented and tested.

use myharness_store::StoreError;
use serde::Serialize;

use crate::{AuditKeyStore, SessionError, SessionService};

const DAY_MS: i64 = 86_400_000;

/// What to delete and what must survive.
#[derive(Debug, Clone)]
pub struct RetentionPolicy<'a> {
    /// `chat`, `cowork`, `code`, … — exactly the stored surface string.
    pub surface: &'a str,
    /// Sessions idle for at least this many days are expired.
    pub older_than_days: u32,
    /// The profile-level `sessionRetentionHold` switch: true suspends every
    /// deletion, regardless of the per-session flag.
    pub hold: bool,
    /// The sweep's clock, supplied by the caller.
    pub now_ms: i64,
}

/// What the sweep did, with every skipped expired session named.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RetentionReport {
    pub surface: String,
    pub cutoff_ms: i64,
    pub deleted: Vec<String>,
    /// Expired sessions kept by a hold (per-session or profile-level).
    pub held: Vec<String>,
    /// Sessions of this surface younger than the cutoff.
    pub kept: usize,
}

impl<K: AuditKeyStore> SessionService<K> {
    /// Delete expired sessions of one surface. Deletion is the normal
    /// [`SessionService::delete`] path: rows, transcript blobs, and the audit
    /// key go together; a held session raises no error and is not touched.
    pub fn sweep_retention(
        &self,
        policy: &RetentionPolicy<'_>,
    ) -> Result<RetentionReport, SessionError> {
        if policy.surface.trim().is_empty() {
            return Err(StoreError::Validation("retention needs a surface").into());
        }
        if policy.older_than_days == 0 {
            return Err(StoreError::Validation("retention needs at least one day").into());
        }
        let cutoff_ms = policy
            .now_ms
            .saturating_sub(i64::from(policy.older_than_days).saturating_mul(DAY_MS));

        let mut report = RetentionReport {
            surface: policy.surface.to_string(),
            cutoff_ms,
            deleted: Vec::new(),
            held: Vec::new(),
            kept: 0,
        };
        for session in self.store.list_sessions(None)? {
            if session.surface != policy.surface {
                continue;
            }
            if session.last_activity_at >= cutoff_ms {
                report.kept += 1;
                continue;
            }
            if policy.hold || session.legal_hold {
                report.held.push(session.id);
                continue;
            }
            self.delete(&session.id)?;
            report.deleted.push(session.id);
        }
        Ok(report)
    }
}
