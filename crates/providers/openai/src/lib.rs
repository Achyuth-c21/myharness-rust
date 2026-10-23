//! The OpenAI Chat Completions adapter — the open model layer's native wall
//! (`docs/13` §C; owner decision recorded 2026-10-23 in `docs/14` §9).
//!
//! This crate is the **native protocol wall** for two provider families that
//! speak one wire:
//!
//! - `openai` — first-party OpenAI (`https://api.openai.com/v1`, static key);
//! - `custom` — any OpenAI-compatible endpoint the user names (base URL
//!   required, key optional: self-hosted servers may answer unauthenticated;
//!   the adapter sends the `authorization: Bearer …` header only when a
//!   credential is present).
//!
//! One wire, one adapter wall: `POST {base}/chat/completions`, `GET
//! {base}/models` for discovery. The app's internal conversation model maps
//! onto this wire inside this adapter — everywhere else the app speaks its
//! Anthropic-shaped internal model (`docs/12` §8.2: mapping a provider's
//! native events onto the app's typed set is application modeling; what is
//! forbidden is translating between providers' wires, `AI-PROVIDER-02`,
//! `AI-EXT-02`). There is no inference gateway and no router.
//!
//! Wire readings pinned for v1, each deliberate (`docs/14` §9):
//!
//! - system blocks join with `\n\n` into **one `role: "system"` message** —
//!   `role: "developer"` breaks OpenAI-compatible gateways that the shipped
//!   reasoning models disagree on, so v1 sends the shape every server
//!   accepts;
//! - `max_tokens` travels as **`max_tokens`** (not `max_completion_tokens`)
//!   for the same tolerance reason;
//! - `stream_options` is **not sent** (some compatible servers reject it);
//!   usage is parsed from chunks that carry it anyway;
//! - `top_k`, thinking budgets, Anthropic betas, and `cache_control` are
//!   **dropped**, visibly recorded — this wire has no spelling for them;
//! - images travel as **`image_url`** data URIs on user turns; documents are
//!   an **explicit `UnsupportedModality`** refusal (`AI-MM-01`: fail-closed,
//!   never a silent drop); images inside tool results are the same refusal
//!   (strictest-compatible v1 content form: text-only tool messages);
//! - assistant `Reasoning`/`RedactedThinking` parts are **omitted from
//!   outgoing history** — the wire cannot replay them, so replaying garbage
//!   is worse than replaying nothing (recorded);
//! - tool results lose their `is_error` flag (no spelling on this wire; the
//!   result text itself travels, so the model still sees the failure);
//! - token counting is **an explicit refusal** — this wire offers no native
//!   counting endpoint, and an estimate is forbidden (`AI-ADAPTER-09`);
//! - the model echo is a **prefix match**: dated revisions
//!   (`gpt-4o-mini-2024-07-18` for `gpt-4o-mini`) stay legal; a genuinely
//!   different model name remains a user-visible refusal (`AI-MODEL-01`,
//!   `AI-ADAPTER-11`).
//!
//! Boundaries this crate holds (`docs/12` §11): credentials arrive resolved
//! in an [`AuthContext`] and exist only as request-header material
//! (`AI-PROVIDER-03`); a concrete [`reqwest::Client`] is injected per
//! adapter, so nothing here is process-global (`AI-ADAPTER-13`); model ids
//! travel verbatim, ungated — the only identity checks are structural
//! (`docs/14` §9 owner decision, 2026-10-23).
//!
//! Traceability: `TEST-AI-021..023`, `AI-PROVIDER-01..05`, `AI-MODEL-01`,
//! `AI-STREAM-01/02`, `AI-TOOL-01`, `AI-MM-01/02`, `AI-USAGE-01`.

pub mod adapter;
pub mod chat;
pub mod discovery;

pub use adapter::{AdapterKind, OpenAiCompatAdapter};
pub use chat::{OpenAiClientConfig, ProbeStatus, DEFAULT_IDLE_TIMEOUT};
pub use discovery::{
    discover_models, DiscoveredModel, DiscoveryOutcome, DiscoveryRequest, OPENAI_DISCOVERY_TIMEOUT,
};

/// The first-party OpenAI base URL, used when the `openai` family's profile
/// leaves the base URL empty. The `custom` family has no default — its base
/// URL is the profile's core field (`docs/14` §9 owner decision).
pub const DEFAULT_OPENAI_BASE: &str = "https://api.openai.com/v1";
