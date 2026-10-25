//! Skills: manifests, discovery, path-confined assets, and selection —
//! roadmap Phase 5 (`docs/13`; `docs/features/mcp-skills-tools.md` §6).
//!
//! A skill is **instructions and assets**, never a model capability and never
//! a way around tool policy (`REQ-TOOL-06`). This crate deliberately does not
//! execute skill scripts. It can list a skill, read a declared asset, and
//! build the structured instruction block a turn carries.
//!
//! What is observed: bundled skill *names* and the existence of user/org/
//! plugin variants (`docs/features/mcp-skills-tools.md` §6). What was not
//! observed: the manifest file shape, the exact mention syntax, and the
//! bundled bodies. Those are rebuild-defined here and marked as such:
//!
//! - a skill is a directory with `manifest.json` (or a bundled placeholder
//!   with no body);
//! - duplicate names resolve by source precedence `Org > User > Plugin >
//!   Bundled` (a rebuild decision; the shipped precedence was not captured);
//! - a selection is a bounded text block beginning `[skill: <name>]`;
//! - scripts are never run; a manifest that points at one still only exposes
//!   it as a file the agent may read under the session's folder grants.
//!
//! Security rules:
//!
//! - names and relative paths are validated; absolute paths, `..`, and
//!   symlinks that escape the skill root are refused;
//! - only files declared by the manifest can be read, each capped at
//!   [`MAX_ASSET_BYTES`];
//! - manifests are capped at [`MAX_MANIFEST_BYTES`] and the catalog at
//!   [`MAX_SKILLS`];
//! - a malformed skill is skipped with a warning, never allowed to abort the
//!   rest of the catalog.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Largest manifest this crate will parse.
pub const MAX_MANIFEST_BYTES: usize = 64 * 1024;

/// Largest declared asset this crate will read.
pub const MAX_ASSET_BYTES: usize = 1024 * 1024;

/// Largest selection block this crate will build.
pub const MAX_SELECTION_BYTES: usize = 16 * 1024;

/// Largest catalog this crate will load from one root.
pub const MAX_SKILLS: usize = 1024;

/// Where a skill came from. Precedence is highest first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SkillSource {
    /// Mounted by an organization; read-only.
    Org,
    /// Installed by the user.
    User,
    /// Bundled by a plugin.
    Plugin,
    /// Shipped with the app.
    Bundled,
}

impl SkillSource {
    fn precedence(self) -> u8 {
        match self {
            SkillSource::Org => 4,
            SkillSource::User => 3,
            SkillSource::Plugin => 2,
            SkillSource::Bundled => 1,
        }
    }
}

/// A skill manifest. `instructions` is inline text; `entry`/`files` are
/// relative paths inside the skill directory. No field is trusted to be a
/// path until it is validated.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillManifest {
    /// Stable identifier (`[a-z0-9][a-z0-9._-]{0,63}`).
    pub name: String,
    /// Human-readable name.
    #[serde(default)]
    pub display_name: String,
    /// One-line description.
    #[serde(default)]
    pub description: String,
    /// Manifest version, for display only.
    #[serde(default = "default_version")]
    pub version: String,
    /// Inline instructions inserted into the turn when selected.
    #[serde(default)]
    pub instructions: Option<String>,
    /// The skill's main file, if it has one.
    #[serde(default)]
    pub entry: Option<String>,
    /// Files the agent may read. `entry` is always readable even if omitted
    /// from this list.
    #[serde(default)]
    pub files: Vec<String>,
    /// True when the manifest declares a script. The crate never runs it.
    #[serde(default)]
    pub has_script: bool,
}

fn default_version() -> String {
    "0.0.0".to_string()
}

/// A loaded skill and the root it came from.
#[derive(Clone, Debug)]
pub struct Skill {
    manifest: SkillManifest,
    source: SkillSource,
    root: Option<PathBuf>,
}

impl Skill {
    /// The validated manifest.
    pub fn manifest(&self) -> &SkillManifest {
        &self.manifest
    }

    /// The source.
    pub fn source(&self) -> SkillSource {
        self.source
    }

    /// True when there is something to insert into a turn. A placeholder
    /// (manifest with no instructions and no readable entry) is listed but
    /// not runnable; the shipped bundled bodies were not captured.
    pub fn is_runnable(&self) -> bool {
        self.manifest.instructions.is_some()
            || self
                .manifest
                .entry
                .as_ref()
                .is_some_and(|entry| self.root.is_some() && read_declared(self, entry).is_ok())
    }

