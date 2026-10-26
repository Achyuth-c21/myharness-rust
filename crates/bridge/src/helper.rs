//! The `settings.Custom3pHelperRun` namespace — roadmap Phase 2 (docs/13 §4
//! Phase 2; docs/18 §3.4; docs/12 §3.1–§3.3; docs/09 §3.4).
//!
//! Two setup-time probes pass through here:
//!
//! - **Model discovery** against an Anthropic-compatible gateway: the shipped
//!   protocol is `GET <gateway-base>/v1/models?limit=1000`, following
//!   `after_id` for at most three pages, with a 10-second timeout per page
//!   (`docs/12` §3.1). A later-page failure keeps the rows already read and
//!   warns; a first-page failure is an error. The implementation lives in the
//!   native provider adapter (`docs/10` §9); this module re-exports its wire
//!   types unchanged and maps the adapter's error classes onto the setup
//!   bridge's codes with the exact provider messages.
//! - **The published model catalog**: a signed document stored as a base64
//!   payload plus a sidecar (`docs/11` §14). On any failure the app must use a
//!   bundled catalog and show a non-blocking staleness warning (`docs/12` §3.3).
//!
//! What is NOT here, and why:
//!
//! - **The catalog signature cannot be verified in this rebuild.** The shipped
//!   verification is "RSA PKCS#1 v1.5 SHA-512 signature" plus a "public key
//!   SHA-256" check (`docs/12` §3.3), but the public key itself was never
//!   captured — there is nothing to verify against. A verifier that accepted an
//!   unknown key would be a policy bypass, so the catalog is treated as
//!   **unverified** and the documented failure path (the bundled fallback plus
//!   a staleness warning) is taken (`docs/14` §9 item 20).
//! - **The inference probe** (a live request to the provider) belongs to the
//!   provider adapters of Phase 3, not to a setup probe.
//!
//! The discovery client is a **probe of a provider-native endpoint**. It is not
//! a translation layer: it speaks the gateway's own `/v1/models` contract and
//! nothing else (`RULES.md` §2, `docs/13` non-goals).

use serde::{Deserialize, Serialize};

use crate::setup::BridgeError;

pub use myharness_provider_gateway::discovery::{
    discover_models as adapter_discover_models, AuthScheme, DiscoveredModel, DiscoveryOutcome,
    DiscoveryRequest,
};
pub use myharness_provider_gateway::discovery::{
    ANTHROPIC_VERSION, DISCOVERY_MAX_PAGES, DISCOVERY_PAGE_TIMEOUT,
};

pub use myharness_provider_openai::discovery::{
    discover_models as openai_adapter_discover_models, DiscoveryOutcome as OpenAiDiscoveryOutcome,
    DiscoveryRequest as OpenAiDiscoveryRequest,
};

pub use myharness_provider_openai_responses::discovery::{
    discover_models as responses_adapter_discover_models,
    DiscoveryRequest as ResponsesDiscoveryRequest,
};

/// Run model discovery against an Anthropic-compatible gateway (the setup
/// probe path).
///
/// The behavior is the adapter's own discovery (`docs/12` §3.1, `docs/10` §9):
/// `GET <base>/v1/models?limit=1000`, cursor pagination capped at three
/// pages, ten seconds per page, first-page failure fatal, later-page failure
/// retaining the rows already read. This wrapper exists so the setup window's
/// probe path keeps its wire types and error codes: the adapter's error
/// classes map onto `CONFIG_VALIDATION` / `PROVIDER_UNREACHABLE` /
/// `PROVIDER_ERROR` with the provider's message verbatim — the same code
/// strings the setup window matches on. No error envelope ever carries a raw
/// response body (`docs/18` §11).
pub async fn discover_models(
    request: &DiscoveryRequest,
    client: &reqwest::Client,
) -> Result<DiscoveryOutcome, BridgeError> {
    // The mapping is the setup bridge's one mapper: the provider's message
    // verbatim, the codes the setup window matches on (`docs/18` §11).
    adapter_discover_models(request, client)
        .await
        .map_err(crate::setup::bridge_provider_error)
}

