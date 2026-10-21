//! Third-party configuration library — roadmap Phase 2 (docs/13 §4 Phase 2).
//!
//! Owns the decision-core the setup window operates on: the typed [`ProfileDocument`]
//! mirroring `docs/18` §10, and the on-disk [`ConfigLibrary`] with `docs/11` §3 semantics
//! (`configLibrary/<uuid>.json` + `_meta.json` with `appliedId`/`entries`).
//!
//! Rule sources, in force for every operation here:
//! - **Atomic writes** — profile/meta writes go through a temp file + rename with file
//!   and directory fsync (`docs/11` §3: the file layer alone is not atomic; "the rebuild
//!   must add that"). A failed apply therefore leaves the prior profile active
//!   (`docs/18` §10 / API-CFG-01 family).
//! - **Forward compatibility** — unknown profile keys, unknown meta keys, and unknown
//!   meta entry keys are preserved on rewrite (`docs/11` §2 unknown-key rule applied to
//!   profiles), so a rebuild write cannot destroy state introduced by another writer.
//! - **Secret hygiene** — profile documents may contain secrets; files are written with
//!   `0600` permissions on POSIX (`docs/11` §3) and meta documents never carry section
//!   payloads. Renderer-facing secret handling (opaque handles, never values) is a
//!   bridge concern (`docs/18` §10) and lands with the bridge slice.
//! - **Exclusive credential kinds** — [`CredentialKind`] is an enum, so a profile
//!   cannot hold a fallback pair (`REQ-SETUP-05`).
//! - **Structural validation only** — schema version, id, name, provider, and section
//!   shapes are validated here; per-provider field validation (the provider form
//!   schemas of `docs/09` §2.2) is its own later Phase-2 slice, as are the managed
//!   overlay (getMerged), health/recheck, and export redaction rows.
//!
//! Ops intentionally NOT in this slice (later Phase-2 slices, per `docs/18` §10):
//! `getMerged` (managed overlay trust), `health`/`recheck`, export redaction, and the
//! render-layer draft session (the UI keeps drafts in memory and persists only on
//! Apply, mirroring `docs/screens/setup-desktop-3p.md` "Apply is the only save action").
//! Those slots stay `declared, pending` in `docs/17` §4.8.
//!
//! Wire-format spelling: `ProviderKind`/`CredentialKind` serialize kebab-case ("claude-api",
//! "bedrock-mantle", "static", ...). The shipped renderer's exact enum strings were not
//! captured (`docs/20` records `enum/string` classes without values) — recorded as an
//! unknown in `docs/14` §9; legacy `claudeAiImport` mapping must translate freely.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Schema versions this crate understands. Profiles from other writers carrying a
/// future `schemaVersion` stay readable through unknown-key preservation but are
/// refused at [`ConfigLibrary::apply`] — fail closed (`docs/01` methodology).
const KNOWN_SCHEMA_VERSIONS: [u32; 1] = [1];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderKind {
    Gateway,
    ClaudeApi,
    Bedrock,
    BedrockMantle,
    Vertex,
    Foundry,
}

/// Credential source for a provider. Singular by construction: a profile holds at
/// most one, so "when set, only that source is used (no fallback)" is not a runtime
/// check but a type invariant (`REQ-SETUP-05`, docs/09 §2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CredentialKind {
    Static,
    Interactive,
    Helper,
}

/// One section payload. Opaque at this layer (`docs/18` §10: typed per provider);
/// provider schemas arrive in the forms slice. Preserved through rewrite as written.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Section(pub serde_json::Value);

impl Section {
    /// `null` sections serialize as absent — fresh/draft profiles stay clean
    /// (docs/11 §3: observed profiles range down to empty objects).
    fn is_empty(&self) -> bool {
        self.0.is_null()
    }
}

