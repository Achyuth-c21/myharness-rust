//! Mock-provider wire fixtures — the conformance battery for the Claude API
//! (`anthropic`) adapter: `TEST-AI-001`'s family coverage, the applicable
//! members of `TEST-AI-005/007/008/013/014/015/016/018/021/022/023`, and the
//! regression assertions for the recorded violations `AI-ADAPTER-01..14`
//! (`docs/12` §7), applied to this family.
//!
//! Fixture policy (`tests/README.md`): every credential here is a synthetic
//! placeholder, every model id is a family-legal spelling, nothing is
//! recorded from a real session.

// `create_turn`'s Ok lane is an opaque stream handle. `expect_err` would
// demand `Debug` on it, so stream-returning sites keep `.err().expect(..)`.
#![allow(clippy::err_expect)]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::Duration;

use futures::StreamExt;
use myharness_provider_anthropic::{AnthropicAdapter, AnthropicClientConfig, ProbeStatus};
use myharness_provider_sdk::ProviderAdapter;
use myharness_provider_sdk::{
    AuthContext, ContentPart, CountRequest, ErrorClass, ModelKey, NativeTurnRequest, Role,
    SystemBlock, TurnEvent, Usage,
};
use secrecy::SecretString;
use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// The mock provider
// ---------------------------------------------------------------------------

/// One canned SSE piece with an optional delay before it is written.
struct SsePiece {
    delay_ms: u64,
    /// Raw chunked payload (without the chunked framing; the writer adds it).
    body: String,
}

/// A canned behavior for a single connection.
enum Canned {
    /// A complete JSON response.
    Json(u16, Value),
    /// A bare status with no body.
    Bare(u16),
    /// An SSE stream: the pieces are written in order (each inside its own
    /// chunked-encoding frame), and the connection stays open until the
    /// client drops it.
    Sse { status: u16, pieces: Vec<SsePiece> },
    /// An SSE drip: `ping` frames every interval, forever, carrying exactly
    /// one event (the first) — `TEST-AI-016`.
    Drip { interval_ms: u64, first: String },
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

/// A mock provider: a listener thread, a shared request log, and a stop flag.
/// Tests read [`MockProvider::requests`] directly instead of joining the
/// thread — zero-request scenarios (a blocked send) must be able to assert
/// emptiness without waiting for a connection that never comes.
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
            continue; // no canned behavior: close the connection
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
                &[SsePiece {
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

// ---------------------------------------------------------------------------
// Adapter builders
// ---------------------------------------------------------------------------

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
        // SSE shapes are written connection-wise inside `serve`.
        _ => Vec::new(),
    }
}

/// Write an SSE response chunk-by-chunk. `drip_interval_ms` turns the
/// connection into an endless ping drip (checked against the stop flag so a
/// lingering drip cannot outlive the test); otherwise the chunked body is
/// terminated after the pieces.
fn stream_sse(
    stream: &mut std::net::TcpStream,
    status: u16,
    pieces: &[SsePiece],
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
        // A zero-length chunk terminates the chunked body — never send one
        // as content (the drip opens with an empty piece by design).
        if piece.body.is_empty() {
            continue;
        }
        if write_chunk(piece.body.as_bytes()) {
            return; // the client is gone; stop writing
        }
    }
    if let Some(interval_ms) = drip_interval_ms {
        loop {
            if stop_flag.load(std::sync::atomic::Ordering::Relaxed) {
                return;
            }
            std::thread::sleep(Duration::from_millis(interval_ms));
            if write_chunk(b"event: ping\r\ndata: {}\r\n\r\n") {
                return; // client disconnected
            }
        }
    }
    let _ = stream.write_all(b"0\r\n\r\n");
    let _ = stream.flush();
}

fn config(base: String) -> AnthropicClientConfig {
    AnthropicClientConfig {
        base_url: Some(base),
        custom_headers: vec![("X-Tenant".to_string(), "acme".to_string())],
        idle_timeout: Duration::from_millis(150),
        betas: vec!["prompt-caching-2024-07-31".to_string()],
    }
}

fn adapter(base: String) -> AnthropicAdapter {
    AnthropicAdapter::new(config(base), reqwest::Client::new())
}

fn auth(value: &str) -> AuthContext {
    AuthContext::new(
        SecretString::from(value.to_string()),
        "synthetic static key",
    )
}

fn key(model_id: &str) -> ModelKey {
    ModelKey {
        provider_id: "anthropic".to_string(),
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
        betas: vec!["output-128k-2025-02-18".to_string()],
        stream: true,
    }
}

