//! The [`ResponsesAdapter`]: the one place in the app that talks the OpenAI
//! **Responses** wire (Phase 8's reference extension; `docs/13` §"Phase 8").
//! It implements the [`myharness_provider_sdk::ProviderAdapter`] contract
//! and adds the one-shot inference probe the setup window's *Test
//! connection* runs (`docs/screens/setup-desktop-3p.md`).
//!
//! The sibling crate (`myharness-provider-openai`) holds the Chat
//! Completions wall; this one holds the Responses wall. They share a design
//! — the adapter-contract posture, the echo gate, the idle window — and
//! deliberately no code: each wire's grammar is its own, and transport
//! plumbing that drifts between walls would become a hidden translation
//! layer (`AI-PROVIDER-02`, `AI-EXT-02`).
//!
//! Boundaries (identical posture to every wall, `docs/12` §11): credentials
//! arrive resolved in an [`AuthContext`] and exist only as request-header
//! material (`AI-PROVIDER-03`); a concrete [`reqwest::Client`] is injected
//! per adapter, so nothing here is process-global (`AI-ADAPTER-13`); model
//! ids travel verbatim, ungated — the only identity checks are structural
//! (`docs/14` §9 owner decision, 2026-10-23).

use std::collections::BTreeMap;
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
use serde_json::{json, Value};

use crate::discovery::{descriptor_from_row, discover_with};
use crate::responses::{
    error_from_envelope, error_from_status, event_disposition, model_echo_ok, parse_response,
    parse_usage, responses_request_body, terminal_stop_reason, Disposition, EventParser,
    ProbeStatus, ResponsesClientConfig, WireEvent,
};
use crate::DEFAULT_OPENAI_BASE;

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

/// Normalize the base URL: trailing slashes off; empty falls back to the
/// first-party host (this family is first-party OpenAI only — there is no
/// user-defined variant of the Responses wire on v1; `docs/14` §9).
pub(crate) fn normalize_base(base_url: &str) -> String {
    let base = base_url.trim().trim_end_matches('/').to_string();
    if base.is_empty() {
        return DEFAULT_OPENAI_BASE.to_string();
    }
    base
}

/// The OpenAI Responses adapter. Constructed per profile by the caller (the
/// bridge); [`reqwest::Client`] is injected so connection pooling is the
/// caller's decision and nothing here is process-global (`AI-ADAPTER-13`).
pub struct ResponsesAdapter {
    config: ResponsesClientConfig,
    client: reqwest::Client,
}

impl ResponsesAdapter {
    pub fn new(config: ResponsesClientConfig, client: reqwest::Client) -> Self {
        let mut config = config;
        config.base_url = normalize_base(&config.base_url);
        ResponsesAdapter { config, client }
    }

    /// The normalized base URL.
    pub fn base_url(&self) -> &str {
        &self.config.base_url
    }

    /// The shared request header set: the static key (this family refuses an
    /// empty credential before anything is sent — `authenticate` is the
    /// gate), and the profile's forwarded custom headers. The credential
    /// exists only as request-header material — never logged, never
    /// persisted, never copied into any other store (`AI-PROVIDER-03`).
    fn request_builder(
        &self,
        method: reqwest::Method,
        url: &str,
        auth: &AuthContext,
    ) -> reqwest::RequestBuilder {
        let mut builder = self.client.request(method, url).header(
            "authorization",
            format!("Bearer {}", auth.credential().expose_secret()),
        );
        for (name, value) in &self.config.custom_headers {
            builder = builder.header(name, value);
        }
        builder
    }

    /// The `{base}/responses` URL. The path is fixed — a request only ever
    /// goes to the configured endpoint (no arbitrary URLs).
    fn responses_url(&self) -> String {
        format!("{}/responses", self.config.base_url)
    }

    /// A URL under this adapter's configured endpoint only.
    fn endpoint_url(&self, path: &str) -> String {
        format!("{}{}", self.config.base_url, path)
    }

    /// The structural gate every send passes first: the id must be a legal
    /// model string. No family predicate screens ids — the id the user
    /// named travels verbatim, and a substitution is a user-visible error
    /// instead (`AI-MODEL-01`; `docs/14` §9 owner decision, 2026-10-23).
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

