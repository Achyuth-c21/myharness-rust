//! The `web.LocalAgentModeSessions` bridge namespace — roadmap Phase 4
//! slice 3 (`docs/13` Phase 4; `docs/18` §4; `docs/11` §14).
//!
//! This module is the composition seam the agent loop deliberately leaves
//! open: it resolves the **applied profile** to a native adapter (same
//! credential resolution as the setup probes), starts/finishes sessions
//! through [`SessionService`], and streams a turn back to the renderer through
//! an observer. It owns no protocol knowledge — every adapter speaks its own
//! wire (`docs/12` §8.1) — and it never substitutes a profile, model, or
//! credential (`AI-MODEL-01`, `AI-PROVIDER-04`).
//!
//! Rules bound to this namespace:
//!
//! - **The renderer is untrusted input** (`docs/10` §8, `ARCH-PROC-03`): ids
//!   are validated here, text/title lengths are bounded, and the timezone
//!   offset is clamped.
//! - **Secrets never cross back** (`docs/18` §10): the credential lives inside
//!   [`AuthContext`] for one turn; errors carry the provider's own message and
//!   never a key.
//! - **Stop is idempotent** (`TEST-SESSION-001` stop clause): the cancel token
//!   is keyed by session and cancelling an idle session is a no-op.
//! - **Opening a session recovers an interrupted stream first**
//!   (`TEST-SESSION-002`): the live row becomes an explicit error turn.
//!
//! Audit keys for the app live in the OS keychain ([`KeyringAuditKeys`],
//! `docs/11` §4.3); the in-memory implementation remains a test seam.

use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::FixedOffset;
use myharness_agent::{run_turn, AgentError, TurnContent, TurnInput, TurnRun};
use myharness_provider_anthropic::AnthropicAdapter;
use myharness_provider_bedrock::BedrockAdapter;
use myharness_provider_gateway::{GatewayAdapter, GatewayClientConfig};
use myharness_provider_openai::{AdapterKind, OpenAiClientConfig, OpenAiCompatAdapter};
use myharness_provider_sdk::{AuthContext, ModelKey, ProviderAdapter, TurnEvent};
use myharness_secrets::SecretStore;
use myharness_sessions::{AuditKeyStore, SessionError, SessionService, UsageSummary};
use myharness_store::{NewSession, SessionRow, Store, TurnRow};
use rand::{rngs::OsRng, RngCore};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use crate::setup::{
    bridge_provider_error, bridge_secret_error, custom_headers_for_probe, first_probe_model,
    parse_probe_scheme, resolve_optional_probe_credential, resolve_probe_credential, BridgeError,
    ConfigBackend, ProbeCredential,
};

/// The default output budget when the renderer does not name one. No profile
/// field carries this in the rebuild (the shipped composer has no such
/// control); the value is an app decision, recorded as such rather than
/// inferred from the captures.
pub const DEFAULT_MAX_TOKENS: u32 = 4096;

/// The largest output budget the bridge will forward. A caller asking for
/// more is a mistake, not a model capability claim.
const MAX_MAX_TOKENS: u32 = 32_768;

const DEFAULT_USAGE_DAYS: u32 = 7;
const MAX_TITLE_CHARS: usize = 200;

// ---------------------------------------------------------------------------
// Target resolution (profile → native adapter)
// ---------------------------------------------------------------------------

/// One resolved send: a native adapter, its credential, and the model it may
/// speak. Constructed per request so credential rotation is honored
/// (`docs/13` Phase 3).
pub struct ChatTarget {
    pub adapter: Box<dyn ProviderAdapter>,
    pub auth: AuthContext,
    pub model: ModelKey,
    pub profile_id: String,
}

/// The seam tests replace with a scripted adapter. Production resolves the
/// applied profile through [`ConfigBackend`].
pub trait ChatTargetResolver: Send + Sync {
    fn resolve(&self) -> Result<ChatTarget, BridgeError>;
}

/// Production resolver over the same backend the setup window uses.
pub struct ProfileTargets {
    backend: &'static Mutex<ConfigBackend>,
}

impl ProfileTargets {
    pub fn new(backend: &'static Mutex<ConfigBackend>) -> ProfileTargets {
        ProfileTargets { backend }
    }
}

impl ChatTargetResolver for ProfileTargets {
    fn resolve(&self) -> Result<ChatTarget, BridgeError> {
        let backend = self.backend.lock().map_err(|_| {
            BridgeError::new("CONFIG_IO", "the configuration lock is unavailable")
        })?;
        backend.chat_target()
    }
}

