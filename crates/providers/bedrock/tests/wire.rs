//! Mock-provider wire fixtures — the conformance battery for the Bedrock
//! adapter: `TEST-AI-001`'s family coverage, the applicable members of
//! `TEST-AI-005/006/007/008/013/014/015/016/018/021/022/023`, and the
//! regression assertions for the recorded violations `AI-ADAPTER-01..14`
//! (`docs/12` §7), applied to this family's native Bedrock Runtime surface.
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
use myharness_provider_bedrock::{
    BedrockAdapter, BedrockClientConfig, ProbeStatus, ANTHROPIC_VERSION_BEDROCK,
};
use myharness_provider_sdk::ProviderAdapter;
use myharness_provider_sdk::{
    AuthContext, ContentPart, CountRequest, ErrorClass, ModelKey, NativeStopReason,
    NativeTurnRequest, Role, SystemBlock, TurnEvent, Usage,
};
use secrecy::SecretString;
use serde_json::{json, Value};

use myharness_provider_bedrock::eventstream::{encode_frame, Header, HeaderValue};

// ---------------------------------------------------------------------------
// The mock provider
// ---------------------------------------------------------------------------

/// One canned bytes piece with an optional delay before it is written.
struct StreamPiece {
    delay_ms: u64,
    /// Raw chunked payload (without the chunked framing; the writer adds it).
    body: Vec<u8>,
}

/// A canned behavior for a single connection.
enum Canned {
    /// A complete JSON response.
    Json(u16, Value),
    /// A bare status with no body.
    Bare(u16),
    /// A native event stream: the pieces are written in order (each inside
    /// its own chunked-encoding frame), and the connection stays open until
    /// the client drops it.
    EventStream {
        status: u16,
        pieces: Vec<StreamPiece>,
    },
    /// A keep-alive drip: empty-payload frames every interval, forever,
    /// carrying exactly one event (the first) — `TEST-AI-016`.
    Drip { interval_ms: u64, first: Vec<u8> },
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
            Canned::EventStream { status, pieces } => {
                stream_frames(&mut stream, *status, pieces, None, &stop_flag)
            }
            Canned::Drip { interval_ms, first } => stream_frames(
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
        // Event-stream shapes are written connection-wise inside `serve`.
        _ => Vec::new(),
    }
}

/// Write a native event-stream response chunk-by-chunk. `drip_interval_ms`
/// turns the connection into an endless keep-alive drip (checked against the
/// stop flag so a lingering drip cannot outlive the test); otherwise the
/// chunked body is terminated after the pieces.
fn stream_frames(
    stream: &mut std::net::TcpStream,
    status: u16,
    pieces: &[StreamPiece],
    drip_interval_ms: Option<u64>,
    stop_flag: &std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    let head = format!(
        "HTTP/1.1 {status} OK\r\nContent-Type: application/vnd.amazon.eventstream\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n"
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
        if write_chunk(&piece.body) {
            return; // the client is gone; stop writing
        }
    }
    if let Some(interval_ms) = drip_interval_ms {
        let keepalive = encode_frame(
            &[Header::new(
                ":message-type",
                HeaderValue::Str("event".to_string()),
            )],
            b"",
        );
        loop {
            if stop_flag.load(std::sync::atomic::Ordering::Relaxed) {
                return;
            }
            std::thread::sleep(Duration::from_millis(interval_ms));
            if write_chunk(&keepalive) {
                return; // client disconnected
            }
        }
    }
    let _ = stream.write_all(b"0\r\n\r\n");
    let _ = stream.flush();
}

/// One family event: the frame bytes the runtime would answer with.
fn frame(event: &str, body: Value) -> StreamPiece {
    StreamPiece {
        delay_ms: 5,
        body: encode_frame(
            &[
                Header::new(":message-type", HeaderValue::Str("event".to_string())),
                Header::new(":event-type", HeaderValue::Str(event.to_string())),
            ],
            body.to_string().as_bytes(),
        ),
    }
}

