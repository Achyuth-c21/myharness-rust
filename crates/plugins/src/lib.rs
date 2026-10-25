//! Plugins: manifests, local installs, org bundles, enablement, and the
//! MCP/skill registrations a plugin contributes — roadmap Phase 5
//! (`docs/13`; `docs/features/mcp-skills-tools.md` §7).
//!
//! The shipped marketplace and `.dxt`/`.mcpb` installer were not captured, so
//! this crate implements only what the rebuild can state honestly:
//!
//! - a **local manifest** format (`plugin.json`) for marketplace/org/local
//!   sources;
//! - install from a local directory into a managed root, with path
//!   confinement, size caps, and no symlink following;
//! - org bundles are **read-only**: uninstall is refused;
//! - a plugin's MCP servers and skills are registered through the normal MCP
//!   policy/approval path; a disabled plugin contributes nothing to a new
//!   session;
//! - signature policy is explicit. Cryptographic verification is **not**
//!   implemented; a plugin that requires a signature is refused rather than
//!   marked verified. An optional signature is reported as `unverified`.
//!
//! Network marketplaces and desktop-extension execution are not implemented
//! here; they are named as unavailable rather than emulated (`docs/14` §9).
//!
//! Secrets: a plugin manifest may not carry env/header **values**. Credentials
//! belong in the MCP settings/keychain path. A non-empty value is refused at
//! load/install time, and the renderer-facing [`PluginView`] never includes
//! transport configuration.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use myharness_mcp::ServerConfig;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Largest plugin manifest.
pub const MAX_PLUGIN_MANIFEST_BYTES: usize = 128 * 1024;

/// Largest number of plugins in one registry.
pub const MAX_PLUGINS: usize = 512;

/// Largest file count copied by one install.
pub const MAX_INSTALL_FILES: usize = 4096;

/// Largest total install size.
pub const MAX_INSTALL_BYTES: u64 = 64 * 1024 * 1024;

/// Where a plugin came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PluginSource {
    Marketplace,
    Org,
    Local,
}

impl PluginSource {
    fn read_only(self) -> bool {
        matches!(self, PluginSource::Org)
    }
}

/// A signature block. The value is opaque here; this crate does not verify it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSignature {
    pub algorithm: String,
    pub key_id: String,
    pub value: String,
}

/// Whether a signature is mandatory for install.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SignaturePolicy {
    /// Unsigned plugins may install; a present signature is `unverified`.
    #[default]
    Optional,
    /// Only a cryptographically verified signature may install. Since
    /// verification is not implemented, every install is refused in this
    /// mode — fail-closed, never "trusted because it has a signature string".
    Required,
}

/// What the registry knows about a signature.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SignatureStatus {
    Unsigned,
    /// A signature was present but could not be verified by this build.
    Unverified,
}

/// A command a plugin advertises. Execution is not implemented; listing it
/// lets the UI show what the plugin contributes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginCommand {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

/// The plugin manifest (`plugin.json`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifest {
    /// Stable identifier (`[a-z0-9][a-z0-9._-]{0,127}`).
    pub name: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default)]
    pub description: String,
    /// Declared capabilities, shown before install.
    #[serde(default)]
    pub permissions: Vec<String>,
    /// MCP servers contributed by the plugin, keyed by server-local name.
    #[serde(default)]
    pub mcp_servers: BTreeMap<String, ServerConfig>,
    /// Skill directories relative to the plugin root.
    #[serde(default)]
    pub skills: Vec<String>,
    /// Advertised commands.
    #[serde(default)]
    pub commands: Vec<PluginCommand>,
    /// Optional signature block.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<PluginSignature>,
}

fn default_version() -> String {
    "0.0.0".to_string()
}

/// A loaded plugin.
#[derive(Clone, Debug)]
pub struct Plugin {
    manifest: PluginManifest,
    source: PluginSource,
    root: PathBuf,
    signature: SignatureStatus,
}

impl Plugin {
    pub fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    pub fn source(&self) -> PluginSource {
        self.source
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn signature(&self) -> SignatureStatus {
        self.signature
    }

    pub fn read_only(&self) -> bool {
        self.source.read_only()
    }
}

/// Renderer-safe catalog row: no env/header values, no raw signature value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginView {
    pub name: String,
    pub display_name: String,
    pub version: String,
    pub description: String,
    pub source: PluginSource,
    pub read_only: bool,
    pub signature: SignatureStatus,
    pub permissions: Vec<String>,
    pub servers: Vec<String>,
    pub skills: Vec<String>,
    pub commands: Vec<String>,
}

