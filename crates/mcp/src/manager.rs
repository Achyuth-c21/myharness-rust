//! MCP server lifecycle, tool cache, and per-session snapshots.
//!
//! Lifecycle rules come from `docs/12` §5.1 and `docs/13` Phase 5: start
//! failures are non-fatal and persisted as server state, tools are namespaced
//! `<server>:<tool>`, and the enabled set is frozen per session. The manager
//! owns no timer and performs no background reconnect; callers decide when to
//! retry (`docs/14` §9 — retry/backoff behavior was not observed).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::{
    validate_server_name, validate_tool_name, ServerConfig, MAX_TOOLS_PER_SERVER,
    MAX_TOOL_SCHEMA_BYTES,
};
use crate::egress::EgressPolicy;
use crate::error::{McpError, McpResult};
use crate::oauth::{AuthError, McpAuthProvider};
use crate::schema_cache::SchemaCache;
use crate::search::{search_catalogs, ToolSearchHit};
use crate::session::{McpConnector, McpSession, RmcpConnector, ServerInfoSummary};
use crate::tool::{split_namespaced, ToolCallResult, ToolDefinition, ToolDescriptor, ToolSnapshot};

/// Current state of one configured server.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ServerStatus {
    /// Configured but not started, or explicitly shut down.
    Stopped,
    /// A connection attempt is in flight.
    Starting,
    /// Connected; the tool cache is valid for this connection.
    Connected {
        /// Redacted server identity.
        server: ServerInfoSummary,
        /// Number of cached tools.
        tool_count: usize,
    },
    /// The last start attempt failed. The message is redacted and bounded.
    Failed {
        /// Redacted, bounded failure message.
        message: String,
    },
    /// The server needs an OAuth authorization the app does not have.
    /// Retrying without a token cannot succeed; the UI must offer a sign-in
    /// path rather than a generic retry.
    NeedsAuth {
        /// Redacted, bounded message.
        message: String,
    },
}

/// Renderer-safe view of one configured server.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerView {
    /// Configured name.
    pub name: String,
    /// Current state.
    pub status: ServerStatus,
    /// Last failure message, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// One server that failed during [`McpManager::start_all`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartFailure {
    /// Configured name.
    pub server: String,
    /// Redacted, bounded message.
    pub message: String,
}

/// Result of starting every configured server.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartSummary {
    /// Servers that connected and listed their tools.
    pub started: Vec<String>,
    /// Servers that failed. Their state remains [`ServerStatus::Failed`].
    pub failed: Vec<StartFailure>,
}

impl StartSummary {
    /// True when every configured server started.
    pub fn all_started(&self) -> bool {
        self.failed.is_empty()
    }
}

/// One server that did not shut down cleanly.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShutdownFailure {
    /// Configured name.
    pub server: String,
    /// Redacted, bounded message.
    pub message: String,
}

struct ServerEntry {
    config: ServerConfig,
    status: ServerStatus,
    session: Option<Box<dyn McpSession>>,
    tools: Vec<ToolDescriptor>,
    error: Option<String>,
}

/// Owns configured MCP servers and their live sessions.
///
/// The manager is deliberately not a global singleton. The app shell decides
/// when to construct it and whether to run [`start_all`](Self::start_all) off
/// the boot path (`docs/13` Phase 5 exit: an unavailable server cannot block
/// app boot).
pub struct McpManager {
    connector: Arc<dyn McpConnector>,
    servers: BTreeMap<String, ServerEntry>,
    start_timeout: Option<Duration>,
    egress: Option<EgressPolicy>,
    auth: Option<Arc<dyn McpAuthProvider>>,
    cache: Option<SchemaCache>,
    cache_warnings: Vec<String>,
}

impl Default for McpManager {
    fn default() -> Self {
        Self::new()
    }
}

impl McpManager {
    /// A manager using the production `rmcp` connector.
    pub fn new() -> Self {
        Self::with_connector(Arc::new(RmcpConnector))
    }

    /// A manager with an injected connector, used by lifecycle tests.
    pub fn with_connector(connector: Arc<dyn McpConnector>) -> Self {
        Self {
            connector,
            servers: BTreeMap::new(),
            start_timeout: None,
            egress: None,
            auth: None,
            cache: None,
            cache_warnings: Vec::new(),
        }
    }

