//! The turn loop — roadmap Phases 4–5 (`docs/13`; `docs/12` §8.2–§8.3).
//! This crate is the **agent layer** between a native provider adapter and the
//! session service: it replays the transcript, asks the adapter for one turn,
//! consumes the typed stream, and records what actually happened.
//!
//! The Phase 4 path is deliberately small and honest:
//!
//! - **One turn at a time; the plain path has no tools.** [`run_turn`] refuses
//!   tool declarations ([`AgentError::Unsupported`]) rather than advertising
//!   something it cannot execute. Phase 5's [`run_turn_with_tools`] is the
//!   explicit tool-enabled path, and it capability-gates the model first.
//! - **No retries and no substitution.** A provider failure is persisted and
//!   returned with its class and message unchanged (`TEST-AI-021`; `docs/12`
//!   §4.2). Retry policy is unobserved, so none is invented here.
//! - **Cancellation leaves a `partial` turn** carrying what the stream
//!   delivered before the drop — and dropping the stream is what cancels the
//!   native request (`TEST-SESSION-001` stop clause; `TEST-AI-015`;
//!   `AI-STREAM-02`). A stop before the first delta still resolves the live
//!   turn (empty `partial`, error names the cancellation), so the start marker
//!   is never mistaken for a crash.
//! - **A live assistant turn is the crash marker.** It is opened as a
//!   `partial` row before the provider is contacted and finished exactly once.
//!   A leftover live row on the next send is recovered as an explicit
//!   interruption (`TEST-SESSION-002`) — never replayed or duplicated.
//! - **Native stop reasons are preserved** on the turn (`turns.stop_reason`,
//!   schema v2) instead of being collapsed to "done" (`docs/12` §4.1;
//!   `AI-ADAPTER-08`).
//!
//! The transcript content shape is this crate's [`TurnContent`]: the app's
//! [`ContentPart`] blocks plus the response-only citation annotations, which
//! have no request-side home in the sdk model. Citations are kept in the
//! record and excluded from replay — no adapter accepts a response annotation
//! as a request block, and discarding them would be the `AI-STREAM-01` loss.
//!
//! Not in this crate: the profile→adapter construction seam (the caller passes
//! a [`ProviderAdapter`]), the renderer bridge, usage aggregation, retention,
//! and drafts. Those compose this loop from `myharness-bridge`; the loop stays
//! free of transport and UI decisions.
//!
//! Phase 5 adds [`tool_policy`] and [`tool_exec`] (the allow/ask/deny engine
//! and the one execution gate) plus [`run_turn_with_tools`], the multi-round
//! loop that makes them real:
//!
//! - tools are filtered by policy **before** the request is built, so an
//!   unconditionally denied tool never reaches the provider (`TEST-AI-019`);
//! - each model tool call is validated against its declared schema and then
//!   passes the execution gate; a denial is a model-visible error result, not
//!   an execution;
//! - the assistant tool-use turn and the matching tool-result turn are both
//!   persisted, so replay after a restart keeps the provider's grammar valid;
//! - the loop is bounded by a caller-chosen round limit (`Unknown` in the
//!   shipped app; the rebuild default is 16) and never invents a retry.
//!
//! A tool result is data. It is inserted only as a `ToolResult` content block;
//! no result can add a tool, change policy, or approve itself.

pub mod permission;
pub mod tool_exec;
pub mod tool_policy;
pub mod tool_schema;

pub use permission::PermissionMode;

use std::collections::{BTreeMap, BTreeSet};

use futures::StreamExt;
use myharness_provider_sdk::{
    AuthContext, ContentPart, ModelKey, NativeStopReason, NativeTurnRequest, ProviderAdapter,
    ProviderError, Role, SystemBlock, ThinkingConfig, ToolSpec, TurnEvent, Usage,
};
use myharness_sessions::{AuditKeyStore, SessionError, SessionService};
use myharness_store::{NewUsageEvent, StoreError, TurnRole, TurnRow, TurnStatus};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio_util::sync::CancellationToken;

use crate::tool_exec::{
    execute_tool_call, ApprovalPrompt, ApprovalRequest, ToolCallRequest, ToolExecution,
    ToolExecutor, ToolOutput,
};
use crate::tool_policy::{
    evaluate, ApprovalContext, ApprovalScope, ApprovalStore, PolicyDecision, ToolPolicy,
};
use crate::tool_schema::validate_arguments;

/// The rebuild's default tool-round bound. The shipped value was not
/// observed (`docs/14` §9); a caller may lower or raise it explicitly.
pub const DEFAULT_MAX_TOOL_ROUNDS: usize = 16;

/// The replayed conversation, in the sdk's message shape.
type Messages = Vec<(Role, Vec<ContentPart>)>;

// ---------------------------------------------------------------------------
// What the caller decides
// ---------------------------------------------------------------------------

/// One turn's decisions, exactly as the caller made them. Nothing here is
/// defaulted by the loop: the adapter sends what the app decided
/// (`AI-PROVIDER-01`, `AI-ADAPTER-10`).
#[derive(Debug, Clone, PartialEq)]
pub struct TurnInput {
    /// The full model identity; its `provider_id` must name the adapter that
    /// receives the turn (`AI-PROVIDER-04` — no silent cross-provider send).
    pub model: ModelKey,
    /// System prompt as blocks so prompt-caching controls survive
    /// (`AI-ADAPTER-05`).
    pub system: Vec<SystemBlock>,
    /// The user content of this turn.
    pub user: Vec<ContentPart>,
    pub max_tokens: u32,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub top_k: Option<u32>,
    pub stop_sequences: Vec<String>,
    pub thinking: Option<ThinkingConfig>,
    pub betas: Vec<String>,
    /// Tools advertised to the provider. [`run_turn`] refuses a non-empty
    /// list because it has no executor; [`run_turn_with_tools`] accepts and
    /// filters it. Declared tools are never silently ignored.
    pub tools: Vec<ToolSpec>,
}