impl ConfigBackend {
    /// Resolve the applied profile to its native adapter. The same credential
    /// resolver the setup probes use runs here with no typed values: stored
    /// handles and helper scripts only, never a fallback.
    ///
    /// A profile that names no model is refused before a session exists — the
    /// rebuild has no model discovery at send time (`docs/14` §9 records the
    /// choice; the renderer has no model picker yet).
    pub fn chat_target(&self) -> Result<ChatTarget, BridgeError> {
        let library = self.library();
        let active = library.active()?.ok_or_else(|| {
            BridgeError::new(
                "CONFIG_MISSING",
                "no profile is applied; apply a profile in Settings before sending",
            )
        })?;
        let document = library.read(&active.id)?;
        let provider = document.provider.ok_or_else(|| {
            BridgeError::new("CONFIG_VALIDATION", "the applied profile names no provider")
        })?;
        let connection = &document.connection.0;
        let model_id = first_probe_model(connection).ok_or_else(|| {
            BridgeError::new(
                "CONFIG_VALIDATION",
                "the applied profile names no model; add one in Connection",
            )
        })?;

        let (adapter, secret): (Box<dyn ProviderAdapter>, SecretString) = match provider {
            myharness_config::ProviderKind::Gateway => {
                let base = optional_string(connection, "inferenceGatewayBaseUrl")?.ok_or_else(
                    || {
                        BridgeError::new(
                            "CONFIG_VALIDATION",
                            "the gateway base URL is required",
                        )
                    },
                )?;
                let scheme = parse_probe_scheme(connection.get("inferenceGatewayAuthScheme"))?;
                let secret = required_secret(
                    connection,
                    self.secrets(),
                    "inferenceGatewayApiKey",
                    true,
                )?;
                let adapter = GatewayAdapter::new(
                    GatewayClientConfig {
                        base_url: base,
                        auth_scheme: scheme,
                        custom_headers: custom_headers_for_probe(connection)?,
                        idle_timeout: idle_timeout(
                            connection,
                            myharness_provider_gateway::DEFAULT_IDLE_TIMEOUT,
                        ),
                        betas: Vec::new(),
                    },
                    reqwest::Client::new(),
                );
                (Box::new(adapter), secret)
            }
            myharness_config::ProviderKind::ClaudeApi => {
                let secret = required_secret(
                    connection,
                    self.secrets(),
                    "inferenceAnthropicApiKey",
                    false,
                )?;
                let adapter = AnthropicAdapter::new(
                    myharness_provider_anthropic::AnthropicClientConfig {
                        base_url: optional_string(connection, "inferenceAnthropicBaseUrl")?,
                        custom_headers: custom_headers_for_probe(connection)?,
                        idle_timeout: myharness_provider_anthropic::DEFAULT_IDLE_TIMEOUT,
                        betas: Vec::new(),
                    },
                    reqwest::Client::new(),
                );
                (Box::new(adapter), secret)
            }
            myharness_config::ProviderKind::Bedrock => {
                let region = optional_string(connection, "inferenceBedrockRegion")?
                    .ok_or_else(|| {
                        BridgeError::new(
                            "CONFIG_VALIDATION",
                            "the Bedrock region is required",
                        )
                    })?;
                let (runtime_base, control_base) =
                    match optional_string(connection, "inferenceBedrockBaseUrl")? {
                        Some(base) => (base.clone(), base),
                        None => (
                            myharness_provider_bedrock::default_runtime_base_url(&region)
                                .map_err(bridge_provider_error)?
                                .trim_end_matches('/')
                                .to_string(),
                            myharness_provider_bedrock::default_control_base_url(&region)
                                .map_err(bridge_provider_error)?
                                .trim_end_matches('/')
                                .to_string(),
                        ),
                    };
                let secret = required_secret(
                    connection,
                    self.secrets(),
                    "inferenceBedrockBearerToken",
                    true,
                )?;
                let adapter = BedrockAdapter::new(
                    myharness_provider_bedrock::BedrockClientConfig {
                        runtime_base_url: runtime_base,
                        control_base_url: control_base,
                        custom_headers: custom_headers_for_probe(connection)?,
                        idle_timeout: myharness_provider_bedrock::DEFAULT_IDLE_TIMEOUT,
                    },
                    reqwest::Client::new(),
                );
                (Box::new(adapter), secret)
            }
            myharness_config::ProviderKind::OpenAi => {
                let base =
                    optional_string(connection, "inferenceOpenAiBaseUrl")?.unwrap_or_default();
                let secret = optional_secret(
                    connection,
                    self.secrets(),
                    "inferenceOpenAiApiKey",
                    false,
                )?;
                let adapter = OpenAiCompatAdapter::new(
                    OpenAiClientConfig {
                        kind: AdapterKind::OpenAi,
                        base_url: base,
                        idle_timeout: idle_timeout(
                            connection,
                            myharness_provider_openai::DEFAULT_IDLE_TIMEOUT,
                        ),
                        custom_headers: custom_headers_for_probe(connection)?,
                    },
                    reqwest::Client::new(),
                );
                (Box::new(adapter), secret)
            }
            myharness_config::ProviderKind::Custom => {
                let base = optional_string(connection, "inferenceCustomBaseUrl")?.ok_or_else(
                    || {
                        BridgeError::new(
                            "CONFIG_VALIDATION",
                            "the custom base URL is required",
                        )
                    },
                )?;
                let secret = optional_secret(
                    connection,
                    self.secrets(),
                    "inferenceCustomApiKey",
                    true,
                )?;
                let adapter = OpenAiCompatAdapter::new(
                    OpenAiClientConfig {
                        kind: AdapterKind::Custom,
                        base_url: base,
                        idle_timeout: idle_timeout(
                            connection,
                            myharness_provider_openai::DEFAULT_IDLE_TIMEOUT,
                        ),
                        custom_headers: custom_headers_for_probe(connection)?,
                    },
                    reqwest::Client::new(),
                );
                (Box::new(adapter), secret)
            }
            // Locked until their adapters land (`docs/04` §3.1): a chat send
            // must refuse, never route through another family.
            myharness_config::ProviderKind::BedrockMantle => {
                return Err(BridgeError::new(
                    "ADAPTER_NOT_AVAILABLE",
                    "the Bedrock Mantle adapter is not part of this build",
                ));
            }
            myharness_config::ProviderKind::Foundry => {
                return Err(BridgeError::new(
                    "ADAPTER_NOT_AVAILABLE",
                    "the Foundry adapter is not part of this build",
                ));
            }
        };

        let provider_id = provider_id(provider);
        let model = ModelKey {
            provider_id: provider_id.to_string(),
            model_id,
            variant: None,
            revision: None,
        };
        Ok(ChatTarget {
            adapter,
            auth: AuthContext::new(secret, "chat turn"),
            model,
            profile_id: active.id,
        })
    }
}