/// Run model discovery against an OpenAI-compatible endpoint — the open
/// model layer's families (`docs/13` §C: first-party `openai` and the
/// user-defined `custom` endpoint, key optional).
///
/// The same wrapper posture as the gateway probe: the adapter's classes map
/// onto the setup bridge's codes with the provider's message verbatim
/// (`docs/18` §11); a keyless `custom` request sends no `authorization`
/// header (`docs/14` §9 owner decision), and no error envelope ever carries
/// a raw response body.
pub async fn discover_openai_models(
    request: &OpenAiDiscoveryRequest,
    client: &reqwest::Client,
) -> Result<OpenAiDiscoveryOutcome, BridgeError> {
    openai_adapter_discover_models(request, client)
        .await
        .map_err(crate::setup::bridge_provider_error)
}

/// Run model discovery against the Responses wall's listing (Phase 8 slice
/// 2; `docs/13` Phase 8; the readings are `docs/14` §9 item 71). The same
/// wrapper posture as the other two probes: the adapter's classes map onto
/// the setup bridge's codes with the provider's message verbatim
/// (`docs/18` §11), and the wall's own outcome shape (single page, tolerant
/// parse warnings) maps onto the shared setup outcome — one page, never
/// partial, the first warning surfaced.
pub async fn discover_responses_models(
    request: &ResponsesDiscoveryRequest,
    client: &reqwest::Client,
) -> Result<DiscoveryOutcome, BridgeError> {
    let outcome = responses_adapter_discover_models(request, client)
        .await
        .map_err(crate::setup::bridge_provider_error)?;
    Ok(DiscoveryOutcome {
        models: outcome
            .models
            .into_iter()
            .map(|model| DiscoveredModel {
                id: model.id,
                raw: model.raw,
            })
            .collect(),
        pages: 1,
        partial: false,
        warning: outcome.warnings.into_iter().next(),
    })
}

// ---------------------------------------------------------------------------
// The published model catalog
// ---------------------------------------------------------------------------

/// The stored catalog envelope (`docs/11` §14 `model-catalog/published.json`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogEnvelope {
    pub format: u32,
    #[serde(default)]
    pub location_hash: String,
    pub url: String,
    #[serde(default)]
    pub fetched_at: u64,
    #[serde(default)]
    pub stale_at: u64,
    #[serde(default)]
    pub etag: String,
    /// The base64 payload of the signed document.
    #[serde(default)]
    pub document_bytes: String,
    /// The verification sidecar.
    #[serde(default)]
    pub sidecar: Option<CatalogSidecar>,
    #[serde(default)]
    pub version: u64,
}

/// The verification sidecar (`docs/11` §14).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSidecar {
    pub schema: u32,
    pub algorithm: String,
    pub signature: String,
    pub public_key_sha256: String,
}

/// The freshness of the stored catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CatalogFreshness {
    /// Fetched, within its validity window, and verified.
    Fresh,
    /// Fetched, within its validity window, but **not verified** — the rebuild has
    /// no public key to verify against (`docs/14` §9 item 20).
    Unverified,
    /// Past its stale-at timestamp: usable, with a staleness warning.
    Stale,
    /// Past its expiry: not usable.
    Expired,
    /// Nothing is stored: the bundled fallback is used.
    Absent,
}

/// The state of the catalog the app should use.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogState {
    pub freshness: CatalogFreshness,
    /// True when the bundled catalog must be used instead of the stored one.
    pub use_bundled_fallback: bool,
    /// The non-blocking staleness warning to show (`docs/12` §3.3).
    pub warning: Option<String>,
}

