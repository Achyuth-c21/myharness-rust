//! The [`LocalAdapter`]: the one place in the app that talks the local
//! runtimes' OpenAI-compatible surface (Phase 8's second reference
//! extension; `docs/13` §"Phase 8"). It implements the
//! [`myharness_provider_sdk::ProviderAdapter`] contract and adds the
//! one-shot inference probe the setup window's *Test connection* runs.
//!
//! The destination gate (`crate::destination`) runs at construction and
//! pins the origin; the per-instance client this wall builds disables
//! redirects (`AI-LOCAL-04`) — the one deliberate deviation from the
//! injected-client posture, recorded in the crate header.

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use futures::Stream;
use myharness_provider_sdk::{
    AuthContext, CountRequest, ErrorClass, ModelCapabilities, ModelDescriptor, ModelKey,
    NativeStopReason, ProtocolFamily, ProviderAdapter, ProviderError, ProviderHealth, TokenCount,
    TurnEvent, TurnStream, Usage,
};
use secrecy::ExposeSecret;
use serde_json::{json, Value};

use crate::chat::{
    chat_request_body, error_from_envelope, error_from_status, model_echo_ok, parse_completion,
    parse_usage, stop_reason, EventParser, LocalClientConfig, ProbeStatus, WireEvent,
};
use crate::destination::Destination;
use crate::discovery::{descriptor_from_row, discover_with};

/// The wall's per-instance client: redirects disabled (`AI-LOCAL-04` — a
/// local endpoint that answers with a redirect to any other origin errors,
/// never follows) and a bounded connect window. Nothing here is
/// process-global (`AI-ADAPTER-13`).
fn wall_client() -> Result<reqwest::Client, ProviderError> {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .build()
        .map_err(|error| ProviderError {
            class: ErrorClass::InvalidRequest,
            status: None,
            message: format!("the local wall's client could not start: {error}"),
        })
}

/// The local provider adapter. Constructed per profile by the caller (the
/// bridge); construction validates and pins the destination.
#[derive(Debug)]
pub struct LocalAdapter {
    config: LocalClientConfig,
    destination: Destination,
    client: reqwest::Client,
}

impl LocalAdapter {
    /// Validate the profile's base URL with the system resolver and build
    /// the wall.
    pub fn new(config: LocalClientConfig) -> Result<Self, ProviderError> {
        Self::with_resolver(config, Arc::new(crate::destination::resolve_system))
    }

    /// The resolver-injecting constructor (tests and constrained
    /// environments). The gate is synchronous by design — it must never
    /// start a runtime to work, so construction is safe from any context.
    pub fn with_resolver(
        config: LocalClientConfig,
        resolver: crate::destination::ResolveFn,
    ) -> Result<Self, ProviderError> {
        let destination = Destination::resolve(&config.base_url, &resolver)?;
        Ok(LocalAdapter {
            config,
            destination,
            client: wall_client()?,
        })
    }

    /// The pinned origin requests are issued against.
    pub fn pinned_origin(&self) -> &str {
        self.destination.origin()
    }

    /// The shared request header set: the credential only as `Bearer` when
    /// one exists (`AI-LOCAL-05` — keyless sends no `authorization` header
    /// at all), plus the profile's forwarded custom headers.
    fn request_builder(
        &self,
        method: reqwest::Method,
        url: &str,
        auth: &AuthContext,
    ) -> reqwest::RequestBuilder {
        let mut builder = self.client.request(method, url);
        let credential = auth.credential().expose_secret().trim();
        if !credential.is_empty() {
            builder = builder.header("authorization", format!("Bearer {credential}"));
        }
        for (name, value) in &self.config.custom_headers {
            builder = builder.header(name, value);
        }
        builder
    }

    /// The `{pinned}/v1/chat/completions` URL.
    fn completions_url(&self) -> String {
        self.destination.url("/v1/chat/completions")
    }

