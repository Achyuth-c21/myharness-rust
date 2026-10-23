//! The provider adapter contract — roadmap Phase 3 (`docs/13` §C Phase 3; `docs/12`
//! §8.1–§8.4; module map: `docs/10` §9).
//!
//! An adapter is **not** a routing layer. It is the only place that knows how to
//! talk to one provider family, and it speaks that family's native protocol
//! directly (`docs/12` §8.1). The types in this crate are the **app-internal
//! model** of a conversation, not a provider wire format: `docs/12` §8.2 says
//! explicitly that mapping a provider's native events onto this small typed set
//! "is normal application modeling, not cross-protocol translation" — what is
//! forbidden (`AI-PROVIDER-02`, `AI-EXT-02`) is translating *between* two
//! providers' wire protocols, or rewriting one protocol into another as a
//! hidden runtime shim.
//!
//! What is deliberately NOT here:
//!
//! - Any HTTP client, endpoint construction, or host policy: adapters own that
//!   (`AI-PROVIDER-01`). This crate just names the contract.
//! - Any credential store: adapters receive resolved credential material through
//!   an [`AuthContext`]; the keychain, helper runner, and slot resolution live in
//!   `myharness-secrets` and the main process (`AI-PROVIDER-03`).
//! - Retry, model-preference, or fallback policy: the agent layer (Phase 4) owns
//!   it, and its default is **no fallback** (`AI-MODEL-01`).
//!
//! Traceability: `AI-PROVIDER-01..05`, `AI-MODEL-01/02`, `AI-STREAM-01/02`,
//! `AI-TOOL-01`, `AI-MM-01/02`, `AI-USAGE-01`, `TEST-AI-021..023`.

use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Provider identity
// ---------------------------------------------------------------------------

/// The native protocol an adapter speaks (`docs/12` §8.1:
/// "adapter contract: `protocol: ProtocolFamily`; native family, not a target
/// for translation").
///
/// Every family in the shipped app is Anthropic Messages, in one transport or
/// another (`docs/12` §1 native-protocol column); the variant carries the
/// transport. Non-Anthropic families (`docs/12` §8.4 lists them as registry
/// entries, not shipped behavior) arrive as their own adapters with their own
/// policy — never through a translation layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProtocolFamily {
    /// Anthropic Messages: Claude API, and the Anthropic-compatible Gateway's
    /// `/v1/messages` + `/v1/models` contract.
    AnthropicMessages,
    /// Anthropic Messages over a cloud transport ("via Bedrock Runtime", "via
    /// Foundry" — `docs/12` §1; the shipped set also lists "via Vertex", removed
    /// from rebuild scope, `docs/04` §3.1). The string is the provider id.
    AnthropicMessagesViaCloud(&'static str),
}

// ---------------------------------------------------------------------------
// Model identity and descriptors (`docs/12` §8.3, §4 rules)
// ---------------------------------------------------------------------------

/// The exact identity of a model choice (`AI-PROVIDER-04`: provider, native id,
/// variant, and revision — "an ID alone is not sufficient").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelKey {
    /// The provider family id (`docs/12` §1: `anthropic`, `gateway`, …).
    pub provider_id: String,
    /// The provider's native model id, exactly as the provider spells it.
    pub model_id: String,
    /// A declared variant — `[1m]`, fast mode — that changes the request
    /// (`docs/12` §4: identity includes the variant).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
    /// A revision or deployment the profile pinned, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
}

/// Input/output modality channel (`docs/12` §8.3 `modalities`). The set is the
/// shipped one; anything else is unknown ⇒ unsupported (`AI-PROVIDER-05`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Modality {
    Text,
    Image,
    Pdf,
    Audio,
    Video,
    File,
}

/// What a model can do (`docs/12` §8.3). Absent metadata means **unsupported**
/// — the UI must not optimistically enable anything (`AI-PROVIDER-05`,
/// `docs/12` §2.3: "Do not assume a capability because a model family usually
/// has it").
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelCapabilities {
    /// Context window in tokens, when the provider/catalog reports one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u64>,
    /// Maximum output tokens, when reported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u64>,
    #[serde(default)]
    pub input_modalities: Vec<Modality>,
    #[serde(default)]
    pub output_modalities: Vec<Modality>,
    #[serde(default)]
    pub tools_supported: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub streamed_tool_arguments: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking_supported: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caching_supported: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fast_mode: Option<bool>,
    /// Where this knowledge came from (`docs/12` §8.3 `source`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<DescriptorSource>,
}

