//! Tool policy and approval engine — roadmap Phase 5 (`docs/13` Phase 5;
//! `docs/12` §5.1; `docs/features/mcp-skills-tools.md` §3).
//!
//! The engine is deliberately pure: it decides, it never executes and never
//! talks to a provider or an MCP server. The agent loop owns execution, so it
//! is the only caller that can turn a decision into a tool call
//! (`docs/12` §4). Keeping the engine separate is what makes the rule
//! "a tool call cannot approve itself or change policy" enforceable: a tool
//! result has no path to [`ApprovalStore::grant`].
//!
//! What is specified and what is a rebuild decision:
//!
//! - **Specified** (`docs/features/mcp-skills-tools.md` §3): allow / ask /
//!   deny; argument-scoped rules checked against actual arguments; per-call,
//!   session, and persistent approvals; persistent approvals gated by
//!   `mcpPersistentAlwaysAllowEnabled`; scheduled tasks have a separate
//!   lifetime and `0` disables lasting approval; denied tools never appear in
//!   the provider request; approvals are recorded in audit/session metadata.
//! - **Rebuild-defined, because the shipped shapes were not observed**
//!   (`docs/14` §9): the rule serialization, the fail-closed precedence
//!   (deny > ask > allow > default), the subset argument match, the default
//!   policy ([`PolicyEffect::Ask`]), and the SHA-256 argument fingerprint.
//!   These are implementation choices, not claims about the shipped app.
//!
//! Not in this slice: reading `builtinToolPolicy` / `toolPolicyEndpoint` from
//! config, the renderer approval prompt, durable approval persistence,
//! tool-search filtering, and wiring the decision into [`crate::run_turn`].
//! [`crate::TurnInput::validate`] still refuses every non-empty tool list, so
//! no UI can advertise a tool before that wiring exists.

use std::collections::BTreeMap;
use std::fmt::Write;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

/// What the policy says about one tool call.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PolicyEffect {
    /// The call may proceed.
    Allow,
    /// The user must approve the call before it proceeds.
    Ask,
    /// The call must not execute, and the tool must be absent from the
    /// provider request when this effect applies unconditionally.
    Deny,
}

/// One approval lifetime.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ApprovalScope {
    /// This call only. Never stored.
    Once,
    /// Every matching call in the current session.
    Session,
    /// Every matching call in every session, when policy allows it.
    Persistent,
}

/// A tool selector.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum ToolPattern {
    /// Any namespaced tool.
    Any,
    /// Exactly this namespaced name (`<server>:<tool>`).
    Exact(String),
    /// Any tool from this server (`<server>:*`).
    Server(String),
}

impl ToolPattern {
    fn matches(&self, tool: &str) -> bool {
        match self {
            Self::Any => true,
            Self::Exact(expected) => expected == tool,
            Self::Server(server) => tool
                .strip_prefix(server.as_str())
                .and_then(|rest| rest.strip_prefix(':'))
                .is_some_and(|tool| !tool.is_empty()),
        }
    }
}

/// How much of the argument object a rule examines.
///
/// `Exact` is a **subset** match: every listed key must equal the actual
/// value, and extra keys are allowed. This is a rebuild-defined shape; the
/// shipped rule syntax was not captured (`docs/14` §9).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum ArgumentMatch {
    /// Match any arguments.
    #[default]
    Any,
    /// Match when every listed top-level key equals the actual value.
    Exact(BTreeMap<String, Value>),
}

impl ArgumentMatch {
    fn matches(&self, arguments: &Value) -> bool {
        match self {
            Self::Any => true,
            Self::Exact(expected) => {
                let Value::Object(actual) = arguments else {
                    return false;
                };
                expected
                    .iter()
                    .all(|(key, value)| actual.get(key) == Some(value))
            }
        }
    }
}

/// One ordered policy rule.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyRule {
    /// Tool selector.
    pub tool: ToolPattern,
    /// Argument selector; defaults to any arguments.
    #[serde(default)]
    pub arguments: ArgumentMatch,
    /// What a match means.
    pub effect: PolicyEffect,
}

/// The local policy.
///
/// Precedence is fail-closed and independent of rule order: any matching
/// `Deny` wins, then any matching `Ask`, then any matching `Allow`, then
/// [`ToolPolicy::default`]. A more specific allow can therefore never override
/// a deny; that is a rebuild decision, stated here rather than implied.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolPolicy {
    /// Effect when no rule matches. Defaults to `Ask`.
    #[serde(default = "default_effect")]
    pub default: PolicyEffect,
    /// Rules. Order does not change precedence.
    #[serde(default)]
    pub rules: Vec<PolicyRule>,
}

