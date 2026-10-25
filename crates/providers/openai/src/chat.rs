//! The OpenAI Chat Completions wire contract — `POST {base}/chat/completions`
//! and the chunked stream shape surrounding it (`docs/13` §C open model
//! layer; owner decision recorded 2026-10-23 in `docs/14` §9).
//!
//! The request builder turns the app's typed conversation model
//! ([`myharness_provider_sdk::NativeTurnRequest`]) into this wire's body —
//! with the v1 compat readings recorded in the crate header (system joins
//! into one `role: "system"` message; `max_tokens`, not
//! `max_completion_tokens`; no `stream_options`; dropped `top_k`, thinking,
//! betas, and `cache_control`). The response side parses the wire's chunk
//! stream (`data: {…}` frames and the `data: [DONE]` sentinel) into app
//! events.
//!
//! Deliberate readings, recorded (`docs/14` §9):
//!
//! - **Model echo = prefix match.** A chunk/response naming
//!   `gpt-4o-mini-2024-07-18` for a request that named `gpt-4o-mini` is a
//!   dated revision of the requested model and stays legal; a name outside
//!   the requested id's prefix is a substitution and stops the turn
//!   (`AI-MODEL-01`, `AI-ADAPTER-11`: no silent swap, no invented shim).
//! - **Stop reasons.** This wire's `finish_reason` spellings map onto the
//!   app's preserved set: `stop`→`end_turn`, `length`→`max_tokens`,
//!   `tool_calls`/`function_call`→`tool_use`, `content_filter`→
//!   `content_filter`, anything else verbatim (`AI-ADAPTER-08`).
//! - **Usage.** `prompt_tokens`/`completion_tokens` map 1:1;
//!   `prompt_tokens_details.cached_tokens` maps to cache-read. Absent usage
//!   stays absent, never zero masquerading (`AI-USAGE-01`).
//! - **Stream termination.** The wire's sentinel is `data: [DONE]`. A stream
//!   that ends without it, or whose chunks carry no `finish_reason`, is
//!   contract-malformed: no stop reason is invented (`docs/01` fail-closed
//!   rule — the same reading the Anthropic-shaped adapters apply).

use std::time::Duration;

use myharness_provider_sdk::{
    ContentPart, ErrorClass, NativeStopReason, ProviderError, Role, SystemBlock, ToolSpec, Usage,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// The documented default idle timeout (the setup profile's
/// `inferenceStreamIdleTimeoutSec` default — `docs/20`; same window the
/// Anthropic-shaped families default to).
pub const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(300);

/// The one-shot inference probe's verdict (the setup window's *Test
/// connection*). `ok: true` means the endpoint took the request and answered
/// for the requested model; errors surface as `ProviderError` instead.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeStatus {
    pub ok: bool,
    /// The model the endpoint says it ran (== the requested id, modulo a
    /// dated revision, else error).
    pub model: String,
    /// The native stop reason, preserved (`TEST-AI-014`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    /// Exact tokens as the provider reported (`AI-USAGE-01`).
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// Wall time around the native request, in milliseconds.
    pub latency_ms: u64,
}

/// The client configuration, read from the profile's connection section by
/// the caller. The adapter owns only the wire policy.
#[derive(Debug, Clone, PartialEq)]
pub struct OpenAiClientConfig {
    /// Which family this adapter instance serves (`openai` or `custom`);
    /// it decides the fallback base URL and whether an empty credential is
    /// an auth failure or a keyless send (`docs/14` §9 owner decision).
    pub kind: crate::adapter::AdapterKind,
    /// The base URL. Trailing slashes are normalized; `openai` with an empty
    /// base URL falls back to the first-party host, `custom` refuses.
    pub base_url: String,
    /// The idle window for streamed turns.
    pub idle_timeout: Duration,
    /// Extra headers the profile forwards (tenant/routing material).
    pub custom_headers: Vec<(String, String)>,
}

// ---------------------------------------------------------------------------
// The request side: the body
// ---------------------------------------------------------------------------

