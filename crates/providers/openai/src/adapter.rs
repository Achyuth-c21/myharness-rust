//! The [`OpenAiCompatAdapter`]: the one place in the app that talks the
//! OpenAI Chat Completions wire (`docs/13` §C open model layer; `docs/10`
//! §9). It implements the [`myharness_provider_sdk::ProviderAdapter`]
//! contract and adds the one-shot inference probe the setup window's *Test
//! connection* runs (`docs/screens/setup-desktop-3p.md`).
//!
//! Boundaries this crate holds (`docs/12` §11 requirements):
//!
//! - **Credentials** arrive resolved in an [`AuthContext`], re-resolved by
//!   the caller per request — rotation without a second credential source
//!   (`AI-PROVIDER-03`; the `AI-ADAPTER-12` regression). The adapter neither
//!   reads the keychain nor writes provider credentials anywhere, and a
//!   keyless send leaves the `authorization` header off entirely (the
//!   `custom` family's keyless servers — `docs/14` §9 owner decision).
//! - **Model ids travel verbatim, ungated** — since the owner's open model
//!   layer (2026-10-23, `docs/14` §9) no family predicate screens ids
//!   anywhere in the rebuild: any id the user names is sent as-is. The
//!   identity checks that remain are structural: the base URL must exist,
//!   the id must be a legal string, and a performed model outside the
//!   requested id's prefix stops the turn visibly (`AI-MODEL-01`).
//! - **Nothing global** (`AI-ADAPTER-13`): a concrete [`reqwest::Client`] is
//!   injected per adapter; there is no fetch interception, no module state,
//!   no side-channel log file.
//! - **No token-count estimate** (`AI-ADAPTER-09`): this wire offers no
//!   counting endpoint, so counting is an explicit refusal.

use std::collections::{BTreeMap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use futures::Stream;
use myharness_provider_sdk::{
    AuthContext, CountRequest, ErrorClass, ModelCapabilities, ModelDescriptor, ModelKey,
    NativeStopReason, ProtocolFamily, ProviderAdapter, ProviderError, ProviderHealth, TokenCount,
    TurnEvent, TurnStream, Usage,
};
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::chat::{
    chat_request_body, error_from_envelope, error_from_status, model_echo_ok, parse_completion,
    parse_usage, stop_reason, ChunkParser, OpenAiClientConfig, ProbeStatus, StreamEvent,
};
use crate::discovery::{descriptor_from_row, discover_with};
use crate::DEFAULT_OPENAI_BASE;

/// Which family an adapter instance serves. Both talk one wire; the kind
/// decides the fallback base URL, the credential-optional behavior, and the
/// stable ids (`docs/14` §9 owner decision, 2026-10-23).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AdapterKind {
    /// First-party OpenAI (`https://api.openai.com/v1`, static key).
    OpenAi,
    /// Any OpenAI-compatible endpoint the user names: base URL required,
    /// key optional.
    Custom,
}

impl AdapterKind {
    /// The stable provider id (`docs/12` §8.2).
    pub fn id(self) -> &'static str {
        match self {
            AdapterKind::OpenAi => "openai",
            AdapterKind::Custom => "custom",
        }
    }

    /// The UI-facing provider name.
    pub fn display_name(self) -> &'static str {
        match self {
            AdapterKind::OpenAi => "OpenAI",
            AdapterKind::Custom => "Custom (OpenAI-compatible)",
        }
    }

    /// Whether an empty credential is a refusal (`openai`) or a keyless
    /// send (`custom` — some self-hosted servers answer without a key).
    pub fn credential_required(self) -> bool {
        matches!(self, AdapterKind::OpenAi)
    }
}

/// The bridge-and-wire vocabulary's status class for an HTTP status with no
/// parseable error envelope (401/403 auth, 429 quota, 404 missing model,
/// 5xx transient, everything else an invalid request).
pub(crate) fn status_class(status: u16) -> ErrorClass {
    match status {
        401 | 403 => ErrorClass::Auth,
        429 => ErrorClass::Quota,
        404 => ErrorClass::ModelNotFound,
        500..=599 => ErrorClass::Transient,
        _ => ErrorClass::InvalidRequest,
    }
}

/// Normalize a base URL for a family: trailing slashes off; the `openai`
/// family falls back to the first-party host when left empty, while the
/// `custom` family's emptiness stays empty (validated at send — the base
/// URL is that family's core field).
pub(crate) fn normalize_base(kind: AdapterKind, base_url: &str) -> String {
    let base = base_url.trim().trim_end_matches('/').to_string();
    if base.is_empty() && matches!(kind, AdapterKind::OpenAi) {
        return DEFAULT_OPENAI_BASE.to_string();
    }
    base
}

