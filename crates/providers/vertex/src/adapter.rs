//! The [`VertexAdapter`]: the one place in the app that talks the Vertex
//! family's native protocol — the Vertex AI raw-prediction surface
//! (`docs/12` §1 row `vertex`; `docs/13` Phase 3 order item 4). It
//! implements the [`myharness_provider_sdk::ProviderAdapter`] contract and
//! adds the one-shot inference probe the setup window's *Test connection*
//! runs.
//!
//! Boundaries this crate holds (`docs/12` §11): credentials arrive resolved
//! in an [`AuthContext`], re-resolved by the caller per request
//! (`AI-PROVIDER-03`); the family's `claude-` model policy gates every send
//! and nothing is substituted (`TEST-AI-003`, `AI-MODEL-01`); nothing here
//! is process-global (`AI-ADAPTER-13`); the credential is never logged,
//! never persisted, and never copied into any other store.
//!
//! Family shape (`docs/09` §2.2 "Vertex AI"): model discovery is **disabled
//! for every credential kind this family offers**, so this adapter refuses
//! listing calls outright instead of guessing a catalog route; the token
//! count surface does not exist, so counting refuses; and the health check
//! is the same adapter-level convenience reading the other adapters use
//! (`docs/14` §9 item 26): reachability over the configured base host.

use std::collections::{BTreeMap, VecDeque};
use std::convert::TryFrom;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use futures::Stream;
use myharness_provider_sdk::{
    AuthContext, ErrorClass, ModelCapabilities, ModelDescriptor, ModelKey, NativeStopReason,
    ProtocolFamily, ProviderAdapter, ProviderError, ProviderHealth, TokenCount, TurnEvent,
    TurnStream, Usage,
};
use secrecy::ExposeSecret;

use crate::messages::{
    error_from_envelope, error_from_status, parse_usage, raw_predict_body, raw_predict_stream_url,
    raw_predict_url, stream_event_from_value, ProbeStatus, StreamEvent, VertexClientConfig,
};
use crate::sse::{DataOnlyParser, SseItem};

/// The Vertex family adapter. Constructed per profile by the caller (the
/// bridge); [`reqwest::Client`] is injected so connection pooling is the
/// caller's decision and nothing here is process-global (`AI-ADAPTER-13`).
pub struct VertexAdapter {
    config: VertexClientConfig,
    client: reqwest::Client,
}

/// A useful debug answer with no credential material — the config carries no
/// credential, and the client's face is its own (`docs/09` §4).
impl std::fmt::Debug for VertexAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VertexAdapter")
            .field("config", &self.config)
            .finish()
    }
}

impl VertexAdapter {
    /// Construct the adapter. Trailing slashes are normalized
    /// (`docs/12` §3.1).
    pub fn new(config: VertexClientConfig, client: reqwest::Client) -> Self {
        let mut config = config;
        config.base_url = config.base_url.trim().trim_end_matches('/').to_string();
        config.project_id = config.project_id.trim().to_string();
        config.region = config.region.trim().to_string();
        VertexAdapter { config, client }
    }

    /// The normalized base URL.
    pub fn base_url(&self) -> &str {
        &self.config.base_url
    }

    /// The shared request header set: the family's Google bearer credential
    /// and forwarded custom headers. The credential exists only as
    /// request-header material — it is never logged, never persisted, and
    /// never copied into any other store (`AI-PROVIDER-03`).
    fn request_builder(
        &self,
        method: reqwest::Method,
        url: &str,
        auth: &AuthContext,
    ) -> reqwest::RequestBuilder {
        let mut builder = self
            .client
            .request(method, url)
            // Google auth: the resolved access token travels as a bearer
            // (`docs/12` §8.5 "Google auth"; `docs/14` §9 item 28).
            .header(
                "Authorization",
                format!("Bearer {}", auth.credential().expose_secret()),
            );
        for (name, value) in &self.config.custom_headers {
            builder = builder.header(name, value);
        }
        builder
    }