fn provider_id(provider: myharness_config::ProviderKind) -> &'static str {
    use myharness_config::ProviderKind;
    match provider {
        ProviderKind::ClaudeApi => "anthropic",
        ProviderKind::Bedrock => "bedrock",
        ProviderKind::BedrockMantle => "mantle",
        ProviderKind::Foundry => "foundry",
        ProviderKind::Gateway => "gateway",
        ProviderKind::OpenAi => "openai",
        ProviderKind::Custom => "custom",
    }
}

fn required_secret(
    connection: &Value,
    store: &dyn SecretStore,
    slot: &'static str,
    allow_helper: bool,
) -> Result<SecretString, BridgeError> {
    match resolve_probe_credential(connection, &BTreeMap::new(), store, slot, allow_helper)? {
        ProbeCredential::Value(secret) => Ok(secret),
        ProbeCredential::Helper(config) => {
            myharness_secrets::helper::run_helper(&config, cfg!(windows))
                .map_err(bridge_secret_error)
        }
    }
}

fn optional_secret(
    connection: &Value,
    store: &dyn SecretStore,
    slot: &'static str,
    allow_helper: bool,
) -> Result<SecretString, BridgeError> {
    match resolve_optional_probe_credential(
        connection,
        &BTreeMap::new(),
        store,
        slot,
        allow_helper,
    )? {
        Some(ProbeCredential::Value(secret)) => Ok(secret),
        Some(ProbeCredential::Helper(config)) => {
            myharness_secrets::helper::run_helper(&config, cfg!(windows))
                .map_err(bridge_secret_error)
        }
        // The family documents a keyless send (custom, and first-party
        // OpenAI whose adapter refuses an empty credential itself).
        None => Ok(SecretString::from(String::new())),
    }
}