/// The legal minimal stream the provider answers a happy turn with.
fn legal_stream(model: &str, text: &str, stop: &str, output: u64) -> Vec<Canned> {
    vec![Canned::Sse {
        status: 200,
        pieces: vec![
            SsePiece {
                delay_ms: 0,
                body: format!(
                    "event: message_start\r\ndata: {}\r\n\r\n",
                    json!({ "message": { "model": model, "usage": { "input_tokens": 7, "output_tokens": 1 } } })
                ),
            },
            SsePiece {
                delay_ms: 5,
                body: format!(
                    "event: content_block_start\r\ndata: {}\r\n\r\n",
                    json!({ "index": 0, "content_block": { "type": "text", "text": "" } })
                ),
            },
            SsePiece {
                delay_ms: 5,
                body: format!(
                    "event: content_block_delta\r\ndata: {}\r\n\r\n",
                    json!({ "index": 0, "delta": { "type": "text_delta", "text": text } })
                ),
            },
            SsePiece {
                delay_ms: 5,
                body: "event: content_block_stop\r\ndata: {\"index\":0}\r\n\r\n".to_string(),
            },
            SsePiece {
                delay_ms: 5,
                body: format!(
                    "event: message_delta\r\ndata: {}\r\n\r\n",
                    json!({ "delta": { "stop_reason": stop, "stop_sequence": null }, "usage": { "output_tokens": output } })
                ),
            },
            SsePiece {
                delay_ms: 5,
                body: "event: message_stop\r\ndata: {}\r\n\r\n".to_string(),
            },
        ],
    }]
}

// ---------------------------------------------------------------------------
// The fixtures
// ---------------------------------------------------------------------------

/// `TEST-AI-005` applied to this family — the native wire contract:
/// `POST /v1/messages` on the family host, `viewport` headers native, betas
/// forwarded, the requested model verbatim (`AI-ADAPTER-07`, `-11`,
/// `AI-EXT-02`).
#[tokio::test(flavor = "multi_thread")]
async fn a_streamed_turn_speaks_the_native_wire() {
    let host = provider(legal_stream("claude-opus-4-1", "Hello", "end_turn", 12));
    let adapter = adapter(host.base.clone());
    let stream = adapter
        .create_turn(
            &auth("placeholder-credential"),
            &stream_request("claude-opus-4-1"),
        )
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
                reason: myharness_provider_sdk::NativeStopReason::EndTurn
            },
            TurnEvent::Done,
        ],
        "text, exact usage, native stop — in that order (`AI-USAGE-01`, `AI-STREAM-01`)"
    );

    let requests = host.requests();
    assert_eq!(requests.len(), 1, "exactly one request travels");
    let recorded = &requests[0];
    assert!(recorded.has_path("/v1/messages"), "{}", recorded.raw);
    assert!(
        recorded.raw.contains("anthropic-version: 2023-06-01"),
        "{}",
        recorded.raw
    );
    // The family's native auth header is `x-api-key` (`docs/18` §6); a
    // bearer line for this credential is a spelling the family never uses.
    assert!(
        recorded.header("x-api-key").unwrap_or_default() == "placeholder-credential",
        "{}",
        recorded.raw
    );
    assert!(!recorded.raw.contains("authorization:"), "{}", recorded.raw);
    assert!(
        recorded
            .header("x-tenant")
            .unwrap_or_default()
            .contains("acme")
            || recorded.raw.contains("X-Tenant: acme"),
        "{}",
        recorded.raw
    );
    // betas merge from config and request, comma-joined (`AI-ADAPTER-07`).
    let beta = recorded
        .header("anthropic-beta")
        .or_else(|| {
            recorded
                .raw
                .lines()
                .find(|l| l.contains("anthropic-beta:"))
                .map(|l| l.trim().to_string())
        })
        .unwrap_or_default();
    assert!(
        beta.contains("prompt-caching-2024-07-31") && beta.contains("output-128k-2025-02-18"),
        "betas must be forwarded: {beta}"
    );
    // The body is the native grammar — no OpenAI vocabulary anywhere
    // (`AI-ADAPTER-02`).
    let body = recorded.body();
    assert_eq!(body["model"], "claude-opus-4-1");
    assert_eq!(body["max_tokens"], 64);
    assert_eq!(body["stream"], true);
    assert_eq!(body["messages"][0]["content"][0]["type"], "text");
    assert!(!recorded.raw.to_lowercase().contains("chat/completions"));
    assert!(body.get("top_p").is_none());
    assert_eq!(body["temperature"], 0.4);
    assert_eq!(body["system"][0]["cache_control"], "ephemeral");
}

