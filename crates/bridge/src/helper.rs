//! The `settings.Custom3pHelperRun` namespace — roadmap Phase 2 (docs/13 §4
//! Phase 2; docs/18 §3.4; docs/12 §3.1–§3.3; docs/09 §3.4).
//!
//! Two setup-time probes live here:
//!
//! - **Model discovery** against an Anthropic-compatible gateway: the shipped
//!   protocol is `GET <gateway-base>/v1/models?limit=1000`, following
//!   `after_id` for at most three pages, with a 10-second timeout per page
//!   (`docs/12` §3.1). A later-page failure keeps the rows already read and
//!   warns; a first-page failure is an error. Trailing slashes are normalized,
//!   and models are de-duplicated by their stable id.
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

use std::collections::BTreeSet;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::setup::BridgeError;

/// The per-page timeout (`docs/12` §3.1: "Per-page timeout: 10 seconds").
pub const DISCOVERY_PAGE_TIMEOUT: Duration = Duration::from_secs(10);

/// The maximum number of pages (`docs/12` §3.1: "maximum 3 pages").
pub const DISCOVERY_MAX_PAGES: usize = 3;

/// The `anthropic-version` header the gateway contract sends
/// (`docs/09` §2.2: "`/v1/models` is called with `anthropic-version: 2023-06-01`").
pub const ANTHROPIC_VERSION: &str = "2023-06-01";

/// How the gateway consumes the credential (`docs/09` §2.2 auth scheme).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuthScheme {
    Bearer,
    XApiKey,
}

/// A model discovery request. This is a wire type: the setup window sends it
/// across the bridge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryRequest {
    /// The gateway base URL. Trailing slashes are normalized.
    pub base_url: String,
    /// The credential, if the gateway needs one. The user typed it; it is never
    /// logged and never stored by the probe.
    #[serde(default)]
    pub api_key: Option<String>,
    /// How the credential is presented.
    #[serde(default = "default_auth_scheme")]
    pub auth_scheme: AuthScheme,
    /// Routing/tenant headers (`docs/09` §2.2: custom inference headers).
    #[serde(default)]
    pub custom_headers: Vec<(String, String)>,
    /// The per-page timeout, in seconds. The documented value is 10.
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
    /// The maximum number of pages. The documented value is 3.
    #[serde(default = "default_max_pages")]
    pub max_pages: usize,
}

fn default_auth_scheme() -> AuthScheme {
    AuthScheme::Bearer
}

fn default_timeout_secs() -> u64 {
    DISCOVERY_PAGE_TIMEOUT.as_secs()
}

fn default_max_pages() -> usize {
    DISCOVERY_MAX_PAGES
}

/// One discovered model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredModel {
    /// The stable id — the deduplication key (`docs/12` §3.1).
    pub id: String,
    /// The raw row, kept so the catalog metadata can be joined onto it later.
    pub raw: Value,
}

/// The outcome of a discovery run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryOutcome {
    /// The models, de-duplicated by id.
    pub models: Vec<DiscoveredModel>,
    /// How many pages were read.
    pub pages: usize,
    /// True when a later page failed and the rows already read were kept
    /// (`docs/12` §3.1).
    pub partial: bool,
    /// The warning to show, if any.
    pub warning: Option<String>,
}