fn optional_string(connection: &Value, key: &str) -> Result<Option<String>, BridgeError> {
    match connection.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(raw)) => {
            let trimmed = raw.trim();
            Ok(if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            })
        }
        Some(other) => Err(BridgeError::new(
            "CONFIG_VALIDATION",
            format!("{key} expects a string, found {other:?}"),
        )),
    }
}

fn idle_timeout(connection: &Value, default: std::time::Duration) -> std::time::Duration {
    connection
        .get("inferenceStreamIdleTimeoutSec")
        .and_then(Value::as_u64)
        .map(std::time::Duration::from_secs)
        .unwrap_or(default)
}

// ---------------------------------------------------------------------------
// Audit keys in the OS keychain
// ---------------------------------------------------------------------------

/// The app's audit-key store: one keychain entry per session under
/// `session-audit/<uuid>` (`docs/11` §4.3). A missing entry is `None`; a
/// backend failure is an error, never a regenerated key.
pub struct KeyringAuditKeys {
    store: Box<dyn SecretStore>,
}

impl KeyringAuditKeys {
    pub fn new(store: Box<dyn SecretStore>) -> KeyringAuditKeys {
        KeyringAuditKeys { store }
    }

    fn account(session_id: &str) -> String {
        format!("session-audit/{session_id}")
    }
}

impl AuditKeyStore for KeyringAuditKeys {
    fn create(&self, session_id: &str) -> Result<[u8; 32], SessionError> {
        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);
        let value = SecretString::from(hex_encode(&key));
        self.store
            .store(&Self::account(session_id), &value)
            .map_err(|_| SessionError::AuditKeyStore)?;
        Ok(key)
    }

    fn get(&self, session_id: &str) -> Result<Option<[u8; 32]>, SessionError> {
        match self.store.load(&Self::account(session_id)) {
            Ok(None) => Ok(None),
            Ok(Some(value)) => {
                use secrecy::ExposeSecret;
                hex_decode(value.expose_secret()).map(Some).ok_or(SessionError::AuditKeyStore)
            }
            Err(_) => Err(SessionError::AuditKeyStore),
        }
    }

    fn remove(&self, session_id: &str) -> Result<(), SessionError> {
        self.store
            .delete(&Self::account(session_id))
            .map(|_| ())
            .map_err(|_| SessionError::AuditKeyStore)
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn hex_decode(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64 {
        return None;
    }
    let bytes = text.as_bytes();
    let mut out = [0u8; 32];
    for (index, slot) in out.iter_mut().enumerate() {
        let pair = std::str::from_utf8(&bytes[index * 2..index * 2 + 2]).ok()?;
        *slot = u8::from_str_radix(pair, 16).ok()?;
    }
    Some(out)
}

// ---------------------------------------------------------------------------
// The command surface
// ---------------------------------------------------------------------------

/// One command in the rebuild's `web.LocalAgentModeSessions` subset
/// (`docs/18` §3.4/§4). The set is enumerated on purpose — no generic CRUD.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ChatCommand {
    /// The sidebar/home list; `archived` narrows to one side.
    ListSessions {
        #[serde(default)]
        archived: Option<bool>,
    },
    /// Create the session before the first send ("sending creates a session
    /// immediately", docs/features/chat-and-composer.md).
    CreateSession {
        #[serde(default)]
        title: Option<String>,
    },
    /// The transcript, after crash recovery.
    OpenSession { id: String },
    SendTurn {
        session_id: String,
        text: String,
        #[serde(default)]
        max_tokens: Option<u32>,
    },
    /// Idempotent: cancelling an idle session reports `stopped: false`.
    StopTurn { session_id: String },
    RenameSession { id: String, title: String },
    ArchiveSession { id: String, archived: bool },
    DeleteSession { id: String },
    /// The Usage page. `utcOffsetMinutes` is minutes east of UTC, computed by
    /// the renderer from the device zone; absent falls back to the process
    /// zone.
    UsageSummary {
        #[serde(default)]
        days: Option<u32>,
        #[serde(default)]
        utc_offset_minutes: Option<i32>,
    },
}

/// One command's answer.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ChatResponse {
    Sessions { sessions: Vec<SessionRow> },
    Session { session: SessionView },
    Turn {
        session_id: String,
        user_turn: TurnSummary,
        assistant_turn: TurnSummary,
        stop_reason: Option<String>,
        usage: Option<Value>,
    },
    Stopped { stopped: bool },
    Ack,
    Usage { summary: UsageSummary },
    Error {
        error: BridgeError,
        /// True when the user turn was already persisted before the failure,
        /// so the renderer knows the draft became a transcript entry and must
        /// not be restored (`TEST-SESSION-009` boundary).
        user_turn_recorded: bool,
    },
}

