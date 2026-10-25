//! MCP, tools, and approval composition for the chat bridge — roadmap Phase 5
//! (`docs/13`; `docs/12` §5.1; `docs/features/mcp-skills-tools.md`).
//!
//! This module is the seam between the transport-independent [`McpManager`]
//! and the agent's tool loop. It owns:
//!
//! - the rebuild's on-disk MCP configuration (`mcp/servers.json`, a
//!   rebuild-defined format because the shipped file shape was not captured);
//! - the per-session frozen tool snapshot and its enabled set;
//! - the agent [`ToolExecutor`] adapter that preserves text/images/documents
//!   and refuses audio explicitly (the provider SDK has no audio part yet);
//! - the renderer approval prompt (a Tauri event plus a oneshot response);
//! - per-server OAuth storage in the OS keychain and a refresh HTTP client;
//! - the schema cache and tool search.
//!
//! Rules that are deliberately visible here:
//!
//! - **Secrets never cross to the renderer.** Server headers/env, OAuth tokens,
//!   and raw tool arguments are not in status/search payloads; the approval
//!   preview is redacted and bounded.
//! - **A cached tool is metadata only.** Execution requires a live session and
//!   a frozen snapshot (`McpManager::call_tool`).
//! - **MCP boot failure is non-fatal.** The Tauri shell starts the manager off
//!   the boot path and emits a toast for each failure; this module only
//!   supplies the status/summary.
//! - **No silent model substitution.** Tool availability is per session; a
//!   changed server catalog cannot mutate a snapshot already in use.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use myharness_agent::tool_exec::{ApprovalPrompt, ApprovalRequest, ToolExecutor, ToolOutput};
use myharness_agent::tool_policy::{
    ApprovalContext, ApprovalRecord, ApprovalScope, ApprovalStore, ToolPolicy,
};
use myharness_agent::ToolTurnOutcome;
use myharness_mcp::{
    EgressPolicy, McpManager, OAuthConfig, OAuthError, OAuthHttp, OAuthManager, OAuthStore,
    OAuthToken, SchemaCache, ServerConfig, ServerView, ToolCallResult, ToolContent, ToolSearchHit,
    ToolSnapshot,
};
use myharness_provider_sdk::{ContentPart, ToolSpec, TurnEvent};
use myharness_secrets::SecretStore;
use secrecy::ExposeSecret as _;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

/// Bound on an approval argument preview.
pub const MAX_APPROVAL_PREVIEW_BYTES: usize = 4 * 1024;

/// Bound on `mcp/servers.json`. The shipped configuration file size was not
/// captured; this keeps a corrupt or hostile local file from exhausting memory
/// during app boot.
pub const MAX_MCP_CONFIG_BYTES: u64 = 2 * 1024 * 1024;

/// Bound on `mcp/approvals.json`, for the same reason.
pub const MAX_APPROVAL_FILE_BYTES: u64 = 2 * 1024 * 1024;

fn read_bounded(path: &Path, limit: u64, label: &str) -> std::io::Result<Vec<u8>> {
    let file = std::fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("{label} exceeds the {limit}-byte limit"),
        ));
    }
    Ok(bytes)
}

// ---------------------------------------------------------------------------
// On-disk configuration
// ---------------------------------------------------------------------------

/// One configured MCP server in the rebuild file.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerEntry {
    /// Namespaced server name.
    pub name: String,
    /// Transport definition. Values in env/headers are secrets.
    pub config: ServerConfig,
    /// Optional OAuth client configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oauth: Option<OAuthConfig>,
    /// Managed/developer/plugin provenance, for display only.
    #[serde(default = "default_source")]
    pub source: String,
    /// Disabled entries stay configured but are not started.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_source() -> String {
    "developer".to_string()
}

fn default_true() -> bool {
    true
}

/// The rebuild's MCP settings file.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpConfig {
    /// Servers, in file order.
    #[serde(default)]
    pub servers: Vec<McpServerEntry>,
    /// `mcpPersistentAlwaysAllowEnabled`.
    #[serde(default)]
    pub persistent_always_allow: bool,
    /// `mcpScheduledTaskApprovalLifetimeDays`; 0 disables lasting scopes for
    /// scheduled tasks.
    #[serde(default)]
    pub scheduled_approval_lifetime_days: u32,
    /// `toolSearchEnabled`.
    #[serde(default)]
    pub tool_search_enabled: bool,
    /// `disabledBuiltinTools` and any explicitly disabled MCP names.
    #[serde(default)]
    pub disabled_builtin_tools: BTreeSet<String>,
    /// Local policy rules. Defaults to fail-closed `Ask` when absent.
    #[serde(default)]
    pub policy: ToolPolicy,
    /// Managed/developer egress allowlist. Absent means no policy supplied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub egress: Option<EgressPolicy>,
    /// Per-tool call timeout in seconds; absent means no deadline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_timeout_seconds: Option<u64>,
    /// Whether a startup failure should be surfaced as a toast. The shipped
    /// behavior is a persistent actionable toast (`docs/features/
    /// mcp-skills-tools.md` §2); the renderer owns the presentation.
    #[serde(default = "default_true")]
    pub startup_failure_toast: bool,
}

/// A loaded config plus non-fatal warnings.
#[derive(Clone, Debug, Default)]
pub struct LoadedMcpConfig {
    pub config: McpConfig,
    pub warnings: Vec<String>,
}

