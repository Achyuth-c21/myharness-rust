//! The Phase-5 bridge tool-turn battery: session tool selection, a frozen
//! snapshot, the agent loop's tool round, provider replay, persistence, and
//! the renderer response (`TEST-MCP-002`, `TEST-MCP-006`, `TEST-AI-019`).
//!
//! The MCP server and the provider are both scripted. The wire behavior of
//! each is pinned by its own battery; this test asks whether the bridge
//! composes them without inventing policy or shrinking a failed selection.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use futures::stream;
use myharness_agent::tool_policy::{PolicyEffect, ToolPolicy};
use myharness_agent::PermissionMode;
use myharness_bridge::chat::{
    dispatch, ChatCommand, ChatResponse, ChatService, ChatTarget, ChatTargetResolver,
};
use myharness_bridge::mcp::{BridgeApprovalPrompt, BuiltMcp, McpConfig, ToolRuntime};
use myharness_bridge::setup::BridgeError;
use myharness_mcp::{
    McpConnector, McpError, McpManager, McpResult, McpSession, ServerConfig, ServerInfoSummary,
    ToolCallResult, ToolDefinition,
};
use myharness_provider_sdk::{
    AuthContext, ContentPart, CountRequest, ErrorClass, ModelCapabilities, ModelDescriptor,
    ModelKey, NativeStopReason, NativeTurnRequest, ProtocolFamily, ProviderAdapter, ProviderError,
    ProviderHealth, Role, TokenCount, TurnEvent, TurnStream, Usage,
};
use myharness_sessions::{InMemoryAuditKeys, SessionService};
use myharness_store::Store;
use serde_json::{json, Value};
use tempfile::TempDir;

// ---------------------------------------------------------------------
// Scripted MCP server
// ---------------------------------------------------------------------

struct MockSession;

#[async_trait]
impl McpSession for MockSession {
    fn server_info(&self) -> ServerInfoSummary {
        ServerInfoSummary::unknown()
    }

    async fn list_tools(&self) -> McpResult<Vec<ToolDefinition>> {
        Ok(vec![ToolDefinition {
            name: "greet".into(),
            title: Some("Greet".into()),
            description: Some("Greet a person.".into()),
            input_schema: json!({
                "type": "object",
                "properties": {"name": {"type": "string"}},
                "required": ["name"]
            }),
            output_schema: None,
        }])
    }

    async fn call_tool(&self, tool: &str, arguments: Value) -> McpResult<ToolCallResult> {
        if tool != "greet" {
            return Err(McpError::Protocol(format!("unknown tool {tool}")));
        }
        let name = arguments
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("nobody");
        Ok(ToolCallResult::text(format!("hello {name}")))
    }

    fn is_closed(&self) -> bool {
        false
    }

    async fn shutdown(&mut self) -> McpResult<()> {
        Ok(())
    }
}

struct MockConnector;

#[async_trait]
impl McpConnector for MockConnector {
    async fn connect(
        &self,
        _server: &str,
        _config: &ServerConfig,
    ) -> McpResult<Box<dyn McpSession>> {
        Ok(Box::new(MockSession))
    }
}

// ---------------------------------------------------------------------
// Scripted provider
// ---------------------------------------------------------------------

type Rounds = Arc<Mutex<VecDeque<Vec<Result<TurnEvent, ProviderError>>>>>;

struct ScriptedAdapter {
    rounds: Rounds,
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