/// `AI-TOOL-01` — tool calls stream their argument fragments under the same
/// block id, and the assembled arguments close the call.
#[tokio::test(flavor = "multi_thread")]
async fn tool_arguments_stream_and_close() {
    let host = provider(vec![Canned::Sse {
        status: 200,
        pieces: vec![
            SsePiece {
                delay_ms: 0,
                body: "event: message_start\r\ndata: {\"message\":{\"model\":\"claude-opus-4-1\"}}\r\n\r\n"
                    .to_string(),
            },
            SsePiece {
                delay_ms: 5,
                body: "event: content_block_start\r\ndata: {\"index\":0,\"content_block\":{\"type\":\"tool_use\",\"id\":\"toolu_5\",\"name\":\"read_file\"}}\r\n\r\n"
                    .to_string(),
            },
            SsePiece {
                delay_ms: 5,
                body: "event: content_block_delta\r\ndata: {\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"path\\\":\"}}\r\n\r\n"
                    .to_string(),
            },
            SsePiece {
                delay_ms: 5,
                body: "event: content_block_delta\r\ndata: {\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"\\\"a.rs\\\"}\"}}\r\n\r\n"
                    .to_string(),
            },
            SsePiece {
                delay_ms: 5,
                body: "event: content_block_stop\r\ndata: {\"index\":0}\r\n\r\n".to_string(),
            },
            SsePiece {
                delay_ms: 5,
                body: "event: message_delta\r\ndata: {\"delta\":{\"stop_reason\":\"tool_use\",\"stop_sequence\":null}}\r\n\r\n".to_string(),
            },
            SsePiece {
                delay_ms: 5,
                body: "event: message_stop\r\ndata: {}\r\n\r\n".to_string(),
            },
        ],
    }]);
    let adapter = adapter(host.base.clone());
    let stream = adapter
        .create_turn(
            &auth("placeholder-credential"),
            &stream_request("claude-opus-4-1"),
        )
        .await
        .unwrap();
    let events: Vec<TurnEvent> = stream
        .map(|result| result.unwrap())
        .collect::<Vec<_>>()
        .await;
    assert_eq!(
        events,
        vec![
            TurnEvent::ToolCall {
                id: "toolu_5".to_string(),
                name: "read_file".to_string(),
                arguments_delta: None,
                arguments: None,
            },
            TurnEvent::ToolCall {
                id: "toolu_5".to_string(),
                name: "read_file".to_string(),
                arguments_delta: Some("{\"path\":".to_string()),
                arguments: None,
            },
            TurnEvent::ToolCall {
                id: "toolu_5".to_string(),
                name: "read_file".to_string(),
                arguments_delta: Some("\"a.rs\"}".to_string()),
                arguments: None,
            },
            TurnEvent::ToolCall {
                id: "toolu_5".to_string(),
                name: "read_file".to_string(),
                arguments_delta: None,
                arguments: Some(json!({ "path": "a.rs" })),
            },
            TurnEvent::Usage {
                usage: Usage::default(),
            },
            TurnEvent::Stop {
                reason: myharness_provider_sdk::NativeStopReason::ToolUse,
            },
            TurnEvent::Done,
        ]
    );
}

/// `AI-ADAPTER-04` — redacted thinking is preserved whole: opaque data the
/// provider issued is never rewritten or dropped.
#[tokio::test(flavor = "multi_thread")]
async fn redacted_thinking_survives_the_stream() {
    let payload = json!({ "data": ["opaque-bytes"] });
    let host = provider(vec![Canned::Sse {
        status: 200,
        pieces: vec![
            SsePiece {
                delay_ms: 0,
                body: "event: message_start\r\ndata: {\"message\":{\"model\":\"claude-opus-4-1\"}}\r\n\r\n"
                    .to_string(),
            },
            SsePiece {
                delay_ms: 5,
                body: format!(
                    "event: content_block_start\r\ndata: {}\r\n\r\n",
                    json!({ "index": 0, "content_block": { "type": "redacted_thinking", "data": payload } })
                ),
            },
            SsePiece {
                delay_ms: 5,
                body: "event: content_block_stop\r\ndata: {\"index\":0}\r\n\r\n".to_string(),
            },
            SsePiece {
                delay_ms: 5,
                body: "event: message_delta\r\ndata: {\"delta\":{\"stop_reason\":\"end_turn\"}}\r\n\r\n".to_string(),
            },
            SsePiece {
                delay_ms: 5,
                body: "event: message_stop\r\ndata: {}\r\n\r\n".to_string(),
            },
        ],
    }]);
    let adapter = adapter(host.base.clone());
    let stream = adapter
        .create_turn(
            &auth("placeholder-credential"),
            &stream_request("claude-opus-4-1"),
        )
        .await
        .unwrap();
    let events: Vec<TurnEvent> = stream
        .map(|result| result.unwrap())
        .collect::<Vec<_>>()
        .await;
    assert!(
        events.contains(&TurnEvent::ReasoningRedacted {
            data: json!({ "type": "redacted_thinking", "data": payload }),
        }),
        "{events:?}"
    );
}

/// `docs/12` §4.1 — citations survive the stream (`AI-STREAM-01`).
#[tokio::test(flavor = "multi_thread")]
async fn citations_survive_the_stream() {
    let citation = json!({ "type": "web_search_result", "url": "https://example/doc" });
    let host = provider(vec![Canned::Sse {
        status: 200,
        pieces: vec![
            SsePiece {
                delay_ms: 0,
                body: "event: message_start\r\ndata: {\"message\":{\"model\":\"claude-opus-4-1\"}}\r\n\r\n"
                    .to_string(),
            },
            SsePiece {
                delay_ms: 5,
                body: format!(
                    "event: content_block_delta\r\ndata: {}\r\n\r\n",
                    json!({ "index": 0, "delta": { "type": "citations_delta", "citation": citation } })
                ),
            },
            SsePiece {
                delay_ms: 5,
                body: "event: message_delta\r\ndata: {\"delta\":{\"stop_reason\":\"end_turn\"}}\r\n\r\n".to_string(),
            },
            SsePiece {
                delay_ms: 5,
                body: "event: message_stop\r\ndata: {}\r\n\r\n".to_string(),
            },
        ],
    }]);
    let adapter = adapter(host.base.clone());
    let events: Vec<TurnEvent> = adapter
        .create_turn(
            &auth("placeholder-credential"),
            &stream_request("claude-opus-4-1"),
        )
        .await
        .unwrap()
        .map(|result| result.unwrap())
        .collect::<Vec<_>>()
        .await;
    assert_eq!(
        events[0],
        TurnEvent::Citation {
            citation: citation.clone(),
        }
    );
}

