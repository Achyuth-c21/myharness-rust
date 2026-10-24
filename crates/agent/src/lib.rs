//! The turn loop — roadmap Phase 4 slice 2 (`docs/13` Phase 4; `docs/12`
//! §8.2–§8.3). This crate is the **agent layer** between a native provider
//! adapter and the session service: it replays the transcript, asks the
//! adapter for one turn, consumes the typed stream, and records what actually
//! happened.
//!
//! Slice 2 is deliberately small and honest:
//!
//! - **One turn, no tools.** [`ProviderAdapter::create_turn`] receives the
//!   replayed conversation plus the new user turn. Tool declarations are
//!   refused ([`AgentError::Unsupported`]) rather than advertised while
//!   nothing can execute them (`docs/14` §9: an unavailable capability is
//!   stated, never emulated); tool execution belongs to the MCP phase.
//! - **No retries and no substitution.** A provider failure is persisted and
//!   returned with its class and message unchanged (`TEST-AI-021`; `docs/12`
//!   §4.2). Retry policy is unobserved, so none is invented here.
//! - **Cancellation leaves a `partial` turn** carrying what the stream
//!   delivered before the drop — and dropping the stream is what cancels the
//!   native request (`TEST-SESSION-001` stop clause; `TEST-AI-015`;
//!   `AI-STREAM-02`).
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
//! Not in this slice: the profile→adapter construction seam (the caller passes
//! a [`ProviderAdapter`]), the renderer bridge, usage aggregation, retention,
//! and drafts.

use futures::StreamExt;
use myharness_provider_sdk::{
    AuthContext, ContentPart, ModelKey, NativeStopReason, NativeTurnRequest, ProviderAdapter,
    ProviderError, Role, SystemBlock, ThinkingConfig, ToolSpec, TurnEvent, Usage,
};
use myharness_sessions::{AuditKeyStore, SessionError, SessionService};
use myharness_store::{NewUsageEvent, StoreError, TurnRole, TurnRow, TurnStatus};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

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
    /// Must stay empty in this slice: declared tools are refused, never
    /// ignored (Phase 5 adds execution with the MCP layer).
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

    fn validate(&self, adapter_id: &str) -> Result<(), AgentError> {
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
        if !self.tools.is_empty() {
            return Err(AgentError::Unsupported(
                "tool declarations arrive with the MCP phase; declaring tools the loop cannot execute is refused",
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
    /// A live tap for the renderer bridge (slice 3): every non-error stream
    /// event is passed through before it is accumulated.
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
///   before the provider is contacted.
/// - The user turn is written before the send, so a crash or cancellation
///   cannot lose what the user asked.
/// - A provider failure writes an `error` assistant turn and a usage event
///   carrying the provider's message, then returns [`AgentError::Provider`].
/// - Cancellation writes the delivered content as `partial` (when there is
///   any) and a usage event marked cancelled, then returns
///   [`AgentError::Cancelled`].
pub async fn run_turn<K: AuditKeyStore>(
    run: TurnRun<'_, K>,
) -> Result<TurnOutcome, AgentError> {
    let TurnRun {
        sessions,
        adapter,
        auth,
        session_id,
        input,
        cancel,
        observer,
    } = run;

    input.validate(adapter.id())?;

    // A turn stopped before anything was sent leaves no trace at all: there
    // was no attempt to record.
    if cancel.is_cancelled() {
        return Err(AgentError::Cancelled);
    }

    let (history, next_seq) = replay(sessions, session_id)?;
    let recorder = Recorder {
        sessions,
        session_id,
        next_seq,
        model_id: &input.model.model_id,
    };

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
    let request = NativeTurnRequest {
        model: input.model.clone(),
        system: input.system.clone(),
        messages,
        tools: Vec::new(),
        max_tokens: input.max_tokens,
        temperature: input.temperature,
        top_p: input.top_p,
        top_k: input.top_k,
        stop_sequences: input.stop_sequences.clone(),
        thinking: input.thinking,
        betas: input.betas.clone(),
        stream: true,
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
                    Some(Ok(event)) => {
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

    Ok(TurnOutcome {
        user_turn,
        assistant_turn,
        stop_reason: stop,
        usage: usage.unwrap_or_default(),
    })
}

// ---------------------------------------------------------------------------
// Replay
// ---------------------------------------------------------------------------

/// Load the transcript and turn it into request messages.
///
/// Rules, in order:
///
/// - Only `user` and `assistant` turns become messages; `system`/`tool` rows
///   are not request messages in this slice.
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
            "assistant" => Role::Assistant,
            _ => continue,
        };
        let bytes = sessions
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
fn push_message(
    messages: &mut Messages,
    role: Role,
    parts: Vec<ContentPart>,
) {
    match messages.last_mut() {
        Some((last_role, last_parts)) if *last_role == role => last_parts.extend(parts),
        _ => messages.push((role, parts)),
    }
}

// ---------------------------------------------------------------------------
// Recording
// ---------------------------------------------------------------------------

/// Writes the turn tail. Assistant turns only — the user turn already exists.
struct Recorder<'a, K: AuditKeyStore> {
    sessions: &'a SessionService<K>,
    session_id: &'a str,
    /// The sequence of the user turn this run wrote; the assistant turn is the
    /// next slot.
    next_seq: i64,
    model_id: &'a str,
}

impl<K: AuditKeyStore> Recorder<'_, K> {
    /// The assistant turn is always the slot after the user turn — even when
    /// earlier failed attempts left later slots free (the store refuses a
    /// duplicate sequence, and replay has already established the tail).
    fn record_assistant(
        &self,
        status: TurnStatus,
        content: &TurnContent,
        usage: Option<&Usage>,
        error: Option<&str>,
        stop: Option<&NativeStopReason>,
    ) -> Result<TurnRow, SessionError> {
        let usage_json = usage.map(|usage| {
            serde_json::to_value(usage).unwrap_or(serde_json::Value::Null)
        });
        self.sessions.append_turn(
            self.session_id,
            self.next_seq + 1,
            TurnRole::Assistant,
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
        let turn =
            self.record_assistant(TurnStatus::Error, content, usage, Some(&error.message), None)?;
        self.record_usage(usage, Some(&error.message))?;
        Ok(turn)
    }

    /// Cancellation records what arrived as `partial`; a cancel before any
    /// content has nothing to show and writes no assistant turn.
    fn record_cancelled(
        &self,
        content: &TurnContent,
        usage: Option<&Usage>,
    ) -> Result<(), SessionError> {
        if !content.parts.is_empty() || !content.citations.is_empty() {
            self.record_assistant(
                TurnStatus::Partial,
                content,
                usage,
                Some("the turn was cancelled"),
                None,
            )?;
        }
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
            // Terminal bookkeeping is handled by the loop.
            TurnEvent::Usage { .. }
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
