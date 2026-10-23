//! Unit tests of the parse/serialize halves of the family's wire contract.
//! The mock-provider wire fixtures live in `tests/wire.rs`.

use crate::eventstream::{crc32c, encode_frame, FrameDecoder, Header, HeaderValue};
use crate::messages::{
    encode_model_segment, error_from_envelope, frame_to_stream_event, invoke_body, StreamEvent,
};
use crate::{
    default_control_base_url, default_runtime_base_url, BedrockAdapter, BedrockClientConfig,
    ANTHROPIC_VERSION_BEDROCK, DEFAULT_IDLE_TIMEOUT,
};
use myharness_provider_sdk::{
    ContentPart, ErrorClass, ModelKey, NativeStopReason, Role, SystemBlock, ToolSpec,
};
use serde_json::{json, Value};

fn model(id: &str) -> ModelKey {
    ModelKey {
        provider_id: "bedrock".to_string(),
        model_id: id.to_string(),
        variant: None,
        revision: None,
    }
}

fn config() -> BedrockClientConfig {
    BedrockClientConfig {
        runtime_base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".to_string(),
        control_base_url: "https://bedrock.us-east-1.amazonaws.com".to_string(),
        custom_headers: Vec::new(),
        idle_timeout: DEFAULT_IDLE_TIMEOUT,
    }
}

/// The hosted hosts are the region-derivation the family records (`docs/12`
/// §1: `bedrock-runtime[-fips].<region>.amazonaws.com` and the control-plane
/// row), and an empty region is a configuration error, not a request.
#[test]
fn the_hosts_derive_from_the_region() {
    assert_eq!(
        default_runtime_base_url("us-east-1").unwrap(),
        "https://bedrock-runtime.us-east-1.amazonaws.com"
    );
    assert_eq!(
        default_control_base_url("us-east-1").unwrap(),
        "https://bedrock.us-east-1.amazonaws.com"
    );
    assert!(default_runtime_base_url("  ").is_err());
}

/// The adapter normalizes trailing slashes on both endpoints (`docs/12` §3.1).
#[test]
fn the_adapter_normalizes_trailing_slashes() {
    let mut config = config();
    config.runtime_base_url = "http://127.0.0.1:9//".to_string();
    config.control_base_url = "http://127.0.0.2:9/".to_string();
    let adapter = BedrockAdapter::new(config, reqwest::Client::new());
    assert_eq!(adapter.runtime_base_url(), "http://127.0.0.1:9");
    assert_eq!(adapter.control_base_url(), "http://127.0.0.2:9");
}

/// The native body shape: the family's `anthropic_version`, **no `model`
/// field** (the path carries the model), tools only when declared, cache
/// controls preserved (`AI-ADAPTER-05/10/11`).
#[test]
fn the_invoke_body_is_native_and_complete() {
    let request = myharness_provider_sdk::NativeTurnRequest {
        model: model("anthropic.claude-3-5-sonnet-20240620-v1:0"),
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
        betas: Vec::new(),
        stream: true,
    };
    let body = invoke_body(&request).unwrap();
    assert_eq!(body["anthropic_version"], json!(ANTHROPIC_VERSION_BEDROCK));
    // The model travels in the path, not the body (`docs/14` §9).
    assert!(body.get("model").is_none());
    assert_eq!(body["stream"], json!(true));
    assert_eq!(body["max_tokens"], json!(64));
    // Nothing invented (`AI-ADAPTER-10`).
    assert!(body.get("top_k").is_none());
    assert_eq!(body["temperature"], json!(0.5));
    assert_eq!(body["stop_sequences"], json!(["\n\n"]));
    assert_eq!(
        body["system"],
        json!([{
            "type": "text", "text": "be brief", "cache_control": "ephemeral"
        }])
    );
    assert_eq!(body["messages"][0]["content"][1]["type"], json!("image"));
    assert_eq!(
        body["messages"][1]["content"][2]["type"],
        json!("redacted_thinking")
    );
    // Tool-result parts stay parts — never flattened to text
    // (`AI-ADAPTER-06`, `AI-MM-02`).
    assert_eq!(
        body["messages"][2]["content"][0]["type"],
        json!("tool_result")
    );
    assert_eq!(
        body["messages"][2]["content"][0]["content"][1],
        json!({ "type": "image", "source": { "type": "base64", "media_type": "image/jpeg", "data": "anBn" } })
    );
    assert_eq!(body["tools"][0]["input_schema"]["type"], json!("object"));
    assert_eq!(body["tools"][0]["cache_control"], json!("ephemeral"));
}

