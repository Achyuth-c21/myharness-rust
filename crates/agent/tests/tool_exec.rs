//! Tool execution gate tests (`TEST-MCP-003/004/008`, `TEST-AI-019`).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use async_trait::async_trait;
use myharness_agent::tool_exec::{
    execute_tool_call, ToolCallRequest, ToolExecution, ToolExecutor, ToolOutput,
};
use myharness_agent::tool_policy::{
    ApprovalContext, ApprovalScope, ApprovalStore, ArgumentMatch, PolicyEffect, PolicyRule,
    ToolPattern, ToolPolicy,
};
use myharness_provider_sdk::ContentPart;
use serde_json::{json, Value};

struct MockExecutor {
    calls: AtomicUsize,
    result: Mutex<Result<ToolOutput, String>>,
}

impl Default for MockExecutor {
    fn default() -> Self {
        Self {
            calls: AtomicUsize::new(0),
            result: Mutex::new(Ok(ToolOutput::text("ok"))),
        }
    }
}

impl MockExecutor {
    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl ToolExecutor for MockExecutor {
    async fn execute(&self, _tool: &str, _arguments: Value) -> Result<ToolOutput, String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.result.lock().unwrap().clone()
    }
}

fn context() -> ApprovalContext {
    ApprovalContext::interactive("session-1", true)
}

fn call(name: &str, arguments: Value) -> ToolCallRequest {
    ToolCallRequest {
        id: "toolu_1".to_string(),
        name: name.to_string(),
        arguments,
    }
}

fn deny_rule(tool: &str, arguments: ArgumentMatch) -> PolicyRule {
    PolicyRule {
        tool: ToolPattern::Exact(tool.to_string()),
        arguments,
        effect: PolicyEffect::Deny,
    }
}

#[tokio::test]
async fn a_denied_call_never_reaches_the_executor() {
    let policy = ToolPolicy {
        default: PolicyEffect::Allow,
        rules: vec![deny_rule("danger:exec", ArgumentMatch::Any)],
    };
    let store = ApprovalStore::default();
    let executor = MockExecutor::default();

    let outcome = execute_tool_call(
        &policy,
        &store,
        &context(),
        &call("danger:exec", json!({})),
        &executor,
        10,
    )
    .await;
    assert!(matches!(outcome, ToolExecution::Denied { .. }));
    assert_eq!(executor.calls(), 0);
}

#[tokio::test]
async fn ask_returns_scopes_without_executing() {
    let policy = ToolPolicy::default();
    let store = ApprovalStore::default();
    let executor = MockExecutor::default();

    let outcome = execute_tool_call(
        &policy,
        &store,
        &context(),
        &call("safe:read", json!({})),
        &executor,
        10,
    )
    .await;
    assert_eq!(
        outcome,
        ToolExecution::NeedsApproval {
            allowed_scopes: vec![
                ApprovalScope::Once,
                ApprovalScope::Session,
                ApprovalScope::Persistent
            ]
        }
    );
    assert_eq!(executor.calls(), 0);
}

#[tokio::test]
async fn an_allowed_call_executes_and_preserves_content() {
    let policy = ToolPolicy {
        default: PolicyEffect::Allow,
        rules: Vec::new(),
    };
    let store = ApprovalStore::default();
    let executor = MockExecutor::default();
    *executor.result.lock().unwrap() = Ok(ToolOutput {
        content: vec![
            ContentPart::Text {
                text: "screenshot".to_string(),
            },
            ContentPart::Image {
                media_type: "image/png".to_string(),
                data: "aGk=".to_string(),
            },
        ],
        structured_content: Some(json!({"ok": true})),
        is_error: false,
    });

    let outcome = execute_tool_call(
        &policy,
        &store,
        &context(),
        &call("safe:read", json!({"path": "x"})),
        &executor,
        10,
    )
    .await;
    let ToolExecution::Completed(output) = outcome else {
        panic!("expected a completed call");
    };
    assert_eq!(executor.calls(), 1);
    assert!(!output.is_error);
    assert_eq!(output.content.len(), 2);
    assert!(matches!(
        &output.content[1],
        ContentPart::Image { media_type, .. } if media_type == "image/png"
    ));
    assert_eq!(output.structured_content, Some(json!({"ok": true})));
}

