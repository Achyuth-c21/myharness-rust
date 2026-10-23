//! The Claude API adapter's native Messages contract — `POST /v1/messages`
//! on the first-party host (or the configured base) and the
//! `anthropic-version: 2023-06-01` header family (`docs/12` §1 row
//! `anthropic`; `docs/18` §6; `docs/09` §2.2).
//!
//! The request builder turns the app's typed model
//! ([`myharness_provider_sdk::NativeTurnRequest`]) into the **native** body —
//! the content-block grammar is the Anthropic one, passed through with no
//! flattening (`AI-MM-02`; the `AI-ADAPTER-04/05/06` regressions). The SSE
//! side parses the provider's native event stream into the app's typed set —
//! normal application modeling, not cross-protocol translation (`docs/12`
//! §8.2).
//!
//! Deliberate readings, recorded (`docs/14` §9):
//!
//! - **Idle timeout semantics.** `docs/12` §4.1 records the flow: "A stream
//!   idle timeout applies only to a stream sending keep-alives". The shipped
//!   Claude *API family's* own default was not separately captured; the
//!   rebuild applies the app's documented 300-second reading and records it
//!   (`docs/14` §9). The window arms at stream start, and *content-bearing*
//!   events reset it; `ping` frames keep the transport alive but are not
//!   progress — a stream that sends only pings times out (`TEST-AI-016`).
//! - **Token counting.** The endpoint is `POST /v1/messages/count_tokens`.
//!   When the host declines the endpoint the adapter errors; it never
//!   estimates (`AI-ADAPTER-09`).
//! - **Model echo.** `message_start` carries the model that was actually
//!   run. When it differs from the requested id, the stream refuses to
//!   continue: substitution is a user-visible event, never something to ride
//!   through (`AI-MODEL-01`, `AI-ADAPTER-11`).

use std::str;
use std::time::Duration;