    /// The skill root, if it has one (bundled placeholders do not).
    pub fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }

    /// Read one declared asset. `relative` must be the entry or a declared
    /// file; anything else is refused before touching the filesystem.
    pub fn read_asset(&self, relative: &str) -> Result<Vec<u8>, SkillError> {
        let Some(root) = &self.root else {
            return Err(SkillError::NotRunnable(self.manifest.name.clone()));
        };
        if !self.declared(relative) {
            return Err(SkillError::UndeclaredAsset(relative.to_string()));
        }
        read_confined(root, relative)
    }

    fn declared(&self, relative: &str) -> bool {
        self.manifest.entry.as_deref() == Some(relative)
            || self.manifest.files.iter().any(|file| file == relative)
    }
}

/// A renderer-safe catalog row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillView {
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub version: String,
    pub source: SkillSource,
    pub runnable: bool,
    pub has_script: bool,
}

impl From<&Skill> for SkillView {
    fn from(skill: &Skill) -> SkillView {
        SkillView {
            name: skill.manifest.name.clone(),
            display_name: if skill.manifest.display_name.is_empty() {
                skill.manifest.name.clone()
            } else {
                skill.manifest.display_name.clone()
            },
            description: skill.manifest.description.clone(),
            version: skill.manifest.version.clone(),
            source: skill.source,
            runnable: skill.is_runnable(),
            has_script: skill.manifest.has_script,
        }
    }
}

/// A selection to insert into a turn. It is data; the agent loop decides how
/// to place it in the request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillSelection {
    pub name: String,
    pub instructions: String,
}

/// Skill failures. Messages carry names/paths, never file contents.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SkillError {
    #[error("the skill name is invalid: {0}")]
    InvalidName(String),
    #[error("the skill manifest is invalid: {0}")]
    InvalidManifest(String),
    #[error("the skill {0} is a catalog placeholder and has no captured body")]
    NotRunnable(String),
    #[error("the skill asset {0} is not declared by the manifest")]
    UndeclaredAsset(String),
    #[error("the skill asset path is unsafe: {0}")]
    UnsafePath(String),
    #[error("the skill asset {0} is too large")]
    AssetTooLarge(String),
    #[error("the skill file could not be read: {0}")]
    Io(String),
}

/// The loaded catalog plus loader warnings.
#[derive(Debug, Default)]
pub struct SkillRegistry {
    skills: BTreeMap<String, Skill>,
    warnings: Vec<String>,
}

impl SkillRegistry {
    /// Load every skill directory under `root` with the given source.
    ///
    /// A missing root is empty, not an error. A malformed skill is skipped
    /// and reported. Duplicate names inside one load resolve to the first
    /// directory in lexical order; duplicates across sources resolve by
    /// [`SkillSource::precedence`].
    pub fn load(root: &Path, source: SkillSource) -> SkillRegistry {
        let mut registry = SkillRegistry::default();
        registry.merge_root(root, source);
        registry
    }

    /// Add the documented bundled names as placeholders. Their bodies were
    /// not captured, so they are listed but explicitly not runnable.
    pub fn with_bundled_placeholders(mut self) -> SkillRegistry {
        for (name, description) in [
            ("docx", "Create and edit Word documents."),
            ("pdf", "Generate and manipulate PDF documents."),
            ("pdf-reading", "Extract and read PDF documents."),
            ("pptx", "Create and edit PowerPoint presentations."),
            ("xlsx", "Create and edit Excel workbooks."),
            ("frontend-design", "Design and implementation guidance."),
            (
                "consolidate-memory",
                "Consolidate session memory (observed catalog entry).",
            ),
            ("context", "Context guidance (observed catalog entry)."),
            ("explain-usage", "Explain usage (observed catalog entry)."),
            (
                "find-skills",
                "Find available skills (observed catalog entry).",
            ),
            ("schedule", "Scheduling guidance (observed catalog entry)."),
            ("setup-claude", "Setup guidance (observed catalog entry)."),
        ] {
            let manifest = SkillManifest {
                name: name.to_string(),
                display_name: name.replace('-', " "),
                description: description.to_string(),
                version: "bundled".to_string(),
                instructions: None,
                entry: None,
                files: Vec::new(),
                has_script: false,
            };
            let skill = Skill {
                manifest,
                source: SkillSource::Bundled,
                root: None,
            };
            self.insert(skill);
        }
        self
    }