/// Nothing invented in the minimal request: no tools, no temperature, no
/// thinking keys that the caller did not decide (`AI-ADAPTER-10`).
#[test]
fn the_minimal_body_invents_nothing() {
    let request = myharness_provider_sdk::NativeTurnRequest {
        model: model("anthropic.claude-3-5-haiku"),
        system: Vec::new(),
        messages: vec![(
            Role::User,
            vec![ContentPart::Text {
                text: "ping".to_string(),
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
    let body = invoke_body(&request).unwrap();
    assert!(body.get("system").is_none());
    assert!(body.get("tools").is_none());
    assert!(body.get("temperature").is_none());
    assert!(body.get("top_p").is_none());
    assert!(body.get("thinking").is_none());
    assert_eq!(body["stream"], json!(false));
    assert_eq!(
        body["messages"],
        json!([{ "role": "user", "content": [{ "type": "text", "text": "ping" }] }])
    );
}

/// The model path segment keeps the ARN's `:` separators and escapes the
/// rest — notably `/`, so an inference-profile ARN cannot split the path
/// (`docs/14` §9).
#[test]
fn the_model_segment_is_path_safe() {
    assert_eq!(
        encode_model_segment("anthropic.claude-3-5-sonnet-20240620-v1:0"),
        "anthropic.claude-3-5-sonnet-20240620-v1:0"
    );
    assert_eq!(
        encode_model_segment("arn:aws:bedrock:us-east-1::inference-profile/abc"),
        "arn:aws:bedrock:us-east-1::inference-profile%2Fabc"
    );
    assert_eq!(encode_model_segment("we ird"), "we%20ird");
}

/// The published CRC-32C check value pins the implementation's CRC
/// independent of every encode/decode path (`"123456789"` → `0xE3069283`).
#[test]
fn the_crc32c_matches_the_published_check_value() {
    assert_eq!(crc32c(b"123456789"), 0xE3069283);
    let _ = crc32c(&[]);
}

/// A frame assembled **by hand** (no encoder path) round-trips through the
/// decoder: the header block, the payload, and both CRCs verified byte by
/// byte. This is the ground-truth check on the format reading itself.
#[test]
fn a_hand_assembled_frame_round_trips() {
    // Frame: total = 2; prelude (12 bytes); header `:message-type`=str
    // "event"; payload "{}"; CRC u32.
    let mut bytes: Vec<u8> = Vec::new();
    // Header block: name_length(1) + ":message-type"(13) + type(1)
    //              + value_length(2) + "event"(5) = 22.
    let headers_length = 1u32 + 13 + 1 + 2 + 5;
    // total = 12 (prelude) + 22 (headers) + 2 (payload) + 4 (crc).
    let total = 12u32 + headers_length + 2 + 4;
    bytes.extend_from_slice(&total.to_be_bytes());
    bytes.extend_from_slice(&headers_length.to_be_bytes());
    bytes.extend_from_slice(&crc32c(&bytes[..8]).to_be_bytes());
    // Header: name ":message-type" (13), type 7 (str), value "event" (5).
    bytes.push(13);
    bytes.extend_from_slice(b":message-type");
    bytes.push(7);
    bytes.extend_from_slice(&(5u16).to_be_bytes());
    bytes.extend_from_slice(b"event");
    // Payload: "{}".
    bytes.extend_from_slice(b"{}");
    bytes.extend_from_slice(&crc32c(&bytes).to_be_bytes());

    let mut decoder = FrameDecoder::new();
    let frames = decoder.feed(&bytes);
    assert_eq!(frames.len(), 1);
    let frame = frames.into_iter().next().unwrap().unwrap();
    assert_eq!(frame.message_type(), Some("event"));

    // The encoder produces the same bytes for the same frame — one spelling,
    // two paths.
    let encoded = encode_frame(
        &[Header::new(
            ":message-type",
            HeaderValue::Str("event".to_string()),
        )],
        b"{}",
    );
    assert_eq!(encoded, bytes);

    // A corrupt payload CRC is refused, not ridden through.
    let mut corrupt = encoded.clone();
    let last = corrupt.len() - 1;
    corrupt[last] ^= 0xFF;
    let mut decoder = FrameDecoder::new();
    let frames = decoder.feed(&corrupt);
    assert!(matches!(frames[0], Err(ref error) if error.message.contains("CRC")));
}

/// One frame delivered across arbitrary chunk boundaries yields exactly one
/// frame once the final byte lands.
#[test]
fn frames_arrive_exact_across_chunks() {
    let frame = encode_frame(
        &[
            Header::new(":message-type", HeaderValue::Str("event".to_string())),
            Header::new(":event-type", HeaderValue::Str("message_stop".to_string())),
        ],
        br#"{"type":"message_stop"}"#,
    );
    let mut decoder = FrameDecoder::new();
    let mut out = Vec::new();
    for chunk in frame.chunks(7) {
        out.extend(decoder.feed(chunk));
    }
    assert_eq!(out.len(), 1);
    let frame = out.into_iter().next().unwrap().unwrap();
    assert_eq!(frame.event_type(), Some("message_stop"));
}

/// Frame construal: exception frames map to provider errors; keep-alives
/// stay Pings; the grammar's events parse.
#[test]
fn frames_construe_to_stream_events() {
    let exception = encode_frame(
        &[
            Header::new(":message-type", HeaderValue::Str("exception".to_string())),
            Header::new(
                ":exception-type",
                HeaderValue::Str("ThrottledException".to_string()),
            ),
        ],
        br#"{"message":"Rate exceeded","__type":"ThrottledException"}"#,
    );
    let mut decoder = FrameDecoder::new();
    let frame = decoder
        .feed(&exception)
        .into_iter()
        .next()
        .unwrap()
        .unwrap();
    match frame_to_stream_event(&frame).unwrap() {
        Err(error) => {
            assert_eq!(error.class, ErrorClass::Quota);
            assert_eq!(error.message, "Rate exceeded");
        }
        _ => panic!("an exception frame is an error"),
    }

    let keepalive = encode_frame(
        &[Header::new(
            ":message-type",
            HeaderValue::Str("event".to_string()),
        )],
        b"",
    );
    let mut decoder = FrameDecoder::new();
    let frame = decoder
        .feed(&keepalive)
        .into_iter()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(
        frame_to_stream_event(&frame).unwrap().unwrap(),
        StreamEvent::Ping
    );

    let ping = encode_frame(
        &[
            Header::new(":message-type", HeaderValue::Str("event".to_string())),
            Header::new(":event-type", HeaderValue::Str("ping".to_string())),
        ],
        br#"{}"#,
    );
    let mut decoder = FrameDecoder::new();
    let frame = decoder.feed(&ping).into_iter().next().unwrap().unwrap();
    assert_eq!(
        frame_to_stream_event(&frame).unwrap().unwrap(),
        StreamEvent::Ping
    );
}

/// The exception-name mapping (`TEST-AI-021`): the name index decides before
/// the status; the status fallback covers nameless envelopes.
#[test]
fn exception_envelopes_map_to_classes() {
    let throttled = json!({ "__type": "ThrottledException", "message": "Rate exceeded" });
    assert_eq!(
        error_from_envelope(Some(429), &throttled).class,
        ErrorClass::Quota
    );
    let denied = json!({ "__type": "AccessDeniedException", "message": "User is not authorized" });
    assert_eq!(
        error_from_envelope(Some(403), &denied).class,
        ErrorClass::Auth
    );
    let not_found = json!({ "reason": "ResourceNotFoundException", "message": "model not found" });
    assert_eq!(
        error_from_envelope(Some(404), &not_found).class,
        ErrorClass::ModelNotFound
    );
    let wrapped = json!({ "Output": { "__type": "ValidationException", "message": "bad body" } });
    let error = error_from_envelope(Some(400), &wrapped);
    assert_eq!(error.class, ErrorClass::InvalidRequest);
    assert_eq!(error.message, "bad body");
    let unknown = json!({ "message": "Something else" });
    assert_eq!(
        error_from_envelope(Some(503), &unknown).class,
        ErrorClass::Transient
    );
    // The unknown-name fallback is status-driven, not guesswork.
    assert_eq!(
        error_from_envelope(Some(500), &json!({ "__type": "WeirdException" })).class,
        ErrorClass::Transient
    );
}

/// A stop reason that is not in the family grammar carries verbatim
/// (`AI-ADAPTER-08`; the `Other` arm), and content-policy spellings stay
/// distinguishable.
#[test]
fn stop_reasons_preserve_the_native_spelling() {
    assert_eq!(
        NativeStopReason::parse("end_turn"),
        NativeStopReason::EndTurn
    );
    assert_eq!(
        NativeStopReason::parse("pause_turn"),
        NativeStopReason::PauseTurn
    );
    assert_eq!(
        NativeStopReason::parse("guardrail_intervened"),
        NativeStopReason::Other("guardrail_intervened".to_string())
    );
    assert_eq!(
        NativeStopReason::parse("guardrail_intervened").wire(),
        "guardrail_intervened"
    );
}

/// The answer's `type`-ful content blocks carry one-to-one, and an unknown
/// block kind is a visible refusal, never a silent drop (`AI-MM-01`).
#[test]
fn unknown_block_kinds_refuse() {
    let response = json!({
        "content": [{ "type": "spotify_playlist", "tracks": [] }],
        "stop_reason": "end_turn",
        "usage": { "input_tokens": 1, "output_tokens": 1 }
    });
    assert!(BedrockAdapter::events_from_response(&response, &model("m")).is_err());
    let response = json!({
        "content": [{
            "type": "tool_use",
            "id": "toolu_9",
            "name": "get_weather",
            "input": json!({ "city": "Osaka" })
        }],
        "stop_reason": "tool_use",
        "usage": { "input_tokens": 3, "output_tokens": 5 }
    });
    let events = BedrockAdapter::events_from_response(&response, &model("m")).unwrap();
    assert!(
        events.contains(&myharness_provider_sdk::TurnEvent::ToolCall {
            id: "toolu_9".to_string(),
            name: "get_weather".to_string(),
            arguments_delta: None,
            arguments: Some(json!({ "city": "Osaka" })),
        })
    );
    let _ = config();
    let _ = Value::Null;
}