    /// Supply the egress allowlist checked before an HTTP/SSE connection.
    ///
    /// `None` (the default) means no policy was supplied by the host; a
    /// supplied policy is fail-closed (`TEST-AI-020`). Stdio servers are not
    /// inspected here — see [`crate::egress`].
    pub fn with_egress(mut self, egress: EgressPolicy) -> Self {
        self.egress = Some(egress);
        self
    }

    /// Bound each connect/list-tools phase.
    ///
    /// The shipped startup timeout was not captured (`docs/14` §9), so the
    /// manager defaults to no deadline and the caller chooses one. A timeout
    /// records a normal [`ServerStatus::Failed`] state, which lets
    /// [`start_all`](Self::start_all) continue to the next server.
    pub fn with_start_timeout(mut self, timeout: Option<Duration>) -> Self {
        self.start_timeout = timeout;
        self
    }

    /// Supply the per-server OAuth/authorization provider. A provider error
    /// is recorded per server; it never aborts the rest of startup.
    pub fn with_auth_provider(mut self, auth: Arc<dyn McpAuthProvider>) -> Self {
        self.auth = Some(auth);
        self
    }

    /// Supply a loaded on-disk schema cache. Cache warnings are retained and
    /// exposed through [`cache_warnings`](Self::cache_warnings).
    pub fn with_schema_cache(mut self, cache: SchemaCache) -> Self {
        self.cache_warnings.extend(cache.warnings().iter().cloned());
        self.cache = Some(cache);
        self
    }

    /// Warnings from the cache loader and from failed cache writes. They are
    /// diagnostics, never reasons to block a server.
    pub fn cache_warnings(&self) -> &[String] {
        &self.cache_warnings
    }

    /// Cached tools for display/search, independent of live sessions.
    pub fn cached_tools(&self) -> Vec<ToolDescriptor> {
        self.cache
            .as_ref()
            .map(SchemaCache::all_tools)
            .unwrap_or_default()
    }

    /// Search live tools first and cached-only tools second. A cached hit is
    /// metadata only; it cannot execute until a live session exists.
    pub fn search_tools(
        &self,
        query: &str,
        limit: usize,
        disabled: &BTreeSet<String>,
    ) -> Vec<ToolSearchHit> {
        let live = self.list_tools();
        let live_names: BTreeSet<&str> = live.iter().map(|tool| tool.name.as_str()).collect();
        let cached: Vec<ToolDescriptor> = self
            .cached_tools()
            .into_iter()
            .filter(|tool| !live_names.contains(tool.name.as_str()))
            .collect();
        search_catalogs(&live, &cached, query, limit, disabled)
    }

    /// Configure a server. Nothing is spawned until [`start`](Self::start).
    pub fn add_server(&mut self, name: impl Into<String>, config: ServerConfig) -> McpResult<()> {
        let name = name.into();
        validate_server_name(&name)?;
        config.validate()?;
        if self.servers.contains_key(&name) {
            return Err(McpError::DuplicateServer(name));
        }
        self.servers.insert(
            name,
            ServerEntry {
                config,
                status: ServerStatus::Stopped,
                session: None,
                tools: Vec::new(),
                error: None,
            },
        );
        Ok(())
    }

    /// Configured names, sorted by the manager's map order.
    pub fn server_names(&self) -> impl Iterator<Item = &str> {
        self.servers.keys().map(String::as_str)
    }

    /// Renderer-safe state for every configured server.
    pub fn statuses(&self) -> Vec<ServerView> {
        self.servers
            .iter()
            .map(|(name, entry)| ServerView {
                name: name.clone(),
                status: entry.status.clone(),
                error: entry.error.clone(),
            })
            .collect()
    }

