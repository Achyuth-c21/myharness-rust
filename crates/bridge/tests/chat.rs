//! The Phase-4 chat bridge battery: profile→adapter resolution, the session
//! command surface, streaming, idempotent stop, crash recovery on open, usage
//! aggregation, and the error envelope (`docs/18` §4/§11; `TEST-SESSION-001`,
//! `-002`, `-008`; `TEST-DATA-004`).
//!
//! The adapter is scripted: provider wire behavior is pinned by each
//! provider's own battery. These tests ask whether the bridge composes the
//! session service, the agent loop, and the renderer contract without
//! inventing policy.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use futures::{stream, StreamExt};
use myharness_bridge::chat::{
    dispatch, ChatCommand, ChatResponse, ChatService, ChatTarget, ChatTargetResolver,
};
use myharness_bridge::setup::{BridgeError, ConfigBackend};
use myharness_config::{ProviderKind, Section};
use myharness_provider_sdk::{
    AuthContext, CountRequest, ErrorClass, ModelCapabilities, ModelDescriptor, ModelKey,
    NativeStopReason, NativeTurnRequest, ProtocolFamily, ProviderAdapter, ProviderError,
    ProviderHealth, TokenCount, TurnEvent, TurnStream, Usage,
};
use myharness_sessions::{InMemoryAuditKeys, SessionService};
use myharness_store::Store;
use secrecy::SecretString;
use serde_json::json;
use tempfile::TempDir;

// ---------------------------------------------------------------------
// Scripted target + adapter
// ---------------------------------------------------------------------

#[derive(Clone, Copy)]
enum Tail {
    End,
    Pending,
}

struct ScriptedTargets {
    script: Vec<Result<TurnEvent, ProviderError>>,
    tail: Tail,
    calls: Arc<Mutex<Vec<NativeTurnRequest>>>,
}

impl ChatTargetResolver for ScriptedTargets {
    fn resolve(&self) -> Result<ChatTarget, BridgeError> {
        Ok(ChatTarget {
            adapter: Box::new(ScriptedAdapter {
                script: self.script.clone(),
                tail: self.tail,
                calls: Arc::clone(&self.calls),
            }),
            auth: AuthContext::new(SecretString::from("stub-credential"), "test"),
            model: ModelKey {
                provider_id: "stub".into(),
                model_id: "stub-model".into(),
                variant: None,
                revision: None,
            },
            profile_id: "test-profile".into(),
        })
    }
}

struct ScriptedAdapter {
    script: Vec<Result<TurnEvent, ProviderError>>,
    tail: Tail,
    calls: Arc<Mutex<Vec<NativeTurnRequest>>>,
}

#[async_trait]
impl ProviderAdapter for ScriptedAdapter {
    fn id(&self) -> &str {
        "stub"
    }

    fn protocol(&self) -> ProtocolFamily {
        ProtocolFamily::AnthropicMessages
    }

    fn display_name(&self) -> &str {
        "Stub"
    }

    async fn authenticate(&self, _auth: &AuthContext) -> Result<(), ProviderError> {
        Ok(())
    }

    async fn list_models(
        &self,
        _auth: &AuthContext,
    ) -> Result<Vec<ModelDescriptor>, ProviderError> {
        Ok(Vec::new())
    }

    fn capabilities(&self, _model: &ModelKey, _descriptors: &[ModelDescriptor]) -> ModelCapabilities {
        ModelCapabilities::default()
    }

    async fn create_turn(
        &self,
        _auth: &AuthContext,
        request: &NativeTurnRequest,
    ) -> Result<TurnStream, ProviderError> {
        self.calls
            .lock()
            .expect("calls mutex")
            .push(request.clone());
        let head = stream::iter(self.script.clone());
        Ok(match self.tail {
            Tail::End => Box::pin(head),
            Tail::Pending => Box::pin(head.chain(stream::pending())),
        })
    }

    async fn count_tokens(
        &self,
        _auth: &AuthContext,
        _request: &CountRequest,
    ) -> Result<TokenCount, ProviderError> {
        Err(ProviderError {
            class: ErrorClass::Configuration,
            status: None,
            message: "the stub does not count".into(),
        })
    }