impl TurnInput {
    /// A text-only turn — the smallest honest input, used by tests and the
    /// first renderer wire-up.
    pub fn text(model: ModelKey, max_tokens: u32, user_text: impl Into<String>) -> TurnInput {
        TurnInput {
            model,
            system: Vec::new(),
            user: vec![ContentPart::Text {
                text: user_text.into(),
            }],
            max_tokens,
            temperature: None,
            top_p: None,
            top_k: None,
            stop_sequences: Vec::new(),
            thinking: None,
            betas: Vec::new(),
            tools: Vec::new(),
        }
    }

    fn validate(&self, adapter_id: &str, tools_supported: bool) -> Result<(), AgentError> {
        if self.model.provider_id != adapter_id {
            return Err(AgentError::ProviderMismatch {
                model_provider: self.model.provider_id.clone(),
                adapter: adapter_id.to_string(),
            });
        }
        if self.model.model_id.trim().is_empty() {
            return Err(AgentError::Validation("the model id is empty"));
        }
        if self.max_tokens == 0 {
            return Err(AgentError::Validation(
                "max_tokens must be greater than zero",
            ));
        }
        if self.user.is_empty() {
            return Err(AgentError::Validation("the turn has no user content"));
        }
        if !self.tools.is_empty() && !tools_supported {
            return Err(AgentError::Unsupported(
                "this entry point has no tool executor; use run_turn_with_tools rather than advertising tools that cannot run",
            ));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// The transcript record shape
// ---------------------------------------------------------------------------

/// One stored turn's content: request-replayable parts plus the response-only
/// annotations the sdk model has no request block for.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TurnContent {
    #[serde(default)]
    pub parts: Vec<ContentPart>,
    /// Citation annotations kept from the stream (`AI-STREAM-01`). Excluded
    /// from replay on purpose; see the crate docs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub citations: Vec<serde_json::Value>,
}

impl TurnContent {
    /// A one-block text turn.
    pub fn text(text: impl Into<String>) -> TurnContent {
        TurnContent {
            parts: vec![ContentPart::Text { text: text.into() }],
            citations: Vec::new(),
        }
    }

    /// The concatenated text blocks — a convenience for assertions and the
    /// first renderer surface, not a rendering policy.
    pub fn plain_text(&self) -> String {
        self.parts
            .iter()
            .filter_map(|part| match part {
                ContentPart::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }
}

// ---------------------------------------------------------------------------
// Failures
// ---------------------------------------------------------------------------

/// Failures at the agent layer. Provider errors keep their class and message
/// verbatim — the UI and the ledger both see the provider's own wording.
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error(transparent)]
    Session(#[from] SessionError),
    #[error(transparent)]
    Store(#[from] StoreError),
    /// The model belongs to a different provider than the adapter it was
    /// handed to. Sending would be a silent cross-provider substitution
    /// (`AI-MODEL-01`).
    #[error("the model belongs to provider {model_provider}, but this adapter is {adapter}")]
    ProviderMismatch {
        model_provider: String,
        adapter: String,
    },
    /// The stored transcript cannot be replayed; replay fails loudly rather
    /// than sending a shortened conversation (`docs/14` §9).
    #[error("the transcript could not be replayed: {0}")]
    Transcript(&'static str),
    #[error("{0}")]
    Validation(&'static str),
    #[error("{0}")]
    Unsupported(&'static str),
    /// The adapter broke the turn contract (a stream that ends without a stop
    /// reason). The turn is recorded as an error, never as a success.
    #[error("the provider stream broke the turn contract: {0}")]
    Protocol(&'static str),
    /// The caller stopped the turn. Any delivered content is recorded as a
    /// `partial` turn first (`TEST-SESSION-001`).
    #[error("the turn was cancelled")]
    Cancelled,
    /// The token was already cancelled before anything was persisted: there
    /// is no attempt to record and no provider contact, so the caller must not
    /// report the user's message as recorded.
    #[error("the turn was cancelled before it started")]
    CancelledBeforeSend,
    /// The model kept requesting tools after the bounded number of rounds.
    /// Every requested tool call was executed (or refused) and persisted
    /// before this is returned, so the transcript can still be replayed.
    #[error("the tool loop reached its {rounds}-round limit")]
    ToolLoopLimit { rounds: usize },
    /// The host could not record an approval the user granted. This is a host
    /// failure, not a policy denial; the turn stops rather than executing on
    /// an unrecorded grant.
    #[error("the tool approval could not be recorded: {0}")]
    Approval(String),
    #[error("the provider failed ({:?}): {}", .0.class, .0.message)]
    Provider(ProviderError),
}

// ---------------------------------------------------------------------------
// The run
// ---------------------------------------------------------------------------

/// Everything one turn needs. Construct with [`TurnRun::new`] and add a live
/// tap with [`TurnRun::with_observer`].
pub struct TurnRun<'a, K: AuditKeyStore> {
    pub sessions: &'a SessionService<K>,
    pub adapter: &'a dyn ProviderAdapter,
    pub auth: &'a AuthContext,
    pub session_id: &'a str,
    pub input: TurnInput,
    pub cancel: &'a CancellationToken,
    /// A live tap for the renderer bridge: content events (text, reasoning,
    /// citations, tool calls) are passed through before they are accumulated.
    /// Usage, stop, and error events are folded into the final response.
    pub observer: Option<&'a (dyn Fn(&TurnEvent) + Send + Sync)>,
}

impl<'a, K: AuditKeyStore> TurnRun<'a, K> {
    pub fn new(
        sessions: &'a SessionService<K>,
        adapter: &'a dyn ProviderAdapter,
        auth: &'a AuthContext,
        session_id: &'a str,
        input: TurnInput,
        cancel: &'a CancellationToken,
    ) -> TurnRun<'a, K> {
        TurnRun {
            sessions,
            adapter,
            auth,
            session_id,
            input,
            cancel,
            observer: None,
        }
    }

    pub fn with_observer(
        mut self,
        observer: &'a (dyn Fn(&TurnEvent) + Send + Sync),
    ) -> TurnRun<'a, K> {
        self.observer = Some(observer);
        self
    }
}

/// What a completed turn produced.
#[derive(Debug)]
pub struct TurnOutcome {
    pub user_turn: TurnRow,
    pub assistant_turn: TurnRow,
    /// The provider's own stop spelling, preserved (`AI-ADAPTER-08`).
    pub stop_reason: NativeStopReason,
    /// Usage exactly as the adapter reported it (absent channels stay absent
    /// here; the ledger stores numbers).
    pub usage: Usage,
}

/// Run one turn: persist the user turn, stream the provider's reply, then
/// persist the assistant turn — `complete`, `partial`, or `error`.
///
/// Side-effect contract:
///
/// - A validation/refusal error happens **before** any row is written and
///   before the provider is contacted. Recovery of an earlier interrupted
///   attempt runs after those checks, so a refused turn writes nothing.
/// - The user turn and the live assistant marker are written before the send,
///   so a crash or cancellation cannot lose what the user asked and the next
///   open can name the interruption.
/// - A provider failure finishes the live turn as `error` and writes a usage
///   event carrying the provider's message, then returns
///   [`AgentError::Provider`].
/// - Cancellation finishes the live turn as `partial` (empty when no delta
///   arrived) and writes a usage event marked cancelled, then returns
///   [`AgentError::Cancelled`].
pub async fn run_turn<K: AuditKeyStore>(run: TurnRun<'_, K>) -> Result<TurnOutcome, AgentError> {
    let TurnRun {
        sessions,
        adapter,
        auth,
        session_id,
        input,
        cancel,
        observer,
    } = run;

    input.validate(adapter.id(), false)?;

    // A turn stopped before anything was sent leaves no trace at all: there
    // was no attempt to record.
    if cancel.is_cancelled() {
        return Err(AgentError::CancelledBeforeSend);
    }

    // A live assistant row from a previous process is a crash marker, not a
    // reply to replay: name the interruption before reading the transcript.
    sessions.recover_interrupted(session_id)?;

    let (history, next_seq) = replay(sessions, session_id)?;
    let assistant = sessions.begin_assistant_turn(
        session_id,
        next_seq + 1,
        &encode(&TurnContent::default()),
    )?;

    // The user turn is the one thing that must survive whatever follows.
    let user_content = TurnContent {
        parts: input.user.clone(),
        citations: Vec::new(),
    };
    let user_turn = sessions.append_turn(
        session_id,
        next_seq,
        TurnRole::User,
        TurnStatus::Complete,
        &encode(&user_content),
        None,
        None,
        None,
    )?;

    let mut messages = history;
    push_message(&mut messages, Role::User, input.user.clone());
    let request = build_request(&input, messages, Vec::new());
    let outcome = execute_round(
        sessions,
        adapter,
        auth,
        session_id,
        assistant,
        &input.model.model_id,
        request,
        &BTreeMap::new(),
        cancel,
        observer,
    )
    .await?;

    Ok(TurnOutcome {
        user_turn,
        assistant_turn: outcome.assistant_turn,
        stop_reason: outcome.stop,
        usage: outcome.usage.unwrap_or_default(),
    })
}

/// The result of one provider round, before a tool loop decides whether the
/// round is final.
struct RoundOutcome {
    assistant_turn: TurnRow,
    content: TurnContent,
    stop: NativeStopReason,
    usage: Option<Usage>,
}

/// Send one request and finish exactly one live assistant turn.
///
/// This is the shared engine for both the plain turn and each round of the
/// tool loop. It performs no policy or tool decisions: the caller owns those.
#[allow(clippy::too_many_arguments)]
async fn execute_round<K: AuditKeyStore>(
    sessions: &SessionService<K>,
    adapter: &dyn ProviderAdapter,
    auth: &AuthContext,
    session_id: &str,
    assistant: TurnRow,
    model_id: &str,
    request: NativeTurnRequest,
    wire_to_internal: &BTreeMap<String, String>,
    cancel: &CancellationToken,
    observer: Option<&(dyn Fn(&TurnEvent) + Send + Sync)>,
) -> Result<RoundOutcome, AgentError> {
    let recorder = Recorder {
        sessions,
        session_id,
        assistant,
        model_id,
    };

    // Shape/liveness check only; the adapter performs no network round-trip
    // (`docs/12` §8.2). A failure here is still an attempt worth recording
    // next to the user's message.
    if let Err(error) = adapter.authenticate(auth).await {
        recorder.record_failure(&TurnContent::default(), None, &error)?;
        return Err(AgentError::Provider(error));
    }

    let mut stream = match adapter.create_turn(auth, &request).await {
        Ok(stream) => stream,
        Err(error) => {
            recorder.record_failure(&TurnContent::default(), None, &error)?;
            return Err(AgentError::Provider(error));
        }
    };

    let mut accumulator = Accumulator::default();
    let mut usage: Option<Usage> = None;
    let mut stop: Option<NativeStopReason> = None;
    let mut failure: Option<ProviderError> = None;
    let mut cancelled = false;

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                cancelled = true;
                break;
            }
            next = stream.next() => {
                match next {
                    None => break,
                    Some(Ok(TurnEvent::Done)) => break,
                    // A stop race with cancellation resolves as a completed
                    // turn: the model did finish, only the bookkeeping event
                    // was still in flight.
                    Some(Ok(TurnEvent::Stop { reason })) => stop = Some(reason),
                    Some(Ok(TurnEvent::Usage { usage: reported })) => usage = Some(reported),
                    Some(Ok(TurnEvent::Error { error })) => {
                        failure = Some(error);
                        break;
                    }
                    Some(Ok(mut event)) => {
                        map_wire_tool_call(&mut event, wire_to_internal);
                        if let Some(observer) = observer {
                            observer(&event);
                        }
                        accumulator.push(&event);
                    }
                    Some(Err(error)) => {
                        failure = Some(error);
                        break;
                    }
                }
            }
        }
    }

    // Dropping the stream cancels the native request (`TEST-AI-015`).
    drop(stream);
    let content = accumulator.finish();

    if let Some(error) = failure {
        recorder.record_failure(&content, usage.as_ref(), &error)?;
        return Err(AgentError::Provider(error));
    }

    if cancelled && stop.is_none() {
        recorder.record_cancelled(&content, usage.as_ref())?;
        return Err(AgentError::Cancelled);
    }

    let Some(stop) = stop else {
        recorder.record_protocol_break(&content, usage.as_ref())?;
        return Err(AgentError::Protocol(
            "the provider stream ended without a stop reason",
        ));
    };

    let assistant_turn = recorder.record_assistant(
        TurnStatus::Complete,
        &content,
        usage.as_ref(),
        None,
        Some(&stop),
    )?;
    recorder.record_usage(usage.as_ref(), None)?;

    Ok(RoundOutcome {
        assistant_turn,
        content,
        stop,
        usage,
    })
}

/// The provider-safe name for an internal MCP tool name.
///
/// Internal names are `<server>:<tool>` (`docs/12` §5.1), but Anthropic and
/// OpenAI-compatible wires only accept `[A-Za-z0-9_-]` and bound the length.
/// The mapping keeps a readable prefix and appends a stable hash so distinct
/// internal names cannot collapse to one wire name. It is rebuild-defined
/// because the shipped wire spelling was not captured (`docs/14` §9); the
/// internal name remains the policy, snapshot, and transcript identity.
pub fn wire_tool_name(internal: &str) -> String {
    let mut prefix: String = internal
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
                character
            } else {
                '_'
            }
        })
        .collect();
    if prefix.is_empty() {
        prefix.push_str("tool");
    }
    if prefix.len() > 40 {
        prefix.truncate(40);
    }
    let digest = Sha256::digest(internal.as_bytes());
    let hash: String = digest
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("mcp_{prefix}_{hash}")
}