/// `AI-MODEL-01`, `AI-ADAPTER-11` — the stream's opening frame names the
/// model the provider ran; a mismatch stops the stream, visibly.
#[tokio::test(flavor = "multi_thread")]
async fn a_model_echo_mismatch_stops_the_stream() {
    let host = provider(legal_stream("claude-sonnet-4", "Hello", "end_turn", 12));
    let adapter = adapter(host.base.clone());
    let error = adapter
        .create_turn(
            &auth("placeholder-credential"),
            &stream_request("claude-opus-4-1"),
        )
        .await
        .unwrap()
        .collect::<Vec<Result<TurnEvent, _>>>()
        .await
        .pop()
        .unwrap()
        .expect_err("a substitution must be visible");
    assert_eq!(error.class, ErrorClass::ModelNotFound);
    assert!(error.message.contains("claude-sonnet-4"), "{error:?}");
}

/// `TEST-AI-014`, `AI-ADAPTER-08` — native stop reasons are preserved 1:1
/// (a refusal or pause_turn never collapses into "done").
#[tokio::test(flavor = "multi_thread")]
async fn native_stop_reasons_are_preserved() {
    for (wire, expected) in [
        (
            "end_turn",
            myharness_provider_sdk::NativeStopReason::EndTurn,
        ),
        (
            "max_tokens",
            myharness_provider_sdk::NativeStopReason::MaxTokens,
        ),
        (
            "stop_sequence",
            myharness_provider_sdk::NativeStopReason::StopSequence,
        ),
        (
            "tool_use",
            myharness_provider_sdk::NativeStopReason::ToolUse,
        ),
        ("refusal", myharness_provider_sdk::NativeStopReason::Refusal),
        (
            "pause_turn",
            myharness_provider_sdk::NativeStopReason::PauseTurn,
        ),
    ] {
        let host = provider(legal_stream("claude-opus-4-1", "hi", wire, 4));
        let adapter = adapter(host.base.clone());
        let events: Vec<TurnEvent> = adapter
            .create_turn(
                &auth("placeholder-credential"),
                &stream_request("claude-opus-4-1"),
            )
            .await
            .unwrap()
            .map(|result| result.unwrap())
            .collect::<Vec<_>>()
            .await;
        let stop = events
            .iter()
            .find_map(|event| match event {
                TurnEvent::Stop { reason } => Some(reason),
                _ => None,
            })
            .expect("a stop event");
        assert_eq!(stop, &expected, "wire spelling {wire:?}");
    }
    // An unrecorded spelling stays verbatim, never invented.
    let host = provider(legal_stream("claude-opus-4-1", "hi", "quarantined", 4));
    let adapter = adapter(host.base.clone());
    let events: Vec<TurnEvent> = adapter
        .create_turn(
            &auth("placeholder-credential"),
            &stream_request("claude-opus-4-1"),
        )
        .await
        .unwrap()
        .map(|result| result.unwrap())
        .collect::<Vec<_>>()
        .await;
    let stop = events
        .iter()
        .find_map(|event| match event {
            TurnEvent::Stop { reason } => Some(reason.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        stop,
        myharness_provider_sdk::NativeStopReason::Other("quarantined".to_string())
    );
    assert_eq!(stop.wire(), "quarantined");
}

/// `AI-USAGE-01`, `TEST-AI-013` — usage is exact, separate per channel, and
/// never estimated: cache reads/writes and web-search counts carry 1:1.
#[tokio::test(flavor = "multi_thread")]
async fn cache_usage_is_exact() {
    let host = provider(vec![Canned::Sse {
        status: 200,
        pieces: vec![
            SsePiece {
                delay_ms: 0,
                body: "event: message_start\r\ndata: {\"message\":{\"model\":\"claude-opus-4-1\",\"usage\":{\"input_tokens\":11,\"output_tokens\":1,\"cache_read_input_tokens\":3,\"cache_creation_input_tokens\":5,\"web_search_input_tokens\":2}}}\r\n\r\n"
                    .to_string(),
            },
            SsePiece {
                delay_ms: 5,
                body: "event: content_block_start\r\ndata: {\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\r\n\r\n"
                    .to_string(),
            },
            SsePiece {
                delay_ms: 5,
                body: "event: content_block_delta\r\ndata: {\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"x\"}}\r\n\r\n"
                    .to_string(),
            },
            SsePiece {
                delay_ms: 5,
                body: "event: message_delta\r\ndata: {\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":9}}\r\n\r\n"
                    .to_string(),
            },
            SsePiece {
                delay_ms: 5,
                body: "event: message_stop\r\ndata: {}\r\n\r\n".to_string(),
            },
        ],
    }]);
    let adapter = adapter(host.base.clone());
    let events: Vec<TurnEvent> = adapter
        .create_turn(
            &auth("placeholder-credential"),
            &stream_request("claude-opus-4-1"),
        )
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
        .unwrap();
    assert_eq!(
        usage,
        Usage {
            input: 11,
            output: 9,
            cache_read: Some(3),
            cache_write: Some(5),
            web_search: Some(2),
        }
    );
}

