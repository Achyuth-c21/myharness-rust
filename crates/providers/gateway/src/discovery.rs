//! Gateway model discovery — `docs/12` §3.1 (the `TEST-AI-005` discovery half
//! + `TEST-AI-018`).
//!
//! The client was built in Phase 2 as a setup-time probe (`crates/bridge`
//! `Custom3pHelperRun`) and moved here because `docs/10` §9 assigns discovery
//! to `providers`. `crates/bridge` now delegates to this module; the wire
//! behavior, error codes, and test battery are unchanged.
//!
//! The shipped protocol is `GET <gateway-base>/v1/models?limit=1000`, following
//! `after_id` for at most three pages, with a 10-second timeout per page. A
//! later-page failure keeps the rows already read and warns; a first-page
//! failure is an error. Trailing slashes are normalized, and models are
//! de-duplicated by their stable id.
//!
//! This is a **probe of a provider-native endpoint**. It is not a translation
//! layer: it speaks the gateway's own `/v1/models` contract and nothing else
//! (`RULES.md` §2, `docs/13` non-goals).

use std::collections::BTreeSet;
use std::time::Duration;

use myharness_provider_sdk::{ErrorClass, ProviderError};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The per-page timeout (`docs/12` §3.1: "Per-page timeout: 10 seconds").
pub const DISCOVERY_PAGE_TIMEOUT: Duration = Duration::from_secs(10);

/// The maximum number of pages (`docs/12` §3.1: "maximum 3 pages").
pub const DISCOVERY_MAX_PAGES: usize = 3;

/// The `anthropic-version` header the gateway contract sends
/// (`docs/09` §2.2: "`/v1/models` is called with `anthropic-version:
/// 2023-06-01`").
pub const ANTHROPIC_VERSION: &str = "2023-06-01";

/// How the gateway consumes the credential (`docs/09` §2.2 auth scheme; the
/// form select offers exactly `bearer` and `x-api-key`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuthScheme {
    Bearer,
    XApiKey,
}

/// A model discovery request. This is a wire type: the setup window sends it
/// across the bridge, and the adapter's [`crate::GatewayAdapter`] runs the
/// same discovery against a resolved credential — see [`discover_pages`],
/// which keeps credential material out of wire types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryRequest {
    /// The gateway base URL. Trailing slashes are normalized.
    pub base_url: String,
    /// The credential, if the gateway needs one. The user typed it; it is never
    /// logged and never stored by the probe (`docs/09` §4).
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
///
/// This is the wire-type entry point (the setup-window probe path). The
/// adapter path uses [`discover_pages`] with the credential the caller
/// resolved, so no secret material travels inside a wire type.
pub async fn discover_models(
    request: &DiscoveryRequest,
    client: &reqwest::Client,
) -> Result<DiscoveryOutcome, ProviderError> {
    discover_pages(
        &request.base_url,
        request.auth_scheme,
        request.api_key.clone(),
        &request.custom_headers,
        client,
        request.timeout_secs,
        request.max_pages,
    )
    .await
}

