//! Workspace trust store — the bridge's record of which workspace folders the
//! user has approved for Code sessions.
//!
//! UI-SCREEN-32 (`docs/screens/trust-workspace.md`), FEAT-CODE-13, REQ-CODE-09,
//! IMP-41, TEST-CODE-009. The observed original gates folder-executing Code
//! sessions behind "Trust this workspace?" — Cancel aborts, Trust records the
//! decision and proceeds. The rebuild keeps the same semantics on the
//! privileged side of the boundary (`docs/10` §8; `ARCH-PROC-02/03/05`):
//!
//! - **Trust decisions are stored, never inferred.** An absent record is
//!   untrusted; there is no heuristic that makes a folder trusted.
//! - The store holds only canonical folder paths and the decision time — no
//!   session content, no secrets.
//! - The file is one app-managed JSON document (the `mcp/approvals.json`
//!   pattern: a durable security-decision record), written atomically.
//! - A corrupt file is renamed aside (`.corrupt-<ms>`) and the store starts
//!   empty — the fail-closed direction. A decision that cannot be persisted is
//!   an error, never a silent "trusted".
//!
//! The renderer never decides trust; it asks (`checkWorkspaceTrust`) and
//! records (`trustWorkspace`). Enforcement happens in the chat service's
//! create/send paths, so untrusted execution is impossible even if the UI is
//! bypassed.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io;
use std::path::PathBuf;

/// One stored decision. `trusted_at` is epoch milliseconds (device-local
/// provenance only; never displayed as authoritative anywhere).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TrustedWorkspace {
    pub folder: String,
    #[serde(rename = "trustedAt")]
    pub trusted_at: i64,
}

#[derive(Serialize, Deserialize, Debug, Default)]
struct TrustFile {
    version: u32,
    workspaces: Vec<TrustedWorkspace>,
}

/// The trust decisions. `path = None` keeps the store in memory — tests only;
/// production always passes a file so a decision survives restart.
pub struct WorkspaceTrustStore {
    path: Option<PathBuf>,
    workspaces: BTreeMap<String, i64>,
}

impl WorkspaceTrustStore {
    /// Open (and load) the store. A load problem is survivable only in the
    /// fail-closed direction: the store comes up empty, never pre-trusted.
    pub fn new(path: Option<PathBuf>) -> Self {
        let mut store = Self {
            path,
            workspaces: BTreeMap::new(),
        };
        store.load();
        store
    }

    fn load(&mut self) {
        let Some(path) = self.path.as_deref() else {
            return;
        };
        let Ok(raw) = std::fs::read(path) else {
            return; // Missing file: no decisions yet. Any other read error also
                    // surfaces as "no decisions", which is the safe state.
        };
        match serde_json::from_slice::<TrustFile>(&raw) {
            Ok(file) => {
                for workspace in file.workspaces {
                    self.workspaces.insert(workspace.folder, workspace.trusted_at);
                }
            }
            Err(_) => {
                // Corrupt decisions must not be served or silently deleted
                // (the memory-store precedent): rename aside, start empty.
                let stamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis())
                    .unwrap_or_default();
                let aside = path.with_extension(format!("corrupt-{stamp}"));
                let _ = std::fs::rename(path, aside);
                self.workspaces.clear();
            }
        }
    }

    /// Is this canonical folder trusted? The caller passes the canonicalized
    /// path; this type never canonicalizes on its own.
    pub fn is_trusted(&self, canonical_folder: &str) -> bool {
        self.workspaces.contains_key(canonical_folder)
    }

    /// Record the decision for an already-canonicalized folder and persist.
    /// Persisting is part of deciding: the file is written first, and only a
    /// successful write commits the decision to memory — a decision that
    /// cannot be stored did not happen, in either direction.
    pub fn trust(&mut self, canonical_folder: &str, trusted_at: i64) -> io::Result<()> {
        let mut file = self.snapshot();
        file.workspaces.push(TrustedWorkspace {
            folder: canonical_folder.to_string(),
            trusted_at,
        });
        if self.path.is_some() {
            self.write(&file)?;
        }
        self.workspaces
            .insert(canonical_folder.to_string(), trusted_at);
        Ok(())
    }

    fn snapshot(&self) -> TrustFile {
        TrustFile {
            version: 1,
            workspaces: self
                .workspaces
                .iter()
                .map(|(folder, trusted_at)| TrustedWorkspace {
                    folder: folder.clone(),
                    trusted_at: *trusted_at,
                })
                .collect(),
        }
    }

    fn write(&self, file: &TrustFile) -> io::Result<()> {
        let path = self
            .path
            .as_deref()
            .expect("write is only called when a path is set");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut raw = serde_json::to_vec(file)?;
        raw.push(b'\n');
        // Atomic write: the decision must survive a crash as either absent or
        // complete, never half-written (the config-library rule).
        let temp = path.with_extension(format!("tmp-{}", std::process::id()));
        std::fs::write(&temp, raw)?;
        std::fs::rename(&temp, path)
    }
}

/// Epoch milliseconds for the decision timestamp.
pub fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_absent_record_is_untrusted() {
        let store = WorkspaceTrustStore::new(None);
        assert!(!store.is_trusted("/tmp/anywhere"));
    }

    #[test]
    fn a_decision_persists_and_survives_a_reopen() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let path = dir.path().join("workspace-trust.json");
        {
            let mut store = WorkspaceTrustStore::new(Some(path.clone()));
            store.trust("/tmp/trusted-repo", 1_234).expect("persist");
            assert!(store.is_trusted("/tmp/trusted-repo"));
            assert!(!store.is_trusted("/tmp/other"));
        }
        let reopened = WorkspaceTrustStore::new(Some(path));
        assert!(reopened.is_trusted("/tmp/trusted-repo"));
    }

    #[test]
    fn a_corrupt_file_is_set_aside_and_the_store_starts_empty() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let path = dir.path().join("workspace-trust.json");
        std::fs::write(&path, b"{ not json").expect("corrupt file");
        let store = WorkspaceTrustStore::new(Some(path.clone()));
        assert!(!store.is_trusted("/tmp/trusted-repo"));
        // The corrupt file was renamed aside, not deleted (the memory-store
        // precedent), and the store path is free again for the next decision.
        let aside = std::fs::read_dir(dir.path())
            .expect("dir")
            .filter_map(Result::ok)
            .find(|entry| entry.file_name().to_string_lossy().contains(".corrupt-"))
            .expect("corrupt file renamed aside");
        assert!(aside.metadata().expect("metadata").len() > 0);
        let reopened = WorkspaceTrustStore::new(Some(path));
        assert!(!reopened.is_trusted("/tmp/trusted-repo"));
    }

    #[test]
    fn a_failed_persist_is_an_error_not_a_silent_decision() {
        // A directory as the store path makes every write fail.
        let dir = tempfile::TempDir::new().expect("temp dir");
        let mut store = WorkspaceTrustStore::new(Some(dir.path().to_path_buf()));
        assert!(store.trust("/tmp/trusted-repo", 5).is_err());
        assert!(
            !store.is_trusted("/tmp/trusted-repo"),
            "a decision that cannot be stored did not happen"
        );
    }
}