fn wire_tool_names(tools: &[ToolSpec]) -> BTreeMap<String, String> {
    tools
        .iter()
        .map(|tool| (wire_tool_name(&tool.name), tool.name.clone()))
        .collect()
}

fn map_wire_tool_call(event: &mut TurnEvent, wire_to_internal: &BTreeMap<String, String>) {
    if let TurnEvent::ToolCall { name, .. } = event {
        if let Some(internal) = wire_to_internal.get(name) {
            *name = internal.clone();
        }
    }
}

fn wire_content_part(part: ContentPart) -> ContentPart {
    match part {
        ContentPart::ToolUse { id, name, input } => ContentPart::ToolUse {
            id,
            name: wire_tool_name(&name),
            input,
        },
        other => other,
    }
}

fn build_request(input: &TurnInput, messages: Messages, tools: Vec<ToolSpec>) -> NativeTurnRequest {
    let messages = messages
        .into_iter()
        .map(|(role, parts)| (role, parts.into_iter().map(wire_content_part).collect()))
        .collect();
    let tools = tools
        .into_iter()
        .map(|mut tool| {
            tool.name = wire_tool_name(&tool.name);
            tool
        })
        .collect();
    NativeTurnRequest {
        model: input.model.clone(),
        system: input.system.clone(),
        messages,
        tools,
        max_tokens: input.max_tokens,
        temperature: input.temperature,
        top_p: input.top_p,
        top_k: input.top_k,
        stop_sequences: input.stop_sequences.clone(),
        thinking: input.thinking,
        betas: input.betas.clone(),
        stream: true,
    }
}