/// The OpenAI-compat adapter. Constructed per profile by the caller (the
/// bridge); [`reqwest::Client`] is injected so connection pooling is the
/// caller's decision and nothing here is process-global (`AI-ADAPTER-13`).
pub struct OpenAiCompatAdapter {
    config: OpenAiClientConfig,
    client: reqwest::Client,
}

impl OpenAiCompatAdapter {
    pub fn new(config: OpenAiClientConfig, client: reqwest::Client) -> Self {
        let mut config = config;
        config.base_url = normalize_base(config.kind, &config.base_url);
        OpenAiCompatAdapter { config, client }
    }

    /// The normalized base URL.
    pub fn base_url(&self) -> &str {
        &self.config.base_url
    }

    /// The shared request header set: the credential per the family's rule,
    /// and the profile's forwarded custom headers. The credential exists
    /// only as request-header material — never logged, never persisted,
    /// never copied into any other store (`AI-PROVIDER-03`).
    fn request_builder(
        &self,
        method: reqwest::Method,
        url: &str,
        auth: &AuthContext,
    ) -> reqwest::RequestBuilder {
        let mut builder = self.client.request(method, url);
        let credential = auth.credential().expose_secret();
        if !credential.trim().is_empty() {
            // `openai` authenticates with a static key; `custom` sends it
            // when present and stays keyless when not (`docs/14` §9).
            builder = builder.header("authorization", format!("Bearer {credential}"));
        }
        for (name, value) in &self.config.custom_headers {
            builder = builder.header(name, value);
        }
        builder
    }

    /// The `{base}/chat/completions` URL. The path is fixed — a request
    /// only ever goes to the configured endpoint (the `TEST-AI-022`
    /// reading: no arbitrary URLs).
    fn chat_url(&self) -> String {
        format!("{}/chat/completions", self.config.base_url)
    }

    /// A URL under this adapter's configured endpoint only.
    fn endpoint_url(&self, path: &str) -> String {
        format!("{}{}", self.config.base_url, path)
    }

    /// The structural gate every send passes first (the endpoint must exist
    /// and the id must be a legal model string). No family predicate
    /// screens ids here — the id the user named travels verbatim, and a
    /// substitution is a user-visible error instead (`AI-MODEL-01`;
    /// `docs/14` §9 owner decision, 2026-10-23).
    fn gate_model(&self, model: &ModelKey) -> Result<(), ProviderError> {
        if self.config.base_url.is_empty() {
            return Err(ProviderError {
                class: ErrorClass::Configuration,
                status: None,
                message: "the base URL is required before sending anything".to_string(),
            });
        }
        match myharness_config::forms::validate_open_model_id(&model.model_id) {
            Ok(()) => Ok(()),
            Err(reason) => Err(ProviderError {
                class: ErrorClass::InvalidRequest,
                status: None,
                message: format!("the requested model was not sent: {reason}"),
            }),
        }
    }

