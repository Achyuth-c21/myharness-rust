//! The Chat Completions grammar as the local runtimes document it — the
//! request builder (the app's typed conversation model onto
//! `POST {base}/v1/chat/completions`), the response and SSE parsing, the
//! error envelope, and the recorded mappings. Own code, sibling design
//! (`AI-PROVIDER-02`); every reading is pinned by this crate's fixtures and
//! recorded in `docs/14` §9.

use std::time::Duration;

use myharness_provider_sdk::{
    ContentPart, ErrorClass, NativeStopReason, ProviderError, Role, ToolSpec, Usage,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// The documented default idle timeout (the setup profile's
/// `inferenceStreamIdleTimeoutSec` default — `docs/20`; the window every
/// wall defaults to).
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
/// the caller. The wall owns the destination gate and the wire policy.
#[derive(Debug, Clone, PartialEq)]
pub struct LocalClientConfig {
    /// The base URL; validated and pinned at adapter construction (the
    /// gate is `crate::destination`), empty falls to the family default.
    pub base_url: String,
    /// The idle window for streamed turns.
    pub idle_timeout: Duration,
    /// Extra headers the profile forwards (routing/tenant material — the
    /// forms refuse credential-shaped ones).
    pub custom_headers: Vec<(String, String)>,
}

// ---------------------------------------------------------------------------
// The request side
// ---------------------------------------------------------------------------

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

/// One user content part, natively: text and images (`image_url` data URIs
/// — the runtimes' compat surfaces carry images); documents are refused
/// (`UnsupportedModality`, `AI-MM-01/02`) — no compat grammar was pinned
/// for them and a guessed shape is worse than a refusal.
fn user_part(part: &ContentPart) -> Result<Option<Value>, ProviderError> {
    match part {
        ContentPart::Text { text } => Ok(Some(json!({ "type": "text", "text": text }))),
        ContentPart::Image { media_type, data } => Ok(Some(json!({
            "type": "image_url",
            "image_url": { "url": format!("data:{media_type};base64,{data}") },
        }))),
        other => Err(ProviderError {
            class: ErrorClass::UnsupportedModality,
            status: None,
            message: format!(
                "the local compat surface cannot carry a content part of kind {:?} on a user turn",
                part_kind(other)
            ),
        }),
    }
}

/// One tool result, natively: a `role: "tool"` message keyed to the issuing
/// call. Media inside tool results is refused (`AI-MM-02`); `is_error` has
/// no spelling — the result text carries the failure.
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
                        "the local compat surface cannot carry media inside a tool result (text-only); a {:?} part was refused and nothing was sent",
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

/// One assistant turn's replay messages: text as a plain assistant message;
/// tool calls as `tool_calls` with the JSON *encoding* of the stored input.
/// Assistant `Reasoning`/`RedactedThinking` parts are omitted — the compat
/// surfaces replay reasoning only through server-side state the app does
/// not hold (`docs/14` §9).
fn assistant_messages(parts: &[ContentPart]) -> Result<Vec<Value>, ProviderError> {
    let mut text_parts: Vec<Value> = Vec::new();
    let mut calls: Vec<Value> = Vec::new();
    for part in parts {
        match part {
            ContentPart::Text { text } => text_parts.push(json!({ "type": "text", "text": text })),
            ContentPart::ToolUse { id, name, input } => calls.push(json!({
                "id": id,
                "type": "function",
                "function": { "name": name, "arguments": input.to_string() },
            })),
            ContentPart::Reasoning { .. } | ContentPart::RedactedThinking { .. } => {}
            other => {
                return Err(ProviderError {
                    class: ErrorClass::UnsupportedModality,
                    status: None,
                    message: format!(
                        "the local compat surface cannot replay a {:?} part in assistant history",
                        part_kind(other)
                    ),
                })
            }
        }
    }
    let mut messages = Vec::new();
    if !text_parts.is_empty() {
        messages.push(json!({ "role": "assistant", "content": text_parts }));
    }
    if !calls.is_empty() {
        messages.push(json!({ "role": "assistant", "tool_calls": calls }));
    }
    Ok(messages)
}

/// The `messages` array of a request — history order preserved, consecutive
/// same-role user turns merged only where the grammar demands (a tool
/// message must follow its call; a user turn's ToolResult parts become
/// their own tool messages in order).
fn request_messages(
    system: &[myharness_provider_sdk::SystemBlock],
    messages: &[(Role, Vec<ContentPart>)],
) -> Result<Vec<Value>, ProviderError> {
    let mut out: Vec<Value> = Vec::new();
    let instructions = system
        .iter()
        .map(|block| block.text.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");
    if !instructions.is_empty() {
        out.push(json!({ "role": "system", "content": instructions }));
    }
    for (role, parts) in messages {
        match role {
            Role::Assistant => out.extend(assistant_messages(parts)?),
            Role::User => {
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

/// One tool, natively: the Chat Completions spelling nests under a
/// `function` object (unlike the Responses wire's flat tools). Name,
/// description, and the JSON Schema travel exactly (`AI-TOOL-01`);
/// `cache_control` has no spelling here and is dropped, recorded.
fn tool(spec: &ToolSpec) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": spec.name,
            "parameters": spec.input_schema,
            "description": spec.description,
        },
    })
}

/// Build the native Chat Completions body. Everything the caller decided
/// travels; nothing is invented: no `tools` key when no tools were given,
/// no `temperature`/`top_p` when unset (the `AI-ADAPTER-10` regression),
/// the model id verbatim (`AI-ADAPTER-11`), `max_tokens` (the readings
/// record the compat surfaces' spelling, not `max_completion_tokens`), no
/// `stream_options`. `top_k`, thinking budgets, stop sequences, and betas
/// are dropped (no pinned spelling), recorded.
pub(crate) fn chat_request_body(
    request: &myharness_provider_sdk::NativeTurnRequest,
) -> Result<Value, ProviderError> {
    let mut body = json!({
        "model": request.model.model_id,
        "messages": request_messages(&request.system, &request.messages)?,
        "max_tokens": request.max_tokens,
        "stream": request.stream,
    });
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
// The response side: errors, usage, stop reasons, echo
// ---------------------------------------------------------------------------

/// The platform's error envelope (`{"error": {"message", "type", "code", …}}`),
/// the same shape both runtimes' compat surfaces answer with. The mapping is
/// the `TEST-AI-021` vocabulary; the provider's own wording travels and
/// nothing else from the body is echoed (it could quote a credential).
pub(crate) fn error_from_envelope(status: Option<u16>, body: &Value) -> ProviderError {
    let owner = body.get("error").cloned().unwrap_or_else(|| body.clone());
    let kind = owner.get("type").and_then(Value::as_str);
    let code = owner.get("code").and_then(|code| {
        code.as_str()
            .map(str::to_string)
            .or_else(|| code.as_i64().map(|code| code.to_string()))
    });
    let text = owner
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let combined = text.to_lowercase();

    let quota = code.as_deref() == Some("insufficient_quota")
        || kind == Some("insufficient_quota")
        || combined.contains("insufficient quota")
        || combined.contains("billing")
        || combined.contains("exceeded your current quota");
    let context = combined.contains("context length")
        || combined.contains("maximum context")
        || combined.contains("too long");

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
                    .map(status_class)
                    .unwrap_or(ErrorClass::InvalidRequest)
            }
        });
    let message = if text.is_empty() {
        match (kind, code.as_deref()) {
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

/// The status class for an HTTP status with no parseable error envelope
/// (401/403 auth, 429 quota, 404 missing model, 5xx transient, everything
/// else an invalid request).
pub(crate) fn status_class(status: u16) -> ErrorClass {
    match status {
        401 | 403 => ErrorClass::Auth,
        429 => ErrorClass::Quota,
        404 => ErrorClass::ModelNotFound,
        500..=599 => ErrorClass::Transient,
        _ => ErrorClass::InvalidRequest,
    }
}

/// Parse a non-success response when no envelope can be read.
pub(crate) fn error_from_status(status: u16) -> ProviderError {
    ProviderError {
        class: status_class(status),
        status: Some(status),
        message: format!("the provider answered with status {status}"),
    }
}

/// The usage numbers of a response, mapped off this wire's names
/// (`AI-USAGE-01`: exact, separate). Absent means absent, never zero
/// masquerading.
pub(crate) fn parse_usage(usage: &Value) -> Option<Usage> {
    let read = |key: &str| usage.get(key).and_then(Value::as_u64);
    let input = read("prompt_tokens");
    let output = read("completion_tokens");
    let cache_read = usage
        .get("prompt_tokens_details")
        .and_then(|details| details.get("cached_tokens"))
        .and_then(Value::as_u64);
    if input.is_some() || output.is_some() || cache_read.is_some() {
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

/// This wire's stop map (`AI-ADAPTER-08`): stop→`end_turn`,
/// length→`max_tokens`, tool_calls/function_call→`tool_use`,
/// content_filter→`ContentFilter`; unknown spellings stay verbatim.
pub(crate) fn stop_reason(finish: &str) -> NativeStopReason {
    match finish {
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
// The SSE event stream
// ---------------------------------------------------------------------------

/// One parsed SSE frame: the `event:` line (transport spelling) and the
/// `data:` payload. Chat Completions dispatches on the `data` payload's
/// shape; the event line is unused by this grammar.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct WireEvent {
    pub event: Option<String>,
    pub data: String,
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
// The non-streaming completion object
// ---------------------------------------------------------------------------

/// One tool call of a completed choice.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CompletedToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

/// The subset of a completed completion the adapter models.
#[derive(Debug)]
pub(crate) struct ParsedCompletion {
    pub model: Option<String>,
    /// Assistant text (the message's content when it is a string; a null
    /// content with tool calls is the tool-call posture).
    pub text: String,
    /// Reasoning summary text (`delta.reasoning_content`'s non-streaming
    /// sibling when the runtime reports one).
    pub reasoning: String,
    pub tool_calls: Vec<CompletedToolCall>,
    pub finish_reason: Option<String>,
    pub usage: Option<Usage>,
}

/// Parse a completed completion object, tolerantly: unknown message shapes
/// are refused, never silently dropped (`AI-MM-01`).
pub(crate) fn parse_completion(body: &Value) -> Result<ParsedCompletion, ProviderError> {
    if body.get("error").is_some() {
        // A 200 answer that carries an error envelope is the error's own
        // posture — surfaced, never treated as an empty success.
        return Err(error_from_envelope(None, body));
    }
    let choice = body
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .ok_or_else(|| ProviderError {
            class: ErrorClass::InvalidRequest,
            status: None,
            message: "the completion carried no choice".to_string(),
        })?;
    let message = choice.get("message").cloned().unwrap_or_else(|| json!({}));
    let mut text = String::new();
    match message.get("content") {
        Some(Value::String(text_value)) => text.push_str(text_value),
        Some(Value::Array(parts)) => {
            for part in parts {
                if part.get("type").and_then(Value::as_str) == Some("text") {
                    if let Some(fragment) = part.get("text").and_then(Value::as_str) {
                        text.push_str(fragment);
                    }
                } else {
                    return Err(ProviderError {
                        class: ErrorClass::InvalidRequest,
                        status: None,
                        message:
                            "the completion carried a message content part of an unhandled shape"
                                .to_string(),
                    });
                }
            }
        }
        Some(Value::Null) | None => {}
        Some(other) => {
            return Err(ProviderError {
                class: ErrorClass::InvalidRequest,
                status: None,
                message: format!("the completion message content is an unhandled shape: {other:?}"),
            })
        }
    }
    let mut reasoning = String::new();
    if let Some(reasoning_text) = message.get("reasoning_content").and_then(Value::as_str) {
        reasoning.push_str(reasoning_text);
    } else if let Some(reasoning_text) = message.get("reasoning").and_then(Value::as_str) {
        reasoning.push_str(reasoning_text);
    }
    let mut tool_calls = Vec::new();
    for (index, call) in message
        .get("tool_calls")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .enumerate()
    {
        let function = call.get("function").cloned().unwrap_or_else(|| json!({}));
        let name = function
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if name.is_empty() {
            return Err(ProviderError {
                class: ErrorClass::InvalidRequest,
                status: None,
                message:
                    "the response carried a function call without a name; nothing was executed"
                        .to_string(),
            });
        }
        let arguments = function
            .get("arguments")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let id = call
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| format!("call-{index}"));
        tool_calls.push(CompletedToolCall {
            id,
            name,
            arguments,
        });
    }
    Ok(ParsedCompletion {
        model: body
            .get("model")
            .and_then(Value::as_str)
            .map(str::to_string),
        text,
        reasoning,
        tool_calls,
        finish_reason: choice
            .get("finish_reason")
            .and_then(Value::as_str)
            .map(str::to_string),
        usage: body.get("usage").and_then(parse_usage),
    })
}
