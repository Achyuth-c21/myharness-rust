//! Mock-endpoint wire fixtures — the conformance battery for the Responses
//! adapter (Phase 8's reference extension, `docs/13` §"Phase 8"), plus the
//! recorded-reading assertions the crate header pins (`docs/14` §9).
//!
//! The mock endpoint shares the sibling batteries' *design* — a listener
//! thread with canned behaviors and a request log — and deliberately no
//! code. Fixture policy (`tests/README.md`): every credential here is a
//! synthetic placeholder, every model id is a benign spelling, nothing is
//! recorded from a real session.

// `create_turn`'s Ok lane is an opaque stream handle. `expect_err` would
// demand `Debug` on it, so stream-returning sites keep `.err().expect(..)`.
#![allow(clippy::err_expect)]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::Duration;

use futures::StreamExt;
use myharness_provider_openai_responses::{
    ResponsesAdapter, ResponsesClientConfig, DEFAULT_IDLE_TIMEOUT,
};
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
#[derive(Clone)]
struct SsePiece {
    delay_ms: u64,
    /// Raw payload (without the chunked framing; the writer adds it).
    body: String,
}

fn sse(event: &str, payload: Value) -> SsePiece {
    SsePiece {
        delay_ms: 0,
        body: format!("event: {event}\ndata: {payload}\n\n"),
    }
}