    /// Start one server, list its tools, and cache them.
    ///
    /// A failure is recorded on the server and returned; it never removes the
    /// configuration, so a later retry is possible.
    pub async fn start(&mut self, server: &str) -> McpResult<()> {
        let connector = Arc::clone(&self.connector);
        let start_timeout = self.start_timeout;
        let egress = self.egress.clone();
        let auth = self.auth.clone();
        let entry = self
            .servers
            .get_mut(server)
            .ok_or_else(|| McpError::UnknownServer(server.to_string()))?;
        if let Some(session) = entry.session.as_ref() {
            if !session.is_closed() {
                return Ok(());
            }
            // The transport died. Drop the dead handle and rebuild the cache;
            // the config remains so this is a reconnect, not a removal.
            entry.session = None;
            entry.tools.clear();
            entry.status = ServerStatus::Stopped;
        }

        entry.status = ServerStatus::Starting;
        entry.error = None;
        entry.tools.clear();
        let mut config = entry.config.clone();

        if let Some(auth) = auth {
            if matches!(config, ServerConfig::Http { .. } | ServerConfig::Sse { .. }) {
                let authorization = auth.authorization_header(server);
                let authorization = match start_timeout {
                    Some(limit) => match tokio::time::timeout(limit, authorization).await {
                        Ok(result) => result,
                        Err(_) => {
                            return Err(record_failure(
                                entry,
                                server,
                                &config,
                                &format!(
                                    "OAuth did not resolve within {} ms",
                                    duration_millis(limit)
                                ),
                            ));
                        }
                    },
                    None => authorization.await,
                };
                match authorization {
                    Ok(Some((name, value))) => {
                        if let Err(error) = insert_authorization_header(&mut config, &name, &value)
                        {
                            return Err(record_failure(entry, server, &config, &error));
                        }
                    }
                    Ok(None) => {}
                    Err(AuthError::NeedsAuth { message, .. }) => {
                        return Err(record_needs_auth(entry, server, &config, &message));
                    }
                    Err(AuthError::Failed { message, .. }) => {
                        return Err(record_failure(entry, server, &config, &message));
                    }
                }
            }
        }

        if let Some(egress) = &egress {
            if let Err(error) = egress.check_server(&config) {
                return Err(record_failure(entry, server, &config, &error.to_string()));
            }
        }

        let connect = connector.connect(server, &config);
        let connected = match start_timeout {
            Some(limit) => match tokio::time::timeout(limit, connect).await {
                Ok(result) => result,
                Err(_) => {
                    return Err(record_failure(
                        entry,
                        server,
                        &config,
                        &format!(
                            "the server did not start within {} ms",
                            duration_millis(limit)
                        ),
                    ));
                }
            },
            None => connect.await,
        };
        let mut session = match connected {
            Ok(session) => session,
            Err(error) => {
                return Err(record_failure(entry, server, &config, &error.to_string()));
            }
        };

        let list_tools = session.list_tools();
        let definitions = match start_timeout {
            Some(limit) => match tokio::time::timeout(limit, list_tools).await {
                Ok(result) => result,
                Err(_) => {
                    let _ = shutdown_bounded(server, &mut session).await;
                    return Err(record_failure(
                        entry,
                        server,
                        &config,
                        &format!(
                            "the server did not list tools within {} ms",
                            duration_millis(limit)
                        ),
                    ));
                }
            },
            None => list_tools.await,
        };
        let definitions = match definitions {
            Ok(definitions) => definitions,
            Err(error) => {
                let _ = shutdown_bounded(server, &mut session).await;
                return Err(record_failure(entry, server, &config, &error.to_string()));
            }
        };

        let tools = match validate_catalog(server, definitions) {
            Ok(tools) => tools,
            Err(message) => {
                let _ = shutdown_bounded(server, &mut session).await;
                return Err(record_failure(entry, server, &config, &message));
            }
        };

        let info = session.server_info();
        let tool_count = tools.len();
        let cache_tools = tools.clone();
        entry.tools = tools;
        entry.session = Some(session);
        entry.status = ServerStatus::Connected {
            server: info,
            tool_count,
        };

        if let Some(cache) = self.cache.as_mut() {
            if let Err(error) = cache.replace_server(server, &cache_tools) {
                self.cache_warnings.push(format!(
                    "the MCP schema cache could not be written for {server}: {error}"
                ));
            }
        }
        Ok(())
    }

    /// Start every configured server, collecting failures.
    ///
    /// This is the non-blocking-boot primitive: one unavailable server never
    /// prevents the others from starting, and the caller receives a summary
    /// instead of the first error (`TEST-MCP-009`).
    pub async fn start_all(&mut self) -> StartSummary {
        let names: Vec<String> = self.servers.keys().cloned().collect();
        let mut summary = StartSummary::default();
        for name in names {
            match self.start(&name).await {
                Ok(()) => summary.started.push(name),
                Err(error) => summary.failed.push(StartFailure {
                    server: name,
                    message: error.to_string(),
                }),
            }
        }
        summary
    }