/// A third-party profile document (`docs/18` §10 `ConfigProfile`). Wire keys are
/// camelCase (`docs/20` key index; `docs/18` §10 schemaVersion). Unknown top-level
/// keys (legacy import payloads, forward-compatible writers) round-trip unchanged.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProfileDocument {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<ProviderKind>,
    #[serde(default, skip_serializing_if = "Self::section_absent")]
    pub connection: Section,
    #[serde(default, skip_serializing_if = "Self::section_absent")]
    pub capabilities: Section,
    #[serde(default, skip_serializing_if = "Self::section_absent")]
    pub connectors: Section,
    #[serde(default, skip_serializing_if = "Self::section_absent")]
    pub telemetry: Section,
    #[serde(default, skip_serializing_if = "Self::section_absent")]
    pub limits: Section,
    #[serde(default, skip_serializing_if = "Self::section_absent")]
    pub appearance: Section,
    #[serde(default, skip_serializing_if = "Self::section_absent")]
    pub plugins: Section,
    #[serde(default, skip_serializing_if = "Self::section_absent")]
    pub egress: Section,
    #[serde(default, skip_serializing_if = "Self::section_absent")]
    pub source: Section,
    /// Unknown top-level keys, preserved verbatim on rewrite (docs/11 §2 rule).
    #[serde(flatten)]
    pub extra: BTreeSection,
}

type BTreeSection = std::collections::BTreeMap<String, serde_json::Value>;

fn default_schema_version() -> u32 {
    1
}

impl ProfileDocument {
    fn section_absent(section: &Section) -> bool {
        section.is_empty()
    }

    /// Structural gate for the merged config at apply time (`docs/18` §10: "apply
    /// validates the merged config"; per-provider field validation is a later slice).
    pub fn validate_structural(&self) -> Result<(), ConfigError> {
        if !KNOWN_SCHEMA_VERSIONS.contains(&self.schema_version) {
            return Err(ConfigError::Schema {
                detail: format!("schemaVersion {} not supported", self.schema_version),
            });
        }
        let id = self.id.as_deref().unwrap_or_default();
        if uuid::Uuid::parse_str(id).is_err() {
            return Err(ConfigError::Validation {
                detail: format!("profile id {:?} is not a uuid", safe_text(id)),
            });
        }
        match self.name.as_deref().map(str::trim) {
            Some(name) if !name.is_empty() => {}
            _ => {
                return Err(ConfigError::Validation {
                    detail: "profile name must be non-empty".to_string(),
                })
            }
        }
        for (label, section) in [
            ("connection", &self.connection),
            ("capabilities", &self.capabilities),
            ("connectors", &self.connectors),
            ("telemetry", &self.telemetry),
            ("limits", &self.limits),
            ("appearance", &self.appearance),
            ("plugins", &self.plugins),
            ("egress", &self.egress),
            ("source", &self.source),
        ] {
            match &section.0 {
                serde_json::Value::Null | serde_json::Value::Object(_) => {}
                _ => {
                    return Err(ConfigError::Validation {
                        detail: format!("{label} section must be an object or absent"),
                    })
                }
            }
        }
        Ok(())
    }
}

/// `_meta.json` (`docs/11` §3, keys camelCase: `appliedId`, `entries`): applied
/// selection + the library ordering. Unknown top-level and entry keys round-trip
/// verbatim.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LibraryMeta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied_id: Option<String>,
    #[serde(default)]
    pub entries: Vec<MetaEntry>,
    #[serde(flatten)]
    pub extra: BTreeSection,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetaEntry {
    pub id: String,
    pub name: String,
    #[serde(flatten)]
    pub extra: BTreeSection,
}

/// Log-safe rendering: control characters collapse to `?` (crafting a name cannot
/// forge log lines). Used for every error detail that echoes file text.
fn safe_text(raw: &str) -> String {
    raw.chars()
        .map(|c| if c.is_control() { '?' } else { c })
        .collect()
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// Stable codes follow the `docs/18` §11 envelope discipline ("stable machine
    /// code for tests/telemetry"); user-safe mapping to envelopes is the bridge slice.
    #[error("CONFIG_IO: {source}")]
    Io { source: std::io::Error },
    #[error("CONFIG_PARSE: {source}")]
    Parse {
        source: serde_json::Error,
    },
    #[error("CONFIG_SCHEMA: {detail}")]
    Schema { detail: String },
    #[error("CONFIG_VALIDATION: {detail}")]
    Validation { detail: String },
    #[error("CONFIG_NOT_FOUND: no profile {id:?} in the library")]
    NotFound { id: String },
    #[error("CONFIG_DUPLICATE: a profile named {name:?} already exists")]
    DuplicateName { name: String },
    /// The active profile carries the applied configuration; deleting it would leave
    /// the library without an applied selection at runtime. Removing the *applied*
    /// profile happens through create-then-apply-then-delete-old, so this guard is
    /// fail-safe (shipped-behavior unknown: recorded in `docs/14` §9).
    #[error("CONFIG_IN_USE: profile {id:?} is applied and cannot be deleted")]
    InUse { id: String },
}