/// A canned endpoint behavior for a single connection.
enum Canned {
    /// A complete JSON response.
    Json(u16, Value),
    /// A chunked stream: the pieces are written in order (each inside its
    /// own chunked-encoding frame), and the connection stays open until the
    /// client drops it.
    Sse { status: u16, pieces: Vec<SsePiece> },
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

/// Read one request off `socket`, forced non-blocking and bounded: until
/// the content-length is satisfied, the head is complete and the client
/// pauses (`WouldBlock`), or `stop` flips. Returns the raw bytes read.
///
/// The non-blocking spin is what keeps the mock honest: a bodyless GET
/// never satisfies a content-length check, and a client that pools the
/// connection after its response never closes it while the test blocks in
/// `join` (the close is async work on the test's runtime) — a blocking
/// read would wait for bytes that never arrive.
fn read_request(socket: &mut std::net::TcpStream, stop: &std::sync::atomic::AtomicBool) -> String {
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
    let stop_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let requests_thread = std::sync::Arc::clone(&requests);
    let stop_thread = std::sync::Arc::clone(&stop_flag);
    let handle = std::thread::spawn(move || {
        // Non-blocking accept so the stop flag is re-checked between
        // connections: a blocking `incoming()` would leave `Drop`'s join()
        // waiting on an accept() that never returns once the tests stop
        // connecting (the zero-request cases). The accepted socket is forced
        // non-blocking explicitly — the client may keep the connection pooled
        // after its response (its close is async work on the test's runtime,
        // which is blocked in this join), so no read here may ever wait for
        // client bytes.
        let _ = listener.set_nonblocking(true);
        let mut index = 0usize;
        while !stop_thread.load(std::sync::atomic::Ordering::Relaxed) {
            let Ok(socket) = listener.accept().map(|(socket, _)| socket) else {
                std::thread::sleep(Duration::from_millis(3));
                continue;
            };
            let _ = socket.set_nonblocking(true);
            let mut socket = socket;
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
                    let head = format!(
                        "HTTP/1.1 {status} OK\r\ncontent-type: text/event-stream\r\ntransfer-encoding: chunked\r\n\r\n"
                    );
                    out.push(head);
                    for piece in pieces {
                        if piece.delay_ms > 0 {
                            std::thread::sleep(Duration::from_millis(piece.delay_ms));
                        }
                        let chunk = piece.body.as_bytes();
                        out.push(format!("{:x}\r\n", chunk.len()));
                        out.push(piece.body);
                        out.push("\r\n".to_string());
                    }
                    // The terminating chunk: without it hyper reads the
                    // close as a truncated chunked body and errors the
                    // stream ("error decoding response body") instead of
                    // ending it cleanly.
                    out.push("0\r\n\r\n".to_string());
                }
            }
            let _ = socket.write_all(out.join("").as_bytes());
            let _ = socket.flush();
            // No keep-alive tail: both canned shapes end their bodies
            // completely (content-length / the chunked terminator), and the
            // client may keep the connection pooled — waiting here for its
            // close would stall until reqwest's ~90 s idle eviction. The
            // socket drops at the end of the iteration, which is the
            // server-side close.
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

const KEY: &str = "sk-test-openai-responses-000000000000";

fn adapter(base: &str) -> ResponsesAdapter {
    ResponsesAdapter::new(
        ResponsesClientConfig {
            base_url: base.to_string(),
            idle_timeout: DEFAULT_IDLE_TIMEOUT,
            custom_headers: Vec::new(),
        },
        reqwest::Client::new(),
    )
}

fn auth() -> AuthContext {
    AuthContext::new(SecretString::from(KEY), "test static key")
}

fn model(id: &str) -> ModelKey {
    ModelKey {
        provider_id: "openai-responses".to_string(),
        model_id: id.to_string(),
        variant: None,
        revision: None,
    }
}

fn text_request(id: &str, text: &str) -> NativeTurnRequest {
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
        stream: false,
    }
}

/// A completed non-streaming response object.
fn completed_response(model: &str, status: &str, text: &str) -> Value {
    json!({
        "id": "resp_test_1",
        "object": "response",
        "model": model,
        "status": status,
        "output": [
            { "type": "message", "role": "assistant", "content": [
                { "type": "output_text", "text": text, "annotations": [] }
            ]}
        ],
        "usage": {
            "input_tokens": 9,
            "output_tokens": 3,
            "input_tokens_details": { "cached_tokens": 4 },
            "output_tokens_details": { "reasoning_tokens": 1 },
        },
        "incomplete_details": null,
    })
}

/// Drain a stream into events, failing the test on a provider error.
async fn collect(stream: myharness_provider_sdk::TurnStream) -> Vec<TurnEvent> {
    stream
        .map(|event| event.expect("the battery stream carries no error"))
        .collect::<Vec<_>>()
        .await
}

// ---------------------------------------------------------------------------
// The battery
// ---------------------------------------------------------------------------

/// An empty credential refuses before anything is sent (`TEST-EXT-005`
/// adjacency; `AI-ADAPTER-12`: no substituted credential, no request).
#[tokio::test]
async fn an_empty_credential_refuses_before_any_contact() {
    let endpoint = serve(|_, _| Canned::Json(200, completed_response("m-1", "completed", "hi")));
    let adapter = adapter(&endpoint.base);
    let error = adapter
        .authenticate(&AuthContext::new(SecretString::from("   "), "empty"))
        .await
        .expect_err("an empty credential must refuse");
    assert_eq!(error.class, ErrorClass::Auth);
    assert!(endpoint.requests().is_empty());
    endpoint.stop();
}

/// The recorded body shape: the Responses spellings (`instructions`,
/// `input` items, `max_output_tokens`, flat function tools, `store: false`)
/// and the documented drops (`top_k`, thinking, betas, `cache_control`).
#[tokio::test]
async fn the_request_body_spells_the_responses_wire_and_drops_recorded_fields() {
    let endpoint = serve(|_, _| Canned::Json(200, completed_response("m-1", "completed", "ok")));
    let adapter = adapter(&endpoint.base);
    let request = NativeTurnRequest {
        model: model("m-1"),
        system: vec![
            SystemBlock {
                text: "first".to_string(),
                cache_control: Some("ephemeral".to_string()),
            },
            SystemBlock {
                text: "second".to_string(),
                cache_control: None,
            },
        ],
        messages: vec![(
            Role::User,
            vec![
                ContentPart::Text {
                    text: "see this".to_string(),
                },
                ContentPart::Image {
                    media_type: "image/png".to_string(),
                    data: "aGk=".to_string(),
                },
                ContentPart::Document {
                    media_type: "application/pdf".to_string(),
                    data: "aGk=".to_string(),
                    name: Some("spec.pdf".to_string()),
                },
            ],
        )],
        tools: vec![ToolSpec {
            name: "lookup".to_string(),
            description: Some("find things".to_string()),
            input_schema: json!({ "type": "object" }),
            cache_control: Some("ephemeral".to_string()),
        }],
        max_tokens: 77,
        temperature: Some(0.5),
        top_p: None,
        top_k: Some(7),
        stop_sequences: vec!["STOP".to_string()],
        thinking: Some(myharness_provider_sdk::ThinkingConfig { budget_tokens: 512 }),
        betas: vec!["some-beta".to_string()],
        stream: false,
    };
    // The non-streaming send returns a buffered event stream this test does
    // not consume — the recorded request body is the assertion.
    let _ = adapter
        .create_turn(&auth(), &request)
        .await
        .expect("the send");
    let recorded = &endpoint.requests()[0];
    assert!(recorded.has_path("/responses"));
    assert_eq!(
        recorded.header("authorization"),
        Some(format!("Bearer {KEY}"))
    );
    let body = recorded.body();
    assert_eq!(body["model"], "m-1");
    assert_eq!(body["instructions"], "first\n\nsecond");
    assert_eq!(body["max_output_tokens"], 77);
    assert_eq!(body["store"], false);
    assert_eq!(body["stream"], false);
    assert_eq!(body["temperature"], 0.5);
    // Native content: text, image, and — this wire's own — the PDF.
    let content = body["input"][0]["content"].as_array().unwrap();
    assert_eq!(content[0]["type"], "input_text");
    assert_eq!(content[1]["image_url"], "data:image/png;base64,aGk=");
    assert_eq!(content[2]["type"], "input_file");
    assert_eq!(content[2]["filename"], "spec.pdf");
    // Flat tools: no nested `function` object.
    let tool = &body["tools"][0];
    assert_eq!(tool["type"], "function");
    assert_eq!(tool["name"], "lookup");
    assert_eq!(tool["description"], "find things");
    // No spelling, no key: the dropped fields never appear.
    assert!(body.get("top_k").is_none());
    assert!(body.get("reasoning").is_none());
    assert!(body.get("betas").is_none());
    assert!(body.get("stop").is_none());
    endpoint.stop();
}

/// A completed non-streaming response maps text, exact usage (cache read
/// included; reasoning tokens are a subset with no app channel), and the
/// `end_turn` stop (`AI-USAGE-01`, `AI-ADAPTER-08`).
#[tokio::test]
async fn a_completed_response_maps_text_usage_and_end_turn() {
    let endpoint = serve(|_, _| Canned::Json(200, completed_response("m-1", "completed", "hello")));
    let adapter = adapter(&endpoint.base);
    let stream = adapter
        .create_turn(&auth(), &text_request("m-1", "hi"))
        .await
        .expect("the send");
    let events = collect(stream).await;
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
    assert!(matches!(events.last(), Some(TurnEvent::Done)));
    assert!(events.iter().any(
        |event| matches!(event, TurnEvent::Stop { reason } if *reason == myharness_provider_sdk::NativeStopReason::EndTurn)
    ));
    endpoint.stop();
}

/// A turn that emitted function calls stops as `tool_use` — this wire's
/// terminal event carries no per-choice finish reason; the emitted items
/// are the equivalent fact.
#[tokio::test]
async fn a_turn_that_emitted_function_calls_stops_as_tool_use() {
    let endpoint = serve(|_, _| {
        Canned::Json(
            200,
            json!({
                "id": "resp_2", "model": "m-1", "status": "completed",
                "output": [
                    { "type": "function_call", "call_id": "call_1", "name": "lookup",
                      "arguments": "{\"q\":\"x\"}" },
                    { "type": "message", "role": "assistant", "content": [
                        { "type": "output_text", "text": "", "annotations": [] } ] }
                ],
                "usage": { "input_tokens": 5, "output_tokens": 7 },
                "incomplete_details": null,
            }),
        )
    });
    let adapter = adapter(&endpoint.base);
    let stream = adapter
        .create_turn(&auth(), &text_request("m-1", "hi"))
        .await
        .expect("the send");
    let events = collect(stream).await;
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

/// `incomplete` + `max_output_tokens` preserves the native stop reason
/// (`AI-ADAPTER-08`: nothing collapses to "done").
#[tokio::test]
async fn an_incomplete_response_preserves_max_output_tokens() {
    let endpoint = serve(|_, _| {
        Canned::Json(
            200,
            json!({
                "id": "resp_3", "model": "m-1", "status": "incomplete",
                "output": [ { "type": "message", "role": "assistant", "content": [
                    { "type": "output_text", "text": "partial", "annotations": [] } ] } ],
                "incomplete_details": { "reason": "max_output_tokens" },
                "usage": { "input_tokens": 5, "output_tokens": 128 },
            }),
        )
    });
    let adapter = adapter(&endpoint.base);
    let stream = adapter
        .create_turn(&auth(), &text_request("m-1", "hi"))
        .await
        .expect("the send");
    let events = collect(stream).await;
    assert!(events.iter().any(
        |event| matches!(event, TurnEvent::Stop { reason } if *reason == myharness_provider_sdk::NativeStopReason::MaxTokens)
    ));
    endpoint.stop();
}

/// The model echo gate: a dated revision stays legal, a different model is
/// a user-visible refusal (`AI-MODEL-01`).
#[tokio::test]
async fn the_model_echo_gate_refuses_a_substitution() {
    // A dated revision is legal.
    let endpoint =
        serve(|_, _| Canned::Json(200, completed_response("m-1-2026-01-01", "completed", "hi")));
    let legal = adapter(&endpoint.base);
    let _ = legal
        .create_turn(&auth(), &text_request("m-1", "hi"))
        .await
        .expect("a dated revision of the requested id stays legal");
    endpoint.stop();

    // A different model is a refusal.
    let endpoint =
        serve(|_, _| Canned::Json(200, completed_response("other-model", "completed", "hi")));
    let substituting = adapter(&endpoint.base);
    let error = substituting
        .create_turn(&auth(), &text_request("m-1", "hi"))
        .await
        .err()
        .expect("a substitution must refuse");
    assert_eq!(error.class, ErrorClass::ModelNotFound);
    endpoint.stop();
}

/// Documents replay natively as `input_file`, and assistant history replays
/// `output_text` plus `function_call` items — while reasoning is omitted
/// (the recorded v1 reading) and tool results become
/// `function_call_output` keyed to the call.
#[tokio::test]
async fn replay_maps_natively_and_omits_reasoning() {
    let endpoint = serve(|_, _| Canned::Json(200, completed_response("m-1", "completed", "ok")));
    let adapter = adapter(&endpoint.base);
    let request = NativeTurnRequest {
        model: model("m-1"),
        system: Vec::new(),
        messages: vec![
            (
                Role::Assistant,
                vec![
                    ContentPart::Text {
                        text: "I will look".to_string(),
                    },
                    ContentPart::ToolUse {
                        id: "call_9".to_string(),
                        name: "lookup".to_string(),
                        input: json!({ "q": "x" }),
                    },
                    ContentPart::Reasoning {
                        thinking: "because".to_string(),
                        signature: None,
                    },
                ],
            ),
            (
                Role::User,
                vec![ContentPart::ToolResult {
                    tool_use_id: "call_9".to_string(),
                    content: vec![ContentPart::Text {
                        text: "found it".to_string(),
                    }],
                    is_error: false,
                    cache_control: None,
                }],
            ),
        ],
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
    let _ = adapter
        .create_turn(&auth(), &request)
        .await
        .expect("the send");
    let input = endpoint.requests()[0].body()["input"]
        .as_array()
        .unwrap()
        .clone();
    // Assistant message with output_text.
    assert_eq!(input[0]["role"], "assistant");
    assert_eq!(input[0]["content"][0]["type"], "output_text");
    assert_eq!(input[0]["content"][0]["text"], "I will look");
    // The tool call replays flat with encoded arguments.
    assert_eq!(input[1]["type"], "function_call");
    assert_eq!(input[1]["call_id"], "call_9");
    assert_eq!(input[1]["arguments"], "{\"q\":\"x\"}");
    // Reasoning was omitted; the tool result follows as its own item.
    assert_eq!(input[2]["type"], "function_call_output");
    assert_eq!(input[2]["call_id"], "call_9");
    assert_eq!(input[2]["output"], "found it");
    assert_eq!(input.len(), 3);
    endpoint.stop();
}

/// Media inside a tool result is an explicit refusal, upstream of any
/// bytes (`AI-MM-01`; the recorded v1 reading).
#[tokio::test]
async fn media_inside_a_tool_result_is_refused_upstream() {
    let endpoint = serve(|_, _| Canned::Json(200, completed_response("m-1", "completed", "ok")));
    let adapter = adapter(&endpoint.base);
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
    let error = adapter
        .create_turn(&auth(), &request)
        .await
        .err()
        .expect("media in a tool result must refuse");
    assert_eq!(error.class, ErrorClass::UnsupportedModality);
    assert!(endpoint.requests().is_empty(), "nothing was sent");
    endpoint.stop();
}

/// The streamed text lane: deltas map, usage lands once from the terminal
/// event, and the stream terminates on `response.completed` — no sentinel
/// needed.
#[tokio::test]
async fn the_stream_maps_deltas_and_terminates_on_response_completed() {
    let endpoint = serve(|_, _| Canned::Sse {
        status: 200,
        pieces: vec![
            sse(
                "response.created",
                json!({ "type": "response.created", "response": { "id": "resp_s1", "model": "m-1" } }),
            ),
            sse(
                "response.in_progress",
                json!({ "type": "response.in_progress", "response": { "id": "resp_s1" } }),
            ),
            sse(
                "response.output_text.delta",
                json!({ "type": "response.output_text.delta", "delta": "Hel" }),
            ),
            sse(
                "response.output_text.delta",
                json!({ "type": "response.output_text.delta", "delta": "lo" }),
            ),
            sse(
                "response.completed",
                json!({
                    "type": "response.completed",
                    "response": {
                        "id": "resp_s1", "model": "m-1", "status": "completed",
                        "usage": { "input_tokens": 11, "output_tokens": 2,
                                   "input_tokens_details": { "cached_tokens": 6 } },
                        "incomplete_details": null,
                    },
                }),
            ),
        ],
    });
    let adapter = adapter(&endpoint.base);
    let mut request = text_request("m-1", "hi");
    request.stream = true;
    let stream = adapter
        .create_turn(&auth(), &request)
        .await
        .expect("the send");
    let events = collect(stream).await;
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
        TurnEvent::Usage { usage } if usage.input == 11 && usage.output == 2 && usage.cache_read == Some(6)
    )));
    assert!(events.iter().any(
        |event| matches!(event, TurnEvent::Stop { reason } if *reason == myharness_provider_sdk::NativeStopReason::EndTurn)
    ));
    assert!(matches!(events.last(), Some(TurnEvent::Done)));
    endpoint.stop();
}

/// The streamed function-call lane: announcement → argument fragments →
/// the done item assembles and validates the arguments; the stop is
/// `tool_use` (`AI-TOOL-01`).
#[tokio::test]
async fn the_stream_assembles_function_calls_across_events() {
    let endpoint = serve(|_, _| Canned::Sse {
        status: 200,
        pieces: vec![
            sse(
                "response.created",
                json!({ "type": "response.created", "response": { "id": "resp_s2", "model": "m-1" } }),
            ),
            sse(
                "response.output_item.added",
                json!({ "type": "response.output_item.added", "output_index": 0,
                            "item": { "type": "function_call", "id": "item_1", "call_id": "call_1", "name": "lookup" } }),
            ),
            sse(
                "response.function_call_arguments.delta",
                json!({ "type": "response.function_call_arguments.delta", "item_id": "item_1", "delta": "{\"q\":" }),
            ),
            sse(
                "response.function_call_arguments.delta",
                json!({ "type": "response.function_call_arguments.delta", "item_id": "item_1", "delta": "\"x\"}" }),
            ),
            sse(
                "response.output_item.done",
                json!({ "type": "response.output_item.done", "output_index": 0,
                            "item": { "type": "function_call", "id": "item_1", "call_id": "call_1",
                                      "name": "lookup", "arguments": "{\"q\":\"x\"}" } }),
            ),
            sse(
                "response.completed",
                json!({ "type": "response.completed", "response": { "id": "resp_s2", "model": "m-1",
                            "status": "completed", "usage": { "input_tokens": 4, "output_tokens": 5 } } }),
            ),
        ],
    });
    let adapter = adapter(&endpoint.base);
    let mut request = text_request("m-1", "look up x");
    request.stream = true;
    let stream = adapter
        .create_turn(&auth(), &request)
        .await
        .expect("the send");
    let events = collect(stream).await;
    // The announcement, the fragments, and the assembled close all appear.
    assert!(events.iter().any(|event| matches!(
        event,
        TurnEvent::ToolCall { id, name, arguments_delta: None, arguments: None }
            if id == "call_1" && name == "lookup"
    )));
    let fragments: usize = events
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
            if id == "call_1" && name == "lookup" && arguments == &json!({"q": "x"})
    )));
    assert!(events.iter().any(
        |event| matches!(event, TurnEvent::Stop { reason } if *reason == myharness_provider_sdk::NativeStopReason::ToolUse)
    ));
    endpoint.stop();
}