impl McpConfig {
    /// Load `mcp/servers.json` from `root`. A missing file is an empty config;
    /// a malformed file is an empty config plus a warning so MCP cannot block
    /// app boot.
    pub fn load(root: &Path) -> LoadedMcpConfig {
        let path = root.join("servers.json");
        let bytes = match read_bounded(&path, MAX_MCP_CONFIG_BYTES, "the MCP configuration") {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return LoadedMcpConfig::default();
            }
            Err(error) => {
                return LoadedMcpConfig {
                    config: McpConfig::default(),
                    warnings: vec![format!("the MCP configuration could not be read: {error}")],
                };
            }
        };
        match serde_json::from_slice::<McpConfig>(&bytes) {
            Ok(config) => LoadedMcpConfig {
                config,
                warnings: Vec::new(),
            },
            Err(error) => LoadedMcpConfig {
                config: McpConfig::default(),
                warnings: vec![format!("the MCP configuration is not valid JSON: {error}")],
            },
        }
    }

    /// Validate names and duplicate entries before building a manager.
    pub fn validate(&self) -> Result<(), String> {
        let mut seen = BTreeSet::new();
        for entry in &self.servers {
            if !seen.insert(entry.name.clone()) {
                return Err(format!("duplicate MCP server name {:?}", entry.name));
            }
            myharness_mcp::validate_server_name(&entry.name).map_err(|error| error.to_string())?;
            entry.config.validate().map_err(|error| error.to_string())?;
            if let Some(oauth) = &entry.oauth {
                oauth.validate().map_err(|error| error.to_string())?;
            }
        }
        Ok(())
    }

    /// Build the manager, auth provider, schema cache, and policy.
    ///
    /// The caller supplies the keychain store and HTTP client so this function
    /// stays testable and the production adapters remain at the Tauri edge.
    pub fn build(
        &self,
        cache_path: &Path,
        oauth_store: Arc<dyn OAuthStore>,
        oauth_http: Arc<dyn OAuthHttp>,
    ) -> Result<BuiltMcp, String> {
        self.validate()?;
        let mut oauth = OAuthManager::new(oauth_store, oauth_http);
        for entry in &self.servers {
            if let Some(config) = &entry.oauth {
                oauth = oauth
                    .with_config(entry.name.clone(), config.clone())
                    .map_err(|error| error.to_string())?;
            }
        }
        let mut manager = McpManager::new()
            .with_auth_provider(Arc::new(oauth))
            .with_schema_cache(SchemaCache::load(cache_path));
        if let Some(egress) = &self.egress {
            manager = manager.with_egress(egress.clone());
        }
        let mut warnings = Vec::new();
        for entry in &self.servers {
            if !entry.enabled {
                continue;
            }
            if let Err(error) = manager.add_server(entry.name.clone(), entry.config.clone()) {
                warnings.push(format!(
                    "MCP server {:?} was not configured: {error}",
                    entry.name
                ));
            }
        }
        Ok(BuiltMcp {
            manager: Arc::new(tokio::sync::Mutex::new(manager)),
            config: self.clone(),
            warnings,
        })
    }
}

/// The manager plus the settings needed by the tool runtime.
pub struct BuiltMcp {
    pub manager: Arc<tokio::sync::Mutex<McpManager>>,
    pub config: McpConfig,
    pub warnings: Vec<String>,
}

// ---------------------------------------------------------------------------
// Keychain-backed OAuth store
// ---------------------------------------------------------------------------

/// OAuth tokens stored as one JSON value per server in the OS keychain.
pub struct KeyringOAuthStore {
    store: Arc<dyn SecretStore>,
}

impl KeyringOAuthStore {
    pub fn new(store: Arc<dyn SecretStore>) -> KeyringOAuthStore {
        KeyringOAuthStore { store }
    }

    fn account(server: &str) -> String {
        format!("mcp-oauth/{server}")
    }
}

impl fmt::Debug for KeyringOAuthStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KeyringOAuthStore")
            .finish_non_exhaustive()
    }
}

impl OAuthStore for KeyringOAuthStore {
    fn get(&self, server: &str) -> Result<Option<OAuthToken>, OAuthError> {
        let value = self
            .store
            .load(&Self::account(server))
            .map_err(|error| OAuthError::Store(error.to_string()))?;
        let Some(value) = value else {
            return Ok(None);
        };
        serde_json::from_str(value.expose_secret())
            .map(Some)
            .map_err(|_| {
                OAuthError::Store("the stored OAuth token is not in the expected shape".into())
            })
    }

    fn set(&self, server: &str, token: &OAuthToken) -> Result<(), OAuthError> {
        let serialized = serde_json::to_string(token)
            .map_err(|_| OAuthError::Store("the OAuth token could not be serialized".into()))?;
        self.store
            .store(&Self::account(server), &SecretString::from(serialized))
            .map_err(|error| OAuthError::Store(error.to_string()))
    }

    fn forget(&self, server: &str) -> Result<(), OAuthError> {
        self.store
            .delete(&Self::account(server))
            .map(|_| ())
            .map_err(|error| OAuthError::Store(error.to_string()))
    }
}

// ---------------------------------------------------------------------------
// OAuth HTTP refresh
// ---------------------------------------------------------------------------

/// Production OAuth refresh over `reqwest`.
///
/// The request shape is a rebuild-defined OAuth 2.0 refresh grant. It is not a
/// claim about the shipped client, whose authorization-code flow was not
/// captured. Errors never include the response body (it can echo a token).
pub struct ReqwestOAuthHttp {
    client: reqwest::Client,
    egress: Option<EgressPolicy>,
}

