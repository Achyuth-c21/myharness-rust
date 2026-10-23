//! The [`BedrockAdapter`]: the one place in the app that talks the Bedrock
//! family's native protocol — the Bedrock Runtime invoke surface
//! (`docs/12` §1 row `bedrock`; `docs/13` Phase 3 order item 3). It
//! implements the [`myharness_provider_sdk::ProviderAdapter`] contract and
//! adds the one-shot inference probe the setup window's *Test connection*
//! runs (`docs/screens/setup-desktop-3p.md`, Phase 3 slice 3).
//!
//! Boundaries this crate holds (`docs/12` §11): credentials arrive resolved
//! in an [`AuthContext`], re-resolved by the caller per request
//! (`AI-PROVIDER-03`); the model policy gates every send and nothing is
//! substituted (`AI-MODEL-01`, `AI-ADAPTER-11`); nothing here is
//! process-global (`AI-ADAPTER-13`); the credential is never logged, never
//! persisted, and never copied into any other store.

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

use crate::discovery::{descriptor_from_row, DiscoveryOutcome};
use crate::eventstream::FrameDecoder;
use crate::messages::{
    encode_model_segment, error_from_envelope, error_from_status, frame_to_stream_event,
    invoke_body, parse_usage, ProbeStatus, StreamEvent,
};
use crate::BedrockClientConfig;

/// The Bedrock family adapter. Constructed per profile by the caller (the
/// bridge); [`reqwest::Client`] is injected so connection pooling is the
/// caller's decision and nothing here is process-global (`AI-ADAPTER-13`).
pub struct BedrockAdapter {
    config: BedrockClientConfig,
    client: reqwest::Client,
}

/// A useful debug answer with no credential material — the config carries no
/// credential, and the client's face is its own (`docs/09` §4).
impl std::fmt::Debug for BedrockAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BedrockAdapter")
            .field("config", &self.config)
            .finish()
    }
}

impl BedrockAdapter {
    /// Construct the adapter. Trailing slashes are normalized
    /// (`docs/12` §3.1).
    pub fn new(config: BedrockClientConfig, client: reqwest::Client) -> Self {
        let mut config = config;
        config.runtime_base_url = config
            .runtime_base_url
            .trim()
            .trim_end_matches('/')
            .to_string();
        config.control_base_url = config
            .control_base_url
            .trim()
            .trim_end_matches('/')
            .to_string();
        BedrockAdapter { config, client }
    }

    /// The normalized runtime base URL.
    pub fn runtime_base_url(&self) -> &str {
        &self.config.runtime_base_url
    }

    /// The normalized control-plane base URL.
    pub fn control_base_url(&self) -> &str {
        &self.config.control_base_url
    }

    /// The shared request header set: the family's native bearer credential
    /// (`docs/09` §2.2 "static bearer") and forwarded custom headers. The
    /// credential exists only as request-header material — it is never
    /// logged, never persisted, and never copied into any other store
    /// (`AI-PROVIDER-03`).
    fn request_builder(
        &self,
        method: reqwest::Method,
        url: &str,
        auth: &AuthContext,
    ) -> reqwest::RequestBuilder {
        let mut builder = self
            .client
            .request(method, url)
            // The family's native auth: a bearer credential (`docs/09` §2.2);
            // there is no scheme field to interpret for this family.
            .header(
                "Authorization",
                format!("Bearer {}", auth.credential().expose_secret()),
            );
        for (name, value) in &self.config.custom_headers {
            builder = builder.header(name, value);
        }
        builder
    }

    /// The `invoke` URL for a model id or ARN (`docs/14` §9: the path is the
    /// model). A request only ever goes to the configured runtime endpoint.
    fn invoke_url(&self, model_id: &str) -> String {
        format!(
            "{}/model/{}/invoke",
            self.config.runtime_base_url,
            encode_model_segment(model_id)
        )
    }