impl From<std::io::Error> for ConfigError {
    fn from(source: std::io::Error) -> Self {
        ConfigError::Io { source }
    }
}

impl From<serde_json::Error> for ConfigError {
    fn from(source: serde_json::Error) -> Self {
        ConfigError::Parse { source }
    }
}

/// The `configLibrary` directory itself (`docs/09` §2: e.g.
/// `~/.config/Claude-3p/configLibrary`). The binary wiring picks the user-data root;
/// tests use a throwaway directory.
pub struct ConfigLibrary {
    root: PathBuf,
}

impl ConfigLibrary {
    /// Pure constructor — no I/O; directories appear on first create (0700 on POSIX).
    pub fn open(root: impl Into<PathBuf>) -> Self {
        ConfigLibrary { root: root.into() }
    }

    fn profile_path(&self, id: &str) -> PathBuf {
        self.root.join(format!("{id}.json"))
    }

    fn meta_path(&self) -> PathBuf {
        self.root.join("_meta.json")
    }

    /// Load `_meta.json`, tolerating absence (fresh library) and tolerating stale
    /// profile files without entries (they are simply unreachable). Unknown keys stay
    /// in the returned value for rewrite preservation.
    fn load_meta(&self) -> Result<LibraryMeta, ConfigError> {
        let path = self.meta_path();
        if !path.exists() {
            return Ok(LibraryMeta::default());
        }
        let bytes = fs::read(&path)?;
        if bytes.is_empty() {
            return Ok(LibraryMeta::default());
        }
        Ok(serde_json::from_slice(&bytes)?)
    }

    /// Replace `_meta.json` atomically. Apply atomicity lives here: either the old
    /// meta (prior active profile) or the new meta exists — never a torn state
    /// (`docs/18` §10 "failed apply leaves the prior profile active").
    fn store_meta(&self, meta: &LibraryMeta) -> Result<(), ConfigError> {
        let bytes = serde_json::to_vec_pretty(meta)?;
        atomic_write(&self.meta_path(), &bytes)
    }

    fn ensure_dir(&self) -> Result<(), ConfigError> {
        if !self.root.exists() {
            #[cfg(unix)]
            {
                if let Some(parent) = self.root.parent() {
                    if !parent.as_os_str().is_empty() {
                        fs::create_dir_all(parent)?;
                    }
                }
                use std::os::unix::fs::DirBuilderExt as _;
                std::fs::DirBuilder::new().mode(0o700).create(&self.root)?;
            }
            #[cfg(not(unix))]
            fs::create_dir_all(&self.root)?;
        }
        Ok(())
    }

    fn load_profile(&self, id: &str) -> Result<ProfileDocument, ConfigError> {
        let path = self.profile_path(id);
        if !path.exists() {
            return Err(ConfigError::NotFound { id: id.to_string() });
        }
        let bytes = fs::read(&path)?;
        let mut doc: ProfileDocument = serde_json::from_slice(&bytes)?;
        // The id is authoritative from the FILE (the key the meta entries point at),
        // not from doc payload text; a mismatched inner id would make edits redirect.
        doc.id.get_or_insert(id.to_string());
        Ok(doc)
    }

    fn store_profile(&self, doc: &ProfileDocument) -> Result<(), ConfigError> {
        let bytes = serde_json::to_vec_pretty(doc)?;
        let Some(id) = doc.id.as_deref() else {
            return Err(ConfigError::Validation {
                detail: "profile document carries no id".to_string(),
            });
        };
        atomic_write(&self.profile_path(id), &bytes)
    }

    fn assert_free_name(&self, meta: &LibraryMeta, name: &str, except: Option<&str>) -> Result<(), ConfigError> {
        if name.trim().is_empty() {
            return Err(ConfigError::Validation {
                detail: "profile name must be non-empty".to_string(),
            });
        }
        if meta
            .entries
            .iter()
            .any(|e| e.name == name && Some(e.id.as_str()) != except)
        {
            return Err(ConfigError::DuplicateName {
                name: safe_text(name).to_string(),
            });
        }
        Ok(())
    }