impl ReqwestOAuthHttp {
    pub fn new() -> Result<ReqwestOAuthHttp, OAuthError> {
        ReqwestOAuthHttp::with_egress(None)
    }

    /// Build the refresh client with the same host allowlist as the MCP
    /// transport. Redirects are disabled: a refresh token must never be
    /// replayed to a redirect target that the policy did not check.
    pub fn with_egress(egress: Option<EgressPolicy>) -> Result<ReqwestOAuthHttp, OAuthError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|error| OAuthError::Refresh {
                server: "unknown".into(),
                message: format!("the OAuth HTTP client could not be built: {error}"),
            })?;
        Ok(ReqwestOAuthHttp { client, egress })
    }
}

#[async_trait]
impl OAuthHttp for ReqwestOAuthHttp {
    async fn refresh(
        &self,
        config: &OAuthConfig,
        refresh_token: &str,
    ) -> Result<OAuthToken, OAuthError> {
        config.validate()?;
        if let Some(egress) = &self.egress {
            egress
                .check_url(&config.token_url)
                .map_err(|error| OAuthError::Refresh {
                    server: "unknown".into(),
                    message: format!("the OAuth token endpoint is not allowed: {error}"),
                })?;
        }
        let mut form = vec![
            ("grant_type", "refresh_token".to_string()),
            ("refresh_token", refresh_token.to_string()),
            ("client_id", config.client_id.clone()),
        ];
        if !config.scopes.is_empty() {
            form.push(("scope", config.scopes.join(" ")));
        }
        let response = self
            .client
            .post(&config.token_url)
            .form(&form)
            .send()
            .await
            .map_err(|error| OAuthError::Refresh {
                server: "unknown".into(),
                message: format!("the token endpoint could not be reached: {error}"),
            })?;
        let status = response.status();
        if !status.is_success() {
            return Err(OAuthError::Refresh {
                server: "unknown".into(),
                message: format!("the token endpoint returned HTTP {}", status.as_u16()),
            });
        }
        let body: Value = response.json().await.map_err(|_| OAuthError::Refresh {
            server: "unknown".into(),
            message: "the token endpoint did not return a JSON object".into(),
        })?;
        let access_token = body
            .get("access_token")
            .and_then(Value::as_str)
            .filter(|token| !token.is_empty())
            .ok_or_else(|| OAuthError::Refresh {
                server: "unknown".into(),
                message: "the token endpoint returned no access token".into(),
            })?;
        let refresh_token = body
            .get("refresh_token")
            .and_then(Value::as_str)
            .map(str::to_string);
        let token_type = body
            .get("token_type")
            .and_then(Value::as_str)
            .map(str::to_string);
        let expires_at = body
            .get("expires_in")
            .and_then(Value::as_u64)
            .map(|seconds| unix_now().saturating_add(seconds));
        Ok(OAuthToken {
            access_token: access_token.to_string(),
            refresh_token,
            token_type,
            expires_at,
        })
    }
}

// ---------------------------------------------------------------------------
// Approval prompt
// ---------------------------------------------------------------------------

/// The renderer-facing approval request. Argument values are a redacted,
/// bounded preview; the real arguments stay in the main process.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolApprovalEvent {
    pub id: String,
    pub session_id: String,
    pub call_id: String,
    pub tool_name: String,
    pub display_name: String,
    pub arguments_preview: Value,
    pub allowed_scopes: Vec<ApprovalScope>,
}

/// The renderer's answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ApprovalDecision {
    Allow,
    Deny,
}

struct PendingApproval {
    allowed_scopes: Vec<ApprovalScope>,
    sender: oneshot::Sender<Option<ApprovalScope>>,
}

/// Emits a `chat://tool-approval` event and waits for `respond`.
pub struct BridgeApprovalPrompt {
    emit: Arc<dyn Fn(ToolApprovalEvent) + Send + Sync>,
    pending: Mutex<BTreeMap<String, PendingApproval>>,
}

impl BridgeApprovalPrompt {
    pub fn new(emit: Arc<dyn Fn(ToolApprovalEvent) + Send + Sync>) -> BridgeApprovalPrompt {
        BridgeApprovalPrompt {
            emit,
            pending: Mutex::new(BTreeMap::new()),
        }
    }

    /// Resolve one pending request. Unknown ids are an error, never a silent
    /// no-op (the renderer may be stale after a reload).
    pub fn respond(
        &self,
        approval_id: &str,
        decision: ApprovalDecision,
        scope: Option<ApprovalScope>,
    ) -> Result<(), String> {
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| "the approval registry is unavailable".to_string())?;
        // Validate before removing: a stale renderer choosing a scope that was
        // not offered must not consume the prompt. The user can then answer
        // again with an offered scope.
        let allowed_scopes = pending
            .get(approval_id)
            .ok_or_else(|| "the approval request is no longer pending".to_string())?
            .allowed_scopes
            .clone();
        let answer = match decision {
            ApprovalDecision::Deny => None,
            ApprovalDecision::Allow => {
                let scope = scope.unwrap_or(ApprovalScope::Once);
                if !allowed_scopes.contains(&scope) {
                    return Err("the selected approval scope was not offered".to_string());
                }
                Some(scope)
            }
        };
        let pending = pending
            .remove(approval_id)
            .ok_or_else(|| "the approval request is no longer pending".to_string())?;
        pending
            .sender
            .send(answer)
            .map_err(|_| "the approval request is no longer waiting".to_string())
    }

    pub fn pending_count(&self) -> usize {
        self.pending
            .lock()
            .map(|pending| pending.len())
            .unwrap_or(0)
    }
}