    async fn health_check(&self, _auth: &AuthContext) -> Result<ProviderHealth, ProviderError> {
        Ok(ProviderHealth {
            reachable: true,
            authenticated: Some(true),
            detail: None,
        })
    }
}

fn reply(text: &str) -> Vec<Result<TurnEvent, ProviderError>> {
    vec![
        Ok(TurnEvent::TextDelta { text: text.into() }),
        Ok(TurnEvent::Usage {
            usage: Usage {
                input: 7,
                output: 2,
                cache_read: Some(3),
                cache_write: None,
                web_search: None,
            },
        }),
        Ok(TurnEvent::Stop {
            reason: NativeStopReason::EndTurn,
        }),
        Ok(TurnEvent::Done),
    ]
}

// ---------------------------------------------------------------------
// Fixture
// ---------------------------------------------------------------------

struct Fixture {
    #[allow(dead_code)]
    dir: TempDir,
    service: Arc<ChatService<InMemoryAuditKeys, ScriptedTargets>>,
    calls: Arc<Mutex<Vec<NativeTurnRequest>>>,
}

impl Fixture {
    fn new(script: Vec<Result<TurnEvent, ProviderError>>, tail: Tail) -> Fixture {
        let dir = TempDir::new().expect("temp dir");
        let store = Store::open(&dir.path().join("chat.db"), &dir.path().join("files-root"))
            .expect("open store");
        let calls = Arc::new(Mutex::new(Vec::new()));
        let service = ChatService::new(
            SessionService::new(store, InMemoryAuditKeys::new()),
            ScriptedTargets {
                script,
                tail,
                calls: Arc::clone(&calls),
            },
        );
        Fixture {
            dir,
            service: Arc::new(service),
            calls,
        }
    }

    async fn create(&self) -> String {
        let response = dispatch(
            self.service.as_ref(),
            ChatCommand::CreateSession {
                title: Some("first".into()),
            },
            None,
        )
        .await;
        match response {
            ChatResponse::Session { session } => session.session.id,
            other => panic!("expected a session, got {other:?}"),
        }
    }

    async fn open(&self, id: &str) -> myharness_bridge::chat::SessionView {
        let response = dispatch(
            self.service.as_ref(),
            ChatCommand::OpenSession { id: id.into() },
            None,
        )
        .await;
        match response {
            ChatResponse::Session { session } => session,
            other => panic!("expected a session, got {other:?}"),
        }
    }

    fn calls(&self) -> Vec<NativeTurnRequest> {
        self.calls.lock().expect("calls mutex").clone()
    }
}

// ---------------------------------------------------------------------
// The command surface
// ---------------------------------------------------------------------