    /// Read a response's JSON, or turn a failure into the mapped provider
    /// error. A non-success status is parsed for the error envelope (its
    /// message only — no raw bodies, `docs/18` §5.4).
    async fn response_json(response: reqwest::Response) -> Result<serde_json::Value, ProviderError> {
        let status = response.status();
        if !status.is_success() {
            let status = status.as_u16();
            let body = response.json::<serde_json::Value>().await.ok();
            return Err(match body {
                Some(body) => error_from_envelope(Some(status), &body),
                None => error_from_status(status),
            });
        }
        response
            .json::<serde_json::Value>()
            .await
            .map_err(|error| ProviderError {
                class: ErrorClass::InvalidRequest,
                status: Some(status.as_u16()),
                message: format!("the provider answer could not be read: {error}"),
            })
    }

    /// Open a request; unreachable transports map to `Unreachable`.
    async fn send(
        &self,
        builder: reqwest::RequestBuilder,
    ) -> Result<reqwest::Response, ProviderError> {
        builder.send().await.map_err(|error| ProviderError {
            class: ErrorClass::Unreachable,
            status: None,
            message: format!("the provider could not be reached: {error}"),
        })
    }

    /// A non-success response, mapped: envelope first, bare status otherwise.
    async fn response_error(response: reqwest::Response) -> ProviderError {
        let status = response.status().as_u16();
        let body = response.json::<serde_json::Value>().await.ok();
        match body {
            Some(body) => error_from_envelope(Some(status), &body),
            None => error_from_status(status),
        }
    }

    /// Send non-streaming `rawPredict`.
    async fn send_raw_predict(
        &self,
        auth: &AuthContext,
        request: &myharness_provider_sdk::NativeTurnRequest,
    ) -> Result<serde_json::Value, ProviderError> {
        let builder = self
            .request_builder(
                reqwest::Method::POST,
                &raw_predict_url(&self.config, &request.model.model_id),
                auth,
            )
            .json(&raw_predict_body(request)?);
        let response = self.send(builder).await?;
        Self::response_json(response).await
    }

    /// The policy gate every send passes first (`TEST-AI-003`): the family's
    /// recorded model shape — `claude-` prefix, none of the listed
    /// characters (`docs/12` §2.2; `docs/09` §2.2).
    fn gate_model(&self, model: &ModelKey) -> Result<(), ProviderError> {
        match myharness_config::forms::validate_vertex_model_id(&model.model_id) {
            Ok(()) => Ok(()),
            Err(reason) => Err(ProviderError {
                class: ErrorClass::InvalidRequest,
                status: None,
                message: format!(
                    "the requested model {:?} does not pass the Vertex model policy and was not sent: {reason}",
                    model.model_id
                ),
            }),
        }
    }

    /// The beta-list gate: no beta transport is recorded for this family, so
    /// a beta-carrying request is refused visibly rather than silently
    /// dropped (the same fail-closed reading as the Bedrock adapter;
    /// `docs/14` §9 item 28).
    fn gate_betas(&self, betas: &[String]) -> Result<(), ProviderError> {
        if betas.is_empty() {
            return Ok(());
        }
        Err(ProviderError {
            class: ErrorClass::InvalidRequest,
            status: None,
            message: format!(
                "the Vertex surface has no recorded beta transport ({:?} requested); the turn was refused rather than the identifiers dropped (`docs/14` §9 item 28)",
                betas.join(", ")
            ),
        })
    }

