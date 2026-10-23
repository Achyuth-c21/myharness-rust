//! The Vertex adapter's native contract — Google's Vertex AI raw-prediction
//! routes whose payload grammar is the Anthropic Messages one
//! (`docs/12` §1 "Anthropic Messages via Vertex"; §8.5 "Anthropic Vertex
//! client / Google auth"). Every spelling below is a recorded reading
//! (`docs/14` §9 item 28); the family's wire was never observed from the
//! shipped app.
//!
//! The request builder turns the app's typed model
//! ([`myharness_provider_sdk::NativeTurnRequest`]) into the **native** body:
//! the Anthropic content-block grammar, with the model carried by the
//! request *path* (no `model` field) and no `anthropic_version`
//! declaration (none is recorded for this family).

use std::time::Duration;

use myharness_provider_sdk::{
    ContentDelta, ErrorClass, NativeStopReason, ProviderError, SystemBlock, ToolSpec, Usage,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// The applied idle default (`docs/12` §4.1's documented 300-second reading;
/// this family's own shipped default was not captured — `docs/14` §9 item
/// 25's reading).
pub const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(300);

/// The one-shot inference probe's verdict — the setup window's *Test
/// connection*. `ok: true` means the host took the request and answered for
/// the requested model; errors surface as `ProviderError`.
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

/// The Vertex client configuration, read from the profile's connection
/// section by the caller. The adapter owns only the wire policy.
#[derive(Debug, Clone, PartialEq)]
pub struct VertexClientConfig {
    /// The regional endpoint; the form's `inferenceVertexBaseUrl` override
    /// replaces the region-derived `https://{region}-aiplatform.googleapis.com`.
    /// Trailing slashes are normalized (`docs/12` §3.1).
    pub base_url: String,
    /// The GCP project id (the request path carries it).
    pub project_id: String,
    /// The GCP region (the request path carries it).
    pub region: String,
    /// Routing/tenant headers (`docs/09` §2.2 custom headers).
    pub custom_headers: Vec<(String, String)>,
    /// The idle window for streamed turns (`TEST-AI-016`; the reading above).
    pub idle_timeout: Duration,
}

// ---------------------------------------------------------------------------
// The request side: the native body and the model path
// ---------------------------------------------------------------------------

fn system_block(block: &SystemBlock) -> Value {
    let mut out = json!({ "type": "text", "text": block.text });
    if let Some(control) = &block.cache_control {
        out["cache_control"] = json!(control);
    }
    out
}

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
fn content_part(part: &myharness_provider_sdk::ContentPart) -> Result<Value, ProviderError> {
    Ok(match part {
        myharness_provider_sdk::ContentPart::Text { text } => {
            json!({ "type": "text", "text": text })
        }
        myharness_provider_sdk::ContentPart::Image { media_type, data } => json!({
            "type": "image",
            "source": { "type": "base64", "media_type": media_type, "data": data },
        }),
        myharness_provider_sdk::ContentPart::Document {
            media_type, data, ..
        } => json!({
            "type": "document",
            "source": { "type": "base64", "media_type": media_type, "data": data },
        }),
        myharness_provider_sdk::ContentPart::ToolUse { id, name, input } => json!({
            "type": "tool_use", "id": id, "name": name, "input": input,
        }),
        myharness_provider_sdk::ContentPart::ToolResult {
            tool_use_id,
            content,
            is_error,
            cache_control,
        } => {
            let mut out = json!({
                "type": "tool_result",
                "tool_use_id": tool_use_id,
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
        myharness_provider_sdk::ContentPart::Reasoning {
            thinking,
            signature,
        } => {
            let mut out = json!({ "type": "thinking", "thinking": thinking });
            if let Some(signature) = signature {
                out["signature"] = json!(signature);
            }
            out
        }
        myharness_provider_sdk::ContentPart::RedactedThinking { data } => {
            json!({ "type": "redacted_thinking", "data": data })
        }
    })
}

/// The message array of a request, natively.
fn messages_array(
    messages: &[(myharness_provider_sdk::Role, Vec<myharness_provider_sdk::ContentPart>)],
) -> Result<Vec<Value>, ProviderError> {
    use myharness_provider_sdk::Role;
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

/// Build the native raw-prediction body: the Anthropic Messages grammar with
/// **no `model` field** (the request path carries it) and **no
/// `anthropic_version`** (none is recorded for this family — `docs/14` §9
/// item 28). Everything the caller decided travels; nothing is invented.
pub(crate) fn raw_predict_body(
    request: &myharness_provider_sdk::NativeTurnRequest,
) -> Result<Value, ProviderError> {
    let mut body = json!({
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

/// Percent-encode a path segment: the unreserved byte set travels, every
/// other byte escapes — including `:`, so no identifier can be confused
/// with the route verb's separator (only the fixed `:rawPredict`/
/// `:streamRawPredict` suffix carries a literal colon).
pub(crate) fn encode_path_segment(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for byte in segment.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// The raw-prediction request URL
/// (`{base}/v1/projects/{project}/locations/{region}/publishers/google/models/{model}:rawPredict`);
/// the `:` that separates the verb travels literally *outside* the encoded
/// segments (`docs/14` §9 item 28).
pub(crate) fn raw_predict_url(config: &VertexClientConfig, model: &str) -> String {
    format!(
        "{}/v1/projects/{}/locations/{}/publishers/google/models/{}:rawPredict",
        config.base_url.trim_end_matches('/'),
        encode_path_segment(&config.project_id),
        encode_path_segment(&config.region),
        encode_path_segment(model),
    )
}

/// The streaming variant's URL (`…:streamRawPredict`).
pub(crate) fn raw_predict_stream_url(config: &VertexClientConfig, model: &str) -> String {
    format!(
        "{}/v1/projects/{}/locations/{}/publishers/google/models/{}:streamRawPredict",
        config.base_url.trim_end_matches('/'),
        encode_path_segment(&config.project_id),
        encode_path_segment(&config.region),
        encode_path_segment(model),
    )
}

// ---------------------------------------------------------------------------
// The response side: errors
// ---------------------------------------------------------------------------

/// The bridge vocabulary's status class (an HTTP status with no parseable
/// error envelope).
pub(crate) fn status_class(status: u16) -> ErrorClass {
    match status {
        401 | 403 => ErrorClass::Auth,
        429 => ErrorClass::Quota,
        404 => ErrorClass::ModelNotFound,
        500..=599 => ErrorClass::Transient,
        _ => ErrorClass::InvalidRequest,
    }
}

/// Parse what a provider error means (the `TEST-AI-021` mapping) off a
/// Google-shaped error envelope — the `.error` object's `code`, `message`,
/// and `status`; the shape-tolerant hunt keeps reading when the exact face
/// differs. The message keeps the provider's wording; nothing else from the
/// body is echoed.
pub(crate) fn error_from_envelope(status: Option<u16>, body: &Value) -> ProviderError {
    let error = body.get("error").and_then(Value::as_object);
    let text = error
        .and_then(|error| error.get("message"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let reason = error
        .and_then(|error| error.get("status"))
        .and_then(Value::as_str);
    let class = match reason {
        Some("UNAUTHENTICATED" | "PERMISSION_DENIED" | "UNAUTHENTICATED_") => ErrorClass::Auth,
        Some("RESOURCE_EXHAUSTED") => ErrorClass::Quota,
        Some("NOT_FOUND") => ErrorClass::ModelNotFound,
        Some("INVALID_ARGUMENT" | "FAILED_PRECONDITION") => ErrorClass::InvalidRequest,
        Some("UNAVAILABLE" | "INTERNAL" | "DEADLINE_EXCEEDED" | "ABORTED") => ErrorClass::Transient,
        _ => match status {
            Some(status) => status_class(status),
            None => ErrorClass::InvalidRequest,
        },
    };
    let message = if text.is_empty() {
        match reason {
            Some(reason) => format!("the provider reported the status {reason:?}"),
            None => "the provider reported an error without a message body".to_string(),
        }
    } else {
        text
    };
    ProviderError { class, status, message }
}

/// Parse what a non-success response means when no envelope can be read.
pub(crate) fn error_from_status(status: u16) -> ProviderError {
    ProviderError {
        class: status_class(status),
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

// ---------------------------------------------------------------------------
// The stream side: data lines → typed stream events
// ---------------------------------------------------------------------------

/// One parsed stream event, transport-shaped — the app's typed events come
/// out of the state machine in [`crate::adapter`].
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum StreamEvent {
    /// A transport keep-alive; not progress (the idle-window reading above).
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

/// Construe one stream-event payload into a typed stream event. Unknown
/// event kinds surface as unsupported modalities, never silently dropped
/// (`AI-MM-01`, `AI-STREAM-01`).
pub(crate) fn stream_event_from_value(body: &Value) -> Option<Result<StreamEvent, ProviderError>> {
    let kind = body.get("type").and_then(Value::as_str).unwrap_or("");
    match kind {
        "message_start" => {
            let message = body.get("message").cloned().unwrap_or_else(|| json!({}));
            Some(Ok(StreamEvent::MessageStart {
                model: message
                    .get("model")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                usage: message.get("usage").and_then(parse_usage),
            }))
        }
        "content_block_start" => Some(Ok(StreamEvent::BlockStart {
            index: body.get("index").and_then(Value::as_u64).unwrap_or(0) as usize,
            block: body
                .get("content_block")
                .cloned()
                .unwrap_or_else(|| json!({})),
        })),
        "content_block_delta" => {
            let index = body.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
            let delta = body.get("delta").unwrap_or(&Value::Null);
            let delta_kind = delta.get("type").and_then(Value::as_str).unwrap_or("");
            let parsed = match delta_kind {
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
                    return Some(Err(ProviderError {
                        class: ErrorClass::UnsupportedModality,
                        status: None,
                        message: format!(
                            "the stream carried a content delta of an unhandled kind: {other:?}"
                        ),
                    }))
                }
            };
            Some(Ok(StreamEvent::Delta {
                index,
                delta: parsed,
            }))
        }
        "content_block_stop" => Some(Ok(StreamEvent::BlockStop {
            index: body.get("index").and_then(Value::as_u64).unwrap_or(0) as usize,
        })),
        "message_delta" => {
            let delta = body.get("delta").cloned().unwrap_or_else(|| json!({}));
            Some(Ok(StreamEvent::MessageDelta {
                stop_reason: delta
                    .get("stop_reason")
                    .and_then(Value::as_str)
                    .map(NativeStopReason::parse),
                stop_sequence: delta
                    .get("stop_sequence")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                usage: body.get("usage").and_then(parse_usage),
            }))
        }
        "message_stop" => Some(Ok(StreamEvent::MessageStop)),
        "ping" => Some(Ok(StreamEvent::Ping)),
        "error" => {
            let error = body.get("error").cloned().unwrap_or_else(|| json!({}));
            Some(Ok(StreamEvent::Error {
                kind: error
                    .get("type")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                message: error
                    .get("message")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            }))
        }
        "" => Some(Err(ProviderError {
            class: ErrorClass::InvalidRequest,
            status: None,
            message: "the stream carried an event without a type".to_string(),
        })),
        other => Some(Err(ProviderError {
            class: ErrorClass::UnsupportedModality,
            status: None,
            message: format!("the stream carried an event of an unhandled kind: {other:?}"),
        })),
    }
}
