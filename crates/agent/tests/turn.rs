//! The slice-2 turn-loop battery: `TEST-SESSION-001`'s send/stream/stop
//! clauses against a scripted adapter, plus the replay, refusal, and ledger
//! rules the loop promises (`docs/12` §8.2; `docs/13` Phase 4).
//!
//! The adapter here is scripted on purpose: the native wire behavior of each
//! provider is pinned by that provider's own battery. These tests ask only
//! whether the agent layer records exactly what the stream delivered.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use futures::{stream, StreamExt};
use myharness_agent::{run_turn, AgentError, TurnContent, TurnInput, TurnRun};
use myharness_provider_sdk::{
    AuthContext, ContentPart, CountRequest, ErrorClass, ModelCapabilities, ModelDescriptor,
    ModelKey, NativeStopReason, NativeTurnRequest, ProtocolFamily, ProviderAdapter, ProviderError,
    ProviderHealth, Role, SystemBlock, TokenCount, ToolSpec, TurnEvent, TurnStream, Usage,
};
use myharness_sessions::{InMemoryAuditKeys, SessionService};
use myharness_store::{NewSession, Store, TurnRole, TurnRow, TurnStatus};
use secrecy::SecretString;
use tempfile::TempDir;
use tokio_util::sync::CancellationToken;

// ---------------------------------------------------------------------
// Fixture
// ---------------------------------------------------------------------

struct Fixture {
    #[allow(dead_code)]
    dir: TempDir,
    service: SessionService<InMemoryAuditKeys>,
}

impl Fixture {
    fn new() -> Fixture {
        let dir = TempDir::new().expect("temp dir");
        let store = Store::open(&dir.path().join("agent.db"), &dir.path().join("files-root"))
            .expect("open store");
        Fixture {
            dir,
            service: SessionService::new(store, InMemoryAuditKeys::new()),
        }
    }

    fn start(&self) -> String {
        self.service
            .start(NewSession {
                surface: "chat",
                title: "slice 2",
                ..Default::default()
            })
            .expect("start session")
            .id
    }
}

fn model(provider: &str, id: &str) -> ModelKey {
    ModelKey {
        provider_id: provider.into(),
        model_id: id.into(),
        variant: None,
        revision: None,
    }
}

fn auth() -> AuthContext {
    AuthContext::new(SecretString::from("stub-credential"), "scripted")
}

fn failure(class: ErrorClass, message: &str) -> ProviderError {
    ProviderError {
        class,
        status: None,
        message: message.into(),
    }
}

/// A minimal completed reply.
fn reply(text: &str) -> Vec<Result<TurnEvent, ProviderError>> {
    vec![
        Ok(TurnEvent::TextDelta { text: text.into() }),
        Ok(TurnEvent::Stop {
            reason: NativeStopReason::EndTurn,
        }),
        Ok(TurnEvent::Done),
    ]
}

fn stored_content(service: &SessionService<InMemoryAuditKeys>, turn: &TurnRow) -> TurnContent {
    let bytes = service
        .store()
        .files
        .read(&turn.content_ref)
        .expect("read blob")
        .expect("blob exists");
    serde_json::from_slice(&bytes).expect("transcript shape")
}

// ---------------------------------------------------------------------
// The scripted adapter
// ---------------------------------------------------------------------

#[derive(Clone, Copy)]
enum Tail {
    /// The stream ends when the script ends.
    End,
    /// The stream never ends after the script — only cancelling/dropping does.
    Pending,
}

struct StubAdapter {
    id: &'static str,
    script: Vec<Result<TurnEvent, ProviderError>>,
    tail: Tail,
    auth_error: Option<ProviderError>,
    /// Taken by the first `create_turn`, so a later run in the same test can
    /// succeed.
    create_error: Mutex<Option<ProviderError>>,
    /// Cancels the token from inside `create_turn`: the live turn exists, no
    /// delta has arrived yet, and the loop must still resolve the marker.
    cancel_on_create: Option<CancellationToken>,
    calls: Mutex<Vec<NativeTurnRequest>>,
}

impl StubAdapter {
    fn new(script: Vec<Result<TurnEvent, ProviderError>>) -> StubAdapter {
        StubAdapter {
            id: "stub",
            script,
            tail: Tail::End,
            auth_error: None,
            create_error: Mutex::new(None),
            cancel_on_create: None,
            calls: Mutex::new(Vec::new()),
        }
    }