    /// Translate a complete (non-streaming) raw-prediction answer into the
    /// typed event set (`docs/12` §8.2). The model echo is **conditional**:
    /// an answer that names a different model ends the turn visibly, but an
    /// answer without a model field is not an echo failure (`AI-MODEL-01`;
    /// `docs/14` §9 item 28).
    pub(crate) fn events_from_response(
        response: &serde_json::Value,
        requested_model: &ModelKey,
    ) -> Result<Vec<TurnEvent>, ProviderError> {
        if let Some(ran) = response.get("model").and_then(serde_json::Value::as_str) {
            if ran != requested_model.model_id {
                return Err(ProviderError {
                    class: ErrorClass::ModelNotFound,
                    status: None,
                    message: format!(
                        "the provider performed model {ran:?} while the request named {:?}; the turn was refused",
                        requested_model.model_id
                    ),
                });
            }
        }
        let mut events = Vec::new();
        for block in response
            .get("content")
            .and_then(serde_json::Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default()
        {
            let kind = block
                .get("type")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            match kind {
                "text" => events.push(TurnEvent::TextDelta {
                    text: block
                        .get("text")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                }),
                "tool_use" => events.push(TurnEvent::ToolCall {
                    id: block
                        .get("id")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    name: block
                        .get("name")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    arguments_delta: None,
                    arguments: Some(block.get("input").cloned().unwrap_or_default()),
                }),
                "thinking" => events.push(TurnEvent::ReasoningDelta {
                    text: block
                        .get("thinking")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    signature: block
                        .get("signature")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string),
                }),
                "redacted_thinking" => {
                    events.push(TurnEvent::ReasoningRedacted { data: block.clone() })
                }
                other => {
                    return Err(ProviderError {
                        class: ErrorClass::UnsupportedModality,
                        status: None,
                        message: format!(
                            "the response carried a content block of an unhandled kind: {other:?}"
                        ),
                    });
                }
            }
        }
        let usage = response
            .get("usage")
            .and_then(parse_usage)
            .unwrap_or_default();
        if usage != Usage::default() {
            events.push(TurnEvent::Usage { usage });
        }
        match response
            .get("stop_reason")
            .and_then(serde_json::Value::as_str)
            .map(NativeStopReason::parse)
        {
            Some(reason) => events.push(TurnEvent::Stop { reason }),
            None => {
                return Err(ProviderError {
                    class: ErrorClass::InvalidRequest,
                    status: None,
                    message: "the response carried no stop reason".to_string(),
                });
            }
        }
        events.push(TurnEvent::Done);
        Ok(events)
    }

    /// The one-shot inference probe (the setup window's *Test connection*).
    /// Sends a 1-token turn to the requested model; reports exact usage and
    /// the native stop reason (`TEST-AI-013/014`), returns provider error
    /// classes verbatim (`TEST-AI-021`), and refuses a model swap. Latency
    /// is measured around the native request.
    pub async fn probe_inference(
        &self,
        auth: &AuthContext,
        model_id: &str,
    ) -> Result<ProbeStatus, ProviderError> {
        let model = ModelKey {
            provider_id: self.id().to_string(),
            model_id: model_id.to_string(),
            variant: None,
            revision: None,
        };
        self.gate_model(&model)?;
        self.authenticate(auth).await?;
        let request = myharness_provider_sdk::NativeTurnRequest {
            model,
            system: Vec::new(),
            messages: vec![(
                myharness_provider_sdk::Role::User,
                vec![myharness_provider_sdk::ContentPart::Text {
                    text: "ping".to_string(),
                }],
            )],
            tools: Vec::new(),
            max_tokens: 1,
            temperature: None,
            top_p: None,
            top_k: None,
            stop_sequences: Vec::new(),
            thinking: None,
            betas: Vec::new(),
            stream: false,
        };
        let started = Instant::now();
        let response = self.send_raw_predict(auth, &request).await?;
        let latency = started.elapsed();
        // The model echo gate applies to probes too — when the answer names
        // a model (`AI-MODEL-01`; the conditional reading above).
        if let Some(ran) = response
            .get("model")
            .and_then(serde_json::Value::as_str)
            .filter(|ran| *ran != model_id)
        {
            return Err(ProviderError {
                class: ErrorClass::ModelNotFound,
                status: None,
                message: format!(
                    "the provider performed model {ran:?} while the probe named {model_id:?}"
                ),
            });
        }
        let ran = response
            .get("model")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(model_id);
        let stop_reason = response
            .get("stop_reason")
            .and_then(serde_json::Value::as_str)
            .map(NativeStopReason::parse)
            .map(|reason| reason.wire());
        let usage = response
            .get("usage")
            .and_then(parse_usage)
            .unwrap_or_default();
        Ok(ProbeStatus {
            ok: true,
            model: ran.to_string(),
            stop_reason,
            output_tokens: usage.output,
            input_tokens: usage.input,
            latency_ms: u64::try_from(latency.as_millis()).unwrap_or(u64::MAX),
        })
    }
}