/// Run model discovery against an Anthropic-compatible gateway.
///
/// The protocol is the shipped one (`docs/12` §3.1): `GET <base>/v1/models?limit=1000`,
/// following the cursor for at most three pages, ten seconds per page. A
/// first-page failure is an error; a later-page failure keeps the rows already
/// read and warns.
pub async fn discover_models(
    request: &DiscoveryRequest,
    client: &reqwest::Client,
) -> Result<DiscoveryOutcome, BridgeError> {
    let base = request.base_url.trim().trim_end_matches('/');
    if base.is_empty() {
        return Err(BridgeError::new(
            "CONFIG_VALIDATION",
            "the gateway base URL is required for model discovery",
        ));
    }
    let url = format!("{base}/v1/models?limit=1000");
    let timeout = if request.timeout_secs == 0 {
        DISCOVERY_PAGE_TIMEOUT
    } else {
        Duration::from_secs(request.timeout_secs)
    };
    let max_pages = if request.max_pages == 0 {
        DISCOVERY_MAX_PAGES
    } else {
        request.max_pages
    };

    let mut models: Vec<DiscoveredModel> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut cursor: Option<String> = None;
    let mut pages = 0usize;
    let mut partial = false;
    let mut warning: Option<String> = None;

    loop {
        let page_url = match &cursor {
            Some(cursor) => format!("{url}&after_id={}", encode(cursor)),
            None => url.clone(),
        };
        let page = fetch_page(client, request, &page_url, timeout).await;
        let body = match page {
            Ok(body) => body,
            Err(error) => {
                if pages == 0 {
                    // The first page failing is an error (docs/12 §3.1).
                    return Err(error);
                }
                // A later page failing keeps what was read and warns.
                partial = true;
                warning = Some(error.message);
                break;
            }
        };
        pages += 1;

        let (rows, next) = parse_page(&body);
        for row in rows {
            let id = match model_id(&row) {
                Some(id) => id,
                None => continue,
            };
            if seen.insert(id.clone()) {
                models.push(DiscoveredModel { id, raw: row });
            }
        }

        match next {
            Some(next) if pages < max_pages => cursor = Some(next),
            Some(_) => {
                // The page cap was reached: the list is partial by contract, and the
                // docs say the app keeps what it read.
                partial = true;
                warning = Some(format!(
                    "the gateway advertised more models than the {}-page discovery limit",
                    max_pages
                ));
                break;
            }
            None => break,
        }
    }

    Ok(DiscoveryOutcome {
        models,
        pages,
        partial,
        warning,
    })
}

async fn fetch_page(
    client: &reqwest::Client,
    request: &DiscoveryRequest,
    url: &str,
    timeout: Duration,
) -> Result<Value, BridgeError> {
    let mut builder = client
        .get(url)
        .header("anthropic-version", ANTHROPIC_VERSION);
    if let Some(key) = &request.api_key {
        builder = match request.auth_scheme {
            AuthScheme::Bearer => builder.header("authorization", format!("Bearer {key}")),
            AuthScheme::XApiKey => builder.header("x-api-key", key.clone()),
        };
    }
    for (name, value) in &request.custom_headers {
        builder = builder.header(name, value);
    }
    let response = builder.timeout(timeout).send().await.map_err(|error| {
        BridgeError::new(
            "PROVIDER_UNREACHABLE",
            format!("the gateway could not be reached: {error}"),
        )
    })?;
    let status = response.status();
    if !status.is_success() {
        // The status is reported; the body is not — it can carry a token, and the
        // error envelope carries no raw body (docs/18 §11).
        return Err(BridgeError::new(
            "PROVIDER_ERROR",
            format!("the gateway answered with status {status}"),
        ));
    }
    response.json::<Value>().await.map_err(|error| {
        BridgeError::new(
            "PROVIDER_ERROR",
            format!("the gateway answer could not be read: {error}"),
        )
    })
}

/// The model rows of a page, and the cursor of the next page.
///
/// The response shape was not captured (`docs/20` records classes, not values),
/// so the parse is shape-tolerant: a `data` or `models` array of objects, an
/// `id` / `model` / `name` string on each row, and a cursor under `after_id` /
/// `next_cursor` / `last_id`. Nothing is invented.
fn parse_page(body: &Value) -> (Vec<Value>, Option<String>) {
    let rows = body
        .get("data")
        .or_else(|| body.get("models"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let cursor = ["after_id", "next_cursor", "last_id"]
        .iter()
        .find_map(|key| {
            body.get(*key)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        })
        .filter(|_| body.get("has_more").and_then(Value::as_bool) != Some(false));
    (rows, cursor)
}

fn model_id(row: &Value) -> Option<String> {
    ["id", "model", "name"]
        .iter()
        .find_map(|key| row.get(*key).and_then(Value::as_str))
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
}

/// Percent-encode a cursor value for a query string.
fn encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
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
