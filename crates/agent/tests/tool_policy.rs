//! Policy and approval tests (`TEST-MCP-003/004/005/008`, `TEST-AI-019`).

use std::collections::BTreeMap;

use myharness_agent::tool_policy::{
    ApprovalContext, ApprovalError, ApprovalScope, ApprovalStore, ArgumentMatch, PolicyDecision,
    PolicyEffect, PolicyRule, ToolPattern, ToolPolicy, evaluate,
};
use serde_json::{Value, json};

fn interactive() -> ApprovalContext {
    ApprovalContext::interactive("session-1", true)
}

fn rule(tool: ToolPattern, arguments: ArgumentMatch, effect: PolicyEffect) -> PolicyRule {
    PolicyRule {
        tool,
        arguments,
        effect,
    }
}

fn exact_rule(tool: &str, arguments: &[(&str, Value)], effect: PolicyEffect) -> PolicyRule {
    let arguments = arguments
        .iter()
        .map(|(key, value)| ((*key).to_string(), value.clone()))
        .collect::<BTreeMap<_, _>>();
    rule(
        ToolPattern::Exact(tool.to_string()),
        ArgumentMatch::Exact(arguments),
        effect,
    )
}

#[test]
fn default_is_ask_with_every_allowed_scope() {
    let decision = ToolPolicy::default().decide("github:issues", &json!({}), &interactive());
    assert_eq!(
        decision,
        PolicyDecision::Ask {
            allowed_scopes: vec![
                ApprovalScope::Once,
                ApprovalScope::Session,
                ApprovalScope::Persistent
            ]
        }
    );
}

#[test]
fn deny_wins_over_allow_and_ask() {
    let policy = ToolPolicy {
        default: PolicyEffect::Allow,
        rules: vec![
            rule(ToolPattern::Any, ArgumentMatch::Any, PolicyEffect::Allow),
            rule(
                ToolPattern::Exact("github:delete".to_string()),
                ArgumentMatch::Any,
                PolicyEffect::Ask,
            ),
            rule(
                ToolPattern::Exact("github:delete".to_string()),
                ArgumentMatch::Any,
                PolicyEffect::Deny,
            ),
        ],
    };
    assert_eq!(
        policy.decide("github:delete", &json!({}), &interactive()),
        PolicyDecision::Deny {
            reason: "denied by a matching policy rule".to_string()
        }
    );
    assert_eq!(
        policy.decide("github:issues", &json!({}), &interactive()),
        PolicyDecision::Allow
    );
}

#[test]
fn argument_scoped_rules_check_the_actual_arguments() {
    let policy = ToolPolicy {
        default: PolicyEffect::Allow,
        rules: vec![exact_rule(
            "github:create_issue",
            &[("repo", json!("prod"))],
            PolicyEffect::Deny,
        )],
    };
    assert_eq!(
        policy.decide(
            "github:create_issue",
            &json!({"repo": "prod", "title": "deploy"}),
            &interactive()
        ),
        PolicyDecision::Deny {
            reason: "denied by a matching policy rule".to_string()
        }
    );
    assert_eq!(
        policy.decide(
            "github:create_issue",
            &json!({"repo": "dev", "title": "deploy"}),
            &interactive()
        ),
        PolicyDecision::Allow
    );
    // The tool stays declared: only one argument set is denied.
    assert!(!policy.hides_tool("github:create_issue", &interactive()));
}

#[test]
fn an_unconditional_deny_hides_the_tool() {
    let policy = ToolPolicy {
        default: PolicyEffect::Allow,
        rules: vec![rule(
            ToolPattern::Server("danger".to_string()),
            ArgumentMatch::Any,
            PolicyEffect::Deny,
        )],
    };
    assert!(policy.hides_tool("danger:exec", &interactive()));
    assert!(!policy.hides_tool("safe:read", &interactive()));
    assert_eq!(
        policy.visible_tool_names(["danger:exec", "safe:read", "other:read"], &interactive()),
        vec!["safe:read", "other:read"]
    );
}

#[test]
fn a_server_pattern_matches_only_that_server() {
    let policy = ToolPolicy {
        default: PolicyEffect::Ask,
        rules: vec![rule(
            ToolPattern::Server("github".to_string()),
            ArgumentMatch::Any,
            PolicyEffect::Allow,
        )],
    };
    assert_eq!(
        policy.decide("github:issues", &json!({}), &interactive()),
        PolicyDecision::Allow
    );
    assert!(matches!(
        policy.decide("gitlab:issues", &json!({}), &interactive()),
        PolicyDecision::Ask { .. }
    ));
}

#[test]
fn persistent_scope_is_gated_by_context() {
    let mut store = ApprovalStore::default();
    let closed = ApprovalContext::interactive("session-1", false);
    assert_eq!(
        closed.allowed_scopes(),
        vec![ApprovalScope::Once, ApprovalScope::Session]
    );
    assert_eq!(
        store.grant(
            &closed,
            "github:issues",
            &json!({"repo": "prod"}),
            ApprovalScope::Persistent,
            100
        ),
        Err(ApprovalError::ScopeNotAllowed(ApprovalScope::Persistent))
    );

    let open = interactive();
    assert_eq!(
        open.allowed_scopes(),
        vec![
            ApprovalScope::Once,
            ApprovalScope::Session,
            ApprovalScope::Persistent
        ]
    );
    let record = store
        .grant(
            &open,
            "github:issues",
            &json!({"repo": "prod"}),
            ApprovalScope::Persistent,
            100,
        )
        .unwrap();
    assert_eq!(record.scope, ApprovalScope::Persistent);
    assert_eq!(record.expires_at, None);
}

