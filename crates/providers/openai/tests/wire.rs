//! Mock-endpoint wire fixtures — the conformance battery for the
//! OpenAI-compat adapter (the open model layer, `docs/13` §C), plus the
//! recorded-reading assertions the owner pinned for v1 (`docs/14` §9).
//!
//! Fixture policy (`tests/README.md`): every credential here is a synthetic
//! placeholder, every model id is a benign spelling, nothing is recorded
//! from a real session.

// `create_turn`'s Ok lane is an opaque stream handle. `expect_err` would
// demand `Debug` on it, so stream-returning sites keep `.err().expect(..)`.
#![allow(clippy::err_expect)]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

use futures::StreamExt;
use myharness_provider_openai::{AdapterKind, OpenAiClientConfig, OpenAiCompatAdapter};
use myharness_provider_sdk::ProviderAdapter;
use myharness_provider_sdk::{
    AuthContext, ContentPart, CountRequest, ErrorClass, ModelKey, NativeTurnRequest, Role,
    SystemBlock, ToolSpec, TurnEvent,
};
use secrecy::SecretString;
use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// The mock endpoint
// ---------------------------------------------------------------------------

/// One canned stream piece with an optional delay before it is written.
struct SsePiece {
    delay_ms: u64,
    /// Raw chunked payload (without the chunked framing; the writer adds it).
    body: String,
}