impl From<&Plugin> for PluginView {
    fn from(plugin: &Plugin) -> PluginView {
        PluginView {
            name: plugin.manifest.name.clone(),
            display_name: if plugin.manifest.display_name.is_empty() {
                plugin.manifest.name.clone()
            } else {
                plugin.manifest.display_name.clone()
            },
            version: plugin.manifest.version.clone(),
            description: plugin.manifest.description.clone(),
            source: plugin.source,
            read_only: plugin.read_only(),
            signature: plugin.signature,
            permissions: plugin.manifest.permissions.clone(),
            servers: plugin.manifest.mcp_servers.keys().cloned().collect(),
            skills: plugin.manifest.skills.clone(),
            commands: plugin
                .manifest
                .commands
                .iter()
                .map(|command| command.name.clone())
                .collect(),
        }
    }
}

/// An MCP server contributed by an enabled plugin, already namespaced as
/// `<plugin>.<server>`.
#[derive(Clone, Debug)]
pub struct PluginMcpServer {
    pub name: String,
    pub plugin: String,
    pub config: ServerConfig,
}

/// A skill directory contributed by an enabled plugin.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginSkillRoot {
    pub plugin: String,
    pub root: PathBuf,
}

/// Enablement. Disabled wins; a plugin is enabled by default.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSettings {
    #[serde(default)]
    pub disabled: BTreeSet<String>,
    /// Signature policy for new installs.
    #[serde(default)]
    pub signature_policy: SignaturePolicy,
}

impl PluginSettings {
    pub fn is_enabled(&self, name: &str) -> bool {
        !self.disabled.contains(name)
    }
}

/// Plugin failures. Messages name paths/plugins, never manifest values.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PluginError {
    #[error("the plugin name is invalid: {0}")]
    InvalidName(String),
    #[error("the plugin manifest is invalid: {0}")]
    InvalidManifest(String),
    #[error("the plugin manifest carries a credential value; configure it in MCP settings")]
    CredentialInManifest,
    #[error("the plugin path is unsafe: {0}")]
    UnsafePath(String),
    #[error("the plugin {0} is already installed")]
    AlreadyInstalled(String),
    #[error("the plugin {0} is read-only and cannot be uninstalled")]
    ReadOnly(String),
    #[error("the plugin {0} is not installed")]
    NotInstalled(String),
    #[error("a signature is required by policy, but signature verification is not implemented")]
    SignatureRequired,
    #[error("the plugin exceeds an install bound")]
    TooLarge,
    #[error("the plugin file operation failed: {0}")]
    Io(String),
}

/// Installed/loaded plugins plus warnings.
#[derive(Debug, Default)]
pub struct PluginRegistry {
    plugins: BTreeMap<String, Plugin>,
    warnings: Vec<String>,
}

impl PluginRegistry {
    /// Load every directory under `root` that contains `plugin.json`.
    pub fn load(root: &Path, source: PluginSource) -> PluginRegistry {
        let mut registry = PluginRegistry::default();
        registry.merge_root(root, source);
        registry
    }

    /// Merge another root into this registry.
    pub fn merge_root(&mut self, root: &Path, source: PluginSource) {
        let entries = match fs::read_dir(root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
            Err(error) => {
                self.warnings
                    .push(format!("the plugin directory could not be read: {error}"));
                return;
            }
        };
        let mut directories: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            // Reject symlinked plugin directories: uninstall must never remove
            // a directory outside the managed root.
            .filter(|path| {
                fs::symlink_metadata(path)
                    .map(|metadata| metadata.file_type().is_dir())
                    .unwrap_or(false)
            })
            .collect();
        directories.sort();
        for directory in directories.into_iter().take(MAX_PLUGINS) {
            let manifest_path = directory.join("plugin.json");
            match load_plugin(&directory, &manifest_path, source) {
                Ok(plugin) => {
                    if self.plugins.contains_key(&plugin.manifest.name) {
                        self.warnings.push(format!(
                            "the plugin {:?} was ignored because it is already loaded",
                            plugin.manifest.name
                        ));
                    } else {
                        self.plugins.insert(plugin.manifest.name.clone(), plugin);
                    }
                }
                Err(error) => self.warnings.push(format!(
                    "the plugin in {} was skipped: {error}",
                    directory.display()
                )),
            }
        }
    }

