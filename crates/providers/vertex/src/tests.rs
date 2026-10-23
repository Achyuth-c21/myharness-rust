//! Unit tests of the parse/serialize halves of the family's wire contract.
//! The mock-provider wire fixtures live in `tests/wire.rs`.

use crate::messages::{
    encode_path_segment, error_from_envelope, raw_predict_body, raw_predict_stream_url,
    raw_predict_url, status_class, stream_event_from_value,
};
use crate::sse::{DataOnlyParser, SseItem};
use crate::{default_base_url as default_base_url_for_tests, VertexAdapter, VertexClientConfig,
    DEFAULT_IDLE_TIMEOUT};
use myharness_provider_sdk::{
    ContentPart, ErrorClass, ModelKey, NativeStopReason, Role, SystemBlock, TurnEvent, Usage,
};
use serde_json::{json, Value};

fn model(id: &str) -> ModelKey {
    ModelKey {
        provider_id: "vertex".to_string(),
        model_id: id.to_string(),
        variant: None,
        revision: None,
    }
}

fn config() -> VertexClientConfig {
    VertexClientConfig {
        base_url: "https://us-east5-aiplatform.googleapis.com".to_string(),
        project_id: "my-project".to_string(),
        region: "us-east5".to_string(),
        custom_headers: Vec::new(),
        idle_timeout: DEFAULT_IDLE_TIMEOUT,
    }
}

/// The regional host derives from the region (`docs/12` §1), and an empty
/// region is a configuration error, not a request.
#[test]
fn the_host_derives_from_the_region() {
    assert_eq!(
        default_base_url_for_tests("us-east5").unwrap(),
        "https://us-east5-aiplatform.googleapis.com"
    );
    assert!(default_base_url_for_tests("  ").is_err());
}

/// The adapter normalizes trailing slashes on the endpoint and trims the
/// identifiers it carries (`docs/12` §3.1).
#[test]
fn the_adapter_normalizes_trailing_slashes() {
    let mut config = config();
    config.base_url = "http://127.0.0.1:9//".to_string();
    config.project_id = " proj ".to_string();
    let adapter = VertexAdapter::new(config, reqwest::Client::new());
    assert_eq!(adapter.base_url(), "http://127.0.0.1:9");
}

/// The native body: the Anthropic grammar with **no `model` field** (the
/// path carries it) and **no `anthropic_version`** (none is recorded for
/// this family; `docs/14` §9 item 28), tools only when declared, cache
/// controls preserved (`AI-ADAPTER-05/10`).
#[test]
fn the_raw_predict_body_is_native_and_complete() {
    let request = myharness_provider_sdk::NativeTurnRequest {
        model: model("claude-sonnet-4-5"),
        system: vec![SystemBlock {
            text: "be brief".to_string(),
            cache_control: Some("ephemeral".to_string()),
        }],
        messages: vec![
            (
                Role::User,
                vec![
                    ContentPart::Text {
                        text: "look".to_string(),
                    },
                    ContentPart::Image {
                        media_type: "image/png".to_string(),
                        data: "aW1n".to_string(),
                    },
                ],
            ),
            (
                Role::Assistant,
                vec![ContentPart::ToolUse {
                    id: "toolu_1".to_string(),
                    name: "get_weather".to_string(),
                    input: json!({ "city": "Osaka" }),
                }],
            ),
            (
                Role::User,
                vec![ContentPart::ToolResult {
                    tool_use_id: "toolu_1".to_string(),
                    content: vec![
                        ContentPart::Text {
                            text: "sunny".to_string(),
                        },
                        ContentPart::Image {
                            media_type: "image/jpeg".to_string(),
                            data: "anBn".to_string(),
                        },
                    ],
                    is_error: true,
                    cache_control: None,
                }],
            ),
        ],
        tools: vec![myharness_provider_sdk::ToolSpec {
            name: "get_weather".to_string(),
            description: None,
            input_schema: json!({ "type": "object", "properties": {} }),
            cache_control: None,
        }],
        max_tokens: 64,
        temperature: Some(0.5),
        top_p: None,
        top_k: None,
        stop_sequences: Vec::new(),
        thinking: Some(myharness_provider_sdk::ThinkingConfig { budget_tokens: 512 }),
        betas: Vec::new(),
        stream: true,
    };
    let body = raw_predict_body(&request).unwrap();
    // The model and the version declaration stay out of the body
    // (`docs/14` §9 item 28).
    assert!(body.get("model").is_none());
    assert!(body.get("anthropic_version").is_none());
    assert_eq!(body["stream"], json!(true));
    assert_eq!(body["max_tokens"], json!(64));
    assert_eq!(body["thinking"]["budget_tokens"], json!(512));
    // Nothing invented (`AI-ADAPTER-10`).
    assert!(body.get("top_k").is_none());
    assert_eq!(body["temperature"], json!(0.5));
    assert_eq!(body["system"][0]["cache_control"], json!("ephemeral"));
    // Tool-result parts stay parts — never flattened to text
    // (`AI-ADAPTER-06`, `AI-MM-02`).
    assert_eq!(body["messages"][2]["content"][0]["type"], json!("tool_result"));
    assert_eq!(
        body["messages"][2]["content"][0]["content"][1],
        json!({ "type": "image", "source": { "type": "base64", "media_type": "image/jpeg", "data": "anBn" } })
    );
}