/// Decide which catalog to use.
///
/// The rule is the documented one: "On failure, the app must use a bundled
/// catalog and show a non-blocking staleness warning" (`docs/12` §3.3). The
/// rebuild additionally treats an **unverified** stored catalog as a failure,
/// because the shipped public key was never captured and an unverified catalog
/// must not silently become the source of truth.
pub fn catalog_state(
    envelope: Option<&CatalogEnvelope>,
    now_epoch_secs: u64,
    signature_verified: bool,
) -> CatalogState {
    let Some(envelope) = envelope else {
        return CatalogState {
            freshness: CatalogFreshness::Absent,
            use_bundled_fallback: true,
            warning: Some(
                "no model catalog has been downloaded; the bundled catalog is in use".to_string(),
            ),
        };
    };
    if envelope.stale_at != 0 && now_epoch_secs > envelope.stale_at {
        return CatalogState {
            freshness: CatalogFreshness::Expired,
            use_bundled_fallback: true,
            warning: Some(
                "the model catalog has expired; the bundled catalog is in use".to_string(),
            ),
        };
    }
    // The sidecar must be well-formed, and the signature must have been checked.
    // The signature cannot be checked without the shipped public key
    // (`docs/14` §9 item 20), so the caller passes `signature_verified = false`
    // today and a stored catalog is never "verified".
    let sidecar_present = envelope.sidecar.as_ref().is_some_and(|s| {
        s.schema == 1 && s.algorithm == "RSASSA-PKCS1-v1_5-SHA512" && !s.signature.is_empty()
    });
    if !(sidecar_present && signature_verified) {
        return CatalogState {
            freshness: CatalogFreshness::Unverified,
            use_bundled_fallback: true,
            warning: Some(
                "the model catalog could not be verified; the bundled catalog is in use"
                    .to_string(),
            ),
        };
    }
    CatalogState {
        freshness: CatalogFreshness::Fresh,
        use_bundled_fallback: false,
        warning: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// A tiny HTTP server that answers each request with the next canned body.
    fn mock_server(bodies: Vec<(u16, String)>) -> (String, std::thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().unwrap().port();
        let handle = std::thread::spawn(move || {
            let mut requests = Vec::new();
            for (status, body) in bodies {
                let Ok((mut stream, _)) = listener.accept() else {
                    break;
                };
                let mut buffer = [0u8; 4096];
                let read = stream.read(&mut buffer).unwrap_or(0);
                requests.push(String::from_utf8_lossy(&buffer[..read]).to_string());
                let response = format!(
                    "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            }
            requests
        });
        (format!("http://127.0.0.1:{port}"), handle)
    }

    fn request(base_url: String) -> DiscoveryRequest {
        DiscoveryRequest {
            base_url,
            api_key: Some("placeholder-credential".to_string()),
            auth_scheme: AuthScheme::Bearer,
            custom_headers: vec![("X-Tenant".to_string(), "acme".to_string())],
            timeout_secs: 5,
            max_pages: DISCOVERY_MAX_PAGES,
        }
    }

    /// docs/12 §3.1: discovery reads `/v1/models?limit=1000` and follows the
    /// cursor.
    #[tokio::test]
    async fn discovery_reads_the_documented_endpoint_and_follows_the_cursor() {
        let (base, server) = mock_server(vec![
            (
                200,
                json!({
                    "data": [{ "id": "claude-opus-4-1" }, { "id": "claude-sonnet-4" }],
                    "has_more": true,
                    "after_id": "claude-sonnet-4",
                })
                .to_string(),
            ),
            (
                200,
                json!({
                    "data": [{ "id": "claude-haiku-4" }],
                    "has_more": false,
                })
                .to_string(),
            ),
        ]);
        let client = reqwest::Client::new();
        let outcome = discover_models(&request(base.clone()), &client)
            .await
            .unwrap();
        assert_eq!(outcome.pages, 2);
        assert_eq!(outcome.models.len(), 3);
        assert!(!outcome.partial);
        assert_eq!(
            outcome
                .models
                .iter()
                .map(|m| m.id.as_str())
                .collect::<Vec<_>>(),
            vec!["claude-opus-4-1", "claude-sonnet-4", "claude-haiku-4"]
        );

        let requests = server.join().unwrap();
        assert!(
            requests[0].contains("/v1/models?limit=1000"),
            "{}",
            requests[0]
        );
        assert!(
            requests[0].contains("anthropic-version: 2023-06-01"),
            "{}",
            requests[0]
        );
        assert!(requests[0].contains("authorization: Bearer placeholder-credential"));
        assert!(
            requests[1].contains("after_id=claude-sonnet-4"),
            "{}",
            requests[1]
        );
    }

    /// A later-page failure keeps the rows already read and warns.
    #[tokio::test]
    async fn a_later_page_failure_keeps_what_was_read() {
        let (base, _server) = mock_server(vec![
            (
                200,
                json!({ "data": [{ "id": "claude-opus-4-1" }], "has_more": true, "after_id": "x" })
                    .to_string(),
            ),
            (500, json!({ "error": "boom" }).to_string()),
        ]);
        let client = reqwest::Client::new();
        let outcome = discover_models(&request(base), &client).await.unwrap();
        assert!(outcome.partial);
        assert_eq!(outcome.models.len(), 1);
        assert!(outcome.warning.is_some());
    }

    /// A first-page failure is an error, not an empty result.
    #[tokio::test]
    async fn a_first_page_failure_is_an_error() {
        let (base, _server) = mock_server(vec![(500, json!({ "error": "boom" }).to_string())]);
        let client = reqwest::Client::new();
        let error = discover_models(&request(base), &client).await.unwrap_err();
        assert_eq!(error.code, "PROVIDER_ERROR");
        // The body never reaches the message (docs/18 §11).
        assert!(!error.message.contains("boom"));
    }

    /// The page cap is three, and reaching it is reported as partial.
    #[tokio::test]
    async fn the_page_cap_is_three() {
        let (base, _server) = mock_server(vec![
            (
                200,
                json!({ "data": [{ "id": "a" }], "has_more": true, "after_id": "a" }).to_string(),
            ),
            (
                200,
                json!({ "data": [{ "id": "b" }], "has_more": true, "after_id": "b" }).to_string(),
            ),
            (
                200,
                json!({ "data": [{ "id": "c" }], "has_more": true, "after_id": "c" }).to_string(),
            ),
        ]);
        let client = reqwest::Client::new();
        let outcome = discover_models(&request(base), &client).await.unwrap();
        assert_eq!(outcome.pages, 3);
        assert!(outcome.partial);
        assert!(outcome.warning.is_some());
    }

    /// Duplicate ids collapse (docs/12 §3.1: "deduplicate by stable id").
    #[tokio::test]
    async fn duplicate_ids_collapse() {
        let (base, _server) = mock_server(vec![(
            200,
            json!({
                "data": [{ "id": "claude-opus-4-1" }, { "id": "claude-opus-4-1" }],
                "has_more": false,
            })
            .to_string(),
        )]);
        let client = reqwest::Client::new();
        let outcome = discover_models(&request(base), &client).await.unwrap();
        assert_eq!(outcome.models.len(), 1);
    }

    /// The catalog falls back to the bundled copy whenever it cannot be trusted.
    #[test]
    fn an_unverified_catalog_falls_back_to_the_bundled_copy() {
        let envelope = CatalogEnvelope {
            format: 1,
            location_hash: "hash".to_string(),
            url: "https://downloads.claude.ai/model-catalog/v1/catalog.json".to_string(),
            fetched_at: 100,
            stale_at: 1_000,
            etag: "etag".to_string(),
            document_bytes: "base64".to_string(),
            sidecar: Some(CatalogSidecar {
                schema: 1,
                algorithm: "RSASSA-PKCS1-v1_5-SHA512".to_string(),
                signature: "sig".to_string(),
                public_key_sha256: "sha".to_string(),
            }),
            version: 2707,
        };
        // The sidecar is well-formed, but the signature cannot be checked without
        // the shipped public key (docs/14 §9 item 20): the catalog is unverified.
        let state = catalog_state(Some(&envelope), 200, false);
        assert_eq!(state.freshness, CatalogFreshness::Unverified);
        assert!(state.use_bundled_fallback);
        assert!(state.warning.is_some());
    }

    /// A catalog whose signature has actually been checked is used as it stands.
    #[test]
    fn a_verified_catalog_is_used() {
        let envelope = CatalogEnvelope {
            format: 1,
            location_hash: "hash".to_string(),
            url: "https://downloads.claude.ai/model-catalog/v1/catalog.json".to_string(),
            fetched_at: 100,
            stale_at: 1_000,
            etag: "etag".to_string(),
            document_bytes: "base64".to_string(),
            sidecar: Some(CatalogSidecar {
                schema: 1,
                algorithm: "RSASSA-PKCS1-v1_5-SHA512".to_string(),
                signature: "sig".to_string(),
                public_key_sha256: "sha".to_string(),
            }),
            version: 2707,
        };
        let state = catalog_state(Some(&envelope), 200, true);
        assert_eq!(state.freshness, CatalogFreshness::Fresh);
        assert!(!state.use_bundled_fallback);
        assert!(state.warning.is_none());
    }

    /// An expired catalog is not used, and an absent one falls back.
    #[test]
    fn an_expired_or_absent_catalog_falls_back() {
        let envelope = CatalogEnvelope {
            format: 1,
            location_hash: String::new(),
            url: "https://downloads.claude.ai/model-catalog/v1/catalog.json".to_string(),
            fetched_at: 100,
            stale_at: 200,
            etag: String::new(),
            document_bytes: String::new(),
            sidecar: None,
            version: 1,
        };
        let state = catalog_state(Some(&envelope), 300, true);
        assert_eq!(state.freshness, CatalogFreshness::Expired);
        assert!(state.use_bundled_fallback);

        let state = catalog_state(None, 300, true);
        assert_eq!(state.freshness, CatalogFreshness::Absent);
        assert!(state.use_bundled_fallback);
    }
}