    /// Install a local plugin directory into `destination_root`.
    ///
    /// The manifest is validated and copied only after all checks pass. The
    /// copy is staged under a temporary name and renamed; no symlinks are
    /// followed. If `policy` is [`SignaturePolicy::Required`], every install
    /// is refused because verification is not implemented.
    pub fn install(
        &mut self,
        source_dir: &Path,
        destination_root: &Path,
        source: PluginSource,
        policy: SignaturePolicy,
    ) -> Result<PluginView, PluginError> {
        if source == PluginSource::Org {
            return Err(PluginError::ReadOnly(
                "an org bundle is mounted read-only".to_string(),
            ));
        }
        let manifest_path = source_dir.join("plugin.json");
        let plugin = load_plugin(source_dir, &manifest_path, source)?;
        if policy == SignaturePolicy::Required {
            return Err(PluginError::SignatureRequired);
        }
        if self.plugins.contains_key(&plugin.manifest.name) {
            return Err(PluginError::AlreadyInstalled(plugin.manifest.name));
        }
        let destination = destination_root.join(&plugin.manifest.name);
        if destination.exists() {
            return Err(PluginError::AlreadyInstalled(plugin.manifest.name));
        }
        fs::create_dir_all(destination_root).map_err(io_error)?;
        let staging = destination_root.join(format!(".{}.installing", plugin.manifest.name));
        if staging.exists() {
            fs::remove_dir_all(&staging).map_err(io_error)?;
        }
        copy_tree(source_dir, &staging)?;
        fs::rename(&staging, &destination).map_err(io_error)?;
        let plugin = load_plugin(&destination, &destination.join("plugin.json"), source)?;
        let view = PluginView::from(&plugin);
        self.plugins.insert(plugin.manifest.name.clone(), plugin);
        Ok(view)
    }

    /// Uninstall a plugin. Org bundles are read-only and refuse removal.
    pub fn uninstall(&mut self, name: &str) -> Result<(), PluginError> {
        let plugin = self
            .plugins
            .get(name)
            .ok_or_else(|| PluginError::NotInstalled(name.to_string()))?;
        if plugin.read_only() {
            return Err(PluginError::ReadOnly(name.to_string()));
        }
        let root = plugin.root.clone();
        fs::remove_dir_all(&root).map_err(io_error)?;
        self.plugins.remove(name);
        Ok(())
    }

    /// Renderer-safe catalog rows.
    pub fn catalog(&self) -> Vec<PluginView> {
        self.plugins.values().map(PluginView::from).collect()
    }

    /// Loader warnings.
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    pub fn get(&self, name: &str) -> Option<&Plugin> {
        self.plugins.get(name)
    }

    /// MCP servers contributed by enabled plugins, namespaced
    /// `<plugin>.<server>`.
    pub fn mcp_servers(&self, settings: &PluginSettings) -> Vec<PluginMcpServer> {
        let mut servers = Vec::new();
        for plugin in self.plugins.values() {
            if !settings.is_enabled(&plugin.manifest.name) {
                continue;
            }
            for (local, config) in &plugin.manifest.mcp_servers {
                let name = format!("{}.{}", plugin.manifest.name, local);
                servers.push(PluginMcpServer {
                    name,
                    plugin: plugin.manifest.name.clone(),
                    config: config.clone(),
                });
            }
        }
        servers.sort_by(|left, right| left.name.cmp(&right.name));
        servers
    }

    /// Skill directories contributed by enabled plugins.
    pub fn skill_roots(&self, settings: &PluginSettings) -> Vec<PluginSkillRoot> {
        let mut roots = Vec::new();
        for plugin in self.plugins.values() {
            if !settings.is_enabled(&plugin.manifest.name) {
                continue;
            }
            for skill in &plugin.manifest.skills {
                if validate_relative(skill).is_ok() {
                    roots.push(PluginSkillRoot {
                        plugin: plugin.manifest.name.clone(),
                        root: plugin.root.join(skill),
                    });
                }
            }
        }
        roots
    }
}