/// Reasoning summaries stream as reasoning deltas; refusal text streams as
/// text; a stream with only keep-alives between content still terminates
/// correctly.
#[tokio::test]
async fn the_stream_maps_reasoning_and_survives_keep_alives() {
    let endpoint = serve(|_, _| Canned::Sse {
        status: 200,
        pieces: vec![
            sse(
                "response.created",
                json!({ "type": "response.created", "response": { "id": "resp_s3", "model": "m-1" } }),
            ),
            SsePiece {
                delay_ms: 0,
                body: ": transport keep-alive\n\n".to_string(),
            },
            sse(
                "response.reasoning_summary_text.delta",
                json!({ "type": "response.reasoning_summary_text.delta", "delta": "thinking" }),
            ),
            sse(
                "response.refusal.delta",
                json!({ "type": "response.refusal.delta", "delta": "I cannot" }),
            ),
            sse(
                "response.completed",
                json!({ "type": "response.completed", "response": { "id": "resp_s3", "model": "m-1",
                            "status": "completed", "usage": { "input_tokens": 2, "output_tokens": 3 } } }),
            ),
        ],
    });
    let adapter = adapter(&endpoint.base);
    let mut request = text_request("m-1", "hi");
    request.stream = true;
    let stream = adapter
        .create_turn(&auth(), &request)
        .await
        .expect("the send");
    let events = collect(stream).await;
    assert!(events.iter().any(|event| matches!(
        event,
        TurnEvent::ReasoningDelta { text, .. } if text == "thinking"
    )));
    assert!(events
        .iter()
        .any(|event| matches!(event, TurnEvent::TextDelta { text } if text == "I cannot")));
    assert!(matches!(events.last(), Some(TurnEvent::Done)));
    endpoint.stop();
}