    /// The structural gate every send passes first: the id must be a legal
    /// model string; the id the user named travels verbatim (`AI-MODEL-01`).
    fn gate_model(&self, model: &ModelKey) -> Result<(), ProviderError> {
        match myharness_config::forms::validate_open_model_id(&model.model_id) {
            Ok(()) => Ok(()),
            Err(reason) => Err(ProviderError {
                class: ErrorClass::InvalidRequest,
                status: None,
                message: format!("the requested model was not sent: {reason}"),
            }),
        }
    }

    async fn send(
        &self,
        builder: reqwest::RequestBuilder,
    ) -> Result<reqwest::Response, ProviderError> {
        builder.send().await.map_err(|error| ProviderError {
            class: ErrorClass::Unreachable,
            status: None,
            message: format!("the local endpoint could not be reached: {error}"),
        })
    }

    /// A non-success response, mapped: envelope first, bare status otherwise.
    async fn response_error(response: reqwest::Response) -> ProviderError {
        let status = response.status().as_u16();
        let body = response.json::<Value>().await.ok();
        match body {
            Some(body) => error_from_envelope(Some(status), &body),
            None => error_from_status(status),
        }
    }

    /// Send non-streaming completions.
    async fn send_completion(
        &self,
        auth: &AuthContext,
        request: &myharness_provider_sdk::NativeTurnRequest,
    ) -> Result<Value, ProviderError> {
        let builder = self
            .request_builder(reqwest::Method::POST, &self.completions_url(), auth)
            .json(&chat_request_body(request)?);
        let response = self.send(builder).await?;
        let status = response.status();
        if !status.is_success() {
            return Err(Self::response_error(response).await);
        }
        response
            .json::<Value>()
            .await
            .map_err(|error| ProviderError {
                class: ErrorClass::InvalidRequest,
                status: None,
                message: format!("the provider answer could not be read: {error}"),
            })
    }