/// Create → send → open restores the transcript from SQLite; rename, archive,
/// and delete all go through the same chain-writing service.
#[tokio::test]
async fn create_send_and_open_restore_the_transcript() {
    let fixture = Fixture::new(reply("Hi there"), Tail::End);
    let session = fixture.create().await;

    let response = dispatch(
        fixture.service.as_ref(),
        ChatCommand::SendTurn {
            session_id: session.clone(),
            text: "hello".into(),
            max_tokens: None,
        },
        None,
    )
    .await;
    let ChatResponse::Turn {
        user_turn,
        assistant_turn,
        stop_reason,
        usage,
        ..
    } = response
    else {
        panic!("expected a turn, got {response:?}");
    };
    assert_eq!(user_turn.role, "user");
    assert_eq!(user_turn.content.plain_text(), "hello");
    assert_eq!(assistant_turn.status, "complete");
    assert_eq!(assistant_turn.content.plain_text(), "Hi there");
    assert_eq!(stop_reason.as_deref(), Some("end_turn"));
    assert_eq!(usage.as_ref().and_then(|v| v.get("input")), Some(&json!(7)));

    // The request replayed the stored user turn and carried the model verbatim.
    let calls = fixture.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].model.model_id, "stub-model");
    assert_eq!(calls[0].messages.len(), 1);

    let opened = fixture.open(&session).await;
    assert_eq!(opened.session.title, "first");
    assert_eq!(opened.session.model.as_deref(), Some("stub-model"));
    assert_eq!(opened.turns.len(), 2);
    assert_eq!(opened.turns[1].status, "complete");
    assert_eq!(opened.turns[1].stop_reason.as_deref(), Some("end_turn"));

    let list = dispatch(
        fixture.service.as_ref(),
        ChatCommand::ListSessions { archived: None },
        None,
    )
    .await;
    let ChatResponse::Sessions { sessions } = list else {
        panic!("expected sessions");
    };
    assert_eq!(sessions.len(), 1);

    let renamed = dispatch(
        fixture.service.as_ref(),
        ChatCommand::RenameSession {
            id: session.clone(),
            title: "renamed".into(),
        },
        None,
    )
    .await;
    assert!(matches!(renamed, ChatResponse::Ack));
    assert_eq!(fixture.open(&session).await.session.title, "renamed");

    assert!(matches!(
        dispatch(
            fixture.service.as_ref(),
            ChatCommand::ArchiveSession {
                id: session.clone(),
                archived: true,
            },
            None,
        )
        .await,
        ChatResponse::Ack
    ));
    let archived = dispatch(
        fixture.service.as_ref(),
        ChatCommand::ListSessions {
            archived: Some(true),
        },
        None,
    )
    .await;
    assert!(matches!(archived, ChatResponse::Sessions { sessions } if sessions.len() == 1));

    assert!(matches!(
        dispatch(
            fixture.service.as_ref(),
            ChatCommand::DeleteSession {
                id: session.clone(),
            },
            None,
        )
        .await,
        ChatResponse::Ack
    ));
    let gone = dispatch(
        fixture.service.as_ref(),
        ChatCommand::OpenSession { id: session },
        None,
    )
    .await;
    assert!(matches!(
        gone,
        ChatResponse::Error { error, .. } if error.code == "SESSION_ERROR"
    ));
}

/// Stop cancels the running turn; the partial reply is kept and stop stays
/// idempotent afterwards (`TEST-SESSION-001` stop clause).
#[tokio::test]
async fn stop_is_idempotent_and_keeps_the_partial_reply() {
    let fixture = Fixture::new(
        vec![Ok(TurnEvent::TextDelta {
            text: "par".into(),
        })],
        Tail::Pending,
    );
    let session = fixture.create().await;
    let service = Arc::clone(&fixture.service);

    let send = {
        let service = Arc::clone(&service);
        let session = session.clone();
        tokio::spawn(async move {
            dispatch(
                service.as_ref(),
                ChatCommand::SendTurn {
                    session_id: session,
                    text: "go".into(),
                    max_tokens: None,
                },
                None,
            )
            .await
        })
    };

    // Wait until the send has claimed the running slot.
    let mut stopped = false;
    for _ in 0..100 {
        tokio::task::yield_now().await;
        let response = dispatch(
            service.as_ref(),
            ChatCommand::StopTurn {
                session_id: session.clone(),
            },
            None,
        )
        .await;
        if let ChatResponse::Stopped { stopped: true } = response {
            stopped = true;
            break;
        }
    }
    assert!(stopped, "the running turn was never claimed");

    // A second stop before the task resolves is still an accepted no-op.
    let again = dispatch(
        service.as_ref(),
        ChatCommand::StopTurn {
            session_id: session.clone(),
        },
        None,
    )
    .await;
    assert!(matches!(again, ChatResponse::Stopped { stopped: true }));

    let outcome = send.await.expect("send task");
    assert!(matches!(
        outcome,
        ChatResponse::Error {
            error,
            user_turn_recorded: true
        } if error.code == "TURN_CANCELLED"
    ));

    let opened = fixture.open(&session).await;
    assert_eq!(opened.turns.len(), 2);
    assert_eq!(opened.turns[1].status, "partial");
    assert_eq!(opened.turns[1].content.plain_text(), "par");
    assert_eq!(
        opened.turns[1].error.as_deref(),
        Some("the turn was cancelled")
    );

    // Idle now: stop reports false, and stays false.
    for _ in 0..2 {
        let idle = dispatch(
            fixture.service.as_ref(),
            ChatCommand::StopTurn {
                session_id: session.clone(),
            },
            None,
        )
        .await;
        assert!(matches!(idle, ChatResponse::Stopped { stopped: false }));
    }
}