    /// Read a response's JSON, or turn a failure into the mapped provider
    /// error: the wire's error envelope first, bare status otherwise. The
    /// message keeps the provider's own wording — no raw bodies (`docs/18`
    /// §11).
    async fn response_json(response: reqwest::Response) -> Result<Value, ProviderError> {
        let status = response.status();
        if !status.is_success() {
            let status = status.as_u16();
            let body = response.json::<Value>().await.ok();
            return Err(match body {
                Some(body) => error_from_envelope(Some(status), &body),
                None => error_from_status(status),
            });
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
        let body = response.json::<Value>().await.ok();
        match body {
            Some(body) => error_from_envelope(Some(status), &body),
            None => error_from_status(status),
        }
    }

    /// Send non-streaming chat completions.
    async fn send_completion(
        &self,
        auth: &AuthContext,
        request: &myharness_provider_sdk::NativeTurnRequest,
    ) -> Result<Value, ProviderError> {
        let builder = self
            .request_builder(reqwest::Method::POST, &self.chat_url(), auth)
            .json(&chat_request_body(request)?);
        let response = self.send(builder).await?;
        Self::response_json(response).await
    }

    /// Translate a complete (non-streaming) completion into the typed event
    /// set (`docs/12` §8.2): content, reasoning, tool calls, usage, stop
    /// reason, done. Unknown block kinds are refused, never silently
    /// dropped (`AI-MM-01`); a missing stop reason is contract-malformed
    /// and no "end of turn" is invented.
    fn events_from_response(
        response: &Value,
        requested_model: &ModelKey,
    ) -> Result<Vec<TurnEvent>, ProviderError> {
        let completion = parse_completion(response)?;
        // The echo gate (prefix match — the recorded `docs/14` §9 reading):
        // a dated revision of the requested id stays legal; a different
        // model is a substitution, user-visible, never ridden through.
        if let Some(ran) = completion.model.as_deref() {
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
        let mut events = Vec::new();
        if let Some(text) = completion.text.filter(|text| !text.is_empty()) {
            events.push(TurnEvent::TextDelta { text });
        }
        if let Some(reasoning) = completion.reasoning.filter(|text| !text.is_empty()) {
            events.push(TurnEvent::ReasoningDelta {
                text: reasoning,
                signature: None,
            });
        }
        for call in completion.tool_calls {
            let name = call
                .name
                .filter(|name| !name.is_empty())
                .ok_or_else(|| ProviderError {
                    class: ErrorClass::InvalidRequest,
                    status: None,
                    message: "the completion carried a tool call without a function name; nothing was executed"
                        .to_string(),
                })?;
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
                id: call.id.unwrap_or_default(),
                name,
                arguments_delta: None,
                arguments: Some(arguments),
            });
        }
        if let Some(usage) = completion.usage {
            if usage != Usage::default() {
                events.push(TurnEvent::Usage { usage });
            }
        }
        match completion.finish_reason {
            Some(raw) => events.push(TurnEvent::Stop {
                reason: stop_reason(&raw),
            }),
            None => {
                return Err(ProviderError {
                    class: ErrorClass::InvalidRequest,
                    status: None,
                    message: "the completion carried no stop reason".to_string(),
                })
            }
        }
        events.push(TurnEvent::Done);
        Ok(events)
    }

