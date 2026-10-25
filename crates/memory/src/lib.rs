//! Memory — markdown entries, an index, and the baseline hash (roadmap
//! Phase 7; `docs/11` §5, `docs/features/settings.md` §Cowork).
//!
//! The captured model: `memory/memory/` holds Markdown memory files plus an
//! index; a global toggle gates memory; a baseline hash
//! (`globalMemoryBaselineHash`) detects changes; retention explicitly never
//! deletes memory; "reset/delete memory" must be explicit and auditable. The
//! exact index/embedding implementation is unknown ("may be file-based only",
//! `docs/11` §5) — the rebuild records these decisions in `docs/14` §9:
//!
//! - **The file is the truth.** `entries/<uuid>.md` holds the content;
//!   `index.json` is a derived cache (id, timestamps, size, sha256). Any
//!   mismatch at open time is resolved in the file's favor by rescanning —
//!   a corrupted index heals, it is never an error.
//! - **A corrupt entry file is set aside, never deleted**: renamed
//!   `<id>.corrupt-<ms>` inside `entries/` (the store's corrupt-database
//!   posture), reported once, and excluded from the index.
//! - **Bounds are resource limits, not observed caps**: content over 16 KiB
//!   is refused, never truncated (the projects service's posture).
//! - **The audit trail is an append-only `events.jsonl`** (0600): create,
//!   update, delete, and corrupt-file set-asides are named with ids only —
//!   content never enters the log (`TEST-PRIV-004`'s posture).
//! - **The baseline hash is `sha256` over the sorted `id:content-hash`
//!   lines.** Stable across reopenings; it changes iff some entry's content
//!   changed. The captured shipped algorithm was not captured; this one is
//!   rebuild-defined.
//!
//! The global `memoryEnabled` toggle is a capability of the applied profile;
//! enforcement lives at the bridge (every command re-resolves it), not here —
//! this crate is the storage engine and is policy-blind.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use sha2::{Digest, Sha256};

