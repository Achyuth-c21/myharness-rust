//! On-disk MCP tool/schema cache — roadmap Phase 5 (`docs/12` §5.1;
//! `docs/13` Phase 5 "tool discovery, schema cache").
//!
//! The shipped app keeps a schema cache so a server's tools can be shown and
//! searched before a fresh connection completes (`custom3p-mcp-schemas.json`
//! is the observed file name; its exact schema was not captured). This module
//! implements the rebuild's cache with explicit, bounded behavior:
//!
//! - the cache is **untrusted input**: every entry is re-validated on load,
//!   schema sizes are capped, and a bad entry is dropped with a warning;
//! - a missing cache is empty, not an error; a corrupt cache is also empty and
//!   leaves the old file untouched until the next successful save;
//! - writes are atomic (temp file + rename + fsync) and `0600` on POSIX
//!   because descriptions/schemas can contain sensitive text (`docs/11` §3);
//! - the cache is **display/search data only**. Execution still requires a
//!   live session and a frozen [`ToolSnapshot`](crate::ToolSnapshot).
//!
//! The exact shipped cache format and eviction policy were not observed
//! (`docs/14` §9); this is a rebuild-defined format, stated as such.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::{
    validate_server_name, validate_tool_name, MAX_TOOLS_PER_SERVER, MAX_TOOL_SCHEMA_BYTES,
};
use crate::tool::ToolDescriptor;

/// The cache format version written by this rebuild.
pub const CACHE_VERSION: u32 = 1;

/// A loaded schema cache and any entries the loader refused.
#[derive(Debug)]
pub struct SchemaCache {
    path: PathBuf,
    servers: BTreeMap<String, Vec<ToolDescriptor>>,
    warnings: Vec<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct CacheDocument {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    servers: BTreeMap<String, CachedServer>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct CachedServer {
    #[serde(default)]
    tools: Vec<ToolDescriptor>,
}

impl SchemaCache {
    /// Load a cache from `path`. Missing or unreadable/corrupt files produce
    /// an empty cache plus a warning; they never abort MCP startup.
    pub fn load(path: impl Into<PathBuf>) -> SchemaCache {
        let path = path.into();
        let mut cache = SchemaCache {
            path,
            servers: BTreeMap::new(),
            warnings: Vec::new(),
        };
        let bytes = match fs::read(&cache.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return cache,
            Err(error) => {
                cache
                    .warnings
                    .push(format!("the MCP schema cache could not be read: {error}"));
                return cache;
            }
        };
        let document: CacheDocument = match serde_json::from_slice(&bytes) {
            Ok(document) => document,
            Err(error) => {
                cache
                    .warnings
                    .push(format!("the MCP schema cache is not valid JSON: {error}"));
                return cache;
            }
        };
        if document.version != CACHE_VERSION {
            cache.warnings.push(format!(
                "the MCP schema cache has version {}, expected {}; it was ignored",
                document.version, CACHE_VERSION
            ));
            return cache;
        }
        for (server, entry) in document.servers {
            if validate_server_name(&server).is_err() {
                cache
                    .warnings
                    .push("the MCP schema cache contains an invalid server name".to_string());
                continue;
            }
            if entry.tools.len() > MAX_TOOLS_PER_SERVER {
                cache.warnings.push(format!(
                    "the MCP schema cache for {server:?} has too many tools; the server was ignored"
                ));
                continue;
            }
            let mut tools = Vec::with_capacity(entry.tools.len());
            let mut seen = BTreeSet::new();
            for tool in entry.tools {
                match validate_cached_tool(&server, tool) {
                    Ok(tool) if seen.insert(tool.name.clone()) => tools.push(tool),
                    Ok(tool) => cache.warnings.push(format!(
                        "the MCP schema cache for {server:?} repeats the tool {:?}; the duplicate was ignored",
                        tool.name
                    )),
                    Err(warning) => cache.warnings.push(warning),
                }
            }
            tools.sort_by(|left, right| left.name.cmp(&right.name));
            cache.servers.insert(server, tools);
        }
        cache
    }

    /// The cache file path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Entries the loader refused. They are reported, never silently hidden.
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// Cached tools for one server, sorted by namespaced name.
    pub fn server_tools(&self, server: &str) -> &[ToolDescriptor] {
        self.servers
            .get(server)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    /// Every cached tool, sorted by namespaced name.
    pub fn all_tools(&self) -> Vec<ToolDescriptor> {
        let mut tools: Vec<ToolDescriptor> = self
            .servers
            .values()
            .flat_map(|tools| tools.iter().cloned())
            .collect();
        tools.sort_by(|left, right| left.name.cmp(&right.name));
        tools
    }

    /// Replace one server's cached tools and persist the cache.
    ///
    /// The write is atomic; a failed write leaves the previous file intact and
    /// returns the I/O error to the caller (MCP startup continues — the cache
    /// is an optimization, not a prerequisite).
    pub fn replace_server(
        &mut self,
        server: &str,
        tools: &[ToolDescriptor],
    ) -> Result<(), std::io::Error> {
        validate_server_name(server).map_err(|error| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, error.to_string())
        })?;
        let mut accepted = Vec::with_capacity(tools.len().min(MAX_TOOLS_PER_SERVER));
        for tool in tools.iter().take(MAX_TOOLS_PER_SERVER) {
            match validate_cached_tool(server, tool.clone()) {
                Ok(tool) => accepted.push(tool),
                Err(warning) => self.warnings.push(warning),
            }
        }
        accepted.sort_by(|left, right| left.name.cmp(&right.name));
        self.servers.insert(server.to_string(), accepted);
        self.save()
    }

    /// Remove one server from the cache and persist.
    pub fn remove_server(&mut self, server: &str) -> Result<(), std::io::Error> {
        self.servers.remove(server);
        self.save()
    }

    /// Persist the cache atomically.
    pub fn save(&self) -> Result<(), std::io::Error> {
        let document = CacheDocument {
            version: CACHE_VERSION,
            servers: self
                .servers
                .iter()
                .map(|(server, tools)| {
                    (
                        server.clone(),
                        CachedServer {
                            tools: tools.clone(),
                        },
                    )
                })
                .collect(),
        };
        let bytes = serde_json::to_vec_pretty(&document).map_err(std::io::Error::other)?;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let temporary = self.path.with_extension("json.tmp");
        {
            let mut file = fs::File::create(&temporary)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                file.set_permissions(fs::Permissions::from_mode(0o600))?;
            }
            file.write_all(&bytes)?;
            file.sync_all()?;
        }
        fs::rename(&temporary, &self.path)?;
        if let Some(parent) = self.path.parent() {
            if let Ok(directory) = fs::File::open(parent) {
                let _ = directory.sync_all();
            }
        }
        Ok(())
    }
}

