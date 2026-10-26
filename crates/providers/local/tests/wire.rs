//! Mock-endpoint wire fixtures — the conformance battery for the local
//! wall (Phase 8's second reference extension, `docs/13` §"Phase 8"), plus
//! the recorded-reading assertions the crate header pins (`docs/14` §9).
//!
//! The mock endpoint shares the sibling batteries' *design* — a
//! non-blocking listener thread with the stop flag re-checked between
//! connections, requests read before answering, complete chunked bodies
//! (the terminating chunk), and no waiting for the client's close
//! (`docs/14` §9 item 72's harness traps). Fixture policy
//! (`tests/README.md`): every credential here is a synthetic placeholder,
//! every model id is a benign spelling, nothing is recorded from a real
//! session.

#![allow(clippy::err_expect)]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use futures::StreamExt;
use myharness_provider_local::{
    Destination, LocalAdapter, LocalClientConfig, DEFAULT_IDLE_TIMEOUT,
};
use myharness_provider_sdk::ProviderAdapter;
use myharness_provider_sdk::{
    AuthContext, ContentPart, CountRequest, ErrorClass, ModelKey, NativeTurnRequest, Role,
    SystemBlock, ToolSpec, TurnEvent,
};
use secrecy::SecretString;
use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// The mock local endpoint
// ---------------------------------------------------------------------------

fn sse(payload: Value) -> String {
    format!("event: message\ndata: {payload}\n\n")
}

/// A canned endpoint behavior for a single connection.
enum Canned {
    /// A complete JSON response.
    Json(u16, Value),
    /// A chunked SSE stream: the pieces are written in order (each inside
    /// its own chunked-encoding frame), then the terminating chunk.
    Sse { status: u16, pieces: Vec<String> },
    /// A redirect to another origin — the wall must refuse it, not follow.
    Redirect(String),
}

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

struct MockEndpoint {
    base: String,
    requests: std::sync::Arc<std::sync::Mutex<Vec<Recorded>>>,
    stop_flag: std::sync::Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl MockEndpoint {
    fn requests(&self) -> Vec<Recorded> {
        self.requests.lock().unwrap().clone()
    }

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

/// Read one request off `socket`, forced non-blocking and bounded: until the
/// content-length is satisfied, the head is complete and the client pauses,
/// or `stop` flips. A server must read before answering — a response head
/// arriving before the client's request body finished is a hyper send
/// error — and must never wait for the client's pooled close.
fn read_request(socket: &mut std::net::TcpStream, stop: &AtomicBool) -> String {
    let _ = socket.set_nonblocking(true);
    let mut raw = String::new();
    let mut buffer = [0u8; 4096];
    loop {
        match socket.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => raw.push_str(&String::from_utf8_lossy(&buffer[..read])),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if raw.contains("\r\n\r\n") || stop.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(1));
                continue;
            }
            Err(_) => break,
        }
        if raw.contains("\r\n\r\n") {
            let head_end = raw.find("\r\n\r\n").unwrap() + 4;
            let body = &raw[head_end..];
            if let Some(length) = raw.to_lowercase().lines().find_map(|line| {
                line.strip_prefix("content-length:")
                    .and_then(|value| value.trim().parse::<usize>().ok())
            }) {
                if body.len() >= length {
                    break;
                }
            }
        }
    }
    raw
}