    /// Merge another root into this registry.
    pub fn merge_root(&mut self, root: &Path, source: SkillSource) {
        let entries = match fs::read_dir(root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
            Err(error) => {
                self.warnings
                    .push(format!("the skills directory could not be read: {error}"));
                return;
            }
        };
        let mut directories: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            // A symlinked skill directory could point outside the catalog;
            // scan only real directories. Asset reads canonicalize again.
            .filter(|path| {
                fs::symlink_metadata(path)
                    .map(|metadata| metadata.file_type().is_dir())
                    .unwrap_or(false)
            })
            .collect();
        directories.sort();
        for directory in directories.into_iter().take(MAX_SKILLS) {
            let manifest_path = directory.join("manifest.json");
            match load_skill(&directory, &manifest_path, source) {
                Ok(skill) => self.insert(skill),
                Err(error) => self.warnings.push(format!(
                    "the skill in {} was skipped: {error}",
                    directory.display()
                )),
            }
        }
    }

    /// Load one skill directory (a directory containing `manifest.json`), or
    /// scan it as a parent if it does not. Plugin manifests point at either a
    /// single skill directory or a parent; both shapes are accepted.
    pub fn merge_skill_or_root(&mut self, directory: &Path, source: SkillSource) {
        let real_directory = fs::symlink_metadata(directory)
            .map(|metadata| metadata.file_type().is_dir())
            .unwrap_or(false);
        if !real_directory {
            self.warnings.push(format!(
                "the skill path {} is not a real directory and was skipped",
                directory.display()
            ));
            return;
        }
        if directory.join("manifest.json").is_file() {
            match load_skill(directory, &directory.join("manifest.json"), source) {
                Ok(skill) => self.insert(skill),
                Err(error) => self.warnings.push(format!(
                    "the skill in {} was skipped: {error}",
                    directory.display()
                )),
            }
        } else {
            self.merge_root(directory, source);
        }
    }

    /// Insert with source precedence. A lower-precedence duplicate is kept
    /// out of the catalog; the duplicate is reported.
    pub fn insert(&mut self, skill: Skill) {
        if let Some(existing) = self.skills.get(&skill.manifest.name) {
            if existing.source.precedence() >= skill.source.precedence() {
                self.warnings.push(format!(
                    "the skill {:?} from {:?} was ignored because a higher-precedence source already provides it",
                    skill.manifest.name, skill.source
                ));
                return;
            }
        }
        self.skills.insert(skill.manifest.name.clone(), skill);
    }

    /// Catalog rows, sorted by name.
    pub fn catalog(&self) -> Vec<SkillView> {
        self.skills.values().map(SkillView::from).collect()
    }

    /// Loader warnings.
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// Look up one skill.
    pub fn get(&self, name: &str) -> Option<&Skill> {
        self.skills.get(name)
    }

    /// Build the instruction block for a selection.
    ///
    /// Inline instructions win. Otherwise the declared entry file is read
    /// (bounded) and used as the instruction text. A placeholder is an error,
    /// not an empty selection.
    pub fn select(&self, name: &str) -> Result<SkillSelection, SkillError> {
        let skill = self
            .skills
            .get(name)
            .ok_or_else(|| SkillError::InvalidName(name.to_string()))?;
        let body = if let Some(instructions) = &skill.manifest.instructions {
            instructions.clone()
        } else if let Some(entry) = &skill.manifest.entry {
            let bytes = skill.read_asset(entry)?;
            String::from_utf8(bytes).map_err(|_| {
                SkillError::InvalidManifest("the skill entry is not valid UTF-8".to_string())
            })?
        } else {
            return Err(SkillError::NotRunnable(name.to_string()));
        };
        let mut instructions = format!("[skill: {}]\n{}", skill.manifest.name, body);
        if instructions.len() > MAX_SELECTION_BYTES {
            let boundary = floor_char_boundary(&instructions, MAX_SELECTION_BYTES);
            instructions.truncate(boundary);
            instructions.push_str("\n[truncated]");
        }
        Ok(SkillSelection {
            name: skill.manifest.name.clone(),
            instructions,
        })
    }

    /// Remove a skill from the catalog (disablement is a view, but removal is
    /// useful for tests and uninstall).
    pub fn remove(&mut self, name: &str) -> Option<Skill> {
        self.skills.remove(name)
    }
}

