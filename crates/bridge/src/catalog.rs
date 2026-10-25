//! Skills/plugins catalog composition — roadmap Phase 5 (`docs/13`;
//! `docs/features/mcp-skills-tools.md` §6–§7).
//!
//! This module wires the two registries to disk and to the bridge command
//! surface. It does **not** execute skills or plugin commands, and it does not
//! install from a network marketplace: both are unobserved and are reported as
//! unavailable rather than faked.
//!
//! Layout under the app data directory (rebuild-defined):
//!
//! ```text
//! skills.json          skill settings (creation flag, disabled names)
//! plugins.json         plugin settings (disabled names, signature policy)
//! skills/              user skills
//! plugins/             locally installed plugins
//! org/skills/          read-only org skills
//! org/plugins/         read-only org plugin bundles
//! ```

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use myharness_plugins::{
    PluginMcpServer, PluginRegistry, PluginSettings, PluginSource, PluginView, SignaturePolicy,
};
use myharness_skills::{SkillRegistry, SkillSelection, SkillSettings, SkillSource, SkillView};
use serde::{Deserialize, Serialize};

use crate::mcp::McpConfig;

/// Everything the Skills surface needs.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsView {
    pub skills: Vec<SkillView>,
    pub disabled: BTreeSet<String>,
    pub creation_enabled: bool,
    pub warnings: Vec<String>,
}

/// Everything the Plugins surface needs.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginsView {
    pub plugins: Vec<PluginView>,
    pub disabled: BTreeSet<String>,
    pub signature_policy: SignaturePolicy,
    pub warnings: Vec<String>,
}

/// The catalog service. One instance per process; mutations are serialized.
pub struct CatalogRuntime {
    root: PathBuf,
    plugins_root: PathBuf,
    org_plugins_root: PathBuf,
    user_skills_root: PathBuf,
    org_skills_root: PathBuf,
    skills_file: PathBuf,
    plugins_file: PathBuf,
    skills: Mutex<SkillRegistry>,
    skill_settings: Mutex<SkillSettings>,
    plugins: Mutex<PluginRegistry>,
    plugin_settings: Mutex<PluginSettings>,
    warnings: Mutex<Vec<String>>,
}

impl std::fmt::Debug for CatalogRuntime {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CatalogRuntime")
            .field("root", &self.root)
            .finish_non_exhaustive()
    }
}

