//! The Bedrock adapter's native contract — the Bedrock Runtime invoke
//! surface whose payload grammar is the Anthropic Messages one, with the
//! family's `anthropic_version` body field (`docs/12` §1 "Anthropic Messages
//! via Bedrock Runtime"; §8.5 "AWS SDK Bedrock Runtime stream, Anthropic
//! body").
//!
//! The request builder turns the app's typed model
//! ([`myharness_provider_sdk::NativeTurnRequest`]) into the **native** body —
//! the Anthropic content-block grammar passed through with no flattening
//! (`AI-MM-02`; the `AI-ADAPTER-04/05/06` regressions). The model id travels
//! in the request path, not the body (the path selects the model the runtime
//! addresses); the body carries the family's `anthropic_version`
//! declaration.
//!
//! Deliberate readings, recorded (`docs/14` §9):
//!
//! - **The path is the model.** The request answers
//!   `POST {base}/model/{model-or-arn}/invoke[-with-response-stream]`; the
//!   path segment keeps the ARN's `:` separators and percent-encodes
//!   everything outside the unreserved set (so an inference-profile ARN's
//!   `/` cannot split the path).
//! - **The override scope.** The form records one optional base URL for the
//!   family (`inferenceBedrockBaseUrl`); it replaces the runtime endpoint,
//!   and, having no other recorded control-plane key, it is also where the
//!   model listing is answered when it is set. Recorded as an approved
//!   decision.
//! - **Idle timeout semantics.** The window arms at stream start and
//!   content-bearing events reset it; transport keep-alives (empty-payload
//!   frames) never do — a stream that sends only keep-alives times out
//!   (`TEST-AI-016`).
//! - **Model echo, conditionally.** An answer that names a model different
//!   from the request ends the turn visibly (`AI-MODEL-01`, `AI-ADAPTER-11`);
//!   a Bedrock answer that carries no model field is not an echo failure —
//!   the field's presence is provider-dependent, and inventing a comparison
//!   is a substitution-by-reading.
//! - **No native token count.** The recorded family surface exposes no
//!   token-count endpoint; the adapter errors instead of estimating
//!   (`AI-ADAPTER-09`).

use std::time::Duration;