fn piece(delay_ms: u64, event: &str, body: Value) -> StreamPiece {
    let mut out = frame(event, body);
    out.delay_ms = delay_ms;
    out
}

fn config(base: String) -> BedrockClientConfig {
    BedrockClientConfig {
        // One override answers both surfaces (`docs/14` §9) — the mock
        // stands in for both here.
        runtime_base_url: base.clone(),
        control_base_url: base,
        custom_headers: vec![("X-Tenant".to_string(), "acme".to_string())],
        idle_timeout: Duration::from_millis(150),
    }
}

fn adapter(base: String) -> BedrockAdapter {
    BedrockAdapter::new(config(base), reqwest::Client::new())
}

fn auth(value: &str) -> AuthContext {
    AuthContext::new(
        SecretString::from(value.to_string()),
        "synthetic static bearer",
    )
}

fn key(model_id: &str) -> ModelKey {
    ModelKey {
        provider_id: "bedrock".to_string(),
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
    vec![Canned::EventStream {
        status: 200,
        pieces: vec![
            piece(
                0,
                "message_start",
                json!({
                    "message": { "id": "msg_b", "role": "assistant", "model": model, "type": "message",
                                 "usage": { "input_tokens": 7, "output_tokens": 1 } }
                }),
            ),
            piece(
                0,
                "content_block_start",
                json!({ "index": 0, "content_block": { "type": "text", "text": "" } }),
            ),
            piece(
                0,
                "content_block_delta",
                json!({ "index": 0, "delta": { "type": "text_delta", "text": text } }),
            ),
            piece(0, "content_block_stop", json!({ "index": 0 })),
            piece(
                0,
                "message_delta",
                json!({
                    "delta": { "stop_reason": stop, "stop_sequence": null },
                    "usage": { "output_tokens": output }
                }),
            ),
            piece(0, "message_stop", json!({})),
        ],
    }]
}

// ---------------------------------------------------------------------------
// The fixtures
// ---------------------------------------------------------------------------

/// `TEST-AI-005` applied to this family — the native wire contract:
/// `POST /model/{id}/invoke-with-response-stream` on the runtime host, the
/// family's bearer credential, the requested model verbatim in the path and
/// absent from the body, the `anthropic_version` declaration in its place
/// (`AI-EXT-02`; `docs/14` §9).
#[tokio::test(flavor = "multi_thread")]
async fn a_streamed_turn_speaks_the_native_wire() {
    let host = provider(legal_stream(
        "anthropic.claude-sonnet-4-5",
        "Hello",
        "end_turn",
        12,
    ));
    let adapter = adapter(host.base.clone());
    let stream = adapter
        .create_turn(
            &auth("placeholder-credential"),
            &stream_request("anthropic.claude-sonnet-4-5"),
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
        recorded.has_path("/model/anthropic.claude-sonnet-4-5/invoke-with-response-stream"),
        "{}",
        recorded.raw
    );
    // The family's native credential is a bearer (`docs/09` §2.2
    // "static bearer"); `x-api-key` is a spelling this family never uses.
    assert!(
        recorded
            .header("authorization")
            .unwrap_or_default()
            .contains("Bearer placeholder-credential"),
        "{}",
        recorded.raw
    );
    assert!(!recorded.raw.contains("x-api-key"), "{}", recorded.raw);
    assert!(
        recorded
            .header("x-tenant")
            .unwrap_or_default()
            .contains("acme")
            || recorded.raw.contains("X-Tenant: acme"),
        "{}",
        recorded.raw
    );
    // The body is the native grammar — the model lives in the path, the
    // `anthropic_version` in the body, no OpenAI vocabulary anywhere
    // (`AI-ADAPTER-02`; `docs/14` §9).
    let body = recorded.body();
    assert_eq!(body["anthropic_version"], ANTHROPIC_VERSION_BEDROCK);
    assert!(body.get("model").is_none());
    assert_eq!(body["max_tokens"], 64);
    assert_eq!(body["stream"], true);
    assert_eq!(body["messages"][0]["content"][0]["type"], "text");
    assert!(!recorded.raw.to_lowercase().contains("chat/completions"));
    assert!(!recorded.raw.contains("converse"));
    assert!(body.get("top_p").is_none());
    assert_eq!(body["temperature"], 0.4);
    assert_eq!(body["system"][0]["cache_control"], "ephemeral");
}

/// `AI-TOOL-01` — tool calls stream their argument fragments under the same
/// block id, and the assembled arguments close the call.
#[tokio::test(flavor = "multi_thread")]
async fn tool_arguments_stream_and_close() {
    let host = provider(vec![Canned::EventStream {
        status: 200,
        pieces: vec![
            piece(
                0,
                "message_start",
                json!({ "message": { "model": "anthropic.claude-sonnet-4-5" } }),
            ),
            piece(
                0,
                "content_block_start",
                json!({
                    "index": 0,
                    "content_block": { "type": "tool_use", "id": "toolu_7", "name": "get_weather", "input": {} }
                }),
            ),
            piece(
                0,
                "content_block_delta",
                json!({ "index": 0, "delta": { "type": "input_json_delta", "partial_json": "{\"city\": " } }),
            ),
            piece(
                0,
                "content_block_delta",
                json!({ "index": 0, "delta": { "type": "input_json_delta", "partial_json": "\"Osaka\"}" } }),
            ),
            piece(0, "content_block_stop", json!({ "index": 0 })),
            piece(
                0,
                "message_delta",
                json!({ "delta": { "stop_reason": "tool_use" }, "usage": { "output_tokens": 9 } }),
            ),
            piece(0, "message_stop", json!({})),
        ],
    }]);
    let mut request = stream_request("anthropic.claude-sonnet-4-5");
    request.tools = vec![myharness_provider_sdk::ToolSpec {
        name: "get_weather".to_string(),
        description: None,
        input_schema: json!({ "type": "object", "properties": {} }),
        cache_control: None,
    }];
    let events: Vec<TurnEvent> = adapter(host.base.clone())
        .create_turn(&auth("placeholder-credential"), &request)
        .await
        .expect("open")
        .map(|result| result.unwrap())
        .collect::<Vec<_>>()
        .await;
    // The call opens with ids and names, then fragments travel verbatim
    // (`AI-ADAPTER-11`), then the assembled arguments close it.
    assert_eq!(
        events.first().cloned().unwrap(),
        TurnEvent::ToolCall {
            id: "toolu_7".to_string(),
            name: "get_weather".to_string(),
            arguments_delta: None,
            arguments: None,
        }
    );
    assert!(events
        .iter()
        .any(|event| matches!(event, TurnEvent::ToolCall {
        arguments_delta: Some(fragments),
        ..
    } if fragments == "{\"city\": ")));
    assert_eq!(events.last().cloned().unwrap(), TurnEvent::Done);
    let arguments = events.iter().find_map(|event| match event {
        TurnEvent::ToolCall {
            arguments: Some(arguments),
            ..
        } => Some(arguments.clone()),
        _ => None,
    });
    assert_eq!(
        arguments,
        Some(json!({ "city": "Osaka" })),
        "the assembled arguments close the call"
    );
    // The body carries the tool spec.
    let body = host.requests()[0].body();
    assert_eq!(body["tools"][0]["name"], "get_weather");
}

/// `AI-ADAPTER-04` — the opaque redacted-thinking block is preserved whole;
/// thinking deltas and signatures ride the stream.
#[tokio::test(flavor = "multi_thread")]
async fn redacted_thinking_survives_the_stream() {
    let host = provider(vec![Canned::EventStream {
        status: 200,
        pieces: vec![
            piece(0, "message_start", json!({ "message": {} })),
            piece(
                0,
                "content_block_start",
                json!({
                    "index": 0,
                    "content_block": { "type": "redacted_thinking", "data": "opaque" }
                }),
            ),
            piece(
                0,
                "content_block_start",
                json!({ "index": 1, "content_block": { "type": "thinking", "thinking": "" } }),
            ),
            piece(
                0,
                "content_block_delta",
                json!({ "index": 1, "delta": { "type": "thinking_delta", "thinking": "hmm" } }),
            ),
            piece(
                0,
                "content_block_delta",
                json!({ "index": 1, "delta": { "type": "signature_delta", "signature": "sig==" } }),
            ),
            piece(0, "content_block_stop", json!({ "index": 1 })),
            piece(
                0,
                "message_delta",
                json!({ "delta": { "stop_reason": "end_turn" }, "usage": { "output_tokens": 4 } }),
            ),
            piece(0, "message_stop", json!({})),
        ],
    }]);
    let mut request = stream_request("anthropic.claude-sonnet-4-5");
    request.thinking = Some(myharness_provider_sdk::ThinkingConfig { budget_tokens: 512 });
    let events: Vec<TurnEvent> = adapter(host.base.clone())
        .create_turn(&auth("placeholder-credential"), &request)
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
    // The thinking budget travels in the body.
    let body = host.requests()[0].body();
    assert_eq!(body["thinking"]["budget_tokens"], 512);
}

/// `AI-STREAM-01` — citation annotations ride the stream.
#[tokio::test(flavor = "multi_thread")]
async fn citations_survive_the_stream() {
    let host = provider(vec![Canned::EventStream {
        status: 200,
        pieces: vec![
            piece(0, "message_start", json!({ "message": {} })),
            piece(
                0,
                "content_block_start",
                json!({
                    "index": 0,
                    "content_block": { "type": "text", "text": "" }
                }),
            ),
            piece(
                0,
                "content_block_delta",
                json!({
                    "index": 0,
                    "delta": { "type": "citations_delta", "citation": { "title": "Doc", "index": 2 } }
                }),
            ),
            piece(0, "content_block_stop", json!({ "index": 0 })),
            piece(
                0,
                "message_delta",
                json!({ "delta": { "stop_reason": "end_turn" }, "usage": { "output_tokens": 3 } }),
            ),
            piece(0, "message_stop", json!({})),
        ],
    }]);
    let events: Vec<TurnEvent> = adapter(host.base.clone())
        .create_turn(
            &auth("placeholder-credential"),
            &stream_request("anthropic.claude-sonnet-4-5"),
        )
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

/// `AI-MODEL-01`, `AI-ADAPTER-11` — a stream that opens on a different model
/// than the request named stops visibly.
#[tokio::test(flavor = "multi_thread")]
async fn a_model_echo_mismatch_stops_the_stream() {
    let host = provider(vec![Canned::EventStream {
        status: 200,
        pieces: vec![piece(
            0,
            "message_start",
            json!({ "message": { "model": "anthropic.claude-3-haiku-20240307" } }),
        )],
    }]);
    let error = adapter(host.base.clone())
        .create_turn(
            &auth("placeholder-credential"),
            &stream_request("anthropic.claude-sonnet-4-5"),
        )
        .await
        .unwrap()
        .map(|result| result.map(|_| ()))
        .collect::<Vec<Result<(), _>>>()
        .await
        .iter()
        .find_map(|result| result.as_ref().err().cloned())
        .expect("the echo mismatch must stop the stream");
    assert_eq!(error.class, ErrorClass::ModelNotFound, "{error:?}");
}

/// `TEST-AI-014`, `AI-ADAPTER-08` — native stop reasons preserved 1:1,
/// including spellings outside the known grammar (carried verbatim, never
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
        let host = provider(legal_stream("anthropic.claude-sonnet-4-5", "x", stop, 3));
        let events: Vec<TurnEvent> = adapter(host.base.clone())
            .create_turn(
                &auth("placeholder-credential"),
                &stream_request("anthropic.claude-sonnet-4-5"),
            )
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
/// events and the repair that preserved `web_search_input_tokens`.
#[tokio::test(flavor = "multi_thread")]
async fn cache_usage_is_exact() {
    let host = provider(vec![Canned::EventStream {
        status: 200,
        pieces: vec![
            piece(
                0,
                "message_start",
                json!({
                    "message": {
                        "model": "anthropic.claude-sonnet-4-5",
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
                "message_delta",
                json!({
                    "delta": { "stop_reason": "end_turn" },
                    "usage": { "output_tokens": 12, "cache_read_input_tokens": 260 }
                }),
            ),
            piece(0, "message_stop", json!({})),
        ],
    }]);
    let events: Vec<TurnEvent> = adapter(host.base.clone())
        .create_turn(
            &auth("placeholder-credential"),
            &stream_request("anthropic.claude-sonnet-4-5"),
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
        .unwrap_or_default();
    // The final usage is exactly what the provider last reported, and no
    // count is replaced by a guess (`AI-USAGE-01`).
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
/// named; nothing is silently skipped.
#[tokio::test(flavor = "multi_thread")]
async fn an_unsubscribed_assistant_block_is_named() {
    let host = provider(vec![Canned::EventStream {
        status: 200,
        pieces: vec![
            piece(0, "message_start", json!({ "message": {} })),
            piece(
                0,
                "content_block_start",
                json!({
                    "index": 0,
                    "content_block": { "type": "spotify_playlist", "tracks": [] }
                }),
            ),
        ],
    }]);
    let error = adapter(host.base.clone())
        .create_turn(
            &auth("placeholder-credential"),
            &stream_request("anthropic.claude-sonnet-4-5"),
        )
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
                "__type": "UnrecognizedClientException",
                "message": "the security token is invalid"
            }),
            class: ErrorClass::Auth,
        },
        Case {
            status: 403,
            envelope: json!({
                "__type": "AccessDeniedException",
                "message": "not authorized to call bedrock"
            }),
            class: ErrorClass::Auth,
        },
        Case {
            status: 429,
            envelope: json!({
                "__type": "ThrottledException",
                "message": "rate exceeded"
            }),
            class: ErrorClass::Quota,
        },
        Case {
            status: 404,
            envelope: json!({
                "__type": "ResourceNotFoundException",
                "message": "no such model id"
            }),
            class: ErrorClass::ModelNotFound,
        },
        Case {
            status: 400,
            envelope: json!({
                "reason": "ValidationException",
                "message": "context length exceeded"
            }),
            class: ErrorClass::InvalidRequest,
        },
        Case {
            status: 503,
            envelope: json!({
                "__type": "ServiceUnavailableException",
                "message": "try again"
            }),
            class: ErrorClass::Transient,
        },
    ];
    for case in cases {
        let host = provider(vec![Canned::Json(case.status, case.envelope.clone())]);
        let adapter = adapter(host.base.clone());
        let error = adapter
            .create_turn(
                &auth("placeholder-credential"),
                &stream_request("anthropic.claude-sonnet-4-5"),
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
        .create_turn(
            &auth("placeholder-credential"),
            &stream_request("anthropic.claude-sonnet-4-5"),
        )
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
        .create_turn(&auth("   "), &stream_request("anthropic.claude-sonnet-4-5"))
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

/// `TEST-AI-007/008`, `AI-MODEL-02` — a model the Bedrock policy rejects
/// (`docs/12` §2.2; only `anthropic.*` and Bedrock ARNs are the family's)
/// is blocked before any bytes go out; a benign id is allowed through
/// verbatim.
#[tokio::test(flavor = "multi_thread")]
async fn a_non_bedrock_model_blocks_the_send() {
    let host = provider(vec![]);
    let adapter = adapter(host.base.clone());
    let error = adapter
        .create_turn(&auth("placeholder-credential"), &stream_request("gpt-4o"))
        .await
        .err()
        .expect("a non-Bedrock family id must refuse");
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

/// `AI-MM-02`, `AI-ADAPTER-06` — images inside tool results reach the
/// provider as images, on the wire.
#[tokio::test(flavor = "multi_thread")]
async fn tool_result_images_reach_the_provider() {
    // The provider answers legally; the assertion inspects the REQUEST body.
    let host = provider(legal_stream(
        "anthropic.claude-sonnet-4-5",
        "noted",
        "end_turn",
        2,
    ));
    let adapter = adapter(host.base.clone());
    let mut request = stream_request("anthropic.claude-sonnet-4-5");
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
/// keep-alives: an empty-payload event frame is transport, not progress.
#[tokio::test(flavor = "multi_thread")]
async fn a_pigeonhole_stream_times_out_at_the_idle_window() {
    let host = provider(vec![Canned::Drip {
        interval_ms: 30,
        first: encode_frame(
            &[Header::new(
                ":message-type",
                HeaderValue::Str("event".to_string()),
            )],
            b"",
        ),
    }]);
    // The idle window is 150 ms; keep-alives every 30 ms keep the transport
    // alive forever, but only content resets the window.
    let started = std::time::Instant::now();
    let error = adapter(host.base.clone())
        .create_turn(
            &auth("placeholder-credential"),
            &stream_request("anthropic.claude-sonnet-4-5"),
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
    let host = provider(legal_stream_with_first_then_portion());
    let adapter = adapter(host.base.clone());
    let mut stream = adapter
        .create_turn(
            &auth("placeholder-credential"),
            &stream_request("anthropic.claude-sonnet-4-5"),
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
    drop(stream);
    host.stop();
}

fn legal_stream_with_first_then_portion() -> Vec<Canned> {
    vec![Canned::EventStream {
        status: 200,
        pieces: vec![
            piece(0, "message_start", json!({ "message": {} })),
            piece(
                5,
                "content_block_delta",
                json!({ "index": 0, "delta": { "type": "text_delta", "text": "first" } }),
            ),
        ],
    }]
}

/// `AI-ADAPTER-09`, `TEST-AI-013` — this family's surface exposes no native
/// token-count endpoint: the refusal is explicit, zero requests travel, and
/// no estimate is ever produced.
#[tokio::test(flavor = "multi_thread")]
async fn token_counting_has_no_native_endpoint() {
    let host = provider(vec![]);
    let error = adapter(host.base.clone())
        .count_tokens(
            &auth("placeholder-credential"),
            &CountRequest {
                model: key("anthropic.claude-sonnet-4-5"),
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
    assert!(error.message.contains("no native token-count"), "{error:?}");
    assert!(
        host.requests().is_empty(),
        "zero requests must have travelled"
    );
}

/// `TEST-AI-018` — discovery walks the control plane's model APIs: pages
/// follow the cursor, rows de-duplicate by stable id, a later-page failure
/// retains what was read, and the first-page failure is fatal.
#[tokio::test(flavor = "multi_thread")]
async fn discovery_walks_the_control_plane_and_keeps_partial_rows() {
    // Happy path + cursor: two pages. The first page carries no query.
    let host = provider(vec![
        Canned::Json(
            200,
            json!({
                "modelSummaries": [
                    { "modelId": "anthropic.claude-sonnet-4-5", "modelName": "Sonnet 4.5" },
                    { "modelId": "cohere.command-r-v1:0", "modelName": "not ours" },
                    { "modelId": "meta.llama3-8b-instruct-v1:0", "modelName": "not ours" }
                ],
                "nextToken": "cursor-1"
            }),
        ),
        Canned::Json(
            200,
            json!({
                "modelSummaries": [
                    { "modelId": "anthropic.claude-sonnet-4-5", "modelName": "dup across pages" },
                    { "modelId": "mistral.mistral-small-2402-v1:0", "modelName": "not ours" }
                ],
                "nextToken": null
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
        vec![
            "anthropic.claude-sonnet-4-5",
            "cohere.command-r-v1:0",
            "meta.llama3-8b-instruct-v1:0",
            "mistral.mistral-small-2402-v1:0"
        ],
        "all advertised rows carry (the family filter is the caller's policy layer)"
    );
    assert!(
        models.iter().all(|m| m.key.provider_id == "bedrock"),
        "the provider id is the family's"
    );
    let sonnet = models
        .iter()
        .find(|m| m.key.model_id == "anthropic.claude-sonnet-4-5")
        .expect("one stable dedupe key");
    assert_eq!(sonnet.display_name.as_deref(), Some("Sonnet 4.5"));
    // Unknown capabilities stay unsupported (`AI-PROVIDER-05`).
    assert!(!sonnet.capabilities.tools_supported);
    let requests = host.requests();
    assert!(
        requests[0].has_path("/foundation-models"),
        "{}",
        requests[0].raw
    );
    assert!(
        !requests[0].raw.contains("limit="),
        "no injected limit parameter: {}",
        requests[0].raw
    );
    assert!(
        !requests[0].raw.contains("byProvider="),
        "no invented filter parameter: {}",
        requests[0].raw
    );
    assert!(
        requests[1].raw.contains("nextToken=cursor-1"),
        "{}",
        requests[1].raw
    );
    assert!(
        requests[0]
            .header("authorization")
            .unwrap_or_default()
            .contains("Bearer placeholder-credential"),
        "{}",
        requests[0].raw
    );

    // Later-page failure: retain what was read and warn.
    let host = provider(vec![
        Canned::Json(
            200,
            json!({ "modelSummaries": [ { "modelId": "anthropic.claude-sonnet-4-5" } ], "nextToken": "x" }),
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
                    "modelSummaries": [ { "modelId": format!("anthropic.model-{i}") } ],
                    "nextToken": format!("cursor-{i}")
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
/// substitution even on the probe path. The model echo is conditional: an
/// answer with no model field is not a refusal (`docs/14` §9).
#[tokio::test(flavor = "multi_thread")]
async fn the_probe_inference_is_real() {
    let host = provider(vec![Canned::Json(
        200,
        json!({
            "id": "msg_probe",
            "type": "message",
            "role": "assistant",
            "model": "anthropic.claude-sonnet-4-5",
            "content": [ { "type": "text", "text": "pong" } ],
            "stop_reason": "end_turn",
            "usage": { "input_tokens": 7, "output_tokens": 12 }
        }),
    )]);
    let status: ProbeStatus = adapter(host.base.clone())
        .probe_inference(
            &auth("placeholder-credential"),
            "anthropic.claude-sonnet-4-5",
        )
        .await
        .expect("the probe must complete");
    assert!(status.ok);
    assert_eq!(status.model, "anthropic.claude-sonnet-4-5");
    assert_eq!(status.stop_reason.as_deref(), Some("end_turn"));
    assert_eq!(status.output_tokens, 12);
    assert_eq!(status.input_tokens, 7);
    assert!(status.latency_ms > 0);
    let requests = host.requests();
    let body = requests[0].body();
    assert_eq!(body["max_tokens"], 1);
    assert_eq!(body["stream"], false);
    assert_eq!(body["anthropic_version"], ANTHROPIC_VERSION_BEDROCK);
    assert_eq!(
        body["messages"][0]["content"][0],
        json!({ "type": "text", "text": "ping" })
    );
    assert!(
        requests[0].has_path("/model/anthropic.claude-sonnet-4-5/invoke"),
        "{}",
        requests[0].raw
    );

    // A probe answer without a model field still completes — the echo is
    // conditional (`docs/14` §9): the model went out in the path.
    let host = provider(vec![Canned::Json(
        200,
        json!({
            "content": [ { "type": "text", "text": "pong" } ],
            "stop_reason": "end_turn",
            "usage": { "input_tokens": 7, "output_tokens": 12 }
        }),
    )]);
    let status = adapter(host.base.clone())
        .probe_inference(
            &auth("placeholder-credential"),
            "anthropic.claude-sonnet-4-5",
        )
        .await
        .expect("a model-less answer is not an echo failure");
    assert_eq!(status.model, "anthropic.claude-sonnet-4-5");

    // A probe failure carries the provider's error class verbatim.
    let host = provider(vec![Canned::Json(
        403,
        json!({ "__type": "AccessDeniedException", "message": "not authorized" }),
    )]);
    let error = adapter(host.base.clone())
        .probe_inference(
            &auth("placeholder-credential"),
            "anthropic.claude-sonnet-4-5",
        )
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
            "model": "anthropic.claude-3-haiku-20240307",
            "stop_reason": "end_turn",
            "usage": { "input_tokens": 1, "output_tokens": 1 }
        }),
    )]);
    let error = adapter(host.base.clone())
        .probe_inference(
            &auth("placeholder-credential"),
            "anthropic.claude-sonnet-4-5",
        )
        .await
        .expect_err("the echo mismatch must refuse");
    assert_eq!(error.class, ErrorClass::ModelNotFound);
}

/// An inference-profile ARN travels encoded in the request path: its `:`
/// separators stay (they cannot split a path) and its `/` is percent-encoded
/// (`docs/14` §9).
#[tokio::test(flavor = "multi_thread")]
async fn an_arn_model_travels_encoded_in_the_path() {
    let arn = "arn:aws:bedrock:us-east-1::inference-profile/proj/sonnet";
    let host = provider(vec![Canned::Json(
        200,
        json!({
            "stop_reason": "end_turn",
            "usage": { "input_tokens": 1, "output_tokens": 1 }
        }),
    )]);
    let status = adapter(host.base.clone())
        .probe_inference(&auth("placeholder-credential"), arn)
        .await
        .expect("the probe accepts an ARN");
    assert_eq!(status.model, arn);
    let raw = &host.requests()[0].raw;
    assert!(
        raw.contains(
            "POST /model/arn:aws:bedrock:us-east-1::inference-profile%2Fproj%2Fsonnet/invoke "
        ),
        "{}",
        raw
    );
}

/// `TEST-AI-022` — a request only ever goes to the configured endpoints: the
/// runtime surface's two invoke methods and the control plane's model APIs,
/// and nothing else.
#[tokio::test(flavor = "multi_thread")]
async fn requests_travel_only_to_the_configured_endpoints() {
    let host = provider(vec![
        Canned::Json(
            200,
            json!({
                "stop_reason": "end_turn",
                "usage": { "input_tokens": 1, "output_tokens": 1 }
            }),
        ),
        Canned::Json(200, json!({ "modelSummaries": [] })),
    ]);
    let adapter = adapter(host.base.clone());
    adapter
        .probe_inference(
            &auth("placeholder-credential"),
            "anthropic.claude-sonnet-4-5",
        )
        .await
        .unwrap();
    adapter
        .health_check(&auth("placeholder-credential"))
        .await
        .unwrap();
    let mut combined = String::new();
    for recorded in host.requests() {
        combined.push_str(&recorded.raw);
    }
    assert!(
        !combined.to_lowercase().contains("chat/completions"),
        "no OpenAI vocabulary: {combined}"
    );
    assert!(
        !combined.contains("/converse"),
        "not the Converse surface: {combined}"
    );
}

/// `AI-ADAPTER-02` — a `nativeTurn` that carries beta identifiers is
/// refused on this surface: no beta transport is recorded for the family
/// (`docs/14` §9), and dropping the caller's list silently is exactly the
/// `AI-ADAPTER-07` failure mode.
#[tokio::test(flavor = "multi_thread")]
async fn a_beta_list_with_no_transport_is_refused_visibly() {
    let host = provider(vec![]);
    let mut request = stream_request("anthropic.claude-sonnet-4-5");
    request.betas = vec!["output-128k-2025-02-18".to_string()];
    let error = adapter(host.base.clone())
        .create_turn(&auth("placeholder-credential"), &request)
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
    let adapter = BedrockAdapter::new(
        config("http://127.0.0.1:9".to_string()),
        reqwest::Client::new(),
    );
    let face = format!("{adapter:?}");
    assert!(
        !face.contains("placeholder-credential") && !face.contains("synthetic static bearer"),
        "useful without leaking: {face}"
    );
    let auth = auth("placeholder-credential");
    let face = format!("{auth:?}");
    assert!(
        !face.contains("placeholder-credential"),
        "the AuthContext debug answer is the label, never the value: {face}"
    );
}