    /// The merged, namespaced tool cache from connected servers.
    ///
    /// Sorted by namespaced name so the provider request and the session
    /// snapshot have a stable order (`TEST-MCP-002`).
    pub fn list_tools(&self) -> Vec<ToolDescriptor> {
        let mut tools: Vec<ToolDescriptor> = self
            .servers
            .values()
            .filter(|entry| matches!(&entry.status, ServerStatus::Connected { .. }))
            .flat_map(|entry| entry.tools.iter().cloned())
            .collect();
        tools.sort_by(|left, right| left.name.cmp(&right.name));
        tools
    }

    /// Freeze the requested subset of the current cache.
    ///
    /// Empty `enabled` means an empty snapshot, not "all tools": the session
    /// enablement map is explicit (`docs/12` §5.1). Names that are not
    /// available are reported in [`ToolSnapshot::missing`] rather than being
    /// dropped.
    pub fn snapshot(&self, enabled: &[String]) -> ToolSnapshot {
        let all = self.list_tools();
        let by_name: BTreeMap<&str, &ToolDescriptor> =
            all.iter().map(|tool| (tool.name.as_str(), tool)).collect();
        let mut seen = BTreeSet::new();
        let mut tools = Vec::new();
        let mut missing = Vec::new();
        for name in enabled {
            if !seen.insert(name.as_str()) {
                continue;
            }
            match by_name.get(name.as_str()) {
                Some(tool) => tools.push((*tool).clone()),
                None => missing.push(name.clone()),
            }
        }
        tools.sort_by(|left, right| left.name.cmp(&right.name));
        missing.sort();
        ToolSnapshot { tools, missing }
    }

    /// Freeze the whole current cache.
    pub fn snapshot_all(&self) -> ToolSnapshot {
        ToolSnapshot {
            tools: self.list_tools(),
            missing: Vec::new(),
        }
    }

    /// Invoke one tool from a frozen snapshot.
    ///
    /// The snapshot gate runs before the session is touched, so a tool that
    /// was not enabled for this session never reaches the server. The caller
    /// supplies the timeout because the shipped default for
    /// `mcpToolTimeoutSec` was not captured (`docs/14` §9); `None` means the
    /// caller chose no deadline.
    pub async fn call_tool(
        &self,
        snapshot: &ToolSnapshot,
        name: &str,
        arguments: Value,
        timeout: Option<Duration>,
    ) -> McpResult<ToolCallResult> {
        let (server, tool) = split_namespaced(name)?;
        if !snapshot.contains(name) {
            return Err(McpError::ToolNotInSnapshot {
                name: name.to_string(),
            });
        }
        let entry = self
            .servers
            .get(server)
            .ok_or_else(|| McpError::UnknownServer(server.to_string()))?;
        if !matches!(&entry.status, ServerStatus::Connected { .. }) {
            return Err(McpError::Server {
                server: server.to_string(),
                message: "the MCP server is not connected".to_string(),
            });
        }
        let session = entry.session.as_ref().ok_or_else(|| McpError::Server {
            server: server.to_string(),
            message: "the MCP server has no live session".to_string(),
        })?;
        let call = session.call_tool(tool, arguments);
        match timeout {
            Some(limit) => match tokio::time::timeout(limit, call).await {
                Ok(result) => result,
                Err(_) => Err(McpError::ToolTimeout {
                    name: name.to_string(),
                    millis: duration_millis(limit),
                }),
            },
            None => call.await,
        }
    }

    /// Shut down every live session. Idempotent.
    pub async fn shutdown(&mut self) -> Vec<ShutdownFailure> {
        let names: Vec<String> = self.servers.keys().cloned().collect();
        let mut failures = Vec::new();
        for name in names {
            let Some(entry) = self.servers.get_mut(&name) else {
                continue;
            };
            if let Some(mut session) = entry.session.take() {
                if let Err(error) = shutdown_bounded(&name, &mut session).await {
                    failures.push(ShutdownFailure {
                        server: name.clone(),
                        message: entry.config.redact(&error.to_string()),
                    });
                }
            }
            entry.tools.clear();
            entry.status = ServerStatus::Stopped;
            entry.error = None;
        }
        failures
    }