use myharness_provider_sdk::{
    ContentDelta, ContentPart, ErrorClass, NativeStopReason, ProviderError, Role, SystemBlock,
    ToolSpec, Usage,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::eventstream::Frame;

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

/// The Bedrock client configuration, read from the profile's connection
/// section by the caller. The adapter owns only the wire policy.
#[derive(Debug, Clone, PartialEq)]
pub struct BedrockClientConfig {
    /// The runtime endpoint; an explicit override replaces the
    /// `bedrock-runtime.<region>.amazonaws.com` default. Trailing slashes are
    /// normalized (`docs/12` §3.1), and the override is where the model
    /// listing is answered too (`docs/14` §9).
    pub runtime_base_url: String,
    /// The control-plane endpoint the listing uses (the override above when
    /// one is configured, else the region-derived `bedrock.<region>` host).
    pub control_base_url: String,
    /// Routing/tenant headers (`docs/09` §2.2 custom headers).
    pub custom_headers: Vec<(String, String)>,
    /// The idle window for streamed turns (`TEST-AI-016`; the reading above).
    pub idle_timeout: Duration,
}

// ---------------------------------------------------------------------------
// The request side: the native body and the model path
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

/// Build the native `invoke` body: the Anthropic Messages grammar with the
/// family's `anthropic_version` and **no `model` field** — the request path
/// carries the model (`docs/14` §9). Everything the caller decided travels;
/// nothing is invented: no `tools` key when no tools were given, no
/// `temperature` when unset (the `AI-ADAPTER-10` regression).
pub(crate) fn invoke_body(
    request: &myharness_provider_sdk::NativeTurnRequest,
) -> Result<Value, ProviderError> {
    let mut body = json!({
        "anthropic_version": crate::ANTHROPIC_VERSION_BEDROCK,
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

/// The request path's model segment: the id or ARN, with the ARN's `:`
/// separators kept (they cannot split a path) and every other unsafe byte —
/// notably `/`, which would split the path — percent-encoded (`docs/14` §9).
pub(crate) fn encode_model_segment(model: &str) -> String {
    let mut out = String::with_capacity(model.len());
    for byte in model.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b':' | b';' => {
                out.push(byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
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
/// Bedrock exception envelope — the exception name under `__type`/`type`/
/// `reason`, the wording under `message`. The message keeps the provider's
/// own error wording; nothing else from the body is echoed (it could carry a
/// credential the provider chose to quote).
pub(crate) fn error_from_envelope(status: Option<u16>, body: &Value) -> ProviderError {
    // Some error faces wrap the real envelope under `Output`; the owner of
    // the exception fields is whichever face actually carries them.
    let owner = match body.get("Output") {
        Some(output) if output.is_object() => output.clone(),
        _ => body.clone(),
    };
    let kind = ["__type", "type", "reason"]
        .iter()
        .find_map(|key| owner.get(*key).and_then(Value::as_str));
    let text = owner
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let class = match kind {
        Some(kind) => exception_class(kind, status),
        None => match status {
            Some(status) => status_class(status),
            None => ErrorClass::InvalidRequest,
        },
    };
    let message = if text.is_empty() {
        match kind {
            Some(kind) => format!("the provider reported the exception {kind:?}"),
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

/// The exception-name mapping (Bedrock spellings are name-indexed
/// exceptions; the suffixes are the reconstructive reading — see
/// `docs/14` §9).
fn exception_class(kind: &str, status: Option<u16>) -> ErrorClass {
    if kind.contains("AccessDenied")
        || kind.contains("UnrecognizedClient")
        || kind.contains("InvalidSignature")
        || kind.contains("ExpiredToken")
        || kind.contains("SecurityToken")
        || kind.contains("InvalidToken")
    {
        ErrorClass::Auth
    } else if kind.contains("Throttled") || kind.contains("TooManyRequests") {
        ErrorClass::Quota
    } else if kind.contains("ModelNotAuthorized")
        || kind.contains("ResourceNotFound")
        || kind.contains("ModelNotFound")
        || kind.contains("NotFound")
    {
        ErrorClass::ModelNotFound
    } else if kind.contains("ServiceUnavailable")
        || kind.contains("InternalServer")
        || kind.contains("InternalFailure")
        || kind.contains("ModelError")
    {
        ErrorClass::Transient
    } else if kind.contains("Validation") {
        ErrorClass::InvalidRequest
    } else {
        match status {
            Some(status) => status_class(status),
            None => ErrorClass::InvalidRequest,
        }
    }
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
// The stream side: frames → typed stream events
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

fn from_json_error(error: serde_json::Error) -> ProviderError {
    ProviderError {
        class: ErrorClass::InvalidRequest,
        status: None,
        message: format!("the stream carried a payload that is not valid JSON: {error}"),
    }
}

/// Construe one event-stream frame into a typed stream event.
///
/// `:message-type: exception` frames are terminal errors. Empty payloads are
/// transport keep-alives regardless of the event name — never progress
/// (`TEST-AI-016`). A named event with a non-empty payload that is not part
/// of the grammar surfaces as an unsupported event, never a silent drop
/// (`AI-MM-01`, `AI-STREAM-01`).
pub(crate) fn frame_to_stream_event(frame: &Frame) -> Option<Result<StreamEvent, ProviderError>> {
    if frame.payload.is_empty() {
        // An empty payload is a transport keep-alive, whatever else the
        // frame says.
        return Some(Ok(StreamEvent::Ping));
    }
    let Ok(payload) = std::str::from_utf8(&frame.payload) else {
        return Some(Err(ProviderError {
            class: ErrorClass::InvalidRequest,
            status: None,
            message: "the stream carried bytes that are not valid UTF-8".to_string(),
        }));
    };
    if frame.message_type() == Some("exception") {
        let body: Value = match serde_json::from_str(payload) {
            Ok(body) => body,
            Err(error) => return Some(Err(from_json_error(error))),
        };
        return Some(Err(error_from_envelope(None, &body)));
    }
    let event = match frame.event_type() {
        Some(event) => event.to_string(),
        None => {
            // A payload-carrying frame with no event name is not part of the
            // family's grammar: surface it, never guess.
            return Some(Err(ProviderError {
                class: ErrorClass::UnsupportedModality,
                status: None,
                message: "the stream carried an event frame without an event type".to_string(),
            }));
        }
    };
    let body: Value = match serde_json::from_str(payload) {
        Ok(body) => body,
        Err(error) => return Some(Err(from_json_error(error))),
    };
    match event.as_str() {
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
                    return Some(Err(ProviderError {
                        class: ErrorClass::UnsupportedModality,
                        status: None,
                        message: format!(
                            "the stream carried a content delta of an unhandled kind: {other:?}"
                        ),
                    }));
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
        other => Some(Err(ProviderError {
            class: ErrorClass::UnsupportedModality,
            status: None,
            message: format!("the stream carried an event of an unhandled kind: {other:?}"),
        })),
    }
}
