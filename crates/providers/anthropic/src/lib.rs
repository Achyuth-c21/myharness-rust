//! The Claude API (`anthropic` family) adapter — native Anthropic Messages
//! against the first-party host (`docs/12` §1 row `anthropic`; `docs/13`
//! Phase 3 order item 2; `docs/10` §9 assigns native provider adapters to
//! `crates/providers/*`).
//!
//! Family contract, as recorded (`docs/09` §2.2 "Claude API"; `docs/18` §6):
//!
//! - Endpoint `api.anthropic.com/v1/messages` with `x-api-key` (or OAuth)
//!   auth; `anthropic-version: 2023-06-01`; the model list is `/v1/models`
//!   or the profile's static `inferenceModels` list.
//! - Optional base URL: the setup field's on-wire spelling is unverified
//!   (`UNVERIFIED_ANTHROPIC_BASE_URL` in `crates/config/src/forms.rs`;
//!   recorded in `docs/14` §9). When absent the first-party default host
//!   applies — `docs/12` §1 records `api.anthropic.com`.
//! - `sk-ant-oat…` tokens are Anthropic OAuth access tokens whose OAuth beta
//!   header value was never captured (`docs/09` §2.2; `docs/14` §9 item 23).
//!   This adapter refuses them — visibly — rather than guessing a header.
//!
//! An adapter is **not** a routing or translation layer (`docs/12` §8.1): it
//! speaks the Anthropic Messages protocol directly, and maps only between
//! that protocol and the app-internal model in `myharness-provider-sdk`
//! (modeling, not translation — the same wording `docs/12` §8.2 allows).
//! No code is shared with the *gateway* adapter beyond the shared contract
//! crate: each family owns its native client, its discovery, and its wire
//! fixtures.
//!
//! Boundaries (`docs/12` §11; identical in discipline to the gateway slice):
//! credentials arrive resolved in [`myharness_provider_sdk::AuthContext`] and
//! are re-resolved per request; the model policy gates every send; the model
//! id travels verbatim and an echo mismatch ends the turn; nothing here is
//! process-global.

pub mod adapter;
pub mod discovery;
pub mod messages;
#[cfg(test)]
mod tests;

pub use adapter::AnthropicAdapter;
pub use discovery::{discover_models, DiscoveredModel, DiscoveryOutcome};
pub use messages::{AnthropicClientConfig, ProbeStatus, DEFAULT_IDLE_TIMEOUT};

/// The first-party default host the family contract records
/// (`docs/12` §1: default host shape `api.anthropic.com`).
pub const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";

/// The `anthropic-version` header the Anthropic Messages contract sends
/// (`docs/09` §2.2: the shared Anthropic-compatible contract statement;
/// `docs/12` §3.1 names the value).
pub const ANTHROPIC_VERSION: &str = "2023-06-01";