/// Opening a session recovers a live row from a dead process as an explicit
/// interruption (`TEST-SESSION-002`).
#[tokio::test]
async fn opening_a_session_recovers_an_interrupted_stream() {
    let fixture = Fixture::new(reply("never"), Tail::End);
    let session = fixture.create().await;
    fixture
        .service
        .sessions()
        .begin_assistant_turn(&session, 0, b"{\"parts\":[]}")
        .expect("live marker");

    let opened = fixture.open(&session).await;
    assert_eq!(opened.turns.len(), 1);
    assert_eq!(opened.turns[0].status, "error");
    assert_eq!(
        opened.turns[0].error.as_deref(),
        Some("the stream was interrupted by an app restart")
    );
    assert_eq!(
        fixture
            .service
            .sessions()
            .verify(&session)
            .expect("verify"),
        3
    );

    // Opening again does not append a second interruption row.
    let again = fixture.open(&session).await;
    assert_eq!(again.turns.len(), 1);
}

/// Usage aggregates the ledger and clamps the renderer's timezone input.
#[tokio::test]
async fn usage_summary_aggregates_the_ledger() {
    let fixture = Fixture::new(reply("counted"), Tail::End);
    let session = fixture.create().await;
    dispatch(
        fixture.service.as_ref(),
        ChatCommand::SendTurn {
            session_id: session,
            text: "count me".into(),
            max_tokens: None,
        },
        None,
    )
    .await;

    let response = dispatch(
        fixture.service.as_ref(),
        ChatCommand::UsageSummary {
            days: Some(7),
            utc_offset_minutes: Some(0),
        },
        None,
    )
    .await;
    let ChatResponse::Usage { summary } = response else {
        panic!("expected usage, got {response:?}");
    };
    assert_eq!(summary.days, 7);
    assert_eq!(summary.totals.events, 1);
    assert_eq!(summary.totals.input_tokens, 7);
    assert_eq!(summary.totals.output_tokens, 2);
    assert_eq!(summary.totals.cache_read_tokens, 3);

    let bad = dispatch(
        fixture.service.as_ref(),
        ChatCommand::UsageSummary {
            days: Some(7),
            utc_offset_minutes: Some(900),
        },
        None,
    )
    .await;
    assert!(matches!(
        bad,
        ChatResponse::Error { error, .. } if error.code == "CONFIG_VALIDATION"
    ));
}

/// An empty message is refused before any row is written, and the error says
/// the draft was not recorded.
#[tokio::test]
async fn an_empty_message_is_refused_before_persistence() {
    let fixture = Fixture::new(reply("never"), Tail::End);
    let session = fixture.create().await;
    let response = dispatch(
        fixture.service.as_ref(),
        ChatCommand::SendTurn {
            session_id: session.clone(),
            text: "   ".into(),
            max_tokens: None,
        },
        None,
    )
    .await;
    assert!(matches!(
        response,
        ChatResponse::Error {
            error,
            user_turn_recorded: false
        } if error.code == "CONFIG_VALIDATION"
    ));
    assert!(fixture.open(&session).await.turns.is_empty());
    assert!(fixture.calls().is_empty());
}

/// Deleting a held session is refused and the session survives.
#[tokio::test]
async fn delete_honors_the_legal_hold() {
    let fixture = Fixture::new(reply("never"), Tail::End);
    let session = fixture.create().await;
    fixture
        .service
        .sessions()
        .set_legal_hold(&session, true)
        .expect("hold");

    let response = dispatch(
        fixture.service.as_ref(),
        ChatCommand::DeleteSession {
            id: session.clone(),
        },
        None,
    )
    .await;
    let ChatResponse::Error { error, .. } = response else {
        panic!("expected an error");
    };
    assert_eq!(error.code, "SESSION_ERROR");
    assert!(error.message.contains("legal hold"), "{}", error.message);
    assert_eq!(fixture.open(&session).await.turns.len(), 0);
}