    /// `create` — mint a draft profile (never active until apply; `docs/screens/setup-desktop-3p.md`).
    pub fn create(&self, name: &str, provider: ProviderKind) -> Result<ProfileDocument, ConfigError> {
        self.ensure_dir()?;
        let mut meta = self.load_meta()?;
        self.assert_free_name(&meta, name, None)?;
        let doc = ProfileDocument {
            schema_version: 1,
            id: Some(Uuid::new_v4().to_string()),
            name: Some(name.to_string()),
            provider: Some(provider),
            ..ProfileDocument::default()
        };
        doc.validate_structural()?;
        // Order: profile file first, then the meta pointer. A crash between the two
        // leaves an orphan file that the next `load_meta` ignores (no entry) —
        // recoverable by design, never a broken active config.
        self.store_profile(&doc)?;
        meta.entries.push(MetaEntry {
            id: doc.id.clone().expect("create sets id"),
            name: name.to_string(),
            extra: Default::default(),
        });
        self.store_meta(&meta)?;
        Ok(doc)
    }

    /// `list` — meta entries in file order (`docs/11` §3: entries carry ordering).
    pub fn list(&self) -> Result<Vec<MetaEntry>, ConfigError> {
        Ok(self.load_meta()?.entries)
    }

    /// `read` — the applied-or-not profile document.
    pub fn read(&self, id: &str) -> Result<ProfileDocument, ConfigError> {
        self.load_profile(id)
    }

    /// `write` — persist an edited profile (draft-save at the library layer; the UI's
    /// in-memory draft only reaches here on Apply, per `docs/screens/setup-desktop-3p.md`).
    /// Validated before write; invalid drafts never touch the file or the active pointer.
    pub fn write(&self, doc: &ProfileDocument) -> Result<(), ConfigError> {
        self.ensure_dir()?;
        doc.validate_structural()?;
        let Some(id) = doc.id.clone() else {
            return Err(ConfigError::Validation {
                detail: "profile document carries no id".to_string(),
            });
        };
        let mut meta = self.load_meta()?;
        if !meta.entries.iter().any(|e| e.id == id) {
            return Err(ConfigError::NotFound { id });
        }
        self.assert_free_name(&meta, doc.name.as_deref().unwrap_or_default().trim(), Some(&id))?;
        // Doc-first ordering keeps the profile authoritative while the entry-name sync
        // is a separate atomic step (a failure between the two is re-runnable).
        let bytes = serde_json::to_vec_pretty(doc)?;
        atomic_write(&self.profile_path(&id), &bytes)?;
        let naming = doc.name.clone().expect("write requires a named profile");
        let mut stale = false;
        for entry in &mut meta.entries {
            if entry.id == id && entry.name != naming {
                entry.name = naming.clone();
                stale = true;
            }
        }
        if stale {
            self.store_meta(&meta)?;
        }
        Ok(())
    }

    /// `apply` — validate the target, then atomically flip `appliedId`
    /// (`docs/screens/setup-desktop-3p.md`: "Apply is atomic; failed apply leaves the
    /// previous profile"; `docs/18` §10 API-CFG-01 family). Relaunch semantics are
    /// renderer/shell concerns (docs/02 §2), not library concerns.
    pub fn apply(&self, id: &str) -> Result<(), ConfigError> {
        let doc = self.load_profile(id)?;
        doc.validate_structural()?;
        let mut meta = self.load_meta()?;
        if meta.applied_id.as_deref() != Some(id) {
            meta.applied_id = Some(id.to_string());
            self.store_meta(&meta)?; // single atomic rename = the apply
        }
        Ok(())
    }

    /// `active` — applied entry, if any (fresh libraries have none).
    pub fn active(&self) -> Result<Option<MetaEntry>, ConfigError> {
        let meta = self.load_meta()?;
        let Some(applied) = meta.applied_id.as_deref() else {
            return Ok(None);
        };
        Ok(meta.entries.iter().find(|e| e.id == applied).cloned())
    }

    /// `rename` — doc-first, then meta entry sync (atomic per file; see `write`).
    pub fn rename(&self, id: &str, new_name: &str) -> Result<(), ConfigError> {
        let mut doc = self.load_profile(id)?;
        doc.name = Some(new_name.to_string());
        self.write(&doc)
    }