/// Turn a successful streaming response into the typed [`TurnStream`]: the
/// bytes stream feeds the data-only SSE reader, the state machine guards the
/// model echo and the idle window, and dropping the returned stream cancels
/// the native request (`TEST-AI-015`).
fn finish_stream_into_turn_stream(
    response: reqwest::Response,
    requested_model: String,
    idle_timeout: Duration,
) -> TurnStream {
    // A zero-sized window means "no idle timeout" — the profile documents a
    // nonzero default; this is the one deliberate off-switch.
    let idle_armed = !idle_timeout.is_zero();
    Box::pin(TurnStreamImpl {
        inner: Box::pin(response.bytes_stream()),
        parser: DataOnlyParser::new(),
        requested_model,
        idle_timeout,
        idle: Box::pin(tokio::time::sleep(idle_timeout)),
        idle_armed,
        tools: BTreeMap::new(),
        usage: Usage::default(),
        stop: None,
        outbox: VecDeque::new(),
        finished: false,
        ended: false,
    })
}

#[async_trait::async_trait]
impl ProviderAdapter for VertexAdapter {
    fn id(&self) -> &str {
        "vertex"
    }

    /// The family's protocol: Anthropic Messages over the cloud's Vertex AI
    /// raw-prediction transport (`docs/12` §1; `AI-EXT-02`).
    fn protocol(&self) -> ProtocolFamily {
        ProtocolFamily::AnthropicMessagesViaCloud("vertex")
    }

    fn display_name(&self) -> &str {
        "Vertex AI"
    }

    /// A shape check with no network: the family's credential is a Google
    /// access token (`docs/12` §8.5); a non-empty one is shaped. The
    /// interactive/workforce/vendor identity kinds never reach the adapter
    /// with material — the bridge refuses them fail-closed (`docs/14` §9
    /// item 28).
    async fn authenticate(&self, auth: &AuthContext) -> Result<(), ProviderError> {
        if auth.credential().expose_secret().trim().is_empty() {
            return Err(ProviderError {
                class: ErrorClass::Auth,
                status: None,
                message: "the credential is empty; nothing was sent".to_string(),
            });
        }
        Ok(())
    }

    /// Model discovery is **unavailable** on this family: the app disables
    /// it for every credential kind the family offers (`docs/09` §2.2
    /// "Vertex AI"), so the adapter refuses listing calls outright instead
    /// of guessing a catalog route. A static list of model ids is how this
    /// family's models travel in the rebuild.
    async fn list_models(
        &self,
        _auth: &AuthContext,
    ) -> Result<Vec<ModelDescriptor>, ProviderError> {
        Err(ProviderError {
            class: ErrorClass::UnsupportedModality,
            status: None,
            message: "model discovery is unavailable for this family's credential kinds (docs/09 §2.2 \"Vertex AI\"); model ids travel through the profile's static list".to_string(),
        })
    }

    /// Capability lookup over the resolved descriptor list; absent metadata
    /// stays unsupported (`AI-PROVIDER-05`).
    fn capabilities(&self, model: &ModelKey, descriptors: &[ModelDescriptor]) -> ModelCapabilities {
        descriptors
            .iter()
            .find(|descriptor| descriptor.key.model_id == model.model_id)
            .map(|descriptor| descriptor.capabilities.clone())
            .unwrap_or_default()
    }