/// Nothing invented in the minimal request (`AI-ADAPTER-10`).
#[test]
fn the_minimal_body_invents_nothing() {
    let request = myharness_provider_sdk::NativeTurnRequest {
        model: model("claude-3-5-haiku"),
        system: Vec::new(),
        messages: vec![(
            Role::User,
            vec![ContentPart::Text { text: "ping".to_string() }],
        )],
        tools: Vec::new(),
        max_tokens: 1,
        temperature: None,
        top_p: None,
        top_k: None,
        stop_sequences: Vec::new(),
        thinking: None,
        betas: Vec::new(),
        stream: false,
    };
    let body = raw_predict_body(&request).unwrap();
    assert!(body.get("system").is_none());
    assert!(body.get("tools").is_none());
    assert!(body.get("temperature").is_none());
    assert!(body.get("top_p").is_none());
    assert!(body.get("anthropic_version").is_none());
    assert_eq!(body["stream"], json!(false));
}

/// The request URL is the Vertex raw-prediction route: project/region/model
/// segments encoded, the verb separated by a literal `:` (`docs/14` §9
/// item 28).
#[test]
fn the_urls_match_the_raw_prediction_routes() {
    let mut config = config();
    config.base_url = "http://127.0.0.1:9/".to_string();
    assert_eq!(
        raw_predict_url(&config, "claude-sonnet-4-5"),
        "http://127.0.0.1:9/v1/projects/my-project/locations/us-east5/publishers/google/models/claude-sonnet-4-5:rawPredict"
    );
    assert_eq!(
        raw_predict_stream_url(&config, "claude-sonnet-4-5"),
        "http://127.0.0.1:9/v1/projects/my-project/locations/us-east5/publishers/google/models/claude-sonnet-4-5:streamRawPredict"
    );
    // Path segments escape punctuation (a colon never reshapes them — even
    // the family's harmless separator is escaped here: only the verb's `:`
    // travels literally).
    assert_eq!(encode_path_segment("a/b"), "a%2Fb");
    assert_eq!(encode_path_segment("a:b"), "a%3Ab");
    assert_eq!(encode_path_segment("claude-ok-1.0"), "claude-ok-1.0");
}

/// The Google error envelope mapping (`TEST-AI-021`): the `.error.status`
/// reason decides, the HTTP status covers nameless envelopes, and unknown
/// reasons fall back correctly.
#[test]
fn error_envelopes_map_to_classes() {
    let unauthenticated = json!({
        "error": { "code": 401, "message": "request had invalid authentication", "status": "UNAUTHENTICATED" }
    });
    let error = error_from_envelope(Some(401), &unauthenticated);
    assert_eq!(error.class, ErrorClass::Auth);
    assert_eq!(error.message, "request had invalid authentication");
    assert_eq!(
        error_from_envelope(Some(429), &json!({
            "error": { "code": 429, "message": "quota exceeded", "status": "RESOURCE_EXHAUSTED" }
        })).class,
        ErrorClass::Quota
    );
    assert_eq!(
        error_from_envelope(Some(404), &json!({
            "error": { "code": 404, "message": "not found", "status": "NOT_FOUND" }
        })).class,
        ErrorClass::ModelNotFound
    );
    assert_eq!(
        error_from_envelope(Some(400), &json!({
            "error": { "code": 400, "message": "bad body", "status": "INVALID_ARGUMENT" }
        })).class,
        ErrorClass::InvalidRequest
    );
    assert_eq!(
        error_from_envelope(Some(503), &json!({
            "error": { "code": 503, "message": "try again", "status": "UNAVAILABLE" }
        })).class,
        ErrorClass::Transient
    );
    // A nameless envelope falls back to the status; an unknown reason too.
    assert_eq!(
        error_from_envelope(Some(500), &json!({ "message": "no error object" })).class,
        ErrorClass::Transient
    );
    assert_eq!(
        error_from_envelope(Some(500), &json!({
            "error": { "code": 500, "message": "weird", "status": "SOMETHING_ELSE" }
        })).class,
        ErrorClass::Transient
    );
    // The status fallback covers the whole vocabulary.
    assert_eq!(status_class(403), ErrorClass::Auth);
    assert_eq!(status_class(404), ErrorClass::ModelNotFound);
    assert_eq!(status_class(400), ErrorClass::InvalidRequest);
}