fn serve(canned_for: impl Fn(usize, &Value) -> Canned + Send + 'static) -> MockEndpoint {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a local port");
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    let requests = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let stop_flag = std::sync::Arc::new(AtomicBool::new(false));
    let requests_thread = std::sync::Arc::clone(&requests);
    let stop_thread = std::sync::Arc::clone(&stop_flag);
    let handle = std::thread::spawn(move || {
        let _ = listener.set_nonblocking(true);
        let mut index = 0usize;
        while !stop_thread.load(std::sync::atomic::Ordering::Relaxed) {
            let Ok(mut socket) = listener.accept().map(|(socket, _)| socket) else {
                std::thread::sleep(Duration::from_millis(3));
                continue;
            };
            let current = index;
            index += 1;
            let raw = read_request(&mut socket, &stop_thread);
            let body: Value = raw
                .split_once("\r\n\r\n")
                .and_then(|(_, body)| serde_json::from_str(body).ok())
                .unwrap_or(Value::Null);
            requests_thread.lock().unwrap().push(Recorded { raw });
            let canned = canned_for(current, &body);
            let mut out = Vec::new();
            match canned {
                Canned::Json(status, body) => {
                    let payload = body.to_string();
                    out.push(format!(
                        "HTTP/1.1 {status} OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{payload}",
                        payload.len()
                    ));
                }
                Canned::Sse { status, pieces } => {
                    out.push(format!(
                        "HTTP/1.1 {status} OK\r\ncontent-type: text/event-stream\r\ntransfer-encoding: chunked\r\n\r\n"
                    ));
                    for piece in pieces {
                        let chunk = piece.as_bytes();
                        out.push(format!("{:x}\r\n", chunk.len()));
                        out.push(piece);
                        out.push("\r\n".to_string());
                    }
                    // The terminating chunk: without it hyper reads the
                    // close as a truncated chunked body.
                    out.push("0\r\n\r\n".to_string());
                }
                Canned::Redirect(location) => {
                    out.push(format!(
                        "HTTP/1.1 302 Found\r\nlocation: {location}\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
                    ));
                }
            }
            let _ = socket.write_all(out.join("").as_bytes());
            let _ = socket.flush();
        }
    });
    MockEndpoint {
        base,
        requests,
        stop_flag,
        handle: Some(handle),
    }
}

// ---------------------------------------------------------------------------
// The harness
// ---------------------------------------------------------------------------

const KEY: &str = "sk-test-local-000000000000";

fn adapter(base: &str) -> LocalAdapter {
    LocalAdapter::new(LocalClientConfig {
        base_url: base.to_string(),
        idle_timeout: DEFAULT_IDLE_TIMEOUT,
        custom_headers: Vec::new(),
    })
    .expect("a loopback literal pins")
}

fn keyless(base: &str) -> LocalAdapter {
    LocalAdapter::new(LocalClientConfig {
        base_url: base.to_string(),
        idle_timeout: DEFAULT_IDLE_TIMEOUT,
        custom_headers: Vec::new(),
    })
    .expect("a loopback literal pins")
}

fn auth() -> AuthContext {
    AuthContext::new(SecretString::from(KEY), "test static key")
}

fn empty_auth() -> AuthContext {
    AuthContext::new(SecretString::from("   "), "keyless")
}

fn model(id: &str) -> ModelKey {
    ModelKey {
        provider_id: "local".to_string(),
        model_id: id.to_string(),
        variant: None,
        revision: None,
    }
}

fn text_request(id: &str, text: &str, stream: bool) -> NativeTurnRequest {
    NativeTurnRequest {
        model: model(id),
        system: vec![SystemBlock {
            text: "be terse".to_string(),
            cache_control: None,
        }],
        messages: vec![(
            Role::User,
            vec![ContentPart::Text {
                text: text.to_string(),
            }],
        )],
        tools: Vec::new(),
        max_tokens: 128,
        temperature: None,
        top_p: None,
        top_k: None,
        stop_sequences: Vec::new(),
        thinking: None,
        betas: Vec::new(),
        stream,
    }
}

fn completion(model: &str, text: &str, finish: &str) -> Value {
    json!({
        "id": "chatcmpl-local-1",
        "object": "chat.completion",
        "model": model,
        "choices": [ { "index": 0,
            "message": { "role": "assistant", "content": text },
            "finish_reason": finish } ],
        "usage": {
            "prompt_tokens": 9,
            "completion_tokens": 3,
            "prompt_tokens_details": { "cached_tokens": 4 }
        }
    })
}