    fn with_cancel_on_create(mut self, token: CancellationToken) -> StubAdapter {
        self.cancel_on_create = Some(token);
        self
    }

    fn with_tail(mut self, tail: Tail) -> StubAdapter {
        self.tail = tail;
        self
    }

    fn with_auth_error(mut self, error: ProviderError) -> StubAdapter {
        self.auth_error = Some(error);
        self
    }

    fn with_one_create_error(self, error: ProviderError) -> StubAdapter {
        *self.create_error.lock().expect("create error mutex") = Some(error);
        self
    }

    fn calls(&self) -> Vec<NativeTurnRequest> {
        self.calls.lock().expect("calls mutex").clone()
    }
}

#[async_trait]
impl ProviderAdapter for StubAdapter {
    fn id(&self) -> &str {
        self.id
    }

    fn protocol(&self) -> ProtocolFamily {
        ProtocolFamily::AnthropicMessages
    }

    fn display_name(&self) -> &str {
        "Stub"
    }

    async fn authenticate(&self, _auth: &AuthContext) -> Result<(), ProviderError> {
        match &self.auth_error {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
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
        if let Some(error) = self
            .create_error
            .lock()
            .expect("create error mutex")
            .take()
        {
            return Err(error);
        }
        if let Some(token) = &self.cancel_on_create {
            token.cancel();
        }
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
        Err(failure(ErrorClass::Configuration, "the stub does not count"))
    }

    async fn health_check(&self, _auth: &AuthContext) -> Result<ProviderHealth, ProviderError> {
        Ok(ProviderHealth {
            reachable: true,
            authenticated: Some(true),
            detail: None,
        })
    }
}

// ---------------------------------------------------------------------
// Send / stream / stop (`TEST-SESSION-001`)
// ---------------------------------------------------------------------

/// A plain reply streams in, lands as one complete assistant turn with the
/// native stop reason and the reported usage, and the chain grows by one link
/// per turn.
#[tokio::test]
async fn a_reply_streams_and_is_recorded_with_its_stop_reason_and_usage() {
    let fixture = Fixture::new();
    let session = fixture.start();
    let adapter = StubAdapter::new(vec![
        Ok(TurnEvent::TextDelta {
            text: "Hel".into(),
        }),
        Ok(TurnEvent::TextDelta {
            text: "lo".into(),
        }),
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
    ]);
    let cancel = CancellationToken::new();
    let auth = auth();

    let outcome = run_turn(TurnRun::new(
        &fixture.service,
        &adapter,
        &auth,
        &session,
        TurnInput::text(model("stub", "stub-model"), 64, "hi"),
        &cancel,
    ))
    .await
    .expect("turn runs");

    assert_eq!(outcome.user_turn.seq, 0);
    assert_eq!(outcome.assistant_turn.seq, 1);
    assert_eq!(outcome.assistant_turn.status, "complete");
    assert_eq!(outcome.assistant_turn.stop_reason.as_deref(), Some("end_turn"));
    assert_eq!(outcome.stop_reason, NativeStopReason::EndTurn);
    assert_eq!(outcome.usage.input, 7);
    assert_eq!(
        stored_content(&fixture.service, &outcome.assistant_turn).plain_text(),
        "Hello"
    );

    let usage = fixture.service.store().list_usage(&session).expect("usage");
    assert_eq!(usage.len(), 1);
    assert_eq!(usage[0].model, "stub-model");
    assert_eq!(usage[0].input_tokens, 7);
    assert_eq!(usage[0].output_tokens, 2);
    assert_eq!(usage[0].cache_read_tokens, 3);
    assert_eq!(usage[0].cache_write_tokens, 0);
    assert_eq!(usage[0].search_requests, 0);
    assert!(usage[0].error.is_none());

    // Genesis link + one link per turn.
    assert_eq!(fixture.service.verify(&session).expect("verify"), 4);
}

/// The transcript content is JSON in the agent's shape, and the turn's
/// `usage_json` keeps the exact `Option` shape the adapter reported (absent
/// cache channels are absent, not zero).
#[tokio::test]
async fn the_turn_keeps_the_exact_reported_usage_shape() {
    let fixture = Fixture::new();
    let session = fixture.start();
    let adapter = StubAdapter::new(vec![
        Ok(TurnEvent::Usage {
            usage: Usage {
                input: 5,
                output: 1,
                cache_read: None,
                cache_write: None,
                web_search: None,
            },
        }),
        Ok(TurnEvent::Stop {
            reason: NativeStopReason::EndTurn,
        }),
        Ok(TurnEvent::Done),
    ]);
    let cancel = CancellationToken::new();
    let auth = auth();

    let outcome = run_turn(TurnRun::new(
        &fixture.service,
        &adapter,
        &auth,
        &session,
        TurnInput::text(model("stub", "stub-model"), 64, "hi"),
        &cancel,
    ))
    .await
    .expect("turn runs");

    let raw = outcome
        .assistant_turn
        .usage_json
        .expect("the turn carries its usage");
    let usage: serde_json::Value = serde_json::from_str(&raw).expect("usage json");
    assert_eq!(usage["input"], 5);
    assert!(usage.get("cacheRead").is_none());

    let ledger = fixture.service.store().list_usage(&session).expect("usage");
    assert_eq!(ledger[0].cache_read_tokens, 0);
}

/// The next turn replays the stored conversation in order, carries the caller's
/// system blocks and budgets verbatim, and asks for a stream.
#[tokio::test]
async fn the_next_request_replays_the_transcript_and_carries_the_decisions() {
    let fixture = Fixture::new();
    let session = fixture.start();
    let adapter = StubAdapter::new(reply("Hi"));
    let cancel = CancellationToken::new();
    let auth = auth();
    let key = model("stub", "stub-model");

    run_turn(TurnRun::new(
        &fixture.service,
        &adapter,
        &auth,
        &session,
        TurnInput::text(key.clone(), 64, "hi"),
        &cancel,
    ))
    .await
    .expect("first turn");

    let system = vec![SystemBlock {
        text: "be brief".into(),
        cache_control: Some("ephemeral".into()),
    }];
    let mut input = TurnInput::text(key, 128, "again");
    input.system = system.clone();
    input.temperature = Some(0.4);
    run_turn(TurnRun::new(
        &fixture.service,
        &adapter,
        &auth,
        &session,
        input,
        &cancel,
    ))
    .await
    .expect("second turn");

    let calls = adapter.calls();
    assert_eq!(calls.len(), 2);
    let request = &calls[1];
    assert!(request.stream);
    assert_eq!(request.max_tokens, 128);
    assert_eq!(request.temperature, Some(0.4));
    assert!(request.tools.is_empty());
    assert_eq!(request.system, system);
    assert_eq!(
        request.messages,
        vec![
            (
                Role::User,
                vec![ContentPart::Text {
                    text: "hi".into()
                }]
            ),
            (
                Role::Assistant,
                vec![ContentPart::Text {
                    text: "Hi".into()
                }]
            ),
            (
                Role::User,
                vec![ContentPart::Text {
                    text: "again".into()
                }]
            ),
        ]
    );
}

/// A failed send leaves two user turns in a row; replay merges them into one
/// message instead of sending a sequence the Messages grammar rejects — and
/// nothing the user wrote is dropped.
#[tokio::test]
async fn consecutive_user_turns_are_merged_on_replay() {
    let fixture = Fixture::new();
    let session = fixture.start();
    let adapter = StubAdapter::new(reply("Hi"))
        .with_one_create_error(failure(ErrorClass::Auth, "the credential is empty"));
    let cancel = CancellationToken::new();
    let auth = auth();
    let key = model("stub", "stub-model");

    let refused = run_turn(TurnRun::new(
        &fixture.service,
        &adapter,
        &auth,
        &session,
        TurnInput::text(key.clone(), 64, "hi"),
        &cancel,
    ))
    .await;
    assert!(matches!(refused, Err(AgentError::Provider(_))));

    run_turn(TurnRun::new(
        &fixture.service,
        &adapter,
        &auth,
        &session,
        TurnInput::text(key, 64, "again"),
        &cancel,
    ))
    .await
    .expect("second turn");

    let calls = adapter.calls();
    assert_eq!(
        calls[1].messages,
        vec![(
            Role::User,
            vec![
                ContentPart::Text {
                    text: "hi".into()
                },
                ContentPart::Text {
                    text: "again".into()
                },
            ]
        )]
    );
}

// ---------------------------------------------------------------------
// Failures (`TEST-AI-021` wording, `TEST-SESSION-001` error state)
// ---------------------------------------------------------------------

/// A provider refusal is persisted next to the user's message with the
/// provider's own wording and returned with its class unchanged.
#[tokio::test]
async fn a_provider_refusal_is_recorded_and_returned_verbatim() {
    let fixture = Fixture::new();
    let session = fixture.start();
    let adapter = StubAdapter::new(Vec::new())
        .with_one_create_error(failure(ErrorClass::Auth, "the credential is empty"));
    let cancel = CancellationToken::new();
    let auth = auth();

    let result = run_turn(TurnRun::new(
        &fixture.service,
        &adapter,
        &auth,
        &session,
        TurnInput::text(model("stub", "stub-model"), 64, "hi"),
        &cancel,
    ))
    .await;

    match result {
        Err(AgentError::Provider(error)) => {
            assert_eq!(error.class, ErrorClass::Auth);
            assert_eq!(error.message, "the credential is empty");
        }
        other => panic!("expected the provider error, got {other:?}"),
    }

    let turns = fixture.service.store().list_turns(&session).expect("turns");
    assert_eq!(turns.len(), 2);
    assert_eq!(turns[0].role, "user");
    assert_eq!(turns[1].role, "assistant");
    assert_eq!(turns[1].status, "error");
    assert_eq!(turns[1].error.as_deref(), Some("the credential is empty"));
    assert_eq!(turns[1].stop_reason, None);

    let usage = fixture.service.store().list_usage(&session).expect("usage");
    assert_eq!(usage.len(), 1);
    assert_eq!(usage[0].error.as_deref(), Some("the credential is empty"));
    assert_eq!(usage[0].input_tokens, 0);
    assert_eq!(fixture.service.verify(&session).expect("verify"), 4);
}

/// An authentication failure is recorded without contacting the inference
/// endpoint — the adapter's shape check failed before a send.
#[tokio::test]
async fn an_authentication_failure_is_recorded_without_a_send() {
    let fixture = Fixture::new();
    let session = fixture.start();
    let adapter = StubAdapter::new(Vec::new())
        .with_auth_error(failure(ErrorClass::Configuration, "no credential is configured"));
    let cancel = CancellationToken::new();
    let auth = auth();

    let result = run_turn(TurnRun::new(
        &fixture.service,
        &adapter,
        &auth,
        &session,
        TurnInput::text(model("stub", "stub-model"), 64, "hi"),
        &cancel,
    ))
    .await;

    assert!(matches!(result, Err(AgentError::Provider(_))));
    assert!(adapter.calls().is_empty());
    let turns = fixture.service.store().list_turns(&session).expect("turns");
    assert_eq!(turns.len(), 2);
    assert_eq!(
        turns[1].error.as_deref(),
        Some("no credential is configured")
    );
}

/// An error event mid-stream keeps what arrived before it: the turn is `error`
/// with the partial content, not an empty turn.
#[tokio::test]
async fn a_midstream_error_keeps_what_arrived() {
    let fixture = Fixture::new();
    let session = fixture.start();
    let adapter = StubAdapter::new(vec![
        Ok(TurnEvent::TextDelta {
            text: "par".into(),
        }),
        Ok(TurnEvent::Error {
            error: failure(ErrorClass::Transient, "provider overloaded"),
        }),
    ]);
    let cancel = CancellationToken::new();
    let auth = auth();

    let result = run_turn(TurnRun::new(
        &fixture.service,
        &adapter,
        &auth,
        &session,
        TurnInput::text(model("stub", "stub-model"), 64, "hi"),
        &cancel,
    ))
    .await;
    assert!(matches!(result, Err(AgentError::Provider(_))));

    let turns = fixture.service.store().list_turns(&session).expect("turns");
    assert_eq!(turns[1].status, "error");
    assert_eq!(
        stored_content(&fixture.service, &turns[1]).plain_text(),
        "par"
    );
    assert_eq!(turns[1].error.as_deref(), Some("provider overloaded"));
    let usage = fixture.service.store().list_usage(&session).expect("usage");
    assert_eq!(usage[0].error.as_deref(), Some("provider overloaded"));
}

/// A stream that ends without ever reporting a stop is a contract break: the
/// turn is an error, never a success, and the delivered content is kept.
#[tokio::test]
async fn a_stream_without_a_stop_reason_is_a_protocol_error() {
    let fixture = Fixture::new();
    let session = fixture.start();
    let adapter = StubAdapter::new(vec![
        Ok(TurnEvent::TextDelta {
            text: "half".into(),
        }),
        Ok(TurnEvent::Done),
    ]);
    let cancel = CancellationToken::new();
    let auth = auth();

    let result = run_turn(TurnRun::new(
        &fixture.service,
        &adapter,
        &auth,
        &session,
        TurnInput::text(model("stub", "stub-model"), 64, "hi"),
        &cancel,
    ))
    .await;
    assert!(matches!(result, Err(AgentError::Protocol(_))));

    let turns = fixture.service.store().list_turns(&session).expect("turns");
    assert_eq!(turns[1].status, "error");
    assert_eq!(
        stored_content(&fixture.service, &turns[1]).plain_text(),
        "half"
    );
    assert_eq!(turns[1].error.as_deref(), Some("the provider stream ended without a stop reason"));
}

// ---------------------------------------------------------------------
// Stop (`TEST-SESSION-001` stop clause, `TEST-AI-015`)
// ---------------------------------------------------------------------

/// Stopping mid-stream leaves the delivered content as a `partial` turn and a
/// usage event marked cancelled; the drop is what cancels the request.
#[tokio::test]
async fn cancelling_leaves_a_partial_turn_with_what_arrived() {
    let fixture = Fixture::new();
    let session = fixture.start();
    let adapter = StubAdapter::new(vec![Ok(TurnEvent::TextDelta {
        text: "par".into(),
    })])
    .with_tail(Tail::Pending);
    let cancel = CancellationToken::new();
    let tap = |event: &TurnEvent| {
        if matches!(event, TurnEvent::TextDelta { .. }) {
            cancel.cancel();
        }
    };
    let auth = auth();

    let result = run_turn(
        TurnRun::new(
            &fixture.service,
            &adapter,
            &auth,
            &session,
            TurnInput::text(model("stub", "stub-model"), 64, "hi"),
            &cancel,
        )
        .with_observer(&tap),
    )
    .await;
    assert!(matches!(result, Err(AgentError::Cancelled)));

    let turns = fixture.service.store().list_turns(&session).expect("turns");
    assert_eq!(turns.len(), 2);
    assert_eq!(turns[1].status, "partial");
    assert_eq!(
        stored_content(&fixture.service, &turns[1]).plain_text(),
        "par"
    );
    assert_eq!(turns[1].error.as_deref(), Some("the turn was cancelled"));
    assert_eq!(turns[1].stop_reason, None);

    let usage = fixture.service.store().list_usage(&session).expect("usage");
    assert_eq!(usage.len(), 1);
    assert_eq!(usage[0].error.as_deref(), Some("the turn was cancelled"));
}

/// A turn stopped before the send leaves no trace at all — there was no
/// attempt to record, and no provider contact.
#[tokio::test]
async fn a_pre_cancelled_turn_writes_nothing_and_sends_nothing() {
    let fixture = Fixture::new();
    let session = fixture.start();
    let adapter = StubAdapter::new(reply("never sent"));
    let cancel = CancellationToken::new();
    cancel.cancel();
    let auth = auth();

    let result = run_turn(TurnRun::new(
        &fixture.service,
        &adapter,
        &auth,
        &session,
        TurnInput::text(model("stub", "stub-model"), 64, "hi"),
        &cancel,
    ))
    .await;

    assert!(matches!(result, Err(AgentError::CancelledBeforeSend)));
    assert!(adapter.calls().is_empty());
    assert!(fixture
        .service
        .store()
        .list_turns(&session)
        .expect("turns")
        .is_empty());
    assert!(fixture
        .service
        .store()
        .list_usage(&session)
        .expect("usage")
        .is_empty());

    // Stop is idempotent: a second attempt on the same cancelled token still
    // writes nothing and still does not contact the provider.
    let again = run_turn(TurnRun::new(
        &fixture.service,
        &adapter,
        &auth,
        &session,
        TurnInput::text(model("stub", "stub-model"), 64, "hi"),
        &cancel,
    ))
    .await;
    assert!(matches!(again, Err(AgentError::CancelledBeforeSend)));
    assert!(adapter.calls().is_empty());
    assert!(fixture
        .service
        .store()
        .list_turns(&session)
        .expect("turns")
        .is_empty());
}

/// A stop that lands after the live turn is opened but before the first delta
/// still resolves that turn as an empty `partial`: recovery must not later
/// mistake it for a crash.
#[tokio::test]
async fn a_cancel_before_the_first_delta_still_resolves_the_live_turn() {
    let fixture = Fixture::new();
    let session = fixture.start();
    let cancel = CancellationToken::new();
    let adapter = StubAdapter::new(Vec::new())
        .with_tail(Tail::Pending)
        .with_cancel_on_create(cancel.clone());
    let auth = auth();

    let result = run_turn(TurnRun::new(
        &fixture.service,
        &adapter,
        &auth,
        &session,
        TurnInput::text(model("stub", "stub-model"), 64, "hi"),
        &cancel,
    ))
    .await;
    assert!(matches!(result, Err(AgentError::Cancelled)));

    let turns = fixture.service.store().list_turns(&session).expect("turns");
    assert_eq!(turns.len(), 2);
    assert_eq!(turns[1].status, "partial");
    assert!(stored_content(&fixture.service, &turns[1]).parts.is_empty());
    assert_eq!(turns[1].error.as_deref(), Some("the turn was cancelled"));

    // Recovery has nothing to do: the marker was resolved by the cancellation.
    assert_eq!(
        fixture
            .service
            .recover_interrupted(&session)
            .expect("recover"),
        0
    );
}

/// A live assistant row left by a previous process is an interruption, not a
/// reply: the next send names it, replays only the real transcript, and never
/// duplicates the attempt (`TEST-SESSION-002`).
#[tokio::test]
async fn a_crashed_stream_is_marked_interrupted_and_not_replayed() {
    let dir = TempDir::new().expect("temp dir");
    let keys = Arc::new(InMemoryAuditKeys::new());
    let db = dir.path().join("agent.db");
    let files = dir.path().join("files-root");
    let session;
    {
        let store = Store::open(&db, &files).expect("open store");
        let service = SessionService::new(store, Arc::clone(&keys));
        session = service
            .start(NewSession {
                surface: "chat",
                title: "crash",
                ..Default::default()
            })
            .expect("start")
            .id;
        service
            .append_turn(
                &session,
                0,
                TurnRole::User,
                TurnStatus::Complete,
                &serde_json::to_vec(&TurnContent::text("before the crash")).unwrap(),
                None,
                None,
                None,
            )
            .expect("user turn");
        service
            .begin_assistant_turn(
                &session,
                1,
                &serde_json::to_vec(&TurnContent::default()).unwrap(),
            )
            .expect("live marker");
        // The process "dies" here: no finish, store dropped.
    }

    let store = Store::open(&db, &files).expect("reopen store");
    let service = SessionService::new(store, Arc::clone(&keys));
    let adapter = StubAdapter::new(reply("after the restart"));
    let cancel = CancellationToken::new();
    let auth = auth();

    let outcome = run_turn(TurnRun::new(
        &service,
        &adapter,
        &auth,
        &session,
        TurnInput::text(model("stub", "stub-model"), 64, "again"),
        &cancel,
    ))
    .await
    .expect("turn after recovery");

    let turns = service.store().list_turns(&session).expect("turns");
    assert_eq!(turns.len(), 4);
    assert_eq!(turns[1].status, "error");
    assert_eq!(
        turns[1].error.as_deref(),
        Some("the stream was interrupted by an app restart")
    );
    assert_eq!(outcome.user_turn.seq, 2);
    assert_eq!(outcome.assistant_turn.seq, 3);

    // The interrupted turn carried no content, so replay is the two user
    // messages merged into one role run — nothing fabricated.
    let calls = adapter.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].messages.len(), 1);
    assert_eq!(calls[0].messages[0].0, Role::User);
    assert_eq!(calls[0].messages[0].1.len(), 2);

    // Idempotent: a second recovery finds no live turn and adds no row.
    assert_eq!(service.recover_interrupted(&session).expect("recover"), 0);
    assert_eq!(service.store().list_turns(&session).expect("turns").len(), 4);
    // created, user, started, interrupted finish, user, started, finish
    assert_eq!(service.verify(&session).expect("verify"), 7);
}