    /// Send non-streaming Responses.
    async fn send_response_object(
        &self,
        auth: &AuthContext,
        request: &myharness_provider_sdk::NativeTurnRequest,
    ) -> Result<Value, ProviderError> {
        let builder = self
            .request_builder(reqwest::Method::POST, &self.responses_url(), auth)
            .json(&responses_request_body(request)?);
        let response = self.send(builder).await?;
        Self::response_json(response).await
    }

    /// Translate a completed (non-streaming) response object into the typed
    /// event set (`docs/12` §8.2). The echo gate applies; annotations
    /// surface as citations; a missing terminal status is
    /// contract-malformed and no "end of turn" is invented.
    fn events_from_response(
        body: &Value,
        requested_model: &ModelKey,
    ) -> Result<Vec<TurnEvent>, ProviderError> {
        let parsed = parse_response(body)?;
        if let Some(error) = &parsed.error {
            return Err(error_from_envelope(None, error));
        }
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
        let had_function_calls = !parsed.function_calls.is_empty();
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
        for citation in parsed.citations {
            events.push(TurnEvent::Citation { citation });
        }
        for call in parsed.function_calls {
            let name = call
                .name
                .filter(|name| !name.is_empty())
                .ok_or_else(|| ProviderError {
                    class: ErrorClass::InvalidRequest,
                    status: None,
                    message:
                        "the response carried a function call without a name; nothing was executed"
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
                id: call.call_id.unwrap_or_default(),
                name,
                arguments_delta: None,
                arguments: Some(arguments),
            });
        }
        if let Some(usage) = parsed.usage {
            if usage != Usage::default() {
                events.push(TurnEvent::Usage { usage });
            }
        }
        let status = parsed.status.as_deref().ok_or_else(|| ProviderError {
            class: ErrorClass::InvalidRequest,
            status: None,
            message: "the response object carried no terminal status".to_string(),
        })?;
        if status == "failed" {
            return Err(ProviderError {
                class: ErrorClass::Transient,
                status: None,
                message: "the response object reports status \"failed\" without an error body"
                    .to_string(),
            });
        }
        events.push(TurnEvent::Stop {
            reason: terminal_stop_reason(
                status,
                parsed.incomplete_reason.as_deref(),
                had_function_calls,
            ),
        });
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
        let response = self.send_response_object(auth, &request).await?;
        let latency = started.elapsed();
        let parsed = parse_response(&response)?;
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
        let status = parsed
            .status
            .clone()
            .unwrap_or_else(|| "completed".to_string());
        Ok(ProbeStatus {
            ok: true,
            model: ran,
            stop_reason: Some(
                terminal_stop_reason(&status, parsed.incomplete_reason.as_deref(), false).wire(),
            ),
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

/// One in-flight function call, keyed by the wire's `item_id`: announced by
/// `response.output_item.added`, its arguments streamed by
/// `response.function_call_arguments.delta`, closed by
/// `response.output_item.done` with the assembled arguments (`AI-TOOL-01`).
#[derive(Debug, Clone)]
struct ToolBlockState {
    call_id: String,
    name: String,
    buffer: String,
}

/// The `TurnStream` implementation for streamed turns: parses the wire's
/// event frames into the app's typed events, guards the model echo (prefix
/// match), preserves usage and stop reasons, and enforces the idle window
/// (`TEST-AI-014/015/016`).
///
/// Unlike the Chat Completions wall there is no `[DONE]` sentinel: the
/// terminal events are `response.completed` / `response.failed` / `error`,
/// and a stream that ends without one is malformed — no stop reason is
/// invented.
///
/// Dropping the stream cancels the native request: the `reqwest` response
/// (and its body task) drops with it, which closes the connection — the
/// caller keeps whatever was already emitted and marks it stopped
/// (`docs/12` §4.1).
pub(crate) struct TurnStreamImpl {
    inner: Pin<Box<dyn Stream<Item = reqwest::Result<bytes::Bytes>> + Send>>,
    parser: EventParser,
    requested_model: String,
    idle_timeout: Duration,
    /// The idle-window timer (re-armed on wire progress; empty/comment
    /// frames are not progress).
    idle: Pin<Box<tokio::time::Sleep>>,
    /// Whether the idle window governs this stream at all (a `0` window
    /// means "no idle timeout"; the profile documents a nonzero default).
    idle_armed: bool,
    tools: BTreeMap<String, ToolBlockState>,
    usage: Usage,
    saw_usage: bool,
    /// Whether the turn emitted (or began) a function call — the terminal
    /// mapping's `tool_use` fact on this wire.
    emitted_function_calls: bool,
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

    /// Absorb one byte chunk; append zero or more typed events to the
    /// outbox, or terminate the stream on a malformed/error frame.
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

    /// One parsed wire frame. Any frame carrying real bytes re-arms the
    /// idle window; empty/comment frames never do (`TEST-AI-016`).
    fn step(&mut self, event: WireEvent) -> Result<(), ProviderError> {
        if event.data.trim().is_empty() {
            return Ok(());
        }
        self.rearm_idle();
        let body: Value = serde_json::from_str(event.data.trim()).map_err(|error| {
            self.finished = true;
            ProviderError {
                class: ErrorClass::InvalidRequest,
                status: None,
                message: format!("the stream carried a frame that is not valid JSON: {error}"),
            }
        })?;
        // Dispatch on the payload's own `type`; the `event:` line is the
        // fallback spelling (the payload is the authoritative shape).
        let event_type = body
            .get("type")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or(event.event)
            .unwrap_or_default();
        match event_disposition(&event_type) {
            Disposition::Noop => Ok(()),
            Disposition::Unknown => {
                self.finished = true;
                Err(ProviderError {
                    class: ErrorClass::InvalidRequest,
                    status: None,
                    message: format!(
                        "the stream carried an undocumented event type {event_type:?}; no content was dropped silently"
                    ),
                })
            }
            Disposition::Map => self.map_event(&event_type, &body),
        }
    }

    /// Map one dispatchable event onto typed events.
    fn map_event(&mut self, event_type: &str, body: &Value) -> Result<(), ProviderError> {
        match event_type {
            "response.created" => {
                let response = body.get("response").cloned().unwrap_or_else(|| json!({}));
                if let Some(ran) = response.get("model").and_then(Value::as_str) {
                    // The echo gate (`AI-MODEL-01`).
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
                Ok(())
            }
            "response.output_text.delta" => {
                if let Some(text) = body.get("delta").and_then(Value::as_str) {
                    if !text.is_empty() {
                        self.outbox.push_back(Ok(TurnEvent::TextDelta {
                            text: text.to_string(),
                        }));
                    }
                }
                Ok(())
            }
            // A refusal streams as text too — it is model output, surfaced
            // as such; the stop reason stays the wire's own.
            "response.refusal.delta" => {
                if let Some(text) = body.get("delta").and_then(Value::as_str) {
                    if !text.is_empty() {
                        self.outbox.push_back(Ok(TurnEvent::TextDelta {
                            text: text.to_string(),
                        }));
                    }
                }
                Ok(())
            }
            "response.reasoning_summary_text.delta" => {
                if let Some(text) = body.get("delta").and_then(Value::as_str) {
                    if !text.is_empty() {
                        self.outbox.push_back(Ok(TurnEvent::ReasoningDelta {
                            text: text.to_string(),
                            signature: None,
                        }));
                    }
                }
                Ok(())
            }
            "response.output_text.annotation.added" => {
                if let Some(annotation) = body.get("annotation") {
                    self.outbox.push_back(Ok(TurnEvent::Citation {
                        citation: annotation.clone(),
                    }));
                }
                Ok(())
            }
            "response.output_item.added" => {
                let item = body.get("item").cloned().unwrap_or_else(|| json!({}));
                if item.get("type").and_then(Value::as_str) == Some("function_call") {
                    self.emitted_function_calls = true;
                    // The announcement carries the item's id inside the item
                    // object (the wire's spelling); the argument deltas key
                    // on the top-level `item_id`.
                    let item_id = item
                        .get("id")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .unwrap_or_default();
                    let call_id = item
                        .get("call_id")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .unwrap_or_else(|| item_id.clone());
                    let name = item
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    self.tools.insert(
                        item_id.clone(),
                        ToolBlockState {
                            call_id: call_id.clone(),
                            name: name.clone(),
                            buffer: String::new(),
                        },
                    );
                    self.outbox.push_back(Ok(TurnEvent::ToolCall {
                        id: call_id,
                        name,
                        arguments_delta: None,
                        arguments: None,
                    }));
                }
                Ok(())
            }
            "response.function_call_arguments.delta" => {
                let item_id = body
                    .get("item_id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                if let Some(state) = self.tools.get_mut(&item_id) {
                    if let Some(fragment) = body.get("delta").and_then(Value::as_str) {
                        if !fragment.is_empty() {
                            state.buffer.push_str(fragment);
                            self.outbox.push_back(Ok(TurnEvent::ToolCall {
                                id: state.call_id.clone(),
                                name: state.name.clone(),
                                arguments_delta: Some(fragment.to_string()),
                                arguments: None,
                            }));
                        }
                    }
                    Ok(())
                } else {
                    self.finished = true;
                    Err(ProviderError {
                        class: ErrorClass::InvalidRequest,
                        status: None,
                        message: format!(
                            "the stream streamed arguments for item {item_id:?} that was never announced"
                        ),
                    })
                }
            }
            "response.output_item.done" => {
                let item = body.get("item").cloned().unwrap_or_else(|| json!({}));
                match item.get("type").and_then(Value::as_str) {
                    Some("function_call") => self.close_tool_call(&item),
                    Some("message") => {
                        // The accumulated text already streamed; the
                        // annotations have not — surface them verbatim
                        // (`AI-STREAM-01`).
                        for part in item
                            .get("content")
                            .and_then(Value::as_array)
                            .cloned()
                            .unwrap_or_default()
                        {
                            if part.get("type").and_then(Value::as_str) == Some("output_text") {
                                if let Some(annotations) =
                                    part.get("annotations").and_then(Value::as_array)
                                {
                                    for annotation in annotations {
                                        self.outbox.push_back(Ok(TurnEvent::Citation {
                                            citation: annotation.clone(),
                                        }));
                                    }
                                }
                            }
                        }
                        Ok(())
                    }
                    // Documented items with no app model (reasoning) close
                    // as no-ops; an unknown item type is refused.
                    Some("reasoning") => Ok(()),
                    other => {
                        self.finished = true;
                        Err(ProviderError {
                            class: ErrorClass::InvalidRequest,
                            status: None,
                            message: format!(
                                "the stream closed an output item of type {other:?} that this adapter does not model"
                            ),
                        })
                    }
                }
            }
            "response.completed" => {
                let response = body.get("response").cloned().unwrap_or_else(|| json!({}));
                if let Some(value) = response.get("usage").and_then(parse_usage) {
                    self.saw_usage = true;
                    self.usage = value;
                }
                let status = response
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("completed");
                let incomplete_reason = response
                    .get("incomplete_details")
                    .and_then(|details| details.get("reason"))
                    .and_then(Value::as_str)
                    .map(str::to_string);
                self.finish_with(terminal_stop_reason(
                    status,
                    incomplete_reason.as_deref(),
                    self.emitted_function_calls,
                ));
                Ok(())
            }
            "response.failed" => {
                let response = body.get("response").cloned().unwrap_or_else(|| json!({}));
                self.finished = true;
                Err(
                    match response.get("error").filter(|error| !error.is_null()) {
                        Some(error) => error_from_envelope(None, error),
                        None => ProviderError {
                            class: ErrorClass::Transient,
                            status: None,
                            message: "the stream reported a failed response without an error body"
                                .to_string(),
                        },
                    },
                )
            }
            "error" => {
                self.finished = true;
                Err(error_from_envelope(None, body))
            }
            // The disposition table is the single source of what maps; this
            // arm is unreachable unless the table and the matcher drift.
            _ => Ok(()),
        }
    }

    /// Close one announced function call with its assembled arguments
    /// (`AI-TOOL-01`: assembled arguments validated at close; an invalid
    /// buffer is a visible error and nothing executes half-parsed).
    fn close_tool_call(&mut self, item: &Value) -> Result<(), ProviderError> {
        let item_id = item
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| {
                item.get("call_id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .unwrap_or_default();
        let state = self.tools.remove(&item_id);
        let Some(state) = state else {
            // A done item the stream never announced: use the item's own
            // fields (some servers send complete done items unannounced).
            let call_id = item
                .get("call_id")
                .and_then(Value::as_str)
                .unwrap_or(&item_id)
                .to_string();
            let name = item
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let buffer = item
                .get("arguments")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            self.emitted_function_calls = true;
            return self.flush_tool(call_id, name, buffer);
        };
        // The done item carries the final arguments string; prefer it over
        // the buffer (identical in practice, authoritative by contract).
        let buffer = item
            .get("arguments")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or(state.buffer);
        self.flush_tool(state.call_id, state.name, buffer)
    }

    fn flush_tool(
        &mut self,
        call_id: String,
        name: String,
        buffer: String,
    ) -> Result<(), ProviderError> {
        let arguments = if buffer.trim().is_empty() {
            json!({})
        } else {
            serde_json::from_str(&buffer).map_err(|error| {
                self.finished = true;
                ProviderError {
                    class: ErrorClass::InvalidRequest,
                    status: None,
                    message: format!(
                        "the tool arguments for {name:?} were not valid JSON; nothing was executed: {error}"
                    ),
                }
            })?
        };
        self.outbox.push_back(Ok(TurnEvent::ToolCall {
            id: call_id,
            name,
            arguments_delta: None,
            arguments: Some(arguments),
        }));
        Ok(())
    }

    /// Emit the exact usage the provider reported (nothing estimated when
    /// nothing arrived), the preserved stop reason, and the typed
    /// end-of-stream (`TEST-AI-013/014`).
    fn finish_with(&mut self, reason: NativeStopReason) {
        self.saw_terminal = true;
        if self.saw_usage {
            self.outbox.push_back(Ok(TurnEvent::Usage {
                usage: self.usage.clone(),
            }));
        }
        self.outbox.push_back(Ok(TurnEvent::Stop { reason }));
        self.outbox.push_back(Ok(TurnEvent::Done));
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
                    if this.saw_terminal {
                        return Poll::Ready(None);
                    }
                    return Poll::Ready(Some(Err(ProviderError {
                        class: ErrorClass::Unreachable,
                        status: None,
                        message:
                            "the stream ended before a terminal event (response.completed, response.failed, or error)"
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

/// Turn a successful streaming response into the typed [`TurnStream`]: the
/// bytes stream feeds the event parser, the state machine guards the model
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
        parser: EventParser::new(),
        requested_model,
        idle_timeout,
        idle: Box::pin(tokio::time::sleep(idle_timeout)),
        idle_armed,
        tools: BTreeMap::new(),
        usage: Usage::default(),
        saw_usage: false,
        emitted_function_calls: false,
        outbox: std::collections::VecDeque::new(),
        finished: false,
        saw_terminal: false,
    })
}

#[async_trait::async_trait]
impl ProviderAdapter for ResponsesAdapter {
    fn id(&self) -> &str {
        "openai-responses"
    }

    fn protocol(&self) -> ProtocolFamily {
        ProtocolFamily::OpenAiResponses
    }

    fn display_name(&self) -> &str {
        "OpenAI Responses"
    }

    /// A shape check with no network: this family is first-party OpenAI —
    /// an empty credential refuses outright before anything is sent.
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

    /// The native listing: single-page `GET {base}/models` (the platform's
    /// shared model listing surface), rows mapped to descriptors (`docs/10`
    /// §9 assigns discovery to `providers`; `TEST-AI-018`).
    async fn list_models(&self, auth: &AuthContext) -> Result<Vec<ModelDescriptor>, ProviderError> {
        let credential = auth.credential().expose_secret().to_string();
        let outcome = discover_with(
            &self.config.base_url,
            credential,
            &self.config.custom_headers,
            &self.client,
            crate::discovery::RESPONSES_DISCOVERY_TIMEOUT.as_secs(),
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
        if request.stream {
            let builder = self
                .request_builder(reqwest::Method::POST, &self.responses_url(), auth)
                .json(&responses_request_body(request)?);
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
            let response = self.send_response_object(auth, request).await?;
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
                "the Responses surface offers no native token-count endpoint for {:?}; no count was produced and none was estimated",
                request.model.model_id
            ),
        })
    }

    /// `GET {base}/models?limit=1` as the reachability check (`docs/12` §8.2
    /// `healthCheck`). No inference happens here.
    async fn health_check(&self, auth: &AuthContext) -> Result<ProviderHealth, ProviderError> {
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