/// Where a descriptor's knowledge came from (`docs/12` §8.3); the merge rules
/// pin the precedence (`docs/12` §3.4: profile list, then discovery, then
/// catalog, then bundled fallback).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DescriptorSource {
    /// The provider answered a native model listing (`TEST-AI-018`).
    Provider,
    /// The signed published catalog (untrusted until verified — Phase 2 keeps
    /// it unverified and falls back, `docs/14` §9 item 20).
    Catalog,
    /// The profile's explicit `inferenceModels` list (`docs/12` §3.2).
    Static,
    /// A per-profile override.
    User,
}

/// One model as the UI and the agent layer see it (`docs/12` §8.3
/// `ModelDescriptor`, narrowed to the fields Phase 3 fills natively: the
/// catalog-only display surfaces stay with the catalog code).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDescriptor {
    pub key: ModelKey,
    /// The provider's display name when it reports one (`docs/12` §2.3: gateway
    /// rows carry `display_name`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    pub capabilities: ModelCapabilities,
    /// Anything the provider row carried that this rebuild has not modeled.
    /// Nothing is dropped silently and nothing is interpreted without evidence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unmodeled: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Turn request and its content model (`docs/12` §4, §5.2)
// ---------------------------------------------------------------------------

/// A message role (Anthropic Messages: `user` / `assistant`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    User,
    Assistant,
}

/// One content block, in the app's model. For the Anthropic-compatible
/// Gateway this maps 1:1 onto the native content-block types — that is the
/// point of a native adapter: nothing is flattened (`AI-MODALITY-01/02`,
/// `AI-TOOL-01`; the `AI-ADAPTER-06` regression).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    Text {
        text: String,
    },
    /// A base64 image; `media_type` is the MIME type (`docs/12` §2.3: media
    /// types and ordering are preserved; `AI-ADAPTER-06` regression for
    /// images inside tool results).
    Image {
        media_type: String,
        data: String,
    },
    /// A base64 document (PDF); the same preservation rules as images pass.
    Document {
        media_type: String,
        data: String,
        name: Option<String>,
    },
    /// A model-issued tool call (`docs/12` §8.2 `tool_call`).
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    /// A tool result — content may be text, images, or any provider-supported
    /// part (`docs/12` §5.2; the `AI-ADAPTER-06` regression); `is_error` is
    /// preserved so an execution failure stays visible to the model.
    ToolResult {
        tool_use_id: String,
        #[serde(default)]
        content: Vec<ContentPart>,
        #[serde(default)]
        is_error: bool,
        /// Prompt-caching control on the result block (`AI-ADAPTER-05`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cache_control: Option<String>,
    },
    /// A thinking block with its signature — both survive verbatim
    /// (`docs/12` §4.1; the `AI-ADAPTER-04` regression).
    Reasoning {
        thinking: String,
        signature: Option<String>,
    },
    /// A redacted-thinking block: opaque bytes the provider issued to protect
    /// a chain of thought. Dropped data cannot be replayed (`AI-ADAPTER-04`),
    /// so the raw payload is kept.
    RedactedThinking {
        data: serde_json::Value,
    },
}

/// A prompt-caching control block (`AI-ADAPTER-05`: "drops prompt caching" is
/// a recorded violation; the shipped contract carries
/// `cache_control` on system blocks and content).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemBlock {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<String>,
}

/// One tool in the app's model (`docs/12` §5.2). Names, descriptions, and the
/// input schema travel exactly; the schema is validated by the caller, and the
/// provider's own strict/schema modes are preserved (`AI-TOOL-01`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolSpec {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The JSON Schema of the arguments, verbatim.
    pub input_schema: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<String>,
}

/// Extended-thinking configuration (`docs/12` §8.3 `reasoning`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThinkingConfig {
    pub budget_tokens: u32,
}