    /// Build the native request and open the native stream (`docs/12` §4.1).
    /// A non-streaming request becomes a buffered event list from the same
    /// contract.
    async fn create_turn(
        &self,
        auth: &AuthContext,
        request: &myharness_provider_sdk::NativeTurnRequest,
    ) -> Result<TurnStream, ProviderError> {
        self.gate_model(&request.model)?;
        self.gate_betas(&request.betas)?;
        self.authenticate(auth).await?;
        if request.stream {
            let builder = self
                .request_builder(
                    reqwest::Method::POST,
                    &raw_predict_stream_url(&self.config, &request.model.model_id),
                    auth,
                )
                .json(&raw_predict_body(request)?);
            let response = self.send(builder).await?;
            if !response.status().is_success() {
                return Err(Self::response_error(response).await);
            }
            Ok(finish_stream_into_turn_stream(
                response,
                request.model.model_id.clone(),
                self.config.idle_timeout,
            ))
        } else {
            let response = self.send_raw_predict(auth, request).await?;
            let events = Self::events_from_response(&response, &request.model)?;
            Ok(Box::pin(futures::stream::iter(events.into_iter().map(Ok))))
        }
    }

    /// The surface exposes no native token-count endpoint; the answer is an
    /// error — never length/4, never an estimate (`AI-ADAPTER-09`; `docs/14`
    /// §9 item 28). The request is refused before any bytes are sent.
    async fn count_tokens(
        &self,
        auth: &AuthContext,
        request: &myharness_provider_sdk::CountRequest,
    ) -> Result<TokenCount, ProviderError> {
        self.gate_model(&request.model)?;
        self.authenticate(auth).await?;
        Err(ProviderError {
            class: ErrorClass::InvalidRequest,
            status: None,
            message: format!(
                "the Vertex surface offers no native token-count endpoint for {:?}; no count was produced and none was estimated",
                request.model.model_id
            ),
        })
    }

    /// The family's health check is the same adapter-level convenience
    /// reading as the other adapters (`docs/14` §9 item 26): reachability
    /// over the configured base host, with an authentication verdict when
    /// the host answers 401/403. No inference happens, and no listing route
    /// is invented (discovery is unavailable for this family).
    async fn health_check(&self, auth: &AuthContext) -> Result<ProviderHealth, ProviderError> {
        let builder = self.request_builder(reqwest::Method::GET, &self.config.base_url, auth);
        let Ok(response) = self.send(builder).await else {
            return Ok(ProviderHealth {
                reachable: false,
                authenticated: None,
                detail: None,
            });
        };
        let status = response.status().as_u16();
        if (401..=403).contains(&status) {
            Ok(ProviderHealth {
                reachable: true,
                authenticated: Some(false),
                detail: Some(format!("status {status}")),
            })
        } else if (200..299).contains(&status) {
            Ok(ProviderHealth {
                reachable: true,
                authenticated: Some(true),
                detail: None,
            })
        } else {
            Ok(ProviderHealth {
                reachable: true,
                authenticated: None,
                detail: Some(format!("status {status}")),
            })
        }
    }
}

// ---------------------------------------------------------------------------
// The streaming state machine
// ---------------------------------------------------------------------------

/// One in-flight tool call whose arguments stream in as
/// `input_json_delta` fragments (`AI-TOOL-01`: ids, names, and argument
/// deltas preserved; assembled arguments validated at close).
#[derive(Debug, Clone)]
struct ToolBlockState {
    id: String,
    name: String,
    buffer: String,
}