/// The adapter-side discovery entry: the same native protocol run with a
/// credential the caller resolved per request (rotation). The credential
/// material exists only as this page's request-header content
/// (`AI-PROVIDER-03`) — never logged, never persisted.
pub async fn discover_pages(
    base_url: &str,
    scheme: AuthScheme,
    credential: Option<String>,
    custom_headers: &[(String, String)],
    client: &reqwest::Client,
    timeout_secs: u64,
    max_pages: usize,
) -> Result<DiscoveryOutcome, ProviderError> {
    let base = base_url.trim().trim_end_matches('/');
    if base.is_empty() {
        return Err(ProviderError {
            class: ErrorClass::Configuration,
            status: None,
            message: "the gateway base URL is required for model discovery".to_string(),
        });
    }
    let url = format!("{base}/v1/models?limit=1000");
    let timeout = if timeout_secs == 0 {
        DISCOVERY_PAGE_TIMEOUT
    } else {
        Duration::from_secs(timeout_secs)
    };
    let max_pages = if max_pages == 0 {
        DISCOVERY_MAX_PAGES
    } else {
        max_pages
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
        let page = fetch_page(
            client,
            scheme,
            &credential,
            custom_headers,
            &page_url,
            timeout,
        )
        .await;
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
    scheme: AuthScheme,
    credential: &Option<String>,
    custom_headers: &[(String, String)],
    url: &str,
    timeout: Duration,
) -> Result<Value, ProviderError> {
    let mut builder = client
        .get(url)
        .header("anthropic-version", ANTHROPIC_VERSION);
    if let Some(key) = credential {
        builder = match scheme {
            AuthScheme::Bearer => builder.header("authorization", format!("Bearer {key}")),
            AuthScheme::XApiKey => builder.header("x-api-key", key),
        };
    }
    for (name, value) in custom_headers {
        builder = builder.header(name, value);
    }
    let response = builder
        .timeout(timeout)
        .send()
        .await
        .map_err(|error| ProviderError {
            class: ErrorClass::Unreachable,
            status: None,
            message: format!("the gateway could not be reached: {error}"),
        })?;
    let status = response.status();
    if !status.is_success() {
        // The status is reported; the body is not — it can carry a token, and the
        // error message carries no raw body (docs/18 §11).
        return Err(ProviderError {
            class: status_class(status.as_u16()),
            status: Some(status.as_u16()),
            message: format!("the gateway answered with status {status}"),
        });
    }
    response
        .json::<Value>()
        .await
        .map_err(|error| ProviderError {
            class: ErrorClass::InvalidRequest,
            status: Some(status.as_u16()),
            message: format!("the gateway answer could not be read: {error}"),
        })
}

/// The bridge vocabulary's status class (an HTTP status with no parseable
/// error envelope).
pub(crate) fn status_class(status: u16) -> ErrorClass {
    match status {
        401 | 403 => ErrorClass::Auth,
        429 => ErrorClass::Quota,
        404 => ErrorClass::ModelNotFound,
        500..=599 => ErrorClass::Transient,
        _ => ErrorClass::InvalidRequest,
    }
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

/// Map a discovery row onto a descriptor (`docs/12` §2.3: gateway rows carry
/// capability fields; absent metadata stays unsupported, never optimistic).
pub(crate) fn descriptor_from_row(row: &Value) -> Option<myharness_provider_sdk::ModelDescriptor> {
    let id = model_id(row)?;
    let tools_supported = row
        .get("capabilities")
        .and_then(|capabilities| {
            capabilities
                .as_array()
                .map(|values| {
                    values.iter().any(|value| {
                        matches!(value.as_str(), Some("tools" | "tool_use" | "tool-use"))
                    })
                })
                .or_else(|| {
                    capabilities
                        .get("tools")
                        .or_else(|| capabilities.get("tool_use"))
                        .and_then(Value::as_bool)
                })
        })
        .unwrap_or(false)
        || row
            .get("supports_tools")
            .and_then(Value::as_bool)
            .unwrap_or(false);
    let capabilities = myharness_provider_sdk::ModelCapabilities {
        context_window: row.get("context_window").and_then(Value::as_u64),
        max_output_tokens: row.get("max_output_tokens").and_then(Value::as_u64),
        tools_supported,
        ..Default::default()
    };
    Some(myharness_provider_sdk::ModelDescriptor {
        key: myharness_provider_sdk::ModelKey {
            provider_id: "gateway".to_string(),
            model_id: id,
            variant: None,
            revision: None,
        },
        display_name: row
            .get("display_name")
            .and_then(Value::as_str)
            .map(str::to_string),
        capabilities,
        unmodeled: Some(row.clone()),
    })
}

// The unit tests live with the wire fixtures: `tests/wire.rs` covers discovery
// against a mock gateway (moved here from `crates/bridge`, unchanged in
// behavior — see `crates/bridge/src/helper.rs` for the delegation shim that
// keeps the setup-window path byte-comparable).