async fn collect(stream: myharness_provider_sdk::TurnStream) -> Vec<TurnEvent> {
    stream
        .map(|event| event.expect("the battery stream carries no error"))
        .collect::<Vec<_>>()
        .await
}

// ---------------------------------------------------------------------------
// The battery
// ---------------------------------------------------------------------------

/// `AI-LOCAL-05`: an absent credential sends NO authorization header at all
/// — keyless is the family default, and the destination gate, not the
/// credential, decides where requests may go.
#[tokio::test]
async fn a_keyless_send_carries_no_authorization_header() {
    let endpoint = serve(|_, _| Canned::Json(200, completion("m-1", "pong", "stop")));
    let wall = keyless(&endpoint.base);
    let _ = wall
        .create_turn(&empty_auth(), &text_request("m-1", "hi", false))
        .await
        .expect("the keyless send");
    let recorded = &endpoint.requests()[0];
    assert!(recorded.has_path("/v1/chat/completions"));
    assert!(
        recorded.header("authorization").is_none(),
        "a keyless send carries no authorization header"
    );
    endpoint.stop();
}

/// A present credential travels as `Bearer` and nowhere else
/// (`AI-PROVIDER-03`).
#[tokio::test]
async fn a_credential_travels_as_bearer() {
    let endpoint = serve(|_, _| Canned::Json(200, completion("m-1", "pong", "stop")));
    let wall = adapter(&endpoint.base);
    let _ = wall
        .create_turn(&auth(), &text_request("m-1", "hi", false))
        .await
        .expect("the send");
    assert_eq!(
        endpoint.requests()[0].header("authorization"),
        Some(format!("Bearer {KEY}"))
    );
    endpoint.stop();
}

/// The recorded body shape: system joins into one system message,
/// `max_tokens` (not `max_completion_tokens`), no `stream_options`, the
/// dropped fields never appear, and tools nest under `function` — the
/// Chat Completions spelling, distinct from the Responses wall's flat tools.
#[tokio::test]
async fn the_request_body_spells_the_chat_completions_wire() {
    let endpoint = serve(|_, _| Canned::Json(200, completion("m-1", "ok", "stop")));
    let wall = adapter(&endpoint.base);
    let mut request = text_request("m-1", "see this", false);
    request.system = vec![
        SystemBlock {
            text: "first".to_string(),
            cache_control: Some("ephemeral".to_string()),
        },
        SystemBlock {
            text: "second".to_string(),
            cache_control: None,
        },
    ];
    request.temperature = Some(0.5);
    request.top_k = Some(7);
    request.thinking = Some(myharness_provider_sdk::ThinkingConfig { budget_tokens: 512 });
    request.betas = vec!["some-beta".to_string()];
    request.tools = vec![ToolSpec {
        name: "lookup".to_string(),
        description: Some("find things".to_string()),
        input_schema: json!({ "type": "object" }),
        cache_control: Some("ephemeral".to_string()),
    }];
    let _ = wall.create_turn(&auth(), &request).await.expect("the send");
    let body = endpoint.requests()[0].body();
    assert_eq!(body["model"], "m-1");
    assert_eq!(body["messages"][0]["role"], "system");
    assert_eq!(body["messages"][0]["content"], "first\n\nsecond");
    assert_eq!(body["max_tokens"], 128);
    assert_eq!(body["temperature"], 0.5);
    let tool = &body["tools"][0];
    assert_eq!(tool["type"], "function");
    assert_eq!(tool["function"]["name"], "lookup");
    assert_eq!(tool["function"]["description"], "find things");
    // No spelling, no key: the dropped fields never appear.
    assert!(body.get("top_k").is_none());
    assert!(body.get("reasoning").is_none());
    assert!(body.get("betas").is_none());
    assert!(body.get("stream_options").is_none());
    assert!(body.get("max_completion_tokens").is_none());
    endpoint.stop();
}