/// An undocumented event type is surfaced, never silently consumed — a
/// grammar the adapter does not know may be carrying content it would drop
/// (`AI-MM-01`).
#[tokio::test]
async fn an_undocumented_event_type_is_an_error() {
    let endpoint = serve(|_, _| Canned::Sse {
        status: 200,
        pieces: vec![
            sse(
                "response.created",
                json!({ "type": "response.created", "response": { "id": "r", "model": "m-1" } }),
            ),
            sse(
                "response.something_new",
                json!({ "type": "response.something_new", "payload": { "secret": 1 } }),
            ),
        ],
    });
    let adapter = adapter(&endpoint.base);
    let mut request = text_request("m-1", "hi");
    request.stream = true;
    let mut stream = adapter
        .create_turn(&auth(), &request)
        .await
        .expect("the send");
    let mut last = None;
    while let Some(event) = stream.next().await {
        last = Some(event);
    }
    let error = last
        .expect("the stream carries the refusal")
        .expect_err("unknown events refuse");
    assert!(error.message.contains("response.something_new"));
    endpoint.stop();
}

/// A stream that ends without a terminal event is contract-malformed: no
/// stop reason is invented (`docs/01` fail-closed rule).
#[tokio::test]
async fn a_stream_without_a_terminal_event_is_malformed() {
    let endpoint = serve(|_, _| Canned::Sse {
        status: 200,
        pieces: vec![
            sse(
                "response.created",
                json!({ "type": "response.created", "response": { "id": "r", "model": "m-1" } }),
            ),
            sse(
                "response.output_text.delta",
                json!({ "type": "response.output_text.delta", "delta": "half" }),
            ),
        ],
    });
    let adapter = adapter(&endpoint.base);
    let mut request = text_request("m-1", "hi");
    request.stream = true;
    let mut stream = adapter
        .create_turn(&auth(), &request)
        .await
        .expect("the send");
    let mut last = None;
    while let Some(event) = stream.next().await {
        last = Some(event);
    }
    let error = last
        .expect("the stream carries the refusal")
        .expect_err("no terminal event");
    assert!(error.message.contains("terminal event"));
    endpoint.stop();
}

