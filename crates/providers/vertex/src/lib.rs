//! The Google Vertex AI adapter — the `vertex` family's native protocol
//! client (`docs/12` §1 row `vertex`; `docs/13` Phase 3 order item 4). The
//! family carries the Anthropic Messages payload grammar over Google's
//! Vertex AI raw-prediction transport with Google-auth bearer credentials
//! (`docs/12` §8.5 "Anthropic Vertex client / Google auth").
//!
//! The family's wire was never observed from the shipped app (`docs/14` §7
//! row 100; the shipped implementation is minified); every reconstructive
//! spelling below is recorded as an approved decision in `docs/14` §9 item
//! 28 and pinned by the crate's wire fixtures:
//!
//! - **The base URL** is `https://{region}-aiplatform.googleapis.com`
//!   (the form records a region and an optional `inferenceVertexBaseUrl`
//!   override replaces it in full — no other override key exists).
//! - **The request path** is
//!   `POST {base}/v1/projects/{project}/locations/{region}/publishers/google/models/{model}:rawPredict`
//!   (and `:streamRawPredict` when streaming) — the standard Vertex AI
//!   raw-prediction routes; the model travels in the path and the body
//!   carries the Anthropic grammar **without a `model` field**, and without
//!   an `anthropic_version` declaration (no such field is recorded for this
//!   family; inventing one is the `AI-ADAPTER-10` failure shape).
//! - **The credential** is a Google access token sent as
//!   `Authorization: Bearer {material}`. The form's interactive/workforce/
//!   vendor-profile (`external-idp`) kinds have sign-in flows that were
//!   never captured: they are refused fail-closed upstream — the adapter
//!   takes a resolved token and never mints one (`docs/14` §9 item 28).
//!   The credentials *file* field is draft metadata only: the adapter never
//!   parses identity files (service-account JWT signing is an invented
//!   surface), it carries the resolved bearer material.
//! - **Streaming** is *data-only* — `data:` lines whose payload is one
//!   Anthropic stream-event JSON each, with no `event:` name lines and an
//!   optional `data: [DONE]` sentinel treated as transport. SSE comments
//!   and blank lines are transport keep-alives and never reset the idle
//!   window (`TEST-AI-016`).
//! - **The model echo is conditional** (as in the Bedrock reading): an
//!   answer that names a different model ends the turn visibly; absence of
//!   a `model` field is provider-dependent and not an echo failure
//!   (`AI-MODEL-01`; `docs/14` §9 item 28).
//! - **No native token count** — the surface exposes none; the adapter
//!   errors instead of estimating (`AI-ADAPTER-09`). **No model listing
//!   either**: the app disables model discovery for every credential kind
//!   this family offers (`docs/09` §2.2 "Vertex AI"), so the listing is an
//!   explicit refusal, never a guessed call (`AI-PROVIDER-05`).
//! - **No beta transport is recorded** for this family: a request that
//!   asks for beta identifiers is refused visibly rather than silently
//!   dropped (the same fail-closed reading as the Bedrock adapter).
//! - **The idle default** is the documented 300-second reading (`docs/12`
//!   §4.1; the family's own shipped default was not captured — `docs/14`
//!   §9 item 25's reading).

pub mod adapter;
pub mod messages;
pub(crate) mod sse;
#[cfg(test)]
mod tests;

pub use adapter::VertexAdapter;
pub use messages::{ProbeStatus, VertexClientConfig, DEFAULT_IDLE_TIMEOUT};

/// The default regional host (`docs/12` §1:
/// `{region}-aiplatform.googleapis.com`). An empty region is a
/// configuration error, not a request.
pub fn default_base_url(region: &str) -> Result<String, myharness_provider_sdk::ProviderError> {
    let region = region.trim();
    if region.is_empty() {
        return Err(myharness_provider_sdk::ProviderError {
            class: myharness_provider_sdk::ErrorClass::Configuration,
            status: None,
            message: "the GCP region is required for the Vertex host".to_string(),
        });
    }
    Ok(format!("https://{region}-aiplatform.googleapis.com"))
}