/// The command envelope uses the renderer's camelCase spellings.
#[test]
fn commands_round_trip_through_json() {
    let raw = serde_json::to_string(&ChatCommand::SendTurn {
        session_id: "abc".into(),
        text: "hi".into(),
        max_tokens: Some(64),
    })
    .unwrap();
    assert_eq!(
        raw,
        r#"{"type":"sendTurn","sessionId":"abc","text":"hi","maxTokens":64}"#
    );
    let back: ChatCommand = serde_json::from_str(&raw).unwrap();
    assert_eq!(
        back,
        ChatCommand::SendTurn {
            session_id: "abc".into(),
            text: "hi".into(),
            max_tokens: Some(64),
        }
    );
}

// ---------------------------------------------------------------------
// The profile → adapter factory
// ---------------------------------------------------------------------

fn gateway_backend() -> (TempDir, ConfigBackend, String) {
    let dir = TempDir::new().expect("temp dir");
    let backend = ConfigBackend::new(
        dir.path().join("config"),
        Box::new(myharness_secrets::MemoryStore::default()),
    );
    let id = backend
        .library()
        .create("Workspace", ProviderKind::Gateway)
        .expect("create profile")
        .id
        .expect("profile id");
    let mut document = backend.library().read(&id).expect("read profile");
    document.connection = Section(json!({
        "inferenceGatewayBaseUrl": "https://gateway.example",
        "inferenceGatewayApiKey": "sk-test-inline",
        "inferenceModels": ["claude-sonnet-4-5"],
    }));
    backend.library().write(&document).expect("write profile");
    backend.library().apply(&id).expect("apply profile");
    (dir, backend, id)
}

/// The applied gateway profile resolves to the native adapter and its model;
/// no profile, or no model, is a clear pre-send refusal.
#[test]
fn the_applied_profile_resolves_to_its_native_adapter() {
    let (_dir, backend, id) = gateway_backend();
    let target = backend.chat_target().expect("target");
    assert_eq!(target.adapter.id(), "gateway");
    assert_eq!(target.model.provider_id, "gateway");
    assert_eq!(target.model.model_id, "claude-sonnet-4-5");
    assert_eq!(target.profile_id, id);

    // No applied profile: refuse with the setup pointer.
    let empty = ConfigBackend::new(
        TempDir::new().unwrap().path().join("empty"),
        Box::new(myharness_secrets::MemoryStore::default()),
    );
    let error = empty.chat_target().err().expect("no profile is an error");
    assert_eq!(error.code, "CONFIG_MISSING");

    // Applied profile with an empty model list: refuse before a session exists.
    let mut document = backend.library().read(&id).expect("read profile");
    document.connection = Section(json!({
        "inferenceGatewayBaseUrl": "https://gateway.example",
        "inferenceGatewayApiKey": "sk-test-inline",
        "inferenceModels": [],
    }));
    backend.library().write(&document).expect("write profile");
    backend.library().apply(&id).expect("apply profile");
    let error = backend.chat_target().err().expect("no model is an error");
    assert_eq!(error.code, "CONFIG_VALIDATION");
    assert!(error.message.contains("names no model"), "{}", error.message);
}

/// A locked family is refused by name, never routed through another adapter.
#[test]
fn a_locked_family_is_refused() {
    let dir = TempDir::new().expect("temp dir");
    let backend = ConfigBackend::new(
        dir.path().join("config"),
        Box::new(myharness_secrets::MemoryStore::default()),
    );
    let id = backend
        .library()
        .create("Mantle", ProviderKind::BedrockMantle)
        .expect("create profile")
        .id
        .expect("profile id");
    let mut document = backend.library().read(&id).expect("read profile");
    document.connection = Section(json!({
        "inferenceBedrockMantleRegion": "us-east-1",
        "inferenceBedrockMantleApiKey": "sk-test-inline",
        "inferenceModels": ["some-model"],
    }));
    backend.library().write(&document).expect("write profile");
    // Apply may refuse a form the rebuild has not pinned; either way chat must
    // not resolve to an adapter. If apply passes, the factory refuses.
    let _ = backend.library().apply(&id);
    if let Ok(target) = backend.chat_target() {
        panic!("a locked family resolved to {}", target.adapter.id());
    }
}