#[async_trait]
impl ApprovalPrompt for BridgeApprovalPrompt {
    async fn approve(
        &self,
        request: ApprovalRequest<'_>,
        cancel: &CancellationToken,
    ) -> Option<ApprovalScope> {
        let id = uuid::Uuid::new_v4().to_string();
        let (sender, receiver) = oneshot::channel();
        let event = ToolApprovalEvent {
            id: id.clone(),
            session_id: request.session_id.to_string(),
            call_id: request.call_id.to_string(),
            tool_name: request.tool.to_string(),
            display_name: display_name(request.tool),
            arguments_preview: redact_preview(request.arguments),
            allowed_scopes: request.allowed_scopes.to_vec(),
        };
        {
            let Ok(mut pending) = self.pending.lock() else {
                return None;
            };
            pending.insert(
                id.clone(),
                PendingApproval {
                    allowed_scopes: request.allowed_scopes.to_vec(),
                    sender,
                },
            );
        }
        (self.emit)(event);
        tokio::select! {
            _ = cancel.cancelled() => {
                if let Ok(mut pending) = self.pending.lock() {
                    pending.remove(&id);
                }
                None
            }
            answer = receiver => answer.ok().flatten(),
        }
    }
}

/// A safe display name: the tool half of `<server>:<tool>`.
fn display_name(tool: &str) -> String {
    tool.split_once(':')
        .map(|(_, name)| name)
        .unwrap_or(tool)
        .to_string()
}

/// Redact and bound an approval preview.
///
/// Keys that look credential-bearing are replaced wholesale. Strings are
/// capped; arrays/objects are capped by count and depth. The whole preview is
/// capped by serialized size. These are rebuild-defined bounds (the shipped
/// preview rules were not captured) and are conservative on purpose.
pub fn redact_preview(arguments: &Value) -> Value {
    let redacted = redact_value(arguments, 0);
    match serde_json::to_vec(&redacted) {
        Ok(bytes) if bytes.len() <= MAX_APPROVAL_PREVIEW_BYTES => redacted,
        _ => serde_json::json!({"preview": "[truncated: the arguments are too large to show]"}),
    }
}

fn redact_value(value: &Value, depth: usize) -> Value {
    if depth > 6 {
        return Value::String("[truncated]".into());
    }
    match value {
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (key, value) in map.iter().take(64) {
                if key_looks_secret(key) {
                    out.insert(key.clone(), Value::String("[redacted]".into()));
                } else {
                    out.insert(key.clone(), redact_value(value, depth + 1));
                }
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(
            items
                .iter()
                .take(32)
                .map(|item| redact_value(item, depth + 1))
                .collect(),
        ),
        Value::String(text) => {
            let mut bounded: String = text.chars().take(512).collect();
            if text.chars().count() > 512 {
                bounded.push_str("…[truncated]");
            }
            Value::String(bounded)
        }
        other => other.clone(),
    }
}

fn key_looks_secret(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    [
        "token",
        "secret",
        "password",
        "authorization",
        "api_key",
        "apikey",
        "credential",
        "private_key",
    ]
    .iter()
    .any(|needle| key.contains(needle))
}

// ---------------------------------------------------------------------------
// MCP executor adapter
// ---------------------------------------------------------------------------

/// Executes a call against a frozen snapshot through the live manager.
pub struct McpToolExecutor {
    manager: Arc<tokio::sync::Mutex<McpManager>>,
    snapshot: ToolSnapshot,
    timeout: Option<Duration>,
}

impl McpToolExecutor {
    pub fn new(
        manager: Arc<tokio::sync::Mutex<McpManager>>,
        snapshot: ToolSnapshot,
        timeout: Option<Duration>,
    ) -> McpToolExecutor {
        McpToolExecutor {
            manager,
            snapshot,
            timeout,
        }
    }
}

#[async_trait]
impl ToolExecutor for McpToolExecutor {
    async fn execute(&self, tool: &str, arguments: Value) -> Result<ToolOutput, String> {
        let manager = self.manager.lock().await;
        let result = manager
            .call_tool(&self.snapshot, tool, arguments, self.timeout)
            .await
            .map_err(|error| error.to_string())?;
        drop(manager);
        map_tool_result(tool, result)
    }
}

/// Preserve every provider-supported MCP content part.
///
/// Audio is an explicit error until the provider SDK carries an audio part
/// (`docs/13` Phase 5 exit only requires images; audio was not captured).
/// A binary resource is a document only when its MIME type is a PDF; other
/// binaries are an explicit error rather than silent base64 text.
pub fn map_tool_result(tool: &str, result: ToolCallResult) -> Result<ToolOutput, String> {
    let mut content = Vec::new();
    for part in result.content {
        match part {
            ToolContent::Text { text } => content.push(ContentPart::Text { text }),
            ToolContent::Image { data, mime_type } => {
                content.push(ContentPart::Image {
                    media_type: mime_type,
                    data,
                });
            }
            ToolContent::Audio { .. } => {
                return Err(format!(
                    "the MCP tool {tool} returned audio, which this provider layer cannot carry"
                ));
            }
            ToolContent::Resource {
                uri,
                mime_type,
                text,
                blob,
            } => {
                if let Some(text) = text {
                    content.push(ContentPart::Text { text });
                } else if let Some(blob) = blob {
                    match mime_type.as_deref() {
                        Some("image/png") | Some("image/jpeg") | Some("image/gif") => {
                            content.push(ContentPart::Image {
                                media_type: mime_type.unwrap_or_else(|| "image/png".into()),
                                data: blob,
                            });
                        }
                        Some("application/pdf") => content.push(ContentPart::Document {
                            media_type: "application/pdf".into(),
                            data: blob,
                            name: Some(uri),
                        }),
                        _ => {
                            return Err(format!(
                                "the MCP tool {tool} returned a binary resource with an unsupported MIME type"
                            ));
                        }
                    }
                } else {
                    content.push(ContentPart::Text {
                        text: format!("[resource] {uri}"),
                    });
                }
            }
            ToolContent::ResourceLink {
                uri,
                name,
                mime_type,
            } => {
                let mime = mime_type.unwrap_or_else(|| "unknown".into());
                content.push(ContentPart::Text {
                    text: format!("[resource link] {name} ({mime}): {uri}"),
                });
            }
        }
    }
    Ok(ToolOutput {
        content,
        structured_content: result.structured_content,
        is_error: result.is_error,
    })
}

// ---------------------------------------------------------------------------
// Runtime
// ---------------------------------------------------------------------------

/// Per-process tool state shared by the chat command and the Tauri shell.
pub struct ToolRuntime {
    manager: Arc<tokio::sync::Mutex<McpManager>>,
    policy: ToolPolicy,
    approvals: tokio::sync::Mutex<ApprovalStore>,
    prompt: Arc<BridgeApprovalPrompt>,
    persistent_allowed: bool,
    scheduled_lifetime_days: u32,
    timeout: Option<Duration>,
    snapshots: Mutex<BTreeMap<String, ToolSnapshot>>,
    disabled: BTreeSet<String>,
    tool_search_enabled: bool,
    approval_file: Option<PathBuf>,
    oauth_store: Option<Arc<dyn OAuthStore>>,
    warnings: Mutex<Vec<String>>,
}

impl fmt::Debug for ToolRuntime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ToolRuntime")
            .field("persistent_allowed", &self.persistent_allowed)
            .field("tool_search_enabled", &self.tool_search_enabled)
            .finish_non_exhaustive()
    }
}