// ---------------------------------------------------------------------------
// The tool loop
// ---------------------------------------------------------------------------

/// Everything one tool-enabled turn needs.
pub struct ToolTurnRun<'a, K: AuditKeyStore> {
    /// The base turn: transcript, adapter, model, and the user's content.
    pub run: TurnRun<'a, K>,
    /// The one policy authority for visibility and execution.
    pub policy: &'a ToolPolicy,
    /// Approvals the host has already granted for this session.
    pub approvals: &'a mut ApprovalStore,
    /// Session/policy facts (including whether a persistent scope is offered).
    pub context: ApprovalContext,
    /// The executor that receives only calls the gate allowed.
    pub executor: &'a dyn ToolExecutor,
    /// The UI/headless prompt used when the policy says `Ask`.
    pub prompt: &'a dyn ApprovalPrompt,
    /// Epoch seconds for approval records. Supplied by the host so tests are
    /// deterministic and no clock is invented in the core.
    pub now: u64,
    /// Bound on provider rounds. Defaults to [`DEFAULT_MAX_TOOL_ROUNDS`].
    pub max_rounds: usize,
    /// Code permission mode. `Default` for every non-Code caller.
    pub permission_mode: PermissionMode,
    /// True only when policy explicitly permits `Bypass`. A `Bypass` mode with
    /// this false is a validation error, never a silent downgrade.
    pub bypass_allowed: bool,
    /// Host-declared file-edit tools. `AcceptEdits` auto-approves only names in
    /// this set. No built-in edit tools exist in this slice, so production
    /// leaves it empty.
    pub edit_tools: Option<&'a BTreeSet<String>>,
}