// ---------------------------------------------------------------------
// Refusals and the content model
// ---------------------------------------------------------------------

/// Tool declarations are refused before any contact, not silently dropped
/// (Phase 5 owns execution).
#[tokio::test]
async fn tool_declarations_are_refused_before_contact() {
    let fixture = Fixture::new();
    let session = fixture.start();
    let adapter = StubAdapter::new(reply("unreachable"));
    let cancel = CancellationToken::new();
    let auth = auth();
    let mut input = TurnInput::text(model("stub", "stub-model"), 64, "hi");
    input.tools = vec![ToolSpec {
        name: "read_file".into(),
        description: None,
        input_schema: serde_json::json!({ "type": "object" }),
        cache_control: None,
    }];

    let result = run_turn(TurnRun::new(
        &fixture.service,
        &adapter,
        &auth,
        &session,
        input,
        &cancel,
    ))
    .await;

    assert!(matches!(result, Err(AgentError::Unsupported(_))));
    assert!(adapter.calls().is_empty());
    assert!(fixture
        .service
        .store()
        .list_turns(&session)
        .expect("turns")
        .is_empty());
}

/// A model whose provider does not name the adapter is refused: no silent
/// cross-provider send (`AI-MODEL-01`).
#[tokio::test]
async fn a_model_for_another_provider_is_refused() {
    let fixture = Fixture::new();
    let session = fixture.start();
    let adapter = StubAdapter::new(reply("unreachable"));
    let cancel = CancellationToken::new();
    let auth = auth();

    let result = run_turn(TurnRun::new(
        &fixture.service,
        &adapter,
        &auth,
        &session,
        TurnInput::text(model("gateway", "claude-x"), 64, "hi"),
        &cancel,
    ))
    .await;

    assert!(matches!(result, Err(AgentError::ProviderMismatch { .. })));
    assert!(adapter.calls().is_empty());
}