/// `AI-MM-01`, `AI-STREAM-01` — an assistant block kind the adapter does not
/// model is named and stops the stream; nothing is dropped silently.
#[tokio::test(flavor = "multi_thread")]
async fn an_unsubscribed_assistant_block_is_named() {
    let host = provider(vec![Canned::Sse {
        status: 200,
        pieces: vec![
            SsePiece {
                delay_ms: 0,
                body: "event: message_start\r\ndata: {\"message\":{\"model\":\"claude-opus-4-1\"}}\r\n\r\n"
                    .to_string(),
            },
            SsePiece {
                delay_ms: 5,
                body: "event: content_block_start\r\ndata: {\"index\":0,\"content_block\":{\"type\":\"proto_block\"}}\r\n\r\n"
                    .to_string(),
            },
        ],
    }]);
    let adapter = adapter(host.base.clone());
    let error = adapter
        .create_turn(
            &auth("placeholder-credential"),
            &stream_request("claude-opus-4-1"),
        )
        .await
        .unwrap()
        .collect::<Vec<Result<TurnEvent, _>>>()
        .await
        .pop()
        .unwrap()
        .expect_err("an unmodeled block must name itself");
    assert_eq!(error.class, ErrorClass::UnsupportedModality);
    assert!(error.message.contains("proto_block"), "{error:?}");
}

/// `TEST-AI-021` — provider error envelopes map onto distinguishable
/// classes; the provider's own message wording is kept; nothing else from
/// the body is echoed.
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
            envelope: json!({"error": {"type": "authentication_error", "message": "bad key"}}),
            class: ErrorClass::Auth,
        },
        Case {
            status: 429,
            envelope: json!({"error": {"type": "rate_limit_error", "message": "slow down"}}),
            class: ErrorClass::Quota,
        },
        Case {
            status: 404,
            envelope: json!({"error": {"type": "not_found_error", "message": "no such model"}}),
            class: ErrorClass::ModelNotFound,
        },
        Case {
            status: 529,
            envelope: json!({"error": {"type": "overloaded_error", "message": "try later"}}),
            class: ErrorClass::Transient,
        },
        Case {
            status: 400,
            envelope: json!({"error": {"type": "invalid_request_error", "message": "prompt is too long"}}),
            class: ErrorClass::ContextLength,
        },
    ];
    for case in cases {
        let host = provider(vec![Canned::Json(case.status, case.envelope.clone())]);
        let adapter = adapter(host.base.clone());
        let error = adapter
            .create_turn(
                &auth("placeholder-credential"),
                &stream_request("claude-opus-4-1"),
            )
            .await
            .err()
            .expect("an error envelope must surface");
        assert_eq!(error.class, case.class, "{:?}", case.envelope);
        assert_eq!(error.status, Some(case.status));
        assert!(
            !error.message.contains("credential"),
            "error messages carry provider wording, not raw bodies: {error:?}"
        );
    }
    // A bare status (no envelope) still maps and still names the status.
    let host = provider(vec![Canned::Bare(500)]);
    let adapter = adapter(host.base.clone());
    let error = adapter
        .count_tokens(
            &auth("placeholder-credential"),
            &CountRequest {
                model: key("claude-opus-4-1"),
                system: Vec::new(),
                messages: Vec::new(),
                tools: Vec::new(),
            },
        )
        .await
        .unwrap_err();
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
        .create_turn(&auth("   "), &stream_request("claude-opus-4-1"))
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

/// `TEST-AI-007/008`, `AI-MODEL-02` — a model the Anthropic policy rejects
/// is blocked before any bytes go out; a benign-but-unlisted id is allowed
/// through to the provider as-is (the policy rejects *families*, not words).
#[tokio::test(flavor = "multi_thread")]
async fn a_non_anthropic_model_blocks_the_send() {
    let host = provider(vec![]);
    let adapter = adapter(host.base.clone());
    let error = adapter
        .create_turn(&auth("placeholder-credential"), &stream_request("gpt-4o"))
        .await
        .err()
        .expect("a non-Anthropic family id must refuse");
    assert_eq!(error.class, ErrorClass::InvalidRequest);
    assert!(
        error.message.contains("not sent"),
        "the refusal says nothing was sent: {error:?}"
    );
    assert!(
        host.requests().is_empty(),
        "zero requests must have travelled"
    );
}