/// One system block set, natively: this wire takes a single system
/// *message*, so block texts concatenate with `\n\n` (`docs/14` §9 v1
/// reading — one `role: "system"` message; `cache_control` has no spelling
/// on this wire and is dropped, recorded).
fn system_content(blocks: &[SystemBlock]) -> String {
    blocks
        .iter()
        .map(|block| block.text.as_str())
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// One tool, natively (`AI-TOOL-01`: name, description, and the JSON Schema
/// of the arguments travel exactly; `cache_control` has no spelling here and
/// is dropped, recorded).
fn tool(spec: &ToolSpec) -> Value {
    let mut function = json!({
        "name": spec.name,
        "parameters": spec.input_schema,
    });
    if let Some(description) = &spec.description {
        function["description"] = json!(description);
    }
    json!({ "type": "function", "function": function })
}

/// The stable internal tag used in refusal messages (never a wire spelling).
fn part_kind(part: &ContentPart) -> &'static str {
    match part {
        ContentPart::Text { .. } => "text",
        ContentPart::Image { .. } => "image",
        ContentPart::Document { .. } => "document",
        ContentPart::ToolUse { .. } => "tool_use",
        ContentPart::ToolResult { .. } => "tool_result",
        ContentPart::Reasoning { .. } => "reasoning",
        ContentPart::RedactedThinking { .. } => "redacted_thinking",
    }
}

/// One user content part — text or image (`image_url` data URI, the
/// recorded v1 mapping). A `Document` part has no spelling on this wire:
/// the refusal is explicit and upstream of any bytes (`AI-MM-01`).
fn user_part(part: &ContentPart) -> Result<Option<Value>, ProviderError> {
    match part {
        ContentPart::Text { text } => Ok(Some(json!({ "type": "text", "text": text }))),
        ContentPart::Image { media_type, data } => Ok(Some(json!({
            "type": "image_url",
            "image_url": { "url": format!("data:{media_type};base64,{data}") },
        }))),
        ContentPart::Document { .. } => Err(ProviderError {
            class: ErrorClass::UnsupportedModality,
            status: None,
            message: "the OpenAI-compatible wire cannot carry a document; split the PDF into images or text and retry"
                .to_string(),
        }),
        other => Err(ProviderError {
            class: ErrorClass::UnsupportedModality,
            status: None,
            message: format!(
                "this wire cannot carry a content part of kind {:?} on a user turn",
                part_kind(other)
            ),
        }),
    }
}

/// One tool result, natively: its own `role: "tool"` message keyed to the
/// issuing call. Tool-result images and documents are refused on v1
/// (recorded; the strictest-compatible content form is text-only tool
/// messages); `is_error` has no spelling here and is dropped — the result
/// text itself carries the failure to the model.
fn tool_message(part: &ContentPart) -> Result<Option<Value>, ProviderError> {
    let ContentPart::ToolResult {
        tool_use_id,
        content,
        ..
    } = part
    else {
        return Ok(None);
    };
    let mut pieces = Vec::new();
    for inner in content {
        match inner {
            ContentPart::Text { text } => pieces.push(text.clone()),
            other => {
                return Err(ProviderError {
                    class: ErrorClass::UnsupportedModality,
                    status: None,
                    message: format!(
                        "the OpenAI-compatible wire cannot carry media inside a tool result (v1: text-only tool messages); a {:?} part was refused and nothing was sent",
                        part_kind(other)
                    ),
                })
            }
        }
    }
    Ok(Some(json!({
        "role": "tool",
        "tool_call_id": tool_use_id,
        "content": pieces.join("\n\n"),
    })))
}

/// One assistant turn, natively: the text parts form `content`; the tool
/// calls form `tool_calls` (arguments are the JSON *encoding* of the stored
/// input, which is what this wire expects). Assistant
/// `Reasoning`/`RedactedThinking` parts are omitted — the wire cannot replay
/// them (`docs/14` §9 owner decision: a replay attempt is worse than replay
/// absence).
fn assistant_message(parts: &[ContentPart]) -> Result<Value, ProviderError> {
    let mut text_parts: Vec<String> = Vec::new();
    let mut tool_calls = Vec::new();
    for part in parts {
        match part {
            ContentPart::Text { text } => text_parts.push(text.clone()),
            ContentPart::ToolUse { id, name, input } => tool_calls.push(json!({
                "id": id,
                "type": "function",
                "function": {
                    "name": name,
                    "arguments": input.to_string(),
                },
            })),
            ContentPart::Reasoning { .. } | ContentPart::RedactedThinking { .. } => {}
            other => {
                return Err(ProviderError {
                    class: ErrorClass::UnsupportedModality,
                    status: None,
                    message: format!(
                        "this wire cannot replay a {:?} part in assistant history",
                        part_kind(other)
                    ),
                })
            }
        }
    }
    // `content` stays a plain string (universally accepted; empty is legal
    // when only tool_calls are present).
    let mut message = json!({
        "role": "assistant",
        "content": text_parts.join("\n\n"),
    });
    if !tool_calls.is_empty() {
        message["tool_calls"] = json!(tool_calls);
    }
    Ok(message)
}