fn load_skill(root: &Path, manifest_path: &Path, source: SkillSource) -> Result<Skill, SkillError> {
    let root = fs::canonicalize(root).map_err(|error| SkillError::Io(error.to_string()))?;
    let manifest_path =
        fs::canonicalize(manifest_path).map_err(|error| SkillError::Io(error.to_string()))?;
    if !manifest_path.starts_with(&root) {
        return Err(SkillError::UnsafePath(
            "the manifest resolves outside the skill directory".to_string(),
        ));
    }
    let bytes = fs::read(&manifest_path).map_err(|error| SkillError::Io(error.to_string()))?;
    if bytes.len() > MAX_MANIFEST_BYTES {
        return Err(SkillError::InvalidManifest(
            "the manifest is larger than the limit".to_string(),
        ));
    }
    let manifest: SkillManifest = serde_json::from_slice(&bytes)
        .map_err(|error| SkillError::InvalidManifest(error.to_string()))?;
    validate_name(&manifest.name)?;
    for file in manifest.files.iter().chain(manifest.entry.iter()) {
        validate_relative(file)?;
    }
    Ok(Skill {
        manifest,
        source,
        root: Some(root),
    })
}

fn validate_name(name: &str) -> Result<(), SkillError> {
    if name.is_empty()
        || name.len() > 64
        || !name.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '-' | '_' | '.')
        })
        || name.starts_with(['-', '_', '.'])
    {
        return Err(SkillError::InvalidName(name.to_string()));
    }
    Ok(())
}

fn validate_relative(path: &str) -> Result<(), SkillError> {
    if path.is_empty() || path.contains('\0') {
        return Err(SkillError::UnsafePath(path.to_string()));
    }
    let candidate = Path::new(path);
    if candidate.is_absolute()
        || candidate
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(SkillError::UnsafePath(path.to_string()));
    }
    Ok(())
}

fn floor_char_boundary(value: &str, mut index: usize) -> usize {
    while index > 0 && !value.is_char_boundary(index) {
        index -= 1;
    }
    index
}

fn read_declared(skill: &Skill, relative: &str) -> Result<Vec<u8>, SkillError> {
    let Some(root) = &skill.root else {
        return Err(SkillError::NotRunnable(skill.manifest.name.clone()));
    };
    read_confined(root, relative)
}

fn read_confined(root: &Path, relative: &str) -> Result<Vec<u8>, SkillError> {
    validate_relative(relative)?;
    let root = fs::canonicalize(root).map_err(|error| SkillError::Io(error.to_string()))?;
    let candidate = root.join(relative);
    let canonical =
        fs::canonicalize(&candidate).map_err(|error| SkillError::Io(error.to_string()))?;
    if !canonical.starts_with(&root) {
        return Err(SkillError::UnsafePath(relative.to_string()));
    }
    let metadata = fs::metadata(&canonical).map_err(|error| SkillError::Io(error.to_string()))?;
    if !metadata.is_file() {
        return Err(SkillError::UnsafePath(relative.to_string()));
    }
    if metadata.len() > MAX_ASSET_BYTES as u64 {
        return Err(SkillError::AssetTooLarge(relative.to_string()));
    }
    fs::read(&canonical).map_err(|error| SkillError::Io(error.to_string()))
}

/// Global/skill enablement. `skillCreationEnabled` gates creation/upload; it
/// never gates selecting an already-installed skill.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillSettings {
    #[serde(default)]
    pub skill_creation_enabled: bool,
    #[serde(default)]
    pub disabled: BTreeSet<String>,
}