/// The native turn an adapter is asked to produce (`docs/12` §8.2
/// `NativeTurnRequest`). Everything optional is optional — the adapter sends
/// only what the caller actually decided (`AI-PROVIDER-01`), and it never
/// invents tool/temperature/vision defaults (`AI-ADAPTER-10` regression).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeTurnRequest {
    /// Which model — the full identity, never a bare id (`AI-PROVIDER-04`).
    pub model: ModelKey,
    /// The system prompt as blocks, so prompt-caching controls survive
    /// (`AI-ADAPTER-05`).
    #[serde(default)]
    pub system: Vec<SystemBlock>,
    /// The conversation so far, with every part the app holds.
    #[serde(default)]
    pub messages: Vec<(Role, Vec<ContentPart>)>,
    /// Tools, exactly as declared — or absent (`AI-ADAPTER-10`).
    #[serde(default)]
    pub tools: Vec<ToolSpec>,
    /// Max output tokens. Anthropic Messages requires it; a missing value is
    /// a configuration error the adapter surfaces, not a guessed default.
    pub max_tokens: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stop_sequences: Vec<String>,
    /// Thinking mode, when the app chose it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking: Option<ThinkingConfig>,
    /// Anthropic beta identifiers to advertise for this turn (`AI-ADAPTER-07`:
    /// beta headers must be forwarded, or provider beta features silently
    /// disappear).
    #[serde(default)]
    pub betas: Vec<String>,
    /// Whether to stream. The agent layer streams; a probe may not.
    #[serde(default)]
    pub stream: bool,
}

/// Cancellation travels with the stream: dropping the returned [`TurnStream`]
/// must cancel the native request (`TEST-AI-015`).
pub type TurnStream = Pin<Box<dyn Stream<Item = Result<TurnEvent, ProviderError>> + Send>>;

// ---------------------------------------------------------------------------
// The typed turn stream (`docs/12` §8.2 `TurnEvent`)
// ---------------------------------------------------------------------------

/// A stream delta's kind. Instead of a marker trait the stream carries typed
/// events, per `docs/12` §4.1 ("adapter emits typed events").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentDelta {
    Text {
        text: String,
    },
    /// Input JSON delta for streamed tool arguments (`docs/12` §8.3
    /// `tools.streamedArguments`).
    InputJson {
        partial_json: String,
    },
    Thinking {
        thinking: String,
    },
    Signature {
        signature: String,
    },
    /// A citation annotation delta (`docs/12` §4.1 lists citations among what
    /// must survive the stream). App-model extension over §8.2's smallest set,
    /// recorded so nothing is dropped (`AI-STREAM-01`).
    Citation {
        citation: serde_json::Value,
    },
}

/// The reason the model stopped, preserved 1:1 (`docs/12` §4.1: "preserve
/// native stop reasons rather than collapsing them to done";
/// `AI-ADAPTER-08` regression: content-filter/refusal stays distinguishable).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeStopReason {
    EndTurn,
    MaxTokens,
    StopSequence,
    ToolUse,
    Refusal,
    /// A provider paused the turn (`pause_turn` — resumed by re-sending).
    PauseTurn,
    /// The provider reported a content-policy stop in some other spelling.
    ContentFilter,
    /// Any other stop reason, carried verbatim forever — unknowns are not
    /// collapsed into something they are not (`docs/01` fail-closed rule).
    Other(String),
}

impl NativeStopReason {
    /// The exact stop reason string to send back to the provider when replaying.
    pub fn wire(&self) -> String {
        match self {
            NativeStopReason::EndTurn => "end_turn".to_string(),
            NativeStopReason::MaxTokens => "max_tokens".to_string(),
            NativeStopReason::StopSequence => "stop_sequence".to_string(),
            NativeStopReason::ToolUse => "tool_use".to_string(),
            NativeStopReason::Refusal => "refusal".to_string(),
            NativeStopReason::PauseTurn => "pause_turn".to_string(),
            NativeStopReason::ContentFilter => "content_filter".to_string(),
            NativeStopReason::Other(raw) => raw.clone(),
        }
    }

    /// Parse a provider stop-reason string. Unknown spellings stay verbatim.
    pub fn parse(raw: &str) -> NativeStopReason {
        match raw {
            "end_turn" => NativeStopReason::EndTurn,
            "max_tokens" => NativeStopReason::MaxTokens,
            "stop_sequence" => NativeStopReason::StopSequence,
            "tool_use" => NativeStopReason::ToolUse,
            "refusal" => NativeStopReason::Refusal,
            "pause_turn" => NativeStopReason::PauseTurn,
            other => {
                if other.contains("filter") || other.contains("policy") {
                    NativeStopReason::ContentFilter
                } else {
                    NativeStopReason::Other(other.to_string())
                }
            }
        }
    }
}