impl Default for ToolPolicy {
    fn default() -> Self {
        Self {
            default: default_effect(),
            rules: Vec::new(),
        }
    }
}

fn default_effect() -> PolicyEffect {
    PolicyEffect::Ask
}

/// The result of evaluating the policy for one call.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "decision",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PolicyDecision {
    /// Execute without prompting.
    Allow,
    /// Prompt before executing. The listed scopes are the choices the host
    /// may offer; `Persistent` is absent when policy disables it.
    Ask {
        /// Scopes allowed in this context, in `Once`, `Session`, `Persistent`
        /// order.
        allowed_scopes: Vec<ApprovalScope>,
    },
    /// Do not execute. The reason is safe to show; it never contains
    /// arguments.
    Deny {
        /// Why the call was denied.
        reason: String,
    },
}

/// Runtime facts the policy needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApprovalContext {
    /// Session that owns a session-scoped approval.
    pub session_id: String,
    /// True for a scheduled task; its lasting approvals use a separate
    /// lifetime.
    pub scheduled: bool,
    /// Resolved `mcpPersistentAlwaysAllowEnabled`.
    pub persistent_allowed: bool,
    /// Resolved `mcpScheduledTaskApprovalLifetimeDays`; ignored for an
    /// interactive session.
    pub scheduled_lifetime_days: u32,
}

impl ApprovalContext {
    /// An interactive session.
    pub fn interactive(session_id: impl Into<String>, persistent_allowed: bool) -> Self {
        Self {
            session_id: session_id.into(),
            scheduled: false,
            persistent_allowed,
            scheduled_lifetime_days: 0,
        }
    }

    /// A scheduled task. `lifetime_days == 0` disables every lasting scope.
    pub fn scheduled(
        session_id: impl Into<String>,
        persistent_allowed: bool,
        lifetime_days: u32,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            scheduled: true,
            persistent_allowed,
            scheduled_lifetime_days: lifetime_days,
        }
    }

    /// The scopes this context may offer.
    pub fn allowed_scopes(&self) -> Vec<ApprovalScope> {
        let mut scopes = vec![ApprovalScope::Once];
        if !self.scheduled || self.scheduled_lifetime_days > 0 {
            scopes.push(ApprovalScope::Session);
            if self.persistent_allowed {
                scopes.push(ApprovalScope::Persistent);
            }
        }
        scopes
    }
}

impl ToolPolicy {
    /// Decide one call against the actual arguments.
    pub fn decide(
        &self,
        tool: &str,
        arguments: &Value,
        context: &ApprovalContext,
    ) -> PolicyDecision {
        let mut saw_ask = false;
        let mut saw_allow = false;
        for rule in &self.rules {
            if !rule.tool.matches(tool) || !rule.arguments.matches(arguments) {
                continue;
            }
            match rule.effect {
                PolicyEffect::Deny => {
                    return PolicyDecision::Deny {
                        reason: "denied by a matching policy rule".to_string(),
                    };
                }
                PolicyEffect::Ask => saw_ask = true,
                PolicyEffect::Allow => saw_allow = true,
            }
        }
        if saw_ask {
            return PolicyDecision::Ask {
                allowed_scopes: context.allowed_scopes(),
            };
        }
        if saw_allow {
            return PolicyDecision::Allow;
        }
        match self.default {
            PolicyEffect::Allow => PolicyDecision::Allow,
            PolicyEffect::Ask => PolicyDecision::Ask {
                allowed_scopes: context.allowed_scopes(),
            },
            PolicyEffect::Deny => PolicyDecision::Deny {
                reason: "denied by the default policy".to_string(),
            },
        }
    }

    /// True when the tool is denied **unconditionally**, so it must not be
    /// declared to the provider.
    ///
    /// Argument-scoped denies are not visible here: `null` does not match an
    /// exact argument selector, so the tool stays declared and the denied
    /// arguments are refused at execution. That is the only safe split —
    /// hiding a tool because one argument set is denied would make the
    /// allowed argument sets unreachable.
    pub fn hides_tool(&self, tool: &str, context: &ApprovalContext) -> bool {
        matches!(
            self.decide(tool, &Value::Null, context),
            PolicyDecision::Deny { .. }
        )
    }