impl<'a, K: AuditKeyStore> ToolTurnRun<'a, K> {
    pub fn new(
        run: TurnRun<'a, K>,
        policy: &'a ToolPolicy,
        approvals: &'a mut ApprovalStore,
        context: ApprovalContext,
        executor: &'a dyn ToolExecutor,
        prompt: &'a dyn ApprovalPrompt,
        now: u64,
    ) -> ToolTurnRun<'a, K> {
        ToolTurnRun {
            run,
            policy,
            approvals,
            context,
            executor,
            prompt,
            now,
            max_rounds: DEFAULT_MAX_TOOL_ROUNDS,
            permission_mode: PermissionMode::Default,
            bypass_allowed: false,
            edit_tools: None,
        }
    }

    pub fn with_max_rounds(mut self, max_rounds: usize) -> ToolTurnRun<'a, K> {
        self.max_rounds = max_rounds;
        self
    }

    /// Apply a Code permission mode and the policy fact that gates `Bypass`.
    pub fn with_permission_mode(
        mut self,
        mode: PermissionMode,
        bypass_allowed: bool,
    ) -> ToolTurnRun<'a, K> {
        self.permission_mode = mode;
        self.bypass_allowed = bypass_allowed;
        self
    }

    /// Declare the host's file-edit tools for `AcceptEdits`. Production has no
    /// built-in edit tools yet; the set is a seam, not a claim.
    pub fn with_edit_tools(mut self, edit_tools: &'a BTreeSet<String>) -> ToolTurnRun<'a, K> {
        self.edit_tools = Some(edit_tools);
        self
    }
}

/// What a tool-enabled turn produced.
#[derive(Debug)]
pub struct ToolTurnOutcome {
    pub user_turn: TurnRow,
    /// Every assistant round, oldest first. The last one is the final reply
    /// when `final_stop_reason` is not `tool_use`.
    pub assistant_turns: Vec<TurnRow>,
    /// Every tool-result row, oldest first. Each row may hold several result
    /// blocks when the model issued parallel calls in one round.
    pub tool_turns: Vec<TurnRow>,
    pub final_stop_reason: NativeStopReason,
    /// One entry per provider round, in order. The ledger already received
    /// each round separately; the bridge uses this for the response surface.
    pub usages: Vec<Usage>,
}