    fn capabilities(
        &self,
        _model: &ModelKey,
        _descriptors: &[ModelDescriptor],
    ) -> ModelCapabilities {
        ModelCapabilities {
            tools_supported: true,
            ..ModelCapabilities::default()
        }
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
        let round = self
            .rounds
            .lock()
            .expect("rounds mutex")
            .pop_front()
            .unwrap_or_else(|| {
                vec![
                    Err(ProviderError {
                        class: ErrorClass::Transient,
                        status: None,
                        message: "the scripted provider ran out of rounds".into(),
                    }),
                    Ok(TurnEvent::Done),
                ]
            });
        Ok(Box::pin(stream::iter(round)))
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

struct ScriptedTargets {
    rounds: Rounds,
    calls: Arc<Mutex<Vec<NativeTurnRequest>>>,
}

impl ChatTargetResolver for ScriptedTargets {
    fn resolve(&self) -> Result<ChatTarget, BridgeError> {
        Ok(ChatTarget {
            adapter: Box::new(ScriptedAdapter {
                rounds: Arc::clone(&self.rounds),
                calls: Arc::clone(&self.calls),
            }),
            auth: AuthContext::new(secrecy::SecretString::from("test-credential"), "test"),
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

// ---------------------------------------------------------------------
// Fixture
// ---------------------------------------------------------------------

struct Fixture {
    #[allow(dead_code)]
    dir: TempDir,
    service: Arc<ChatService<InMemoryAuditKeys, ScriptedTargets>>,
    calls: Arc<Mutex<Vec<NativeTurnRequest>>>,
    runtime: Arc<ToolRuntime>,
}

impl Fixture {
    async fn new() -> Fixture {
        Fixture::new_with(
            ToolPolicy {
                default: PolicyEffect::Allow,
                rules: Vec::new(),
            },
            Arc::new(BridgeApprovalPrompt::new(Arc::new(|_| {}))),
        )
        .await
    }

    async fn new_with(policy: ToolPolicy, prompt: Arc<BridgeApprovalPrompt>) -> Fixture {
        Fixture::new_with_config(policy, BTreeSet::new(), prompt).await
    }

    async fn new_with_config(
        policy: ToolPolicy,
        disabled: BTreeSet<String>,
        prompt: Arc<BridgeApprovalPrompt>,
    ) -> Fixture {
        let dir = TempDir::new().expect("temp dir");
        let store = Store::open(&dir.path().join("chat.db"), &dir.path().join("files-root"))
            .expect("open store");
        let calls = Arc::new(Mutex::new(Vec::new()));

        let mut manager = McpManager::with_connector(Arc::new(MockConnector));
        manager
            .add_server(
                "echo",
                ServerConfig::Stdio {
                    command: "mock".into(),
                    args: Vec::new(),
                    env: BTreeMap::new(),
                },
            )
            .unwrap();
        manager.start("echo").await.unwrap();
        let built = BuiltMcp {
            manager: Arc::new(tokio::sync::Mutex::new(manager)),
            config: McpConfig {
                policy,
                disabled_builtin_tools: disabled,
                ..McpConfig::default()
            },
            warnings: Vec::new(),
        };
        let runtime = Arc::new(ToolRuntime::new(built, prompt, None, None));

        let rounds = Arc::new(Mutex::new(VecDeque::from([
            vec![
                Ok(TurnEvent::ToolCall {
                    id: "call-1".into(),
                    name: myharness_agent::wire_tool_name("echo:greet"),
                    arguments_delta: None,
                    arguments: Some(json!({"name": "Ada"})),
                }),
                Ok(TurnEvent::Stop {
                    reason: NativeStopReason::ToolUse,
                }),
                Ok(TurnEvent::Done),
            ],
            vec![
                Ok(TurnEvent::TextDelta {
                    text: "hello Ada".into(),
                }),
                Ok(TurnEvent::Usage {
                    usage: Usage {
                        input: 11,
                        output: 3,
                        cache_read: None,
                        cache_write: None,
                        web_search: None,
                    },
                }),
                Ok(TurnEvent::Stop {
                    reason: NativeStopReason::EndTurn,
                }),
                Ok(TurnEvent::Done),
            ],
        ])));
        let service = ChatService::new(
            SessionService::new(store, InMemoryAuditKeys::new()),
            ScriptedTargets {
                rounds,
                calls: Arc::clone(&calls),
            },
        )
        .with_tools(Arc::clone(&runtime));
        Fixture {
            dir,
            service: Arc::new(service),
            calls,
            runtime,
        }
    }

    async fn create(&self) -> String {
        let response = dispatch(
            self.service.as_ref(),
            ChatCommand::CreateSession {
                title: Some("tools".into()),
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
}

#[tokio::test]
async fn plan_mode_does_not_require_a_live_selected_mcp_server() {
    let fixture = Fixture::new().await;
    let folder = fixture.dir.path().join("plan-repo");
    std::fs::create_dir_all(&folder).expect("folder");
    let cwd = std::fs::canonicalize(&folder).expect("canonical");
    let created = dispatch(
        fixture.service.as_ref(),
        ChatCommand::CreateCodeSession {
            title: Some("plan".into()),
            cwd: cwd.to_str().expect("utf-8").into(),
            permission_mode: PermissionMode::Plan,
        },
        None,
    )
    .await;
    let ChatResponse::Session { session } = created else {
        panic!("expected a Code session, got {created:?}");
    };
    let session = session.session.id;

    let selected = dispatch(
        fixture.service.as_ref(),
        ChatCommand::SetSessionTools {
            session_id: session.clone(),
            tools: vec!["echo:greet".into()],
        },
        None,
    )
    .await;
    assert!(matches!(selected, ChatResponse::Ack));
    // Remove the cached snapshot and the server. Plan must not resolve either:
    // it advertises no tools, and a selected-but-offline server cannot block it.
    fixture.runtime.clear_session(&session).await;
    fixture
        .runtime
        .manager()
        .lock()
        .await
        .remove_server("echo")
        .await
        .expect("remove server");

    let response = dispatch(
        fixture.service.as_ref(),
        ChatCommand::SendCodeTurn {
            session_id: session,
            text: "plan only".into(),
            max_tokens: Some(64),
        },
        None,
    )
    .await;
    let ChatResponse::ToolTurn { tool_turns, .. } = response else {
        panic!("expected the Plan turn to complete, got {response:?}");
    };
    // The scripted provider ignores the empty tool list and emits a call; the
    // agent refuses it as undeclared and persists a tool error, never executes.
    assert_eq!(tool_turns.len(), 1);
    let calls = fixture.calls.lock().expect("calls mutex").clone();
    assert_eq!(calls.len(), 2);
    assert!(calls[0].tools.is_empty());
}

#[tokio::test]
async fn a_selected_tool_runs_once_and_its_result_is_replayed_to_the_provider() {
    let fixture = Fixture::new().await;
    let session = fixture.create().await;

    let listed = dispatch(fixture.service.as_ref(), ChatCommand::McpTools, None).await;
    let ChatResponse::McpTools { tools } = listed else {
        panic!("expected live tools, got {listed:?}");
    };
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "echo:greet");

    let selected = dispatch(
        fixture.service.as_ref(),
        ChatCommand::SetSessionTools {
            session_id: session.clone(),
            tools: vec!["echo:greet".into()],
        },
        None,
    )
    .await;
    assert!(matches!(selected, ChatResponse::Ack));

    let response = dispatch(
        fixture.service.as_ref(),
        ChatCommand::SendTurn {
            session_id: session.clone(),
            text: "greet Ada".into(),
            max_tokens: Some(64),
        },
        None,
    )
    .await;
    let ChatResponse::ToolTurn {
        assistant_turns,
        tool_turns,
        stop_reason,
        usage,
        ..
    } = response
    else {
        panic!("expected a tool turn, got {response:?}");
    };
    assert_eq!(assistant_turns.len(), 2);
    assert_eq!(tool_turns.len(), 1);
    assert_eq!(stop_reason.as_deref(), Some("end_turn"));
    assert_eq!(
        usage.as_ref().and_then(|value| value.get("input")),
        Some(&json!(11))
    );

    let calls = fixture.calls.lock().expect("calls mutex").clone();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].tools.len(), 1);
    assert_eq!(
        calls[0].tools[0].name,
        myharness_agent::wire_tool_name("echo:greet")
    );
    // Round two replays the assistant tool_use and the tool result as a
    // provider `tool` message. The result is data, not a model instruction.
    assert!(calls[1].messages.iter().any(|(role, parts)| {
        *role == Role::User
            && parts.iter().any(|part| {
                matches!(
                    part,
                    ContentPart::ToolResult { tool_use_id, content, is_error, .. }
                        if tool_use_id == "call-1"
                            && !is_error
                            && content.iter().any(|part| matches!(
                                part,
                                ContentPart::Text { text } if text == "hello Ada"
                            ))
                )
            })
    }));

    let opened = fixture.open(&session).await;
    assert_eq!(opened.turns.len(), 4);
    assert_eq!(opened.turns[0].role, "user");
    assert_eq!(opened.turns[1].role, "assistant");
    assert_eq!(opened.turns[2].role, "tool");
    assert_eq!(opened.turns[3].role, "assistant");
    assert_eq!(opened.turns[3].status, "complete");

    // The persisted selection survives a fresh open and is visible to the
    // renderer's picker.
    let extra = &opened.session.extra;
    assert_eq!(extra["mcpTools"], json!(["echo:greet"]));
}

#[tokio::test]
async fn a_missing_tool_fails_before_the_selection_is_stored_or_sent() {
    let fixture = Fixture::new().await;
    let session = fixture.create().await;
    let response = dispatch(
        fixture.service.as_ref(),
        ChatCommand::SetSessionTools {
            session_id: session.clone(),
            tools: vec!["echo:missing".into()],
        },
        None,
    )
    .await;
    let ChatResponse::Error { error, .. } = response else {
        panic!("expected a typed failure, got {response:?}");
    };
    assert_eq!(error.code, "MCP_TOOLS_UNAVAILABLE");
    assert!(error.message.contains("echo:missing"));
    assert!(fixture.calls.lock().expect("calls mutex").is_empty());

    let opened = fixture.open(&session).await;
    assert!(opened.session.extra.get("mcpTools").is_none());
}

#[tokio::test]
async fn an_ask_policy_prompts_once_and_a_once_grant_is_not_persisted() {
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    let prompt = Arc::new(BridgeApprovalPrompt::new(Arc::new(move |event| {
        let _ = sender.send(event);
    })));
    let fixture = Fixture::new_with(ToolPolicy::default(), Arc::clone(&prompt)).await;
    let session = fixture.create().await;
    let selected = dispatch(
        fixture.service.as_ref(),
        ChatCommand::SetSessionTools {
            session_id: session.clone(),
            tools: vec!["echo:greet".into()],
        },
        None,
    )
    .await;
    assert!(matches!(selected, ChatResponse::Ack));

    let responder = {
        let prompt = Arc::clone(&prompt);
        tokio::spawn(async move {
            let event = receiver.recv().await.expect("approval event");
            assert!(!event.session_id.is_empty());
            assert_eq!(event.tool_name, "echo:greet");
            assert_eq!(event.arguments_preview["name"], "Ada");
            assert_eq!(
                event.allowed_scopes,
                vec![
                    myharness_agent::tool_policy::ApprovalScope::Once,
                    myharness_agent::tool_policy::ApprovalScope::Session
                ]
            );
            prompt
                .respond(
                    &event.id,
                    myharness_bridge::mcp::ApprovalDecision::Allow,
                    Some(myharness_agent::tool_policy::ApprovalScope::Once),
                )
                .unwrap();
        })
    };
    // The event's session id is the real one; assert it in the main task by
    // reading the session after the turn instead of guessing in the responder.
    let response = dispatch(
        fixture.service.as_ref(),
        ChatCommand::SendTurn {
            session_id: session.clone(),
            text: "greet Ada".into(),
            max_tokens: Some(64),
        },
        None,
    )
    .await;
    responder.await.unwrap();
    let ChatResponse::ToolTurn { tool_turns, .. } = response else {
        panic!("expected a tool turn, got {response:?}");
    };
    assert_eq!(tool_turns.len(), 1);
    let opened = fixture.open(&session).await;
    assert!(opened.session.extra["mcpApprovals"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn a_disabled_tool_is_never_offered_or_selectable() {
    let fixture = Fixture::new_with_config(
        ToolPolicy {
            default: PolicyEffect::Allow,
            rules: Vec::new(),
        },
        BTreeSet::from(["echo:*".to_string()]),
        Arc::new(BridgeApprovalPrompt::new(Arc::new(|_| {}))),
    )
    .await;
    let session = fixture.create().await;

    let listed = dispatch(fixture.service.as_ref(), ChatCommand::McpTools, None).await;
    let ChatResponse::McpTools { tools } = listed else {
        panic!("expected live tools, got {listed:?}");
    };
    assert!(tools.is_empty(), "a disabled tool must not be offered");

    let selected = dispatch(
        fixture.service.as_ref(),
        ChatCommand::SetSessionTools {
            session_id: session.clone(),
            tools: vec!["echo:greet".into()],
        },
        None,
    )
    .await;
    let ChatResponse::Error { error, .. } = selected else {
        panic!("expected a typed failure, got {selected:?}");
    };
    assert_eq!(error.code, "MCP_TOOLS_UNAVAILABLE");
    assert!(error.message.contains("disabled by policy"));

    let opened = fixture.open(&session).await;
    assert!(opened.session.extra.get("mcpTools").is_none());
}
