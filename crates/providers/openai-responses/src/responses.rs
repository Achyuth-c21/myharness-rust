//! The OpenAI **Responses** wire contract — `POST {base}/responses` and its
//! SSE event stream (`docs/13` Phase 8 reference extension).
//!
//! The request builder turns the app's typed conversation model
//! ([`myharness_provider_sdk::NativeTurnRequest`]) into the Responses body:
//! system blocks join into `instructions`, history maps to typed
//! `input` items (`input_text`/`input_image`/`input_file`,
//! `function_call`/`function_call_output`), and `max_output_tokens` carries
//! the output budget. `store: false` is explicit — a desktop harness with
//! its own store does not opt into server-side retention by omission
//! (`docs/14` §9).
//!
//! The response side parses two shapes: the whole `response` object
//! (non-streaming) and the SSE event stream whose `data` payloads dispatch
//! on their own `type` field. Documented lifecycle events with no app
//! mapping are consumed as no-ops; an undocumented event type is an error —
//! a grammar the adapter does not know may be carrying content it would
//! silently drop (`AI-MM-01`).

use std::time::Duration;

use myharness_provider_sdk::{
    ContentPart, ErrorClass, NativeStopReason, ProviderError, Role, ToolSpec, Usage,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// The documented default idle timeout (the setup profile's
/// `inferenceStreamIdleTimeoutSec` default — `docs/20`; same window the
/// sibling families default to).
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
pub struct ResponsesClientConfig {
    /// The base URL. Trailing slashes are normalized; empty falls back to
    /// the first-party host (this family is first-party OpenAI: a static
    /// key is required — `docs/14` §9).
    pub base_url: String,
    /// The idle window for streamed turns.
    pub idle_timeout: Duration,
    /// Extra headers the profile forwards (tenant/routing material).
    pub custom_headers: Vec<(String, String)>,
}

// ---------------------------------------------------------------------------
// The request side: the body
// ---------------------------------------------------------------------------

/// One tool, natively: the Responses wire spells tools **flat** —
/// `{type: "function", name, parameters, …}` — not nested under a
/// `function` object the way Chat Completions does. Name, description, and
/// the JSON Schema travel exactly (`AI-TOOL-01`); `cache_control` has no
/// spelling here and is dropped, recorded.
fn tool(spec: &ToolSpec) -> Value {
    let mut tool = json!({
        "type": "function",
        "name": spec.name,
        "parameters": spec.input_schema,
    });
    if let Some(description) = &spec.description {
        tool["description"] = json!(description);
    }
    tool
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

/// One user content part, natively. This wire carries images
/// (`input_image`, data URI) **and** documents (`input_file`, base64 data
/// URI) natively — the one family where a PDF need not be refused.
fn user_part(part: &ContentPart) -> Result<Option<Value>, ProviderError> {
    match part {
        ContentPart::Text { text } => Ok(Some(json!({ "type": "input_text", "text": text }))),
        ContentPart::Image { media_type, data } => Ok(Some(json!({
            "type": "input_image",
            "image_url": format!("data:{media_type};base64,{data}"),
        }))),
        ContentPart::Document {
            media_type,
            data,
            name,
        } => {
            let mut file = json!({
                "type": "input_file",
                "file_data": format!("data:{media_type};base64,{data}"),
            });
            if let Some(name) = name {
                file["filename"] = json!(name);
            }
            Ok(Some(file))
        }
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

/// One tool result, natively: a `function_call_output` input item keyed to
/// the issuing call. Media inside tool results is refused on v1 (recorded —
/// the result-content spelling for media was not pinned when this adapter
/// was written; an explicit refusal beats a guessed shape); `is_error` has
/// no spelling — the result text carries the failure to the model.
fn function_call_output(part: &ContentPart) -> Result<Option<Value>, ProviderError> {
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
                        "the Responses wire cannot carry media inside a tool result (v1: text-only tool results); a {:?} part was refused and nothing was sent",
                        part_kind(other)
                    ),
                })
            }
        }
    }
    Ok(Some(json!({
        "type": "function_call_output",
        "call_id": tool_use_id,
        "output": pieces.join("\n\n"),
    })))
}