    /// The `invoke-with-response-stream` URL for a model id or ARN.
    fn invoke_stream_url(&self, model_id: &str) -> String {
        format!(
            "{}/model/{}/invoke-with-response-stream",
            self.config.runtime_base_url,
            encode_model_segment(model_id)
        )
    }

    /// The model-apis listing URL on the control plane.
    fn models_url(&self) -> String {
        format!("{}/foundation-models", self.config.control_base_url)
    }

    /// Read a response's JSON, or turn a failure into the mapped provider
    /// error. A non-success status is parsed for the native error envelope
    /// (its exception name and message only — no raw bodies, `docs/18` §5.4).
    async fn response_json(
        response: reqwest::Response,
    ) -> Result<serde_json::Value, ProviderError> {
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

    /// Send non-streaming `invoke`.
    async fn send_invoke(
        &self,
        auth: &AuthContext,
        request: &myharness_provider_sdk::NativeTurnRequest,
    ) -> Result<serde_json::Value, ProviderError> {
        let builder = self
            .request_builder(
                reqwest::Method::POST,
                &self.invoke_url(&request.model.model_id),
                auth,
            )
            .json(&invoke_body(request)?);
        let response = self.send(builder).await?;
        Self::response_json(response).await
    }

    /// The policy gate every send passes first (`TEST-AI-007/008`): the
    /// family's recorded model-ID policy, `anthropic.*` ids and Bedrock
    /// inference/provisioned ARNs.
    fn gate_model(&self, model: &ModelKey) -> Result<(), ProviderError> {
        match myharness_config::forms::validate_bedrock_model_id(&model.model_id) {
            Ok(()) => Ok(()),
            Err(reason) => Err(ProviderError {
                class: ErrorClass::InvalidRequest,
                status: None,
                message: format!(
                    "the requested model {:?} does not pass the Bedrock model policy and was not sent: {reason}",
                    model.model_id
                ),
            }),
        }
    }

    /// The beta-list gate. The family's recorded surface has no beta
    /// transport: the invoke grammar carries no beta slot and no beta header
    /// spelling was ever captured for it (`docs/14` §9). Forwarding would be
    /// a guess; dropping silently is exactly the `AI-ADAPTER-07` failure
    /// mode. A caller that asks for betas is refused visibly, before any
    /// bytes travel.
    fn gate_betas(&self, betas: &[String]) -> Result<(), ProviderError> {
        if betas.is_empty() {
            return Ok(());
        }
        Err(ProviderError {
            class: ErrorClass::InvalidRequest,
            status: None,
            message: format!(
                "the Bedrock Runtime surface has no recorded beta transport ({:?} requested); the turn was refused rather than the identifiers dropped — beta features remain first-party-surface features (`docs/14` §9)",
                betas.join(", ")
            ),
        })
    }

    /// Translate a complete (non-streaming) invoke answer into the typed
    /// event set (`docs/12` §8.2): content, usage, stop reason, done. Content
    /// blocks are preserved part-for-part; an unknown block kind is an error,
    /// never a silent drop (`AI-MM-01`, `AI-STREAM-01`).
    ///
    /// The model echo is **conditional**: an answer that names a model
    /// different from the request ends the turn visibly, but a Bedrock
    /// answer without a model field is not an echo failure — the field's
    /// presence is provider-dependent, and inventing a comparison would
    /// refuse whole surfaces by reading (`AI-MODEL-01`; `docs/14` §9).
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
                "redacted_thinking" => events.push(TurnEvent::ReasoningRedacted {
                    data: block.clone(),
                }),
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
                // A response without a stop reason is contract-malformed:
                // refusing an invented "end of turn" is the point.
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

    /// The one-shot inference probe (the setup window's *Test connection*;
    /// `docs/screens/setup-desktop-3p.md` — a real request, nothing
    /// persisted, no draft changed by the answer).
    ///
    /// Sends a 1-token turn to the requested model. Reports exact usage and
    /// the native stop reason (`TEST-AI-013/014` probe rows), returns
    /// provider error classes verbatim (`TEST-AI-021`), and refuses a model
    /// swap (`AI-MODEL-01`). Latency is measured around the native request.
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
        let response = self.send_invoke(auth, &request).await?;
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
/// bytes stream feeds the event-stream decoder, the state machine guards the
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
        decoder: FrameDecoder::new(),
        requested_model,
        idle_timeout,
        idle: Box::pin(tokio::time::sleep(idle_timeout)),
        idle_armed,
        tools: BTreeMap::new(),
        usage: Usage::default(),
        stop: None,
        outbox: VecDeque::new(),
        finished: false,
        saw_message_stop: false,
    })
}

