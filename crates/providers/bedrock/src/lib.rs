//! The Bedrock (`bedrock` family) adapter — native Anthropic Messages over the
//! AWS Bedrock Runtime surface (`docs/12` §1 row `bedrock`; `docs/13` Phase 3
//! order item 3; `docs/10` §9 assigns native provider adapters to
//! `crates/providers/*`).
//!
//! Family contract, as recorded (`docs/09` §2.2 "Bedrock"; `docs/18` §6;
//! `docs/12` §1/§2.2): the host shape `bedrock-runtime[-fips].<region>.
//! amazonaws.com` (control plane `bedrock.<region>.amazonaws.com`), static
//! bearer / profile-SSO / web-identity / helper / vendor credentials, optional
//! base URL, and the recorded model-ID shapes `anthropic.*` or Bedrock
//! inference / provisioned ARNs — which, since the owner's open model layer
//! (2026-10-23, `docs/14` §9), travel verbatim instead of being gated:
//! `docs/12` §2.2's family predicates no longer govern the rebuild.
//! `docs/12` §8.5 maps this
//! family to "AWS SDK Bedrock Runtime stream, Anthropic body": the payload
//! grammar is the Anthropic Messages one, in this rebuild hand-built and
//! wire-tested against this crate's own native client — no AWS SDK, no shared
//! code with any other adapter crate beyond the contract crate.
//!
//! The exact request paths, headers, and stream framing were **never
//! captured** for this family (`docs/14` §7 "Exact provider request paths";
//! the shipped implementation is minified). This adapter therefore
//! reconstructs the family's native surface from the recorded contract and
//! records every reconstructive spelling as an approved decision with its
//! basis (`docs/14` §9, wire fixtures pin the choice) — `Unknown` is an
//! answer, and a reconstruction is labeled as one.
//!
//! Boundaries this crate holds (`docs/12` §11; identical discipline to the
//! gateway and Claude API slices):
//!
//! - **Credentials** arrive resolved in an [`myharness_provider_sdk::
//!   AuthContext`], re-resolved by the caller per request — rotation without
//!   a second credential source (`AI-PROVIDER-03`; the `AI-ADAPTER-12`
//!   regression). Profile/SSO, web-identity, and vendor-profile sign-in flows
//!   were never observed, so this rebuild refuses them fail-closed instead of
//!   emulating an identity flow (`docs/14` §9).
//! - **Model ids travel verbatim, ungated** since the owner's open model
//!   layer (2026-10-23, `docs/14` §9): the recorded family shapes are now
//!   descriptive, not a policy gate.
//! - **No substitution**: the model id or ARN travels verbatim in the request
//!   path (`AI-ADAPTER-11`); an answer that names a different model ends the
//!   turn visibly (`AI-MODEL-01`).
//! - **Nothing global** (`AI-ADAPTER-13`): a concrete [`reqwest::Client`] is
//!   injected per adapter; there is no fetch interception, no module state.

pub mod adapter;
pub mod discovery;
pub mod eventstream;
pub mod messages;
#[cfg(test)]
mod tests;

pub use adapter::BedrockAdapter;
pub use discovery::{discover_models, DiscoveredModel, DiscoveryOutcome};
pub use messages::{BedrockClientConfig, ProbeStatus, DEFAULT_IDLE_TIMEOUT};

/// The runtime default host for a region (`docs/12` §1: host shape
/// `bedrock-runtime[-fips].<region>.amazonaws.com`; the non-FIPS host is the
/// spelling the region field yields — no FIPS flag exists in the captured
/// form, so none is invented).
pub fn default_runtime_base_url(
    region: &str,
) -> Result<String, myharness_provider_sdk::ProviderError> {
    let region = region.trim();
    if region.is_empty() {
        return Err(myharness_provider_sdk::ProviderError {
            class: myharness_provider_sdk::ErrorClass::Configuration,
            status: None,
            message: "the Bedrock region is required to form the runtime endpoint".to_string(),
        });
    }
    Ok(format!("https://bedrock-runtime.{region}.amazonaws.com"))
}

/// The control-plane default host for a region (`docs/12` §1 records
/// `bedrock.<region>.amazonaws.com` among the family's control-plane hosts;
/// the model APIs live there).
pub fn default_control_base_url(
    region: &str,
) -> Result<String, myharness_provider_sdk::ProviderError> {
    let region = region.trim();
    if region.is_empty() {
        return Err(myharness_provider_sdk::ProviderError {
            class: myharness_provider_sdk::ErrorClass::Configuration,
            status: None,
            message: "the Bedrock region is required to form the control-plane endpoint"
                .to_string(),
        });
    }
    Ok(format!("https://bedrock.{region}.amazonaws.com"))
}

/// The `anthropic_version` value the Bedrock Runtime's Anthropic payload
/// grammar requires. The Messages API version header value (`2023-06-01`) is
/// a first-party spelling; Bedrock's body value is a distinct, separately
/// documented spelling. Reconstruction basis: `docs/12` §8.5 "Anthropic
/// body"; the value itself is part of the `docs/14` §9 approved decision.
pub const ANTHROPIC_VERSION_BEDROCK: &str = "bedrock-2023-05-31";