/// The `TurnStream` implementation for streamed turns: parses the provider's
/// data-only SSE into the app's typed events, guards the model echo
/// (conditionally — see `events_from_response`), preserves usage and stop
/// reasons, and enforces the idle window (`TEST-AI-014/015/016`).
///
/// Dropping the stream cancels the native request: the `reqwest` response
/// (and its body task) drops with it, which closes the connection — the
/// caller keeps whatever was already emitted and marks it stopped
/// (`docs/12` §4.1).
pub(crate) struct TurnStreamImpl {
    inner: Pin<Box<dyn Stream<Item = reqwest::Result<bytes::Bytes>> + Send>>,
    parser: DataOnlyParser,
    requested_model: String,
    idle_timeout: Duration,
    /// The idle-window timer (re-armed on content progress; keep-alives are
    /// not progress — the reading in this crate's module docs).
    idle: Pin<Box<tokio::time::Sleep>>,
    /// Whether the idle window governs this stream at all (a `0` window
    /// means "no idle timeout"; the profile documents a nonzero default).
    idle_armed: bool,
    tools: BTreeMap<usize, ToolBlockState>,
    usage: Usage,
    stop: Option<NativeStopReason>,
    outbox: VecDeque<Result<TurnEvent, ProviderError>>,
    finished: bool,
    ended: bool,
}

impl TurnStreamImpl {
    fn rearm_idle(&mut self) {
        if self.idle_armed {
            self.idle = Box::pin(tokio::time::sleep(self.idle_timeout));
        }
    }

    /// Absorb one byte chunk; append zero or more typed events to the
    /// outbox, or terminate the stream on a malformed/error item.
    fn consume(&mut self, chunk: &[u8]) -> Result<(), ProviderError> {
        for parsed in self.parser.feed(chunk) {
            match parsed {
                Err(error) => {
                    // Malformed bytes/lines surface — nothing is silently
                    // skipped (`AI-MM-01`; `docs/12` §4.1).
                    self.finished = true;
                    return Err(error);
                }
                Ok(SseItem::Keepalive) => {} // transport, never progress
                Ok(SseItem::Sentinel) => {
                    // The transport terminator (`data: [DONE]`): when the
                    // message has already named a stop reason, the turn may
                    // complete through the sentinel instead of a
                    // `message_stop` event; otherwise it is contract-
                    // incomplete and the stream must say so.
                    if !self.ended {
                        self.end_turn()?;
                    }
                }
                Ok(SseItem::Data(value)) => match stream_event_from_value(&value) {
                    None => {
                        self.finished = true;
                        return Err(ProviderError {
                            class: ErrorClass::InvalidRequest,
                            status: None,
                            message: "the stream carried an event that produced nothing"
                                .to_string(),
                        });
                    }
                    Some(Err(error)) => {
                        self.finished = true;
                        return Err(error);
                    }
                    Some(Ok(event)) => self.step(event)?,
                },
            }
        }
        Ok(())
    }