/// Reasoning blocks keep their signature in order, and a citation is kept in
/// the record while staying out of the replayed request.
#[tokio::test]
async fn reasoning_signatures_and_citations_survive_the_record() {
    let fixture = Fixture::new();
    let session = fixture.start();
    let citation = serde_json::json!({ "cited_text": "source", "document_index": 0 });
    let adapter = StubAdapter::new(vec![
        Ok(TurnEvent::ReasoningDelta {
            text: "think ".into(),
            signature: None,
        }),
        Ok(TurnEvent::ReasoningDelta {
            text: "more".into(),
            signature: Some("sig-1".into()),
        }),
        Ok(TurnEvent::TextDelta {
            text: "answer".into(),
        }),
        Ok(TurnEvent::Citation {
            citation: citation.clone(),
        }),
        Ok(TurnEvent::Stop {
            reason: NativeStopReason::EndTurn,
        }),
        Ok(TurnEvent::Done),
    ]);
    let cancel = CancellationToken::new();
    let auth = auth();
    let key = model("stub", "stub-model");

    let outcome = run_turn(TurnRun::new(
        &fixture.service,
        &adapter,
        &auth,
        &session,
        TurnInput::text(key.clone(), 64, "hi"),
        &cancel,
    ))
    .await
    .expect("turn runs");

    let content = stored_content(&fixture.service, &outcome.assistant_turn);
    assert_eq!(
        content.parts,
        vec![
            ContentPart::Reasoning {
                thinking: "think more".into(),
                signature: Some("sig-1".into()),
            },
            ContentPart::Text {
                text: "answer".into()
            },
        ]
    );
    assert_eq!(content.citations, vec![citation]);

    run_turn(TurnRun::new(
        &fixture.service,
        &adapter,
        &auth,
        &session,
        TurnInput::text(key, 64, "again"),
        &cancel,
    ))
    .await
    .expect("second turn");

    let calls = adapter.calls();
    assert_eq!(
        calls[1].messages[1],
        (
            Role::Assistant,
            vec![
                ContentPart::Reasoning {
                    thinking: "think more".into(),
                    signature: Some("sig-1".into()),
                },
                ContentPart::Text {
                    text: "answer".into()
                },
            ]
        )
    );
}