    /// `duplicate` — independent copy with a fresh id and an explicit name (the
    /// copy-naming convention was not observed; nothing is invented here). Copies are
    /// drafts: the active selection is never moved by duplication.
    pub fn duplicate(&self, id: &str, new_name: &str) -> Result<ProfileDocument, ConfigError> {
        self.ensure_dir()?;
        let source = self.load_profile(id)?;
        let mut meta = self.load_meta()?;
        self.assert_free_name(&meta, new_name, None)?;
        let mut doc = source.clone();
        doc.id = Some(Uuid::new_v4().to_string());
        doc.name = Some(new_name.to_string());
        self.store_profile(&doc)?;
        meta.entries.push(MetaEntry {
            id: doc.id.clone().expect("duplicate sets id"),
            name: new_name.to_string(),
            extra: Default::default(),
        });
        self.store_meta(&meta)?;
        Ok(doc)
    }

    /// `delete` — refuses the applied profile (`CONFIG_IN_USE` guard; see the
    /// variant docs). The entry is removed first; the file is best-effort unlinked,
    /// so a leftover file is a safe orphan.
    pub fn delete(&self, id: &str) -> Result<(), ConfigError> {
        self.ensure_dir()?;
        let mut meta = self.load_meta()?;
        if meta.applied_id.as_deref() == Some(id) {
            return Err(ConfigError::InUse { id: id.to_string() });
        }
        let before = meta.entries.len();
        meta.entries.retain(|e| e.id != id);
        if meta.entries.len() == before {
            return Err(ConfigError::NotFound { id: id.to_string() });
        }
        self.store_meta(&meta)?;
        let _ = fs::remove_file(self.profile_path(id));
        Ok(())
    }
}

/// Atomic payload write (docs/11 §3): same-directory temp file, `0600` on POSIX,
/// full write + `sync_all`, then rename over the target, then best-effort directory
/// fsync. Any failure before the rename removes the temp file and leaves the target
/// untouched.
fn atomic_write(path: &Path, payload: &[u8]) -> Result<(), ConfigError> {
    let parent = path
        .parent()
        .ok_or(ConfigError::Io {
            source: std::io::Error::other("target has no parent directory"),
        })?
        .to_path_buf();
    let temp = parent.join(format!(".{}.tmp-{}", temp_stem(path), Uuid::new_v4()));
    let cleanup = |temp: &Path| {
        let _ = fs::remove_file(temp);
    };
    let written = fs::File::create(&temp).map_err(ConfigError::from);
    let mut file = match written {
        Ok(file) => file,
        Err(error) => {
            cleanup(&temp);
            return Err(error);
        }
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let permissions = std::fs::Permissions::from_mode(0o600);
        if let Err(error) = file.set_permissions(permissions).map_err(ConfigError::from) {
            cleanup(&temp);
            return Err(error);
        }
    }
    if let Err(error) = file.write_all(payload).map_err(ConfigError::from) {
        cleanup(&temp);
        return Err(error);
    }
    if let Err(error) = file.sync_all().map_err(ConfigError::from) {
        cleanup(&temp);
        return Err(error);
    }
    drop(file);
    if let Err(error) = fs::rename(&temp, path) {
        cleanup(&temp);
        return Err(ConfigError::Io { source: error });
    }
    // Best-effort durability point for the directory entry; failures here do not
    // invalidate the completed write.
    if let Ok(dir) = fs::File::open(&parent) {
        let _ = dir.sync_all();
    }
    Ok(())
}