impl SkillSettings {
    /// Visible catalog rows: placeholders remain visible (they are real
    /// catalog entries) but `runnable` is false.
    pub fn visible<'a>(&self, registry: &'a SkillRegistry) -> Vec<&'a Skill> {
        registry
            .skills
            .values()
            .filter(|skill| !self.disabled.contains(&skill.manifest.name))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    fn write_skill(root: &Path, name: &str, manifest: serde_json::Value, files: &[(&str, &str)]) {
        let directory = root.join(name);
        fs::create_dir_all(&directory).unwrap();
        fs::write(
            directory.join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        for (path, content) in files {
            let target = directory.join(path);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(target, content).unwrap();
        }
    }

    #[test]
    fn a_manifest_skill_loads_and_reads_only_declared_files() {
        let dir = TempDir::new().unwrap();
        write_skill(
            dir.path(),
            "writer",
            json!({
                "name": "writer",
                "description": "writes",
                "entry": "SKILL.md",
                "files": ["assets/template.txt"]
            }),
            &[
                ("SKILL.md", "# Writer\nUse the template."),
                ("assets/template.txt", "template"),
                ("secret.txt", "do not read"),
            ],
        );
        let registry = SkillRegistry::load(dir.path(), SkillSource::User);
        assert!(registry.warnings().is_empty());
        let skill = registry.get("writer").unwrap();
        assert!(skill.is_runnable());
        assert_eq!(
            skill.read_asset("assets/template.txt").unwrap(),
            b"template"
        );
        assert!(matches!(
            skill.read_asset("secret.txt"),
            Err(SkillError::UndeclaredAsset(_))
        ));
        let selection = registry.select("writer").unwrap();
        assert!(selection.instructions.starts_with("[skill: writer]"));
    }

    #[test]
    fn path_traversal_is_refused() {
        let dir = TempDir::new().unwrap();
        write_skill(
            dir.path(),
            "bad",
            json!({"name": "bad", "entry": "../outside.txt"}),
            &[("SKILL.md", "x")],
        );
        let registry = SkillRegistry::load(dir.path(), SkillSource::User);
        assert!(registry.get("bad").is_none());
        assert!(registry.warnings()[0].contains("unsafe"));
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_escape_is_refused() {
        let dir = TempDir::new().unwrap();
        write_skill(
            dir.path(),
            "linked",
            json!({"name": "linked", "files": ["link.txt"]}),
            &[],
        );
        let outside = dir.path().join("outside.txt");
        fs::write(&outside, "outside").unwrap();
        std::os::unix::fs::symlink(&outside, dir.path().join("linked/link.txt")).unwrap();
        let registry = SkillRegistry::load(dir.path(), SkillSource::User);
        let skill = registry.get("linked").unwrap();
        assert!(matches!(
            skill.read_asset("link.txt"),
            Err(SkillError::UnsafePath(_))
        ));
    }

    #[test]
    fn an_org_skill_wins_over_a_user_skill_with_the_same_name() {
        let user = TempDir::new().unwrap();
        let org = TempDir::new().unwrap();
        write_skill(
            user.path(),
            "same",
            json!({"name": "same", "instructions": "user"}),
            &[],
        );
        write_skill(
            org.path(),
            "same",
            json!({"name": "same", "instructions": "org"}),
            &[],
        );
        let mut registry = SkillRegistry::load(user.path(), SkillSource::User);
        registry.merge_root(org.path(), SkillSource::Org);
        assert_eq!(registry.get("same").unwrap().source(), SkillSource::Org);
        assert_eq!(
            registry.select("same").unwrap().instructions,
            "[skill: same]\norg"
        );
    }

    #[test]
    fn a_placeholder_is_listed_but_not_runnable() {
        let registry = SkillRegistry::default().with_bundled_placeholders();
        let view = registry
            .catalog()
            .into_iter()
            .find(|view| view.name == "docx")
            .unwrap();
        assert!(!view.runnable);
        assert!(matches!(
            registry.select("docx"),
            Err(SkillError::NotRunnable(_))
        ));
    }

    #[test]
    fn disabled_skills_are_hidden_from_the_visible_catalog() {
        let dir = TempDir::new().unwrap();
        write_skill(
            dir.path(),
            "one",
            json!({"name": "one", "instructions": "x"}),
            &[],
        );
        let registry = SkillRegistry::load(dir.path(), SkillSource::User);
        let mut settings = SkillSettings::default();
        settings.disabled.insert("one".into());
        assert!(settings.visible(&registry).is_empty());
    }

    #[test]
    fn an_oversized_asset_is_refused_without_reading_it() {
        let dir = TempDir::new().unwrap();
        write_skill(
            dir.path(),
            "big",
            json!({"name": "big", "files": ["big.bin"]}),
            &[],
        );
        fs::write(
            dir.path().join("big/big.bin"),
            vec![0u8; MAX_ASSET_BYTES + 1],
        )
        .unwrap();
        let registry = SkillRegistry::load(dir.path(), SkillSource::User);
        assert!(matches!(
            registry.get("big").unwrap().read_asset("big.bin"),
            Err(SkillError::AssetTooLarge(_))
        ));
    }
}