    /// Filter tool names for the provider request (`TEST-AI-019`).
    pub fn visible_tool_names<'a>(
        &self,
        tools: impl IntoIterator<Item = &'a str>,
        context: &ApprovalContext,
    ) -> Vec<&'a str> {
        tools
            .into_iter()
            .filter(|tool| !self.hides_tool(tool, context))
            .collect()
    }
}

/// One stored approval, also the audit record the host writes to session
/// metadata.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalRecord {
    /// Session that granted it (for a persistent grant, the granting session).
    pub session_id: String,
    /// Exact namespaced tool name.
    pub tool: String,
    /// SHA-256 of the canonical argument JSON.
    pub arguments_fingerprint: String,
    /// Lifetime that was granted.
    pub scope: ApprovalScope,
    /// Unix seconds when it was granted.
    pub granted_at: u64,
    /// Unix seconds when it stops applying; `None` means no expiry in this
    /// slice (a session ends when the host clears it; a persistent grant is
    /// durable in a later store).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
}

/// Approval storage failures.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ApprovalError {
    /// `Once` is a per-call decision, not a stored grant.
    #[error("per-call approval is not stored; it applies to one call only")]
    OnceCannotBeStored,
    /// The context does not offer this scope.
    #[error("the {0:?} approval scope is not allowed in this context")]
    ScopeNotAllowed(ApprovalScope),
}

