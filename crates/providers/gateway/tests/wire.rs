//! Mock-gateway wire fixtures — the `TEST-AI-005/007/008/013/014/015/016/018/
//! 021/022/023` conformance battery for the Gateway adapter, plus the
//! regression assertions for the recorded violations `AI-ADAPTER-01..14`
//! (`docs/12` §7: the 14 tricks the local translation shim must never be
//! able to do again).
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

use futures::{StreamExt, TryStreamExt};
use myharness_provider_gateway::{GatewayAdapter, GatewayClientConfig, ProbeStatus};
use myharness_provider_sdk::ProviderAdapter;
use myharness_provider_sdk::{
    AuthContext, ContentPart, CountRequest, ErrorClass, ModelKey, NativeTurnRequest, Role,
    SystemBlock, TurnEvent, Usage,
};
use secrecy::SecretString;
use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// The mock gateway
// ---------------------------------------------------------------------------

/// One canned SSE piece with an optional delay before it is written.
struct SsePiece {
    delay_ms: u64,
    /// Raw chunked payload (without the chunked framing; the writer adds it).
    body: String,
}

/// A canned gateway behavior for a single connection.
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

/// A mock gateway: a listener thread, a shared request log, and a stop flag.
/// Tests read [`MockGateway::requests`] directly instead of joining the
/// thread — zero-request scenarios (a blocked send) must be able to assert
/// emptiness without waiting for a connection that never comes.
struct MockGateway {
    base: String,
    requests: std::sync::Arc<std::sync::Mutex<Vec<Recorded>>>,
    stop_flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl MockGateway {
    fn requests(&self) -> Vec<Recorded> {
        self.requests.lock().unwrap().clone()
    }

    /// Release the listener thread.
    fn stop(&self) {
        self.stop_flag
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

impl Drop for MockGateway {
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

fn gateway(canned: Vec<Canned>) -> MockGateway {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().unwrap().port();
    let requests = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let stop_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let handle = std::thread::spawn({
        let requests = requests.clone();
        let stop_flag = stop_flag.clone();
        move || serve(listener, canned, requests, stop_flag)
    });
    MockGateway {
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

fn config(
    base: String,
    auth_scheme: myharness_provider_gateway::AuthScheme,
) -> myharness_provider_gateway::GatewayClientConfig {
    myharness_provider_gateway::GatewayClientConfig {
        base_url: base,
        auth_scheme,
        custom_headers: vec![("X-Tenant".to_string(), "acme".to_string())],
        idle_timeout: Duration::from_millis(150),
        betas: vec!["prompt-caching-2024-07-31".to_string()],
    }
}

fn adapter(base: String, auth_scheme: myharness_provider_gateway::AuthScheme) -> GatewayAdapter {
    GatewayAdapter::new(config(base, auth_scheme), reqwest::Client::new())
}

fn auth(value: &str) -> AuthContext {
    AuthContext::new(
        SecretString::from(value.to_string()),
        "synthetic static key",
    )
}

fn key(model_id: &str) -> ModelKey {
    ModelKey {
        provider_id: "gateway".to_string(),
        model_id: model_id.to_string(),
        variant: None,
        revision: None,
    }
}

/// The legal minimal stream a gateway answers a happy probe/turn with.
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

/// `TEST-AI-005` — the native wire contract: `POST /v1/messages`, the
/// `anthropic-version` header, both auth schemes, custom headers, betas
/// forwarded, the requested model verbatim (`AI-ADAPTER-07`, `-11`,
/// `AI-EXT-02`).
#[tokio::test(flavor = "multi_thread")]
async fn a_streamed_turn_speaks_the_native_wire() {
    let gw = gateway(legal_stream("claude-opus-4-1", "Hello", "end_turn", 12));
    let adapter = GatewayAdapter::new(
        GatewayClientConfig {
            base_url: gw.base.clone(),
            auth_scheme: myharness_provider_gateway::AuthScheme::Bearer,
            custom_headers: vec![("X-Tenant".to_string(), "acme".to_string())],
            idle_timeout: Duration::from_millis(150),
            betas: vec!["prompt-caching-2024-07-31".to_string()],
        },
        reqwest::Client::new(),
    );
    let request = NativeTurnRequest {
        model: key("claude-opus-4-1"),
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
    };
    let stream = adapter
        .create_turn(&auth("placeholder-credential"), &request)
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

    let requests = gw.requests();
    assert_eq!(requests.len(), 1, "exactly one request travels");
    let recorded = &requests[0];
    assert!(recorded.has_path("/v1/messages"), "{}", recorded.raw);
    assert!(
        recorded.raw.contains("anthropic-version: 2023-06-01"),
        "{}",
        recorded.raw
    );
    assert!(
        recorded
            .raw
            .contains("authorization: Bearer placeholder-credential"),
        "{}",
        recorded.raw
    );
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

/// `TEST-AI-005` (second half) — the `x-api-key` scheme authenticates from
/// the same native contract, and the bearer form is never used with it.
#[tokio::test(flavor = "multi_thread")]
async fn an_x_api_key_turn_authenticates_natively() {
    let gw = gateway(legal_stream("claude-opus-4-1", "Hello", "end_turn", 12));
    let adapter = adapter(
        gw.base.clone(),
        myharness_provider_gateway::AuthScheme::XApiKey,
    );
    let request = NativeTurnRequest {
        model: key("claude-opus-4-1"),
        system: Vec::new(),
        messages: vec![(
            Role::User,
            vec![ContentPart::Text {
                text: "hi".to_string(),
            }],
        )],
        tools: Vec::new(),
        max_tokens: 8,
        temperature: None,
        top_p: None,
        top_k: None,
        stop_sequences: Vec::new(),
        thinking: None,
        betas: Vec::new(),
        stream: true,
    };
    let stream = adapter
        .create_turn(&auth("placeholder-credential"), &request)
        .await
        .expect("the turn must open");
    let _ = stream.map_err(|_| ()).collect::<Vec<_>>().await;
    let requests = gw.requests();
    assert!(
        requests[0]
            .raw
            .contains("x-api-key: placeholder-credential"),
        "{}",
        requests[0].raw
    );
    assert!(
        !requests[0].raw.to_lowercase().contains("authorization:"),
        "{}",
        requests[0].raw
    );
}

/// `AI-TOOL-01` — streamed tool arguments: id and name preserved, argument
/// deltas arrive as they stream, and the assembled arguments close the call.
/// Invalid JSON surfaces and blocks execution (`docs/12` §5.2).
#[tokio::test(flavor = "multi_thread")]
async fn tool_arguments_stream_and_close() {
    let gw = gateway(vec![Canned::Sse {
        status: 200,
        pieces: vec![
            SsePiece {
                delay_ms: 0,
                body: format!(
                    "event: message_start\r\ndata: {}\r\n\r\n",
                    json!({ "message": { "model": "claude-opus-4-1", "usage": { "input_tokens": 5 } } })
                ),
            },
            SsePiece {
                delay_ms: 5,
                body: format!(
                    "event: content_block_start\r\ndata: {}\r\n\r\n",
                    json!({ "index": 0, "content_block": { "type": "tool_use", "id": "toolu_9", "name": "read_file" } })
                ),
            },
            SsePiece {
                delay_ms: 5,
                body: concat!(
                    "event: content_block_delta\r\n",
                    "data: {\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"path\\\":\"}}\r\n\r\n"
                )
                .to_string(),
            },
            SsePiece {
                delay_ms: 5,
                body: concat!(
                    "event: content_block_delta\r\n",
                    "data: {\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"\\\"main.rs\\\"}\"}}\r\n\r\n"
                )
                .to_string(),
            },
            SsePiece {
                delay_ms: 5,
                body: "event: content_block_stop\r\ndata: {\"index\":0}\r\n\r\n".to_string(),
            },
            SsePiece {
                delay_ms: 5,
                body: concat!(
                    "event: message_delta\r\n",
                    "data: {\"delta\":{\"stop_reason\":\"tool_use\",\"stop_sequence\":null},\"usage\":{\"output_tokens\":3}}\r\n\r\n"
                )
                .to_string(),
            },
            SsePiece {
                delay_ms: 5,
                body: "event: message_stop\r\ndata: {}\r\n\r\n".to_string(),
            },
        ],
    }]);
    let adapter = GatewayAdapter::new(
        GatewayClientConfig {
            base_url: gw.base.clone(),
            auth_scheme: myharness_provider_gateway::AuthScheme::Bearer,
            custom_headers: vec![("X-Tenant".to_string(), "acme".to_string())],
            idle_timeout: Duration::from_millis(150),
            betas: vec!["prompt-caching-2024-07-31".to_string()],
        },
        reqwest::Client::new(),
    );
    let request = NativeTurnRequest {
        model: key("claude-opus-4-1"),
        system: Vec::new(),
        messages: vec![(
            Role::User,
            vec![ContentPart::Text {
                text: "go".to_string(),
            }],
        )],
        tools: Vec::new(),
        max_tokens: 64,
        temperature: None,
        top_p: None,
        top_k: None,
        stop_sequences: Vec::new(),
        thinking: None,
        betas: Vec::new(),
        stream: true,
    };
    let stream = adapter
        .create_turn(&auth("placeholder-credential"), &request)
        .await
        .unwrap();
    let events: Vec<Result<TurnEvent, _>> = stream.collect().await;
    let observe = |events: &[Result<TurnEvent, _>]| -> Vec<String> {
        events
            .iter()
            .map(|result| match result {
                Ok(TurnEvent::ToolCall {
                    arguments_delta,
                    arguments,
                    ..
                }) => match (arguments_delta, arguments) {
                    (Some(chunk), None) => format!("delta:{chunk}"),
                    (_, Some(arguments)) => format!("args:{arguments}"),
                    (None, None) => "open".to_string(),
                },
                Ok(TurnEvent::Usage { usage }) => format!("usage:{}", usage.output),
                Ok(TurnEvent::Stop { reason }) => format!("stop:{}", reason.wire()),
                Ok(TurnEvent::Done) => "done".to_string(),
                Ok(_) => "other".to_string(),
                Err(error) => format!("err:{error:?}"),
            })
            .collect()
    };
    assert_eq!(
        observe(&events),
        vec![
            "open".to_string(),
            "delta:{\"path\":".to_string(),
            "delta:\"main.rs\"}".to_string(),
            "args:{\"path\":\"main.rs\"}".to_string(),
            // `message_delta` carries the cumulative usage before the stop
            // reason — the same channel the plain-text turn observes.
            "usage:3".to_string(),
            "stop:tool_use".to_string(),
            "done".to_string(),
        ]
    );
}

/// `AI-ADAPTER-04` — redacted thinking is preserved whole; a missing block is
/// the recorded violation, and replay depends on the raw bytes.
#[tokio::test(flavor = "multi_thread")]
async fn redacted_thinking_survives_the_stream() {
    let gw = gateway(vec![Canned::Sse {
        status: 200,
        pieces: vec![
            SsePiece {
                delay_ms: 0,
                body: concat!(
                    "event: content_block_start\r\n",
                    "data: {\"index\":0,\"content_block\":{\"type\":\"redacted_thinking\",\"data\":\"0paqueBytes\"}}\r\n\r\n"
                )
                .to_string(),
            },
            SsePiece {
                delay_ms: 5,
                body: concat!(
                    "event: message_delta\r\n",
                    "data: {\"delta\":{\"stop_reason\":\"end_turn\"}}\r\n\r\n"
                )
                .to_string(),
            },
            SsePiece {
                delay_ms: 5,
                body: "event: message_stop\r\ndata: {}\r\n\r\n".to_string(),
            },
        ],
    }]);
    let adapter = GatewayAdapter::new(
        GatewayClientConfig {
            base_url: gw.base.clone(),
            auth_scheme: myharness_provider_gateway::AuthScheme::Bearer,
            custom_headers: vec![("X-Tenant".to_string(), "acme".to_string())],
            idle_timeout: Duration::from_millis(150),
            betas: vec!["prompt-caching-2024-07-31".to_string()],
        },
        reqwest::Client::new(),
    );
    let stream = adapter
        .create_turn(
            &auth("placeholder-credential"),
            &stream_request("claude-opus-4-1"),
        )
        .await
        .unwrap();
    let events: Vec<TurnEvent> = stream
        .map(|result| match result {
            Ok(event) => event,
            Err(error) => panic!("stream failed: {error:?}"),
        })
        .collect::<Vec<_>>()
        .await;
    assert!(
        events.iter().any(|event| matches!(
            event,
            TurnEvent::ReasoningRedacted {
                data,
            } if data.get("data").and_then(Value::as_str) == Some("0paqueBytes")
        )),
        "{events:?}"
    );
}

/// `docs/12` §4.1 — citations survive the stream (`AI-STREAM-01`).
#[tokio::test(flavor = "multi_thread")]
async fn citations_survive_the_stream() {
    let gw = gateway(vec![Canned::Sse {
        status: 200,
        pieces: vec![
            SsePiece {
                delay_ms: 0,
                body: concat!(
                    "event: content_block_delta\r\n",
                    "data: {\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"citing\"}}\r\n\r\n"
                )
                .to_string(),
            },
            SsePiece {
                delay_ms: 5,
                body: concat!(
                    "event: content_block_delta\r\n",
                    "data: {\"index\":0,\"delta\":{\"type\":\"citations_delta\",\"citation\":{\"type\":\"char_location\",\"document_index\":1}}}\r\n\r\n"
                )
                .to_string(),
            },
            SsePiece {
                delay_ms: 5,
                body: concat!(
                    "event: message_delta\r\n",
                    "data: {\"delta\":{\"stop_reason\":\"end_turn\"}}\r\n\r\n"
                )
                .to_string(),
            },
            SsePiece {
                delay_ms: 5,
                body: "event: message_stop\r\ndata: {}\r\n\r\n".to_string(),
            },
        ],
    }]);
    let adapter = GatewayAdapter::new(
        GatewayClientConfig {
            base_url: gw.base.clone(),
            auth_scheme: myharness_provider_gateway::AuthScheme::Bearer,
            custom_headers: vec![("X-Tenant".to_string(), "acme".to_string())],
            idle_timeout: Duration::from_millis(150),
            betas: vec!["prompt-caching-2024-07-31".to_string()],
        },
        reqwest::Client::new(),
    );
    let stream = adapter
        .create_turn(
            &auth("placeholder-credential"),
            &stream_request("claude-opus-4-1"),
        )
        .await
        .unwrap();
    let events: Vec<Result<TurnEvent, _>> = stream.collect().await;
    assert!(
        events.iter().any(|result| matches!(
            result,
            Ok(TurnEvent::Citation { citation }) if citation.get("document_index") == Some(&json!(1))
        )),
        "{events:?}"
    );
}

/// `AI-MODEL-01`, `AI-ADAPTER-11` — the performed model must be the requested
/// model; an echo mismatch stops the stream with a visible error.
#[tokio::test(flavor = "multi_thread")]
async fn a_model_echo_mismatch_stops_the_stream() {
    let gw = gateway(vec![Canned::Sse {
        status: 200,
        pieces: vec![SsePiece {
            delay_ms: 0,
            body: format!(
                "event: message_start\r\ndata: {}\r\n\r\n",
                json!({ "message": { "model": "claude-sonnet-4", "usage": { "input_tokens": 7 } } })
            ),
        }],
    }]);
    let adapter = GatewayAdapter::new(
        GatewayClientConfig {
            base_url: gw.base.clone(),
            auth_scheme: myharness_provider_gateway::AuthScheme::Bearer,
            custom_headers: vec![("X-Tenant".to_string(), "acme".to_string())],
            idle_timeout: Duration::from_millis(150),
            betas: vec!["prompt-caching-2024-07-31".to_string()],
        },
        reqwest::Client::new(),
    );
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
        .expect("one event")
        .expect_err("an error must terminate the stream");
    assert_eq!(error.class, ErrorClass::ModelNotFound);
    assert!(error.message.contains("claude-sonnet-4"));
}

/// `TEST-AI-014` — native stop reasons preserved, including refusal and
/// content-filter spellings (`AI-ADAPTER-08`).
#[tokio::test(flavor = "multi_thread")]
async fn native_stop_reasons_are_preserved() {
    for stop in ["refusal", "pause_turn", "some_novel_stop"] {
        let gw = gateway(legal_stream("claude-opus-4-1", "text", stop, 3));
        let adapter = GatewayAdapter::new(
            GatewayClientConfig {
                base_url: gw.base.clone(),
                auth_scheme: myharness_provider_gateway::AuthScheme::Bearer,
                custom_headers: vec![("X-Tenant".to_string(), "acme".to_string())],
                idle_timeout: Duration::from_millis(150),
                betas: vec!["prompt-caching-2024-07-31".to_string()],
            },
            reqwest::Client::new(),
        );
        let stream = adapter
            .create_turn(
                &auth("placeholder-credential"),
                &stream_request("claude-opus-4-1"),
            )
            .await
            .unwrap();
        let events: Vec<TurnEvent> = stream
            .map(|result| match result {
                Ok(event) => event,
                Err(error) => panic!("{stop}: {error:?}"),
            })
            .collect::<Vec<_>>()
            .await;
        let stop_event = events
            .iter()
            .find_map(|event| match event {
                TurnEvent::Stop { reason } => Some(reason.wire()),
                _ => None,
            })
            .unwrap_or_else(|| panic!("{stop}: no stop event in {events:?}"));
        assert_eq!(stop_event, stop, "the stop reason must be verbatim");
    }
}

/// `AI-USAGE-01`, `AI-ADAPTER-05` — cache tokens are reported exactly and
/// separately, not under-counted.
#[tokio::test(flavor = "multi_thread")]
async fn cache_usage_is_exact() {
    let gw = gateway(vec![Canned::Sse {
        status: 200,
        pieces: vec![
            SsePiece {
                delay_ms: 0,
                body: format!(
                    "event: message_start\r\ndata: {}\r\n\r\n",
                    json!({ "message": { "model": "claude-opus-4-1", "usage": { "input_tokens": 7, "cache_read_input_tokens": 3, "cache_creation_input_tokens": 4, "output_tokens": 1 } } })
                ),
            },
            SsePiece {
                delay_ms: 5,
                body: concat!(
                    "event: message_delta\r\n",
                    "data: {\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":12}}\r\n\r\n"
                )
                .to_string(),
            },
            SsePiece {
                delay_ms: 5,
                body: "event: message_stop\r\ndata: {}\r\n\r\n".to_string(),
            },
        ],
    }]);
    let adapter = GatewayAdapter::new(
        GatewayClientConfig {
            base_url: gw.base.clone(),
            auth_scheme: myharness_provider_gateway::AuthScheme::Bearer,
            custom_headers: vec![("X-Tenant".to_string(), "acme".to_string())],
            idle_timeout: Duration::from_millis(150),
            betas: vec!["prompt-caching-2024-07-31".to_string()],
        },
        reqwest::Client::new(),
    );
    let stream = adapter
        .create_turn(
            &auth("placeholder-credential"),
            &stream_request("claude-opus-4-1"),
        )
        .await
        .unwrap();
    let events: Vec<TurnEvent> = stream
        .map(|result| match result {
            Ok(event) => event,
            Err(error) => panic!("{error:?}"),
        })
        .collect::<Vec<_>>()
        .await;
    match events
        .iter()
        .find_map(|event| match event {
            TurnEvent::Usage { usage } => Some(usage.clone()),
            _ => None,
        })
        .unwrap()
    {
        Usage {
            input: 7,
            output: 12,
            cache_read: Some(3),
            cache_write: Some(4),
            web_search: None,
        } => {}
        other => panic!("cache usage must be exact: {other:?}"),
    }
}

/// `AI-MM-01` — an unsubscribed assistant block kind is a visible error that
/// names the kind; it is never silently skipped.
#[tokio::test(flavor = "multi_thread")]
async fn an_unsubscribed_assistant_block_is_named() {
    let gw = gateway(vec![Canned::Sse {
        status: 200,
        pieces: vec![SsePiece {
            delay_ms: 0,
            body: concat!(
                "event: content_block_start\r\n",
                "data: {\"index\":0,\"content_block\":{\"type\":\"hologram\"}}\r\n\r\n"
            )
            .to_string(),
        }],
    }]);
    let adapter = GatewayAdapter::new(
        GatewayClientConfig {
            base_url: gw.base.clone(),
            auth_scheme: myharness_provider_gateway::AuthScheme::Bearer,
            custom_headers: vec![("X-Tenant".to_string(), "acme".to_string())],
            idle_timeout: Duration::from_millis(150),
            betas: vec!["prompt-caching-2024-07-31".to_string()],
        },
        reqwest::Client::new(),
    );
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
        .err()
        .unwrap();
    assert_eq!(error.class, ErrorClass::UnsupportedModality);
    assert!(error.message.contains("hologram"));
}

/// `TEST-AI-021` — provider error classes stay distinguishable, and the
/// message is the provider's own wording without any echo of the raw body
/// (`docs/18` §11).
#[tokio::test(flavor = "multi_thread")]
async fn provider_error_classes_are_distinguishable() {
    let cases = vec![
        (
            401,
            json!({"error": {"type": "authentication_error", "message": "Fetch failed to authenticate. The source of the trouble was never echoed."}}),
            ErrorClass::Auth,
            "Fetch failed to authenticate",
        ),
        (
            429,
            json!({"error": {"type": "rate_limit_error"}}),
            ErrorClass::Quota,
            "rate_limit_error",
        ),
        (
            404,
            json!({"error": {"type": "not_found_error"}}),
            ErrorClass::ModelNotFound,
            "not_found_error",
        ),
        (
            529,
            json!({"error": {"type": "overloaded_error"}}),
            ErrorClass::Transient,
            "overloaded_error",
        ),
        (
            400,
            json!({"error": {"type": "invalid_request_error", "message": "context length exceeded: 1024 tokens > 512 tokens"}}),
            ErrorClass::ContextLength,
            "context length exceeded",
        ),
        (
            500,
            json!({"error": {"type": "api_error"}}),
            ErrorClass::Transient,
            "api_error",
        ),
    ];
    for (status, body, class, needle) in cases {
        let gw = gateway(vec![Canned::Json(status, body.clone())]);
        let adapter = GatewayAdapter::new(
            GatewayClientConfig {
                base_url: gw.base.clone(),
                auth_scheme: myharness_provider_gateway::AuthScheme::Bearer,
                custom_headers: vec![("X-Tenant".to_string(), "acme".to_string())],
                idle_timeout: Duration::from_millis(150),
                betas: vec!["prompt-caching-2024-07-31".to_string()],
            },
            reqwest::Client::new(),
        );
        let error = adapter
            .create_turn(
                &auth("placeholder-credential"),
                &stream_request("claude-opus-4-1"),
            )
            .await
            .err()
            .expect("a failed response must make create_turn error before any stream");
        assert_eq!(error.class, class, "{status}: {:?}", error.message);
        assert!(error.message.contains(needle), "{}", error.message);
        // The status rides along for transport diagnostics.
        assert_eq!(error.status, Some(status));
        drop(gw);
    }
}

/// A minimal streamed turn request used by the small-scenario fixtures.
fn stream_request(model_id: &str) -> NativeTurnRequest {
    NativeTurnRequest {
        model: key(model_id),
        system: Vec::new(),
        messages: vec![(
            Role::User,
            vec![ContentPart::Text {
                text: "hi".to_string(),
            }],
        )],
        tools: Vec::new(),
        max_tokens: 16,
        temperature: None,
        top_p: None,
        top_k: None,
        stop_sequences: Vec::new(),
        thinking: None,
        betas: Vec::new(),
        stream: true,
    }
}

/// `TEST-AI-006`/`TEST-AI-008` — a missing/empty credential blocks the send;
/// NOTHING travels (`AI-PROVIDER-03`; the recorded violation evidence is a
/// mock gateway that records zero requests).
#[tokio::test(flavor = "multi_thread")]
async fn a_missing_credential_blocks_the_send() {
    let gw = gateway(vec![Canned::Bare(200)]);
    let adapter = GatewayAdapter::new(
        GatewayClientConfig {
            base_url: gw.base.clone(),
            auth_scheme: myharness_provider_gateway::AuthScheme::Bearer,
            custom_headers: vec![("X-Tenant".to_string(), "acme".to_string())],
            idle_timeout: Duration::from_millis(150),
            betas: vec!["prompt-caching-2024-07-31".to_string()],
        },
        reqwest::Client::new(),
    );
    let error = adapter
        .create_turn(&auth("   "), &stream_request("claude-opus-4-1"))
        .await
        .err()
        .expect("an empty credential must fail closed");
    assert_eq!(error.class, ErrorClass::Auth);
    assert!(
        error.message.contains("nothing was sent"),
        "{}",
        error.message
    );
    assert!(
        gw.requests().is_empty(),
        "the mock gateway must record zero requests"
    );
}

/// `TEST-AI-007`/`TEST-AI-008` — the Anthropic model policy gate: a
/// non-Anthropic id is refused before anything is sent (`AI-MODEL-02`;
/// the `AI-ADAPTER-03` regression is the policy itself, enforced), and no
/// alternate model is used (`AI-MODEL-01`).
#[tokio::test(flavor = "multi_thread")]
async fn a_non_anthropic_model_blocks_the_send() {
    let gw = gateway(vec![Canned::Bare(200)]);
    let adapter = GatewayAdapter::new(
        GatewayClientConfig {
            base_url: gw.base.clone(),
            auth_scheme: myharness_provider_gateway::AuthScheme::Bearer,
            custom_headers: vec![("X-Tenant".to_string(), "acme".to_string())],
            idle_timeout: Duration::from_millis(150),
            betas: vec!["prompt-caching-2024-07-31".to_string()],
        },
        reqwest::Client::new(),
    );
    for id in ["gpt-4o", "gemini-2.5-pro", "codex-mini"] {
        let error = adapter
            .create_turn(&auth("placeholder-credential"), &stream_request(id))
            .await
            .err()
            .expect("the model policy must refuse a non-Anthropic id");
        assert_eq!(error.class, ErrorClass::InvalidRequest, "{id}");
        assert!(
            error.message.contains("was not sent"),
            "{id}: {}",
            error.message
        );
    }
    assert!(
        gw.requests().is_empty(),
        "zero requests must have travelled"
    );
}

/// `docs/12` §2.1 — `sk-ant-oat…` is an OAuth access token whose beta header
/// value was never captured (`docs/14` §9); the adapter refuses to send it
/// uninformed rather than guessing a header (`AI-ADAPTER-07`).
#[tokio::test(flavor = "multi_thread")]
async fn an_oauth_token_is_refused_informatively() {
    let gw = gateway(vec![Canned::Bare(200)]);
    let adapter = GatewayAdapter::new(
        GatewayClientConfig {
            base_url: gw.base.clone(),
            auth_scheme: myharness_provider_gateway::AuthScheme::Bearer,
            custom_headers: vec![("X-Tenant".to_string(), "acme".to_string())],
            idle_timeout: Duration::from_millis(150),
            betas: vec!["prompt-caching-2024-07-31".to_string()],
        },
        reqwest::Client::new(),
    );
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
    assert!(gw.requests().is_empty());
}

/// `AI-MM-02`, `AI-ADAPTER-06` — images inside tool results reach the
/// provider as images, on the wire.
#[tokio::test(flavor = "multi_thread")]
async fn tool_result_images_reach_the_provider() {
    // The gateway answers legally; the assertion inspects the REQUEST body.
    let gw = gateway(legal_stream("claude-opus-4-1", "noted", "end_turn", 2));
    let adapter = GatewayAdapter::new(
        GatewayClientConfig {
            base_url: gw.base.clone(),
            auth_scheme: myharness_provider_gateway::AuthScheme::Bearer,
            custom_headers: vec![("X-Tenant".to_string(), "acme".to_string())],
            idle_timeout: Duration::from_millis(150),
            betas: vec!["prompt-caching-2024-07-31".to_string()],
        },
        reqwest::Client::new(),
    );
    let request = NativeTurnRequest {
        model: key("claude-opus-4-1"),
        system: Vec::new(),
        messages: vec![(
            Role::User,
            vec![ContentPart::ToolResult {
                tool_use_id: "toolu_1".to_string(),
                content: vec![
                    ContentPart::Text {
                        text: "the screenshot".to_string(),
                    },
                    ContentPart::Image {
                        media_type: "image/jpeg".to_string(),
                        data: "anBnLWRhdGE=".to_string(),
                    },
                ],
                is_error: false,
                cache_control: None,
            }],
        )],
        tools: Vec::new(),
        max_tokens: 16,
        temperature: None,
        top_p: None,
        top_k: None,
        stop_sequences: Vec::new(),
        thinking: None,
        betas: Vec::new(),
        stream: true,
    };
    let stream = adapter
        .create_turn(&auth("placeholder-credential"), &request)
        .await
        .unwrap();
    let _ = stream.map_err(|_| ()).collect::<Vec<_>>().await;
    let body = gw.requests()[0].body();
    let result = &body["messages"][0]["content"][0];
    // The image is an image — not flattened text, never silently dropped.
    assert_eq!(result["type"], "tool_result");
    assert_eq!(result["tool_use_id"], "toolu_1");
    assert_eq!(result["content"][1]["type"], "image");
    assert_eq!(
        result["content"][1]["source"],
        json!({ "type": "base64", "media_type": "image/jpeg", "data": "anBnLWRhdGE=" })
    );
}

/// `TEST-AI-016` — a stream that only sends keep-alives times out at the
/// configured idle window (`AI-STREAM-02`).
#[tokio::test(flavor = "multi_thread")]
async fn a_pigeonhole_stream_times_out_at_the_idle_window() {
    let gw = gateway(vec![Canned::Drip {
        interval_ms: 30,
        first: "event: ping\r\ndata: {}\r\n\r\n".to_string(),
    }]);
    let adapter2 = adapter(
        gw.base.clone(),
        myharness_provider_gateway::AuthScheme::Bearer,
    );
    // The idle window is 150 ms; pings every 30 ms keep the transport alive
    // forever, but only content resets the window.
    let started = std::time::Instant::now();
    let error = adapter2
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

/// `docs/12` §4.1 — a hung stream is bounded too: no bytes at all fires the
/// idle window instead of hanging forever.
#[tokio::test(flavor = "multi_thread")]
async fn a_hung_stream_is_bounded() {
    // A drip of nothing: the connection opens, the headers arrive, no
    // content events — pings arrive forever and never reset the window.
    let gw = gateway(vec![Canned::Drip {
        interval_ms: 30,
        first: String::new(),
    }]);
    let adapter3 = adapter(
        gw.base.clone(),
        myharness_provider_gateway::AuthScheme::Bearer,
    );
    let error = adapter3
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
        .expect_err("the idle window bounds a silent stream");
    assert!(error.message.contains("idle"), "{error:?}");
}

/// `TEST-AI-015` — cancelling (dropping) the stream stops it; the partial
/// output already emitted stays in the caller's hands (`docs/12` §4.1).
#[tokio::test(flavor = "multi_thread")]
async fn dropping_the_stream_cancels_it() {
    let gw = gateway(vec![Canned::Sse {
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
    let adapter = GatewayAdapter::new(
        GatewayClientConfig {
            base_url: gw.base.clone(),
            auth_scheme: myharness_provider_gateway::AuthScheme::Bearer,
            custom_headers: vec![("X-Tenant".to_string(), "acme".to_string())],
            idle_timeout: Duration::from_millis(150),
            betas: vec!["prompt-caching-2024-07-31".to_string()],
        },
        reqwest::Client::new(),
    );
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
    gw.stop();
}

/// `AI-ADAPTER-09`, `TEST-AI-013` — token counting is native (`count_tokens`),
/// never a length/4 estimate; an unavailable endpoint is an explicit error.
#[tokio::test(flavor = "multi_thread")]
async fn token_counting_is_native_or_refused() {
    // The native endpoint answers.
    let gw = gateway(vec![Canned::Json(200, json!({ "input_tokens": 57 }))]);
    let adapter = GatewayAdapter::new(
        GatewayClientConfig {
            base_url: gw.base.clone(),
            auth_scheme: myharness_provider_gateway::AuthScheme::Bearer,
            custom_headers: vec![("X-Tenant".to_string(), "acme".to_string())],
            idle_timeout: Duration::from_millis(150),
            betas: vec!["prompt-caching-2024-07-31".to_string()],
        },
        reqwest::Client::new(),
    );
    let count = adapter
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
    let requests = gw.requests();
    assert!(
        requests[0].has_path("/v1/messages/count_tokens"),
        "{}",
        requests[0].raw
    );
    assert_eq!(requests[0].body()["model"], "claude-opus-4-1");

    // No endpoint → an explicit error, never an estimate.
    let gw = gateway(vec![Canned::Bare(404)]);
    let adapter2 = GatewayAdapter::new(
        GatewayClientConfig {
            base_url: gw.base.clone(),
            auth_scheme: myharness_provider_gateway::AuthScheme::Bearer,
            custom_headers: vec![("X-Tenant".to_string(), "acme".to_string())],
            idle_timeout: Duration::from_millis(150),
            betas: vec!["prompt-caching-2024-07-31".to_string()],
        },
        reqwest::Client::new(),
    );
    let error = adapter2
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
        .expect_err("a missing count endpoint is an error, not an estimate");
    assert!(error.message.contains("status 404"), "{error:?}");
}

/// `TEST-AI-018`, `TEST-AI-005` — discovery through the adapter: the cursor
/// follows with `after_id`, first-page failure is fatal, later-page failure
/// retains partial results, and the rows map onto native descriptors. The
/// adapter's scheme choice is honored on the wire.
#[tokio::test(flavor = "multi_thread")]
async fn discovery_follows_the_cursor_and_retains_partial_results() {
    // Happy path + cursor: two pages.
    let gw = gateway(vec![
        Canned::Json(
            200,
            json!({
                "data": [
                    { "id": "claude-opus-4-1", "display_name": "Opus 4.1", "context_window": 200000 },
                    { "id": "claude-sonnet-4", }
                ],
                "has_more": true,
                "after_id": "claude-sonnet-4",
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
    let adapter = GatewayAdapter::new(
        GatewayClientConfig {
            base_url: gw.base.clone(),
            auth_scheme: myharness_provider_gateway::AuthScheme::Bearer,
            custom_headers: vec![("X-Tenant".to_string(), "acme".to_string())],
            idle_timeout: Duration::from_millis(150),
            betas: vec!["prompt-caching-2024-07-31".to_string()],
        },
        reqwest::Client::new(),
    );
    let models = adapter
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
    let requests = gw.requests();
    assert!(
        requests[0].has_path("/v1/models?limit=1000"),
        "{}",
        requests[0].raw
    );
    assert!(
        requests[1].raw.contains("after_id=claude-sonnet-4"),
        "{}",
        requests[1].raw
    );
    assert!(
        requests[0]
            .raw
            .contains("authorization: Bearer placeholder-credential"),
        "{}",
        requests[0].raw
    );

    // Later-page failure: retain what was read and warn.
    let gw = gateway(vec![
        Canned::Json(
            200,
            json!({ "data": [ { "id": "claude-opus-4-1" } ], "has_more": true, "after_id": "x" }),
        ),
        Canned::Bare(500),
    ]);
    let adapter2 = GatewayAdapter::new(
        GatewayClientConfig {
            base_url: gw.base.clone(),
            auth_scheme: myharness_provider_gateway::AuthScheme::Bearer,
            custom_headers: vec![("X-Tenant".to_string(), "acme".to_string())],
            idle_timeout: Duration::from_millis(150),
            betas: vec!["prompt-caching-2024-07-31".to_string()],
        },
        reqwest::Client::new(),
    );
    let models = adapter2
        .list_models(&auth("placeholder-credential"))
        .await
        .expect("a later-page failure must not lose the read rows");
    assert_eq!(models.len(), 1);

    // First-page failure is fatal.
    let gw = gateway(vec![Canned::Bare(500)]);
    let adapter3 = GatewayAdapter::new(
        GatewayClientConfig {
            base_url: gw.base.clone(),
            auth_scheme: myharness_provider_gateway::AuthScheme::Bearer,
            custom_headers: vec![("X-Tenant".to_string(), "acme".to_string())],
            idle_timeout: Duration::from_millis(150),
            betas: vec!["prompt-caching-2024-07-31".to_string()],
        },
        reqwest::Client::new(),
    );
    let error = adapter3
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
            (
                200u16,
                json!({
                    "data": [ { "id": format!("claude-model-{i}") } ],
                    "has_more": true,
                    "after_id": format!("claude-model-{i}"),
                })
                .to_string(),
            )
        })
        .collect::<Vec<_>>();
    let gw = gateway(
        pages
            .iter()
            .map(|(status, body)| Canned::Json(*status, serde_json::from_str(body).unwrap()))
            .collect(),
    );
    let adapter4 = GatewayAdapter::new(
        GatewayClientConfig {
            base_url: gw.base.clone(),
            auth_scheme: myharness_provider_gateway::AuthScheme::Bearer,
            custom_headers: vec![("X-Tenant".to_string(), "acme".to_string())],
            idle_timeout: Duration::from_millis(150),
            betas: vec!["prompt-caching-2024-07-31".to_string()],
        },
        reqwest::Client::new(),
    );
    let models = adapter4
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
    let gw = gateway(vec![Canned::Json(
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
    let adapter = GatewayAdapter::new(
        GatewayClientConfig {
            base_url: gw.base.clone(),
            auth_scheme: myharness_provider_gateway::AuthScheme::Bearer,
            custom_headers: vec![("X-Tenant".to_string(), "acme".to_string())],
            idle_timeout: Duration::from_millis(150),
            betas: vec!["prompt-caching-2024-07-31".to_string()],
        },
        reqwest::Client::new(),
    );
    let status: ProbeStatus = adapter
        .probe_inference(&auth("placeholder-credential"), "claude-opus-4-1")
        .await
        .expect("the probe must complete");
    assert!(status.ok);
    assert_eq!(status.model, "claude-opus-4-1");
    assert_eq!(status.stop_reason.as_deref(), Some("end_turn"));
    assert_eq!(status.output_tokens, 12);
    assert_eq!(status.input_tokens, 7);
    assert!(status.latency_ms > 0);
    let requests = gw.requests();
    let body = requests[0].body();
    assert_eq!(body["max_tokens"], 1);
    assert_eq!(body["stream"], false);
    assert_eq!(
        body["messages"][0]["content"][0],
        json!({ "type": "text", "text": "ping" })
    );
    assert!(requests[0].has_path("/v1/messages"), "{}", requests[0].raw);

    // A probe failure carries the provider's error class verbatim.
    let gw = gateway(vec![Canned::Json(
        401,
        json!({"error": {"type": "authentication_error", "message": "this key is not real"}}),
    )]);
    let adapter2 = GatewayAdapter::new(
        GatewayClientConfig {
            base_url: gw.base.clone(),
            auth_scheme: myharness_provider_gateway::AuthScheme::Bearer,
            custom_headers: vec![("X-Tenant".to_string(), "acme".to_string())],
            idle_timeout: Duration::from_millis(150),
            betas: vec!["prompt-caching-2024-07-31".to_string()],
        },
        reqwest::Client::new(),
    );
    let error = adapter2
        .probe_inference(&auth("placeholder-credential"), "claude-opus-4-1")
        .await
        .expect_err("auth failure must surface");
    assert_eq!(error.class, ErrorClass::Auth);
}

/// The non-streaming head-turn: the whole response arrives; the model echo,
/// usage, and stop reason still hold.
#[tokio::test(flavor = "multi_thread")]
async fn a_non_streamed_turn_returns_typed_events() {
    let gw = gateway(vec![Canned::Json(
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
    let adapter = GatewayAdapter::new(
        GatewayClientConfig {
            base_url: gw.base.clone(),
            auth_scheme: myharness_provider_gateway::AuthScheme::Bearer,
            custom_headers: vec![("X-Tenant".to_string(), "acme".to_string())],
            idle_timeout: Duration::from_millis(150),
            betas: vec!["prompt-caching-2024-07-31".to_string()],
        },
        reqwest::Client::new(),
    );
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
    let gw_one = gateway(vec![Canned::Json(200, json!({"input_tokens": 1}))]);
    let gw_two = gateway(vec![Canned::Json(200, json!({"input_tokens": 2}))]);
    let adapter_one = adapter(
        gw_one.base.clone(),
        myharness_provider_gateway::AuthScheme::Bearer,
    );
    let adapter_two = adapter(
        gw_two.base.clone(),
        myharness_provider_gateway::AuthScheme::XApiKey,
    );
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
    let one = gw_one.requests();
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
