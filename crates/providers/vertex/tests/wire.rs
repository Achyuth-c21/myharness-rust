//! Mock-provider wire fixtures — the conformance battery for the Vertex
//! adapter: `TEST-AI-003`'s family coverage (`TEST-AI-001`'s shape), the
//! applicable members of `TEST-AI-006/007/008/010/011/012/013/014/015/016/
//! 022/023`, and the regression assertions for `AI-ADAPTER-01..14`
//! (`docs/12` §7), applied to this family's native raw-prediction surface.
//!
//! Fixture policy (`tests/README.md`): every credential here is a synthetic
//! placeholder, every model id is a family-legal `claude-…` spelling,
//! nothing is recorded from a real session.

// `create_turn`'s Ok lane is an opaque stream handle. `expect_err` would
// demand `Debug` on it, so stream-returning sites keep `.err().expect(..)`.
#![allow(clippy::err_expect)]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::Duration;

use futures::StreamExt;
use myharness_provider_sdk::ProviderAdapter;
use myharness_provider_sdk::{
    AuthContext, ContentPart, CountRequest, ErrorClass, ModelKey, NativeStopReason,
    NativeTurnRequest, Role, SystemBlock, ToolSpec, TurnEvent, Usage,
};
use secrecy::SecretString;
use serde_json::{json, Value};

use myharness_provider_vertex::{ProbeStatus, VertexAdapter, VertexClientConfig};

// ---------------------------------------------------------------------------
// The mock provider
// ---------------------------------------------------------------------------

/// One canned text piece with an optional delay before it is written.
struct StreamPiece {
    delay_ms: u64,
    body: String,
}

/// A canned behavior for a single connection.
enum Canned {
    /// A complete JSON response.
    Json(u16, Value),
    /// A bare status with no body.
    Bare(u16),
    /// A data-only SSE response: the pieces are written in order (each
    /// inside its own chunked-encoding frame), and the connection stays
    /// open until the client drops it.
    Sse {
        status: u16,
        pieces: Vec<StreamPiece>,
    },
    /// A keep-alive drip: comment/blank lines every interval, forever.
    Drip {
        interval_ms: u64,
        first: String,
    },
}

/// A recorded request (head + body).
#[derive(Debug, Clone)]
struct Recorded {
    raw: String,
}

impl Recorded {
    fn has_path(&self, path: &str) -> bool {
        self.raw.starts_with(&format!("POST {path} "))
            || self.raw.starts_with(&format!("GET {path} "))
    }

    fn header(&self, name: &str) -> Option<String> {
        let needle = format!("{name}: ");
        self.raw
            .lines()
            .find_map(|line| line.strip_prefix(&needle).map(|value| value.trim()))
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    }

    fn body(&self) -> Value {
        let head_end = self
            .raw
            .find("\r\n\r\n")
            .or_else(|| self.raw.find("\n\n"))
            .map(|i| i + (if self.raw.contains("\r\n\r\n") { 4 } else { 2 }))
            .expect("a recorded request carries a body");
        serde_json::from_str(&self.raw[head_end..]).expect("a recorded JSON body")
    }
}