    /// Translate a completed (non-streaming) completion into the typed
    /// event set. The echo gate applies; a missing `finish_reason` is
    /// contract-malformed and no "end of turn" is invented.
    fn events_from_completion(
        body: &Value,
        requested_model: &ModelKey,
    ) -> Result<Vec<TurnEvent>, ProviderError> {
        let parsed = parse_completion(body)?;
        if let Some(ran) = parsed.model.as_deref() {
            if !model_echo_ok(&requested_model.model_id, ran) {
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
        let finish = parsed
            .finish_reason
            .as_deref()
            .ok_or_else(|| ProviderError {
                class: ErrorClass::InvalidRequest,
                status: None,
                message: "the completion carried no finish_reason".to_string(),
            })?;
        let mut events = Vec::new();
        if !parsed.text.is_empty() {
            events.push(TurnEvent::TextDelta { text: parsed.text });
        }
        if !parsed.reasoning.is_empty() {
            events.push(TurnEvent::ReasoningDelta {
                text: parsed.reasoning,
                signature: None,
            });
        }
        for call in parsed.tool_calls {
            let arguments = if call.arguments.trim().is_empty() {
                json!({})
            } else {
                serde_json::from_str(&call.arguments).map_err(|error| ProviderError {
                    class: ErrorClass::InvalidRequest,
                    status: None,
                    message: format!(
                        "the tool arguments were not valid JSON; nothing was executed: {error}"
                    ),
                })?
            };
            events.push(TurnEvent::ToolCall {
                id: call.id,
                name: call.name,
                arguments_delta: None,
                arguments: Some(arguments),
            });
        }
        if let Some(usage) = parsed.usage {
            if usage != Usage::default() {
                events.push(TurnEvent::Usage { usage });
            }
        }
        events.push(TurnEvent::Stop {
            reason: stop_reason(finish),
        });
        events.push(TurnEvent::Done);
        Ok(events)
    }

    /// The one-shot inference probe (the setup window's *Test connection*;
    /// `docs/screens/setup-desktop-3p.md` — a real request, nothing
    /// persisted, no draft changed by the answer).
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
        let response = self.send_completion(auth, &request).await?;
        let latency = started.elapsed();
        let parsed = parse_completion(&response)?;
        let ran = parsed.model.clone().unwrap_or_else(|| model_id.to_string());
        if !model_echo_ok(model_id, &ran) {
            return Err(ProviderError {
                class: ErrorClass::ModelNotFound,
                status: None,
                message: format!(
                    "the provider performed model {ran:?} while the probe named {model_id:?}"
                ),
            });
        }
        let finish = parsed
            .finish_reason
            .clone()
            .unwrap_or_else(|| "stop".to_string());
        Ok(ProbeStatus {
            ok: true,
            model: ran,
            stop_reason: Some(stop_reason(&finish).wire()),
            input_tokens: parsed
                .usage
                .as_ref()
                .map(|usage| usage.input)
                .unwrap_or_default(),
            output_tokens: parsed
                .usage
                .as_ref()
                .map(|usage| usage.output)
                .unwrap_or_default(),
            latency_ms: u64::try_from(latency.as_millis()).unwrap_or(u64::MAX),
        })
    }
}

// ---------------------------------------------------------------------------
// The streaming state machine
// ---------------------------------------------------------------------------

/// One in-flight function call, keyed by the wire's `tool_calls` index —
/// the id may travel only on the first fragment, so the index is the join
/// key and the id resolves at the close (`AI-TOOL-01`).
#[derive(Debug, Clone)]
struct ToolBlockState {
    id: Option<String>,
    name: String,
    buffer: String,
}

/// The `TurnStream` implementation for streamed turns (`AI-LOCAL-05..06`
/// grammar; the sibling readings pinned by this crate's fixtures).
///
/// Framing: `data:` JSON frames; a `data: {}` empty-object frame is a
/// keep-alive that never re-arms the idle window (`TEST-AI-016`); the
/// `data: [DONE]` sentinel is REQUIRED — a stream that ends without one is
/// contract-malformed and no stop reason is invented.
pub(crate) struct TurnStreamImpl {
    inner: Pin<Box<dyn Stream<Item = reqwest::Result<bytes::Bytes>> + Send>>,
    parser: EventParser,
    requested_model: String,
    /// The completion id chunks carry; the synthetic tool-call id base.
    completion_id: String,
    idle_timeout: Duration,
    idle: Pin<Box<tokio::time::Sleep>>,
    idle_armed: bool,
    tools: BTreeMap<u64, ToolBlockState>,
    /// The recorded announcement order of every call (the close emits
    /// assembled calls in announcement order).
    tool_order: Vec<u64>,
    usage: Usage,
    saw_usage: bool,
    outbox: std::collections::VecDeque<Result<TurnEvent, ProviderError>>,
    finished: bool,
    saw_terminal: bool,
}

impl TurnStreamImpl {
    fn rearm_idle(&mut self) {
        if self.idle_armed {
            self.idle = Box::pin(tokio::time::sleep(self.idle_timeout));
        }
    }

    fn consume(&mut self, chunk: &[u8]) -> Result<(), ProviderError> {
        for parsed in self.parser.feed(chunk) {
            match parsed {
                Err(error) => {
                    self.finished = true;
                    return Err(error);
                }
                Ok(event) => self.step(event)?,
            }
        }
        Ok(())
    }