impl ToolRuntime {
    pub fn new(
        built: BuiltMcp,
        prompt: Arc<BridgeApprovalPrompt>,
        approval_file: Option<PathBuf>,
        oauth_store: Option<Arc<dyn OAuthStore>>,
    ) -> ToolRuntime {
        let timeout = built.config.tool_timeout_seconds.map(Duration::from_secs);
        let mut runtime = ToolRuntime {
            manager: built.manager,
            policy: built.config.policy.clone(),
            approvals: tokio::sync::Mutex::new(ApprovalStore::default()),
            prompt,
            persistent_allowed: built.config.persistent_always_allow,
            scheduled_lifetime_days: built.config.scheduled_approval_lifetime_days,
            timeout,
            snapshots: Mutex::new(BTreeMap::new()),
            disabled: built.config.disabled_builtin_tools.clone(),
            tool_search_enabled: built.config.tool_search_enabled,
            approval_file,
            oauth_store,
            warnings: Mutex::new(built.warnings),
        };
        runtime.load_approvals();
        runtime
    }

    pub fn manager(&self) -> Arc<tokio::sync::Mutex<McpManager>> {
        Arc::clone(&self.manager)
    }

    pub fn prompt(&self) -> Arc<BridgeApprovalPrompt> {
        Arc::clone(&self.prompt)
    }

    pub fn policy(&self) -> &ToolPolicy {
        &self.policy
    }

    pub fn tool_search_enabled(&self) -> bool {
        self.tool_search_enabled
    }

    pub fn warnings(&self) -> Vec<String> {
        self.warnings.lock().map(|w| w.clone()).unwrap_or_default()
    }

    pub fn context(&self, session_id: &str) -> ApprovalContext {
        ApprovalContext::interactive(session_id, self.persistent_allowed)
    }

    pub fn scheduled_context(&self, session_id: &str) -> ApprovalContext {
        ApprovalContext::scheduled(
            session_id,
            self.persistent_allowed,
            self.scheduled_lifetime_days,
        )
    }

    /// Start every configured server and collect failures. This is called
    /// from a Tauri background task, never the boot path.
    pub async fn start_all(&self) -> myharness_mcp::StartSummary {
        let mut manager = self.manager.lock().await;
        let summary = manager.start_all().await;
        if !summary.failed.is_empty() {
            if let Ok(mut warnings) = self.warnings.lock() {
                for failure in &summary.failed {
                    warnings.push(format!(
                        "MCP server {} failed: {}",
                        failure.server, failure.message
                    ));
                }
            }
        }
        summary
    }

    pub async fn statuses(&self) -> (Vec<ServerView>, Vec<String>) {
        let manager = self.manager.lock().await;
        let mut warnings = manager.cache_warnings().to_vec();
        if let Ok(extra) = self.warnings.lock() {
            warnings.extend(extra.iter().cloned());
        }
        (manager.statuses(), warnings)
    }