/// A completed non-streaming completion maps text, exact usage (cached
/// tokens included), and the `end_turn` stop (`AI-USAGE-01`,
/// `AI-ADAPTER-08`).
#[tokio::test]
async fn a_completed_completion_maps_text_usage_and_end_turn() {
    let endpoint = serve(|_, _| Canned::Json(200, completion("m-1", "hello", "stop")));
    let wall = adapter(&endpoint.base);
    let events = collect(
        wall.create_turn(&auth(), &text_request("m-1", "hi", false))
            .await
            .expect("the send"),
    )
    .await;
    assert_eq!(
        events[0],
        TurnEvent::TextDelta {
            text: "hello".to_string()
        }
    );
    assert!(events.iter().any(|event| matches!(
        event,
        TurnEvent::Usage { usage } if usage.input == 9 && usage.output == 3 && usage.cache_read == Some(4)
    )));
    assert!(events.iter().any(
        |event| matches!(event, TurnEvent::Stop { reason } if *reason == myharness_provider_sdk::NativeStopReason::EndTurn)
    ));
    assert!(matches!(events.last(), Some(TurnEvent::Done)));
    endpoint.stop();
}

/// A tool-call turn stops as `tool_use`; arguments parse or the turn
/// refuses (`AI-TOOL-01`).
#[tokio::test]
async fn a_tool_call_turn_stops_as_tool_use() {
    let endpoint = serve(|_, _| {
        Canned::Json(
            200,
            json!({
                "id": "chatcmpl-local-2", "object": "chat.completion", "model": "m-1",
                "choices": [ { "index": 0,
                    "message": { "role": "assistant", "content": null, "tool_calls": [
                        { "id": "call_1", "type": "function",
                          "function": { "name": "lookup", "arguments": "{\"q\":\"x\"}" } } ] },
                    "finish_reason": "tool_calls" } ],
                "usage": { "prompt_tokens": 5, "completion_tokens": 7 }
            }),
        )
    });
    let wall = adapter(&endpoint.base);
    let events = collect(
        wall.create_turn(&auth(), &text_request("m-1", "look up x", false))
            .await
            .expect("the send"),
    )
    .await;
    assert!(events.iter().any(|event| matches!(
        event,
        TurnEvent::ToolCall { id, name, arguments: Some(arguments), .. }
            if id == "call_1" && name == "lookup" && arguments == &json!({"q": "x"})
    )));
    assert!(events.iter().any(
        |event| matches!(event, TurnEvent::Stop { reason } if *reason == myharness_provider_sdk::NativeStopReason::ToolUse)
    ));
    endpoint.stop();
}