/// `response.failed` and the bare `error` event both surface the
/// provider's own wording (`TEST-AI-021` for stream errors).
#[tokio::test]
async fn stream_failures_surface_the_provider_error() {
    for payload in [
        sse(
            "response.failed",
            json!({ "type": "response.failed", "response": { "id": "r", "status": "failed",
                    "error": { "code": "server_error", "message": "the model had an issue" } } }),
        ),
        sse(
            "error",
            json!({ "type": "error", "code": "rate_limit_error", "message": "slow down" }),
        ),
    ] {
        let endpoint = serve(move |_, _| Canned::Sse {
            status: 200,
            pieces: vec![payload.clone()],
        });
        let adapter = adapter(&endpoint.base);
        let mut request = text_request("m-1", "hi");
        request.stream = true;
        let mut stream = adapter
            .create_turn(&auth(), &request)
            .await
            .expect("the send");
        let mut last = None;
        while let Some(event) = stream.next().await {
            last = Some(event);
        }
        let error = last
            .expect("the stream carries the refusal")
            .expect_err("the failure surfaces");
        assert!(matches!(
            error.class,
            ErrorClass::Transient | ErrorClass::Quota
        ));
        endpoint.stop();
    }
}

/// The idle window fires on a stream that keeps the connection alive
/// without progress (`TEST-AI-016`); keep-alive comments never reset it.
#[tokio::test]
async fn the_idle_window_fires_without_progress() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a local port");
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    std::thread::spawn(move || {
        let stop = std::sync::atomic::AtomicBool::new(false);
        // One connection is all this fixture serves; it drips forever on it.
        if let Some(Ok(mut socket)) = listener.incoming().next() {
            // Read the request before answering: a response head that
            // arrives before the client's request body finished sending is
            // a hyper send error, so the drip must not answer blind.
            let _request = read_request(&mut socket, &stop);
            let _ = socket.write_all(
                b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ntransfer-encoding: chunked\r\n\r\n",
            );
            // One content frame, then comments forever.
            let first = "event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"id\":\"r\",\"model\":\"m-1\"}}\n\n";
            let _ = socket.write_all(format!("{:x}\r\n{first}\r\n", first.len()).as_bytes());
            loop {
                std::thread::sleep(Duration::from_millis(100));
                let comment = ": keep-alive\n\n";
                let _ =
                    socket.write_all(format!("{:x}\r\n{comment}\r\n", comment.len()).as_bytes());
                let _ = socket.flush();
            }
        }
    });
    let local = ResponsesAdapter::new(
        ResponsesClientConfig {
            base_url: base,
            idle_timeout: Duration::from_secs(1),
            custom_headers: Vec::new(),
        },
        reqwest::Client::new(),
    );
    let mut request = text_request("m-1", "hi");
    request.stream = true;
    let mut stream = local
        .create_turn(&auth(), &request)
        .await
        .expect("the send");
    let started = std::time::Instant::now();
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