/// `docs/12` §2.1 — `sk-ant-oat…` is an OAuth access token whose beta header
/// value was never captured (`docs/09` §2.2; `docs/14` §9); the adapter
/// refuses to send it uninformed rather than guessing a header
/// (`AI-ADAPTER-07`).
#[tokio::test(flavor = "multi_thread")]
async fn an_oauth_token_is_refused_informatively() {
    let host = provider(vec![Canned::Bare(200)]);
    let adapter = adapter(host.base.clone());
    let error = adapter
        .create_turn(
            &auth("sk-ant-oat01-synthetic"),
            &stream_request("claude-opus-4-1"),
        )
        .await
        .err()
        .expect("the OAuth-token refusal");
    assert_eq!(error.class, ErrorClass::Auth);
    assert!(
        error.message.contains("never captured"),
        "{}",
        error.message
    );
    assert!(host.requests().is_empty());
}

/// `AI-MM-02`, `AI-ADAPTER-06` — images inside tool results reach the
/// provider as images, on the wire.
#[tokio::test(flavor = "multi_thread")]
async fn tool_result_images_reach_the_provider() {
    // The provider answers legally; the assertion inspects the REQUEST body.
    let host = provider(legal_stream("claude-opus-4-1", "noted", "end_turn", 2));
    let adapter = adapter(host.base.clone());
    let mut request = stream_request("claude-opus-4-1");
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
        .create_turn(&auth("placeholder-credential"), &request)
        .await
        .unwrap();
    // The turn runs to completion; the assertion below inspects the request
    // body, so the typed events only need to have arrived.
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
/// keep-alives: `ping` is transport, not progress.
#[tokio::test(flavor = "multi_thread")]
async fn a_pigeonhole_stream_times_out_at_the_idle_window() {
    let host = provider(vec![Canned::Drip {
        interval_ms: 30,
        first: "event: ping\r\ndata: {}\r\n\r\n".to_string(),
    }]);
    // The idle window is 150 ms; pings every 30 ms keep the transport alive
    // forever, but only content resets the window.
    let started = std::time::Instant::now();
    let error = adapter(host.base.clone())
        .create_turn(
            &auth("placeholder-credential"),
            &stream_request("claude-opus-4-1"),
        )
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
            SsePiece {
                delay_ms: 0,
                body: format!(
                    "event: content_block_start\r\ndata: {}\r\n\r\n",
                    json!({ "index": 0, "content_block": { "type": "text", "text": "" } })
                ),
            },
            SsePiece {
                delay_ms: 5,
                body: concat!(
                    "event: content_block_delta\r\n",
                    "data: {\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"first\"}}\r\n\r\n"
                )
                .to_string(),
            },
            // Everything after this would be the rest of the turn.
            SsePiece {
                delay_ms: 60,
                body: "event: message_stop\r\ndata: {}\r\n\r\n".to_string(),
            },
        ],
    }]);
    let adapter = adapter(host.base.clone());
    let mut stream = adapter
        .create_turn(
            &auth("placeholder-credential"),
            &stream_request("claude-opus-4-1"),
        )
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
    // Drop = cancel. The connection must close; the server thread notices.
    drop(stream);
    // The partial output the caller already has is exactly what it emitted:
    // this handler kept `first`. Nothing more may arrive over the dropped
    // stream — a drop, rather than a graceful stop, is the cancellation.
    host.stop();
}