/// The streamed text lane: deltas map, usage lands from the chunk that
/// reports it, and the stream terminates at the `data: [DONE]` sentinel —
/// which is REQUIRED (`AI-LOCAL-06` grammar).
#[tokio::test]
async fn the_stream_maps_deltas_and_terminates_at_the_sentinel() {
    let endpoint = serve(|_, _| Canned::Sse {
        status: 200,
        pieces: vec![
            sse(
                json!({ "id": "c1", "model": "m-1", "choices": [ { "index": 0,
                    "delta": { "content": "Hel" }, "finish_reason": null } ] }),
            ),
            sse(
                json!({ "id": "c1", "model": "m-1", "choices": [ { "index": 0,
                    "delta": { "content": "lo" }, "finish_reason": null } ] }),
            ),
            sse(
                json!({ "id": "c1", "model": "m-1", "choices": [ { "index": 0,
                    "delta": {}, "finish_reason": "stop" } ],
                    "usage": { "prompt_tokens": 11, "completion_tokens": 2 } }),
            ),
            "data: [DONE]\n\n".to_string(),
        ],
    });
    let wall = adapter(&endpoint.base);
    let events = collect(
        wall.create_turn(&auth(), &text_request("m-1", "hi", true))
            .await
            .expect("the send"),
    )
    .await;
    let texts: Vec<&str> = events
        .iter()
        .filter_map(|event| match event {
            TurnEvent::TextDelta { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(texts, vec!["Hel", "lo"]);
    assert!(events.iter().any(|event| matches!(
        event,
        TurnEvent::Usage { usage } if usage.input == 11 && usage.output == 2
    )));
    assert!(events.iter().any(
        |event| matches!(event, TurnEvent::Stop { reason } if *reason == myharness_provider_sdk::NativeStopReason::EndTurn)
    ));
    assert!(matches!(events.last(), Some(TurnEvent::Done)));
    endpoint.stop();
}

/// Streamed tool fragments assemble with the recorded synthetic ids: a
/// fragment carrying its own id uses it; id-less fragments synthesize
/// `{completion-id}-{index}` — `call-{index}` when the chunk carries no
/// completion id — and the close happens at `finish_reason` (`AI-TOOL-01`).
#[tokio::test]
async fn the_stream_assembles_tool_calls_with_the_recorded_synthetic_ids() {
    let endpoint = serve(|_, _| Canned::Sse {
        status: 200,
        pieces: vec![
            sse(
                json!({ "id": "chatcmpl-x", "model": "m-1", "choices": [ { "index": 0,
                    "delta": { "tool_calls": [ { "index": 0, "id": "call_real", "type": "function",
                        "function": { "name": "lookup", "arguments": "{\"q\":" } } ] },
                    "finish_reason": null } ] }),
            ),
            sse(
                json!({ "id": "chatcmpl-x", "model": "m-1", "choices": [ { "index": 0,
                    "delta": { "tool_calls": [ { "index": 0,
                        "function": { "arguments": "\"x\"}" } } ] },
                    "finish_reason": null } ] }),
            ),
            sse(
                json!({ "id": "chatcmpl-x", "model": "m-1", "choices": [ { "index": 0,
                    "delta": {}, "finish_reason": "tool_calls" } ] }),
            ),
            "data: [DONE]\n\n".to_string(),
        ],
    });
    let wall = adapter(&endpoint.base);
    let events = collect(
        wall.create_turn(&auth(), &text_request("m-1", "look up x", true))
            .await
            .expect("the send"),
    )
    .await;
    let fragments = events
        .iter()
        .filter(|event| {
            matches!(
                event,
                TurnEvent::ToolCall {
                    arguments_delta: Some(_),
                    ..
                }
            )
        })
        .count();
    assert_eq!(fragments, 2);
    assert!(events.iter().any(|event| matches!(
        event,
        TurnEvent::ToolCall { id, name, arguments_delta: None, arguments: Some(arguments) }
            if id == "call_real" && name == "lookup" && arguments == &json!({"q": "x"})
    )));
    assert!(events.iter().any(
        |event| matches!(event, TurnEvent::Stop { reason } if *reason == myharness_provider_sdk::NativeStopReason::ToolUse)
    ));
    endpoint.stop();
}

/// An id-less tool fragment on a chunk with no completion id synthesizes
/// `call-{index}`.
#[tokio::test]
async fn idless_fragments_synthesize_call_index() {
    let endpoint = serve(|_, _| Canned::Sse {
        status: 200,
        pieces: vec![
            sse(json!({ "model": "m-1", "choices": [ { "index": 0,
                    "delta": { "tool_calls": [ { "index": 0, "type": "function",
                        "function": { "name": "lookup", "arguments": "{\"q\":\"x\"}" } } ] },
                    "finish_reason": null } ] })),
            sse(json!({ "model": "m-1", "choices": [ { "index": 0,
                    "delta": {}, "finish_reason": "tool_calls" } ] })),
            "data: [DONE]\n\n".to_string(),
        ],
    });
    let wall = adapter(&endpoint.base);
    let events = collect(
        wall.create_turn(&auth(), &text_request("m-1", "hi", true))
            .await
            .expect("the send"),
    )
    .await;
    assert!(events.iter().any(|event| matches!(
        event,
        TurnEvent::ToolCall { id, arguments: Some(_), .. } if id == "call-0"
    )));
    endpoint.stop();
}

/// `data: {}` keep-alive pings are progress bytes for the transport and
/// never reset the idle window (`TEST-AI-016`); the window still fires on
/// an idle stream that keeps the connection alive.
#[tokio::test]
async fn the_idle_window_fires_without_progress() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a local port");
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    std::thread::spawn(move || {
        if let Some(Ok(mut socket)) = listener.incoming().next() {
            let stop = AtomicBool::new(false);
            let _raw = read_request(&mut socket, &stop);
            let _ = socket.write_all(
                b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ntransfer-encoding: chunked\r\n\r\n",
            );
            let ping = "data: {}\n\n";
            let _ = socket.write_all(format!("{:x}\r\n{ping}\r\n", ping.len()).as_bytes());
            loop {
                std::thread::sleep(Duration::from_millis(100));
                let _ = socket.write_all(format!("{:x}\r\n{ping}\r\n", ping.len()).as_bytes());
                let _ = socket.flush();
            }
        }
    });
    let wall = LocalAdapter::new(LocalClientConfig {
        base_url: base,
        idle_timeout: Duration::from_secs(1),
        custom_headers: Vec::new(),
    })
    .expect("a loopback literal pins");
    let started = std::time::Instant::now();
    let mut stream = wall
        .create_turn(&auth(), &text_request("m-1", "hi", true))
        .await
        .expect("the send");
    let mut last = None;
    while let Some(event) = stream.next().await {
        last = Some(event);
    }
    let error = last
        .expect("the stream carries the refusal")
        .expect_err("the idle window fires");
    assert!(error.message.contains("went idle"));
    assert!(started.elapsed() < Duration::from_secs(6));
}

/// A stream that ends without the `data: [DONE]` sentinel is
/// contract-malformed: no stop reason is invented (`AI-LOCAL-06` grammar).
#[tokio::test]
async fn a_stream_without_the_sentinel_is_malformed() {
    let endpoint = serve(|_, _| Canned::Sse {
        status: 200,
        pieces: vec![sse(
            json!({ "id": "c1", "model": "m-1", "choices": [ { "index": 0,
                "delta": { "content": "half" }, "finish_reason": null } ] }),
        )],
    });
    let wall = adapter(&endpoint.base);
    let mut stream = wall
        .create_turn(&auth(), &text_request("m-1", "hi", true))
        .await
        .expect("the send");
    let mut last = None;
    while let Some(event) = stream.next().await {
        last = Some(event);
    }
    let error = last
        .expect("the stream carries the refusal")
        .expect_err("no sentinel");
    assert!(error.message.contains("sentinel"));
    endpoint.stop();
}

/// `AI-LOCAL-06`: discovery reads `/api/tags` first — Ollama's keyed
/// `models` array with `name` ids.
#[tokio::test]
async fn discovery_reads_the_api_tags_listing_first() {
    let endpoint = serve(|_, request| {
        if request.get("nonempty") == Some(&Value::Null) && false {
            unreachable!()
        }
        Canned::Json(
            200,
            json!({ "models": [
                { "name": "qwen3:8b", "model": "qwen3:8b", "size": 1 },
                { "name": "llama3.2:3b" },
            ]}),
        )
    });
    let wall = adapter(&endpoint.base);
    let models = wall.list_models(&auth()).await.expect("the listing");
    assert_eq!(models.len(), 2);
    assert_eq!(models[0].key.model_id, "qwen3:8b");
    endpoint.stop();
}

/// Discovery falls back to `/v1/models` when `/api/tags` answers 404 — the
/// llama.cpp posture is not an error (`AI-LOCAL-06`).
#[tokio::test]
async fn discovery_falls_back_to_the_openai_listing_on_404() {
    let endpoint = serve(|index, _| {
        if index == 0 {
            Canned::Json(
                404,
                json!({ "error": { "message": "no such endpoint", "code": "not_found" } }),
            )
        } else {
            Canned::Json(
                200,
                json!({ "data": [ { "id": "gpt-4o-local" }, { "id": "m-2" } ] }),
            )
        }
    });
    let wall = adapter(&endpoint.base);
    let models = wall.list_models(&auth()).await.expect("the listing");
    assert_eq!(models.len(), 2);
    assert_eq!(models[0].key.model_id, "gpt-4o-local");
    endpoint.stop();
}

/// `AI-LOCAL-01`: a base URL outside loopback refuses at construction,
/// before any bytes — the local wall connects to loopback only.
#[tokio::test]
async fn a_non_loopback_base_refuses_before_any_contact() {
    let endpoint = serve(|_, _| Canned::Json(200, completion("m-1", "pong", "stop")));
    let error = LocalAdapter::new(LocalClientConfig {
        base_url: "http://192.168.1.10:11434".to_string(),
        idle_timeout: DEFAULT_IDLE_TIMEOUT,
        custom_headers: Vec::new(),
    })
    .expect_err("a non-loopback host must refuse");
    assert!(error.message.contains("loopback only"), "{error:?}");
    assert!(endpoint.requests().is_empty(), "nothing was sent");
    endpoint.stop();
}

/// `AI-LOCAL-02`: `localhost` resolving to a non-loopback address (DNS/IP
/// rebinding) refuses before any bytes — via the injectable resolver.
#[tokio::test]
async fn a_rebinding_localhost_refuses_before_any_contact() {
    let endpoint = serve(|_, _| Canned::Json(200, completion("m-1", "pong", "stop")));
    let rebinding: myharness_provider_local::destination::ResolveFn = std::sync::Arc::new(|_| {
        Ok(vec![
            std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
            std::net::IpAddr::V4(std::net::Ipv4Addr::new(203, 0, 113, 7)),
        ])
    });
    let error = LocalAdapter::with_resolver(
        LocalClientConfig {
            base_url: "http://localhost:11434".to_string(),
            idle_timeout: DEFAULT_IDLE_TIMEOUT,
            custom_headers: Vec::new(),
        },
        rebinding,
    )
    .expect_err("a rebinding localhost must refuse");
    assert!(error.message.contains("rebinding"), "{error:?}");
    assert!(endpoint.requests().is_empty(), "nothing was sent");
    endpoint.stop();
}

/// `AI-LOCAL-04`: a redirect to any other origin is an error, never a
/// follow — the wall's own client disables redirects.
#[tokio::test]
async fn a_redirect_is_refused_not_followed() {
    let endpoint = serve(|_, _| Canned::Redirect("http://127.0.0.1:9/moved".to_string()));
    let wall = adapter(&endpoint.base);
    let error = wall
        .create_turn(&auth(), &text_request("m-1", "hi", false))
        .await
        .err()
        .expect("a redirect must not be followed");
    // The wall surfaced the 302 itself (its client disables redirects) —
    // the redirected origin was never contacted.
    assert!(error.message.contains("302"), "{error:?}");
    assert_eq!(endpoint.requests().len(), 1);
    endpoint.stop();
}

/// The model echo gate: a dated revision stays legal, a different model is
/// a user-visible refusal (`AI-MODEL-01`).
#[tokio::test]
async fn the_model_echo_gate_refuses_a_substitution() {
    let endpoint = serve(|_, _| Canned::Json(200, completion("m-1-2026-01-01", "hi", "stop")));
    let legal = adapter(&endpoint.base);
    let _ = legal
        .create_turn(&auth(), &text_request("m-1", "hi", false))
        .await
        .expect("a dated revision of the requested id stays legal");
    endpoint.stop();

    let endpoint = serve(|_, _| Canned::Json(200, completion("other-model", "hi", "stop")));
    let substituting = adapter(&endpoint.base);
    let error = substituting
        .create_turn(&auth(), &text_request("m-1", "hi", false))
        .await
        .err()
        .expect("a substitution must refuse");
    assert_eq!(error.class, ErrorClass::ModelNotFound);
    endpoint.stop();
}

/// Media inside a tool result is an explicit refusal, upstream of any
/// bytes (`AI-MM-02`); documents are refused on user turns (`AI-MM-01`) —
/// no compat grammar was pinned for them.
#[tokio::test]
async fn media_is_refused_upstream() {
    let endpoint = serve(|_, _| Canned::Json(200, completion("m-1", "ok", "stop")));
    let wall = adapter(&endpoint.base);
    let request = NativeTurnRequest {
        model: model("m-1"),
        system: Vec::new(),
        messages: vec![(
            Role::User,
            vec![ContentPart::ToolResult {
                tool_use_id: "call_1".to_string(),
                content: vec![ContentPart::Image {
                    media_type: "image/png".to_string(),
                    data: "aGk=".to_string(),
                }],
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
        stream: false,
    };
    let error = wall
        .create_turn(&auth(), &request)
        .await
        .err()
        .expect("media in a tool result must refuse");
    assert_eq!(error.class, ErrorClass::UnsupportedModality);
    assert!(endpoint.requests().is_empty(), "nothing was sent");

    let error = wall
        .create_turn(
            &auth(),
            &NativeTurnRequest {
                messages: vec![(
                    Role::User,
                    vec![ContentPart::Document {
                        media_type: "application/pdf".to_string(),
                        data: "aGk=".to_string(),
                        name: Some("spec.pdf".to_string()),
                    }],
                )],
                ..text_request("m-1", "hi", false)
            },
        )
        .await
        .err()
        .expect("a document must refuse");
    assert_eq!(error.class, ErrorClass::UnsupportedModality);
    assert!(endpoint.requests().is_empty(), "nothing was sent");
    endpoint.stop();
}

/// Token counting is an explicit refusal — never an estimate
/// (`AI-ADAPTER-09`).
#[tokio::test]
async fn token_counting_refuses_instead_of_estimating() {
    let endpoint = serve(|_, _| Canned::Json(200, completion("m-1", "hi", "stop")));
    let wall = adapter(&endpoint.base);
    let error = wall
        .count_tokens(
            &auth(),
            &CountRequest {
                model: model("m-1"),
                system: Vec::new(),
                messages: Vec::new(),
                tools: Vec::new(),
            },
        )
        .await
        .expect_err("counting must refuse");
    assert!(error
        .message
        .contains("no count was produced and none was estimated"));
    assert!(endpoint.requests().is_empty(), "counting contacts nobody");
    endpoint.stop();
}

/// The pinned destination is what requests go to: a `/v1`-suffixed base
/// normalizes, and the paths stay the recorded ones.
#[tokio::test]
async fn the_pinned_destination_carries_the_recorded_paths() {
    let endpoint = serve(|_, _| Canned::Json(200, completion("m-1", "ok", "stop")));
    let wall = LocalAdapter::new(LocalClientConfig {
        base_url: format!("{}/v1/", endpoint.base),
        idle_timeout: DEFAULT_IDLE_TIMEOUT,
        custom_headers: Vec::new(),
    })
    .expect("a loopback literal pins");
    let _ = wall
        .create_turn(&auth(), &text_request("m-1", "hi", false))
        .await
        .expect("the send");
    let recorded = &endpoint.requests()[0];
    assert!(
        recorded.has_path("/v1/chat/completions"),
        "not double /v1: {}",
        recorded.raw
    );
    endpoint.stop();
}

/// Destination pinning is checkable directly: the recorded spelling.
#[test]
fn destination_pinning_is_recorded() {
    let any: myharness_provider_local::destination::ResolveFn = std::sync::Arc::new(|_: &str| {
        Ok(vec![std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)])
    });
    let pinned = Destination::resolve("http://localhost:11434", &any).expect("clean localhost");
    assert_eq!(pinned.origin(), "http://127.0.0.1:11434");
}