    /// One parsed stream event. Progress events re-arm the idle window;
    /// keep-alives never do (`TEST-AI-016`).
    fn step(&mut self, event: StreamEvent) -> Result<(), ProviderError> {
        match event {
            StreamEvent::Ping => Ok(()),
            StreamEvent::MessageStart { model, usage } => {
                // Conditional echo, as elsewhere (`docs/14` §9 item 28).
                if let Some(ran) = model {
                    if ran != self.requested_model {
                        self.finished = true;
                        return Err(ProviderError {
                            class: ErrorClass::ModelNotFound,
                            status: None,
                            message: format!(
                                "the provider began model {ran:?} while the request named {:?}; the stream was stopped",
                                self.requested_model
                            ),
                        });
                    }
                }
                if let Some(usage) = usage {
                    self.usage.input = usage.input;
                    if usage.cache_read.is_some() {
                        self.usage.cache_read = usage.cache_read;
                    }
                    if usage.cache_write.is_some() {
                        self.usage.cache_write = usage.cache_write;
                    }
                    // Every reported count survives the stream (`AI-USAGE-01`).
                    if usage.web_search.is_some() {
                        self.usage.web_search = usage.web_search;
                    }
                }
                Ok(())
            }
            StreamEvent::BlockStart { index, block } => {
                self.rearm_idle();
                let kind = block
                    .get("type")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("");
                match kind {
                    "text" | "thinking" => Ok(()), // the deltas carry the content
                    "tool_use" => {
                        let id = block
                            .get("id")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("")
                            .to_string();
                        let name = block
                            .get("name")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("")
                            .to_string();
                        self.tools.insert(
                            index,
                            ToolBlockState {
                                id: id.clone(),
                                name: name.clone(),
                                buffer: String::new(),
                            },
                        );
                        self.outbox.push_back(Ok(TurnEvent::ToolCall {
                            id,
                            name,
                            arguments_delta: None,
                            arguments: None,
                        }));
                        Ok(())
                    }
                    "redacted_thinking" => {
                        // The opaque block is preserved whole — dropped bytes
                        // cannot be replayed (`AI-ADAPTER-04`).
                        self.outbox
                            .push_back(Ok(TurnEvent::ReasoningRedacted { data: block }));
                        Ok(())
                    }
                    other => {
                        self.finished = true;
                        Err(ProviderError {
                            class: ErrorClass::UnsupportedModality,
                            status: None,
                            message: format!(
                                "the stream carried an assistant block of an unhandled kind: {other:?}"
                            ),
                        })
                    }
                }
            }
            StreamEvent::Delta { index, delta } => {
                self.rearm_idle();
                match delta {
                    myharness_provider_sdk::ContentDelta::Text { text } => {
                        self.outbox.push_back(Ok(TurnEvent::TextDelta { text }));
                        Ok(())
                    }
                    myharness_provider_sdk::ContentDelta::InputJson { partial_json } => {
                        if let Some(state) = self.tools.get_mut(&index) {
                            state.buffer.push_str(&partial_json);
                            self.outbox.push_back(Ok(TurnEvent::ToolCall {
                                id: state.id.clone(),
                                name: state.name.clone(),
                                arguments_delta: Some(partial_json),
                                arguments: None,
                            }));
                        }
                        Ok(())
                    }
                    myharness_provider_sdk::ContentDelta::Thinking { thinking } => {
                        self.outbox.push_back(Ok(TurnEvent::ReasoningDelta {
                            text: thinking,
                            signature: None,
                        }));
                        Ok(())
                    }
                    myharness_provider_sdk::ContentDelta::Signature { signature } => {
                        self.outbox.push_back(Ok(TurnEvent::ReasoningDelta {
                            text: String::new(),
                            signature: Some(signature),
                        }));
                        Ok(())
                    }
                    myharness_provider_sdk::ContentDelta::Citation { citation } => {
                        self.outbox.push_back(Ok(TurnEvent::Citation { citation }));
                        Ok(())
                    }
                }
            }
            StreamEvent::BlockStop { index } => {
                if let Some(state) = self.tools.remove(&index) {
                    // Assembled arguments: a valid JSON payload (including an
                    // empty one) completes the call; anything else surfaces
                    // and nothing is executed (`docs/12` §5.2).
                    let arguments = if state.buffer.trim().is_empty() {
                        serde_json::Value::Object(serde_json::Map::new())
                    } else {
                        serde_json::from_str(&state.buffer).map_err(|error| {
                            self.finished = true;
                            ProviderError {
                                class: ErrorClass::InvalidRequest,
                                status: None,
                                message: format!(
                            "the tool arguments for {:?} were not valid JSON; nothing was executed: {error}",
                            state.name
                        ),
                            }
                        })?
                    };
                    self.outbox.push_back(Ok(TurnEvent::ToolCall {
                        id: state.id,
                        name: state.name,
                        arguments_delta: None,
                        arguments: Some(arguments),
                    }));
                }
                Ok(())
            }
            StreamEvent::MessageDelta {
                stop_reason, usage, ..
            } => {
                self.rearm_idle();
                if let Some(reason) = stop_reason {
                    self.stop = Some(reason);
                }
                // The final usage travels on message_delta: exact numbers,
                // never estimated (`AI-USAGE-01`; `TEST-AI-013`).
                if let Some(usage) = usage {
                    self.usage.output = usage.output;
                    if usage.cache_read.is_some() {
                        self.usage.cache_read = usage.cache_read;
                    }
                    if usage.cache_write.is_some() {
                        self.usage.cache_write = usage.cache_write;
                    }
                    // Every reported count survives the stream
                    // (`AI-USAGE-01`).
                    if usage.web_search.is_some() {
                        self.usage.web_search = usage.web_search;
                    }
                }
                Ok(())
            }
            StreamEvent::Error { kind, message } => {
                // The provider stopped the stream with its own error event:
                // preserve the class and the provider wording (`TEST-AI-021`
                // for stream errors; `docs/12` §4.1).
                self.finished = true;
                Err(ProviderError {
                    class: match kind.as_deref() {
                        Some(kind) if kind.contains("quota") || kind.contains("exhausted") => {
                            ErrorClass::Quota
                        }
                        Some(_) => ErrorClass::InvalidRequest,
                        None => ErrorClass::InvalidRequest,
                    },
                    status: None,
                    message: message
                        .unwrap_or_else(|| "the stream ended with an error event".to_string()),
                })
            }
            StreamEvent::MessageStop => {
                self.end_turn()?;
                Ok(())
            }
        }
    }

