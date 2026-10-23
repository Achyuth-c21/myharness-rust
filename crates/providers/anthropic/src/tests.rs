//! Unit tests of the parse/serialize halves of the family's wire contract.
//! The mock-provider wire fixtures live in `tests/wire.rs`.

use crate::discovery::descriptor_from_row;
use crate::messages::{count_tokens_request_body, messages_request_body, SseParser, StreamEvent};
use crate::{AnthropicAdapter, AnthropicClientConfig, DEFAULT_BASE_URL, DEFAULT_IDLE_TIMEOUT};
use myharness_provider_sdk::{
    ContentDelta, ContentPart, ErrorClass, ModelKey, NativeStopReason, Role, SystemBlock, ToolSpec,
};
use serde_json::json;

fn model(id: &str) -> ModelKey {
    ModelKey {
        provider_id: "anthropic".to_string(),
        model_id: id.to_string(),
        variant: None,
        revision: None,
    }
}

fn config(base: Option<String>) -> AnthropicClientConfig {
    AnthropicClientConfig {
        base_url: base,
        custom_headers: Vec::new(),
        idle_timeout: DEFAULT_IDLE_TIMEOUT,
        betas: Vec::new(),
    }
}

/// The absence of a configured base URL means the first-party default host
/// (`docs/12` §1), and a present one is honored and slash-normalized
/// (`docs/12` §3.1; the base URL key's spelling is a `docs/14` §9 unknown).
#[test]
fn the_default_base_is_the_first_party_host() {
    assert_eq!(DEFAULT_BASE_URL, "https://api.anthropic.com");
    let adapter = AnthropicAdapter::new(config(None), reqwest::Client::new());
    assert_eq!(adapter.base_url(), "https://api.anthropic.com");
    let adapter = AnthropicAdapter::new(
        config(Some("http://127.0.0.1:9/".to_string())),
        reqwest::Client::new(),
    );
    assert_eq!(adapter.base_url(), "http://127.0.0.1:9");
}

/// The native body shape: verbatim model id, tools only when declared, cache
/// controls preserved (`AI-ADAPTER-05/10/11`).
#[test]
fn the_messages_body_is_native_and_complete() {
    let request = myharness_provider_sdk::NativeTurnRequest {
        model: model("claude-opus-4-1"),
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
                vec![
                    ContentPart::ToolUse {
                        id: "toolu_1".to_string(),
                        name: "get_weather".to_string(),
                        input: json!({ "city": "Osaka" }),
                    },
                    ContentPart::Reasoning {
                        thinking: "hmm".to_string(),
                        signature: Some("sig==".to_string()),
                    },
                    ContentPart::RedactedThinking {
                        data: json!({ "data": "opaque" }),
                    },
                ],
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
                    cache_control: Some("ephemeral".to_string()),
                }],
            ),
        ],
        tools: vec![ToolSpec {
            name: "get_weather".to_string(),
            description: Some("Get the weather".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": { "city": { "type": "string" } }
            }),
            cache_control: Some("ephemeral".to_string()),
        }],
        max_tokens: 64,
        temperature: Some(0.5),
        top_p: None,
        top_k: None,
        stop_sequences: vec!["\n\n".to_string()],
        thinking: None,
        betas: vec!["interleaved-thinking-2025-05-14".to_string()],
        stream: true,
    };
    let body = messages_request_body(&request).unwrap();
    // Model verbatim, no invented keys (`AI-ADAPTER-11`; `AI-MODEL-02`).
    assert_eq!(body["model"], "claude-opus-4-1");
    assert_eq!(body["max_tokens"], 64);
    assert_eq!(body["stream"], true);
    // Nothing invented from the model family: only what the caller set.
    assert!(body.get("top_p").is_none());
    assert!(body.get("top_k").is_none());
    // System blocks keep the caching control (`AI-ADAPTER-05`).
    assert_eq!(body["system"][0]["cache_control"], "ephemeral");
    // Thinking (with signature) and redacted thinking survive verbatim
    // (`AI-ADAPTER-04`).
    let assistant = &body["messages"][1]["content"];
    assert_eq!(assistant[0]["type"], "tool_use");
    assert_eq!(assistant[1]["thinking"], "hmm");
    assert_eq!(assistant[1]["signature"], "sig==");
    assert_eq!(assistant[2]["type"], "redacted_thinking");
    // Tool results keep images images (`AI-ADAPTER-06`).
    let result = &body["messages"][2]["content"][0];
    assert_eq!(result["type"], "tool_result");
    assert_eq!(result["is_error"], true);
    assert_eq!(result["cache_control"], "ephemeral");
    assert_eq!(result["content"][1]["type"], "image");
    assert_eq!(result["content"][1]["source"]["media_type"], "image/jpeg");
    // Tools keep their schema and control (`AI-TOOL-01`).
    assert_eq!(body["tools"][0]["cache_control"], "ephemeral");
    assert_eq!(
        body["tools"][0]["input_schema"]["properties"]["city"]["type"],
        "string"
    );
}