/// Usage, exactly as the provider reported it (`AI-USAGE-01`: input, output,
/// cache-read, cache-write, web-search counts reported separately; never
/// estimated when the provider reports exactly — `AI-ADAPTER-09` regression
/// lives at [`ProviderAdapter::count_tokens`]).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    #[serde(default)]
    pub input: u64,
    #[serde(default)]
    pub output: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_write: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web_search: Option<u64>,
}

/// An error the provider (or the adapter boundary itself) produced. The class
/// is the `TEST-AI-021` vocabulary — auth, quota, region, model, context,
/// modality, transient, … stay distinguishable — and the message is the
/// provider's user-safe text: never a credential, never a raw body dump.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderError {
    pub class: ErrorClass,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
    /// The user-safe message: status text plus the provider's own error
    /// wording when the provider sent an error envelope.
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorClass {
    /// Missing/invalid credential (`AI-ADAPTER-12` boundary: never substituted).
    Auth,
    /// Rate limit / quota (`TEST-AI-021`).
    Quota,
    /// Wrong region / unsupported region (cloud families).
    Region,
    /// The model is absent from this provider (`AI-MODEL-01`: block, no
    /// substitution).
    ModelNotFound,
    /// The context is too long for the model window (`TEST-AI-021`).
    ContextLength,
    /// The request carries a part the provider cannot accept
    /// (`AI-MM-01`: visible failure, never a silent drop).
    UnsupportedModality,
    /// Local validation rejected the request before send (`AI-MODEL-02`).
    InvalidRequest,
    /// The provider is quite possibly fine later — overloaded, API error.
    Transient,
    /// Could not reach the provider at all.
    Unreachable,
    /// The local configuration is incomplete or invalid (`AI-ADAPTER-12`
    /// class-free variant: nothing was sent).
    Configuration,
    /// The client itself cancelled or dropped the stream (`TEST-AI-015`).
    Cancelled,
}

/// One event out of a turn's stream (`docs/12` §8.2 `TurnEvent`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TurnEvent {
    TextDelta {
        text: String,
    },
    /// Reasoning delta with the signature when the provider signs the block
    /// (`AI-ADAPTER-04`; signatures replay `docs/12` §4.1).
    ReasoningDelta {
        text: String,
        signature: Option<String>,
    },
    /// An opaque redacted-thinking block, kept byte-exact (`AI-ADAPTER-04`).
    ReasoningRedacted {
        data: serde_json::Value,
    },
    /// A citation annotation, kept (`docs/12` §4.1).
    Citation {
        citation: serde_json::Value,
    },
    /// A tool call: the id and name, streamed argument fragments while they
    /// arrive, and the parsed arguments at block end (`AI-TOOL-01`;
    /// `AI-ADAPTER-11` regression: argument data is not re-routed or
    /// rewritten).
    ToolCall {
        id: String,
        name: String,
        arguments_delta: Option<String>,
        arguments: Option<serde_json::Value>,
    },
    /// Usage as reported (`AI-USAGE-01`); emitted once, from the final events.
    Usage {
        usage: Usage,
    },
    /// A preserved native stop reason (`AI-ADAPTER-08`).
    Stop {
        reason: NativeStopReason,
    },
    Error {
        error: ProviderError,
    },
    /// The stream ended following a stop or an unrecoverable error.
    Done,
}

// ---------------------------------------------------------------------------
// Token counting and health
// ---------------------------------------------------------------------------

/// A token-count request: the same native shape as a turn, without output
/// budget decisions (`docs/12` §8.2 `CountRequest`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CountRequest {
    pub model: ModelKey,
    #[serde(default)]
    pub system: Vec<SystemBlock>,
    #[serde(default)]
    pub messages: Vec<(Role, Vec<ContentPart>)>,
    #[serde(default)]
    pub tools: Vec<ToolSpec>,
}

/// A token count, with its origin. `native: false` would mean an *estimate* —
/// this crate never produces one (`AI-ADAPTER-09`); an adapter that cannot
/// count returns an error instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenCount {
    pub input_tokens: u64,
    /// True only when the provider itself counted the tokens.
    pub native: bool,
}