    /// Shut down one server and remove its configuration.
    pub async fn remove_server(&mut self, server: &str) -> McpResult<()> {
        let Some(mut entry) = self.servers.remove(server) else {
            return Err(McpError::UnknownServer(server.to_string()));
        };
        if let Some(mut session) = entry.session.take() {
            shutdown_bounded(server, &mut session)
                .await
                .map_err(|error| McpError::Shutdown {
                    server: server.to_string(),
                    message: entry.config.redact(&error.to_string()),
                })?;
        }
        if let Some(cache) = self.cache.as_mut() {
            if let Err(error) = cache.remove_server(server) {
                self.cache_warnings.push(format!(
                    "the MCP schema cache could not remove {server}: {error}"
                ));
            }
        }
        Ok(())
    }
}

/// A shutdown is bounded so one wedged transport cannot stall `start_all` or
/// app exit. The handle is dropped on timeout; it must not keep the manager
/// alive, and the caller records the failure like any other shutdown error.
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

async fn shutdown_bounded(server: &str, session: &mut Box<dyn McpSession>) -> McpResult<()> {
    match tokio::time::timeout(SHUTDOWN_TIMEOUT, session.shutdown()).await {
        Ok(result) => result,
        Err(_) => Err(McpError::Shutdown {
            server: server.to_string(),
            message: format!(
                "the session did not shut down within {} ms",
                duration_millis(SHUTDOWN_TIMEOUT)
            ),
        }),
    }
}

fn duration_millis(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

fn record_needs_auth(
    entry: &mut ServerEntry,
    server: &str,
    config: &ServerConfig,
    message: &str,
) -> McpError {
    let message = config.redact(message);
    entry.status = ServerStatus::NeedsAuth {
        message: message.clone(),
    };
    entry.error = Some(message.clone());
    McpError::NeedsAuth {
        server: server.to_string(),
        message,
    }
}

fn insert_authorization_header(
    config: &mut ServerConfig,
    name: &str,
    value: &str,
) -> Result<(), String> {
    let headers = match config {
        ServerConfig::Http { headers, .. } | ServerConfig::Sse { headers, .. } => headers,
        ServerConfig::Stdio { .. } => return Ok(()),
    };
    if headers
        .keys()
        .any(|existing| existing.eq_ignore_ascii_case(name))
    {
        // An explicit configured header wins; OAuth never overwrites a
        // credential the operator supplied.
        return Ok(());
    }
    headers.insert(name.to_string(), value.to_string());
    config.validate().map_err(|error| error.to_string())
}

fn record_failure(
    entry: &mut ServerEntry,
    server: &str,
    config: &ServerConfig,
    message: &str,
) -> McpError {
    let message = config.redact(message);
    entry.status = ServerStatus::Failed {
        message: message.clone(),
    };
    entry.error = Some(message.clone());
    McpError::Server {
        server: server.to_string(),
        message,
    }
}

fn validate_catalog(
    server: &str,
    definitions: Vec<ToolDefinition>,
) -> Result<Vec<ToolDescriptor>, String> {
    if definitions.len() > MAX_TOOLS_PER_SERVER {
        return Err(format!(
            "the server returned {} tools, above the {} tool limit",
            definitions.len(),
            MAX_TOOLS_PER_SERVER
        ));
    }
    let mut tools = Vec::with_capacity(definitions.len());
    let mut seen = std::collections::BTreeSet::new();
    for definition in definitions {
        validate_tool_name(&definition.name).map_err(|error| error.to_string())?;
        if !seen.insert(definition.name.clone()) {
            return Err(format!(
                "the server returned the tool name {:?} more than once",
                definition.name
            ));
        }
        let input_size = serde_json::to_vec(&definition.input_schema)
            .map_err(|error| error.to_string())?
            .len();
        if input_size > MAX_TOOL_SCHEMA_BYTES {
            return Err(format!(
                "tool {:?} has a {} byte input schema, above the {} byte limit",
                definition.name, input_size, MAX_TOOL_SCHEMA_BYTES
            ));
        }
        if let Some(output_schema) = &definition.output_schema {
            let output_size = serde_json::to_vec(output_schema)
                .map_err(|error| error.to_string())?
                .len();
            if output_size > MAX_TOOL_SCHEMA_BYTES {
                return Err(format!(
                    "tool {:?} has a {} byte output schema, above the {} byte limit",
                    definition.name, output_size, MAX_TOOL_SCHEMA_BYTES
                ));
            }
        }
        tools.push(
            ToolDescriptor::from_definition(server, definition)
                .map_err(|error| error.to_string())?,
        );
    }
    tools.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(tools)
}