    /// Close the turn: the final usage is the exact number the provider
    /// reported (`TEST-AI-013`; `docs/12` §4.2), then the preserved stop
    /// reason (`TEST-AI-014`), then the typed end-of-stream. A stream that
    /// ended without a stop reason is contract-malformed; no "end_turn" is
    /// invented.
    ///
    /// Both terminal shapes call this: a `message_stop` event, and the
    /// `data: [DONE]` transport sentinel when the message already named a
    /// stop reason (`docs/14` §9 item 28).
    fn end_turn(&mut self) -> Result<(), ProviderError> {
        self.ended = true;
        self.outbox.push_back(Ok(TurnEvent::Usage {
            usage: self.usage.clone(),
        }));
        match self.stop.clone() {
            Some(reason) => {
                self.outbox.push_back(Ok(TurnEvent::Stop { reason }));
                self.outbox.push_back(Ok(TurnEvent::Done));
                Ok(())
            }
            None => {
                self.finished = true;
                self.outbox.push_back(Err(ProviderError {
                    class: ErrorClass::InvalidRequest,
                    status: None,
                    message: "the stream ended without a stop reason".to_string(),
                }));
                Ok(())
            }
        }
    }
}

impl Stream for TurnStreamImpl {
    type Item = Result<TurnEvent, ProviderError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = &mut *self;
        loop {
            if let Some(event) = this.outbox.pop_front() {
                return Poll::Ready(Some(event));
            }
            if this.finished {
                // The state machine finished either at the contract's end
                // (usage, stop, done — `ended`) or at a failure.
                if this.ended {
                    return Poll::Ready(None);
                }
                return Poll::Ready(Some(Err(ProviderError {
                    class: ErrorClass::Unreachable,
                    status: None,
                    message: "the stream ended before the model finished".to_string(),
                })));
            }
            // The idle window: armed governs; a fired timer is a visible
            // error, exactly `TEST-AI-016`.
            if this.idle_armed && matches!(this.idle.as_mut().poll(cx), Poll::Ready(())) {
                this.finished = true;
                return Poll::Ready(Some(Err(ProviderError {
                    class: ErrorClass::Transient,
                    status: None,
                    message: format!(
                        "the stream went idle for {} s without progress",
                        this.idle_timeout.as_secs().max(1)
                    ),
                })));
            }
            match this.inner.as_mut().poll_next(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(None) => {
                    this.finished = true;
                    continue;
                }
                Poll::Ready(Some(chunk)) => match chunk {
                    Err(error) => {
                        this.finished = true;
                        return Poll::Ready(Some(Err(ProviderError {
                            class: ErrorClass::Unreachable,
                            status: None,
                            message: format!("the stream was interrupted: {error}"),
                        })));
                    }
                    Ok(chunk) => {
                        if let Err(error) = this.consume(&chunk) {
                            return Poll::Ready(Some(Err(error)));
                        }
                    }
                },
            }
        }
    }
}