/// Token counting is an explicit refusal — never an estimate
/// (`AI-ADAPTER-09`).
#[tokio::test]
async fn token_counting_refuses_instead_of_estimating() {
    let endpoint = serve(|_, _| Canned::Json(200, completed_response("m-1", "completed", "hi")));
    let adapter = adapter(&endpoint.base);
    let error = adapter
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

/// Discovery reads the platform's shared listing tolerantly: the `data`
/// array shape and the keyed `models` shape both parse; the health check
/// maps its statuses (`TEST-AI-018` adjacency).
#[tokio::test]
async fn discovery_and_health_read_the_listing() {
    let endpoint = serve(|index, _| {
        if index == 0 {
            Canned::Json(
                200,
                json!({ "data": [
                    { "id": "m-1", "display_name": "Model One" },
                    { "id": "m-2" },
                ]}),
            )
        } else if index == 1 {
            Canned::Json(
                401,
                json!({ "error": { "message": "bad key", "type": "authentication_error" } }),
            )
        } else {
            Canned::Json(200, json!({ "models": [ { "model": "m-3" } ] }))
        }
    });
    let adapter = adapter(&endpoint.base);
    let models = adapter.list_models(&auth()).await.expect("the listing");
    assert_eq!(models.len(), 2);
    assert_eq!(models[0].key.model_id, "m-1");
    assert_eq!(models[0].display_name.as_deref(), Some("Model One"));

    let health = adapter
        .health_check(&auth())
        .await
        .expect("the health check");
    assert_eq!(health.authenticated, Some(false));

    // Reuse the same endpoint for the keyed shape: a fresh adapter shares
    // the log, so this is the third connection.
    let models = adapter
        .list_models(&auth())
        .await
        .expect("the keyed listing");
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].key.model_id, "m-3");
    endpoint.stop();
}
