//! The Anthropic-compatible Gateway adapter — roadmap Phase 3 slice 1.
//!
//! This crate is the **native protocol wall** for the `gateway` provider
//! family (`docs/12` §1, `docs/10` §9). It speaks the gateway's own contract
//! and nothing else — there is no translation, routing, or compatibility
//! layer between the app and the provider (`RULES.md` §2 and
//! `AI-PROVIDER-01`/`AI-PROVIDER-02`, `AI-EXT-02`). The `AI-ADAPTER-01..14`
//! rows in `docs/12` §7 are the anti-checklist this implementation is tested
//! against.
//!
//! The wire contract (`docs/12` §3.1, §4.1, §8):
//!
//! - discovery: `GET <base>/v1/models?limit=1000`, `anthropic-version:
//!   2023-06-01`, cursor pagination capped at 3 pages, 10 s per page
//!   (`TEST-AI-005`, `TEST-AI-018`);
//! - inference: `POST <base>/v1/messages`, SSE event stream, idle timeout
//!   default 300 s (`TEST-AI-016`), native stop reasons preserved
//!   (`TEST-AI-014`), no silent substitution (`AI-MODEL-01`);
//! - token counting: `/v1/messages/count_tokens` (the Anthropic-native
//!   endpoint; never an estimate — `AI-ADAPTER-09`), else an explicit error;
//! - credentials arrive already resolved (keychain or helper; `crates/secrets`)
//!   and flow into pre-sendAuth headers only (`AI-PROVIDER-03`).
//!
//! Evidence discipline: the `/v1/models` grammar is captured (`docs/12` §3.1);
//! `/v1/messages` and `count_tokens` shapes are the shipped app's
//! Anthropic-compatible family contract — the strongest reading of "the
//! Gateway is a first-class provider whose contract is Anthropic Messages"
//! (`docs/12` §1). Readings that go further than the captured docs are
//! recorded in `docs/14` §9 and in `MEMORY.md` §7.
//!
//! Traceability: `TEST-AI-005..018`, `TEST-AI-021..023`,
//! `AI-PROVIDER-01..05`, `AI-MODEL-01/02`, `AI-STREAM-01/02`, `AI-TOOL-01`,
//! `AI-MM-01/02`, `AI-USAGE-01`.

pub mod adapter;
pub mod discovery;
pub mod messages;

pub use adapter::GatewayAdapter;
pub use discovery::{
    discover_models, AuthScheme, DiscoveredModel, DiscoveryOutcome, DiscoveryRequest,
};
pub use messages::{GatewayClientConfig, ProbeStatus, DEFAULT_IDLE_TIMEOUT};

#[cfg(test)]
mod tests;