/// Run one tool-enabled turn: persist the user turn, advertise only the tools
/// policy allows, then alternate provider rounds and gated tool execution
/// until the model stops without a tool call or the round bound is reached.
///
/// Every assistant tool-use turn is followed by a persisted tool-result turn
/// before the next request, including when a call is denied, invalid, fails,
/// or is cancelled. That is what keeps the next replay valid.
///
/// The host must confirm the model advertises tool use before calling this
/// entry point (`AI-PROVIDER-05`); the agent validates the request shape but
/// does not perform provider discovery.
pub async fn run_turn_with_tools<K: AuditKeyStore>(
    run: ToolTurnRun<'_, K>,
) -> Result<ToolTurnOutcome, AgentError> {
    let ToolTurnRun {
        run,
        policy,
        approvals,
        context,
        executor,
        prompt,
        now,
        max_rounds,
        permission_mode,
        bypass_allowed,
        edit_tools,
    } = run;
    let TurnRun {
        sessions,
        adapter,
        auth,
        session_id,
        input,
        cancel,
        observer,
    } = run;

    if max_rounds == 0 {
        return Err(AgentError::Validation(
            "the tool round limit must be greater than zero",
        ));
    }
    if permission_mode == PermissionMode::Bypass && !bypass_allowed {
        return Err(AgentError::Validation(
            "bypass permissions are disabled by policy",
        ));
    }
    input.validate(adapter.id(), true)?;
    if cancel.is_cancelled() {
        return Err(AgentError::CancelledBeforeSend);
    }
    // Expired grants are never offered again, and they do not linger in the
    // durable approval file after the next turn.
    approvals.revoke_expired(now);
    sessions.recover_interrupted(session_id)?;
    let (history, next_seq) = replay(sessions, session_id)?;

    // The provider sees only tools whose policy effect is not an
    // unconditional deny (`TEST-AI-019`). Plan mode is stricter: it advertises
    // no tools at all. The full schema map is kept so a stale/undeclared call
    // can still be refused by the gate with a useful message.
    let visible: BTreeSet<String> = if permission_mode == PermissionMode::Plan {
        BTreeSet::new()
    } else {
        policy
            .visible_tool_names(input.tools.iter().map(|tool| tool.name.as_str()), &context)
            .into_iter()
            .map(str::to_string)
            .collect()
    };
    let schemas: BTreeMap<String, Value> = input
        .tools
        .iter()
        .map(|tool| (tool.name.clone(), tool.input_schema.clone()))
        .collect();
    let tools: Vec<ToolSpec> = input
        .tools
        .iter()
        .filter(|tool| visible.contains(&tool.name))
        .cloned()
        .collect();
    let wire_to_internal = wire_tool_names(&tools);

    // The crash marker is opened before the user row, exactly as in the
    // plain turn, so a crash between the two cannot duplicate the user turn.
    let mut assistant = sessions.begin_assistant_turn(
        session_id,
        next_seq + 1,
        &encode(&TurnContent::default()),
    )?;

    let user_content = TurnContent {
        parts: input.user.clone(),
        citations: Vec::new(),
    };
    let user_turn = sessions.append_turn(
        session_id,
        next_seq,
        TurnRole::User,
        TurnStatus::Complete,
        &encode(&user_content),
        None,
        None,
        None,
    )?;

    let mut messages = history;
    push_message(&mut messages, Role::User, input.user.clone());

    // Row sequence: user = next_seq, first assistant = next_seq + 1, first
    // tool row = next_seq + 2, next assistant = next_seq + 3, ...
    let mut seq = next_seq + 2;
    let mut assistant_turns = Vec::new();
    let mut tool_turns = Vec::new();
    let mut usages = Vec::new();

    for round in 0..max_rounds {
        let request = build_request(&input, messages.clone(), tools.clone());
        let outcome = execute_round(
            sessions,
            adapter,
            auth,
            session_id,
            assistant,
            &input.model.model_id,
            request,
            &wire_to_internal,
            cancel,
            observer,
        )
        .await?;

        usages.push(outcome.usage.clone().unwrap_or_default());
        assistant_turns.push(outcome.assistant_turn);

        if outcome.stop != NativeStopReason::ToolUse {
            return Ok(ToolTurnOutcome {
                user_turn,
                assistant_turns,
                tool_turns,
                final_stop_reason: outcome.stop,
                usages,
            });
        }

        let calls: Vec<ToolCallRequest> = outcome
            .content
            .parts
            .iter()
            .filter_map(ToolCallRequest::from_tool_use)
            .collect();
        if calls.is_empty() {
            return Err(AgentError::Protocol(
                "the provider stopped for a tool call but sent no tool_use block",
            ));
        }

        let mut result_parts = Vec::with_capacity(calls.len());
        let mut cancelled_during_tools = false;
        for call in &calls {
            if cancel.is_cancelled() {
                cancelled_during_tools = true;
            }
            let output = if cancelled_during_tools {
                ToolOutput::error("the turn was cancelled before this tool call ran")
            } else {
                execute_one_tool(
                    policy,
                    approvals,
                    &context,
                    executor,
                    prompt,
                    now,
                    session_id,
                    call,
                    schemas.get(&call.name),
                    cancel,
                    permission_mode,
                    edit_tools,
                )
                .await?
            };
            let output = output.sanitize().unwrap_or_else(ToolOutput::error);
            if let Some(observer) = observer {
                observer(&TurnEvent::ToolResult {
                    id: call.id.clone(),
                    parts: output.content.clone(),
                });
            }
            result_parts.push(ContentPart::ToolResult {
                tool_use_id: call.id.clone(),
                content: output.content,
                is_error: output.is_error,
                cache_control: None,
            });
            if cancel.is_cancelled() {
                cancelled_during_tools = true;
            }
        }

        let tool_content = TurnContent {
            parts: result_parts.clone(),
            citations: Vec::new(),
        };
        let tool_turn = sessions.append_turn(
            session_id,
            seq,
            TurnRole::Tool,
            TurnStatus::Complete,
            &encode(&tool_content),
            None,
            None,
            None,
        )?;
        tool_turns.push(tool_turn);

        if cancelled_during_tools {
            return Err(AgentError::Cancelled);
        }

        // The next request needs both halves of the exchange: the assistant's
        // tool_use blocks and the matching tool_result blocks.
        push_message(&mut messages, Role::Assistant, outcome.content.parts);
        push_message(&mut messages, Role::User, result_parts);

        if round + 1 == max_rounds {
            return Err(AgentError::ToolLoopLimit { rounds: max_rounds });
        }

        assistant =
            sessions.begin_assistant_turn(session_id, seq + 1, &encode(&TurnContent::default()))?;
        seq += 2;
    }

    // The loop always returns inside the body; this is unreachable but keeps
    // the type checker honest about the bound.
    Err(AgentError::ToolLoopLimit { rounds: max_rounds })
}