/// A session with its transcript.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    pub session: SessionRow,
    pub turns: Vec<TurnSummary>,
}

/// One stored turn in the shape the transcript renders. Content is the agent
/// crate's transcript JSON (`TurnContent`), decoded here so the renderer never
/// reads blobs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnSummary {
    pub id: String,
    pub seq: i64,
    pub role: String,
    pub status: String,
    pub stop_reason: Option<String>,
    pub error: Option<String>,
    pub created_at: i64,
    pub content: TurnContent,
}

struct ChatFailure {
    error: BridgeError,
    user_turn_recorded: bool,
}

impl ChatFailure {
    fn new(error: BridgeError, user_turn_recorded: bool) -> ChatFailure {
        ChatFailure {
            error,
            user_turn_recorded,
        }
    }

    fn before_send(error: BridgeError) -> ChatFailure {
        ChatFailure::new(error, false)
    }
}

impl From<BridgeError> for ChatFailure {
    fn from(error: BridgeError) -> ChatFailure {
        ChatFailure::before_send(error)
    }
}

fn session_failure(error: impl std::fmt::Display) -> ChatFailure {
    ChatFailure::before_send(BridgeError::new("SESSION_ERROR", error.to_string()))
}

// ---------------------------------------------------------------------------
// The service
// ---------------------------------------------------------------------------

/// Sessions + target resolution + the running-turn registry. One instance per
/// app process (the Tauri shell owns the `OnceLock`); tests construct their
/// own with a scripted resolver.
pub struct ChatService<K: AuditKeyStore, R: ChatTargetResolver> {
    sessions: SessionService<K>,
    targets: R,
    running: Mutex<HashMap<String, CancellationToken>>,
}

impl<K: AuditKeyStore, R: ChatTargetResolver> ChatService<K, R> {
    pub fn new(sessions: SessionService<K>, targets: R) -> ChatService<K, R> {
        ChatService {
            sessions,
            targets,
            running: Mutex::new(HashMap::new()),
        }
    }

    /// The underlying session service (diagnostics and tests).
    pub fn sessions(&self) -> &SessionService<K> {
        &self.sessions
    }
}

/// Execute one command. Streaming events reach `observer` before they are
/// accumulated; the terminal `Turn`/`Error` response is the final state.
pub async fn dispatch<K: AuditKeyStore, R: ChatTargetResolver>(
    service: &ChatService<K, R>,
    command: ChatCommand,
    observer: Option<&(dyn Fn(&TurnEvent) + Send + Sync)>,
) -> ChatResponse {
    match run(service, command, observer).await {
        Ok(response) => response,
        Err(failure) => ChatResponse::Error {
            error: failure.error,
            user_turn_recorded: failure.user_turn_recorded,
        },
    }
}