fn validate_cached_tool(server: &str, mut tool: ToolDescriptor) -> Result<ToolDescriptor, String> {
    if tool.server != server {
        return Err(format!(
            "the MCP schema cache entry {:?} names the wrong server",
            tool.name
        ));
    }
    validate_tool_name(&tool.tool).map_err(|error| error.to_string())?;
    let expected = format!("{server}:{}", tool.tool);
    if tool.name != expected {
        return Err(format!(
            "the MCP schema cache entry {:?} is not namespaced as {expected:?}",
            tool.name
        ));
    }
    let input_size = serde_json::to_vec(&tool.input_schema)
        .map_err(|error| error.to_string())?
        .len();
    if input_size > MAX_TOOL_SCHEMA_BYTES {
        return Err(format!(
            "the MCP schema cache entry {:?} has an oversized input schema",
            tool.name
        ));
    }
    if let Some(schema) = &tool.output_schema {
        let output_size = serde_json::to_vec(schema)
            .map_err(|error| error.to_string())?
            .len();
        if output_size > MAX_TOOL_SCHEMA_BYTES {
            return Err(format!(
                "the MCP schema cache entry {:?} has an oversized output schema",
                tool.name
            ));
        }
    }
    // Normalize the cached name from its validated components so a stale
    // duplicate spelling cannot create a second identity.
    tool.name = expected;
    Ok(tool)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    fn tool(server: &str, name: &str) -> ToolDescriptor {
        ToolDescriptor {
            name: format!("{server}:{name}"),
            server: server.into(),
            tool: name.into(),
            title: None,
            description: Some("fixture".into()),
            input_schema: json!({"type": "object"}),
            output_schema: None,
        }
    }

    #[test]
    fn a_cache_round_trips_and_is_private() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("schemas.json");
        let mut cache = SchemaCache::load(&path);
        cache
            .replace_server(
                "github",
                &[tool("github", "issues"), tool("github", "repos")],
            )
            .unwrap();
        let loaded = SchemaCache::load(&path);
        assert!(loaded.warnings().is_empty());
        assert_eq!(loaded.all_tools().len(), 2);
        assert_eq!(loaded.all_tools()[0].name, "github:issues");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
    }

    #[test]
    fn a_corrupt_cache_is_empty_and_reported() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("schemas.json");
        fs::write(&path, b"{not json").unwrap();
        let cache = SchemaCache::load(&path);
        assert!(cache.all_tools().is_empty());
        assert_eq!(cache.warnings().len(), 1);
        assert!(cache.warnings()[0].contains("not valid JSON"));
    }

    #[test]
    fn a_wrong_server_entry_is_refused_without_dropping_the_rest() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("schemas.json");
        let mut cache = SchemaCache::load(&path);
        cache
            .replace_server("github", &[tool("github", "issues")])
            .unwrap();
        let mut document: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        document["servers"]["github"]["tools"][0]["server"] = json!("other");
        fs::write(&path, serde_json::to_vec(&document).unwrap()).unwrap();
        let loaded = SchemaCache::load(&path);
        assert!(loaded.all_tools().is_empty());
        assert!(!loaded.warnings().is_empty());
    }

    #[test]
    fn a_duplicate_cached_tool_is_ignored_and_reported() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("schemas.json");
        let mut cache = SchemaCache::load(&path);
        cache
            .replace_server("github", &[tool("github", "issues")])
            .unwrap();
        let mut document: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let duplicate = document["servers"]["github"]["tools"][0].clone();
        document["servers"]["github"]["tools"]
            .as_array_mut()
            .unwrap()
            .push(duplicate);
        fs::write(&path, serde_json::to_vec(&document).unwrap()).unwrap();
        let loaded = SchemaCache::load(&path);
        assert_eq!(loaded.all_tools().len(), 1);
        assert!(!loaded.warnings().is_empty());
    }

    #[test]
    fn a_missing_cache_is_not_an_error() {
        let dir = TempDir::new().unwrap();
        let cache = SchemaCache::load(dir.path().join("absent.json"));
        assert!(cache.warnings().is_empty());
        assert!(cache.all_tools().is_empty());
    }
}