#[allow(clippy::too_many_arguments)]
async fn execute_one_tool(
    policy: &ToolPolicy,
    approvals: &mut ApprovalStore,
    context: &ApprovalContext,
    executor: &dyn ToolExecutor,
    prompt: &dyn ApprovalPrompt,
    now: u64,
    session_id: &str,
    call: &ToolCallRequest,
    schema: Option<&Value>,
    cancel: &CancellationToken,
    permission_mode: PermissionMode,
    edit_tools: Option<&BTreeSet<String>>,
) -> Result<ToolOutput, AgentError> {
    let Some(schema) = schema else {
        return Ok(ToolOutput::error(
            "the tool is not declared in this session and was not executed",
        ));
    };
    if let Err(reason) = validate_arguments(schema, &call.arguments) {
        return Ok(ToolOutput::error(format!(
            "invalid tool arguments: {reason}"
        )));
    }

    // Plan mode never advertises tools, but a provider can still hallucinate a
    // call. Refuse it here, before the policy or the executor.
    if permission_mode == PermissionMode::Plan {
        return Ok(ToolOutput::error(
            "the tool call was denied by Plan mode: no changes are executed",
        ));
    }

    // `Bypass` auto-approves an `Ask` decision only when policy permitted the
    // mode (validated before the turn). `AcceptEdits` auto-approves only the
    // host's declared edit tools. A policy `Deny` is never bypassed.
    let auto_allow = match permission_mode {
        PermissionMode::Bypass => true,
        PermissionMode::AcceptEdits => edit_tools
            .is_some_and(|tools| tools.contains(&call.name)),
        PermissionMode::Default | PermissionMode::Plan => false,
    };
    if auto_allow {
        match evaluate(policy, approvals, context, &call.name, &call.arguments, now) {
            PolicyDecision::Deny { reason } => {
                return Ok(ToolOutput::error(format!(
                    "the tool call was denied by policy: {reason}"
                )));
            }
            PolicyDecision::Allow | PolicyDecision::Ask { .. } => {
                return match executor.execute(&call.name, call.arguments.clone()).await {
                    Ok(output) => Ok(output),
                    Err(message) => Ok(ToolOutput::error(message)),
                };
            }
        }
    }

    match execute_tool_call(policy, approvals, context, call, executor, now).await {
        ToolExecution::Completed(output) => Ok(output),
        ToolExecution::Denied { reason } => {
            // A denial is a result, not an execution: the model sees why
            // the call did not run, and the executor was never contacted.
            Ok(ToolOutput::error(format!(
                "the tool call was denied by policy: {reason}"
            )))
        }
        ToolExecution::NeedsApproval { allowed_scopes } => {
            let choice = prompt
                .approve(
                    ApprovalRequest {
                        session_id,
                        call_id: &call.id,
                        tool: &call.name,
                        arguments: &call.arguments,
                        allowed_scopes: &allowed_scopes,
                    },
                    cancel,
                )
                .await;
            let Some(scope) = choice else {
                return Ok(ToolOutput::error(
                    "the tool call was denied by the user or the approval request was cancelled",
                ));
            };
            if !allowed_scopes.contains(&scope) {
                return Ok(ToolOutput::error(
                    "the host returned an approval scope that was not offered",
                ));
            }
            if scope != ApprovalScope::Once {
                approvals
                    .grant(context, &call.name, &call.arguments, scope, now)
                    .map_err(|error| AgentError::Approval(error.to_string()))?;
            }
            // The user approved this exact call. Execute it directly:
            // re-evaluating would prompt again for `Once` (which is
            // deliberately never stored) and would make an approval
            // useless for the call that produced it.
            match executor.execute(&call.name, call.arguments.clone()).await {
                Ok(output) => Ok(output),
                Err(message) => Ok(ToolOutput::error(message)),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Replay
// ---------------------------------------------------------------------------

/// Load the transcript and turn it into request messages.
///
/// Rules, in order:
///
/// - `user` turns become user messages; `assistant` turns become assistant
///   messages; `tool` turns become **user** messages carrying
///   `ToolResult` blocks (the provider grammar places tool results in the
///   user role). `system` rows are not transcript messages.
/// - Empty turns (an error turn that never received content) are skipped —
///   there is nothing to replay.
/// - Consecutive same-role turns are merged. A failed send leaves two user
///   turns in a row, and the Messages grammar rejects that sequence; merging
///   keeps every block without inventing a role or dropping the failed turn.
/// - A stored blob that cannot be read or decoded fails the replay by name —
///   never a silently shortened conversation (`docs/14` §9).
fn replay<K: AuditKeyStore>(
    sessions: &SessionService<K>,
    session_id: &str,
) -> Result<(Messages, i64), AgentError> {
    let turns = sessions.store().list_turns(session_id)?;
    let next_seq = turns.last().map(|turn| turn.seq + 1).unwrap_or(0);

    let mut messages: Messages = Vec::new();
    for turn in &turns {
        let role = match turn.role.as_str() {
            "user" => Role::User,
            // Tool results are user-role blocks in every native provider
            // grammar the sdk models (`docs/12` §8.2).
            "tool" => Role::User,
            "assistant" => Role::Assistant,
            _ => continue,
        };
        let bytes =
            sessions
                .store()
                .files
                .read(&turn.content_ref)?
                .ok_or(AgentError::Transcript(
                    "a stored turn's content blob is missing",
                ))?;
        let content: TurnContent = serde_json::from_slice(&bytes).map_err(|_| {
            AgentError::Transcript("a stored turn's content is not in the transcript shape")
        })?;
        if content.parts.is_empty() {
            continue;
        }
        push_message(&mut messages, role, content.parts);
    }

    Ok((messages, next_seq))
}

/// Append a message, merging into the previous one when the role repeats.
/// Consecutive same-role turns are a normal outcome of a failed send; the
/// Messages grammar wants one message per role run, and merging keeps every
/// block instead of choosing one to drop.
fn push_message(messages: &mut Messages, role: Role, parts: Vec<ContentPart>) {
    match messages.last_mut() {
        Some((last_role, last_parts)) if *last_role == role => last_parts.extend(parts),
        _ => messages.push((role, parts)),
    }
}

// ---------------------------------------------------------------------------
// Recording
// ---------------------------------------------------------------------------

/// Finishes the live assistant turn opened before the send.
struct Recorder<'a, K: AuditKeyStore> {
    sessions: &'a SessionService<K>,
    session_id: &'a str,
    /// The `partial` row opened at send time; finishing it is a one-way
    /// transition enforced by the store.
    assistant: TurnRow,
    model_id: &'a str,
}

impl<K: AuditKeyStore> Recorder<'_, K> {
    fn record_assistant(
        &self,
        status: TurnStatus,
        content: &TurnContent,
        usage: Option<&Usage>,
        error: Option<&str>,
        stop: Option<&NativeStopReason>,
    ) -> Result<TurnRow, SessionError> {
        let usage_json =
            usage.map(|usage| serde_json::to_value(usage).unwrap_or(serde_json::Value::Null));
        self.sessions.finish_turn(
            self.session_id,
            &self.assistant.id,
            status,
            &encode(content),
            usage_json,
            error,
            stop.map(NativeStopReason::wire).as_deref(),
        )
    }

    /// Usage events preserve every count the adapter reported
    /// (`TEST-SESSION-007`). A channel the provider did not report lands as
    /// zero in the ledger (it stores numbers for aggregation); the exact
    /// `Option` shape stays on the turn's `usage_json`.
    fn record_usage(&self, usage: Option<&Usage>, error: Option<&str>) -> Result<(), SessionError> {
        let (input, output, cache_read, cache_write, web_search) = match usage {
            Some(usage) => (
                usage.input,
                usage.output,
                usage.cache_read,
                usage.cache_write,
                usage.web_search,
            ),
            None => (0, 0, None, None, None),
        };
        self.sessions.record_usage(
            self.session_id,
            NewUsageEvent {
                model: self.model_id,
                input_tokens: count(input),
                output_tokens: count(output),
                cache_read_tokens: count(cache_read.unwrap_or(0)),
                cache_write_tokens: count(cache_write.unwrap_or(0)),
                search_requests: count(web_search.unwrap_or(0)),
                error,
            },
        )?;
        Ok(())
    }

    fn record_failure(
        &self,
        content: &TurnContent,
        usage: Option<&Usage>,
        error: &ProviderError,
    ) -> Result<TurnRow, SessionError> {
        let turn = self.record_assistant(
            TurnStatus::Error,
            content,
            usage,
            Some(&error.message),
            None,
        )?;
        self.record_usage(usage, Some(&error.message))?;
        Ok(turn)
    }

    /// Cancellation always resolves the live turn: with no delta it is an
    /// empty `partial` carrying the cancellation text, so recovery cannot
    /// later misread the start marker as a crash.
    fn record_cancelled(
        &self,
        content: &TurnContent,
        usage: Option<&Usage>,
    ) -> Result<(), SessionError> {
        self.record_assistant(
            TurnStatus::Partial,
            content,
            usage,
            Some("the turn was cancelled"),
            None,
        )?;
        self.record_usage(usage, Some("the turn was cancelled"))
    }

    fn record_protocol_break(
        &self,
        content: &TurnContent,
        usage: Option<&Usage>,
    ) -> Result<(), SessionError> {
        let message = "the provider stream ended without a stop reason";
        self.record_assistant(TurnStatus::Error, content, usage, Some(message), None)?;
        self.record_usage(usage, Some(message))
    }
}

/// The ledger stores `i64`; provider counts are `u64`. A count beyond the
/// ledger's range is not an observable token count — cap it rather than wrap
/// into a negative number.
fn count(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn encode(content: &TurnContent) -> Vec<u8> {
    serde_json::to_vec(content).expect("transcript content is JSON-encodable")
}

// ---------------------------------------------------------------------------
// Stream accumulation
// ---------------------------------------------------------------------------

/// Folds typed deltas into the content blocks the transcript stores. Text and
/// reasoning runs stay in arrival order; a signature closes its thinking
/// block; tool arguments are used when the adapter parsed them, otherwise the
/// streamed fragment is parsed here, and the raw fragment is the last resort
/// (never a fabricated empty object).
#[derive(Default)]
struct Accumulator {
    parts: Vec<ContentPart>,
    text: String,
    thinking: String,
    signature: Option<String>,
    citations: Vec<serde_json::Value>,
}

impl Accumulator {
    fn push(&mut self, event: &TurnEvent) {
        match event {
            TurnEvent::TextDelta { text } => {
                self.flush_reasoning();
                self.text.push_str(text);
            }
            TurnEvent::ReasoningDelta { text, signature } => {
                self.flush_text();
                self.thinking.push_str(text);
                if signature.is_some() {
                    self.signature = signature.clone();
                }
            }
            TurnEvent::ReasoningRedacted { data } => {
                self.flush_text();
                self.flush_reasoning();
                self.parts
                    .push(ContentPart::RedactedThinking { data: data.clone() });
            }
            TurnEvent::Citation { citation } => self.citations.push(citation.clone()),
            TurnEvent::ToolCall {
                id,
                name,
                arguments_delta,
                arguments,
            } => {
                self.flush_text();
                self.flush_reasoning();
                let input = arguments
                    .clone()
                    .or_else(|| {
                        arguments_delta
                            .as_deref()
                            .and_then(|raw| serde_json::from_str(raw).ok())
                    })
                    .unwrap_or(serde_json::Value::Null);
                self.parts.push(ContentPart::ToolUse {
                    id: id.clone(),
                    name: name.clone(),
                    input,
                });
            }
            // Terminal/bookkeeping events are handled by the loop. A
            // `ToolResult` is emitted by the loop itself and is not stream
            // content to accumulate.
            TurnEvent::ToolResult { .. }
            | TurnEvent::Usage { .. }
            | TurnEvent::Stop { .. }
            | TurnEvent::Error { .. }
            | TurnEvent::Done => {}
        }
    }

    fn flush_text(&mut self) {
        if !self.text.is_empty() {
            self.parts.push(ContentPart::Text {
                text: std::mem::take(&mut self.text),
            });
        }
    }

    /// A signature-only delta still closes a thinking block: a signature with
    /// an empty body is kept rather than dropped (`AI-ADAPTER-04`).
    fn flush_reasoning(&mut self) {
        if !self.thinking.is_empty() || self.signature.is_some() {
            self.parts.push(ContentPart::Reasoning {
                thinking: std::mem::take(&mut self.thinking),
                signature: self.signature.take(),
            });
        }
    }

    fn finish(mut self) -> TurnContent {
        self.flush_text();
        self.flush_reasoning();
        TurnContent {
            parts: self.parts,
            citations: self.citations,
        }
    }
}