    /// Freeze the enabled set for a session. A later server refresh cannot
    /// change this snapshot (`TEST-MCP-002`).
    /// Build a snapshot without installing it. The caller persists the
    /// selection first, then installs, so a failed session write cannot leave
    /// a frozen runtime snapshot that disagrees with the stored selection.
    pub async fn prepare_session_tools(&self, names: &[String]) -> Result<ToolSnapshot, String> {
        let blocked: Vec<String> = names
            .iter()
            .filter(|name| {
                self.disabled_name(name) || self.policy.hides_tool(name, &self.context("picker"))
            })
            .cloned()
            .collect();
        if !blocked.is_empty() {
            return Err(format!(
                "these tools are disabled by policy and cannot be selected: {}",
                blocked.join(", ")
            ));
        }
        let manager = self.manager.lock().await;
        let snapshot = manager.snapshot(names);
        if !snapshot.missing.is_empty() {
            return Err(format!(
                "these tools are not available from a connected server: {}",
                snapshot.missing.join(", ")
            ));
        }
        Ok(snapshot)
    }

    fn disabled_name(&self, name: &str) -> bool {
        if self.disabled.contains(name) {
            return true;
        }
        name.split_once(':')
            .is_some_and(|(server, _)| self.disabled.contains(&format!("{server}:*")))
    }

    /// Install a prepared snapshot. This cannot fail for a missing tool; the
    /// preparation step is the validation.
    pub fn install_snapshot(&self, session_id: &str, snapshot: ToolSnapshot) -> Result<(), String> {
        self.snapshots
            .lock()
            .map_err(|_| "the tool snapshot registry is unavailable".to_string())?
            .insert(session_id.to_string(), snapshot);
        Ok(())
    }

    pub async fn set_session_tools(
        &self,
        session_id: &str,
        names: &[String],
    ) -> Result<ToolSnapshot, String> {
        let snapshot = self.prepare_session_tools(names).await?;
        self.install_snapshot(session_id, snapshot.clone())?;
        Ok(snapshot)
    }

    /// The session's frozen snapshot, creating it from persisted names on the
    /// first send. Missing names fail the send rather than shrinking it.
    pub async fn snapshot_for(
        &self,
        session_id: &str,
        names: &[String],
    ) -> Result<ToolSnapshot, String> {
        if let Ok(snapshots) = self.snapshots.lock() {
            if let Some(snapshot) = snapshots.get(session_id) {
                return Ok(snapshot.clone());
            }
        }
        self.set_session_tools(session_id, names).await
    }

    /// Tool specs for the provider request, with policy visibility applied by
    /// the agent loop. Cached-only tools are not executable and are not here.
    pub fn tool_specs(snapshot: &ToolSnapshot) -> Vec<ToolSpec> {
        snapshot
            .tools
            .iter()
            .map(|tool| ToolSpec {
                name: tool.name.clone(),
                description: tool.description.clone(),
                input_schema: tool.input_schema.clone(),
                cache_control: None,
            })
            .collect()
    }

    /// Every live executable tool. Unlike search, this ignores
    /// `toolSearchEnabled`: it is the enablement catalog, not a search result.
    pub async fn list_tools(&self) -> Vec<myharness_mcp::ToolDescriptor> {
        let manager = self.manager.lock().await;
        manager
            .list_tools()
            .into_iter()
            .filter(|tool| {
                !self.disabled_name(&tool.name)
                    && !self.policy.hides_tool(&tool.name, &self.context("picker"))
            })
            .collect()
    }

    pub async fn search_tools(&self, query: &str, limit: usize) -> Vec<ToolSearchHit> {
        let manager = self.manager.lock().await;
        manager
            .search_tools(query, limit, &self.disabled)
            .into_iter()
            .filter(|hit| {
                !self
                    .policy
                    .hides_tool(&hit.tool.name, &self.context("search"))
            })
            .collect()
    }

    /// Start/retry one configured server. A live session is left alone.
    pub async fn start_server(&self, server: &str) -> Result<(), String> {
        let mut manager = self.manager.lock().await;
        manager
            .start(server)
            .await
            .map_err(|error| error.to_string())
    }

    /// Current stored grants (session + persistent), for the audit write.
    pub async fn approval_records(&self) -> Vec<ApprovalRecord> {
        self.approvals.lock().await.records()
    }

    pub fn forget_oauth(&self, server: &str) -> Result<(), String> {
        let Some(store) = &self.oauth_store else {
            return Err("OAuth storage is not configured".to_string());
        };
        store.forget(server).map_err(|error| error.to_string())
    }

    pub fn approval_file(&self) -> Option<&Path> {
        self.approval_file.as_deref()
    }

    /// Forget one session's frozen snapshot and session-scoped grants.
    /// Persistent grants are intentionally untouched.
    pub async fn clear_session(&self, session_id: &str) {
        if let Ok(mut snapshots) = self.snapshots.lock() {
            snapshots.remove(session_id);
        }
        self.approvals.lock().await.clear_session(session_id);
        self.persist_approvals().await;
    }

    /// Persist the current approval records after a turn. Failures are
    /// warnings, never a reason to discard a completed turn.
    pub async fn persist_approvals(&self) {
        let Some(path) = &self.approval_file else {
            return;
        };
        let records = self.approvals.lock().await.records();
        if let Err(error) = write_approvals(path, &records) {
            if let Ok(mut warnings) = self.warnings.lock() {
                warnings.push(format!("MCP approvals could not be saved: {error}"));
            }
        }
    }