/// Nothing invented: no tools key, no temperature key when the caller chose
/// none (`AI-ADAPTER-10`).
#[test]
fn the_body_invents_nothing() {
    let request = myharness_provider_sdk::NativeTurnRequest {
        model: model("claude-sonnet-4"),
        system: Vec::new(),
        messages: vec![(
            Role::User,
            vec![ContentPart::Text {
                text: "hi".to_string(),
            }],
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
    let body = messages_request_body(&request).unwrap();
    assert!(body.get("tools").is_none());
    assert!(body.get("temperature").is_none());
    assert!(body.get("top_p").is_none());
    assert!(body.get("system").is_none());
    assert_eq!(body["stream"], false);
}

/// The count body is the same native grammar without an output budget.
#[test]
fn the_count_body_carries_the_request() {
    let request = myharness_provider_sdk::CountRequest {
        model: model("claude-opus-4-1"),
        system: vec![SystemBlock {
            text: "sys".to_string(),
            cache_control: None,
        }],
        messages: vec![(
            Role::User,
            vec![ContentPart::Text {
                text: "hi".to_string(),
            }],
        )],
        tools: Vec::new(),
    };
    let body = count_tokens_request_body(&request).unwrap();
    assert_eq!(body["model"], "claude-opus-4-1");
    assert_eq!(body["system"][0]["type"], "text");
    assert!(body.get("tools").is_none());
    assert!(body.get("max_tokens").is_none());
}

/// A stream parses identically no matter where the byte chunks are cut
/// (`AI-STREAM-01`).
#[test]
fn sse_frames_survive_chunk_boundaries() {
    let full = concat!(
        "event: message_start\n",
        "data: {\"message\":{\"model\":\"claude-opus-4-1\",\"usage\":{\"input_tokens\":7,\"output_tokens\":1}}}\n\n",
        "event: ping\n",
        "data: {}\n\n",
        "event: content_block_start\n",
        "data: {\"index\":0,\"content_block\":{\"type\":\"tool_use\",\"id\":\"toolu_9\",\"name\":\"read_file\"}}\n\n",
        "event: content_block_delta\n",
        "data: {\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"path\\\":\"}}\n\n",
        "event: content_block_delta\n",
        "data: {\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"hi\"}}\n\n",
        "event: content_block_stop\n",
        "data: {\"index\":0}\n\n",
        "event: message_delta\n",
        "data: {\"delta\":{\"stop_reason\":\"tool_use\",\"stop_sequence\":null},\"usage\":{\"output_tokens\":12}}\n\n",
        "event: message_stop\n",
        "data: {}\n\n",
    );
    let parse = |pieces: &[&str]| -> Vec<StreamEvent> {
        let mut parser = SseParser::new();
        let mut events = Vec::new();
        for piece in pieces {
            for result in parser.feed(piece.as_bytes()) {
                events.push(result.unwrap());
            }
        }
        events
    };
    let whole = parse(&[full]);
    let one = parse(&[&full[..40], &full[40..41], &full[41..]]);
    let two = parse(&[&full[..17], &full[17..], ""]);
    assert_eq!(whole, one, "arbitrary splits must not change the parse");
    assert_eq!(whole, two);

    match &whole[0] {
        StreamEvent::MessageStart { model, usage } => {
            assert_eq!(model.as_deref(), Some("claude-opus-4-1"));
            let usage = usage.clone().unwrap();
            assert_eq!(usage.input, 7);
            assert_eq!(usage.output, 1);
        }
        other => panic!("expected message_start first, got {other:?}"),
    }
    // A ping frame is a keep-alive, not progress.
    assert!(matches!(whole[1], StreamEvent::Ping));
    match &whole[2] {
        StreamEvent::BlockStart { index, block } => {
            assert_eq!(*index, 0);
            assert_eq!(block["id"], "toolu_9");
            assert_eq!(block["name"], "read_file");
        }
        other => panic!("expected block start, got {other:?}"),
    }
    match &whole[3] {
        StreamEvent::Delta { index, delta } => {
            assert_eq!(*index, 0);
            match delta {
                ContentDelta::InputJson { partial_json } => {
                    assert_eq!(partial_json, "{\"path\":")
                }
                other => panic!("expected input_json_delta, got {other:?}"),
            }
        }
        other => panic!("expected a delta, got {other:?}"),
    }
    match &whole[6] {
        StreamEvent::MessageDelta {
            stop_reason,
            stop_sequence,
            usage,
        } => {
            assert_eq!(stop_reason.as_ref(), Some(&NativeStopReason::ToolUse));
            assert_eq!(stop_sequence.as_deref(), None);
            assert_eq!(usage.clone().unwrap().output, 12);
        }
        other => panic!("expected message_delta, got {other:?}"),
    }
    assert!(matches!(whole[7], StreamEvent::MessageStop));
}

/// CRLF framing parses the same way (`docs/12` §4.1: SSE).
#[test]
fn crlf_frames_parse() {
    let mut parser = SseParser::new();
    let chunk = b"event: message_stop\r\ndata: {}\r\n\r\n";
    let events = parser.feed(chunk);
    assert_eq!(events.len(), 1);
    assert!(matches!(
        events[0].as_ref().unwrap(),
        StreamEvent::MessageStop
    ));
}

/// A named event with no payload parses as an empty object
/// (`event: message_stop` without `data:` is legal).
#[test]
fn a_payloadless_named_event_parses() {
    let mut parser = SseParser::new();
    let events = parser.feed(b"event: message_stop\n\n");
    assert_eq!(events.len(), 1);
    assert!(matches!(
        events[0].as_ref().unwrap(),
        StreamEvent::MessageStop
    ));
}

/// A malformed frame surfaces instead of being skipped (`AI-MM-01`).
#[test]
fn a_broken_frame_surfaces() {
    let mut parser = SseParser::new();
    let events = parser.feed(b"event: message_start\ndata: {broken\n\n");
    assert!(events[0].is_err());
}

/// Unknown event/delta kinds are named, never silently dropped
/// (`AI-MM-01`; `AI-STREAM-01`).
#[test]
fn an_unsubscribed_stream_event_names_itself() {
    let mut parser = SseParser::new();
    let events = parser.feed(b"event: novel_event\ndata: {}\n\n");
    let error = events[0].as_ref().unwrap_err();
    assert_eq!(error.class, ErrorClass::UnsupportedModality);
    assert!(error.message.contains("novel_event"));

    let events = parser.feed(
        b"event: content_block_delta\ndata: {\"index\":0,\"delta\":{\"type\":\"mood_delta\"}}\n\n",
    );
    let error = events[0].as_ref().unwrap_err();
    assert!(error.message.contains("mood_delta"));
}

/// A row maps onto a descriptor with the family's provider id; the raw row
/// is kept as `unmodeled` so catalog data can be joined later (`docs/12` §8.3).
#[test]
fn a_discovery_row_maps_onto_a_descriptor() {
    let row = json!({
        "id": "claude-opus-4-1",
        "display_name": "Claude Opus 4.1",
        "context_window": 200000,
    });
    let descriptor = descriptor_from_row("anthropic", &row).unwrap();
    assert_eq!(descriptor.key.provider_id, "anthropic");
    assert_eq!(descriptor.key.model_id, "claude-opus-4-1");
    assert_eq!(descriptor.display_name.as_deref(), Some("Claude Opus 4.1"));
    assert_eq!(descriptor.capabilities.context_window, Some(200000));
    // Absent metadata stays unsupported, never optimistic (`AI-PROVIDER-05`).
    assert!(!descriptor.capabilities.tools_supported);
    assert!(descriptor.capabilities.thinking_supported.is_none());
    assert_eq!(descriptor.unmodeled, Some(row));
    // A row without an id is not a model row.
    assert!(descriptor_from_row("anthropic", &json!({ "detail": "x" })).is_none());
}