/// The message array of a request, natively — history order preserved. A
/// user turn's `ToolResult` parts open their own following `role: "tool"`
/// messages (this wire requires tool results as separate messages), keyed to
/// the assistant turn that issued the calls.
fn messages_array(messages: &[(Role, Vec<ContentPart>)]) -> Result<Vec<Value>, ProviderError> {
    // Emits nothing for an empty user turn: the absence of content is the
    // caller's building, not a wire contract.
    let mut out: Vec<Value> = Vec::new();
    for (role, parts) in messages {
        match role {
            Role::Assistant => out.push(assistant_message(parts)?),
            Role::User => {
                // A user message may carry BOTH the tool results of the
                // previous assistant turn and the user's new prompt: tools
                // split off in order, the rest cohere into one user message.
                let mut content: Vec<Value> = Vec::new();
                for part in parts {
                    match part {
                        ContentPart::ToolResult { .. } => {
                            if !content.is_empty() {
                                out.push(json!({ "role": "user", "content": content }));
                                content = Vec::new();
                            }
                            if let Some(message) = tool_message(part)? {
                                out.push(message);
                            }
                        }
                        other => {
                            if let Some(native) = user_part(other)? {
                                content.push(native);
                            }
                        }
                    }
                }
                if !content.is_empty() {
                    out.push(json!({ "role": "user", "content": content }));
                }
            }
        }
    }
    Ok(out)
}

/// Build the native chat-completions body.
///
/// Everything the caller decided travels; nothing is invented: no `tools`
/// key when no tools were given, no `temperature`/`top_p` when unset (the
/// `AI-ADAPTER-10` regression), the model id verbatim (`AI-ADAPTER-11`).
/// The v1 readings (system join, `max_tokens`, no `stream_options`, dropped
/// `top_k`/thinking/betas/`cache_control`) are the crate header's.
pub(crate) fn chat_request_body(
    request: &myharness_provider_sdk::NativeTurnRequest,
) -> Result<Value, ProviderError> {
    let mut messages = Vec::new();
    if !request.system.is_empty() {
        messages.push(json!({
            "role": "system",
            "content": system_content(&request.system),
        }));
    }
    messages.extend(messages_array(&request.messages)?);

    let mut body = json!({
        "model": request.model.model_id,
        "messages": messages,
        "max_tokens": request.max_tokens,
        "stream": request.stream,
    });
    if let Some(temperature) = request.temperature {
        body["temperature"] = json!(temperature);
    }
    if let Some(top_p) = request.top_p {
        body["top_p"] = json!(top_p);
    }
    if request.top_k.is_some() {
        // Dropped, recorded: this wire has no `top_k` spelling
        // (`docs/14` §9 owner decision).
    }
    if !request.stop_sequences.is_empty() {
        body["stop"] = json!(request.stop_sequences);
    }
    if !request.tools.is_empty() {
        body["tools"] = json!(request.tools.iter().map(tool).collect::<Vec<_>>());
    }
    Ok(body)
}

// ---------------------------------------------------------------------------
// The response side: errors, usage, stop reasons
// ---------------------------------------------------------------------------