    /// One parsed wire frame. Real progress re-arms the idle window;
    /// transport keep-alives (comments, empty frames) and the `data: {}`
    /// ping never do (`TEST-AI-016`).
    fn step(&mut self, event: WireEvent) -> Result<(), ProviderError> {
        let data = event.data.trim().to_string();
        if data.is_empty() {
            return Ok(());
        }
        if data == "[DONE]" {
            // The sentinel: the stream's one clean ending.
            self.finish_stream()?;
            return Ok(());
        }
        let body: Value = serde_json::from_str(&data).map_err(|error| {
            self.finished = true;
            ProviderError {
                class: ErrorClass::InvalidRequest,
                status: None,
                message: format!("the stream carried a frame that is not valid JSON: {error}"),
            }
        })?;
        if body == json!({}) {
            // The empty-object keep-alive ping: progress bytes for the
            // transport, never a reset of the idle window.
            return Ok(());
        }
        self.rearm_idle();
        if let Some(error) = body.get("error") {
            self.finished = true;
            return Err(error_from_envelope(None, &json!({ "error": error })));
        }
        if let Some(ran) = body.get("model").and_then(Value::as_str) {
            if !model_echo_ok(&self.requested_model, ran) {
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
            if self.completion_id.is_empty() {
                if let Some(id) = body.get("id").and_then(Value::as_str) {
                    self.completion_id = id.to_string();
                }
            }
        }
        let Some(choices) = body.get("choices").and_then(Value::as_array) else {
            return Ok(());
        };
        let Some(choice) = choices.first() else {
            return Ok(());
        };
        if let Some(delta) = choice.get("delta") {
            self.map_delta(delta)?;
        }
        // The terminal chunk's usage must land BEFORE the close flushes the
        // outbox — the close emits the exact usage the turn reported.
        if let Some(usage) = body.get("usage").filter(|usage| !usage.is_null()) {
            if let Some(value) = parse_usage(usage) {
                self.saw_usage = true;
                self.usage = value;
            }
        }
        if let Some(finish) = choice.get("finish_reason").and_then(Value::as_str) {
            self.finish_with(stop_reason(finish));
        }
        Ok(())
    }

    fn map_delta(&mut self, delta: &Value) -> Result<(), ProviderError> {
        if let Some(text) = delta.get("content").and_then(Value::as_str) {
            if !text.is_empty() {
                self.outbox.push_back(Ok(TurnEvent::TextDelta {
                    text: text.to_string(),
                }));
            }
        }
        let reasoning = delta
            .get("reasoning_content")
            .and_then(Value::as_str)
            .or_else(|| delta.get("reasoning").and_then(Value::as_str));
        if let Some(text) = reasoning {
            if !text.is_empty() {
                self.outbox.push_back(Ok(TurnEvent::ReasoningDelta {
                    text: text.to_string(),
                    signature: None,
                }));
            }
        }
        for call in delta
            .get("tool_calls")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
        {
            let index = call.get("index").and_then(Value::as_u64).unwrap_or(0);
            self.map_tool_fragment(index, &call)?;
        }
        Ok(())
    }

    /// One `delta.tool_calls` fragment. Fragments join by the wire's
    /// `index`; the id (usually only the first fragment's) resolves per
    /// call, and the recorded synthetic spelling (`{completion-id}-{index}`,
    /// `call-{index}` when the chunk carries no completion id) covers a
    /// call whose id never travels (`AI-TOOL-01`).
    fn map_tool_fragment(&mut self, index: u64, call: &Value) -> Result<(), ProviderError> {
        let declared_id = call.get("id").and_then(Value::as_str).map(str::to_string);
        let function = call.get("function").cloned().unwrap_or_else(|| json!({}));
        let name = function
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let fragment = function
            .get("arguments")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if !self.tools.contains_key(&index) {
            let resolved_id = declared_id.clone().unwrap_or_else(|| {
                if self.completion_id.is_empty() {
                    format!("call-{index}")
                } else {
                    format!("{}-{index}", self.completion_id)
                }
            });
            self.tools.insert(
                index,
                ToolBlockState {
                    id: declared_id.clone(),
                    name: name.clone(),
                    buffer: String::new(),
                },
            );
            self.tool_order.push(index);
            self.outbox.push_back(Ok(TurnEvent::ToolCall {
                id: resolved_id,
                name: name.clone(),
                arguments_delta: None,
                arguments: None,
            }));
        }
        let state = self.tools.get_mut(&index).expect("checked above");
        if let Some(id) = declared_id {
            state.id = Some(id);
        }
        if !name.is_empty() {
            state.name = name;
        }
        if !fragment.is_empty() {
            state.buffer.push_str(&fragment);
            let state = self.tools.get(&index).expect("checked above");
            let id = state.id.clone().unwrap_or_else(|| {
                if self.completion_id.is_empty() {
                    format!("call-{index}")
                } else {
                    format!("{}-{index}", self.completion_id)
                }
            });
            self.outbox.push_back(Ok(TurnEvent::ToolCall {
                id,
                name: state.name.clone(),
                arguments_delta: Some(fragment),
                arguments: None,
            }));
        }
        Ok(())
    }

    /// Close the turn at the `finish_reason` chunk: assembled tool calls
    /// (validated — an invalid buffer is a visible error, nothing executes
    /// half-parsed), the exact usage the provider reported, the stop, and
    /// the typed end-of-stream (`TEST-AI-013/014`).
    fn finish_with(&mut self, reason: NativeStopReason) {
        for index in self.tool_order.clone() {
            let Some(state) = self.tools.remove(&index) else {
                continue;
            };
            let id = state.id.clone().unwrap_or_else(|| {
                if self.completion_id.is_empty() {
                    format!("call-{index}")
                } else {
                    format!("{}-{index}", self.completion_id)
                }
            });
            let arguments = if state.buffer.trim().is_empty() {
                json!({})
            } else {
                match serde_json::from_str(&state.buffer) {
                    Ok(arguments) => arguments,
                    Err(error) => {
                        self.finished = true;
                        self.outbox.push_back(Err(ProviderError {
                            class: ErrorClass::InvalidRequest,
                            status: None,
                            message: format!(
                                "the tool arguments for {:?} were not valid JSON; nothing was executed: {error}",
                                state.name
                            ),
                        }));
                        return;
                    }
                }
            };
            self.outbox.push_back(Ok(TurnEvent::ToolCall {
                id,
                name: state.name,
                arguments_delta: None,
                arguments: Some(arguments),
            }));
        }
        if self.saw_usage {
            self.outbox.push_back(Ok(TurnEvent::Usage {
                usage: self.usage.clone(),
            }));
        }
        self.outbox.push_back(Ok(TurnEvent::Stop { reason }));
        self.outbox.push_back(Ok(TurnEvent::Done));
        self.saw_terminal = true;
    }

    /// The sentinel path: end the stream exactly once, with whatever the
    /// `finish_reason` chunk already emitted — or malformed if none arrived
    /// (no stop reason is invented).
    fn finish_stream(&mut self) -> Result<(), ProviderError> {
        if self.saw_terminal {
            self.finished = true;
            return Ok(());
        }
        self.finished = true;
        Err(ProviderError {
            class: ErrorClass::Unreachable,
            status: None,
            message:
                "the stream ended at the [DONE] sentinel without a finish_reason; no stop reason was invented"
                    .to_string(),
        })
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
                    if this.saw_terminal {
                        return Poll::Ready(None);
                    }
                    return Poll::Ready(Some(Err(ProviderError {
                        class: ErrorClass::Unreachable,
                        status: None,
                        message:
                            "the stream ended before the [DONE] sentinel; no stop reason was invented"
                                .to_string(),
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

/// Turn a successful streaming response into the typed [`TurnStream`].
/// Dropping the returned stream cancels the native request (`TEST-AI-015`).
fn finish_stream_into_turn_stream(
    response: reqwest::Response,
    requested_model: String,
    idle_timeout: Duration,
) -> TurnStream {
    let idle_armed = !idle_timeout.is_zero();
    Box::pin(TurnStreamImpl {
        inner: Box::pin(response.bytes_stream()),
        parser: EventParser::new(),
        requested_model,
        completion_id: String::new(),
        idle_timeout,
        idle: Box::pin(tokio::time::sleep(idle_timeout)),
        idle_armed,
        tools: BTreeMap::new(),
        tool_order: Vec::new(),
        usage: Usage::default(),
        saw_usage: false,
        outbox: std::collections::VecDeque::new(),
        finished: false,
        saw_terminal: false,
    })
}

#[async_trait::async_trait]
impl ProviderAdapter for LocalAdapter {
    fn id(&self) -> &str {
        "local"
    }

    fn protocol(&self) -> ProtocolFamily {
        ProtocolFamily::OpenAiChat
    }

    fn display_name(&self) -> &str {
        "Local provider (Ollama / llama.cpp)"
    }

    /// Keyless is the family default (`AI-LOCAL-05`): an empty credential is
    /// not a refusal — it sends no `authorization` header. The destination
    /// gate, not the credential, decides where requests may go.
    async fn authenticate(&self, _auth: &AuthContext) -> Result<(), ProviderError> {
        Ok(())
    }

    /// Discovery over the pinned destination (`AI-LOCAL-06`): `/api/tags`
    /// first, `/v1/models` on a 404 from the first. (`TEST-AI-018`.)
    async fn list_models(&self, auth: &AuthContext) -> Result<Vec<ModelDescriptor>, ProviderError> {
        let credential = auth.credential().expose_secret().trim().to_string();
        let outcome = discover_with(
            &self.destination,
            Some(&credential),
            &self.config.custom_headers,
            &self.client,
        )
        .await?;
        Ok(outcome
            .models
            .iter()
            .filter_map(|model| descriptor_from_row(self.id(), &model.raw))
            .collect())
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

    /// Build the native request and open the event stream. A non-streaming
    /// request becomes a buffered event list from the same contract.
    async fn create_turn(
        &self,
        auth: &AuthContext,
        request: &myharness_provider_sdk::NativeTurnRequest,
    ) -> Result<TurnStream, ProviderError> {
        self.gate_model(&request.model)?;
        self.authenticate(auth).await?;
        let builder = self
            .request_builder(reqwest::Method::POST, &self.completions_url(), auth)
            .json(&chat_request_body(request)?);
        let response = self.send(builder).await?;
        if !response.status().is_success() {
            return Err(Self::response_error(response).await);
        }
        if request.stream {
            Ok(finish_stream_into_turn_stream(
                response,
                request.model.model_id.clone(),
                self.config.idle_timeout,
            ))
        } else {
            let body: Value = response.json().await.map_err(|error| ProviderError {
                class: ErrorClass::InvalidRequest,
                status: None,
                message: format!("the provider answer could not be read: {error}"),
            })?;
            let events = Self::events_from_completion(&body, &request.model)?;
            Ok(Box::pin(futures::stream::iter(events.into_iter().map(Ok))))
        }
    }

    /// Neither runtime documents a compatible count surface on this path: an
    /// explicit refusal, never an estimate (`AI-ADAPTER-09`).
    async fn count_tokens(
        &self,
        _auth: &AuthContext,
        request: &CountRequest,
    ) -> Result<TokenCount, ProviderError> {
        self.gate_model(&request.model)?;
        Err(ProviderError {
            class: ErrorClass::InvalidRequest,
            status: None,
            message: format!(
                "the local compat surface offers no native token-count endpoint for {:?}; no count was produced and none was estimated",
                request.model.model_id
            ),
        })
    }

    /// The pinned destination's own reachability check: the listing endpoint
    /// the runtimes share, no inference (`docs/12` §8.2 `healthCheck`).
    async fn health_check(&self, auth: &AuthContext) -> Result<ProviderHealth, ProviderError> {
        let builder = self.request_builder(
            reqwest::Method::GET,
            &self.destination.url("/v1/models"),
            auth,
        );
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