impl CatalogRuntime {
    /// Load settings and registries. Missing directories/files are empty;
    /// malformed settings are a warning and defaults.
    pub fn load(root: PathBuf) -> CatalogRuntime {
        let mut warnings = Vec::new();
        let skill_settings = load_json::<SkillSettings>(&root.join("skills.json"), &mut warnings)
            .unwrap_or_default();
        let plugin_settings =
            load_json::<PluginSettings>(&root.join("plugins.json"), &mut warnings)
                .unwrap_or_default();

        let plugins_root = root.join("plugins");
        let org_plugins_root = root.join("org").join("plugins");
        let mut plugins = PluginRegistry::load(&plugins_root, PluginSource::Local);
        plugins.merge_root(&org_plugins_root, PluginSource::Org);

        let user_skills_root = root.join("skills");
        let org_skills_root = root.join("org").join("skills");
        let mut skills = SkillRegistry::default().with_bundled_placeholders();
        for plugin_root in plugins.skill_roots(&plugin_settings) {
            skills.merge_skill_or_root(&plugin_root.root, SkillSource::Plugin);
        }
        skills.merge_root(&user_skills_root, SkillSource::User);
        skills.merge_root(&org_skills_root, SkillSource::Org);

        CatalogRuntime {
            skills_file: root.join("skills.json"),
            plugins_file: root.join("plugins.json"),
            root,
            plugins_root,
            org_plugins_root,
            user_skills_root,
            org_skills_root,
            skills: Mutex::new(skills),
            skill_settings: Mutex::new(skill_settings),
            plugins: Mutex::new(plugins),
            plugin_settings: Mutex::new(plugin_settings),
            warnings: Mutex::new(warnings),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn skills(&self) -> SkillsView {
        // Lock order is settings → skills, matching `select_skill`; the other
        // order can deadlock two concurrent reads.
        let settings = self.skill_settings.lock().expect("skill settings lock");
        let skills = self.skills.lock().expect("skills lock");
        let mut warnings = self.warnings.lock().map(|w| w.clone()).unwrap_or_default();
        warnings.extend(skills.warnings().iter().cloned());
        SkillsView {
            skills: skills.catalog(),
            disabled: settings.disabled.clone(),
            creation_enabled: settings.skill_creation_enabled,
            warnings,
        }
    }

    pub fn select_skill(&self, name: &str) -> Result<SkillSelection, String> {
        let settings = self.skill_settings.lock().expect("skill settings lock");
        if settings.disabled.contains(name) {
            return Err(format!("the skill {name} is disabled"));
        }
        let skills = self.skills.lock().expect("skills lock");
        skills.select(name).map_err(|error| error.to_string())
    }

    pub fn set_skill_enabled(&self, name: &str, enabled: bool) -> Result<(), String> {
        {
            let skills = self.skills.lock().expect("skills lock");
            if skills.get(name).is_none() {
                return Err(format!("the skill {name} is not in the catalog"));
            }
        }
        let mut settings = self.skill_settings.lock().expect("skill settings lock");
        if enabled {
            settings.disabled.remove(name);
        } else {
            settings.disabled.insert(name.to_string());
        }
        write_json(&self.skills_file, &*settings)
    }

    pub fn set_skill_creation_enabled(&self, enabled: bool) -> Result<(), String> {
        let mut settings = self.skill_settings.lock().expect("skill settings lock");
        settings.skill_creation_enabled = enabled;
        write_json(&self.skills_file, &*settings)
    }

    pub fn plugins(&self) -> PluginsView {
        let plugins = self.plugins.lock().expect("plugins lock");
        let settings = self.plugin_settings.lock().expect("plugin settings lock");
        let mut warnings = self.warnings.lock().map(|w| w.clone()).unwrap_or_default();
        warnings.extend(plugins.warnings().iter().cloned());
        PluginsView {
            plugins: plugins.catalog(),
            disabled: settings.disabled.clone(),
            signature_policy: settings.signature_policy,
            warnings,
        }
    }

    /// Install from a local directory. A renderer path is untrusted: it is
    /// canonicalized and must exist, and the manifest/copy bounds still apply.
    pub fn install_plugin(&self, path: &str) -> Result<PluginView, String> {
        let path = PathBuf::from(path);
        if !path.is_absolute() {
            return Err("the plugin path must be absolute".to_string());
        }
        let canonical = fs::canonicalize(&path).map_err(|error| error.to_string())?;
        if !canonical.is_dir() {
            return Err("the plugin path is not a directory".to_string());
        }
        let policy = self
            .plugin_settings
            .lock()
            .expect("plugin settings lock")
            .signature_policy;
        let mut plugins = self.plugins.lock().expect("plugins lock");
        let view = plugins
            .install(&canonical, &self.plugins_root, PluginSource::Local, policy)
            .map_err(|error| error.to_string())?;
        drop(plugins);
        self.reload_skills();
        Ok(view)
    }

    pub fn uninstall_plugin(&self, name: &str) -> Result<(), String> {
        {
            let mut plugins = self.plugins.lock().expect("plugins lock");
            plugins.uninstall(name).map_err(|error| error.to_string())?;
        }
        {
            let mut settings = self.plugin_settings.lock().expect("plugin settings lock");
            settings.disabled.remove(name);
            write_json(&self.plugins_file, &*settings)?;
        }
        self.reload_skills();
        Ok(())
    }

    pub fn set_plugin_enabled(&self, name: &str, enabled: bool) -> Result<(), String> {
        {
            let plugins = self.plugins.lock().expect("plugins lock");
            if plugins.get(name).is_none() {
                return Err(format!("the plugin {name} is not installed"));
            }
        }
        {
            let mut settings = self.plugin_settings.lock().expect("plugin settings lock");
            if enabled {
                settings.disabled.remove(name);
            } else {
                settings.disabled.insert(name.to_string());
            }
            write_json(&self.plugins_file, &*settings)?;
        }
        self.reload_skills();
        Ok(())
    }

    pub fn set_signature_policy(&self, policy: SignaturePolicy) -> Result<(), String> {
        let mut settings = self.plugin_settings.lock().expect("plugin settings lock");
        settings.signature_policy = policy;
        write_json(&self.plugins_file, &*settings)
    }

    /// MCP servers contributed by enabled plugins, for the app-boot manager.
    pub fn mcp_servers(&self) -> Vec<PluginMcpServer> {
        let plugins = self.plugins.lock().expect("plugins lock");
        let settings = self.plugin_settings.lock().expect("plugin settings lock");
        plugins.mcp_servers(&settings)
    }

    /// Add enabled plugin servers to an MCP config before it is built.
    ///
    /// A plugin server whose name collides with a configured server is
    /// skipped with a warning. Letting the collision through would make the
    /// whole MCP config invalid and disable every server, which is a worse
    /// outcome than one plugin contribution being absent.
    pub fn apply_plugin_servers(&self, config: &mut McpConfig) {
        let mut names: BTreeSet<String> = config
            .servers
            .iter()
            .map(|entry| entry.name.clone())
            .collect();
        for server in self.mcp_servers() {
            if !names.insert(server.name.clone()) {
                if let Ok(mut warnings) = self.warnings.lock() {
                    warnings.push(format!(
                        "the plugin {} contributes an MCP server named {:?}, which is already configured; the plugin server was skipped",
                        server.plugin, server.name
                    ));
                }
                continue;
            }
            config.servers.push(crate::mcp::McpServerEntry {
                name: server.name,
                config: server.config,
                oauth: None,
                source: format!("plugin:{}", server.plugin),
                enabled: true,
            });
        }
    }

    fn reload_skills(&self) {
        let plugins = self.plugins.lock().expect("plugins lock");
        let settings = self.plugin_settings.lock().expect("plugin settings lock");
        let mut skills = SkillRegistry::default().with_bundled_placeholders();
        for plugin_root in plugins.skill_roots(&settings) {
            skills.merge_skill_or_root(&plugin_root.root, SkillSource::Plugin);
        }
        skills.merge_root(&self.user_skills_root, SkillSource::User);
        skills.merge_root(&self.org_skills_root, SkillSource::Org);
        drop(settings);
        drop(plugins);
        *self.skills.lock().expect("skills lock") = skills;
    }

    /// Paths used by tests/diagnostics.
    pub fn paths(&self) -> (&Path, &Path, &Path, &Path) {
        (
            &self.user_skills_root,
            &self.org_skills_root,
            &self.plugins_root,
            &self.org_plugins_root,
        )
    }
}

fn load_json<T: for<'de> Deserialize<'de> + Default>(
    path: &Path,
    warnings: &mut Vec<String>,
) -> Option<T> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
        Err(error) => {
            warnings.push(format!("{} could not be read: {error}", path.display()));
            return None;
        }
    };
    match serde_json::from_slice(&bytes) {
        Ok(value) => Some(value),
        Err(error) => {
            warnings.push(format!("{} is not valid JSON: {error}", path.display()));
            None
        }
    }
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, &bytes).map_err(|error| error.to_string())?;
    fs::rename(&temporary, path).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    fn write_skill(root: &Path, name: &str, instructions: &str) {
        let directory = root.join(name);
        fs::create_dir_all(&directory).unwrap();
        fs::write(
            directory.join("manifest.json"),
            serde_json::to_vec(&json!({"name": name, "instructions": instructions})).unwrap(),
        )
        .unwrap();
    }

    fn write_plugin(root: &Path, name: &str) {
        let directory = root.join(name);
        fs::create_dir_all(directory.join("skills/one")).unwrap();
        fs::write(
            directory.join("plugin.json"),
            serde_json::to_vec(&json!({
                "name": name,
                "skills": ["skills/one"],
                "mcpServers": {"echo": {"transport": "stdio", "command": "echo", "args": [], "env": {}}}
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(
            directory.join("skills/one/manifest.json"),
            serde_json::to_vec(&json!({"name": "plugin-skill", "instructions": "from plugin"}))
                .unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn settings_and_user_skills_round_trip() {
        let dir = TempDir::new().unwrap();
        write_skill(&dir.path().join("skills"), "mine", "do the thing");
        let catalog = CatalogRuntime::load(dir.path().to_path_buf());
        let view = catalog.skills();
        assert!(view.skills.iter().any(|skill| skill.name == "mine"));
        catalog.set_skill_enabled("mine", false).unwrap();
        assert!(catalog.skills().disabled.contains("mine"));
        assert!(catalog
            .select_skill("mine")
            .unwrap_err()
            .contains("disabled"));
        let reloaded = CatalogRuntime::load(dir.path().to_path_buf());
        assert!(reloaded.skills().disabled.contains("mine"));
    }

    #[test]
    fn a_disabled_plugin_removes_its_skill_and_server() {
        let dir = TempDir::new().unwrap();
        write_plugin(&dir.path().join("plugins"), "toolbox");
        let catalog = CatalogRuntime::load(dir.path().to_path_buf());
        assert!(catalog
            .skills()
            .skills
            .iter()
            .any(|skill| skill.name == "plugin-skill"));
        assert_eq!(catalog.mcp_servers().len(), 1);
        catalog.set_plugin_enabled("toolbox", false).unwrap();
        assert!(!catalog
            .skills()
            .skills
            .iter()
            .any(|skill| skill.name == "plugin-skill"));
        assert!(catalog.mcp_servers().is_empty());
    }

    #[test]
    fn org_plugins_are_read_only() {
        let dir = TempDir::new().unwrap();
        write_plugin(&dir.path().join("org/plugins"), "orgtool");
        let catalog = CatalogRuntime::load(dir.path().to_path_buf());
        assert!(catalog
            .uninstall_plugin("orgtool")
            .unwrap_err()
            .contains("read-only"));
    }

    #[test]
    fn plugin_servers_are_added_to_the_mcp_config_with_provenance() {
        let dir = TempDir::new().unwrap();
        write_plugin(&dir.path().join("plugins"), "toolbox");
        let catalog = CatalogRuntime::load(dir.path().to_path_buf());
        let mut config = McpConfig::default();
        catalog.apply_plugin_servers(&mut config);
        assert_eq!(config.servers.len(), 1);
        assert_eq!(config.servers[0].name, "toolbox.echo");
        assert_eq!(config.servers[0].source, "plugin:toolbox");
    }

    #[test]
    fn a_plugin_server_never_shadows_a_configured_server() {
        let dir = TempDir::new().unwrap();
        write_plugin(&dir.path().join("plugins"), "toolbox");
        let catalog = CatalogRuntime::load(dir.path().to_path_buf());
        let mut config = McpConfig {
            servers: vec![crate::mcp::McpServerEntry {
                name: "toolbox.echo".into(),
                config: myharness_mcp::ServerConfig::Stdio {
                    command: "configured".into(),
                    args: Vec::new(),
                    env: Default::default(),
                },
                oauth: None,
                source: "user".into(),
                enabled: true,
            }],
            ..McpConfig::default()
        };
        catalog.apply_plugin_servers(&mut config);
        assert_eq!(config.servers.len(), 1);
        assert_eq!(config.servers[0].source, "user");
        assert!(catalog
            .skills()
            .warnings
            .iter()
            .any(|warning| warning.contains("already configured")));
    }
}