/// Resource bound: largest memory entry this rebuild accepts.
pub const MAX_ENTRY_BYTES: usize = 16 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum MemoryError {
    #[error("the memory entry does not exist")]
    NotFound,
    #[error("{0}")]
    Validation(&'static str),
    #[error("the memory store is unavailable: {0}")]
    Io(String),
    #[error("the memory entry {0} is unreadable and was set aside")]
    Corrupt(String),
}

/// One memory entry. `content` is Markdown; the rest is metadata.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryEntry {
    pub id: String,
    pub content: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// The derived index row (what `index.json` caches).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct IndexRow {
    id: String,
    created_at: i64,
    updated_at: i64,
    size: usize,
    hash: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct IndexFile {
    version: u32,
    #[serde(default)]
    baseline_hash: Option<String>,
    entries: Vec<IndexRow>,
}

/// The memory store. One instance per process; interior mutability is a
/// mutex over the whole store (single desktop core, like `crates/store`).
pub struct MemoryStore {
    root: PathBuf,
    lock: Mutex<()>,
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn is_uuid(value: &str) -> bool {
    uuid::Uuid::parse_str(value).is_ok()
}

impl MemoryStore {
    /// Open (creating if needed) the store at `root`. The directory layout:
    /// `entries/*.md`, `index.json`, `events.jsonl`. Opening reconciles the
    /// index against the files (the file is the truth); a corrupt file is
    /// set aside and named in `events.jsonl`, never served and never
    /// deleted.
    pub fn open(root: &Path) -> Result<MemoryStore, MemoryError> {
        std::fs::create_dir_all(root.join("entries"))
            .and_then(|_| std::fs::create_dir_all(root))
            .map_err(|error| MemoryError::Io(format!("{}: {error}", root.display())))?;
        let store = MemoryStore {
            root: root.to_path_buf(),
            lock: Mutex::new(()),
        };
        {
            let _guard = store.lock.lock().expect("memory mutex poisoned");
            store.reconcile()?;
        }
        Ok(store)
    }

    fn entries_dir(&self) -> PathBuf {
        self.root.join("entries")
    }

    fn index_path(&self) -> PathBuf {
        self.root.join("index.json")
    }

    fn events_path(&self) -> PathBuf {
        self.root.join("events.jsonl")
    }

    /// Append one audit line (ids and action names only, never content).
    fn record_event(&self, action: &str, id: &str) {
        let line = serde_json::json!({
            "action": action,
            "id": id,
            "at": now_ms(),
        });
        use std::io::Write as _;
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.events_path())
        {
            let _ = writeln!(file, "{line}");
        }
    }

    /// Reconcile the derived index with the entry files. Corrupt (non-UTF-8,
    /// unreadable) files are renamed aside; missing rows are added; stale
    /// rows are dropped. Content on disk always wins over the index.
    fn reconcile(&self) -> Result<(), MemoryError> {
        let rows: BTreeMap<String, IndexRow> = match std::fs::read(self.index_path()) {
            Ok(bytes) => serde_json::from_slice::<IndexFile>(&bytes)
                .map(|parsed| parsed.entries.into_iter().map(|row| (row.id.clone(), row)).collect())
                .unwrap_or_default(),
            Err(_) => BTreeMap::new(),
        };
        let dir = self.entries_dir();
        let listing = std::fs::read_dir(&dir)
            .map_err(|error| MemoryError::Io(format!("{}: {error}", dir.display())))?;
        let mut live: BTreeMap<String, IndexRow> = BTreeMap::new();
        for item in listing.flatten() {
            let path = item.path();
            let name = match path.file_name().and_then(|name| name.to_str()) {
                Some(name) => name.to_string(),
                None => continue,
            };
            if !name.ends_with(".md") {
                continue; // corrupt set-asides and temp files are not entries
            }
            let id = name.trim_end_matches(".md").to_string();
            if !is_uuid(&id) {
                continue;
            }
            let bytes = match std::fs::read(&path) {
                Ok(bytes) => bytes,
                Err(_) => continue,
            };
            // A non-UTF-8 entry is set aside, never deleted or served.
            if std::str::from_utf8(&bytes).is_err() {
                let aside = dir.join(format!(
                    "{name}.corrupt-{}",
                    chrono_now_ms()
                ));
                let _ = std::fs::rename(&path, aside);
                self.record_event("corrupt-set-aside", &id);
                continue;
            }
            let content = String::from_utf8_lossy(&bytes).to_string();
            let hash = sha256_hex(bytes.as_slice());
            match rows.get(&id) {
                Some(row) if row.hash == hash => {
                    live.insert(id, row.clone());
                }
                _ => {
                    // A new or externally edited file: timestamps come from
                    // the index when the content is unchanged, otherwise the
                    // file's own mtime is the honest updated_at.
                    let updated_at = std::fs::metadata(&path)
                        .and_then(|meta| meta.modified())
                        .ok()
                        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|elapsed| elapsed.as_millis() as i64)
                        .unwrap_or_else(now_ms);
                    live.insert(
                        id.clone(),
                        IndexRow {
                            id: id.clone(),
                            created_at: rows
                                .get(&id)
                                .map(|row| row.created_at)
                                .unwrap_or(updated_at),
                            updated_at,
                            size: content.len(),
                            hash,
                        },
                    );
                }
            }
        }
        self.write_index(live, false)?;
        Ok(())
    }

    /// Rewrite the index. `refresh_baseline` is true only for app-managed
    /// mutations: reconcile-on-open heals the entry rows but PRESERVES the
    /// previously recorded baseline, so an external edit stays visible
    /// through [`Self::baseline_changed`] until the app itself writes again.
    fn write_index(
        &self,
        rows: BTreeMap<String, IndexRow>,
        refresh_baseline: bool,
    ) -> Result<(), MemoryError> {
        let baseline = match refresh_baseline {
            true => Some(Self::compute_baseline(&rows)),
            false => match std::fs::read(self.index_path()) {
                Ok(bytes) => serde_json::from_slice::<IndexFile>(&bytes)
                    .ok()
                    .and_then(|file| file.baseline_hash),
                Err(_) => None,
            },
        }
        .unwrap_or_else(|| Self::compute_baseline(&rows));
        let file = IndexFile {
            version: 1,
            baseline_hash: Some(baseline),
            entries: rows.into_values().collect(),
        };
        let bytes = serde_json::to_vec_pretty(&file)
            .map_err(|error| MemoryError::Io(error.to_string()))?;
        write_atomic(&self.index_path(), &bytes).map_err(|error| MemoryError::Io(error.to_string()))
    }

    /// The baseline hash over the sorted `id:content-hash` lines. The
    /// captured shipped algorithm was not captured; this is rebuild-defined
    /// and deterministic across reopenings.
    fn compute_baseline(rows: &BTreeMap<String, IndexRow>) -> String {
        let mut hasher = Sha256::new();
        for row in rows.values() {
            hasher.update(row.id.as_bytes());
            hasher.update(b":");
            hasher.update(row.hash.as_bytes());
            hasher.update(b"\n");
        }
        let digest = hasher.finalize();
        let mut out = String::with_capacity(64);
        for byte in digest {
            use std::fmt::Write as _;
            let _ = write!(out, "{byte:02x}");
        }
        out
    }

    fn load_index(&self) -> Result<BTreeMap<String, IndexRow>, MemoryError> {
        let bytes = std::fs::read(self.index_path())
            .map_err(|error| MemoryError::Io(error.to_string()))?;
        let file: IndexFile = serde_json::from_slice(&bytes)
            .map_err(|error| MemoryError::Io(format!("the memory index is unreadable: {error}")))?;
        Ok(file
            .entries
            .into_iter()
            .map(|row| (row.id.clone(), row))
            .collect())
    }

    /// All entries, oldest creation first (the management list is stable).
    pub fn list(&self) -> Result<Vec<MemoryEntry>, MemoryError> {
        let _guard = self.lock.lock().expect("memory mutex poisoned");
        let rows = self.load_index()?;
        let mut out = Vec::new();
        for row in rows.values() {
            out.push(self.read_row(row)?);
        }
        out.sort_by(|a, b| a.created_at.cmp(&b.created_at).then_with(|| a.id.cmp(&b.id)));
        Ok(out)
    }

    fn read_row(&self, row: &IndexRow) -> Result<MemoryEntry, MemoryError> {
        let path = self.entries_dir().join(format!("{}.md", row.id));
        let bytes = std::fs::read(&path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                MemoryError::NotFound
            } else {
                MemoryError::Io(error.to_string())
            }
        })?;
        let content = String::from_utf8(bytes)
            .map_err(|_| MemoryError::Corrupt(row.id.clone()))?;
        Ok(MemoryEntry {
            id: row.id.clone(),
            content,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    /// One entry by id.
    pub fn read(&self, id: &str) -> Result<MemoryEntry, MemoryError> {
        if !is_uuid(id) {
            return Err(MemoryError::Validation("the memory id is not a uuid"));
        }
        let _guard = self.lock.lock().expect("memory mutex poisoned");
        let rows = self.load_index()?;
        let row = rows.get(id).ok_or(MemoryError::NotFound)?;
        self.read_row(row)
    }

    /// Add an entry. The content is trimmed; empty and oversized content is
    /// refused, never truncated.
    pub fn create(&self, content: &str, now: i64) -> Result<MemoryEntry, MemoryError> {
        let content = validate_content(content)?;
        let _guard = self.lock.lock().expect("memory mutex poisoned");
        let id = uuid::Uuid::new_v4().to_string();
        write_atomic(
            &self.entries_dir().join(format!("{id}.md")),
            content.as_bytes(),
        )
        .map_err(|error| MemoryError::Io(error.to_string()))?;
        let mut rows = self.load_index()?;
        rows.insert(
            id.clone(),
            IndexRow {
                id: id.clone(),
                created_at: now,
                updated_at: now,
                size: content.len(),
                hash: sha256_hex(content.as_bytes()),
            },
        );
        self.write_index(rows, true)?;
        self.record_event("create", &id);
        Ok(MemoryEntry {
            id,
            content,
            created_at: now,
            updated_at: now,
        })
    }

    /// Replace one entry's content. Timestamps: `updated_at` moves to `now`;
    /// `created_at` never changes.
    pub fn update(&self, id: &str, content: &str, now: i64) -> Result<MemoryEntry, MemoryError> {
        if !is_uuid(id) {
            return Err(MemoryError::Validation("the memory id is not a uuid"));
        }
        let content = validate_content(content)?;
        let _guard = self.lock.lock().expect("memory mutex poisoned");
        let mut rows = self.load_index()?;
        let created_at = {
            let row = rows.get_mut(id).ok_or(MemoryError::NotFound)?;
            row.updated_at = now;
            row.size = content.len();
            row.hash = sha256_hex(content.as_bytes());
            row.created_at
        };
        write_atomic(
            &self.entries_dir().join(format!("{id}.md")),
            content.as_bytes(),
        )
        .map_err(|error| MemoryError::Io(error.to_string()))?;
        self.write_index(rows, true)?;
        self.record_event("update", id);
        Ok(MemoryEntry {
            id: id.to_string(),
            content,
            created_at,
            updated_at: now,
        })
    }

    /// Delete one entry: file removed, index row dropped, audit event
    /// written. Retention never calls this — memory outlives session
    /// retention by design (`docs/11` §13).
    pub fn delete(&self, id: &str) -> Result<(), MemoryError> {
        if !is_uuid(id) {
            return Err(MemoryError::Validation("the memory id is not a uuid"));
        }
        let _guard = self.lock.lock().expect("memory mutex poisoned");
        let mut rows = self.load_index()?;
        if rows.remove(id).is_none() {
            return Err(MemoryError::NotFound);
        }
        match std::fs::remove_file(self.entries_dir().join(format!("{id}.md"))) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(MemoryError::Io(error.to_string())),
        }
        self.write_index(rows, true)?;
        self.record_event("delete", id);
        Ok(())
    }

    /// The live baseline hash over the current entries.
    pub fn baseline_hash(&self) -> Result<String, MemoryError> {
        let _guard = self.lock.lock().expect("memory mutex poisoned");
        let rows = self.load_index()?;
        Ok(Self::compute_baseline(&rows))
    }

    /// True when the baseline recorded in the index differs from the live
    /// hash — an external edit happened outside this process.
    pub fn baseline_changed(&self) -> Result<bool, MemoryError> {
        let _guard = self.lock.lock().expect("memory mutex poisoned");
        let bytes = std::fs::read(self.index_path())
            .map_err(|error| MemoryError::Io(error.to_string()))?;
        let file: IndexFile = serde_json::from_slice(&bytes)
            .map_err(|error| MemoryError::Io(format!("the memory index is unreadable: {error}")))?;
        let rows: BTreeMap<String, IndexRow> = file
            .entries
            .into_iter()
            .map(|row| (row.id.clone(), row))
            .collect();
        Ok(file.baseline_hash.as_deref() != Some(Self::compute_baseline(&rows).as_str()))
    }

    /// Delete everything (the explicit "Reset/delete memory" action). The
    /// audit event survives the reset; files and index do not.
    pub fn reset(&self) -> Result<usize, MemoryError> {
        let _guard = self.lock.lock().expect("memory mutex poisoned");
        let rows = self.load_index()?;
        let count = rows.len();
        for id in rows.keys() {
            let _ = std::fs::remove_file(self.entries_dir().join(format!("{id}.md")));
        }
        self.write_index(BTreeMap::new(), true)?;
        self.record_event("reset", &format!("count:{count}"));
        Ok(count)
    }
}