async fn run<K: AuditKeyStore, R: ChatTargetResolver>(
    service: &ChatService<K, R>,
    command: ChatCommand,
    observer: Option<&(dyn Fn(&TurnEvent) + Send + Sync)>,
) -> Result<ChatResponse, ChatFailure> {
    match command {
        ChatCommand::ListSessions { archived } => {
            let sessions = service
                .sessions
                .store()
                .list_sessions(archived)
                .map_err(session_failure)?;
            Ok(ChatResponse::Sessions { sessions })
        }
        ChatCommand::CreateSession { title } => {
            let title = normalise_title(title)?;
            let target = service.targets.resolve()?;
            let session = service
                .sessions
                .start(NewSession {
                    surface: "chat",
                    title: &title,
                    cwd: None,
                    model: Some(&target.model.model_id),
                    effort: None,
                    permission_mode: None,
                    extra: serde_json::json!({
                        "providerId": target.model.provider_id,
                        "profileId": target.profile_id,
                    }),
                })
                .map_err(session_failure)?;
            Ok(ChatResponse::Session {
                session: SessionView {
                    session,
                    turns: Vec::new(),
                },
            })
        }
        ChatCommand::OpenSession { id } => {
            validate_session_id(&id)?;
            service
                .sessions
                .recover_interrupted(&id)
                .map_err(session_failure)?;
            let session = service
                .sessions
                .store()
                .read_session(&id)
                .map_err(session_failure)?;
            let turns = service
                .sessions
                .store()
                .list_turns(&id)
                .map_err(session_failure)?
                .into_iter()
                .map(|turn| turn_summary(service.sessions.store(), turn))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(ChatResponse::Session {
                session: SessionView { session, turns },
            })
        }
        ChatCommand::SendTurn {
            session_id,
            text,
            max_tokens,
        } => service.send_turn(&session_id, &text, max_tokens, observer).await,
        ChatCommand::StopTurn { session_id } => {
            validate_session_id(&session_id)?;
            let stopped = service.stop_turn(&session_id)?;
            Ok(ChatResponse::Stopped { stopped })
        }
        ChatCommand::RenameSession { id, title } => {
            validate_session_id(&id)?;
            let title = normalise_title(Some(title))?;
            service
                .sessions
                .rename(&id, &title)
                .map_err(session_failure)?;
            Ok(ChatResponse::Ack)
        }
        ChatCommand::ArchiveSession { id, archived } => {
            validate_session_id(&id)?;
            service
                .sessions
                .archive(&id, archived)
                .map_err(session_failure)?;
            Ok(ChatResponse::Ack)
        }
        ChatCommand::DeleteSession { id } => {
            validate_session_id(&id)?;
            service.sessions.delete(&id).map_err(session_failure)?;
            Ok(ChatResponse::Ack)
        }
        ChatCommand::UsageSummary {
            days,
            utc_offset_minutes,
        } => service.usage(days, utc_offset_minutes),
    }
}

impl<K: AuditKeyStore, R: ChatTargetResolver> ChatService<K, R> {
    async fn send_turn(
        &self,
        session_id: &str,
        text: &str,
        max_tokens: Option<u32>,
        observer: Option<&(dyn Fn(&TurnEvent) + Send + Sync)>,
    ) -> Result<ChatResponse, ChatFailure> {
        validate_session_id(session_id)?;
        if text.trim().is_empty() {
            return Err(ChatFailure::before_send(BridgeError::new(
                "CONFIG_VALIDATION",
                "the message is empty",
            )));
        }
        let max_tokens = max_tokens.unwrap_or(DEFAULT_MAX_TOKENS);
        if max_tokens == 0 || max_tokens > MAX_MAX_TOKENS {
            return Err(ChatFailure::before_send(BridgeError::new(
                "CONFIG_VALIDATION",
                format!("maxTokens must be between 1 and {MAX_MAX_TOKENS}"),
            )));
        }
        let session = self
            .sessions
            .store()
            .read_session(session_id)
            .map_err(session_failure)?;
        if session.surface != "chat" {
            return Err(ChatFailure::before_send(BridgeError::new(
                "CONFIG_VALIDATION",
                format!("session surface {:?} cannot be sent from the chat surface", session.surface),
            )));
        }

        // Resolve before claiming the slot: a missing credential or model
        // must fail without leaving a running marker behind.
        let target = self.targets.resolve()?;
        let cancel = CancellationToken::new();
        {
            let mut running = self.running.lock().map_err(|_| {
                ChatFailure::before_send(BridgeError::new(
                    "SESSION_ERROR",
                    "the turn registry is unavailable",
                ))
            })?;
            if running.contains_key(session_id) {
                return Err(ChatFailure::before_send(BridgeError::new(
                    "TURN_IN_PROGRESS",
                    "a turn is already running in this session",
                )));
            }
            running.insert(session_id.to_string(), cancel.clone());
        }

        let input = TurnInput::text(target.model.clone(), max_tokens, text);
        let mut turn = TurnRun::new(
            &self.sessions,
            target.adapter.as_ref(),
            &target.auth,
            session_id,
            input,
            &cancel,
        );
        if let Some(observer) = observer {
            turn = turn.with_observer(observer);
        }
        let result = run_turn(turn).await;

        if let Ok(mut running) = self.running.lock() {
            running.remove(session_id);
        }

        match result {
            Ok(outcome) => Ok(ChatResponse::Turn {
                session_id: session_id.to_string(),
                user_turn: turn_summary(self.sessions.store(), outcome.user_turn)?,
                assistant_turn: turn_summary(self.sessions.store(), outcome.assistant_turn)?,
                stop_reason: Some(outcome.stop_reason.wire().to_string()),
                usage: serde_json::to_value(&outcome.usage).ok(),
            }),
            Err(AgentError::Provider(error)) => Err(ChatFailure::new(
                bridge_provider_error(error),
                true,
            )),
            Err(AgentError::Cancelled) => Err(ChatFailure::new(
                BridgeError::new("TURN_CANCELLED", "the turn was cancelled"),
                true,
            )),
            Err(AgentError::Protocol(message)) => Err(ChatFailure::new(
                BridgeError::new("PROVIDER_ERROR", message),
                true,
            )),
            Err(AgentError::Validation(message)) => Err(ChatFailure::before_send(
                BridgeError::new("CONFIG_VALIDATION", message),
            )),
            Err(AgentError::Unsupported(message)) => Err(ChatFailure::before_send(
                BridgeError::new("UNSUPPORTED", message),
            )),
            Err(AgentError::ProviderMismatch {
                model_provider,
                adapter,
            }) => Err(ChatFailure::before_send(BridgeError::new(
                "CONFIG_VALIDATION",
                format!("the model belongs to provider {model_provider}, but this adapter is {adapter}"),
            ))),
            Err(error @ AgentError::Transcript(_)) => Err(session_failure(error)),
            Err(error @ AgentError::Session(_)) => Err(session_failure(error)),
            Err(error @ AgentError::Store(_)) => Err(session_failure(error)),
        }
    }