/// This wire's error envelope: `{"error": {"message", "type", "code", …}}`
/// (OpenAI) and the tolerant spellings compatible servers send. The mapping
/// is the `TEST-AI-021` vocabulary — 401/403 are auth, 429 and
/// `insufficient_quota`/billing wording are quota, 404 is a missing model,
/// context-length wording is a context error, 5xx is transient; the
/// provider's own wording travels, and nothing else from the body is echoed
/// (it could quote a credential).
pub(crate) fn error_from_envelope(status: Option<u16>, body: &Value) -> ProviderError {
    let owner = body.get("error").cloned().unwrap_or_else(|| body.clone());
    let kind = owner.get("type").and_then(Value::as_str);
    let code = owner
        .get("code")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            owner
                .get("code")
                .and_then(Value::as_i64)
                .map(|code| code.to_string())
        });
    let text = owner
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let combined = text.to_lowercase();
    let kinds = kind.unwrap_or("");

    let quota = code.as_deref() == Some("insufficient_quota")
        || kinds.contains("insufficient_quota")
        || combined.contains("insufficient quota")
        || combined.contains("billing")
        || combined.contains("exceeded your current quota");
    let context = combined.contains("context length")
        || combined.contains("maximum context")
        || combined.contains("too long");

    let class = match kind {
        Some("authentication_error") | Some("permission_error") => ErrorClass::Auth,
        Some("not_found_error") => ErrorClass::ModelNotFound,
        Some("rate_limit_error") | Some("insufficient_quota") => ErrorClass::Quota,
        Some("overloaded_error") | Some("api_error") | Some("server_error") => {
            ErrorClass::Transient
        }
        _ => {
            if quota {
                ErrorClass::Quota
            } else if context {
                ErrorClass::ContextLength
            } else {
                status
                    .map(crate::adapter::status_class)
                    .unwrap_or(ErrorClass::InvalidRequest)
            }
        }
    };
    let message = if text.is_empty() {
        match (kind, code) {
            (Some(kind), Some(code)) => {
                format!("the provider reported an error of type {kind:?} ({code})")
            }
            (Some(kind), None) => format!("the provider reported an error of type {kind:?}"),
            _ => "the provider reported an error without a message body".to_string(),
        }
    } else {
        text
    };
    ProviderError {
        class,
        status,
        message,
    }
}

/// Parse a non-success response when no envelope can be read.
pub(crate) fn error_from_status(status: u16) -> ProviderError {
    ProviderError {
        class: crate::adapter::status_class(status),
        status: Some(status),
        message: format!("the provider answered with status {status}"),
    }
}

/// The usage numbers of a completion or chunk, mapped off this wire's names
/// (`AI-USAGE-01`: exact, separate). Absent means absent, never zero
/// masquerading; a count the wire does not name stays absent.
pub(crate) fn parse_usage(usage: &Value) -> Option<Usage> {
    let read = |key: &str| usage.get(key).and_then(Value::as_u64);
    let input = read("prompt_tokens");
    let output = read("completion_tokens");
    let cache_read = usage
        .get("prompt_tokens_details")
        .and_then(|details| details.get("cached_tokens"))
        .and_then(Value::as_u64);
    let known = input.is_some() || output.is_some() || cache_read.is_some();
    if known {
        Some(Usage {
            input: input.unwrap_or_default(),
            output: output.unwrap_or_default(),
            cache_read,
            ..Default::default()
        })
    } else {
        None
    }
}

/// This wire's `finish_reason` vocabulary, preserved onto the app's set
/// (`AI-ADAPTER-08`); unknown spellings stay verbatim.
pub(crate) fn stop_reason(raw: &str) -> NativeStopReason {
    match raw {
        "stop" => NativeStopReason::EndTurn,
        "length" => NativeStopReason::MaxTokens,
        "tool_calls" | "function_call" => NativeStopReason::ToolUse,
        "content_filter" => NativeStopReason::ContentFilter,
        other => NativeStopReason::Other(other.to_string()),
    }
}

/// The model echo gate: a dated revision of the requested id stays legal;
/// anything outside the requested id's prefix is a refusal (`AI-MODEL-01`).
pub(crate) fn model_echo_ok(requested: &str, ran: &str) -> bool {
    ran == requested || ran.starts_with(&format!("{requested}-"))
}

// ---------------------------------------------------------------------------
// The response side: the chunk parser
// ---------------------------------------------------------------------------