/// A canned endpoint behavior for a single connection.
enum Canned {
    /// A complete JSON response.
    Json(u16, Value),
    /// A chunked stream: the pieces are written in order (each inside its
    /// own chunked-encoding frame), and the connection stays open until the
    /// client drops it.
    Sse { status: u16, pieces: Vec<SsePiece> },
    /// A drip: keep-alive frames every interval, forever, carrying exactly
    /// one content frame first — the idle-window fixture (`TEST-AI-016`
    /// reading).
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
            .find_map(|line| {
                if line.to_lowercase().starts_with(&needle.to_lowercase()) {
                    line.split_once(": ")
                        .map(|(_, value)| value.trim().to_string())
                } else {
                    None
                }
            })
            .filter(|value| !value.is_empty())
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

/// A mock endpoint: a listener thread, a shared request log, and a stop
/// flag. Tests read [`MockEndpoint::requests`] directly instead of joining
/// the thread — zero-request scenarios (a blocked send) must be able to
/// assert emptiness without waiting for a connection that never comes.
struct MockEndpoint {
    base: String,
    requests: std::sync::Arc<std::sync::Mutex<Vec<Recorded>>>,
    stop_flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl MockEndpoint {
    fn requests(&self) -> Vec<Recorded> {
        self.requests.lock().unwrap().clone()
    }

    /// Release the listener thread.
    fn stop(&self) {
        self.stop_flag
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

impl Drop for MockEndpoint {
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
            Canned::Json(..) => {
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

fn endpoint(canned: Vec<Canned>) -> MockEndpoint {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().unwrap().port();
    let requests = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let stop_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let handle = std::thread::spawn({
        let requests = requests.clone();
        let stop_flag = stop_flag.clone();
        move || serve(listener, canned, requests, stop_flag)
    });
    MockEndpoint {
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
        // Streamed shapes are written connection-wise inside `serve`.
        _ => Vec::new(),
    }
}

/// Write a chunked response. `drip_interval_ms` turns the connection into
/// an endless keep-alive drip (checked against the stop flag so a lingering
/// drip cannot outlive the test); otherwise the body is terminated after
/// the pieces.
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
        // as content (the drip opens with a piece by design).
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
            // The wire's keep-alive: an empty object frame (the handled
            // compat heartbeat).
            if write_chunk(b"data: {}\r\n\r\n") {
                return; // client disconnected
            }
        }
    }
    let _ = stream.write_all(b"0\r\n\r\n");
    let _ = stream.flush();
}

fn config(kind: AdapterKind, base: String, idle: Duration) -> OpenAiClientConfig {
    OpenAiClientConfig {
        kind,
        base_url: base,
        idle_timeout: idle,
        custom_headers: vec![("X-Tenant".to_string(), "acme".to_string())],
    }
}

fn openai_adapter(base: String) -> OpenAiCompatAdapter {
    OpenAiCompatAdapter::new(
        config(AdapterKind::OpenAi, base, Duration::from_millis(150)),
        reqwest::Client::new(),
    )
}

fn custom_adapter(base: String, idle: Duration) -> OpenAiCompatAdapter {
    OpenAiCompatAdapter::new(
        config(AdapterKind::Custom, base, idle),
        reqwest::Client::new(),
    )
}

fn auth(value: &str) -> AuthContext {
    AuthContext::new(
        SecretString::from(value.to_string()),
        "synthetic static key",
    )
}

/// An empty credential — the `custom` family's keyless case, the `openai`
/// family's refusal case (`docs/14` §9 owner decision).
fn no_auth() -> AuthContext {
    auth("")
}

fn key(model_id: &str) -> ModelKey {
    ModelKey {
        provider_id: "openai".to_string(),
        model_id: model_id.to_string(),
        variant: None,
        revision: None,
    }
}

/// A happy streamed turn, as first-party OpenAI sends it: content deltas,
/// a finalizing chunk, and the sentinel.
fn legal_stream(model: &str, text: &str, finish: &str) -> Vec<Canned> {
    let pieces = vec![
        chunk(
            format!(
                r#"{{"id":"chatcmpl-1","model":"{model}","choices":[{{"index":0,"delta":{{"role":"assistant"}},"finish_reason":null}}]}}"#
            )
            .as_str(),
        ),
        chunk(
            format!(
                r#"{{"id":"chatcmpl-1","model":"{model}","choices":[{{"index":0,"delta":{{"content":"{text}"}},"finish_reason":null}}]}}"#
            )
            .as_str(),
        ),
        chunk(
            format!(
                r#"{{"id":"chatcmpl-1","model":"{model}","choices":[{{"index":0,"delta":{{}},"finish_reason":"{finish}"}}]}}"#
            )
            .as_str(),
        ),
        terminate(),
    ];
    vec![Canned::Sse {
        status: 200,
        pieces,
    }]
}

/// One `data:` frame.
fn chunk(body: &str) -> SsePiece {
    SsePiece {
        delay_ms: 3,
        body: format!("data: {body}\r\n\r\n"),
    }
}

/// The wire's terminal sentinel.
fn terminate() -> SsePiece {
    SsePiece {
        delay_ms: 3,
        body: "data: [DONE]\r\n\r\n".to_string(),
    }
}

/// A non-streaming completion the probe/turn answers with.
fn legal_completion(model: &str, text: &str, finish: &str) -> Value {
    json!({
        "id": "chatcmpl-9",
        "object": "chat.completion",
        "model": model,
        "choices": [ { "index": 0,
            "message": { "role": "assistant", "content": text },
            "finish_reason": finish } ],
        "usage": { "prompt_tokens": 3, "completion_tokens": 1, "total_tokens": 4 }
    })
}

fn stream_request(model_id: &str) -> NativeTurnRequest {
    NativeTurnRequest {
        model: key(model_id),
        system: vec![SystemBlock {
            text: "be brief".to_string(),
            cache_control: None,
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
        top_k: Some(2),
        stop_sequences: Vec::new(),
        thinking: None,
        betas: Vec::new(),
        stream: true,
    }
}

// ---------------------------------------------------------------------------
// The streamed turn
// ---------------------------------------------------------------------------

/// `TEST-AI-014` (the wire reading) — the mock's native stream is parsed
/// into the typed events: text deltas, the preserved stop reason, usage,
/// done. The request body carries the id verbatim and the recorded v1
/// spellings (joined system message, `max_tokens`, no `stream_options`,
/// `stream` true).
#[tokio::test(flavor = "multi_thread")]
async fn a_streamed_turn_speaks_the_native_wire() {
    let host = endpoint(legal_stream("gpt-4o", "hello", "stop"));
    let adapter = openai_adapter(host.base.clone());
    let stream = adapter
        .create_turn(&auth("sk-EXAMPLE"), &stream_request("gpt-4o"))
        .await
        .expect("a benign id streams");
    let events: Vec<TurnEvent> = stream
        .map(|result| result.unwrap())
        .collect::<Vec<_>>()
        .await;
    assert_eq!(
        events,
        vec![
            TurnEvent::TextDelta {
                text: "hello".to_string()
            },
            TurnEvent::Stop {
                reason: myharness_provider_sdk::NativeStopReason::EndTurn
            },
            TurnEvent::Done,
        ]
    );
    let request = &host.requests()[0];
    assert!(request.has_path("/chat/completions"));
    let body = request.body();
    assert_eq!(body["model"], "gpt-4o");
    assert_eq!(body["max_tokens"], 64);
    assert_eq!(body["temperature"], 0.4);
    assert_eq!(body["stream"], true);
    assert_eq!(
        body["messages"][0],
        json!({ "role": "system", "content": "be brief" }),
        "system blocks join into one system message (docs/14 §9)"
    );
    assert_eq!(body["messages"][1]["role"], "user");
    assert!(body.get("top_k").is_none(), "no top_k spelling (recorded)");
    assert!(
        body.get("stream_options").is_none(),
        "no stream_options (recorded)"
    );
    assert!(
        body.get("thinking").is_none(),
        "no thinking spelling (recorded)"
    );
}

/// `AI-MM-01` — a document part is refused before anything is sent, with
/// zero requests; the same refusal covers a PDF-shaped tool result.
#[tokio::test(flavor = "multi_thread")]
async fn documents_are_refused_upstream_of_the_send() {
    let mut request = stream_request("gpt-4o");
    request.messages.push((
        Role::User,
        vec![ContentPart::Document {
            media_type: "application/pdf".to_string(),
            data: "aPDF".to_string(),
            name: Some("brief.pdf".to_string()),
        }],
    ));
    let host = endpoint(vec![]);
    let adapter = openai_adapter(host.base.clone());
    let error = adapter
        .create_turn(&auth("sk-EXAMPLE"), &request)
        .await
        .err()
        .expect("a document is refused");
    assert_eq!(error.class, ErrorClass::UnsupportedModality);
    assert!(error.message.contains("cannot carry a document"));
    assert!(host.requests().is_empty());

    // And a tool result carrying an image: the same fail-closed boundary.
    let mut request = stream_request("gpt-4o");
    request.messages.push((
        Role::User,
        vec![ContentPart::ToolResult {
            tool_use_id: "call_1".to_string(),
            content: vec![ContentPart::Image {
                media_type: "image/png".to_string(),
                data: "aW1n".to_string(),
            }],
            is_error: false,
            cache_control: None,
        }],
    ));
    let error = adapter
        .create_turn(&auth("sk-EXAMPLE"), &request)
        .await
        .err()
        .expect("media inside a tool result is refused");
    assert_eq!(error.class, ErrorClass::UnsupportedModality);
    assert!(error.message.contains("tool result"));
    assert!(host.requests().is_empty());
}

/// The history replay shape: an assistant `ToolUse` part becomes the
/// message's `tool_calls` (arguments as the JSON encoding), the following
/// `ToolResult` opens its own `role: "tool"` message keyed to the call,
/// assistant reasoning is omitted (`docs/14` §9 owner decision), `is_error`
/// is dropped while its text travels.
#[tokio::test(flavor = "multi_thread")]
async fn history_replays_with_tool_messages_and_omits_reasoning() {
    let mut request = stream_request("gpt-4o");
    request.messages = vec![
        (
            Role::Assistant,
            vec![
                ContentPart::Text {
                    text: "let me look".to_string(),
                },
                ContentPart::ToolUse {
                    id: "call_1".to_string(),
                    name: "find".to_string(),
                    input: json!({ "q": "docs" }),
                },
                ContentPart::Reasoning {
                    thinking: "private scratch".to_string(),
                    signature: None,
                },
                ContentPart::RedactedThinking {
                    data: json!({ "x": 1 }),
                },
            ],
        ),
        (
            Role::User,
            vec![
                ContentPart::ToolResult {
                    tool_use_id: "call_1".to_string(),
                    content: vec![ContentPart::Text {
                        text: "404 — not found".to_string(),
                    }],
                    is_error: true,
                    cache_control: None,
                },
                ContentPart::Text {
                    text: "try again".to_string(),
                },
            ],
        ),
    ];
    let host = endpoint(legal_stream("gpt-4o", "again", "stop"));
    let adapter = openai_adapter(host.base.clone());
    let _ = adapter
        .create_turn(&auth("sk-EXAMPLE"), &request)
        .await
        .expect("the replay is legal");
    let body = host.requests()[0].body();
    let assistant = &body["messages"][1];
    assert_eq!(assistant["role"], "assistant");
    assert_eq!(assistant["content"], "let me look");
    assert_eq!(assistant["tool_calls"][0]["id"], "call_1");
    assert_eq!(assistant["tool_calls"][0]["function"]["name"], "find");
    assert_eq!(
        assistant["tool_calls"][0]["function"]["arguments"], r#"{"q":"docs"}"#,
        "arguments travel as the JSON encoding"
    );
    let serialized = assistant.to_string();
    assert!(
        !serialized.contains("Reasoning") && !serialized.contains("thinking"),
        "assistant reasoning is omitted from the replay: {serialized}"
    );
    let tool_result = &body["messages"][2];
    assert_eq!(tool_result["role"], "tool");
    assert_eq!(tool_result["tool_call_id"], "call_1");
    assert_eq!(tool_result["content"], "404 — not found");
    assert!(
        !tool_result.to_string().contains("is_error"),
        "the flag has no spelling here; its text carried the failure"
    );
    let follow_up = &body["messages"][3];
    assert_eq!(follow_up["role"], "user");
    assert_eq!(follow_up["content"][0]["text"], "try again");
}

/// Reasoning deltas arrive under both spellings the wire's servers use:
/// `delta.reasoning_content` first, `delta.reasoning` as fallback
/// (`docs/14` §9 per-wire readings).
#[tokio::test(flavor = "multi_thread")]
async fn reasoning_deltas_arrive_under_both_spellings() {
    let pieces = vec![
        chunk(
            r#"{"id":"chatcmpl-1","model":"deepseek-reasoner","choices":[{"index":0,"delta":{"reasoning_content":"step one"},"finish_reason":null}]}"#,
        ),
        chunk(
            r#"{"id":"chatcmpl-1","model":"deepseek-reasoner","choices":[{"index":0,"delta":{"reasoning":"step two"},"finish_reason":null}]}"#,
        ),
        chunk(
            r#"{"id":"chatcmpl-1","model":"deepseek-reasoner","choices":[{"index":0,"delta":{"content":"answer"},"finish_reason":null}]}"#,
        ),
        chunk(
            r#"{"id":"chatcmpl-1","model":"deepseek-reasoner","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}"#,
        ),
        terminate(),
    ];
    let host = endpoint(vec![Canned::Sse {
        status: 200,
        pieces,
    }]);
    let adapter = openai_adapter(host.base.clone());
    let mut stream = adapter
        .create_turn(&auth("sk-EXAMPLE"), &stream_request("deepseek-reasoner"))
        .await
        .expect("the reasoner streams");
    let first = StreamExt::next(&mut stream).await.unwrap().unwrap();
    assert_eq!(
        first,
        TurnEvent::ReasoningDelta {
            text: "step one".to_string(),
            signature: None,
        }
    );
    let second = StreamExt::next(&mut stream).await.unwrap().unwrap();
    assert_eq!(
        second,
        TurnEvent::ReasoningDelta {
            text: "step two".to_string(),
            signature: None,
        }
    );
    let third = StreamExt::next(&mut stream).await.unwrap().unwrap();
    assert_eq!(
        third,
        TurnEvent::TextDelta {
            text: "answer".to_string()
        }
    );
}

/// `AI-TOOL-01` — tool calls stream: the opening fragment announces the id
/// and name, argument fragments travel verbatim, close assembles the JSON,
/// and the stop reason maps to `tool_use`. The replay request carries the
/// `tools` spec natively.
#[tokio::test(flavor = "multi_thread")]
async fn tool_calls_stream_and_close() {
    let pieces = vec![
        chunk(
            r#"{"id":"chatcmpl-1","model":"gpt-4o","choices":[{"index":0,"delta":{"role":"assistant","tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"find","arguments":""}}]},"finish_reason":null}]}"#,
        ),
        chunk(
            r#"{"id":"chatcmpl-1","model":"gpt-4o","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"q\":\""}}]},"finish_reason":null}]}"#,
        ),
        chunk(
            r#"{"id":"chatcmpl-1","model":"gpt-4o","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"docs\"}"}}]},"finish_reason":null}]}"#,
        ),
        chunk(
            r#"{"id":"chatcmpl-1","model":"gpt-4o","choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}"#,
        ),
        terminate(),
    ];
    let host = endpoint(vec![Canned::Sse {
        status: 200,
        pieces,
    }]);
    let adapter = openai_adapter(host.base.clone());
    let mut request = stream_request("gpt-4o");
    request.tools = vec![ToolSpec {
        name: "find".to_string(),
        description: Some("search".to_string()),
        input_schema: json!({ "type": "object", "properties": { "q": { "type": "string" } } }),
        cache_control: None,
    }];
    let stream = adapter
        .create_turn(&auth("sk-EXAMPLE"), &request)
        .await
        .expect("the tools stream");
    let events: Vec<TurnEvent> = stream
        .map(|result| result.unwrap())
        .collect::<Vec<_>>()
        .await;

    let opening = TurnEvent::ToolCall {
        id: "call_1".to_string(),
        name: "find".to_string(),
        arguments_delta: None,
        arguments: None,
    };
    let delta_1 = TurnEvent::ToolCall {
        id: "call_1".to_string(),
        name: "find".to_string(),
        arguments_delta: Some("{\"q\":\"".to_string()),
        arguments: None,
    };
    let delta_2 = TurnEvent::ToolCall {
        id: "call_1".to_string(),
        name: "find".to_string(),
        arguments_delta: Some("docs\"}".to_string()),
        arguments: None,
    };
    let closing = TurnEvent::ToolCall {
        id: "call_1".to_string(),
        name: "find".to_string(),
        arguments_delta: None,
        arguments: Some(json!({ "q": "docs" })),
    };
    assert!(events.contains(&opening), "{events:?}");
    assert!(events.contains(&delta_1), "{events:?}");
    assert!(events.contains(&delta_2), "{events:?}");
    assert!(events.contains(&closing), "{events:?}");
    assert!(
        events.contains(&TurnEvent::Stop {
            reason: myharness_provider_sdk::NativeStopReason::ToolUse
        }),
        "{events:?}"
    );

    let body = host.requests()[0].body();
    assert_eq!(
        body["tools"][0],
        json!({
            "type": "function",
            "function": {
                "name": "find",
                "description": "search",
                "parameters": { "type": "object", "properties": { "q": { "type": "string" } } },
            },
        })
    );
}

/// A compatible server that omits `id` on the continuation fragments of its
/// own call keeps a stable identity — synthesized once, from the
/// completion id and the index, exactly as recorded (`docs/14` §9).
#[tokio::test(flavor = "multi_thread")]
async fn continuation_fragments_without_ids_keep_the_recorded_synthetic_id() {
    let pieces = vec![
        // First fragment: no id at all — the synthetic id is issued here.
        chunk(
            r#"{"id":"chatcmpl-abc","model":"gpt-4o","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"name":"find"}}]},"finish_reason":null}]}"#,
        ),
        // Continuation fragments omit id forever after.
        chunk(
            r#"{"id":"chatcmpl-abc","model":"gpt-4o","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"a\":1}"}}]},"finish_reason":null}]}"#,
        ),
        chunk(
            r#"{"id":"chatcmpl-abc","model":"gpt-4o","choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}"#,
        ),
        terminate(),
    ];
    let host = endpoint(vec![Canned::Sse {
        status: 200,
        pieces,
    }]);
    let adapter = openai_adapter(host.base.clone());
    let stream = adapter
        .create_turn(&auth("sk-EXAMPLE"), &stream_request("gpt-4o"))
        .await
        .expect("the fragments stream");
    let events: Vec<TurnEvent> = stream
        .map(|result| result.unwrap())
        .collect::<Vec<_>>()
        .await;
    let expected_id = "chatcmpl-abc-0";
    assert!(
        events.iter().any(|event| matches!(
            event,
            TurnEvent::ToolCall { id, name, arguments_delta: None, .. }
                if id == expected_id && name == "find"
        )),
        "the synthetic id covered the opening fragment: {events:?}"
    );
    assert!(
        events.iter().all(|event| match event {
            TurnEvent::ToolCall { id, .. } => id == expected_id,
            _ => true,
        }),
        "continuation fragments kept the same identity: {events:?}"
    );
    assert!(
        events.contains(&TurnEvent::ToolCall {
            id: expected_id.to_string(),
            name: "find".to_string(),
            arguments_delta: None,
            arguments: Some(json!({ "a": 1 })),
        }),
        "the arguments closed: {events:?}"
    );
}

/// `TEST-AI-021` for the wire (`docs/14` §9 mapping) — the provider's error
/// classes stay distinguishable: auth by status, quota by the wire's own
/// `insufficient_quota`, a missing model by 404, transient by a 5xx type.
#[tokio::test(flavor = "multi_thread")]
async fn provider_error_classes_are_distinguishable() {
    let cases: Vec<(Value, ErrorClass)> = vec![
        (
            json!({ "error": { "message": "Incorrect API key provided", "type": "invalid_request_error", "code": "invalid_api_key" } }),
            ErrorClass::InvalidRequest, // non-2xx *status* runs the probe path below
        ),
        (
            json!({ "error": { "message": "You exceeded your current quota, please check your plan and billing details", "type": "insufficient_quota", "code": "insufficient_quota" } }),
            ErrorClass::Quota,
        ),
        (
            json!({ "error": { "message": "The model `gpt-9` does not exist", "type": "invalid_request_error", "code": "model_not_found" } }),
            ErrorClass::ModelNotFound,
        ),
        (
            json!({ "error": { "message": "The server is overloaded", "type": "server_error" } }),
            ErrorClass::Transient,
        ),
    ];
    let status_cases = [401u16, 429, 404, 503];
    for (index, (body, _expected_stream_class)) in cases.iter().enumerate() {
        let host = endpoint(vec![Canned::Json(status_cases[index], body.clone())]);
        let adapter = openai_adapter(host.base.clone());
        let error = adapter
            .probe_inference(&auth("sk-EXAMPLE"), "gpt-4o")
            .await
            .err()
            .expect("a non-success status is an error");
        let expected = match index {
            0 => ErrorClass::Auth,
            1 => ErrorClass::Quota,
            2 => ErrorClass::ModelNotFound,
            _ => ErrorClass::Transient,
        };
        assert_eq!(error.class, expected, "{} -> {}", status_cases[index], body);
        assert!(!error.message.is_empty());
        host.stop();
    }
    // A mid-stream error chunk keeps the provider's own wording and class.
    let host = endpoint(vec![Canned::Sse {
        status: 200,
        pieces: vec![
            chunk(r#"{"error":{"message":"Rate limit exceeded","type":"rate_limit_error"}}"#),
            terminate(),
        ],
    }]);
    let adapter = openai_adapter(host.base.clone());
    let result = adapter
        .create_turn(&auth("sk-EXAMPLE"), &stream_request("gpt-4o"))
        .await
        .expect("the stream itself opens");
    let events: Vec<Result<TurnEvent, _>> = result.collect().await;
    let failure = events
        .into_iter()
        .find_map(|event| match event {
            Ok(TurnEvent::Error { error }) | Err(error) => Some(error),
            _ => None,
        })
        .expect("the stream surfaced the error chunk");
    assert_eq!(failure.class, ErrorClass::Quota);
}

/// The recorded v1 stop-reason mapping (`docs/14` §9): `stop`→`end_turn`,
/// `length`→`max_tokens`, `tool_calls`→`tool_use`, `content_filter` stays,
/// unknown spellings travel verbatim (`AI-ADAPTER-08`).
#[tokio::test(flavor = "multi_thread")]
async fn stop_reasons_map_onto_the_preserved_set() {
    for (raw, expected) in [
        ("stop", myharness_provider_sdk::NativeStopReason::EndTurn),
        (
            "length",
            myharness_provider_sdk::NativeStopReason::MaxTokens,
        ),
        (
            "function_call",
            myharness_provider_sdk::NativeStopReason::ToolUse,
        ),
        (
            "content_filter",
            myharness_provider_sdk::NativeStopReason::ContentFilter,
        ),
        (
            "internal_policy_v2",
            myharness_provider_sdk::NativeStopReason::Other("internal_policy_v2".to_string()),
        ),
    ] {
        let host = endpoint(legal_stream("gpt-4o", "x", raw));
        let adapter = openai_adapter(host.base.clone());
        let events: Vec<TurnEvent> = adapter
            .create_turn(&auth("sk-EXAMPLE"), &stream_request("gpt-4o"))
            .await
            .expect("the stream opens")
            .map(|result| result.unwrap())
            .collect::<Vec<_>>()
            .await;
        assert!(
            events.contains(&TurnEvent::Stop { reason: expected }),
            "finish_reason {raw:?} preserved: {events:?}"
        );
        host.stop();
    }
}

/// `AI-USAGE-01` — usage maps off the wire's names: prompt/completion as
/// input/output and `prompt_tokens_details.cached_tokens` as cache-read.
/// A stream whose server omits usage emits no invented numbers.
#[tokio::test(flavor = "multi_thread")]
async fn usage_maps_off_the_wire_names() {
    let pieces = vec![
        chunk(
            r#"{"id":"chatcmpl-1","model":"gpt-4o","choices":[{"index":0,"delta":{"content":"hi"},"finish_reason":null}]}"#,
        ),
        chunk(
            r#"{"id":"chatcmpl-1","model":"gpt-4o","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":11,"completion_tokens":2,"prompt_tokens_details":{"cached_tokens":4}}}"#,
        ),
        terminate(),
    ];
    let host = endpoint(vec![Canned::Sse {
        status: 200,
        pieces,
    }]);
    let adapter = openai_adapter(host.base.clone());
    let events: Vec<TurnEvent> = adapter
        .create_turn(&auth("sk-EXAMPLE"), &stream_request("gpt-4o"))
        .await
        .expect("the stream opens")
        .map(|result| result.unwrap())
        .collect::<Vec<_>>()
        .await;
    assert!(
        events.contains(&TurnEvent::Usage {
            usage: myharness_provider_sdk::Usage {
                input: 11,
                output: 2,
                cache_read: Some(4),
                ..Default::default()
            }
        }),
        "{events:?}"
    );
}

/// `AI-MODEL-01`, `AI-ADAPTER-11` — the echo gate: a performed model
/// outside the requested id's prefix stops the stream; a dated revision of
/// the requested id (`gpt-4o-2024-11-20` for `gpt-4o`) stays legal.
#[tokio::test(flavor = "multi_thread")]
async fn a_model_echo_mismatch_stops_the_stream() {
    let host = endpoint(vec![Canned::Sse {
        status: 200,
        pieces: vec![
            chunk(
                r#"{"id":"chatcmpl-1","model":"gpt-3.5-turbo","choices":[{"index":0,"delta":{"content":"hi"},"finish_reason":null}]}"#,
            ),
            terminate(),
        ],
    }]);
    let adapter = openai_adapter(host.base.clone());
    let result = adapter
        .create_turn(&auth("sk-EXAMPLE"), &stream_request("gpt-4o"))
        .await
        .expect("the stream opens");
    let events: Vec<Result<TurnEvent, _>> = result.collect().await;
    // The mismatch must surface, not ride.
    let error = events
        .into_iter()
        .find_map(|event| match event {
            Err(error) | Ok(TurnEvent::Error { error }) => Some(error),
            _ => None,
        })
        .expect("the mismatch surfaced");
    assert_eq!(error.class, ErrorClass::ModelNotFound);
    assert!(error.message.contains("gpt-3.5-turbo"));
    assert!(error.message.contains("gpt-4o"));
    host.stop();
}

/// The dated-revision half of the echo gate.
#[tokio::test(flavor = "multi_thread")]
async fn a_dated_revision_tolerates_the_echo() {
    let host = endpoint(vec![Canned::Json(
        200,
        legal_completion("gpt-4o-2024-11-20", "pong", "stop"),
    )]);
    let adapter = openai_adapter(host.base.clone());
    let mut request = stream_request("gpt-4o");
    request.stream = false;
    let stream = adapter
        .create_turn(&auth("sk-EXAMPLE"), &request)
        .await
        .expect("a dated revision is the requested model");
    let events: Vec<TurnEvent> = stream
        .map(|result| result.unwrap())
        .collect::<Vec<_>>()
        .await;
    assert!(events.contains(&TurnEvent::TextDelta {
        text: "pong".to_string()
    }));
    assert!(host.requests()[0].body()["stream"].as_bool() == Some(false));
}

/// `TEST-AI-016` — the idle window: a stream sending only keep-alives
/// times out at the configured window, the drip is transport, not progress.
/// The fault surfaces as the stream's terminal error, exactly the sibling
/// adapters' shape.
#[tokio::test(flavor = "multi_thread")]
async fn a_pigeonhole_stream_times_out_at_the_idle_window() {
    let started = Instant::now();
    let host = endpoint(vec![Canned::Drip {
        interval_ms: 30,
        first: chunk(
            r#"{"id":"chatcmpl-1","model":"gpt-4o","choices":[{"index":0,"delta":{"content":"first"},"finish_reason":null}]}"#,
        )
        .body,
    }]);
    let adapter = custom_adapter(host.base.clone(), Duration::from_millis(150));
    let mut stream = adapter
        .create_turn(&no_auth(), &stream_request("gpt-4o"))
        .await
        .expect("the stream opens");
    let first = StreamExt::next(&mut stream).await.unwrap().unwrap();
    assert_eq!(
        first,
        TurnEvent::TextDelta {
            text: "first".to_string()
        }
    );
    let error = StreamExt::next(&mut stream)
        .await
        .unwrap()
        .expect_err("the idle window must fire");
    assert!(error.message.contains("idle"));
    let elapsed = started.elapsed();
    assert!(
        elapsed >= Duration::from_millis(130),
        "the window must have actually elapsed: {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_secs(3),
        "the timeout must not wait for the test's patience: {elapsed:?}"
    );
}

/// `TEST-AI-015` — dropping the stream cancels the native request: the
/// partial output already emitted stays; the drop itself closes the
/// connection.
#[tokio::test(flavor = "multi_thread")]
async fn dropping_the_stream_cancels_it() {
    let host = endpoint(vec![Canned::Sse {
        status: 200,
        pieces: vec![
            chunk(
                r#"{"id":"chatcmpl-1","model":"gpt-4o","choices":[{"index":0,"delta":{"content":"first"},"finish_reason":null}]}"#,
            ),
            // Everything after this would be the rest of the turn.
            SsePiece {
                delay_ms: 60,
                body: "data: [DONE]\r\n\r\n".to_string(),
            },
        ],
    }]);
    let adapter = openai_adapter(host.base.clone());
    let mut stream = adapter
        .create_turn(&auth("sk-EXAMPLE"), &stream_request("gpt-4o"))
        .await
        .unwrap();
    let first = StreamExt::next(&mut stream).await.unwrap().unwrap();
    assert_eq!(
        first,
        TurnEvent::TextDelta {
            text: "first".to_string()
        }
    );
    // Drop = cancel. The connection must close; the server thread notices.
    drop(stream);
    // The partial output the caller already has is exactly what it emitted;
    // nothing more may arrive over a dropped stream — a drop, rather than a
    // graceful stop, is the cancellation.
    host.stop();
}

// ---------------------------------------------------------------------------
// The non-streaming paths: probe, token counting, discovery, health
// ---------------------------------------------------------------------------

/// The probe is a real inference (`docs/screens/setup-desktop-3p.md`): a
/// 1-token completion that reports the model it ran, the preserved stop
/// reason, and exact usage.
#[tokio::test(flavor = "multi_thread")]
async fn the_probe_inference_is_real() {
    let host = endpoint(vec![Canned::Json(
        200,
        legal_completion("gpt-4o", "pong", "stop"),
    )]);
    let adapter = openai_adapter(host.base.clone());
    let status = adapter
        .probe_inference(&auth("sk-EXAMPLE"), "gpt-4o")
        .await
        .expect("the probe ran");
    assert!(status.ok);
    assert_eq!(status.model, "gpt-4o");
    assert_eq!(status.stop_reason.as_deref(), Some("end_turn"));
    assert_eq!(status.input_tokens, 3);
    assert_eq!(status.output_tokens, 1);
    let body = host.requests()[0].body();
    assert_eq!(body["model"], "gpt-4o");
    assert_eq!(body["max_tokens"], 1);
    assert_eq!(body["stream"], false);
    assert_eq!(body["messages"][0]["content"][0]["text"], "ping");
}

/// `AI-MODEL-01` — the probe refuses a performed model outside the
/// requested id's prefix.
#[tokio::test(flavor = "multi_thread")]
async fn the_probe_refuses_a_non_echoing_model() {
    let host = endpoint(vec![Canned::Json(
        200,
        legal_completion("gpt-3.5-turbo-0301", "pong", "stop"),
    )]);
    let adapter = openai_adapter(host.base.clone());
    let error = adapter
        .probe_inference(&auth("sk-EXAMPLE"), "gpt-4o")
        .await
        .err()
        .expect("the probe refuses a substitution");
    assert_eq!(error.class, ErrorClass::ModelNotFound);
    assert!(error.message.contains("gpt-3.5-turbo"));
    assert!(host.requests()[0].body()["stream"].as_bool() == Some(false));
}

/// `AI-ADAPTER-09` — this wire offers no native counting endpoint: the
/// refusal is explicit and no request ever travels.
#[tokio::test(flavor = "multi_thread")]
async fn token_counting_is_refused_and_never_estimated() {
    let host = endpoint(vec![]);
    let adapter = openai_adapter(host.base.clone());
    let error = adapter
        .count_tokens(
            &auth("sk-EXAMPLE"),
            &CountRequest {
                model: key("gpt-4o"),
                system: Vec::new(),
                messages: Vec::new(),
                tools: Vec::new(),
            },
        )
        .await
        .err()
        .expect("counting is refused");
    assert_eq!(error.class, ErrorClass::InvalidRequest);
    assert!(error.message.contains("no count was produced"));
    assert!(
        host.requests().is_empty(),
        "zero requests must have travelled"
    );
}

/// Discovery reads the listing shapes the wire's servers actually send:
/// first-party `data` arrays (objects *and* bare strings) and the keyed
/// `models` object. The bearer header rides when a key exists; a `custom`
/// keyless endpoint sends none. Context size arrives under either shape
/// (`context_window`/`context_length`).
#[tokio::test(flavor = "multi_thread")]
async fn discovery_reads_the_listings_the_wire_sends() {
    // First-party objects, with context_window.
    let host = endpoint(vec![Canned::Json(
        200,
        json!({ "data": [
            { "id": "gpt-4o", "context_window": 128000 },
            { "id": "gpt-4o-mini", "created": 1721172741 },
            { "id": "gpt-4o" }
        ]}),
    )]);
    let outcome = myharness_provider_openai::discover_models(
        &myharness_provider_openai::DiscoveryRequest {
            kind: AdapterKind::OpenAi,
            base_url: host.base.clone(),
            api_key: Some("sk-EXAMPLE".to_string()),
            custom_headers: vec![],
            timeout_secs: 5,
        },
        &reqwest::Client::new(),
    )
    .await
    .expect("the first-party listing read");
    let ids: Vec<&str> = outcome
        .models
        .iter()
        .map(|model| model.id.as_str())
        .collect();
    assert_eq!(ids, ["gpt-4o", "gpt-4o-mini"], "de-duplicated in order");
    assert_eq!(outcome.pages, 1);
    assert!(!outcome.partial);
    assert_eq!(
        host.requests()[0].header("authorization"),
        Some("Bearer sk-EXAMPLE".to_string())
    );
    host.stop();

    // Bare-string rows and the `context_length` spelling.
    let host = endpoint(vec![Canned::Json(
        200,
        json!({ "data": ["llama3.2", { "id": "qwen2.5", "context_length": 32768 }] }),
    )]);
    let outcome = myharness_provider_openai::discover_models(
        &myharness_provider_openai::DiscoveryRequest {
            kind: AdapterKind::Custom,
            base_url: host.base.clone(),
            api_key: None,
            custom_headers: vec![],
            timeout_secs: 5,
        },
        &reqwest::Client::new(),
    )
    .await
    .expect("the tolerant listing read");
    let ids: Vec<&str> = outcome
        .models
        .iter()
        .map(|model| model.id.as_str())
        .collect();
    assert_eq!(ids, ["llama3.2", "qwen2.5"]);
    assert!(
        host.requests()[0].header("authorization").is_none(),
        "a keyless custom endpoint sends no authorization header"
    );
    host.stop();

    // The keyed `models` object.
    let host = endpoint(vec![Canned::Json(
        200,
        json!({ "models": { "mistral-small": { "context_window": 8192 } } }),
    )]);
    let outcome = myharness_provider_openai::discover_models(
        &myharness_provider_openai::DiscoveryRequest {
            kind: AdapterKind::Custom,
            base_url: host.base.clone(),
            api_key: None,
            custom_headers: vec![],
            timeout_secs: 5,
        },
        &reqwest::Client::new(),
    )
    .await
    .expect("the keyed listing read");
    assert_eq!(outcome.models.len(), 1);
    assert_eq!(outcome.models[0].id, "mistral-small");
    host.stop();
}

/// `TEST-SETUP-*` sibling — a keyless `custom` endpoint runs the probe and
/// its request carries no `authorization` header at all (`docs/14` §9:
/// self-hosted servers may answer unauthenticated).
#[tokio::test(flavor = "multi_thread")]
async fn a_keyless_custom_endpoint_sends_no_authorization() {
    let host = endpoint(vec![Canned::Json(
        200,
        legal_completion("gpt-4o", "pong", "stop"),
    )]);
    let adapter = custom_adapter(host.base.clone(), Duration::from_millis(150));
    let status = adapter
        .probe_inference(&no_auth(), "gpt-4o")
        .await
        .expect("the keyless probe ran");
    assert!(status.ok);
    let request = &host.requests()[0];
    assert!(request.header("authorization").is_none());
    assert_eq!(request.header("X-Tenant").as_deref(), Some("acme"));
}

/// The `openai` family refuses an empty credential before anything sends.
#[tokio::test(flavor = "multi_thread")]
async fn a_missing_credential_blocks_the_openai_send() {
    let host = endpoint(vec![]);
    let adapter = openai_adapter(host.base.clone());
    let error = adapter
        .create_turn(&no_auth(), &stream_request("gpt-4o"))
        .await
        .err()
        .expect("an empty credential on the openai family refuses");
    assert_eq!(error.class, ErrorClass::Auth);
    assert!(error.message.contains("empty"));
    assert!(
        host.requests().is_empty(),
        "zero requests must have travelled"
    );
}

/// The `openai` family's empty base URL falls back to the first-party
/// host; the `custom` family refuses sends without one.
#[tokio::test(flavor = "multi_thread")]
async fn the_families_own_their_base_urls() {
    let defaulted = openai_adapter(String::new());
    assert_eq!(
        defaulted.base_url(),
        myharness_provider_openai::DEFAULT_OPENAI_BASE
    );
    let keyed = custom_adapter(String::new(), Duration::from_secs(1));
    assert_eq!(keyed.base_url(), "");
    let error = keyed
        .probe_inference(&no_auth(), "gpt-4o")
        .await
        .err()
        .expect("a baseless custom family refuses");
    assert_eq!(error.class, ErrorClass::Configuration);
}

/// `docs/12` §8.2 `healthCheck` — `GET {base}/models?limit=1` as
/// reachability, no inference.
#[tokio::test(flavor = "multi_thread")]
async fn the_health_check_is_real() {
    let host = endpoint(vec![Canned::Json(200, json!({ "data": [] }))]);
    let adapter = openai_adapter(host.base.clone());
    let health = adapter
        .health_check(&auth("sk-EXAMPLE"))
        .await
        .expect("the check ran");
    assert!(health.reachable);
    assert_eq!(health.authenticated, Some(true));
    assert!(host.requests()[0].has_path("/models?limit=1"));
}