    fn load_approvals(&mut self) {
        let Some(path) = &self.approval_file else {
            return;
        };
        match read_approvals(path) {
            Ok(records) => {
                let mut store = ApprovalStore::default();
                let restored = store.restore(records, unix_now());
                if let Ok(mut approvals) = self.approvals.try_lock() {
                    *approvals = store;
                }
                if restored == 0 {
                    // Nothing to say; an empty/expired file is normal.
                }
            }
            Err(error) => {
                if let Ok(mut warnings) = self.warnings.lock() {
                    warnings.push(format!("MCP approvals could not be loaded: {error}"));
                }
            }
        }
    }

    /// Run one tool-enabled turn. This owns the approval lock across the turn
    /// so two sends cannot race grants.
    pub async fn run_turn<K: myharness_sessions::AuditKeyStore>(
        &self,
        run: myharness_agent::TurnRun<'_, K>,
        snapshot: &ToolSnapshot,
        session_id: &str,
        scheduled: bool,
        now: u64,
    ) -> Result<ToolTurnOutcome, myharness_agent::AgentError> {
        let executor =
            McpToolExecutor::new(Arc::clone(&self.manager), snapshot.clone(), self.timeout);
        let context = if scheduled {
            self.scheduled_context(session_id)
        } else {
            self.context(session_id)
        };
        let mut approvals = self.approvals.lock().await;
        let result = myharness_agent::run_turn_with_tools(myharness_agent::ToolTurnRun::new(
            run,
            &self.policy,
            &mut approvals,
            context,
            &executor,
            self.prompt.as_ref(),
            now,
        ))
        .await;
        drop(approvals);
        self.persist_approvals().await;
        result
    }
}

// ---------------------------------------------------------------------------
// Approval persistence
// ---------------------------------------------------------------------------