/// One parsed stream frame, transport-shaped — the app's typed events come
/// out of the state machine in [`crate::adapter`].
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum StreamEvent {
    /// An empty or comment frame — a transport keep-alive, not progress.
    Ping,
    /// `data: {…}` — one streamed chunk (or a whole non-streamed-looking
    /// frame a compatible server sent mid-stream).
    Delta {
        /// The completion id — the base for synthesized tool-call ids when a
        /// compatible server omits `id` on continuation fragments
        /// (`docs/14` §9 owner decision).
        completion_id: Option<String>,
        model: Option<String>,
        /// The first choice's `delta` object, verbatim.
        delta: Value,
        /// The choice's `finish_reason`, when this chunk finalized the turn.
        finish_reason: Option<String>,
        /// The chunk-level `usage` object, when the server sent one.
        usage: Option<Value>,
    },
    /// `data: [DONE]` — the wire's terminal sentinel.
    Terminated,
}

fn from_json_error(error: serde_json::Error) -> ProviderError {
    ProviderError {
        class: ErrorClass::InvalidRequest,
        status: None,
        message: format!("the stream carried a frame that is not valid JSON: {error}"),
    }
}

/// Feed raw bytes in; run complete frames out. Handles `data:` lines and the
/// `[DONE]` sentinel, CRLF, and multi-line `data:` — the same framing rules
/// as every other adapter's SSE parser.
pub(crate) struct ChunkParser {
    buffer: String,
}

impl ChunkParser {
    pub(crate) fn new() -> Self {
        ChunkParser {
            buffer: String::new(),
        }
    }

    /// Consume a byte chunk; return the events of every complete frame.
    ///
    /// An error in the list means the bytes were malformed — the caller
    /// surfaces it (`AI-MM-01`) and stops; nothing is dropped silently.
    pub(crate) fn feed(&mut self, chunk: &[u8]) -> Vec<Result<StreamEvent, ProviderError>> {
        let Ok(chunk) = str::from_utf8(chunk) else {
            return vec![Err(ProviderError {
                class: ErrorClass::InvalidRequest,
                status: None,
                message: "the stream carried bytes that are not valid UTF-8".to_string(),
            })];
        };
        self.buffer.push_str(chunk);
        let mut events = Vec::new();
        while let Some(frame) = next_frame(&mut self.buffer) {
            events.push(self.parse_frame(&frame));
        }
        events
    }

    fn parse_frame(&self, frame: &str) -> Result<StreamEvent, ProviderError> {
        let mut data = String::new();
        for line in frame.lines() {
            let line = line.trim();
            if line.starts_with(':') {
                // A comment (some servers keep-alive with `: ping`).
                continue;
            }
            if let Some(payload) = line.strip_prefix("data:") {
                if !data.is_empty() {
                    data.push('\n');
                }
                data.push_str(payload.trim());
            }
        }
        if data.trim().is_empty() {
            return Ok(StreamEvent::Ping);
        }
        if data.trim() == "[DONE]" {
            return Ok(StreamEvent::Terminated);
        }
        let body: Value = serde_json::from_str(data.trim()).map_err(from_json_error)?;
        if body.as_object().is_some_and(|map| map.is_empty()) {
            // The all-but-universal compat keep-alive: an empty object
            // carries nothing to surface (a handled keep-alive, not an
            // invented event).
            return Ok(StreamEvent::Ping);
        }
        if body.get("error").is_some() {
            // A mid-stream error chunk: the state machine maps it verbatim
            // (`TEST-AI-021` for stream errors).
            Err(error_from_envelope(None, &body))
        } else if let Some(choice) = body
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first())
        {
            Ok(StreamEvent::Delta {
                completion_id: body.get("id").and_then(Value::as_str).map(str::to_string),
                model: body
                    .get("model")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                delta: choice.get("delta").cloned().unwrap_or_else(|| json!({})),
                finish_reason: choice
                    .get("finish_reason")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                usage: body.get("usage").cloned(),
            })
        } else {
            // A data frame with neither choices, nor error, nor [DONE]: some
            // compatible servers interpolate control frames. Neither a
            // content event nor a stop reason is invented for unknown shapes;
            // the frame is surfaced by name (`AI-MM-01`, no raw bodies in
            // error text — `docs/18` §11).
            let keys = body
                .as_object()
                .map(|map| map.keys().cloned().collect::<Vec<_>>().join(", "))
                .filter(|keys| !keys.is_empty());
            Err(ProviderError {
                class: ErrorClass::InvalidRequest,
                status: None,
                message: match keys {
                    Some(keys) => format!(
                        "the stream carried a data frame with keys {keys:?} of an unhandled shape"
                    ),
                    None => "the stream carried a data frame of an unhandled shape".to_string(),
                },
            })
        }
    }
}