struct MockProvider {
    base: String,
    requests: std::sync::Arc<std::sync::Mutex<Vec<Recorded>>>,
    stop_flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl MockProvider {
    fn requests(&self) -> Vec<Recorded> {
        self.requests.lock().unwrap().clone()
    }

    /// Release the listener thread.
    fn stop(&self) {
        self.stop_flag
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

impl Drop for MockProvider {
    fn drop(&mut self) {
        self.stop();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn read_request(stream: &mut std::net::TcpStream) -> String {
    let mut buffer = Vec::new();
    loop {
        let mut chunk = [0u8; 8192];
        let Ok(read) = stream.read(&mut chunk) else {
            break;
        };
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
        let text = String::from_utf8_lossy(&buffer).to_string();
        if text.contains("\r\n\r\n") || text.contains("\n\n") {
            let head_end = text
                .find("\r\n\r\n")
                .map(|i| i + 4)
                .or_else(|| text.find("\n\n").map(|i| i + 2));
            let Some(head_end) = head_end else { continue };
            let length = ["content-length", "Content-Length"]
                .iter()
                .find_map(|name| {
                    text.lines()
                        .find_map(|line| {
                            line.strip_prefix(*name).map(|rest| rest.strip_prefix(':'))
                        })
                        .flatten()
                        .and_then(|value| value.trim().parse::<usize>().ok())
                });
            match length {
                Some(length) if text.len() < head_end + length => continue,
                _ => break,
            }
        }
    }
    String::from_utf8_lossy(&buffer).to_string()
}

fn serve(
    listener: TcpListener,
    canned: Vec<Canned>,
    requests: std::sync::Arc<std::sync::Mutex<Vec<Recorded>>>,
    stop_flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    let _ = listener.set_nonblocking(true);
    let mut next = 0usize;
    while !stop_flag.load(std::sync::atomic::Ordering::Relaxed) {
        let Ok((mut stream, _)) = listener.accept() else {
            std::thread::sleep(Duration::from_millis(3));
            continue;
        };
        let raw = read_request(&mut stream);
        requests.lock().unwrap().push(Recorded { raw });
        if next >= canned.len() {
            continue;
        }
        match &canned[next] {
            Canned::Json(..) | Canned::Bare(..) => {
                let _ = stream.write_all(&canned_response(&canned[next]));
                let _ = stream.flush();
            }
            Canned::Sse { status, pieces } => {
                stream_sse(&mut stream, *status, pieces, None, &stop_flag)
            }
            Canned::Drip { interval_ms, first } => stream_sse(
                &mut stream,
                200,
                &[StreamPiece {
                    delay_ms: 0,
                    body: first.clone(),
                }],
                Some(*interval_ms),
                &stop_flag,
            ),
        }
        next += 1;
    }
}

fn provider(canned: Vec<Canned>) -> MockProvider {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().unwrap().port();
    let requests = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let stop_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let handle = std::thread::spawn({
        let requests = requests.clone();
        let stop_flag = stop_flag.clone();
        move || serve(listener, canned, requests, stop_flag)
    });
    MockProvider {
        base: format!("http://127.0.0.1:{port}"),
        requests,
        stop_flag,
        handle: Some(handle),
    }
}

fn canned_response(canned: &Canned) -> Vec<u8> {
    match canned {
        Canned::Json(status, body) => format!(
            "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.to_string().len(),
            body
        )
        .into_bytes(),
        Canned::Bare(status) => format!(
            "HTTP/1.1 {status} OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        )
        .into_bytes(),
        _ => Vec::new(),
    }
}

/// Write a data-only SSE response chunk-by-chunk. `drip_interval_ms` turns
/// the connection into an endless keep-alive drip of SSE comments; the
/// drip checks the stop flag so a lingering drip cannot outlive the test.
fn stream_sse(
    stream: &mut std::net::TcpStream,
    status: u16,
    pieces: &[StreamPiece],
    drip_interval_ms: Option<u64>,
    stop_flag: &std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    let head = format!(
        "HTTP/1.1 {status} OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n"
    );
    if stream.write_all(head.as_bytes()).is_err() {
        return;
    }
    let _ = stream.flush();
    let mut write_chunk = |payload: &[u8]| -> bool {
        let frame = format!("{:x}\r\n", payload.len());
        if stream.write_all(frame.as_bytes()).is_err() {
            return true;
        }
        if stream.write_all(payload).is_err() {
            return true;
        }
        if stream.write_all(b"\r\n").is_err() {
            return true;
        }
        let _ = stream.flush();
        false
    };
    for piece in pieces {
        if piece.delay_ms > 0 {
            std::thread::sleep(Duration::from_millis(piece.delay_ms));
        }
        if piece.body.is_empty() {
            continue;
        }
        if write_chunk(piece.body.as_bytes()) {
            return;
        }
    }
    if let Some(interval_ms) = drip_interval_ms {
        loop {
            if stop_flag.load(std::sync::atomic::Ordering::Relaxed) {
                return;
            }
            std::thread::sleep(Duration::from_millis(interval_ms));
            if write_chunk(b": keepalive\n\n") {
                return;
            }
        }
    }
    let _ = stream.write_all(b"0\r\n\r\n");
    let _ = stream.flush();
}

/// One data-only stream event piece.
fn piece(delay_ms: u64, body: Value) -> StreamPiece {
    StreamPiece {
        delay_ms,
        body: format!("data: {body}\n\n"),
    }
}

fn config(base: String) -> VertexClientConfig {
    VertexClientConfig {
        base_url: base,
        project_id: "synthetic-project".to_string(),
        region: "us-east5".to_string(),
        custom_headers: vec![("X-Tenant".to_string(), "acme".to_string())],
        idle_timeout: Duration::from_millis(150),
    }
}

fn adapter(base: String) -> VertexAdapter {
    VertexAdapter::new(config(base), reqwest::Client::new())
}

fn auth(value: &str) -> AuthContext {
    AuthContext::new(
        SecretString::from(value.to_string()),
        "synthetic google token",
    )
}

fn key(model_id: &str) -> ModelKey {
    ModelKey {
        provider_id: "vertex".to_string(),
        model_id: model_id.to_string(),
        variant: None,
        revision: None,
    }
}

fn stream_request(model_id: &str) -> NativeTurnRequest {
    NativeTurnRequest {
        model: key(model_id),
        system: vec![SystemBlock {
            text: "be brief".to_string(),
            cache_control: Some("ephemeral".to_string()),
        }],
        messages: vec![(
            Role::User,
            vec![ContentPart::Text {
                text: "say hello".to_string(),
            }],
        )],
        tools: Vec::new(),
        max_tokens: 64,
        temperature: Some(0.4),
        top_p: None,
        top_k: None,
        stop_sequences: Vec::new(),
        thinking: None,
        betas: Vec::new(),
        stream: true,
    }
}

/// The legal minimal stream the provider answers a happy turn with.
fn legal_stream(model: &str, text: &str, stop: &str, output: u64) -> Vec<Canned> {
    vec![Canned::Sse {
        status: 200,
        pieces: vec![
            piece(
                0,
                json!({
                    "type": "message_start",
                    "message": { "id": "msg_b", "role": "assistant", "model": model, "type": "message",
                                 "usage": { "input_tokens": 7, "output_tokens": 1 } }
                }),
            ),
            piece(0, json!({ "type": "content_block_start", "index": 0, "content_block": { "type": "text", "text": "" } })),
            piece(0, json!({ "type": "content_block_delta", "index": 0, "delta": { "type": "text_delta", "text": text } })),
            piece(0, json!({ "type": "content_block_stop", "index": 0 })),
            piece(
                0,
                json!({
                    "type": "message_delta",
                    "delta": { "stop_reason": stop, "stop_sequence": null },
                    "usage": { "output_tokens": output }
                }),
            ),
            piece(0, json!({ "type": "message_stop" })),
        ],
    }]
}

// ---------------------------------------------------------------------------
// The fixtures
// ---------------------------------------------------------------------------

/// `TEST-AI-003` applied to this family — the native wire contract:
/// `POST …/publishers/google/models/{model}:streamRawPredict` with Google
/// bearer auth, the model in the path and absent from the body, and **no
/// `anthropic_version`** (`docs/12` §8.5; `docs/14` §9 item 28).
#[tokio::test(flavor = "multi_thread")]
async fn a_streamed_turn_speaks_the_native_wire() {
    let host = provider(legal_stream("claude-sonnet-4-5", "Hello", "end_turn", 12));
    let adapter = adapter(host.base.clone());
    let stream = adapter
        .create_turn(&auth("placeholder-token"), &stream_request("claude-sonnet-4-5"))
        .await
        .expect("the turn must open");
    let events: Vec<TurnEvent> = stream
        .map(|result| match result {
            Ok(event) => event,
            Err(error) => panic!("stream failed: {error:?}"),
        })
        .collect::<Vec<_>>()
        .await;
    assert_eq!(
        events,
        vec![
            TurnEvent::TextDelta {
                text: "Hello".to_string()
            },
            TurnEvent::Usage {
                usage: Usage {
                    input: 7,
                    output: 12,
                    cache_read: None,
                    cache_write: None,
                    web_search: None,
                }
            },
            TurnEvent::Stop {
                reason: NativeStopReason::EndTurn
            },
            TurnEvent::Done,
        ],
        "text, exact usage, native stop — in that order (`AI-USAGE-01`, `AI-STREAM-01`)"
    );
    let requests = host.requests();
    assert_eq!(requests.len(), 1, "exactly one request travels");
    let recorded = &requests[0];
    assert!(
        recorded.has_path(
            "/v1/projects/synthetic-project/locations/us-east5/publishers/google/models/claude-sonnet-4-5:streamRawPredict"
        ),
        "{}",
        recorded.raw
    );
    // Google auth: the resolved token travels as a bearer (`docs/12` §8.5).
    assert!(
        recorded
            .header("authorization")
            .unwrap_or_default()
            .contains("Bearer placeholder-token"),
        "{}",
        recorded.raw
    );
    assert!(
        !recorded.raw.contains("x-api-key"),
        "no first-party spelling on this family: {}",
        recorded.raw
    );
    assert!(
        recorded.header("x-tenant").unwrap_or_default().contains("acme")
            || recorded.raw.contains("X-Tenant: acme"),
        "{}",
        recorded.raw
    );
    // The body is the native grammar — no model field, no anthropic_version,
    // no OpenAI vocabulary anywhere (`AI-ADAPTER-02/10`; `docs/14` §9).
    let body = recorded.body();
    assert!(body.get("model").is_none());
    assert!(body.get("anthropic_version").is_none());
    assert_eq!(body["max_tokens"], 64);
    assert_eq!(body["stream"], true);
    assert_eq!(body["messages"][0]["content"][0]["type"], "text");
    assert!(!recorded.raw.to_lowercase().contains("chat/completions"));
    assert!(!recorded.raw.contains(":rawPredict "), "not the non-stream route");
    assert_eq!(body["system"][0]["cache_control"], "ephemeral");
}

/// `AI-TOOL-01` — tool calls stream their argument fragments and the
/// assembled arguments close the call.
#[tokio::test(flavor = "multi_thread")]
async fn tool_arguments_stream_and_close() {
    let host = provider(vec![Canned::Sse {
        status: 200,
        pieces: vec![
            piece(
                0,
                json!({ "type": "message_start", "message": { "model": "claude-sonnet-4-5" } }),
            ),
            piece(
                0,
                json!({
                    "type": "content_block_start",
                    "index": 0,
                    "content_block": { "type": "tool_use", "id": "toolu_7", "name": "get_weather", "input": {} }
                }),
            ),
            piece(
                0,
                json!({
                    "type": "content_block_delta",
                    "index": 0,
                    "delta": { "type": "input_json_delta", "partial_json": "{\"city\": " }
                }),
            ),
            piece(
                0,
                json!({
                    "type": "content_block_delta",
                    "index": 0,
                    "delta": { "type": "input_json_delta", "partial_json": "\"Osaka\"}" }
                }),
            ),
            piece(0, json!({ "type": "content_block_stop", "index": 0 })),
            piece(
                0,
                json!({ "type": "message_delta", "delta": { "stop_reason": "tool_use" }, "usage": { "output_tokens": 9 } }),
            ),
            piece(0, json!({ "type": "message_stop" })),
        ],
    }]);
    let mut request = stream_request("claude-sonnet-4-5");
    request.tools = vec![ToolSpec {
        name: "get_weather".to_string(),
        description: None,
        input_schema: json!({ "type": "object", "properties": {} }),
        cache_control: None,
    }];
    let events: Vec<TurnEvent> = adapter(host.base.clone())
        .create_turn(&auth("placeholder-token"), &request)
        .await
        .expect("open")
        .map(|result| result.unwrap())
        .collect::<Vec<_>>()
        .await;
    assert_eq!(events.first().cloned().unwrap(), TurnEvent::ToolCall {
        id: "toolu_7".to_string(),
        name: "get_weather".to_string(),
        arguments_delta: None,
        arguments: None,
    });
    // The fragments travel verbatim (`AI-ADAPTER-11`).
    assert!(events.iter().any(|event| matches!(event, TurnEvent::ToolCall {
        arguments_delta: Some(fragments),
        ..
    } if fragments == "{\"city\": ")));
    assert_eq!(events.last().cloned().unwrap(), TurnEvent::Done);
    let arguments = events.iter().find_map(|event| match event {
        TurnEvent::ToolCall { arguments: Some(arguments), .. } => Some(arguments.clone()),
        _ => None,
    });
    assert_eq!(arguments, Some(json!({ "city": "Osaka" })));
    let body = host.requests()[0].body();
    assert_eq!(body["tools"][0]["name"], "get_weather");
}

/// `AI-ADAPTER-04` — the opaque redacted-thinking block is preserved whole;
/// thinking deltas and signatures ride the stream.
#[tokio::test(flavor = "multi_thread")]
async fn redacted_thinking_survives_the_stream() {
    let host = provider(vec![Canned::Sse {
        status: 200,
        pieces: vec![
            piece(0, json!({ "type": "message_start", "message": {} })),
            piece(
                0,
                json!({
                    "type": "content_block_start",
                    "index": 0,
                    "content_block": { "type": "redacted_thinking", "data": "opaque" }
                }),
            ),
            piece(
                0,
                json!({
                    "type": "content_block_start",
                    "index": 1,
                    "content_block": { "type": "thinking", "thinking": "" }
                }),
            ),
            piece(
                0,
                json!({ "type": "content_block_delta", "index": 1, "delta": { "type": "thinking_delta", "thinking": "hmm" } }),
            ),
            piece(
                0,
                json!({ "type": "content_block_delta", "index": 1, "delta": { "type": "signature_delta", "signature": "sig==" } }),
            ),
            piece(0, json!({ "type": "content_block_stop", "index": 1 })),
            piece(
                0,
                json!({ "type": "message_delta", "delta": { "stop_reason": "end_turn" }, "usage": { "output_tokens": 4 } }),
            ),
            piece(0, json!({ "type": "message_stop" })),
        ],
    }]);
    let mut request = stream_request("claude-sonnet-4-5");
    request.thinking = Some(myharness_provider_sdk::ThinkingConfig { budget_tokens: 512 });
    let events: Vec<TurnEvent> = adapter(host.base.clone())
        .create_turn(&auth("placeholder-token"), &request)
        .await
        .expect("open")
        .map(|result| result.unwrap())
        .collect::<Vec<_>>()
        .await;
    assert!(
        events.contains(&TurnEvent::ReasoningRedacted {
            data: json!({ "type": "redacted_thinking", "data": "opaque" }),
        }),
        "{events:?}"
    );
    assert!(
        events.contains(&TurnEvent::ReasoningDelta {
            text: "hmm".to_string(),
            signature: None,
        }),
        "{events:?}"
    );
    assert!(
        events.contains(&TurnEvent::ReasoningDelta {
            text: String::new(),
            signature: Some("sig==".to_string()),
        }),
        "{events:?}"
    );
    let body = host.requests()[0].body();
    assert_eq!(body["thinking"]["budget_tokens"], 512);
}

/// `AI-STREAM-01` — citation annotations ride the stream.
#[tokio::test(flavor = "multi_thread")]
async fn citations_survive_the_stream() {
    let host = provider(vec![Canned::Sse {
        status: 200,
        pieces: vec![
            piece(0, json!({ "type": "message_start", "message": {} })),
            piece(0, json!({ "type": "content_block_start", "index": 0, "content_block": { "type": "text", "text": "" } })),
            piece(
                0,
                json!({
                    "type": "content_block_delta",
                    "index": 0,
                    "delta": { "type": "citations_delta", "citation": { "title": "Doc", "index": 2 } }
                }),
            ),
            piece(0, json!({ "type": "content_block_stop", "index": 0 })),
            piece(
                0,
                json!({ "type": "message_delta", "delta": { "stop_reason": "end_turn" }, "usage": { "output_tokens": 3 } }),
            ),
            piece(0, json!({ "type": "message_stop" })),
        ],
    }]);
    let events: Vec<TurnEvent> = adapter(host.base.clone())
        .create_turn(&auth("placeholder-token"), &stream_request("claude-sonnet-4-5"))
        .await
        .expect("open")
        .map(|result| result.unwrap())
        .collect::<Vec<_>>()
        .await;
    assert!(
        events.contains(&TurnEvent::Citation {
            citation: json!({ "title": "Doc", "index": 2 }),
        }),
        "{events:?}"
    );
}

/// `AI-MODEL-01`, `AI-ADAPTER-11` — a stream that opens on a different
/// model than the request named stops visibly.
#[tokio::test(flavor = "multi_thread")]
async fn a_model_echo_mismatch_stops_the_stream() {
    let host = provider(vec![Canned::Sse {
        status: 200,
        pieces: vec![piece(
            0,
            json!({ "type": "message_start", "message": { "model": "claude-3-haiku" } }),
        )],
    }]);
    let error = adapter(host.base.clone())
        .create_turn(&auth("placeholder-token"), &stream_request("claude-sonnet-4-5"))
        .await
        .unwrap()
        .map(|result| result.map(|_| ()))
        .collect::<Vec<Result<(), _>>>()
        .await
        .iter()
        .find_map(|result| result.as_ref().err().cloned())
        .expect("the echo mismatch must stop the stream");
    assert_eq!(error.class, ErrorClass::ModelNotFound, "{error:?}");
    assert!(error.message.contains("claude-3-haiku"));
}

/// `TEST-AI-014`, `AI-ADAPTER-08` — native stop reasons preserved 1:1,
/// including spellings outside the grammar (carried verbatim, never
/// collapsed into an invented `end_turn`).
#[tokio::test(flavor = "multi_thread")]
async fn native_stop_reasons_are_preserved() {
    for (stop, expected) in [
        ("end_turn", NativeStopReason::EndTurn),
        ("max_tokens", NativeStopReason::MaxTokens),
        ("stop_sequence", NativeStopReason::StopSequence),
        ("tool_use", NativeStopReason::ToolUse),
        ("pause_turn", NativeStopReason::PauseTurn),
        (
            "guardrail_intervened",
            NativeStopReason::Other("guardrail_intervened".to_string()),
        ),
    ] {
        let host = provider(legal_stream("claude-sonnet-4-5", "x", stop, 3));
        let events: Vec<TurnEvent> = adapter(host.base.clone())
            .create_turn(&auth("placeholder-token"), &stream_request("claude-sonnet-4-5"))
            .await
            .expect("open")
            .map(|result| result.unwrap())
            .collect::<Vec<_>>()
            .await;
        let stop = events
            .iter()
            .find_map(|event| match event {
                TurnEvent::Stop { reason } => Some(reason.clone()),
                _ => None,
            })
            .expect("a stop event");
        assert_eq!(stop, expected, "{stop:?}");
    }
}

/// `AI-USAGE-01`, `TEST-AI-013` — usage is exact and every reported count
/// survives, including the cache partitions reported on separate stream
/// events and the preserved web-search count.
#[tokio::test(flavor = "multi_thread")]
async fn cache_usage_is_exact() {
    let host = provider(vec![Canned::Sse {
        status: 200,
        pieces: vec![
            piece(
                0,
                json!({
                    "type": "message_start",
                    "message": {
                        "model": "claude-sonnet-4-5",
                        "usage": {
                            "input_tokens": 25,
                            "cache_read_input_tokens": 100,
                            "cache_creation_input_tokens": 50,
                            "web_search_input_tokens": 3
                        }
                    }
                }),
            ),
            piece(
                0,
                json!({
                    "type": "message_delta",
                    "delta": { "stop_reason": "end_turn" },
                    "usage": { "output_tokens": 12, "cache_read_input_tokens": 260 }
                }),
            ),
            piece(0, json!({ "type": "message_stop" })),
        ],
    }]);
    let events: Vec<TurnEvent> = adapter(host.base.clone())
        .create_turn(&auth("placeholder-token"), &stream_request("claude-sonnet-4-5"))
        .await
        .unwrap()
        .map(|result| result.unwrap())
        .collect::<Vec<_>>()
        .await;
    let usage = events
        .iter()
        .find_map(|event| match event {
            TurnEvent::Usage { usage } => Some(usage.clone()),
            _ => None,
        })
        .unwrap_or_default();
    assert_eq!(
        usage,
        Usage {
            input: 25,
            output: 12,
            cache_read: Some(260),
            cache_write: Some(50),
            web_search: Some(3),
        },
        "every count survives (`AI-USAGE-01`)"
    );
}

/// `AI-MM-01`, `AI-STREAM-01` — an assistant block outside the grammar is
/// named; an unknown event kind too; nothing is silently skipped.
#[tokio::test(flavor = "multi_thread")]
async fn an_unsubscribed_assistant_block_is_named() {
    let host = provider(vec![Canned::Sse {
        status: 200,
        pieces: vec![
            piece(0, json!({ "type": "message_start", "message": {} })),
            piece(
                0,
                json!({
                    "type": "content_block_start",
                    "index": 0,
                    "content_block": { "type": "spotify_playlist", "tracks": [] }
                }),
            ),
        ],
    }]);
    let error = adapter(host.base.clone())
        .create_turn(&auth("placeholder-token"), &stream_request("claude-sonnet-4-5"))
        .await
        .unwrap()
        .map(|result| result.map(|_| ()))
        .collect::<Vec<Result<(), _>>>()
        .await
        .iter()
        .find_map(|result| result.as_ref().err().cloned())
        .expect("the unhandled block must surface");
    assert_eq!(error.class, ErrorClass::UnsupportedModality, "{error:?}");
    assert!(error.message.contains("spotify_playlist"));

    // An unknown event kind is named, never skipped.
    let host = provider(vec![Canned::Sse {
        status: 200,
        pieces: vec![piece(0, json!({ "type": "weird_thing", "payload": {} }))],
    }]);
    let error = adapter(host.base.clone())
        .create_turn(&auth("placeholder-token"), &stream_request("claude-sonnet-4-5"))
        .await
        .unwrap()
        .map(|result| result.map(|_| ()))
        .collect::<Vec<Result<(), _>>>()
        .await
        .iter()
        .find_map(|result| result.as_ref().err().cloned())
        .expect("the unhandled event must surface");
    assert_eq!(error.class, ErrorClass::UnsupportedModality, "{error:?}");
    assert!(error.message.contains("weird_thing"));
}

/// `TEST-AI-021` — provider error envelopes map to the class vocabulary with
/// the provider's wording; raw bodies never surface.
#[tokio::test(flavor = "multi_thread")]
async fn provider_error_classes_are_distinguishable() {
    struct Case {
        status: u16,
        envelope: Value,
        class: ErrorClass,
    }
    let cases = vec![
        Case {
            status: 401,
            envelope: json!({
                "error": { "code": 401, "message": "request had invalid authentication", "status": "UNAUTHENTICATED" }
            }),
            class: ErrorClass::Auth,
        },
        Case {
            status: 403,
            envelope: json!({
                "error": { "code": 403, "message": "permission denied on this project", "status": "PERMISSION_DENIED" }
            }),
            class: ErrorClass::Auth,
        },
        Case {
            status: 429,
            envelope: json!({
                "error": { "code": 429, "message": "quota exceeded", "status": "RESOURCE_EXHAUSTED" }
            }),
            class: ErrorClass::Quota,
        },
        Case {
            status: 404,
            envelope: json!({
                "error": { "code": 404, "message": "model not found on this endpoint", "status": "NOT_FOUND" }
            }),
            class: ErrorClass::ModelNotFound,
        },
        Case {
            status: 400,
            envelope: json!({
                "error": { "code": 400, "message": "context length exceeded", "status": "INVALID_ARGUMENT" }
            }),
            class: ErrorClass::InvalidRequest,
        },
        Case {
            status: 503,
            envelope: json!({
                "error": { "code": 503, "message": "try again", "status": "UNAVAILABLE" }
            }),
            class: ErrorClass::Transient,
        },
    ];
    for case in cases {
        let host = provider(vec![Canned::Json(case.status, case.envelope.clone())]);
        let adapter = adapter(host.base.clone());
        let error = adapter
            .create_turn(&auth("placeholder-token"), &stream_request("claude-sonnet-4-5"))
            .await
            .err()
            .expect("an error envelope must surface");
        assert_eq!(error.class, case.class, "{:?}", case.envelope);
        assert_eq!(error.status, Some(case.status));
        assert!(
            !error.message.contains("placeholder-token"),
            "error messages carry provider wording, not raw bodies: {error:?}"
        );
    }
    // A bare status (no envelope) still maps and still names the status.
    let host = provider(vec![Canned::Bare(500)]);
    let adapter = adapter(host.base.clone());
    let error = adapter
        .create_turn(&auth("placeholder-token"), &stream_request("claude-sonnet-4-5"))
        .await
        .err()
        .expect("a bare failure must surface");
    assert_eq!(error.class, ErrorClass::Transient);
    assert!(error.message.contains("500"), "{error:?}");
}

/// `TEST-AI-006/007` — a missing credential blocks the send with zero
/// requests; nothing is substituted (`AI-ADAPTER-12`).
#[tokio::test(flavor = "multi_thread")]
async fn a_missing_credential_blocks_the_send() {
    let host = provider(vec![]);
    let adapter = adapter(host.base.clone());
    let error = adapter
        .create_turn(&auth("   "), &stream_request("claude-sonnet-4-5"))
        .await
        .err()
        .expect("an empty credential must refuse");
    assert_eq!(error.class, ErrorClass::Auth);
    assert!(
        error.message.contains("empty"),
        "the refusal names the problem: {error:?}"
    );
    assert!(
        host.requests().is_empty(),
        "zero requests must have travelled"
    );
}

/// `TEST-AI-003`, `AI-MODEL-02` — a model the Vertex policy rejects (`docs/12`
/// §2.2: `claude-` prefix; no `/ \ ? # %` or whitespace) is blocked before any
/// bytes go out; a benign family id is allowed through verbatim.
#[tokio::test(flavor = "multi_thread")]
async fn a_non_vertex_model_blocks_the_send() {
    let host = provider(vec![]);
    let adapter = adapter(host.base.clone());
    for id in ["gpt-4o", "claude/bad", "claude ok", "claude?x", "claude%2Fy"] {
        let error = adapter
            .create_turn(&auth("placeholder-token"), &stream_request(id))
            .await
            .err()
            .unwrap_or_else(|| panic!("{id:?} must refuse"));
        assert_eq!(error.class, ErrorClass::InvalidRequest, "{id:?}");
        assert!(
            error.message.contains("not sent"),
            "the refusal says nothing was sent ({id:?}): {error:?}"
        );
    }
    assert!(
        host.requests().is_empty(),
        "zero requests must have travelled"
    );
}

/// `AI-MM-02`, `AI-ADAPTER-06` — images inside tool results reach the
/// provider as images, on the wire.
#[tokio::test(flavor = "multi_thread")]
async fn tool_result_images_reach_the_provider() {
    let host = provider(legal_stream("claude-sonnet-4-5", "noted", "end_turn", 2));
    let adapter = adapter(host.base.clone());
    let mut request = stream_request("claude-sonnet-4-5");
    request.messages.push((
        Role::User,
        vec![ContentPart::ToolResult {
            tool_use_id: "toolu_1".to_string(),
            content: vec![
                ContentPart::Text {
                    text: "screenshot".to_string(),
                },
                ContentPart::Image {
                    media_type: "image/png".to_string(),
                    data: "aW1n".to_string(),
                },
            ],
            is_error: false,
            cache_control: None,
        }],
    ));
    let stream = adapter
        .create_turn(&auth("placeholder-token"), &request)
        .await
        .unwrap();
    let events: Vec<TurnEvent> = stream
        .map(|result| result.unwrap())
        .collect::<Vec<_>>()
        .await;
    assert!(!events.is_empty(), "the stream ran");
    let body = host.requests()[0].body();
    let result = &body["messages"][1]["content"][0];
    assert_eq!(result["type"], "tool_result");
    assert_eq!(result["content"][0]["type"], "text");
    assert_eq!(result["content"][1]["type"], "image");
    assert_eq!(result["content"][1]["source"]["media_type"], "image/png");
    assert_eq!(result["content"][1]["source"]["data"], "aW1n");
}

/// `TEST-AI-016` — the idle window fires when the stream sends only
/// keep-alives: an SSE comment or blank line is transport, not progress.
#[tokio::test(flavor = "multi_thread")]
async fn a_comment_only_stream_times_out_at_the_idle_window() {
    let host = provider(vec![Canned::Drip {
        interval_ms: 30,
        first: ": keep\n\n".to_string(),
    }]);
    let started = std::time::Instant::now();
    let error = adapter(host.base.clone())
        .create_turn(&auth("placeholder-token"), &stream_request("claude-sonnet-4-5"))
        .await
        .unwrap()
        .collect::<Vec<Result<TurnEvent, _>>>()
        .await
        .pop()
        .unwrap()
        .expect_err("the idle window must fire");
    let elapsed = started.elapsed();
    assert!(
        error.message.contains("idle"),
        "the error must name the idle timeout: {:?}",
        error
    );
    assert!(
        elapsed >= Duration::from_millis(130),
        "the window must have actually elapsed: {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_secs(3),
        "the timeout must not wait for the test's patience: {elapsed:?}"
    );
}

/// `TEST-AI-015` — cancelling (dropping) the stream stops it; the partial
/// output already emitted stays in the caller's hands (`docs/12` §4.1).
#[tokio::test(flavor = "multi_thread")]
async fn dropping_the_stream_cancels_it() {
    let host = provider(vec![Canned::Sse {
        status: 200,
        pieces: vec![
            piece(0, json!({ "type": "message_start", "message": {} })),
            piece(
                5,
                json!({ "type": "content_block_delta", "index": 0, "delta": { "type": "text_delta", "text": "first" } }),
            ),
        ],
    }]);
    let adapter = adapter(host.base.clone());
    let mut stream = adapter
        .create_turn(&auth("placeholder-token"), &stream_request("claude-sonnet-4-5"))
        .await
        .unwrap();
    let first = futures::StreamExt::next(&mut stream)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        first,
        TurnEvent::TextDelta {
            text: "first".to_string()
        }
    );
    drop(stream);
    host.stop();
}

/// `AI-ADAPTER-09`, `TEST-AI-013` — this family's surface exposes no native
/// token-count endpoint: the refusal is explicit, zero requests travel, and
/// no estimate is ever produced.
#[tokio::test(flavor = "multi_thread")]
async fn token_counting_has_no_native_endpoint() {
    let host = provider(vec![]);
    let error = adapter(host.base.clone())
        .count_tokens(
            &auth("placeholder-token"),
            &CountRequest {
                model: key("claude-sonnet-4-5"),
                system: Vec::new(),
                messages: vec![(
                    Role::User,
                    vec![ContentPart::Text {
                        text: "tokenize me".to_string(),
                    }],
                )],
                tools: Vec::new(),
            },
        )
        .await
        .expect_err("the count must refuse, never estimate");
    assert_eq!(error.class, ErrorClass::InvalidRequest, "{error:?}");
    assert!(
        error.message.contains("no native token-count"),
        "{error:?}"
    );
    assert!(
        host.requests().is_empty(),
        "zero requests must have travelled"
    );
}

/// Model discovery is unavailable for this family: the app disables it for
/// every credential kind the family offers (`docs/09` §2.2 "Vertex AI"), so
/// the adapter refuses outright and zero traffic travels (`AI-PROVIDER-05`;
/// `docs/14` §9 item 28).
#[tokio::test(flavor = "multi_thread")]
async fn model_discovery_refuses_without_a_request() {
    let host = provider(vec![]);
    let error = adapter(host.base.clone())
        .list_models(&auth("placeholder-token"))
        .await
        .expect_err("the listing must refuse on this family");
    assert_eq!(error.class, ErrorClass::UnsupportedModality, "{error:?}");
    assert!(
        error
            .message
            .contains("discovery is unavailable"),
        "{error:?}"
    );
    assert!(
        host.requests().is_empty(),
        "zero requests must have travelled"
    );
}

/// The setup window's *Test connection*: a real one-token inference that
/// surfaces exact usage, latency, and the native stop reason. The model
/// echo is conditional: an answer without a model field is not a refusal
/// (`docs/14` §9 item 28).
#[tokio::test(flavor = "multi_thread")]
async fn the_probe_inference_is_real() {
    let host = provider(vec![Canned::Json(
        200,
        json!({
            "content": [ { "type": "text", "text": "pong" } ],
            "stop_reason": "end_turn",
            "usage": { "input_tokens": 7, "output_tokens": 12 }
        }),
    )]);
    let status: ProbeStatus = adapter(host.base.clone())
        .probe_inference(&auth("placeholder-token"), "claude-sonnet-4-5")
        .await
        .expect("the probe must complete");
    assert!(status.ok);
    assert_eq!(status.model, "claude-sonnet-4-5");
    assert_eq!(status.stop_reason.as_deref(), Some("end_turn"));
    assert_eq!(status.input_tokens, 7);
    assert_eq!(status.output_tokens, 12);
    assert!(status.latency_ms > 0);
    let requests = host.requests();
    assert!(
        requests[0].has_path(
            "/v1/projects/synthetic-project/locations/us-east5/publishers/google/models/claude-sonnet-4-5:rawPredict"
        ),
        "{}",
        requests[0].raw
    );
    let body = requests[0].body();
    assert_eq!(body["max_tokens"], 1);
    assert_eq!(body["stream"], false);
    assert!(body.get("model").is_none());
    assert!(body.get("anthropic_version").is_none());
    assert_eq!(
        body["messages"][0]["content"][0],
        json!({ "type": "text", "text": "ping" })
    );

    // A probe failure carries the provider's error class verbatim.
    let host = provider(vec![Canned::Json(
        403,
        json!({
            "error": { "code": 403, "message": "permission denied", "status": "PERMISSION_DENIED" }
        }),
    )]);
    let error = adapter(host.base.clone())
        .probe_inference(&auth("placeholder-token"), "claude-sonnet-4-5")
        .await
        .expect_err("auth failure must surface");
    assert_eq!(error.class, ErrorClass::Auth);
}

/// `AI-MODEL-01` — a probe that answers for a different model is a visible
/// refusal, never a ride-through.
#[tokio::test(flavor = "multi_thread")]
async fn the_probe_refuses_a_non_echoing_model() {
    let host = provider(vec![Canned::Json(
        200,
        json!({
            "model": "claude-3-haiku",
            "stop_reason": "end_turn",
            "usage": { "input_tokens": 1, "output_tokens": 1 }
        }),
    )]);
    let error = adapter(host.base.clone())
        .probe_inference(&auth("placeholder-token"), "claude-sonnet-4-5")
        .await
        .expect_err("the echo mismatch must refuse");
    assert_eq!(error.class, ErrorClass::ModelNotFound);
    assert!(error.message.contains("claude-3-haiku"));
}

/// `TEST-AI-022` — a request only ever goes to the configured endpoints: the
/// raw-prediction routes and the base host's health read, and nothing else.
#[tokio::test(flavor = "multi_thread")]
async fn requests_travel_only_to_the_configured_endpoint() {
    let host = provider(vec![
        Canned::Json(
            200,
            json!({
                "stop_reason": "end_turn",
                "usage": { "input_tokens": 1, "output_tokens": 1 }
            }),
        ),
        Canned::Json(200, json!({ "ok": true })),
    ]);
    let adapter = adapter(host.base.clone());
    adapter
        .probe_inference(&auth("placeholder-token"), "claude-sonnet-4-5")
        .await
        .unwrap();
    let health = adapter.health_check(&auth("placeholder-token")).await.unwrap();
    assert!(health.reachable);
    let mut combined = String::new();
    for recorded in host.requests() {
        combined.push_str(&recorded.raw);
    }
    assert!(
        !combined.to_lowercase().contains("chat/completions"),
        "no OpenAI vocabulary: {combined}"
    );
    assert!(
        !combined.contains("/foundation-models"),
        "no Bedrock vocabulary: {combined}"
    );
}

/// `AI-ADAPTER-02` — a `nativeTurn` that carries beta identifiers is refused
/// on this surface: no beta transport is recorded for the family
/// (`docs/14` §9 item 28), and dropping the caller's list silently would be
/// exactly the `AI-ADAPTER-07` failure mode.
#[tokio::test(flavor = "multi_thread")]
async fn a_beta_list_with_no_transport_is_refused_visibly() {
    let host = provider(vec![]);
    let mut request = stream_request("claude-sonnet-4-5");
    request.betas = vec!["output-128k-2025-02-18".to_string()];
    let error = adapter(host.base.clone())
        .create_turn(&auth("placeholder-token"), &request)
        .await
        .err()
        .expect("the beta refusal must be visible");
    assert_eq!(error.class, ErrorClass::InvalidRequest, "{error:?}");
    assert!(
        error.message.contains("beta"),
        "the refusal names the dropped feature: {error:?}"
    );
    assert!(host.requests().is_empty());
}

/// `docs/09` §4, `AI-PROVIDER-03` — the credential appears on the wire and
/// in no debug face.
#[test]
fn debug_faces_never_carry_the_credential() {
    let adapter =
        VertexAdapter::new(config("http://127.0.0.1:9".to_string()), reqwest::Client::new());
    let face = format!("{adapter:?}");
    assert!(
        !face.contains("placeholder-token") && !face.contains("synthetic google token"),
        "useful without leaking: {face}"
    );
    let auth = auth("placeholder-token");
    let face = format!("{auth:?}");
    assert!(
        !face.contains("placeholder-token"),
        "the AuthContext debug answer is the label, never the value: {face}"
    );
}