fn load_plugin(
    root: &Path,
    manifest_path: &Path,
    source: PluginSource,
) -> Result<Plugin, PluginError> {
    let root = fs::canonicalize(root).map_err(io_error)?;
    let manifest_path = fs::canonicalize(manifest_path).map_err(io_error)?;
    if !manifest_path.starts_with(&root) {
        return Err(PluginError::UnsafePath(
            "the manifest resolves outside the plugin directory".to_string(),
        ));
    }
    let bytes = fs::read(&manifest_path).map_err(io_error)?;
    if bytes.len() > MAX_PLUGIN_MANIFEST_BYTES {
        return Err(PluginError::TooLarge);
    }
    let manifest: PluginManifest = serde_json::from_slice(&bytes)
        .map_err(|error| PluginError::InvalidManifest(error.to_string()))?;
    validate_name(&manifest.name)?;
    for (local, config) in &manifest.mcp_servers {
        validate_name(local)?;
        let namespaced = format!("{}.{}", manifest.name, local);
        myharness_mcp::validate_server_name(&namespaced)
            .map_err(|error| PluginError::InvalidManifest(error.to_string()))?;
        config
            .validate()
            .map_err(|error| PluginError::InvalidManifest(error.to_string()))?;
        if config_has_secret_value(config) {
            return Err(PluginError::CredentialInManifest);
        }
    }
    for skill in &manifest.skills {
        validate_relative(skill)?;
    }
    let signature = match manifest.signature.as_ref() {
        None => SignatureStatus::Unsigned,
        Some(signature) => {
            if signature.algorithm != "ed25519"
                || signature.key_id.trim().is_empty()
                || signature.value.trim().is_empty()
            {
                return Err(PluginError::InvalidManifest(
                    "the signature block is incomplete or uses an unsupported algorithm"
                        .to_string(),
                ));
            }
            SignatureStatus::Unverified
        }
    };
    Ok(Plugin {
        manifest,
        source,
        root,
        signature,
    })
}

fn config_has_secret_value(config: &ServerConfig) -> bool {
    match config {
        ServerConfig::Stdio { env, .. } => env.values().any(|value| !value.is_empty()),
        ServerConfig::Http { headers, .. } | ServerConfig::Sse { headers, .. } => {
            headers.values().any(|value| !value.is_empty())
        }
    }
}

fn validate_name(name: &str) -> Result<(), PluginError> {
    if name.is_empty()
        || name.len() > 128
        || !name.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '-' | '_' | '.')
        })
        || name.starts_with(['-', '_', '.'])
    {
        return Err(PluginError::InvalidName(name.to_string()));
    }
    Ok(())
}

fn validate_relative(path: &str) -> Result<(), PluginError> {
    if path.is_empty() || path.contains('\0') {
        return Err(PluginError::UnsafePath(path.to_string()));
    }
    let candidate = Path::new(path);
    if candidate.is_absolute()
        || candidate
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(PluginError::UnsafePath(path.to_string()));
    }
    Ok(())
}

fn copy_tree(source: &Path, destination: &Path) -> Result<(), PluginError> {
    fs::create_dir_all(destination).map_err(io_error)?;
    let mut files = 0usize;
    let mut total = 0u64;
    copy_tree_inner(source, destination, &mut files, &mut total)?;
    Ok(())
}

fn copy_tree_inner(
    source: &Path,
    destination: &Path,
    files: &mut usize,
    total: &mut u64,
) -> Result<(), PluginError> {
    let entries = fs::read_dir(source).map_err(io_error)?;
    for entry in entries {
        let entry = entry.map_err(io_error)?;
        let file_type = entry.file_type().map_err(io_error)?;
        if file_type.is_symlink() {
            return Err(PluginError::UnsafePath(entry.path().display().to_string()));
        }
        let target = destination.join(entry.file_name());
        if file_type.is_dir() {
            fs::create_dir_all(&target).map_err(io_error)?;
            copy_tree_inner(&entry.path(), &target, files, total)?;
        } else if file_type.is_file() {
            *files += 1;
            if *files > MAX_INSTALL_FILES {
                return Err(PluginError::TooLarge);
            }
            let metadata = entry.metadata().map_err(io_error)?;
            *total = total.saturating_add(metadata.len());
            if *total > MAX_INSTALL_BYTES {
                return Err(PluginError::TooLarge);
            }
            fs::copy(entry.path(), &target).map_err(io_error)?;
        } else {
            return Err(PluginError::UnsafePath(entry.path().display().to_string()));
        }
    }
    Ok(())
}