#[async_trait::async_trait]
impl ProviderAdapter for BedrockAdapter {
    fn id(&self) -> &str {
        "bedrock"
    }

    /// The family's protocol: Anthropic Messages over the cloud's Bedrock
    /// Runtime transport (`docs/12` §1; `AI-EXT-02`).
    fn protocol(&self) -> ProtocolFamily {
        ProtocolFamily::AnthropicMessagesViaCloud("bedrock")
    }

    fn display_name(&self) -> &str {
        "Bedrock"
    }

    /// A shape check with no network: the family's recorded credential is a
    /// bearer token (`docs/09` §2.2); a non-empty one is shaped. Family
    /// credential kinds the rebuild cannot resolve (profile/SSO, web
    /// identity, vendor profile) never reach the adapter with material —
    /// the bridge refuses them fail-closed (`docs/14` §9).
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

    /// The native listing: the control plane's model APIs, rows mapped to
    /// descriptors (`docs/18` §6 "Bedrock model APIs or static";
    /// `TEST-AI-018`).
    async fn list_models(&self, auth: &AuthContext) -> Result<Vec<ModelDescriptor>, ProviderError> {
        let outcome: DiscoveryOutcome = crate::discovery::discover_pages(
            &self.config.control_base_url,
            Some(auth.credential().expose_secret().to_string()),
            &self.config.custom_headers,
            &self.client,
            crate::discovery::DISCOVERY_PAGE_TIMEOUT.as_secs(),
            crate::discovery::DISCOVERY_MAX_PAGES,
        )
        .await?;
        let descriptors: Vec<ModelDescriptor> = outcome
            .models
            .iter()
            .filter_map(|model| descriptor_from_row(self.id(), &model.raw))
            .collect();
        Ok(descriptors)
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
                    &self.invoke_stream_url(&request.model.model_id),
                    auth,
                )
                .json(&invoke_body(request)?);
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
            let response = self.send_invoke(auth, request).await?;
            let events = Self::events_from_response(&response, &request.model)?;
            Ok(Box::pin(futures::stream::iter(events.into_iter().map(Ok))))
        }
    }

    /// The recorded family surface exposes no native token-count endpoint;
    /// the answer is an error — never length/4, never an estimate
    /// (`AI-ADAPTER-09`; `docs/14` §9). The request is refused before any
    /// bytes are sent.
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
                "the Bedrock Runtime surface offers no native token-count endpoint for {:?}; no count was produced and none was estimated",
                request.model.model_id
            ),
        })
    }

    /// The family's model-apis first listing page as the reachability check
    /// (`docs/12` §8.2 `healthCheck`). No inference happens here, and no
    /// parameter is invented: the request is the discovery first page.
    async fn health_check(&self, auth: &AuthContext) -> Result<ProviderHealth, ProviderError> {
        let builder = self.request_builder(reqwest::Method::GET, &self.models_url(), auth);
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

/// The `TurnStream` implementation for streamed turns: decodes the provider's
/// native event-stream frames into the app's typed events, guards the model
/// echo (conditionally — see `events_from_response`), preserves usage and
/// stop reasons, and enforces the idle window (`TEST-AI-014/015/016`).
///
/// Dropping the stream cancels the native request: the `reqwest` response
/// (and its body task) drops with it, which closes the connection — the
/// caller keeps whatever was already emitted and marks it stopped
/// (`docs/12` §4.1).
pub(crate) struct TurnStreamImpl {
    inner: Pin<Box<dyn Stream<Item = reqwest::Result<bytes::Bytes>> + Send>>,
    decoder: FrameDecoder,
    requested_model: String,
    idle_timeout: Duration,
    /// The idle-window timer (re-armed on content progress; keep-alives are
    /// not progress — the reading in `messages.rs` module docs).
    idle: Pin<Box<tokio::time::Sleep>>,
    /// Whether the idle window governs this stream at all (a `0` window
    /// means "no idle timeout"; the profile documents a nonzero default).
    idle_armed: bool,
    tools: BTreeMap<usize, ToolBlockState>,
    usage: Usage,
    stop: Option<NativeStopReason>,
    outbox: VecDeque<Result<TurnEvent, ProviderError>>,
    finished: bool,
    saw_message_stop: bool,
}

impl TurnStreamImpl {
    fn rearm_idle(&mut self) {
        if self.idle_armed {
            self.idle = Box::pin(tokio::time::sleep(self.idle_timeout));
        }
    }

    /// Absorb one byte chunk; append zero or more typed events to the
    /// outbox, or terminate the stream on a malformed/error frame.
    fn consume(&mut self, chunk: &[u8]) -> Result<(), ProviderError> {
        for parsed in self.decoder.feed(chunk) {
            match parsed {
                Err(error) => {
                    // Malformed bytes/frames surface — nothing is silently
                    // skipped (`AI-MM-01`; `docs/12` §4.1).
                    self.finished = true;
                    return Err(error);
                }
                Ok(frame) => match frame_to_stream_event(&frame) {
                    None => {
                        self.finished = true;
                        return Err(ProviderError {
                            class: ErrorClass::InvalidRequest,
                            status: None,
                            message: "the event-stream frame produced no stream event".to_string(),
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

    /// One parsed stream frame. Progress events re-arm the idle window;
    /// keep-alives never do (`TEST-AI-016`).
    fn step(&mut self, event: StreamEvent) -> Result<(), ProviderError> {
        match event {
            StreamEvent::Ping => Ok(()),
            StreamEvent::MessageStart { model, usage } => {
                // Conditional echo, as elsewhere (`docs/14` §9).
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
                        Some(kind) if kind.contains("Throttled") => ErrorClass::Quota,
                        Some(_) => ErrorClass::InvalidRequest,
                        None => ErrorClass::InvalidRequest,
                    },
                    status: None,
                    message: message
                        .unwrap_or_else(|| "the stream ended with an error event".to_string()),
                })
            }
            StreamEvent::MessageStop => {
                self.saw_message_stop = true;
                // The final usage is the exact number the provider reported
                // (`TEST-AI-013`; `docs/12` §4.2), then the preserved stop
                // reason (`TEST-AI-014`), then the typed end-of-stream.
                self.outbox.push_back(Ok(TurnEvent::Usage {
                    usage: self.usage.clone(),
                }));
                match self.stop.clone() {
                    Some(reason) => self.outbox.push_back(Ok(TurnEvent::Stop { reason })),
                    None => {
                        // A stream that ended without a stop reason is
                        // contract-malformed; no "end_turn" is invented.
                        self.outbox.push_back(Err(ProviderError {
                            class: ErrorClass::InvalidRequest,
                            status: None,
                            message: "the stream ended without a stop reason".to_string(),
                        }));
                    }
                }
                self.outbox.push_back(Ok(TurnEvent::Done));
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
                return Poll::Ready(None);
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
                    if this.saw_message_stop {
                        return Poll::Ready(None);
                    }
                    return Poll::Ready(Some(Err(ProviderError {
                        class: ErrorClass::Unreachable,
                        status: None,
                        message: "the stream ended before the model finished".to_string(),
                    })));
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