/// The data-only SSE reader: chunk-boundary splits, comments are
/// keep-alives, the `[DONE]` sentinel is its own item, non-JSON data lines
/// and non-data lines surface (`docs/14` §9 item 28).
#[test]
fn the_data_only_reader_parses_lines() {
    let mut parser = DataOnlyParser::new();
    let mut items = Vec::new();
    // Feed in awkward chunks.
    for chunk in b"data: {\"type\":\"ping\"}\r\n\r\n: keep\n\ndata: [DONE]\n".chunks(5) {
        items.extend(parser.feed(chunk));
    }
    assert_eq!(
        items,
        vec![
            Ok(SseItem::Data(json!({ "type": "ping" }))),
            Ok(SseItem::Keepalive), // the blank line after the data block
            Ok(SseItem::Keepalive), // the comment
            Ok(SseItem::Keepalive), // the blank line after the comment
            Ok(SseItem::Sentinel),
        ]
    );

    // A non-JSON data surface, not a skip.
    let mut parser = DataOnlyParser::new();
    let items = parser.feed(b"data: oops\n");
    assert!(matches!(items[0], Err(ref error) if error.message.contains("not valid JSON")));

    // A non-data line is unsupported, never swallowed.
    let mut parser = DataOnlyParser::new();
    let items = parser.feed(b"event: message_start\n");
    assert!(
        matches!(items[0], Err(ref error) if error.class == ErrorClass::UnsupportedModality)
    );
}

/// The event construal: the grammar's events parse; an unknown event kind is
/// named (`AI-MM-01`).
#[test]
fn events_construe_to_stream_events() {
    assert_eq!(
        stream_event_from_value(&json!({
            "type": "message_start",
            "message": {
                "model": "claude-sonnet-4-5",
                "usage": { "input_tokens": 8, "output_tokens": 1 }
            }
        })).unwrap().unwrap(),
        crate::messages::StreamEvent::MessageStart {
            model: Some("claude-sonnet-4-5".to_string()),
            usage: Some(Usage {
                input: 8,
                output: 1,
                cache_read: None,
                cache_write: None,
                web_search: None,
            }),
        }
    );
    assert_eq!(
        stream_event_from_value(&json!({ "type": "message_stop" })).unwrap().unwrap(),
        crate::messages::StreamEvent::MessageStop
    );
    let error = stream_event_from_value(&json!({
        "type": "content_block_delta",
        "index": 0,
        "delta": { "type": "spotify_playlist", "tracks": [] }
    })).unwrap();
    assert!(matches!(error, Err(ref e) if e.class == ErrorClass::UnsupportedModality));
    let error = stream_event_from_value(&json!({ "type": "weird_thing" })).unwrap();
    assert!(
        matches!(error, Err(ref e) if e.class == ErrorClass::UnsupportedModality),
        "{error:?}"
    );
}

/// Stop reasons preserve the native spelling (`AI-ADAPTER-08`), including
/// spellings outside the grammar.
#[test]
fn stop_reasons_preserve_the_native_spelling() {
    assert_eq!(NativeStopReason::parse("end_turn"), NativeStopReason::EndTurn);
    assert_eq!(
        NativeStopReason::parse("pause_turn"),
        NativeStopReason::PauseTurn
    );
    assert_eq!(
        NativeStopReason::parse("guardrail_intervened"),
        NativeStopReason::Other("guardrail_intervened".to_string())
    );
}

/// The answer's content blocks carry one-to-one; the model echo is
/// conditional and enforced when present (`AI-MM-01`, `AI-MODEL-01`;
/// `docs/14` §9 item 28).
#[test]
fn blocks_carry_and_the_echo_is_conditional() {
    let response = json!({
        "model": "claude-sonnet-4-5",
        "content": [
            { "type": "text", "text": "hello" },
            { "type": "tool_use", "id": "toolu_9", "name": "get_weather", "input": {} }
        ],
        "stop_reason": "tool_use",
        "usage": { "input_tokens": 3, "output_tokens": 5 }
    });
    let events = VertexAdapter::events_from_response(&response, &model("claude-sonnet-4-5"))
        .unwrap();
    assert!(events.contains(&TurnEvent::ToolCall {
        id: "toolu_9".to_string(),
        name: "get_weather".to_string(),
        arguments_delta: None,
        arguments: Some(json!({})),
    }));
    // A mismatched echo refuses; an absent one does not.
    let response = json!({
        "model": "claude-3-haiku",
        "content": [],
        "stop_reason": "end_turn"
    });
    let error = VertexAdapter::events_from_response(&response, &model("claude-sonnet-4-5"));
    assert_eq!(error.unwrap_err().class, ErrorClass::ModelNotFound);
    let response = json!({
        "content": [{ "type": "text", "text": "hello" }],
        "stop_reason": "end_turn",
        "usage": { "input_tokens": 1, "output_tokens": 1 }
    });
    let events = VertexAdapter::events_from_response(&response, &model("claude-sonnet-4-5"))
        .unwrap();
    assert!(matches!(events.last(), Some(TurnEvent::Done)));
    let _ = Value::Null;
}
