//! Tool execution gate — roadmap Phase 5 (`docs/12` §4 and §5.1;
//! `docs/features/mcp-skills-tools.md` §3 and §8).
//!
//! [`execute_tool_call`] is the one place where a model-issued tool call is
//! allowed to reach an executor. It evaluates the same [`ToolPolicy`] as the
//! provider-side visibility filter, honors an existing approval, and returns
//! [`ToolExecution::NeedsApproval`] instead of blocking on UI. The host prompts
//! the user, calls [`ApprovalStore::grant`] only after an explicit choice, and
//! calls the gate again. A denied call never reaches [`ToolExecutor`].
//!
//! The executor is deliberately policy-blind: it receives only calls that the
//! gate already allowed or that the user approved. This split is what keeps a
//! tool result from approving itself or changing policy — the result has no
//! reference to the policy or the approval store.
//!
//! Not in this slice: wiring the gate into [`crate::run_turn`], the
//! renderer approval prompt, the MCP manager adapter (which will map
//! `ToolCallResult` to [`ToolOutput`]), and the audit-chain write of
//! [`crate::tool_policy::ApprovalRecord`]. The turn loop still refuses tools,
//! so nothing can call this gate from the app yet.

use async_trait::async_trait;
use myharness_provider_sdk::ContentPart;
use serde_json::Value;

use crate::tool_policy::{
    ApprovalContext, ApprovalScope, ApprovalStore, PolicyDecision, ToolPolicy, evaluate,
};

/// One model-issued tool call.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolCallRequest {
    /// Provider tool-call id, preserved for the result block.
    pub id: String,
    /// Namespaced tool name (`<server>:<tool>`).
    pub name: String,
    /// Arguments exactly as the model produced them.
    pub arguments: Value,
}

impl ToolCallRequest {
    /// Extract a tool call from an accumulated provider block.
    pub fn from_tool_use(part: &ContentPart) -> Option<Self> {
        match part {
            ContentPart::ToolUse { id, name, input } => Some(Self {
                id: id.clone(),
                name: name.clone(),
                arguments: input.clone(),
            }),
            _ => None,
        }
    }
}

/// A tool result in the provider's content shape.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolOutput {
    /// Text, images, and documents exactly as the executor produced them.
    pub content: Vec<ContentPart>,
    /// Optional structured result. The provider encoders decide whether they
    /// can carry it; it is never silently converted to text here.
    pub structured_content: Option<Value>,
    /// True when the tool failed. The model sees this as an error result, not
    /// as a successful answer.
    pub is_error: bool,
}

impl ToolOutput {
    /// A successful text result.
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: vec![ContentPart::Text { text: text.into() }],
            structured_content: None,
            is_error: false,
        }
    }

    /// A model-visible failure. The message is the executor's own wording.
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            content: vec![ContentPart::Text {
                text: message.into(),
            }],
            structured_content: None,
            is_error: true,
        }
    }
}

/// Executes an already-approved tool call.
///
/// Implementations must not consult policy or approvals; the gate is the only
/// authority. A transport failure is returned as `Err(message)` and becomes a
/// model-visible error result, never a silent empty result.
#[async_trait]
pub trait ToolExecutor: Send + Sync {
    /// Run one call. `arguments` is the model's JSON object.
    async fn execute(&self, tool: &str, arguments: Value) -> Result<ToolOutput, String>;
}

/// What the gate decided for one call.
#[derive(Clone, Debug, PartialEq)]
pub enum ToolExecution {
    /// The executor ran (successfully or with a model-visible error).
    Completed(ToolOutput),
    /// Policy denied the call; the executor was not contacted.
    Denied {
        /// Safe reason; never contains arguments.
        reason: String,
    },
    /// The user must choose a scope before the call can run.
    NeedsApproval {
        /// Scopes the host may offer, from [`ApprovalContext::allowed_scopes`].
        allowed_scopes: Vec<ApprovalScope>,
    },
}

/// Decide and, only when allowed, execute one call.
pub async fn execute_tool_call(
    policy: &ToolPolicy,
    store: &ApprovalStore,
    context: &ApprovalContext,
    call: &ToolCallRequest,
    executor: &dyn ToolExecutor,
    now: u64,
) -> ToolExecution {
    match evaluate(policy, store, context, &call.name, &call.arguments, now) {
        PolicyDecision::Deny { reason } => ToolExecution::Denied { reason },
        PolicyDecision::Ask { allowed_scopes } => ToolExecution::NeedsApproval { allowed_scopes },
        PolicyDecision::Allow => match executor.execute(&call.name, call.arguments.clone()).await {
            Ok(output) => ToolExecution::Completed(output),
            Err(message) => ToolExecution::Completed(ToolOutput::error(message)),
        },
    }
}