fn read_approvals(path: &Path) -> Result<Vec<ApprovalRecord>, std::io::Error> {
    match read_bounded(path, MAX_APPROVAL_FILE_BYTES, "the approval store") {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(std::io::Error::other),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}

fn write_approvals(path: &Path, records: &[ApprovalRecord]) -> Result<(), std::io::Error> {
    let bytes = serde_json::to_vec_pretty(records).map_err(std::io::Error::other)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("json.tmp");
    {
        let mut file = std::fs::File::create(&temporary)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }
        std::io::Write::write_all(&mut file, &bytes)?;
        file.sync_all()?;
    }
    std::fs::rename(&temporary, path)?;
    Ok(())
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Helpers for the chat command
// ---------------------------------------------------------------------------

/// Convert a tool-turn outcome into the renderer's aggregate usage.
pub fn aggregate_usage(outcome: &ToolTurnOutcome) -> myharness_provider_sdk::Usage {
    let mut total = myharness_provider_sdk::Usage::default();
    for usage in &outcome.usages {
        total.input = total.input.saturating_add(usage.input);
        total.output = total.output.saturating_add(usage.output);
        total.cache_read = sum_option(total.cache_read, usage.cache_read);
        total.cache_write = sum_option(total.cache_write, usage.cache_write);
        total.web_search = sum_option(total.web_search, usage.web_search);
    }
    total
}

fn sum_option(left: Option<u64>, right: Option<u64>) -> Option<u64> {
    match (left, right) {
        (None, None) => None,
        (left, right) => Some(left.unwrap_or(0).saturating_add(right.unwrap_or(0))),
    }
}

/// A compact event for the renderer's tool rows.
pub fn tool_result_event(id: &str, parts: Vec<ContentPart>) -> TurnEvent {
    TurnEvent::ToolResult {
        id: id.to_string(),
        parts,
    }
}

/// The OAuth store factory used by the Tauri shell. Kept here so the shell
/// does not depend on the keychain account naming.
pub fn keyring_oauth_store(store: Arc<dyn SecretStore>) -> Arc<dyn OAuthStore> {
    Arc::new(KeyringOAuthStore::new(store))
}

/// The OAuth HTTP client factory. The MCP host allowlist is applied to the
/// token endpoint, and redirects are disabled.
pub fn reqwest_oauth_http(
    egress: Option<EgressPolicy>,
) -> Result<Arc<dyn OAuthHttp>, OAuthError> {
    Ok(Arc::new(ReqwestOAuthHttp::with_egress(egress)?))
}

/// A status summary that never includes configuration secrets.
pub fn status_payload(servers: Vec<ServerView>, warnings: Vec<String>) -> Value {
    serde_json::json!({"servers": servers, "warnings": warnings})
}

#[cfg(test)]
mod tests {
    use super::*;
    use myharness_agent::tool_exec::ApprovalRequest;
    use serde_json::json;
    use tokio::sync::mpsc;
    use tokio_util::sync::CancellationToken;

    #[test]
    fn a_missing_or_malformed_config_is_empty_with_a_warning() {
        let dir = tempfile::TempDir::new().unwrap();
        let missing = McpConfig::load(dir.path());
        assert!(missing.config.servers.is_empty());
        assert!(missing.warnings.is_empty());

        std::fs::write(dir.path().join("servers.json"), b"{not json").unwrap();
        let malformed = McpConfig::load(dir.path());
        assert!(malformed.config.servers.is_empty());
        assert!(malformed.warnings[0].contains("not valid JSON"));
    }

    #[test]
    fn an_oversized_config_is_refused_without_reading_it_all() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("servers.json"),
            vec![b' '; MAX_MCP_CONFIG_BYTES as usize + 1],
        )
        .unwrap();
        let loaded = McpConfig::load(dir.path());
        assert!(loaded.config.servers.is_empty());
        assert!(loaded.warnings[0].contains("byte limit"));
    }

    #[tokio::test]
    async fn oauth_refresh_refuses_a_token_endpoint_outside_egress() {
        let http = ReqwestOAuthHttp::with_egress(Some(EgressPolicy::allow_hosts([
            "auth.example.test",
        ])))
        .unwrap();
        let config = OAuthConfig {
            token_url: "https://elsewhere.example/token".into(),
            client_id: "client".into(),
            scopes: Vec::new(),
        };
        let error = http.refresh(&config, "refresh-token").await.unwrap_err();
        let message = error.to_string();
        assert!(message.contains("egress"));
        assert!(!message.contains("elsewhere.example"));
    }

    #[test]
    fn a_config_round_trips_a_stdio_server_without_exposing_secret_values_in_errors() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("servers.json"),
            serde_json::to_vec(&json!({
                "servers": [{
                    "name": "echo",
                    "config": {
                        "transport": "stdio",
                        "command": "echo",
                        "args": [],
                        "env": {"TOKEN": "sk-secret"}
                    }
                }]
            }))
            .unwrap(),
        )
        .unwrap();
        let loaded = McpConfig::load(dir.path());
        assert_eq!(loaded.config.servers.len(), 1);
        assert_eq!(loaded.config.servers[0].name, "echo");
        assert!(loaded.warnings.is_empty());
        // Debug redacts env values; the error path never carries them either.
        let debug = format!("{:?}", loaded.config.servers[0].config);
        assert!(!debug.contains("sk-secret"));
    }

    #[test]
    fn duplicate_server_names_are_refused_before_a_manager_is_built() {
        let config = McpConfig {
            servers: vec![
                McpServerEntry {
                    name: "echo".into(),
                    config: ServerConfig::Stdio {
                        command: "echo".into(),
                        args: Vec::new(),
                        env: BTreeMap::new(),
                    },
                    oauth: None,
                    source: "developer".into(),
                    enabled: true,
                },
                McpServerEntry {
                    name: "echo".into(),
                    config: ServerConfig::Stdio {
                        command: "echo".into(),
                        args: Vec::new(),
                        env: BTreeMap::new(),
                    },
                    oauth: None,
                    source: "developer".into(),
                    enabled: true,
                },
            ],
            ..McpConfig::default()
        };
        assert!(config.validate().unwrap_err().contains("duplicate"));
    }

    #[test]
    fn preview_redacts_credential_shaped_keys_and_bounds_strings() {
        let preview = redact_preview(&json!({
            "repo": "prod",
            "api_key": "sk-secret",
            "nested": {"password": "hunter2"},
            "text": "x".repeat(2_000)
        }));
        assert_eq!(preview["api_key"], "[redacted]");
        assert_eq!(preview["nested"]["password"], "[redacted]");
        assert_eq!(preview["repo"], "prod");
        assert!(preview["text"].as_str().unwrap().chars().count() <= 530);
    }

    #[test]
    fn an_oversized_preview_is_replaced_by_an_explicit_marker() {
        let huge: Vec<String> = (0..100).map(|_| "x".repeat(500)).collect();
        let preview = redact_preview(&json!({"items": huge}));
        assert!(preview.get("preview").is_some());
    }

    #[test]
    fn a_resource_link_is_preserved_as_text() {
        let output = map_tool_result(
            "s:t",
            ToolCallResult {
                content: vec![ToolContent::ResourceLink {
                    uri: "https://example.test/x".into(),
                    name: "x".into(),
                    mime_type: Some("text/plain".into()),
                }],
                structured_content: None,
                is_error: false,
            },
        )
        .unwrap();
        assert!(matches!(
            &output.content[0],
            ContentPart::Text { text } if text.contains("https://example.test/x")
        ));
    }

    #[tokio::test]
    async fn an_approval_event_round_trips_and_rejects_a_scope_that_was_not_offered() {
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let prompt = std::sync::Arc::new(BridgeApprovalPrompt::new(std::sync::Arc::new(
            move |event| {
                let _ = sender.send(event);
            },
        )));
        let waiting = std::sync::Arc::clone(&prompt);
        let cancel = CancellationToken::new();
        let task = tokio::spawn(async move {
            waiting
                .approve(
                    ApprovalRequest {
                        session_id: "session-1",
                        call_id: "call-1",
                        tool: "s:t",
                        arguments: &json!({"repo": "prod", "api_key": "sk-secret"}),
                        allowed_scopes: &[ApprovalScope::Once, ApprovalScope::Session],
                    },
                    &cancel,
                )
                .await
        });
        let event = receiver.recv().await.expect("approval event");
        assert_eq!(event.arguments_preview["api_key"], "[redacted]");
        assert_eq!(event.arguments_preview["repo"], "prod");
        assert!(prompt
            .respond(
                &event.id,
                ApprovalDecision::Allow,
                Some(ApprovalScope::Persistent)
            )
            .is_err());
        prompt
            .respond(
                &event.id,
                ApprovalDecision::Allow,
                Some(ApprovalScope::Session),
            )
            .unwrap();
        assert_eq!(task.await.unwrap(), Some(ApprovalScope::Session));
    }

    #[test]
    fn audio_is_an_explicit_error() {
        let error = map_tool_result(
            "s:t",
            ToolCallResult {
                content: vec![ToolContent::Audio {
                    data: "AAAA".into(),
                    mime_type: "audio/wav".into(),
                }],
                structured_content: None,
                is_error: false,
            },
        )
        .unwrap_err();
        assert!(error.contains("audio"));
    }
}