/// One assistant turn's replay items, natively: text coheres into one
/// `role: "assistant"` message with `output_text` parts; tool calls replay
/// as `function_call` items with the JSON *encoding* of the stored input.
/// Assistant `Reasoning`/`RedactedThinking` parts are omitted — this wire
/// replays reasoning only through encrypted items the app does not hold
/// (`docs/14` §9).
fn assistant_items(parts: &[ContentPart]) -> Result<Vec<Value>, ProviderError> {
    let mut text_parts: Vec<Value> = Vec::new();
    let mut calls = Vec::new();
    for part in parts {
        match part {
            ContentPart::Text { text } => text_parts.push(json!({
                "type": "output_text",
                "text": text,
            })),
            ContentPart::ToolUse { id, name, input } => calls.push(json!({
                "type": "function_call",
                "call_id": id,
                "name": name,
                "arguments": input.to_string(),
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
    let mut items = Vec::new();
    if !text_parts.is_empty() {
        items.push(json!({
            "role": "assistant",
            "content": text_parts,
        }));
    }
    items.extend(calls);
    Ok(items)
}

/// The `input` array of a request, natively — history order preserved. A
/// user turn's `ToolResult` parts become their own `function_call_output`
/// items in order; the rest coheres into one user message.
fn input_items(messages: &[(Role, Vec<ContentPart>)]) -> Result<Vec<Value>, ProviderError> {
    let mut out: Vec<Value> = Vec::new();
    for (role, parts) in messages {
        match role {
            Role::Assistant => out.extend(assistant_items(parts)?),
            Role::User => {
                let mut content: Vec<Value> = Vec::new();
                for part in parts {
                    match part {
                        ContentPart::ToolResult { .. } => {
                            if !content.is_empty() {
                                out.push(json!({ "role": "user", "content": content }));
                                content = Vec::new();
                            }
                            if let Some(item) = function_call_output(part)? {
                                out.push(item);
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

/// Build the native Responses body.
///
/// Everything the caller decided travels; nothing is invented: no `tools`
/// key when no tools were given, no `temperature`/`top_p` when unset (the
/// `AI-ADAPTER-10` regression), the model id verbatim (`AI-ADAPTER-11`).
/// `store: false` is the recorded posture; thinking budgets, `top_k`, stop
/// sequences, and betas are dropped (no pinned spelling), recorded.
pub(crate) fn responses_request_body(
    request: &myharness_provider_sdk::NativeTurnRequest,
) -> Result<Value, ProviderError> {
    let mut input = Vec::new();
    input.extend(input_items(&request.messages)?);

    let mut body = json!({
        "model": request.model.model_id,
        "input": input,
        "max_output_tokens": request.max_tokens,
        "stream": request.stream,
        "store": false,
    });
    let instructions = request
        .system
        .iter()
        .map(|block| block.text.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");
    if !instructions.is_empty() {
        body["instructions"] = json!(instructions);
    }
    if let Some(temperature) = request.temperature {
        body["temperature"] = json!(temperature);
    }
    if let Some(top_p) = request.top_p {
        body["top_p"] = json!(top_p);
    }
    if !request.tools.is_empty() {
        body["tools"] = json!(request.tools.iter().map(tool).collect::<Vec<_>>());
    }
    Ok(body)
}

// ---------------------------------------------------------------------------
// The response side: errors, usage, stop reasons
// ---------------------------------------------------------------------------

/// The platform's error envelope — the same shape Chat Completions answers
/// with (`{"error": {"message", "type", "code", …}}`). The mapping is the
/// `TEST-AI-021` vocabulary; the provider's own wording travels and nothing
/// else from the body is echoed (it could quote a credential).
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

    // The class comes from the envelope's `type` spelling or its `code`
    // spelling, in that order: the stream's bare `error` event carries only
    // a `code` (its `type` is the event name itself), while the platform's
    // HTTP envelopes may spell either. Unmapped spellings fall through to
    // the text heuristics, then the HTTP status.
    let classify = |spelling: &str| match spelling {
        "authentication_error" | "permission_error" | "invalid_api_key" => Some(ErrorClass::Auth),
        "not_found_error" | "model_not_found" => Some(ErrorClass::ModelNotFound),
        "rate_limit_error" | "rate_limit_exceeded" | "insufficient_quota" => {
            Some(ErrorClass::Quota)
        }
        "overloaded_error" | "api_error" | "server_error" | "timeout" => {
            Some(ErrorClass::Transient)
        }
        _ => None,
    };
    let class = [kind, code.as_deref()]
        .iter()
        .flatten()
        .find_map(|spelling| classify(spelling))
        .unwrap_or_else(|| {
            if quota {
                ErrorClass::Quota
            } else if context {
                ErrorClass::ContextLength
            } else {
                status
                    .map(crate::adapter::status_class)
                    .unwrap_or(ErrorClass::InvalidRequest)
            }
        });
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

/// The usage numbers of a response object, mapped off this wire's names
/// (`AI-USAGE-01`: exact, separate). `output_tokens_details.reasoning_tokens`
/// is a subset of output and has no app channel — dropped, recorded. Absent
/// means absent, never zero masquerading.
pub(crate) fn parse_usage(usage: &Value) -> Option<Usage> {
    let read = |key: &str| usage.get(key).and_then(Value::as_u64);
    let input = read("input_tokens");
    let output = read("output_tokens");
    let cache_read = usage
        .get("input_tokens_details")
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

/// This wire's terminal mapping (`AI-ADAPTER-08`): `status: "completed"`
/// ends the turn — `tool_use` when the turn emitted function calls (the
/// terminal event carries no per-choice `finish_reason`; the emitted items
/// are the equivalent fact); `incomplete_details.reason` spells the
/// incomplete reasons. Unknown spellings stay verbatim.
pub(crate) fn terminal_stop_reason(
    status: &str,
    incomplete_reason: Option<&str>,
    emitted_function_calls: bool,
) -> NativeStopReason {
    match status {
        "completed" if emitted_function_calls => NativeStopReason::ToolUse,
        "completed" => NativeStopReason::EndTurn,
        "incomplete" => match incomplete_reason {
            Some("max_output_tokens") => NativeStopReason::MaxTokens,
            Some("content_filter") => NativeStopReason::ContentFilter,
            Some(other) => NativeStopReason::Other(other.to_string()),
            None => NativeStopReason::Other("incomplete".to_string()),
        },
        other => NativeStopReason::Other(other.to_string()),
    }
}

/// The model echo gate: a dated revision of the requested id stays legal;
/// anything outside the requested id's prefix is a refusal (`AI-MODEL-01`).
pub(crate) fn model_echo_ok(requested: &str, ran: &str) -> bool {
    ran == requested || ran.starts_with(&format!("{requested}-"))
}

// ---------------------------------------------------------------------------
// The SSE event stream
// ---------------------------------------------------------------------------

/// One parsed SSE frame: the `event:` line (transport spelling) and the
/// `data:` payload. Responses dispatches on the payload's own `type` field;
/// the event line is the fallback when a frame carries none.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct WireEvent {
    pub event: Option<String>,
    pub data: String,
}

/// The documented event types this adapter understands, split by mapping.
/// Anything outside the union is surfaced as an error (`AI-MM-01`); the
/// no-op set is documented lifecycle the app models nothing from.
pub(crate) fn event_disposition(event_type: &str) -> Disposition {
    match event_type {
        // Content and tools: the events that map onto app events.
        "response.created"
        | "response.output_text.delta"
        | "response.reasoning_summary_text.delta"
        | "response.refusal.delta"
        | "response.output_text.annotation.added"
        | "response.output_item.added"
        | "response.function_call_arguments.delta"
        | "response.output_item.done"
        | "response.completed"
        | "response.failed"
        | "error" => Disposition::Map,
        // Documented lifecycle with no app model: consumed, never surfaced
        // as unknown (their `.done` duplicates arrive through the items and
        // `.delta`s already mapped; annotations also arrive through
        // `output_item.done`, and the `.added` spelling maps above).
        "response.in_progress"
        | "response.content_part.added"
        | "response.content_part.done"
        | "response.output_text.done"
        | "response.reasoning_summary_part.added"
        | "response.reasoning_summary_part.done"
        | "response.reasoning_summary_text.done"
        | "response.refusal.done"
        | "response.function_call_arguments.done"
        | "response.queued" => Disposition::Noop,
        _ => Disposition::Unknown,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Disposition {
    Map,
    Noop,
    Unknown,
}

/// Feed raw bytes in; run complete frames out. Handles `event:`/`data:`
/// lines, CRLF, comments, and multi-line `data:` — the same framing rules
/// as every other adapter's SSE parser.
pub(crate) struct EventParser {
    buffer: String,
}

impl EventParser {
    pub(crate) fn new() -> Self {
        EventParser {
            buffer: String::new(),
        }
    }

    /// Consume a byte chunk; return the events of every complete frame.
    ///
    /// An error in the list means the bytes were malformed — the caller
    /// surfaces it (`AI-MM-01`) and stops; nothing is dropped silently.
    pub(crate) fn feed(&mut self, chunk: &[u8]) -> Vec<Result<WireEvent, ProviderError>> {
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

    fn parse_frame(&self, frame: &str) -> Result<WireEvent, ProviderError> {
        let mut event = None;
        let mut data = String::new();
        for line in frame.lines() {
            let line = line.trim();
            if line.starts_with(':') {
                // A comment (some servers keep-alive with `: ping`).
                continue;
            }
            if let Some(name) = line.strip_prefix("event:") {
                event = Some(name.trim().to_string());
            } else if let Some(payload) = line.strip_prefix("data:") {
                if !data.is_empty() {
                    data.push('\n');
                }
                data.push_str(payload.trim());
            }
        }
        if data.trim().is_empty() {
            // An empty frame is a transport keep-alive, not progress.
            return Ok(WireEvent {
                event: Some(String::new()),
                data: String::new(),
            });
        }
        Ok(WireEvent { event, data })
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
// The non-streaming response object
// ---------------------------------------------------------------------------

/// One output item of a completed response.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OutputFunctionCall {
    pub call_id: Option<String>,
    pub name: Option<String>,
    pub arguments: String,
}

/// The subset of a completed `response` object the adapter models.
#[derive(Debug)]
pub(crate) struct ParsedResponse {
    pub model: Option<String>,
    /// Assistant text, in item order.
    pub text: String,
    /// Reasoning summary text, in item order.
    pub reasoning: String,
    /// Annotations from `output_text` parts, verbatim (`AI-STREAM-01`).
    pub citations: Vec<Value>,
    pub function_calls: Vec<OutputFunctionCall>,
    pub status: Option<String>,
    pub incomplete_reason: Option<String>,
    pub usage: Option<Usage>,
    /// The response-level `error` of a `response.failed` object.
    pub error: Option<Value>,
}

/// Parse a completed `response` object, tolerantly: unknown output item
/// types are refused, never silently dropped (`AI-MM-01`).
pub(crate) fn parse_response(body: &Value) -> Result<ParsedResponse, ProviderError> {
    let response = body
        .get("response")
        .cloned()
        .unwrap_or_else(|| body.clone());
    let mut text = String::new();
    let mut reasoning = String::new();
    let mut citations = Vec::new();
    let mut function_calls = Vec::new();
    for item in response
        .get("output")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
    {
        match item.get("type").and_then(Value::as_str) {
            Some("message") => {
                for part in item
                    .get("content")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default()
                {
                    match part.get("type").and_then(Value::as_str) {
                        Some("output_text") => {
                            if let Some(fragment) = part.get("text").and_then(Value::as_str) {
                                text.push_str(fragment);
                            }
                            if let Some(annotations) =
                                part.get("annotations").and_then(Value::as_array)
                            {
                                citations.extend(annotations.iter().cloned());
                            }
                        }
                        Some("refusal") => {
                            if let Some(fragment) = part.get("refusal").and_then(Value::as_str) {
                                text.push_str(fragment);
                            }
                        }
                        _ => return Err(ProviderError {
                            class: ErrorClass::InvalidRequest,
                            status: None,
                            message:
                                "the response carried a message content part of an unhandled shape"
                                    .to_string(),
                        }),
                    }
                }
            }
            Some("reasoning") => {
                for part in item
                    .get("summary")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default()
                {
                    if let Some(fragment) = part.get("text").and_then(Value::as_str) {
                        reasoning.push_str(fragment);
                    }
                }
            }
            Some("function_call") => function_calls.push(OutputFunctionCall {
                call_id: item
                    .get("call_id")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                name: item.get("name").and_then(Value::as_str).map(str::to_string),
                arguments: item
                    .get("arguments")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
            }),
            _ => {
                return Err(ProviderError {
                    class: ErrorClass::InvalidRequest,
                    status: None,
                    message: "the response carried an output item of an unhandled type".to_string(),
                })
            }
        }
    }
    Ok(ParsedResponse {
        model: response
            .get("model")
            .and_then(Value::as_str)
            .map(str::to_string),
        text,
        reasoning,
        citations,
        function_calls,
        status: response
            .get("status")
            .and_then(Value::as_str)
            .map(str::to_string),
        incomplete_reason: response
            .get("incomplete_details")
            .and_then(|details| details.get("reason"))
            .and_then(Value::as_str)
            .map(str::to_string),
        usage: response.get("usage").and_then(parse_usage),
        error: response
            .get("error")
            .filter(|error| !error.is_null())
            .cloned(),
    })
}