/// Reachability of the provider (`docs/12` §8.2 `healthCheck`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderHealth {
    pub reachable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authenticated: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

// ---------------------------------------------------------------------------
// Auth and the adapter itself (`docs/12` §8.1–§8.2)
// ---------------------------------------------------------------------------

/// The credential material an adapter call uses, resolved fresh by the caller
/// on every request (`docs/13` Phase 3 "auth + credential rotation";
/// rotation is honored because the caller re-resolves the slot each time —
/// the adapter holds no cached credential of its own, so there is no second
/// source to fall back to and nothing to smuggle outside the keychain,
/// `AI-ADAPTER-12`, `AI-PROVIDER-03`).
#[derive(Clone)]
pub struct AuthContext {
    credential: SecretString,
    /// A label for display only, never the value (`docs/09` §4).
    label: String,
}

impl AuthContext {
    /// Wrap resolved credential material. The value is memory-only
    /// (`SecretString`, zeroized on drop).
    pub fn new(credential: SecretString, label: impl Into<String>) -> Self {
        AuthContext {
            credential,
            label: label.into(),
        }
    }

    /// The memory-only credential value. Adapters put it on the request and
    /// never persist, log, or copy it (`AI-PROVIDER-03`).
    pub fn credential(&self) -> &SecretString {
        &self.credential
    }

    /// A display label like "keychain static key" — never the value.
    pub fn label(&self) -> &str {
        &self.label
    }
}

// Keep Debug useful without leaking the credential (`docs/09` §4):
impl std::fmt::Debug for AuthContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthContext")
            .field("label", &self.label)
            .finish()
    }
}

/// The provider adapter contract (`docs/12` §8.2 `ProviderAdapter`).
///
/// Implementors talk their provider's own protocol and nothing else; this
/// trait is the app-facing seam (`docs/10` §9: `providers` — native provider
/// adapters, discovery, capabilities). Every method fails closed and never
/// substitutes (`AI-MODEL-01`).
#[async_trait]
pub trait ProviderAdapter: Send + Sync {
    /// The stable provider id (`docs/12` §8.2: stable provider instance).
    fn id(&self) -> &str;

    /// The native protocol family (`AI-EXT-02`).
    fn protocol(&self) -> ProtocolFamily;

    /// The UI-facing provider name.
    fn display_name(&self) -> &str;

    /// Resolve and validate the credential shape before a send. This is a
    /// shape/liveness check, not a network round-trip; the caller resolved the
    /// material (`AI-PROVIDER-03`).
    async fn authenticate(&self, auth: &AuthContext) -> Result<(), ProviderError>;

    /// The provider's model list, through its native listing when one exists
    /// (discovery is a provider concern: `docs/10` §9; `TEST-AI-018`).
    async fn list_models(&self, auth: &AuthContext) -> Result<Vec<ModelDescriptor>, ProviderError>;

    /// Capability metadata for one model; absent capability is unsupported
    /// (`AI-PROVIDER-05`).
    fn capabilities(&self, model: &ModelKey, descriptors: &[ModelDescriptor]) -> ModelCapabilities;

    /// Produce one raw turn. Dropping the returned stream cancels the native
    /// request; partial output already emitted stays visible and is marked
    /// stopped by the caller (`TEST-AI-015`; `AI-STREAM-02`).
    ///
    /// `authenticate` + the model policy gate beforehand: a missing credential
    /// or an invalid model must block before contacting the provider
    /// (`TEST-AI-006/007/008`; `AI-MODEL-01`).
    async fn create_turn(
        &self,
        auth: &AuthContext,
        request: &NativeTurnRequest,
    ) -> Result<TurnStream, ProviderError>;

    /// Count tokens with the provider's own counter. If the provider offers
    /// none, the adapter errors — never length/4 (`AI-ADAPTER-09`).
    async fn count_tokens(
        &self,
        auth: &AuthContext,
        request: &CountRequest,
    ) -> Result<TokenCount, ProviderError>;

    /// A reachability check (`docs/12` §8.2 `healthCheck`). No inference
    /// happens here; the setup window's *Test connection* probe builds one
    /// from [`ProviderAdapter::create_turn`] with a minimal 1-token request
    /// (`docs/screens/setup-desktop-3p.md`: real inference, no persistence).
    async fn health_check(&self, auth: &AuthContext) -> Result<ProviderHealth, ProviderError>;
}