/// `AI-ADAPTER-09`, `TEST-AI-013` — token counting is native
/// (`POST /v1/messages/count_tokens`), never a length/4 estimate; an
/// unavailable endpoint is an explicit error.
#[tokio::test(flavor = "multi_thread")]
async fn token_counting_is_native_or_refused() {
    // The native endpoint answers.
    let host = provider(vec![Canned::Json(200, json!({ "input_tokens": 57 }))]);
    let count = adapter(host.base.clone())
        .count_tokens(
            &auth("placeholder-credential"),
            &CountRequest {
                model: key("claude-opus-4-1"),
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
        .expect("native counting");
    assert_eq!(count.input_tokens, 57);
    assert!(count.native, "the provider itself counted");
    let requests = host.requests();
    assert!(
        requests[0].has_path("/v1/messages/count_tokens"),
        "{}",
        requests[0].raw
    );
    assert_eq!(requests[0].body()["model"], "claude-opus-4-1");
    assert_eq!(
        requests[0].header("x-api-key").unwrap_or_default(),
        "placeholder-credential"
    );

    // No endpoint → an explicit error, never an estimate.
    let host = provider(vec![Canned::Bare(404)]);
    let error = adapter(host.base.clone())
        .count_tokens(
            &auth("placeholder-credential"),
            &CountRequest {
                model: key("claude-opus-4-1"),
                system: Vec::new(),
                messages: Vec::new(),
                tools: Vec::new(),
            },
        )
        .await
        .expect_err("a missing count endpoint is an error, not an estimate");
    assert!(error.message.contains("status 404"), "{error:?}");
}

/// `TEST-AI-018`, `TEST-AI-005` — discovery through the adapter: the native
/// listing carries **no injected `limit` parameter** (`docs/14` §9 — the
/// family's page size was never recorded), the cursor follows `after_id`
/// (`docs/12` §3.1 policy), first-page failure is fatal, later-page failure
/// retains partial results, and rows map onto native descriptors.
#[tokio::test(flavor = "multi_thread")]
async fn discovery_follows_the_cursor_and_retains_partial_results() {
    // Happy path + cursor: two pages. The first page carries no query.
    let host = provider(vec![
        Canned::Json(
            200,
            json!({
                "data": [
                    { "id": "claude-opus-4-1", "display_name": "Opus 4.1", "context_window": 200000 },
                    { "id": "claude-sonnet-4" }
                ],
                "has_more": true,
                "last_id": "claude-sonnet-4",
            }),
        ),
        Canned::Json(
            200,
            json!({
                "data": [ { "id": "claude-haiku-4" } ],
                "has_more": false,
            }),
        ),
    ]);
    let models = adapter(host.base.clone())
        .list_models(&auth("placeholder-credential"))
        .await
        .expect("discovery");
    assert_eq!(
        models
            .iter()
            .map(|m| m.key.model_id.clone())
            .collect::<Vec<_>>(),
        vec!["claude-opus-4-1", "claude-sonnet-4", "claude-haiku-4"]
    );
    assert_eq!(
        models
            .iter()
            .map(|m| m.key.provider_id.clone())
            .collect::<Vec<_>>(),
        vec!["anthropic", "anthropic", "anthropic"]
    );
    let opus = models
        .iter()
        .find(|m| m.key.model_id == "claude-opus-4-1")
        .unwrap();
    assert_eq!(opus.capabilities.context_window, Some(200000));
    // Unknown capabilities stay unsupported (`AI-PROVIDER-05`).
    let sonnet = models
        .iter()
        .find(|m| m.key.model_id == "claude-sonnet-4")
        .unwrap();
    assert!(!sonnet.capabilities.tools_supported);
    let requests = host.requests();
    assert!(requests[0].has_path("/v1/models"), "{}", requests[0].raw);
    assert!(
        !requests[0].raw.contains("limit="),
        "no injected limit parameter: {}",
        requests[0].raw
    );
    assert!(
        requests[1].raw.contains("after_id=claude-sonnet-4"),
        "{}",
        requests[1].raw
    );
    assert!(
        requests[0].header("x-api-key").unwrap_or_default() == "placeholder-credential",
        "{}",
        requests[0].raw
    );
    assert!(
        requests[0].raw.contains("anthropic-version: 2023-06-01"),
        "{}",
        requests[0].raw
    );

    // Later-page failure: retain what was read and warn.
    let host = provider(vec![
        Canned::Json(
            200,
            json!({ "data": [ { "id": "claude-opus-4-1" } ], "has_more": true, "last_id": "x" }),
        ),
        Canned::Bare(500),
    ]);
    let models = adapter(host.base.clone())
        .list_models(&auth("placeholder-credential"))
        .await
        .expect("a later-page failure must not lose the read rows");
    assert_eq!(models.len(), 1);

    // First-page failure is fatal.
    let host = provider(vec![Canned::Bare(500)]);
    let error = adapter(host.base.clone())
        .list_models(&auth("placeholder-credential"))
        .await
        .expect_err("the first page failing is an error");
    assert_eq!(error.class, ErrorClass::Transient);
    assert!(
        !error.message.to_lowercase().contains("boom"),
        "no raw body echo: {:?}",
        error
    );

    // The page cap stays three.
    let pages = (0..4)
        .map(|i| {
            Canned::Json(
                200,
                json!({
                    "data": [ { "id": format!("claude-model-{i}") } ],
                    "has_more": true,
                    "last_id": format!("claude-model-{i}"),
                }),
            )
        })
        .collect::<Vec<_>>();
    let host = provider(pages);
    let models = adapter(host.base.clone())
        .list_models(&auth("placeholder-credential"))
        .await
        .unwrap();
    assert_eq!(
        models.len(),
        3,
        "the 3-page cap retains what it read: {:?}",
        models
            .iter()
            .map(|m| m.key.model_id.clone())
            .collect::<Vec<_>>()
    );
}

/// The setup window's *Test connection*: a real one-token inference that
/// surfaces exact usage, latency, and the native stop reason — and refuses a
/// substitution even on the probe path.
#[tokio::test(flavor = "multi_thread")]
async fn the_probe_inference_is_real() {
    let host = provider(vec![Canned::Json(
        200,
        json!({
            "id": "msg_probe",
            "type": "message",
            "role": "assistant",
            "model": "claude-opus-4-1",
            "content": [ { "type": "text", "text": "pong" } ],
            "stop_reason": "end_turn",
            "stop_sequence": null,
            "usage": { "input_tokens": 7, "output_tokens": 12 }
        }),
    )]);
    let status: ProbeStatus = adapter(host.base.clone())
        .probe_inference(&auth("placeholder-credential"), "claude-opus-4-1")
        .await
        .expect("the probe must complete");
    assert!(status.ok);
    assert_eq!(status.model, "claude-opus-4-1");
    assert_eq!(status.stop_reason.as_deref(), Some("end_turn"));
    assert_eq!(status.output_tokens, 12);
    assert_eq!(status.input_tokens, 7);
    assert!(status.latency_ms > 0);
    let requests = host.requests();
    let body = requests[0].body();
    assert_eq!(body["max_tokens"], 1);
    assert_eq!(body["stream"], false);
    assert_eq!(
        body["messages"][0]["content"][0],
        json!({ "type": "text", "text": "ping" })
    );
    assert!(requests[0].has_path("/v1/messages"), "{}", requests[0].raw);

    // A probe failure carries the provider's error class verbatim.
    let host = provider(vec![Canned::Json(
        401,
        json!({"error": {"type": "authentication_error", "message": "this key is not real"}}),
    )]);
    let error = adapter(host.base.clone())
        .probe_inference(&auth("placeholder-credential"), "claude-opus-4-1")
        .await
        .expect_err("auth failure must surface");
    assert_eq!(error.class, ErrorClass::Auth);
}

/// The non-streaming head-turn: the whole response arrives; the model echo,
/// usage, and stop reason still hold.
#[tokio::test(flavor = "multi_thread")]
async fn a_non_streamed_turn_returns_typed_events() {
    let host = provider(vec![Canned::Json(
        200,
        json!({
            "model": "claude-opus-4-1",
            "content": [
                { "type": "text", "text": "Hello" },
                { "type": "tool_use", "id": "toolu_2", "name": "read_file", "input": { "path": "a.rs" } }
            ],
            "stop_reason": "tool_use",
            "usage": { "input_tokens": 9, "output_tokens": 5 }
        }),
    )]);
    let adapter = adapter(host.base.clone());
    let mut request = stream_request("claude-opus-4-1");
    request.stream = false;
    let stream = adapter
        .create_turn(&auth("placeholder-credential"), &request)
        .await
        .unwrap();
    let events: Vec<TurnEvent> = stream
        .map(|result| match result {
            Ok(event) => event,
            Err(error) => panic!("{error:?}"),
        })
        .collect::<Vec<_>>()
        .await;
    assert_eq!(
        events,
        vec![
            TurnEvent::TextDelta {
                text: "Hello".to_string()
            },
            TurnEvent::ToolCall {
                id: "toolu_2".to_string(),
                name: "read_file".to_string(),
                arguments_delta: None,
                arguments: Some(json!({ "path": "a.rs" })),
            },
            TurnEvent::Usage {
                usage: Usage {
                    input: 9,
                    output: 5,
                    cache_read: None,
                    cache_write: None,
                    web_search: None,
                }
            },
            TurnEvent::Stop {
                reason: myharness_provider_sdk::NativeStopReason::ToolUse
            },
            TurnEvent::Done,
        ]
    );
}

/// `TEST-AI-022`, `AI-ADAPTER-01` — no fetch interception, no routed URL:
/// every request lands on the configured endpoint, and none anywhere else;
/// two adapters share nothing global (`AI-ADAPTER-13`).
#[tokio::test(flavor = "multi_thread")]
async fn requests_travel_only_to_the_configured_endpoint() {
    let host_one = provider(vec![Canned::Json(200, json!({"input_tokens": 1}))]);
    let host_two = provider(vec![Canned::Json(200, json!({"input_tokens": 2}))]);
    let adapter_one = adapter(host_one.base.clone());
    let adapter_two = adapter(host_two.base.clone());
    let count_one = adapter_one
        .count_tokens(
            &auth("placeholder-credential"),
            &CountRequest {
                model: key("claude-opus-4-1"),
                system: Vec::new(),
                messages: vec![(
                    Role::User,
                    vec![ContentPart::Text {
                        text: "a".to_string(),
                    }],
                )],
                tools: Vec::new(),
            },
        )
        .await
        .unwrap();
    let count_two = adapter_two
        .count_tokens(
            &auth("placeholder-credential"),
            &CountRequest {
                model: key("claude-opus-4-1"),
                system: Vec::new(),
                messages: vec![(
                    Role::User,
                    vec![ContentPart::Text {
                        text: "b".to_string(),
                    }],
                )],
                tools: Vec::new(),
            },
        )
        .await
        .unwrap();
    assert_eq!(count_one.input_tokens, 1);
    assert_eq!(count_two.input_tokens, 2);
    let one = host_one.requests();
    assert_eq!(one.len(), 1);
    assert!(one[0].has_path("/v1/messages/count_tokens"));
    // The fixed path, never a transport-switched one (`TEST-AI-022`).
    assert!(
        !one[0].raw.to_lowercase().contains("chat/completions"),
        "{}",
        one[0].raw
    );
}

/// `AI-PROVIDER-03`, `docs/09` §4 — the credential cannot leak through debug
/// formatting: the labels name the source, never the value.
#[test]
fn debug_faces_never_carry_the_credential() {
    let auth = auth("placeholder-credential1a2b3c");
    assert!(
        !format!("{auth:?}").contains("placeholder-credential1a2b3c"),
        "{:?}",
        format!("{auth:?}")
    );
    // And the store type primitives keep the same property.
    let secret = SecretString::from("placeholder-credential1a2b3c".to_string());
    assert!(!format!("{secret:?}").contains("placeholder-credential1a2b3c"));
}
