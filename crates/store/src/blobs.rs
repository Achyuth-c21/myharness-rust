//! Content-addressed files (`docs/11` §14: transcripts, attachments, outputs
//! are stored as content addressed with references, not rows).
//!
//! A blob's name is the lowercase SHA-256 of its bytes; reads are by hash. The
//! hash is validated *before* it touches a path, so a caller cannot send
//! `../`-shaped "hashes" anywhere — confinement is the check, not a convention
//! (`docs/11` §4.4: paths confined to the granted folder).

use std::io::Write;
use std::path::PathBuf;

use sha2::{Digest, Sha256};

use crate::secure;
use crate::{is_blob_ref, now_ms, StoreError};

/// The content-addressed store rooted at one directory (already 0700).
pub struct BlobStore {
    root: PathBuf,
}

impl BlobStore {
    pub(crate) fn new(root: PathBuf) -> Result<BlobStore, StoreError> {
        std::fs::create_dir_all(&root)?;
        secure::dir(&root)?;
        Ok(BlobStore { root })
    }

    /// Split a validated hash into (prefix dir, leaf path) under `root`. The
    /// two-character prefix directory keeps listings sane; 64 hex characters
    /// cannot escape (`../` is not hex).
    fn hash_path(&self, hash: &str) -> Result<(PathBuf, PathBuf), StoreError> {
        if !is_blob_ref(hash) {
            return Err(StoreError::Validation("not a SHA-256 hex digest"));
        }
        let dir = self.root.join(&hash[..2]);
        Ok((dir.clone(), dir.join(hash)))
    }

    /// Store bytes; idempotent on identical content. Returns the hash the file
    /// is now addressable by.
    pub fn put(&self, bytes: &[u8]) -> Result<String, StoreError> {
        let hash = hex_digest(bytes);
        let (dir, target) = self.hash_path(&hash)?;
        if target.exists() {
            return Ok(hash);
        }
        std::fs::create_dir_all(&dir)?;
        // Temp + rename: a concurrent reader never sees a partial blob, and a
        // crashed write never masquerades as the content it claimed.
        let tmp = dir.join(format!(
            "partial-{}-{}",
            now_ms(),
            std::process::id()
        ));
        {
            let mut file = std::fs::File::create(&tmp)?;
            file.write_all(bytes)?;
            file.sync_all()?;
        }
        std::fs::rename(&tmp, &target)?;
        secure::file(&target)?;
        Ok(hash)
    }

    /// Read back by hash. `None` when the blob is absent.
    pub fn read(&self, hash: &str) -> Result<Option<Vec<u8>>, StoreError> {
        let (_, target) = self.hash_path(hash)?;
        match std::fs::read(&target) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn exists(&self, hash: &str) -> Result<bool, StoreError> {
        let (_, target) = self.hash_path(hash)?;
        Ok(target.exists())
    }

    /// Delete one blob if present. Absence is success — delete-sweeps are
    /// idempotent by design (`TEST-SESSION-004`).
    pub fn delete(&self, hash: &str) -> Result<(), StoreError> {
        let (dir, target) = self.hash_path(hash)?;
        match std::fs::remove_file(&target) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // Tidy a prefix dir left empty by the sweep; failure is fine.
                let _ = std::fs::remove_dir(dir);
                Ok(())
            }
            Err(e) => Err(e.into()),
        }
    }

    /// The set of content files currently on disk (hashes).
    pub fn list(&self) -> Result<Vec<String>, StoreError> {
        let mut hashes = Vec::new();
        for entry in std::fs::read_dir(&self.root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            for file in std::fs::read_dir(entry.path())? {
                let file = file?;
                let name = file.file_name().to_string_lossy().into_owned();
                if is_blob_ref(&name) {
                    hashes.push(name);
                }
            }
        }
        Ok(hashes)
    }
}

fn hex_digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let out = hasher.finalize();
    out.iter().map(|b| format!("{b:02x}")).collect()
}