    /// The one-shot inference probe (the setup window's *Test connection*;
    /// `docs/screens/setup-desktop-3p.md` — a real request, nothing
    /// persisted, no draft changed by the answer).
    ///
    /// Sends a 1-token non-streaming turn to the requested model. Reports
    /// exact usage and the native stop reason, returns provider error
    /// classes verbatim (`TEST-AI-021`), and refuses a model swap (the echo
    /// gate, prefix match). Latency is measured around the native request.
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
        let completion = parse_completion(&response)?;
        // The model echo gate applies to probes too: a performed model
        // outside the requested id's prefix is a refusal (`AI-MODEL-01`).
        let ran = completion
            .model
            .clone()
            .unwrap_or_else(|| model_id.to_string());
        if !model_echo_ok(model_id, &ran) {
            return Err(ProviderError {
                class: ErrorClass::ModelNotFound,
                status: None,
                message: format!(
                    "the provider performed model {ran:?} while the probe named {model_id:?}"
                ),
            });
        }
        Ok(ProbeStatus {
            ok: true,
            model: ran,
            stop_reason: completion.finish_reason.map(|raw| stop_reason(&raw).wire()),
            input_tokens: completion
                .usage
                .as_ref()
                .map(|usage| usage.input)
                .unwrap_or_default(),
            output_tokens: completion
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

/// One in-flight tool call whose arguments stream in as `function.arguments`
/// fragments (`AI-TOOL-01`: ids, names, and argument deltas preserved;
/// assembled arguments validated at close). A compatible server that omits
/// `id` on continuation fragments gets a stable synthesized id derived from
/// the completion id and this index (`docs/14` §9 owner decision).
#[derive(Debug, Clone)]
struct ToolBlockState {
    id: String,
    name: String,
    buffer: String,
}

/// The `TurnStream` implementation for streamed turns: parses the wire's
/// chunk frames into the app's typed events, guards the model echo (prefix
/// match), preserves usage and stop reasons, and enforces the idle window
/// (`TEST-AI-014/015/016`).
///
/// Dropping the stream cancels the native request: the `reqwest` response
/// (and its body task) drops with it, which closes the connection — the
/// caller keeps whatever was already emitted and marks it stopped
/// (`docs/12` §4.1).
pub(crate) struct TurnStreamImpl {
    inner: Pin<Box<dyn Stream<Item = reqwest::Result<bytes::Bytes>> + Send>>,
    parser: ChunkParser,
    requested_model: String,
    idle_timeout: Duration,
    /// The idle-window timer (re-armed on content progress; keep-alive
    /// frames are not progress).
    idle: Pin<Box<tokio::time::Sleep>>,
    /// Whether the idle window governs this stream at all (a `0` window
    /// means "no idle timeout"; the profile documents a nonzero default).
    idle_armed: bool,
    tools: BTreeMap<usize, ToolBlockState>,
    /// The completion id seen so far — the synthesized tool-id base.
    completion_id: Option<String>,
    usage: Usage,
    saw_usage: bool,
    stop: Option<NativeStopReason>,
    outbox: VecDeque<Result<TurnEvent, ProviderError>>,
    finished: bool,
    saw_terminated: bool,
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
        for parsed in self.parser.feed(chunk) {
            match parsed {
                Err(error) => {
                    // Malformed bytes/frames surface — nothing is silently
                    // skipped (`AI-MM-01`).
                    self.finished = true;
                    return Err(error);
                }
                Ok(event) => self.step(event)?,
            }
        }
        Ok(())
    }

    /// One parsed stream frame. Progress events re-arm the idle window;
    /// keep-alives never do (`TEST-AI-016`).
    fn step(&mut self, event: StreamEvent) -> Result<(), ProviderError> {
        match event {
            StreamEvent::Ping => Ok(()),
            StreamEvent::Terminated => {
                self.saw_terminated = true;
                // The final usage is the exact number the provider reported
                // (`TEST-AI-013`; nothing is estimated when nothing arrived),
                // then the preserved stop reason (`TEST-AI-014`), then the
                // typed end-of-stream.
                if self.saw_usage {
                    self.outbox.push_back(Ok(TurnEvent::Usage {
                        usage: self.usage.clone(),
                    }));
                }
                match self.stop.clone() {
                    Some(reason) => self.outbox.push_back(Ok(TurnEvent::Stop { reason })),
                    None => {
                        // A stream that ended without finishing its choices
                        // is contract-malformed; no reason is invented.
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
            StreamEvent::Delta {
                completion_id,
                model,
                delta,
                finish_reason,
                usage,
            } => {
                self.rearm_idle();
                // The echo gate: a chunk that names a model outside the
                // requested id's prefix stops the stream (`AI-MODEL-01`).
                if let Some(ran) = model.as_deref() {
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
                }
                if self.completion_id.is_none() {
                    self.completion_id = completion_id;
                }
                // Usage: last parse wins per field; the carry is exact
                // when the server sends it (some send it every chunk, some
                // never).
                if let Some(value) = usage.as_ref().and_then(parse_usage) {
                    self.saw_usage = true;
                    self.usage.input = value.input;
                    self.usage.output = value.output;
                    if value.cache_read.is_some() {
                        self.usage.cache_read = value.cache_read;
                    }
                }
                // The choice's delta lands first (it may even carry new
                // fragments), then this wire's per-call closure happens on
                // the finalizing chunk — the wire has no per-block stop
                // event, so the assembling close is `finish_reason`'s job.
                self.emit_delta(&delta)?;
                if let Some(raw) = finish_reason.as_deref() {
                    self.flush_open_tools()?;
                    self.stop = Some(stop_reason(raw));
                }
                Ok(())
            }
        }
    }

    /// Close every in-flight tool call with its assembled arguments. This
    /// wire has no `content_block_stop`; the strongest close signal it
    /// sends is the finalizing chunk's `finish_reason`, so the machine
    /// flushes there (an unterminated stream ends in an error regardless —
    /// nothing was dropped silently, nothing executed half-parsed).
    fn flush_open_tools(&mut self) -> Result<(), ProviderError> {
        for (_, state) in std::mem::take(&mut self.tools) {
            let arguments = if state.buffer.trim().is_empty() {
                json!({})
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

    /// Map one choice's `delta` object onto typed events.
    fn emit_delta(&mut self, delta: &Value) -> Result<(), ProviderError> {
        // content: a text fragment; absent/null are transport spelling,
        // empty strings carry no content to surface.
        if let Some(text) = delta
            .get("content")
            .and_then(Value::as_str)
            .filter(|text| !text.is_empty())
        {
            self.outbox.push_back(Ok(TurnEvent::TextDelta {
                text: text.to_string(),
            }));
        }
        // Reasoning: DeepSeek-compatible servers spell
        // `reasoning_content`; others spell `reasoning` — read both, never
        // guessing beyond either (`docs/14` §9 per-wire readings).
        let reasoning = delta
            .get("reasoning_content")
            .and_then(Value::as_str)
            .or_else(|| delta.get("reasoning").and_then(Value::as_str));
        if let Some(thinking) = reasoning.filter(|text| !text.is_empty()) {
            self.outbox.push_back(Ok(TurnEvent::ReasoningDelta {
                text: thinking.to_string(),
                signature: None,
            }));
        }
        // Tool calls: fragments carry {index, id?, function.name?,
        // function.arguments?}.
        if let Some(calls) = delta.get("tool_calls").and_then(Value::as_array) {
            for call in calls {
                self.emit_tool_fragment(call)?;
            }
        }
        Ok(())
    }

    /// Map one streamed tool-call fragment.
    fn emit_tool_fragment(&mut self, call: &Value) -> Result<(), ProviderError> {
        let index = call.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
        let function = call.get("function").cloned().unwrap_or_else(|| json!({}));
        let id = call
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .filter(|id| !id.is_empty());
        let name = function
            .get("name")
            .and_then(Value::as_str)
            .map(str::to_string)
            .filter(|name| !name.is_empty());

        // A new call registers itself: an explicit id, or (when a compatible
        // server omits it) the recorded synthesized id `chunk.id + "-{index}"`.
        if let Some(state) = self.tools.get(&index) {
            if let Some(id) = id.as_ref() {
                if state.id != *id {
                    // The server re-used this index for a different call
                    // without finishing the one under it: a wire contract
                    // miss, named and refused (`AI-MM-01`).
                    self.finished = true;
                    return Err(ProviderError {
                        class: ErrorClass::InvalidRequest,
                        status: None,
                        message: format!(
                            "the stream re-used tool-call index {index} for a different id ({id:?} after {:?})",
                            state.id
                        ),
                    });
                }
            }
            if let Some(name) = name {
                let state = self.tools.get_mut(&index).unwrap();
                if state.name.is_empty() {
                    state.name = name.clone();
                    self.outbox.push_back(Ok(TurnEvent::ToolCall {
                        id: state.id.clone(),
                        name,
                        arguments_delta: None,
                        arguments: None,
                    }));
                }
            }
        } else {
            let id = id.unwrap_or_else(|| self.synthetic_tool_id(index));
            self.tools.insert(
                index,
                ToolBlockState {
                    id: id.clone(),
                    name: name.clone().unwrap_or_default(),
                    buffer: String::new(),
                },
            );
            self.outbox.push_back(Ok(TurnEvent::ToolCall {
                id,
                name: name.unwrap_or_default(),
                arguments_delta: None,
                arguments: None,
            }));
        }

        // Arguments: a fragment of the call's JSON, surfaced as-is
        // (`AI-ADAPTER-11`: argument data is not re-routed or rewritten).
        if let Some(fragment) = function
            .get("arguments")
            .and_then(Value::as_str)
            .filter(|text| !text.is_empty())
        {
            let state = self.tools.get_mut(&index).unwrap();
            state.buffer.push_str(fragment);
            self.outbox.push_back(Ok(TurnEvent::ToolCall {
                id: state.id.clone(),
                name: state.name.clone(),
                arguments_delta: Some(fragment.to_string()),
                arguments: None,
            }));
        }
        Ok(())
    }

    /// The recorded synthesized id for a continuation fragment that always
    /// omits `id`: `chunk.id + "-{index}"`, falling back to a stable
    /// `call-{index}` when no completion id is in sight (`docs/14` §9).
    fn synthetic_tool_id(&self, index: usize) -> String {
        match self.completion_id.as_deref() {
            Some(completion_id) => format!("{completion_id}-{index}"),
            None => format!("call-{index}"),
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
                    if this.saw_terminated {
                        return Poll::Ready(None);
                    }
                    return Poll::Ready(Some(Err(ProviderError {
                        class: ErrorClass::Unreachable,
                        status: None,
                        message: "the stream ended before the [DONE] sentinel".to_string(),
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

/// Turn a successful streaming response into the typed [`TurnStream`]: the
/// bytes stream feeds the chunk parser, the state machine guards the model
/// echo and the idle window, and dropping the returned stream cancels the
/// native request (`TEST-AI-015`).
fn finish_stream_into_turn_stream(
    response: reqwest::Response,
    requested_model: String,
    idle_timeout: Duration,
) -> TurnStream {
    // A zero-sized window means "no idle timeout" — the profile documents a
    // nonzero default (`docs/20`: `inferenceStreamIdleTimeoutSec`); this is
    // the one deliberate off-switch.
    let idle_armed = !idle_timeout.is_zero();
    Box::pin(TurnStreamImpl {
        inner: Box::pin(response.bytes_stream()),
        parser: ChunkParser::new(),
        requested_model,
        idle_timeout,
        idle: Box::pin(tokio::time::sleep(idle_timeout)),
        idle_armed,
        tools: BTreeMap::new(),
        completion_id: None,
        usage: Usage::default(),
        saw_usage: false,
        stop: None,
        outbox: VecDeque::new(),
        finished: false,
        saw_terminated: false,
    })
}

#[async_trait::async_trait]
impl ProviderAdapter for OpenAiCompatAdapter {
    fn id(&self) -> &str {
        self.config.kind.id()
    }

    fn protocol(&self) -> ProtocolFamily {
        ProtocolFamily::OpenAiChat
    }

    fn display_name(&self) -> &str {
        self.config.kind.display_name()
    }

    /// A shape check with no network: the `openai` family refuses an empty
    /// credential outright; the `custom` family accepts an empty one —
    /// keyless servers are the family's core case (`docs/14` §9 owner
    /// decision).
    async fn authenticate(&self, auth: &AuthContext) -> Result<(), ProviderError> {
        let credential = auth.credential().expose_secret();
        if credential.trim().is_empty() {
            if self.config.kind.credential_required() {
                return Err(ProviderError {
                    class: ErrorClass::Auth,
                    status: None,
                    message: "the credential is empty; nothing was sent".to_string(),
                });
            }
            return Ok(());
        }
        Ok(())
    }

    /// The native listing: single-page `GET {base}/models`, rows mapped to
    /// descriptors (the open model layer's discovery; `docs/10` §9 assigns
    /// discovery to `providers`).
    async fn list_models(&self, auth: &AuthContext) -> Result<Vec<ModelDescriptor>, ProviderError> {
        if self.config.base_url.is_empty() {
            return Err(ProviderError {
                class: ErrorClass::Configuration,
                status: None,
                message: "the base URL is required before listing models".to_string(),
            });
        }
        // Keyless for `custom`: an empty credential resolves to no header
        // inside `discover_with`; per-request resolution keeps rotation
        // (`AI-PROVIDER-03`).
        let credential = Some(auth.credential().expose_secret().to_string());
        let outcome = discover_with(
            self.config.kind,
            &self.config.base_url,
            credential,
            &self.config.custom_headers,
            &self.client,
            crate::discovery::OPENAI_DISCOVERY_TIMEOUT.as_secs(),
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

    /// Build the native request and open the chunk stream. A non-streaming
    /// request becomes a buffered event list from the same contract.
    async fn create_turn(
        &self,
        auth: &AuthContext,
        request: &myharness_provider_sdk::NativeTurnRequest,
    ) -> Result<TurnStream, ProviderError> {
        self.gate_model(&request.model)?;
        self.authenticate(auth).await?;
        if request.stream {
            let builder = self
                .request_builder(reqwest::Method::POST, &self.chat_url(), auth)
                .json(&chat_request_body(request)?);
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
            let response = self.send_completion(auth, request).await?;
            let events = Self::events_from_response(&response, &request.model)?;
            Ok(Box::pin(futures::stream::iter(events.into_iter().map(Ok))))
        }
    }

    /// The wire offers no native count endpoint: an explicit refusal, never
    /// an estimate (`AI-ADAPTER-09`).
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
                "the OpenAI-compatible surface offers no native token-count endpoint for {:?}; no count was produced and none was estimated",
                request.model.model_id
            ),
        })
    }

    /// `GET {base}/models?limit=1` as the reachability check (`docs/12` §8.2
    /// `healthCheck`). No inference happens here.
    async fn health_check(&self, auth: &AuthContext) -> Result<ProviderHealth, ProviderError> {
        if self.config.base_url.is_empty() {
            return Ok(ProviderHealth {
                reachable: false,
                authenticated: None,
                detail: Some("the base URL is not configured".to_string()),
            });
        }
        let builder = self.request_builder(
            reqwest::Method::GET,
            &self.endpoint_url("/models?limit=1"),
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

// Unit tests for the body builder and the chunk parser live beside their
// modules' contracts: `tests/wire.rs` runs the wire battery against a mock
// endpoint (the same fixture pattern as the sibling adapters).