/// In-memory approvals.
///
/// This is deliberately not a database: durable persistent approvals and the
/// session-metadata audit write are later Phase 5 slices. The API is split so
/// only the host can grant, and a tool result has no reference to this store.
#[derive(Debug, Default)]
pub struct ApprovalStore {
    session: BTreeMap<SessionKey, ApprovalRecord>,
    persistent: BTreeMap<ArgumentKey, ApprovalRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct SessionKey {
    session_id: String,
    tool: String,
    arguments_fingerprint: String,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ArgumentKey {
    tool: String,
    arguments_fingerprint: String,
}

impl ApprovalStore {
    /// Store a session or persistent approval after an explicit user choice.
    ///
    /// `Once` is rejected because it must not survive the call. Persistent
    /// storage is in-memory here; the durable store is a later slice.
    pub fn grant(
        &mut self,
        context: &ApprovalContext,
        tool: &str,
        arguments: &Value,
        scope: ApprovalScope,
        now: u64,
    ) -> Result<ApprovalRecord, ApprovalError> {
        if scope == ApprovalScope::Once {
            return Err(ApprovalError::OnceCannotBeStored);
        }
        if !context.allowed_scopes().contains(&scope) {
            return Err(ApprovalError::ScopeNotAllowed(scope));
        }
        let arguments_fingerprint = arguments_fingerprint(arguments);
        let expires_at =
            if context.scheduled {
                Some(now.saturating_add(
                    u64::from(context.scheduled_lifetime_days).saturating_mul(86_400),
                ))
            } else {
                None
            };
        let record = ApprovalRecord {
            session_id: context.session_id.clone(),
            tool: tool.to_string(),
            arguments_fingerprint: arguments_fingerprint.clone(),
            scope,
            granted_at: now,
            expires_at,
        };
        match scope {
            ApprovalScope::Session => {
                self.session.insert(
                    SessionKey {
                        session_id: context.session_id.clone(),
                        tool: tool.to_string(),
                        arguments_fingerprint,
                    },
                    record.clone(),
                );
            }
            ApprovalScope::Persistent => {
                self.persistent.insert(
                    ArgumentKey {
                        tool: tool.to_string(),
                        arguments_fingerprint,
                    },
                    record.clone(),
                );
            }
            ApprovalScope::Once => unreachable!("checked above"),
        }
        Ok(record)
    }

    /// True when a session or persistent grant covers this exact call.
    pub fn check(
        &self,
        context: &ApprovalContext,
        tool: &str,
        arguments: &Value,
        now: u64,
    ) -> bool {
        let arguments_fingerprint = arguments_fingerprint(arguments);
        let session_key = SessionKey {
            session_id: context.session_id.clone(),
            tool: tool.to_string(),
            arguments_fingerprint: arguments_fingerprint.clone(),
        };
        if self
            .session
            .get(&session_key)
            .is_some_and(|record| is_active(record, now))
        {
            return true;
        }
        // A persistent grant is usable only while the host still allows
        // persistent approvals. A scheduled task never inherits an
        // interactive grant that has no expiry: its lifetime is separate and
        // must be stated on the record.
        if !context.persistent_allowed {
            return false;
        }
        let persistent_key = ArgumentKey {
            tool: tool.to_string(),
            arguments_fingerprint,
        };
        self.persistent.get(&persistent_key).is_some_and(|record| {
            is_active(record, now) && (!context.scheduled || record.expires_at.is_some())
        })
    }

    /// Drop every expired grant.
    pub fn revoke_expired(&mut self, now: u64) {
        self.session.retain(|_, record| is_active(record, now));
        self.persistent.retain(|_, record| is_active(record, now));
    }

    /// Drop every session-scoped grant for one session. Persistent grants are
    /// intentionally untouched.
    pub fn clear_session(&mut self, session_id: &str) {
        self.session.retain(|key, _| key.session_id != session_id);
    }

    /// Drop every grant, including persistent ones.
    pub fn clear_all(&mut self) {
        self.session.clear();
        self.persistent.clear();
    }

    /// Restore durable grants after a restart. Expired records and `Once`
    /// records are skipped; the return value is the number restored. This is
    /// a host-only operation, like [`grant`](Self::grant); a tool result has
    /// no path to it.
    pub fn restore(
        &mut self,
        records: impl IntoIterator<Item = ApprovalRecord>,
        now: u64,
    ) -> usize {
        let mut restored = 0;
        for record in records {
            if record.scope == ApprovalScope::Once || !is_active(&record, now) {
                continue;
            }
            match record.scope {
                ApprovalScope::Session => {
                    self.session.insert(
                        SessionKey {
                            session_id: record.session_id.clone(),
                            tool: record.tool.clone(),
                            arguments_fingerprint: record.arguments_fingerprint.clone(),
                        },
                        record,
                    );
                    restored += 1;
                }
                ApprovalScope::Persistent => {
                    self.persistent.insert(
                        ArgumentKey {
                            tool: record.tool.clone(),
                            arguments_fingerprint: record.arguments_fingerprint.clone(),
                        },
                        record,
                    );
                    restored += 1;
                }
                ApprovalScope::Once => {}
            }
        }
        restored
    }

    /// Every stored grant, for the host's audit/session metadata write.
    pub fn records(&self) -> Vec<ApprovalRecord> {
        let mut records: Vec<ApprovalRecord> = self
            .session
            .values()
            .chain(self.persistent.values())
            .cloned()
            .collect();
        records.sort_by(|left, right| {
            (
                &left.session_id,
                &left.tool,
                &left.arguments_fingerprint,
                left.scope,
            )
                .cmp(&(
                    &right.session_id,
                    &right.tool,
                    &right.arguments_fingerprint,
                    right.scope,
                ))
        });
        records
    }
}

/// Decide a call and honor an existing session/persistent approval.
///
/// A stored approval can only turn `Ask` into `Allow`; it can never override
/// a `Deny`, and it never changes the policy itself.
pub fn evaluate(
    policy: &ToolPolicy,
    store: &ApprovalStore,
    context: &ApprovalContext,
    tool: &str,
    arguments: &Value,
    now: u64,
) -> PolicyDecision {
    match policy.decide(tool, arguments, context) {
        PolicyDecision::Ask { allowed_scopes } => {
            if store.check(context, tool, arguments, now) {
                PolicyDecision::Allow
            } else {
                PolicyDecision::Ask { allowed_scopes }
            }
        }
        decision => decision,
    }
}

fn is_active(record: &ApprovalRecord, now: u64) -> bool {
    record.expires_at.is_none_or(|expires_at| now < expires_at)
}

/// SHA-256 over a canonical JSON encoding: object keys sorted recursively,
/// array order preserved. The exact encoding is rebuild-defined; it is only
/// used for equality and audit, never sent on a wire.
fn arguments_fingerprint(arguments: &Value) -> String {
    let canonical = canonical_value(arguments);
    let serialized = serde_json::to_string(&canonical).unwrap_or_default();
    hex(Sha256::digest(serialized.as_bytes()).as_slice())
}

fn canonical_value(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let sorted: BTreeMap<String, Value> = map
                .iter()
                .map(|(key, value)| (key.clone(), canonical_value(value)))
                .collect();
            Value::Object(sorted.into_iter().collect())
        }
        Value::Array(values) => Value::Array(values.iter().map(canonical_value).collect()),
        other => other.clone(),
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
}