/// Split the next complete frame off the buffer. Frames are separated by a
/// blank line; tolerate `\n\n` and `\r\n\r\n`.
fn next_frame(buffer: &mut String) -> Option<String> {
    let lf = buffer.find("\n\n").map(|i| (i, 2));
    let crlf = buffer.find("\r\n\r\n").map(|i| (i, 4));
    let (end, consumed) = match (lf, crlf) {
        (Some(a), Some(b)) => {
            if a.0 <= b.0 {
                a
            } else {
                b
            }
        }
        (Some(a), None) => a,
        (None, Some(b)) => b,
        (None, None) => return None,
    };
    let frame = buffer[..end].trim_matches('\r').to_string();
    buffer.drain(..end + consumed);
    Some(frame)
}

// ---------------------------------------------------------------------------
// The non-streaming completion
// ---------------------------------------------------------------------------

/// A non-streaming completion's tool call (`choices[0].message.tool_calls[n]`).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NonStreamToolCall {
    pub id: Option<String>,
    pub name: Option<String>,
    pub arguments: String,
}

/// The subset of a non-streaming completion the adapter models.
#[derive(Debug)]
pub(crate) struct NonStreamCompletion {
    pub model: Option<String>,
    /// Assistant text (`content`, when a string).
    pub text: Option<String>,
    /// Reasoning text (`reasoning_content`, falling back to `reasoning`).
    pub reasoning: Option<String>,
    pub tool_calls: Vec<NonStreamToolCall>,
    pub finish_reason: Option<String>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
struct NonStreamChoice {
    message: Option<NonStreamMessage>,
    finish_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
struct NonStreamMessage {
    content: Option<Value>,
    reasoning_content: Option<String>,
    reasoning: Option<String>,
    tool_calls: Option<Vec<NonStreamToolCallShape>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
struct NonStreamToolCallShape {
    id: Option<String>,
    function: Option<NonStreamFunction>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
struct NonStreamFunction {
    name: Option<String>,
    arguments: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
struct NonStreamTop {
    model: Option<String>,
    choices: Vec<NonStreamChoice>,
    usage: Option<Value>,
}

/// Parse a non-streaming completion, tolerantly: choice 0 is the v1 choice
/// (`n` is never requested above one); the shape reading mirrors the chunk
/// parser (`reasoning_content` first, `reasoning` as fallback).
pub(crate) fn parse_completion(body: &Value) -> Result<NonStreamCompletion, ProviderError> {
    let top: NonStreamTop =
        serde_json::from_value(body.clone()).map_err(|error| ProviderError {
            class: ErrorClass::InvalidRequest,
            status: None,
            message: format!("the completion answer was not a readable shape: {error}"),
        })?;
    let choice = choice(&top)?;
    let message = choice.message.unwrap_or(NonStreamMessage {
        content: None,
        reasoning_content: None,
        reasoning: None,
        tool_calls: None,
    });
    let text = message
        .content
        .as_ref()
        .and_then(Value::as_str)
        .map(str::to_string);
    let reasoning = message.reasoning_content.clone().or(message.reasoning);
    let tool_calls = message
        .tool_calls
        .unwrap_or_default()
        .into_iter()
        .map(|call| NonStreamToolCall {
            id: call.id,
            name: call
                .function
                .as_ref()
                .and_then(|function| function.name.clone()),
            arguments: call
                .function
                .and_then(|function| function.arguments)
                .map(|arguments| match arguments {
                    Value::String(raw) => raw,
                    other => other.to_string(),
                })
                .unwrap_or_default(),
        })
        .collect();
    Ok(NonStreamCompletion {
        model: top.model,
        text,
        reasoning,
        tool_calls,
        finish_reason: choice.finish_reason,
        usage: top.usage.as_ref().and_then(parse_usage),
    })
}

fn choice(top: &NonStreamTop) -> Result<NonStreamChoice, ProviderError> {
    top.choices.first().cloned().ok_or_else(|| ProviderError {
        class: ErrorClass::InvalidRequest,
        status: None,
        message: "the completion answer carried no choice".to_string(),
    })
}