#[test]
fn a_scheduled_task_with_zero_days_offers_only_once() {
    let context = ApprovalContext::scheduled("task-1", true, 0);
    assert_eq!(context.allowed_scopes(), vec![ApprovalScope::Once]);
    let mut store = ApprovalStore::default();
    assert_eq!(
        store.grant(
            &context,
            "github:issues",
            &json!({}),
            ApprovalScope::Session,
            100
        ),
        Err(ApprovalError::ScopeNotAllowed(ApprovalScope::Session))
    );
}

#[test]
fn a_scheduled_approval_expires_after_its_lifetime() {
    let context = ApprovalContext::scheduled("task-1", true, 2);
    let mut store = ApprovalStore::default();
    let record = store
        .grant(
            &context,
            "github:issues",
            &json!({"repo": "prod"}),
            ApprovalScope::Session,
            1_000,
        )
        .unwrap();
    let expiry = 1_000 + 2 * 86_400;
    assert_eq!(record.expires_at, Some(expiry));
    assert!(store.check(&context, "github:issues", &json!({"repo": "prod"}), expiry - 1));
    assert!(!store.check(&context, "github:issues", &json!({"repo": "prod"}), expiry));

    store.revoke_expired(expiry);
    assert!(store.records().is_empty());
}

#[test]
fn a_session_approval_is_argument_and_session_specific() {
    let context = interactive();
    let mut store = ApprovalStore::default();
    store
        .grant(
            &context,
            "github:issues",
            &json!({"repo": "prod", "state": "open"}),
            ApprovalScope::Session,
            10,
        )
        .unwrap();
    assert!(store.check(
        &context,
        "github:issues",
        &json!({"state": "open", "repo": "prod"}),
        11
    ));
    assert!(!store.check(
        &context,
        "github:issues",
        &json!({"repo": "dev", "state": "open"}),
        11
    ));
    let other = ApprovalContext::interactive("session-2", true);
    assert!(!store.check(
        &other,
        "github:issues",
        &json!({"repo": "prod", "state": "open"}),
        11
    ));

    store.clear_session("session-1");
    assert!(!store.check(
        &context,
        "github:issues",
        &json!({"repo": "prod", "state": "open"}),
        11
    ));
    assert!(store.records().is_empty());
}

#[test]
fn a_persistent_approval_survives_a_session_clear() {
    let context = interactive();
    let mut store = ApprovalStore::default();
    store
        .grant(
            &context,
            "github:issues",
            &json!({"repo": "prod"}),
            ApprovalScope::Persistent,
            10,
        )
        .unwrap();
    store.clear_session("session-1");
    let other = ApprovalContext::interactive("session-2", true);
    assert!(store.check(&other, "github:issues", &json!({"repo": "prod"}), 11));
    assert_eq!(store.records().len(), 1);
}

#[test]
fn evaluate_uses_an_approval_but_never_overrides_deny() {
    let policy = ToolPolicy {
        default: PolicyEffect::Ask,
        rules: vec![rule(
            ToolPattern::Exact("danger:exec".to_string()),
            ArgumentMatch::Any,
            PolicyEffect::Deny,
        )],
    };
    let context = interactive();
    let mut store = ApprovalStore::default();
    assert!(matches!(
        evaluate(&policy, &store, &context, "safe:read", &json!({}), 10),
        PolicyDecision::Ask { .. }
    ));
    store
        .grant(
            &context,
            "safe:read",
            &json!({}),
            ApprovalScope::Session,
            10,
        )
        .unwrap();
    assert_eq!(
        evaluate(&policy, &store, &context, "safe:read", &json!({}), 11),
        PolicyDecision::Allow
    );
    assert_eq!(
        evaluate(&policy, &store, &context, "danger:exec", &json!({}), 11),
        PolicyDecision::Deny {
            reason: "denied by a matching policy rule".to_string()
        }
    );
}

#[test]
fn once_is_never_stored() {
    let mut store = ApprovalStore::default();
    assert_eq!(
        store.grant(
            &interactive(),
            "safe:read",
            &json!({}),
            ApprovalScope::Once,
            10
        ),
        Err(ApprovalError::OnceCannotBeStored)
    );
}

#[test]
fn the_policy_round_trips_and_uses_explicit_kinds() {
    let policy = ToolPolicy {
        default: PolicyEffect::Deny,
        rules: vec![PolicyRule {
            tool: ToolPattern::Server("github".to_string()),
            arguments: ArgumentMatch::Exact(
                [("repo".to_string(), json!("prod"))]
                    .into_iter()
                    .collect(),
            ),
            effect: PolicyEffect::Ask,
        }],
    };
    let value = serde_json::to_value(&policy).unwrap();
    assert_eq!(value["default"], "deny");
    assert_eq!(value["rules"][0]["tool"]["kind"], "server");
    assert_eq!(value["rules"][0]["tool"]["value"], "github");
    assert_eq!(value["rules"][0]["arguments"]["kind"], "exact");
    assert_eq!(value["rules"][0]["effect"], "ask");
    let back: ToolPolicy = serde_json::from_value(value).unwrap();
    assert_eq!(policy, back);
}