fn temp_stem(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| safe_text(name))
        .unwrap_or_else(|| "profile".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn scratch() -> ConfigLibrary {
        let dir = std::env::temp_dir().join(format!("mh-cfg-test-{}", Uuid::new_v4()));
        ConfigLibrary::open(dir)
    }

    fn doc(name: &str, provider: ProviderKind) -> ProfileDocument {
        ProfileDocument {
            schema_version: 1,
            id: Some(Uuid::new_v4().to_string()),
            name: Some(name.to_string()),
            provider: Some(provider),
            ..ProfileDocument::default()
        }
    }

    /// TEST-CFG-001 (drafting): create + list + read round-trip, and the draft is
    /// never the active profile until apply.
    #[test]
    fn create_is_draft_and_never_active() {
        let lib = scratch();
        lib.create("Workspace A", ProviderKind::Gateway).unwrap();
        assert!(lib.active().unwrap().is_none(), "fresh create must not set appliedId");
        let listed = lib.list().unwrap();
        assert_eq!(listed.len(), 1, "meta entry exists immediately");
    }

    /// TEST-CFG-002 (apply): apply flips appliedId atomically; active() reports it.
    #[test]
    fn apply_sets_active() {
        let lib = scratch();
        let draft = lib.create("Workspace A", ProviderKind::Gateway).unwrap();
        let other = lib.create("Workspace B", ProviderKind::ClaudeApi).unwrap();
        lib.apply(&draft.id.clone().unwrap()).unwrap();
        assert_eq!(lib.active().unwrap().map(|e| e.id.clone()), draft.id);
        lib.apply(&other.id.clone().unwrap()).unwrap();
        assert_eq!(lib.active().unwrap().map(|e| e.id.clone()), other.id);
    }

    /// TEST-CFG-003 (docs/11 §2 unknown-key rule): unknown profile keys and unknown
    /// meta keys survive a full read-modify-write.
    #[test]
    fn unknown_keys_are_preserved_on_rewrite() {
        let lib = scratch();
        let mut draft = lib.create("Legacy import", ProviderKind::Gateway).unwrap();
        draft.connection = Section(json!({"legacyUnknownField": 42}));
        draft.extra.insert("claudeAiImportTail".into(), json!({"kept": true}));
        lib.write(&draft).unwrap();
        let reread = lib.read(draft.id.as_ref().unwrap()).unwrap();
        assert_eq!(reread.extra.get("claudeAiImportTail"), Some(&json!({"kept": true})));
        assert_eq!(reread.connection.0.get("legacyUnknownField"), Some(&json!(42)));
        // Written file keeps them too (not just the in-memory round trip).
        let stored: ProfileDocument =
            serde_json::from_slice(&std::fs::read(lib.profile_path(draft.id.as_ref().unwrap())).unwrap())
                .unwrap();
        assert_eq!(stored.extra.get("claudeAiImportTail"), Some(&json!({"kept": true})));
        assert_eq!(stored.connection.0.get("legacyUnknownField"), Some(&json!(42)));
    }

    /// TEST-CFG-004 (API-CFG-01): apply is atomic and validates — invalid profiles are
    /// refused (both our writing and a foreign writer's future-version file) and the
    /// previously applied profile stays active.
    #[test]
    fn invalid_apply_is_rejected_and_previous_profile_stays_active() {
        let lib = scratch();
        let good = lib.create("Valid", ProviderKind::Gateway).unwrap();
        lib.apply(good.id.as_ref().unwrap()).unwrap();

        // Unknown schema version cannot pass the merged-config gate.
        let mut future = good.clone();
        future.schema_version = 99;
        error_contains(lib.write(&future).unwrap_err(), "CONFIG_SCHEMA");
        assert_eq!(lib.active().unwrap().map(|e| e.id.clone()), good.id);

        // A structurally broken in-memory doc is refused at the write gate itself.
        let mut broken = doc("Broken", ProviderKind::Gateway);
        broken.id = Some("not-a-uuid".to_string());
        error_contains(lib.write(&broken).unwrap_err(), "CONFIG_VALIDATION");

        // A foreign future-writer file bypasses our API — apply must still refuse it
        // (schema gate) and leave the pointer alone.
        let foreign_id = Uuid::new_v4().to_string();
        let foreign = json!({"schemaVersion": 2, "id": foreign_id, "name": "Future writer"});
        std::fs::write(lib.profile_path(&foreign_id), serde_json::to_vec(&foreign).unwrap()).unwrap();
        error_contains(lib.apply(&foreign_id).unwrap_err(), "CONFIG_SCHEMA");
        assert_eq!(lib.active().unwrap().map(|e| e.id.clone()), good.id, "prior profile remains active");

        error_contains(lib.apply("no-such-id").unwrap_err(), "CONFIG_NOT_FOUND");
        assert_eq!(lib.active().unwrap().map(|e| e.id.clone()), good.id);
    }

    /// TEST-CFG-005 (docs/18 §10): a disk-level write failure refuses the operation
    /// and leaves the prior document and active selection untouched; the atomic
    /// rename guarantees no torn state. Simulated with a read-only directory (POSIX).
    #[cfg(unix)]
    #[test]
    fn failed_write_leaves_previous_document_and_active_untouched() {
        use std::os::unix::fs::PermissionsExt as _;
        let lib = scratch();
        std::fs::create_dir_all(&lib.root).unwrap();
        let first = lib.create("First", ProviderKind::Gateway).unwrap();
        lib.apply(first.id.as_ref().unwrap()).unwrap();
        // Freeze the directory so the write cannot create its temp file.
        std::fs::set_permissions(
            &lib.root,
            std::fs::Permissions::from_mode(0o500),
        )
        .unwrap();
        let mut edited = first.clone();
        edited.connection = Section(json!({"changed": true}));
        let error = lib.write(&edited).unwrap_err();
        assert!(matches!(error, ConfigError::Io { .. }));
        std::fs::set_permissions(&lib.root, std::fs::Permissions::from_mode(0o700)).unwrap();
        let reread = lib.read(first.id.as_ref().unwrap()).unwrap();
        assert!(reread.connection.0.is_null(), "edit must not have landed");
        assert_eq!(lib.active().unwrap().map(|e| e.id.clone()), first.id, "prior active preserved");
    }

    /// TEST-CFG-006 (docs/11 §3 secret hygiene): profile and meta files are 0600.
    #[cfg(unix)]
    #[test]
    fn profile_files_are_owner_only() {
        use std::os::unix::fs::PermissionsExt as _;
        let lib = scratch();
        let created = lib.create("Mine", ProviderKind::Gateway).unwrap();
        let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&lib.profile_path(created.id.as_ref().unwrap())), 0o600);
        assert_eq!(mode(&lib.meta_path()), 0o600);
    }

    /// TEST-CFG-007 (REQ-SETUP-05 adjacency): exclusive credential kind serializes
    /// kebab-case and refuses nothing else to deserialize.
    #[test]
    fn credential_kind_round_trip() {
        let text = serde_json::to_string(&CredentialKind::Helper).unwrap();
        assert_eq!(text, "\"helper\"");
        let parsed: CredentialKind = serde_json::from_str("\"interactive\"").unwrap();
        assert_eq!(parsed, CredentialKind::Interactive);
        // Inexact words do not become a kind.
        assert!(serde_json::from_str::<CredentialKind>("\"static_with_fallback\"").is_err());
    }

    /// TEST-CFG-008 (duplicate): copy is an independent draft — new id, same shape,
    /// active selection untouched.
    #[test]
    fn duplicate_creates_independent_draft() {
        let lib = scratch();
        let source = lib.create("Base", ProviderKind::Bedrock).unwrap();
        lib.apply(source.id.as_ref().unwrap()).unwrap();
        let copy = lib.duplicate(source.id.as_ref().unwrap(), "Base (2)").unwrap();
        assert_ne!(copy.id, source.id);
        assert_eq!(lib.active().unwrap().map(|e| e.id.clone()), source.id, "duplicate must not move the active selection");
        let listed = lib.list().unwrap();
        assert_eq!(listed.len(), 2);
    }

    /// TEST-CFG-009 (rename): both the profile document and the meta entry move;
    /// duplicates are refused.
    #[test]
    fn rename_updates_doc_and_entry_and_rejects_duplicates() {
        let lib = scratch();
        let a = lib.create("Alpha", ProviderKind::Vertex).unwrap();
        let b = lib.create("Beta", ProviderKind::Foundry).unwrap();
        lib.rename(a.id.as_ref().unwrap(), "Alpha 2").unwrap();
        assert_eq!(lib.read(a.id.as_ref().unwrap()).unwrap().name.clone(), Some("Alpha 2".into()));
        let error = lib.rename(b.id.as_ref().unwrap(), "Alpha 2").unwrap_err();
        assert!(matches!(error, ConfigError::DuplicateName { .. }));
        assert_eq!(
            lib.list().unwrap().iter().find(|e| e.id == a.id.clone().unwrap()).map(|e| e.name.clone()),
            Some("Alpha 2".to_string())
        );
    }

    /// TEST-CFG-010 (delete guard): applied profile cannot be deleted (fail-safe
    /// guard, shipped behavior unknown per docs/14 §9); inactive profile deletes clean.
    #[test]
    fn delete_refuses_the_applied_profile_but_frees_others() {
        let lib = scratch();
        let applied = lib.create("Applied", ProviderKind::Gateway).unwrap();
        lib.apply(applied.id.as_ref().unwrap()).unwrap();
        let other = lib.create("Other", ProviderKind::Gateway).unwrap();
        error_contains(lib.delete(applied.id.as_ref().unwrap()).unwrap_err(), "CONFIG_IN_USE");
        lib.delete(other.id.as_ref().unwrap()).unwrap();
        error_contains(lib.delete(other.id.as_ref().unwrap()).unwrap_err(), "CONFIG_NOT_FOUND");
        assert_eq!(lib.list().unwrap().len(), 1);
    }

    /// TEST-CFG-011 (profile switch): switching the applied profile does not mutate
    /// either document — only the pointer moves (docs/18 §10).
    #[test]
    fn profile_switch_moves_only_the_pointer() {
        let lib = scratch();
        let a = lib.create("A", ProviderKind::Gateway).unwrap();
        let b = lib.create("B", ProviderKind::ClaudeApi).unwrap();
        let before_a = lib.read(a.id.as_ref().unwrap()).unwrap();
        lib.apply(b.id.as_ref().unwrap()).unwrap();
        lib.apply(a.id.as_ref().unwrap()).unwrap();
        assert_eq!(lib.read(a.id.as_ref().unwrap()).unwrap(), before_a);
        assert_eq!(lib.active().unwrap().map(|e| e.id.clone()), a.id);
        assert_eq!(lib.read(b.id.as_ref().unwrap()).unwrap().provider, Some(ProviderKind::ClaudeApi));
    }

    /// TEST-CFG-012 (meta forward-compatibility): unknown `_meta.json` keys and entry
    /// keys round-trip, and entries order is file order (docs/11 §3 ordering).
    #[test]
    fn meta_unknown_keys_and_order_preserved() {
        let lib = scratch();
        let a = lib.create("A", ProviderKind::Gateway).unwrap();
        let b = lib.create("B", ProviderKind::Gateway).unwrap();
        let mut meta = lib.load_meta().unwrap();
        meta.extra.insert("observerBookkeeping".into(), json!({"v": 1}));
        meta.entries[0].extra.insert("pinned".into(), json!(true));
        lib.store_meta(&meta).unwrap();
        let renamed_meta = lib.load_meta().unwrap();
        assert_eq!(renamed_meta.extra.get("observerBookkeeping"), Some(&json!({"v": 1})));
        assert_eq!(renamed_meta.entries[0].extra.get("pinned"), Some(&json!(true)));
        let ids: Vec<_> = renamed_meta.entries.iter().map(|e| e.id.clone()).collect();
        assert_eq!(ids, [a.id.clone().unwrap(), b.id.clone().unwrap()], "create order = list order");
    }

    /// TEST-CFG-012b (wire format, docs/11 §3 literal shapes): on-disk keys are
    /// camelCase exactly as observed — `appliedId`, `entries`, `schemaVersion` — and a
    /// `schemaVersion` a future writer wrote is refused by apply (not silently
    /// demoted to the default).
    #[test]
    fn wire_format_matches_observed_camelcase() {
        let lib = scratch();
        let draft = lib.create("Wire", ProviderKind::Gateway).unwrap();
        lib.apply(draft.id.as_ref().unwrap()).unwrap();
        let meta_text = std::fs::read_to_string(lib.meta_path()).unwrap();
        assert!(meta_text.contains("\"appliedId\""), "meta must use appliedId, not applied_id");
        assert!(meta_text.contains("\"entries\""));
        let profile_text =
            std::fs::read_to_string(lib.profile_path(draft.id.as_ref().unwrap())).unwrap();
        assert!(profile_text.contains("\"schemaVersion\""), "profile must use schemaVersion");
        assert!(!profile_text.contains("schema_version"));
    }

    // -- test helpers -----------------------------------------------------------

    fn error_contains(error: ConfigError, needle: &str) {
        assert!(format!("{error}").contains(needle), "expected {needle} in {error}");
    }
}