fn io_error(error: std::io::Error) -> PluginError {
    PluginError::Io(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    fn write_plugin(root: &Path, name: &str, manifest: serde_json::Value) -> PathBuf {
        let directory = root.join(name);
        fs::create_dir_all(&directory).unwrap();
        fs::write(
            directory.join("plugin.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        directory
    }

    fn stdio() -> serde_json::Value {
        json!({"transport": "stdio", "command": "echo", "args": [], "env": {}})
    }

    #[test]
    fn a_plugin_lists_servers_but_never_transport_values() {
        let dir = TempDir::new().unwrap();
        write_plugin(
            dir.path(),
            "toolbox",
            json!({
                "name": "toolbox",
                "mcpServers": {"echo": stdio()},
                "skills": ["skills/one"],
                "permissions": ["network"]
            }),
        );
        let registry = PluginRegistry::load(dir.path(), PluginSource::Local);
        let view = &registry.catalog()[0];
        assert_eq!(view.servers, vec!["echo"]);
        let json = serde_json::to_string(view).unwrap();
        assert!(!json.contains("transport"));
        assert!(!json.contains("\"command\":"));
    }

    #[test]
    fn a_credential_value_in_a_plugin_manifest_is_refused() {
        let dir = TempDir::new().unwrap();
        write_plugin(
            dir.path(),
            "leaky",
            json!({
                "name": "leaky",
                "mcpServers": {"echo": {
                    "transport": "stdio",
                    "command": "echo",
                    "args": [],
                    "env": {"TOKEN": "sk-secret"}
                }}
            }),
        );
        let registry = PluginRegistry::load(dir.path(), PluginSource::Local);
        assert!(registry.get("leaky").is_none());
        assert!(registry.warnings()[0].contains("credential"));
    }

    #[test]
    fn a_disabled_plugin_contributes_no_servers() {
        let dir = TempDir::new().unwrap();
        write_plugin(
            dir.path(),
            "toolbox",
            json!({"name": "toolbox", "mcpServers": {"echo": stdio()}}),
        );
        let registry = PluginRegistry::load(dir.path(), PluginSource::Local);
        let settings = PluginSettings::default();
        assert_eq!(registry.mcp_servers(&settings).len(), 1);
        assert_eq!(registry.mcp_servers(&settings)[0].name, "toolbox.echo");
        let mut disabled = PluginSettings::default();
        disabled.disabled.insert("toolbox".into());
        assert!(registry.mcp_servers(&disabled).is_empty());
    }

    #[test]
    fn an_org_plugin_cannot_be_uninstalled() {
        let dir = TempDir::new().unwrap();
        write_plugin(dir.path(), "org", json!({"name": "org"}));
        let mut registry = PluginRegistry::load(dir.path(), PluginSource::Org);
        assert!(matches!(
            registry.uninstall("org"),
            Err(PluginError::ReadOnly(_))
        ));
    }

    #[test]
    fn required_signature_policy_refuses_every_install() {
        let source = TempDir::new().unwrap();
        let destination = TempDir::new().unwrap();
        let directory = write_plugin(source.path(), "plugin", json!({"name": "plugin"}));
        let mut registry = PluginRegistry::default();
        assert!(matches!(
            registry.install(
                &directory,
                destination.path(),
                PluginSource::Marketplace,
                SignaturePolicy::Required
            ),
            Err(PluginError::SignatureRequired)
        ));
    }

    #[test]
    fn install_copies_and_uninstall_removes() {
        let source = TempDir::new().unwrap();
        let destination = TempDir::new().unwrap();
        let directory = write_plugin(source.path(), "plugin", json!({"name": "plugin"}));
        fs::write(directory.join("README.md"), "hello").unwrap();
        let mut registry = PluginRegistry::default();
        registry
            .install(
                &directory,
                destination.path(),
                PluginSource::Local,
                SignaturePolicy::Optional,
            )
            .unwrap();
        assert!(destination.path().join("plugin/README.md").exists());
        registry.uninstall("plugin").unwrap();
        assert!(!destination.path().join("plugin").exists());
        assert!(registry.get("plugin").is_none());
    }

    #[test]
    fn an_install_symlink_is_refused() {
        let source = TempDir::new().unwrap();
        let destination = TempDir::new().unwrap();
        let directory = write_plugin(source.path(), "plugin", json!({"name": "plugin"}));
        #[cfg(unix)]
        std::os::unix::fs::symlink("/etc/passwd", directory.join("leak")).unwrap();
        let mut registry = PluginRegistry::default();
        assert!(matches!(
            registry.install(
                &directory,
                destination.path(),
                PluginSource::Local,
                SignaturePolicy::Optional
            ),
            Err(PluginError::UnsafePath(_))
        ));
    }
}