    fn stop_turn(&self, session_id: &str) -> Result<bool, ChatFailure> {
        let running = self.running.lock().map_err(|_| {
            ChatFailure::before_send(BridgeError::new(
                "SESSION_ERROR",
                "the turn registry is unavailable",
            ))
        })?;
        match running.get(session_id) {
            Some(token) => {
                token.cancel();
                Ok(true)
            }
            None => Ok(false),
        }
    }

    fn usage(
        &self,
        days: Option<u32>,
        utc_offset_minutes: Option<i32>,
    ) -> Result<ChatResponse, ChatFailure> {
        let days = days.unwrap_or(DEFAULT_USAGE_DAYS);
        let tz = match utc_offset_minutes {
            Some(minutes) => {
                if !(-840..=840).contains(&minutes) {
                    return Err(ChatFailure::before_send(BridgeError::new(
                        "CONFIG_VALIDATION",
                        "the timezone offset must be between -840 and 840 minutes",
                    )));
                }
                FixedOffset::east_opt(minutes * 60).ok_or_else(|| {
                    ChatFailure::before_send(BridgeError::new(
                        "CONFIG_VALIDATION",
                        "the timezone offset is out of range",
                    ))
                })?
            }
            None => *chrono::Local::now().offset(),
        };
        let summary = self
            .sessions
            .usage_summary(None, days, tz, now_ms())
            .map_err(session_failure)?;
        Ok(ChatResponse::Usage { summary })
    }
}

fn normalise_title(title: Option<String>) -> Result<String, ChatFailure> {
    let title = title
        .map(|title| title.trim().to_string())
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| "New chat".to_string());
    if title.chars().count() > MAX_TITLE_CHARS {
        return Err(ChatFailure::before_send(BridgeError::new(
            "CONFIG_VALIDATION",
            format!("the title is longer than {MAX_TITLE_CHARS} characters"),
        )));
    }
    Ok(title)
}

fn validate_session_id(id: &str) -> Result<(), ChatFailure> {
    uuid::Uuid::parse_str(id).map(|_| ()).map_err(|_| {
        ChatFailure::before_send(BridgeError::new(
            "CONFIG_VALIDATION",
            "the session id is not a uuid",
        ))
    })
}

fn turn_summary(store: &Store, turn: TurnRow) -> Result<TurnSummary, ChatFailure> {
    let bytes = store
        .files
        .read(&turn.content_ref)
        .map_err(session_failure)?
        .ok_or_else(|| {
            ChatFailure::before_send(BridgeError::new(
                "SESSION_ERROR",
                format!("turn {} has no stored content", turn.seq),
            ))
        })?;
    let content: TurnContent = serde_json::from_slice(&bytes).map_err(|_| {
        ChatFailure::before_send(BridgeError::new(
            "SESSION_ERROR",
            format!("turn {} content is not in the transcript shape", turn.seq),
        ))
    })?;
    Ok(TurnSummary {
        id: turn.id,
        seq: turn.seq,
        role: turn.role,
        status: turn.status,
        stop_reason: turn.stop_reason,
        error: turn.error,
        created_at: turn.created_at,
        content,
    })
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}