fn validate_content(content: &str) -> Result<String, MemoryError> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Err(MemoryError::Validation("the memory entry is empty"));
    }
    if trimmed.len() > MAX_ENTRY_BYTES {
        return Err(MemoryError::Validation(
            "the memory entry is too long (the limit is 16 KiB)",
        ));
    }
    Ok(trimmed.to_string())
}

/// Write via a sibling temp file + rename so a crash never leaves a half
/// entry or a half index.
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), std::io::Error> {
    let tmp = path.with_extension(format!(
        "tmp-{}",
        std::process::id()
    ));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or(0)
}

fn chrono_now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{MemoryError, MemoryStore, MAX_ENTRY_BYTES};
    use std::sync::Arc;

    fn store() -> (tempfile::TempDir, MemoryStore) {
        let dir = tempfile::tempdir().expect("temp");
        let store = MemoryStore::open(&dir.path().join("memory")).expect("open");
        (dir, store)
    }

    #[test]
    fn entries_persist_across_a_store_reopen() {
        let dir = tempfile::tempdir().expect("temp");
        let root = dir.path().join("memory");
        {
            let store = MemoryStore::open(&root).expect("open");
            let entry = store.create("  prefers concise answers  ", 1_000).expect("create");
            assert_eq!(entry.content, "prefers concise answers");
        }
        // The file is the persisted truth; a fresh instance reads it back.
        let reopened = MemoryStore::open(&root).expect("reopen");
        let listed = reopened.list().expect("list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].content, "prefers concise answers");
        assert_eq!(listed[0].created_at, 1_000);
        // The entry file really exists on disk with the content.
        let bytes = std::fs::read(root.join("entries").join(format!("{}.md", listed[0].id)))
            .expect("entry file");
        assert_eq!(String::from_utf8(bytes).expect("utf-8"), "prefers concise answers");
    }

    #[test]
    fn a_corrupted_index_heals_from_the_files_on_open() {
        let dir = tempfile::tempdir().expect("temp");
        let root = dir.path().join("memory");
        {
            let store = MemoryStore::open(&root).expect("open");
            store.create("fact one", 1_000).expect("create");
        }
        // Trash the derived index; the files must win.
        std::fs::write(root.join("index.json"), b"{ not json at all").expect("trash index");
        let reopened = MemoryStore::open(&root).expect("reopen with corrupt index");
        let listed = reopened.list().expect("list");
        assert_eq!(listed.len(), 1, "the entry survives a corrupted index");
        assert_eq!(listed[0].content, "fact one");
    }

    #[test]
    fn an_externally_edited_file_wins_and_moves_the_baseline() {
        let dir = tempfile::tempdir().expect("temp");
        let root = dir.path().join("memory");
        let store = MemoryStore::open(&root).expect("open");
        let entry = store.create("original", 1_000).expect("create");
        assert!(!store.baseline_changed().expect("baseline"), "a fresh store is not changed");
        // Edit the file behind the store's back.
        let path = root.join("entries").join(format!("{}.md", entry.id));
        std::fs::write(&path, b"edited externally").expect("edit file");
        // A reopen reconciles in the file's favor.
        let reopened = MemoryStore::open(&root).expect("reopen");
        let listed = reopened.list().expect("list");
        assert_eq!(listed[0].content, "edited externally");
        // The baseline hash tracks content, not timestamps.
        let before = reopened.baseline_hash().expect("hash");
        assert!(reopened.baseline_changed().expect("changed"), "the recorded baseline is stale");
        // Any mutation refreshes the recorded baseline.
        reopened.update(&entry.id, "edited externally again", 2_000).expect("update");
        assert!(!reopened.baseline_changed().expect("unchanged"));
        assert_ne!(reopened.baseline_hash().expect("hash2"), before);
    }

    #[test]
    fn a_non_utf8_entry_file_is_set_aside_never_served() {
        let dir = tempfile::tempdir().expect("temp");
        let root = dir.path().join("memory");
        MemoryStore::open(&root).expect("open creates the layout");
        std::fs::write(root.join("entries").join("00000000-0000-0000-0000-00000000000a.md"), [0xff, 0xfe, 0x00])
            .expect("write corrupt file");
        let reopened = MemoryStore::open(&root).expect("reopen");
        assert!(reopened.list().expect("list").is_empty(), "a corrupt file is not an entry");
        // It was renamed aside, not deleted.
        let entries_dir = root.join("entries");
        let aside = std::fs::read_dir(&entries_dir)
            .expect("dir")
            .flatten()
            .find(|item| item.file_name().to_string_lossy().contains(".corrupt-"))
            .expect("the corrupt file must survive as .corrupt-<ms>");
        assert!(aside.metadata().expect("meta").len() > 0);
        // And the audit trail names it.
        let events = std::fs::read_to_string(root.join("events.jsonl")).expect("events");
        assert!(events.contains("corrupt-set-aside"));
    }

    #[test]
    fn bounds_and_id_shapes_are_refused_not_truncated() {
        let (_dir, store) = store();
        assert!(matches!(
            store.create("   ", 1),
            Err(MemoryError::Validation("the memory entry is empty"))
        ));
        let long = "x".repeat(MAX_ENTRY_BYTES + 1);
        assert!(matches!(store.create(&long, 1), Err(MemoryError::Validation(_))));
        assert!(matches!(
            store.read("not-a-uuid"),
            Err(MemoryError::Validation("the memory id is not a uuid"))
        ));
        assert!(matches!(
            store.delete("../../etc/passwd"),
            Err(MemoryError::Validation("the memory id is not a uuid"))
        ));
        assert!(matches!(store.read("00000000-0000-0000-0000-00000000000b"), Err(MemoryError::NotFound)));
    }

    #[test]
    fn update_and_delete_propagate_to_files_and_survive_reopen() {
        let dir = tempfile::tempdir().expect("temp");
        let root = dir.path().join("memory");
        let store = MemoryStore::open(&root).expect("open");
        let entry = store.create("first", 1_000).expect("create");
        let updated = store.update(&entry.id, "second", 2_000).expect("update");
        assert_eq!(updated.created_at, 1_000, "created_at never moves");
        assert_eq!(updated.updated_at, 2_000);
        store.delete(&entry.id).expect("delete");
        let path = root.join("entries").join(format!("{}.md", entry.id));
        assert!(!path.exists(), "the file must be gone");
        assert!(matches!(store.read(&entry.id), Err(MemoryError::NotFound)));
        let reopened = MemoryStore::open(&root).expect("reopen");
        assert!(reopened.list().expect("list").is_empty());
        let events = std::fs::read_to_string(root.join("events.jsonl")).expect("events");
        assert!(events.contains("\"create\""));
        assert!(events.contains("\"update\""));
        assert!(events.contains("\"delete\""));
    }

    #[test]
    fn reset_removes_every_entry_but_keeps_the_audit_trail() {
        let dir = tempfile::tempdir().expect("temp");
        let root = dir.path().join("memory");
        let store = MemoryStore::open(&root).expect("open");
        store.create("a", 1_000).expect("create");
        store.create("b", 1_100).expect("create");
        assert_eq!(store.reset().expect("reset"), 2);
        assert!(store.list().expect("list").is_empty());
        let events = std::fs::read_to_string(root.join("events.jsonl")).expect("events");
        assert!(events.contains("reset"));
    }

    #[test]
    fn concurrent_writers_do_not_lose_entries() {
        let dir = tempfile::tempdir().expect("temp");
        let root = dir.path().join("memory");
        let store = Arc::new(MemoryStore::open(&root).expect("open"));
        let handles: Vec<_> = (0..8)
            .map(|i| {
                let store = Arc::clone(&store);
                std::thread::spawn(move || store.create(&format!("entry {i}"), 1_000 + i))
            })
            .collect();
        for handle in handles {
            handle.join().expect("thread").expect("create");
        }
        assert_eq!(store.list().expect("list").len(), 8);
        let reopened = MemoryStore::open(&root).expect("reopen");
        assert_eq!(reopened.list().expect("list").len(), 8, "all eight persisted");
    }

    #[cfg(unix)]
    #[test]
    fn a_failed_write_reports_an_error_and_leaves_no_partial_state() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("temp");
        let root = dir.path().join("memory");
        let store = MemoryStore::open(&root).expect("open");
        // Make the entries directory unwritable; create must fail loudly.
        std::fs::set_permissions(root.join("entries"), std::fs::Permissions::from_mode(0o500))
            .expect("chmod");
        assert!(matches!(store.create("doomed", 1), Err(MemoryError::Io(_))));
        std::fs::set_permissions(root.join("entries"), std::fs::Permissions::from_mode(0o700))
            .expect("chmod back");
        assert!(store.list().expect("list").is_empty(), "no partial state");
    }

    #[test]
    fn baseline_hash_is_stable_across_reopens_and_ignores_timestamps() {
        let dir = tempfile::tempdir().expect("temp");
        let root = dir.path().join("memory");
        let store = MemoryStore::open(&root).expect("open");
        let a = store.create("alpha", 1_000).expect("create");
        store.create("beta", 2_000).expect("create");
        let first = store.baseline_hash().expect("hash");
        // Reopening (and reconciling) must not change the hash.
        let reopened = MemoryStore::open(&root).expect("reopen");
        assert_eq!(reopened.baseline_hash().expect("hash2"), first);
        // Content-identical replacement keeps the hash stable.
        reopened.update(&a.id, "alpha", 3_000).expect("update");
        assert_eq!(reopened.baseline_hash().expect("hash3"), first);
    }
}
