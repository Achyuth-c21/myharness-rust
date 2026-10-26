//! The OpenAI **Responses** adapter — Phase 8's first reference extension
//! (`docs/13` §"Phase 8"; `docs/12` §8 adapter contract).
//!
//! This crate is the **native protocol wall** for the OpenAI Responses wire:
//! `POST {base}/responses` with its own input-item grammar and its own SSE
//! event stream (`response.output_text.delta` and friends). It is a different
//! native protocol from the Chat Completions wall in
//! `myharness-provider-openai` — a sibling crate sharing a design, never code,
//! and never a translation between the two wires (`AI-PROVIDER-02`,
//! `AI-EXT-02`; `docs/13` Phase 8: "a provider that only speaks OpenAI Chat
//! Completions is supported by an explicit adapter for that native protocol,
//! not by converting Anthropic requests" — the same principle one wire over).
//!
//! Everywhere else the app speaks its Anthropic-shaped internal model; the
//! mapping onto this wire happens only here (`docs/12` §8.2: application
//! modeling, not cross-protocol translation).
//!
//! Wire readings pinned for v1 — this wire was never observed in the shipped
//! app (Phase 8 extensions are rebuild additions), so every reading below is
//! a rebuild-defined choice recorded in `docs/14` §9:
//!
//! - **`store: false` is sent explicitly.** The Responses default retains
//!   the conversation server-side; a desktop harness that keeps its own
//!   store does not opt into a second one by omission.
//! - System blocks join with `\n\n` into the single **`instructions`**
//!   string; `cache_control` has no spelling here and is dropped, recorded.
//! - Documents map **natively** to `input_file` (base64 data URI) — this
//!   wire carries PDFs where Chat Completions cannot; images map to
//!   `input_image` data URIs.
//! - Media inside tool results is **refused** (`UnsupportedModality`) on v1:
//!   the result-content spelling for media was not pinned when this adapter
//!   was written, and a guessed shape that the endpoint then mangles is
//!   worse than an explicit refusal. Text tool results travel as
//!   `function_call_output`; `is_error` has no spelling — the result text
//!   carries the failure.
//! - Assistant `Reasoning`/`RedactedThinking` parts are **omitted from
//!   replay**: this wire replays reasoning only through encrypted items the
//!   app does not hold, and replaying nothing beats replaying garbage.
//! - Thinking budgets, `top_k`, stop sequences, and betas are **dropped**,
//!   visibly recorded — no Responses spelling is pinned for any of them.
//! - The model echo is a **prefix match** (a dated revision of the requested
//!   id stays legal; a different model stops the turn — `AI-MODEL-01`).
//! - Usage maps `input_tokens`/`output_tokens` and
//!   `input_tokens_details.cached_tokens` → cache-read;
//!   `output_tokens_details.reasoning_tokens` is a subset of output and has
//!   no app channel — dropped, recorded (`AI-USAGE-01`: exact, never
//!   estimated).
//! - **Stop reasons**: `status: "completed"` maps to `end_turn`, or
//!   `tool_use` when the turn emitted function calls (this wire's terminal
//!   event carries no per-choice `finish_reason`; the emitted items are the
//!   equivalent fact); `incomplete_details.reason: "max_output_tokens"` →
//!   `max_tokens`; `"content_filter"` → `content_filter`; anything else
//!   verbatim (`AI-ADAPTER-08`).
//! - **Stream termination**: unlike Chat Completions there is no
//!   `[DONE]` sentinel — the terminal events are `response.completed`,
//!   `response.failed`, and `error`. A stream that ends without one is
//!   contract-malformed: no stop reason is invented. Documented lifecycle
//!   events with no app mapping (`response.in_progress`, the
//!   `content_part`/`summary_part` `.added`/`.done` pairs, the
//!   `output_text.done`/`summary_text.done` duplicates) are consumed as
//!   no-ops; an **undocumented** event type is surfaced as an error — a
//!   grammar the adapter does not know may be carrying content it would
//!   silently drop (`AI-MM-01`).
//! - Annotations on `output_text` parts (this wire's citation spelling)
//!   surface as [`TurnEvent::Citation`] verbatim (`AI-STREAM-01`).
//! - Token counting is **an explicit refusal** — no native counting
//!   endpoint, and an estimate is forbidden (`AI-ADAPTER-09`).
//!
//! Boundaries this crate holds (`docs/12` §11): credentials arrive resolved
//! in an [`myharness_provider_sdk::AuthContext`] and exist only as
//! request-header material (`AI-PROVIDER-03`); a concrete
//! [`reqwest::Client`] is injected per adapter, so nothing here is
//! process-global (`AI-ADAPTER-13`); model ids travel verbatim, ungated —
//! the only identity checks are structural (`docs/14` §9 owner decision,
//! 2026-10-23).
//!
//! Traceability: `TEST-EXT-003/005/006/008`, `AI-PROVIDER-01..05`,
//! `AI-MODEL-01`, `AI-STREAM-01/02`, `AI-TOOL-01`, `AI-MM-01/02`,
//! `AI-USAGE-01`.

pub mod adapter;
pub mod discovery;
pub mod responses;

pub use adapter::ResponsesAdapter;
pub use responses::{ProbeStatus, ResponsesClientConfig, DEFAULT_IDLE_TIMEOUT};

/// The first-party OpenAI base URL, used when the profile leaves the base
/// URL empty. The Responses wire shares the platform base with Chat
/// Completions (`https://api.openai.com/v1`).
pub const DEFAULT_OPENAI_BASE: &str = "https://api.openai.com/v1";