#[tokio::test]
async fn an_approved_ask_executes() {
    let policy = ToolPolicy::default();
    let mut store = ApprovalStore::default();
    let executor = MockExecutor::default();
    let request = call("safe:read", json!({"path": "x"}));

    let before = execute_tool_call(&policy, &store, &context(), &request, &executor, 10).await;
    assert!(matches!(before, ToolExecution::NeedsApproval { .. }));
    assert_eq!(executor.calls(), 0);

    store
        .grant(
            &context(),
            "safe:read",
            &json!({"path": "x"}),
            ApprovalScope::Session,
            10,
        )
        .unwrap();
    let after = execute_tool_call(&policy, &store, &context(), &request, &executor, 11).await;
    assert!(matches!(after, ToolExecution::Completed(_)));
    assert_eq!(executor.calls(), 1);
}

#[tokio::test]
async fn an_executor_failure_is_a_model_visible_error() {
    let policy = ToolPolicy {
        default: PolicyEffect::Allow,
        rules: Vec::new(),
    };
    let store = ApprovalStore::default();
    let executor = MockExecutor::default();
    *executor.result.lock().unwrap() = Err("the MCP transport closed".to_string());

    let outcome = execute_tool_call(
        &policy,
        &store,
        &context(),
        &call("safe:read", json!({})),
        &executor,
        10,
    )
    .await;
    let ToolExecution::Completed(output) = outcome else {
        panic!("expected a completed error result");
    };
    assert_eq!(executor.calls(), 1);
    assert!(output.is_error);
    assert_eq!(
        output.content,
        vec![ContentPart::Text {
            text: "the MCP transport closed".to_string()
        }]
    );
}

#[tokio::test]
async fn an_approval_cannot_unlock_a_denied_tool() {
    let policy = ToolPolicy {
        default: PolicyEffect::Allow,
        rules: vec![deny_rule("danger:exec", ArgumentMatch::Any)],
    };
    let mut store = ApprovalStore::default();
    let executor = MockExecutor::default();
    store
        .grant(
            &context(),
            "danger:exec",
            &json!({}),
            ApprovalScope::Session,
            10,
        )
        .unwrap();

    let outcome = execute_tool_call(
        &policy,
        &store,
        &context(),
        &call("danger:exec", json!({})),
        &executor,
        11,
    )
    .await;
    assert!(matches!(outcome, ToolExecution::Denied { .. }));
    assert_eq!(executor.calls(), 0);
}

#[tokio::test]
async fn argument_scoped_deny_is_enforced_at_execution() {
    let mut arguments = std::collections::BTreeMap::new();
    arguments.insert("repo".to_string(), json!("prod"));
    let policy = ToolPolicy {
        default: PolicyEffect::Allow,
        rules: vec![deny_rule(
            "github:create_issue",
            ArgumentMatch::Exact(arguments),
        )],
    };
    let store = ApprovalStore::default();
    let executor = MockExecutor::default();

    let denied = execute_tool_call(
        &policy,
        &store,
        &context(),
        &call("github:create_issue", json!({"repo": "prod"})),
        &executor,
        10,
    )
    .await;
    assert!(matches!(denied, ToolExecution::Denied { .. }));
    assert_eq!(executor.calls(), 0);

    let allowed = execute_tool_call(
        &policy,
        &store,
        &context(),
        &call("github:create_issue", json!({"repo": "dev"})),
        &executor,
        10,
    )
    .await;
    assert!(matches!(allowed, ToolExecution::Completed(_)));
    assert_eq!(executor.calls(), 1);
}

#[test]
fn a_tool_use_block_becomes_a_request() {
    let part = ContentPart::ToolUse {
        id: "toolu_9".to_string(),
        name: "github:issues".to_string(),
        input: json!({"repo": "prod"}),
    };
    assert_eq!(
        ToolCallRequest::from_tool_use(&part),
        Some(ToolCallRequest {
            id: "toolu_9".to_string(),
            name: "github:issues".to_string(),
            arguments: json!({"repo": "prod"}),
        })
    );
    assert_eq!(
        ToolCallRequest::from_tool_use(&ContentPart::Text {
            text: "not a call".to_string()
        }),
        None
    );
}