use myharness_provider_sdk::{
    ContentDelta, ContentPart, ErrorClass, NativeStopReason, ProviderError, Role, SystemBlock,
    ToolSpec, Usage,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// The applied idle default (`docs/12` §4.1's documented 300-second reading;
/// the family's own shipped default was not captured — `docs/14` §9).
pub const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(300);

/// The one-shot inference probe's verdict (the setup window's *Test
/// connection*). `ok: true` means the host took the request and answered for
/// the requested model; errors surface as `ProviderError` instead.
///
/// The field set is the renderer's (`apps/desktop/src/setup/client.ts`
/// `ProbeStatus`), shared across probe-capable families so the setup window
/// stays family-agnostic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeStatus {
    pub ok: bool,
    /// The model the provider says it ran (== the requested id, else error).
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

/// The Claude API client configuration, read from the profile's connection
/// section by the caller. The adapter owns only the wire policy.
#[derive(Debug, Clone, PartialEq)]
pub struct AnthropicClientConfig {
    /// The base URL; `None` means the first-party default host
    /// (`docs/12` §1). Trailing slashes are normalized (`docs/12` §3.1).
    pub base_url: Option<String>,
    /// Routing/tenant headers (`docs/09` §2.2 "custom headers").
    pub custom_headers: Vec<(String, String)>,
    /// The idle window for streamed turns (`TEST-AI-016`; the reading above).
    pub idle_timeout: Duration,
    /// Anthropic beta identifiers, forwarded on every request
    /// (`AI-ADAPTER-07`: "does not forward Anthropic beta headers" is a
    /// recorded violation).
    pub betas: Vec<String>,
}

// ---------------------------------------------------------------------------
// The request side: the native body
// ---------------------------------------------------------------------------

/// Serialize a system block natively (prompt-caching controls preserved —
/// `AI-ADAPTER-05`).
fn system_block(block: &SystemBlock) -> Value {
    let mut out = json!({ "type": "text", "text": block.text });
    if let Some(control) = &block.cache_control {
        out["cache_control"] = json!(control);
    }
    out
}

/// Serialize a tool natively (`AI-TOOL-01`).
fn tool(spec: &ToolSpec) -> Value {
    let mut out = json!({
        "name": spec.name,
        "input_schema": spec.input_schema,
    });
    if let Some(description) = &spec.description {
        out["description"] = json!(description);
    }
    if let Some(control) = &spec.cache_control {
        out["cache_control"] = json!(control);
    }
    out
}

/// Serialize one content part natively — 1:1 with the Anthropic block types,
/// nothing flattened (`AI-MM-02`; the `AI-ADAPTER-04/05/06` regressions).
fn content_part(part: &ContentPart) -> Result<Value, ProviderError> {
    Ok(match part {
        ContentPart::Text { text } => json!({ "type": "text", "text": text }),
        ContentPart::Image { media_type, data } => json!({
            "type": "image",
            "source": { "type": "base64", "media_type": media_type, "data": data },
        }),
        ContentPart::Document {
            media_type, data, ..
        } => json!({
            "type": "document",
            "source": { "type": "base64", "media_type": media_type, "data": data },
        }),
        ContentPart::ToolUse { id, name, input } => json!({
            "type": "tool_use", "id": id, "name": name, "input": input,
        }),
        ContentPart::ToolResult {
            tool_use_id,
            content,
            is_error,
            cache_control,
        } => {
            let mut out = json!({
                "type": "tool_result",
                "tool_use_id": tool_use_id,
                // The parts travel as a block array: images and documents stay
                // images and documents (the `AI-ADAPTER-06` regression).
                "content": content
                    .iter()
                    .map(content_part)
                    .collect::<Result<Vec<_>, _>>()?,
            });
            if *is_error {
                out["is_error"] = json!(true);
            }
            if let Some(control) = cache_control {
                out["cache_control"] = json!(control);
            }
            out
        }
        ContentPart::Reasoning {
            thinking,
            signature,
        } => {
            let mut out = json!({ "type": "thinking", "thinking": thinking });
            if let Some(signature) = signature {
                out["signature"] = json!(signature);
            }
            out
        }
        ContentPart::RedactedThinking { data } => {
            json!({ "type": "redacted_thinking", "data": data })
        }
    })
}

/// The message array of a request, natively.
fn messages_array(messages: &[(Role, Vec<ContentPart>)]) -> Result<Vec<Value>, ProviderError> {
    messages
        .iter()
        .map(|(role, parts)| -> Result<Value, ProviderError> {
            Ok(json!({
                "role": match role {
                    Role::User => "user",
                    Role::Assistant => "assistant",
                },
                "content": parts
                    .iter()
                    .map(content_part)
                    .collect::<Result<Vec<_>, _>>()?,
            }))
        })
        .collect()
}

/// Build the native `/v1/messages` body.
///
/// Everything the caller decided travels; nothing is invented: no `tools` key
/// when no tools were given, no `temperature` when unset (the
/// `AI-ADAPTER-10` regression), the model id verbatim (`AI-ADAPTER-11`).
pub(crate) fn messages_request_body(
    request: &myharness_provider_sdk::NativeTurnRequest,
) -> Result<Value, ProviderError> {
    let mut body = json!({
        "model": request.model.model_id,
        "max_tokens": request.max_tokens,
        "stream": request.stream,
        "messages": messages_array(&request.messages)?,
    });
    if !request.system.is_empty() {
        body["system"] = json!(request.system.iter().map(system_block).collect::<Vec<_>>());
    }
    if !request.tools.is_empty() {
        body["tools"] = json!(request.tools.iter().map(tool).collect::<Vec<_>>());
    }
    if let Some(temperature) = request.temperature {
        body["temperature"] = json!(temperature);
    }
    if let Some(top_p) = request.top_p {
        body["top_p"] = json!(top_p);
    }
    if let Some(top_k) = request.top_k {
        body["top_k"] = json!(top_k);
    }
    if !request.stop_sequences.is_empty() {
        body["stop_sequences"] = json!(request.stop_sequences);
    }
    if let Some(thinking) = request.thinking {
        body["thinking"] = json!({
            "type": "enabled",
            "budget_tokens": thinking.budget_tokens,
        });
    }
    Ok(body)
}

/// Build the native `/v1/messages/count_tokens` body (the same request
/// grammar, no output budget).
pub(crate) fn count_tokens_request_body(
    request: &myharness_provider_sdk::CountRequest,
) -> Result<Value, ProviderError> {
    let mut body = json!({
        "model": request.model.model_id,
        "messages": messages_array(&request.messages)?,
    });
    if !request.system.is_empty() {
        body["system"] = json!(request.system.iter().map(system_block).collect::<Vec<_>>());
    }
    if !request.tools.is_empty() {
        body["tools"] = json!(request.tools.iter().map(tool).collect::<Vec<_>>());
    }
    Ok(body)
}

// ---------------------------------------------------------------------------
// The response side: the SSE parser
// ---------------------------------------------------------------------------

/// Parse what a provider error envelope means (the `TEST-AI-021` mapping).
/// The message keeps the provider's own error wording; nothing else from the
/// body is echoed (it could carry a credential the provider chose to quote).
pub(crate) fn error_from_envelope(status: Option<u16>, body: &Value) -> ProviderError {
    let owner = body.get("error").cloned().unwrap_or_else(|| body.clone());
    let kind = owner.get("type").and_then(Value::as_str); // invalid_request_error, …
    let text = owner
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let class = match kind {
        Some("authentication_error") | Some("permission_error") => ErrorClass::Auth,
        Some("rate_limit_error") => ErrorClass::Quota,
        Some("not_found_error") => ErrorClass::ModelNotFound,
        Some("overloaded_error") | Some("api_error") => ErrorClass::Transient,
        Some("invalid_request_error") if text.contains("context") || text.contains("too long") => {
            ErrorClass::ContextLength
        }
        _ => match status {
            Some(status) => crate::discovery::status_class(status),
            None => ErrorClass::InvalidRequest,
        },
    };
    let message = if text.is_empty() {
        match kind {
            Some(kind) => format!("the provider reported an error of type {kind:?}"),
            None => "the provider reported an error without a message body".to_string(),
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

/// Parse what a non-success response means when no envelope can be read.
pub(crate) fn error_from_status(status: u16) -> ProviderError {
    ProviderError {
        class: crate::discovery::status_class(status),
        status: Some(status),
        message: format!("the provider answered with status {status}"),
    }
}

/// The usage numbers of a message event, mapped 1:1 off the native names
/// (`AI-USAGE-01`: exact, separate). A usage object is usable when it carries
/// any known field — a final `message_delta` often carries only
/// `output_tokens`.
pub(crate) fn parse_usage(usage: &Value) -> Option<Usage> {
    let read = |key: &str| usage.get(key).and_then(Value::as_u64);
    let input = read("input_tokens");
    let output = read("output_tokens");
    let cache_read = read("cache_read_input_tokens");
    let cache_write = read("cache_creation_input_tokens");
    let web_search = read("web_search_input_tokens");
    let known = input.is_some()
        || output.is_some()
        || cache_read.is_some()
        || cache_write.is_some()
        || web_search.is_some();
    known.then_some(Usage {
        input: input.unwrap_or_default(),
        output: output.unwrap_or_default(),
        cache_read,
        cache_write,
        web_search,
    })
}

/// One parsed stream event, transport-shaped — the app's typed events come
/// out of the state machine in [`crate::adapter`].
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum StreamEvent {
    /// `ping` — the transport signalled a keep-alive; not progress (the
    /// idle-window reading above).
    Ping,
    /// `message_start` — the stream opened; carries the model id actually
    /// addressed and the opening usage.
    MessageStart {
        model: Option<String>,
        usage: Option<Usage>,
    },
    /// `content_block_start`.
    BlockStart { index: usize, block: Value },
    /// `content_block_delta` with a typed delta.
    Delta { index: usize, delta: ContentDelta },
    /// `content_block_stop`.
    BlockStop { index: usize },
    /// `message_delta` — the stop reason (preserved 1:1) and final usage.
    MessageDelta {
        stop_reason: Option<NativeStopReason>,
        stop_sequence: Option<String>,
        usage: Option<Usage>,
    },
    /// `message_stop`.
    MessageStop,
    /// `error` — a mid- or terminal-stream error event.
    Error {
        kind: Option<String>,
        message: Option<String>,
    },
}

fn from_json_error(error: serde_json::Error) -> ProviderError {
    ProviderError {
        class: ErrorClass::InvalidRequest,
        status: None,
        message: format!("the stream carried a frame that is not valid JSON: {error}"),
    }
}

/// Feed raw bytes in; run complete frames out. Handles `event:`/`data:`
/// pairs, comments (`: blink`), CRLF, and multi-line `data:`.
pub(crate) struct SseParser {
    buffer: String,
}

impl SseParser {
    pub(crate) fn new() -> Self {
        SseParser {
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
        let mut event = "message".to_string();
        let mut data = String::new();
        for line in frame.lines() {
            if let Some(name) = line.strip_prefix("event:") {
                event = name.trim().to_string();
            } else if let Some(payload) = line.strip_prefix("data:") {
                // Multiple data lines join with \n (SSE framing).
                if !data.is_empty() {
                    data.push('\n');
                }
                data.push_str(payload);
            }
            // Comment lines have no prefix and fall through.
        }
        if data.is_empty() {
            // An event-less or empty frame is a transport keep-alive.
            if event == "message" {
                return Ok(StreamEvent::Ping);
            }
            // A named event with no payload parses as an empty object —
            // `event: message_stop` travels without `data:` in some
            // SDK-identical servers; the event name carries the meaning.
            return self.parse_data(&event, "{}");
        }
        self.parse_data(&event, &data)
    }

    fn parse_data(&self, event: &str, data: &str) -> Result<StreamEvent, ProviderError> {
        match event {
            "ping" => Ok(StreamEvent::Ping),
            "message_start" => {
                let body: Value = serde_json::from_str(data).map_err(from_json_error)?;
                let message = body.get("message").cloned().unwrap_or_else(|| json!({}));
                Ok(StreamEvent::MessageStart {
                    model: message
                        .get("model")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    usage: message.get("usage").and_then(parse_usage),
                })
            }
            "content_block_start" => {
                let body: Value = serde_json::from_str(data).map_err(from_json_error)?;
                Ok(StreamEvent::BlockStart {
                    index: body.get("index").and_then(Value::as_u64).unwrap_or(0) as usize,
                    block: body
                        .get("content_block")
                        .cloned()
                        .unwrap_or_else(|| json!({})),
                })
            }
            "content_block_delta" => {
                let body: Value = serde_json::from_str(data).map_err(from_json_error)?;
                let index = body.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
                let delta = body.get("delta").unwrap_or(&Value::Null);
                let kind = delta.get("type").and_then(Value::as_str).unwrap_or("");
                let parsed = match kind {
                    "text_delta" => ContentDelta::Text {
                        text: delta
                            .get("text")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                    },
                    "input_json_delta" => ContentDelta::InputJson {
                        partial_json: delta
                            .get("partial_json")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                    },
                    "thinking_delta" => ContentDelta::Thinking {
                        thinking: delta
                            .get("thinking")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                    },
                    "signature_delta" => ContentDelta::Signature {
                        signature: delta
                            .get("signature")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                    },
                    "citations_delta" => ContentDelta::Citation {
                        citation: delta.get("citation").cloned().unwrap_or_else(|| json!({})),
                    },
                    other => {
                        // Unknown delta kinds are named, never dropped
                        // (`AI-MM-01`, `AI-STREAM-01`).
                        return Err(ProviderError {
                            class: ErrorClass::UnsupportedModality,
                            status: None,
                            message: format!(
                                "the stream carried a content delta of an unhandled kind: {other:?}"
                            ),
                        });
                    }
                };
                Ok(StreamEvent::Delta {
                    index,
                    delta: parsed,
                })
            }
            "content_block_stop" => {
                let body: Value = serde_json::from_str(data).map_err(from_json_error)?;
                Ok(StreamEvent::BlockStop {
                    index: body.get("index").and_then(Value::as_u64).unwrap_or(0) as usize,
                })
            }
            "message_delta" => {
                let body: Value = serde_json::from_str(data).map_err(from_json_error)?;
                let delta = body.get("delta").cloned().unwrap_or_else(|| json!({}));
                Ok(StreamEvent::MessageDelta {
                    stop_reason: delta
                        .get("stop_reason")
                        .and_then(Value::as_str)
                        .map(NativeStopReason::parse),
                    stop_sequence: delta
                        .get("stop_sequence")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    usage: body.get("usage").and_then(parse_usage),
                })
            }
            "message_stop" => Ok(StreamEvent::MessageStop),
            "error" => {
                let body: Value = serde_json::from_str(data).map_err(from_json_error)?;
                let error = body.get("error").cloned().unwrap_or_else(|| json!({}));
                Ok(StreamEvent::Error {
                    kind: error
                        .get("type")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    message: error
                        .get("message")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                })
            }
            other => Err(ProviderError {
                class: ErrorClass::UnsupportedModality,
                status: None,
                message: format!("the stream carried an event of an unhandled kind: {other:?}"),
            }),
        }
    }
}

/// Split the next complete frame off the buffer. `Some(frame)` when a frame
/// completed, `None` while the buffer still has no complete frame.
fn next_frame(buffer: &mut String) -> Option<String> {
    // Frames are separated by a blank line; tolerate \n\n and \r\n\r\n.
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
