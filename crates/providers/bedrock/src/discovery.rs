//! Bedrock model discovery — the family's native Bedrock model APIs listing,
//! or the profile's static list (`docs/18` §6: "Bedrock | … | Bedrock model
//! APIs or static"; `docs/09` §2.2 "Bedrock"; `docs/10` §9 assigns discovery
//! to `providers`).
//!
//! What is recorded, and therefore sent:
//!
//! - The listing is answered by the family's control-plane host
//!   (`docs/12` §1 records `bedrock.<region>.amazonaws.com` among the
//!   control-plane hosts; the configured override replaces both endpoints —
//!   `docs/14` §9): `GET {control}/foundation-models` with the same bearer
//!   credential the inference surface uses.
//! - **No query parameters are injected.** No parameter for this family's
//!   model APIs is recorded (`docs/12` §3.1 lists the built-in listing
//!   *policy*, not a Bedrock wire parameter), so the rebuild sends the bare
//!   endpoint and filters client-side. Recorded in `docs/14` §9.
//! - The app's built-in listing policy applies: a `nextToken` cursor
//!   followed for at most three pages, ten seconds per page, a later-page
//!   failure keeping the rows already read, and de-duplication by stable id
//!   (`docs/12` §3.1).
//! - The row shape is shape-tolerant (objects with a `modelId`/`id`/`model`
//!   string; display names under `modelName`/`display_name`) because an
//!   exact capture was never made; nothing is invented. Recorded in
//!   `docs/14` §9.
//!
//! This is a **probe of a provider-native endpoint**. It is not a translation
//! layer: it speaks the Bedrock model APIs listing contract and nothing else.

use std::collections::BTreeSet;
use std::time::Duration;

use myharness_provider_sdk::{ErrorClass, ProviderError};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The per-page timeout (`docs/12` §3.1: "Per-page timeout: 10 seconds").
pub const DISCOVERY_PAGE_TIMEOUT: Duration = Duration::from_secs(10);

/// The maximum number of pages (`docs/12` §3.1: "maximum 3 pages").
pub const DISCOVERY_MAX_PAGES: usize = 3;

/// One discovered model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredModel {
    /// The stable id — the de-duplication key (`docs/12` §3.1).
    pub id: String,
    /// The raw row, kept so catalog metadata can be joined onto it later.
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
    /// True when a later page failed (or the page cap was reached) and the
    /// rows already read were kept (`docs/12` §3.1).
    pub partial: bool,
    /// The warning to show, if any.
    pub warning: Option<String>,
}

/// Run the native listing against the control-plane host. The credential
/// material exists only as the request's header content (`AI-PROVIDER-03`)
/// — never logged, never persisted.
pub async fn discover_models(
    control_base_url: &str,
    credential: Option<String>,
    custom_headers: &[(String, String)],
    client: &reqwest::Client,
) -> Result<DiscoveryOutcome, ProviderError> {
    discover_pages(
        control_base_url,
        credential,
        custom_headers,
        client,
        DISCOVERY_PAGE_TIMEOUT.as_secs(),
        DISCOVERY_MAX_PAGES,
    )
    .await
}

/// The full-parameter form used by tests. `timeout_secs == 0` and
/// `max_pages == 0` fall back to the documented defaults.
pub async fn discover_pages(
    control_base_url: &str,
    credential: Option<String>,
    custom_headers: &[(String, String)],
    client: &reqwest::Client,
    timeout_secs: u64,
    max_pages: usize,
) -> Result<DiscoveryOutcome, ProviderError> {
    let base = control_base_url.trim().trim_end_matches('/');
    if base.is_empty() {
        return Err(ProviderError {
            class: ErrorClass::Configuration,
            status: None,
            message: "the Bedrock control-plane base URL is required for model discovery"
                .to_string(),
        });
    }
    let url = format!("{base}/foundation-models");
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
        // The first page carries no query parameters: none is invented for
        // this family (`docs/14` §9), and the native default applies.
        let page_url = match &cursor {
            Some(cursor) => format!("{url}?nextToken={}", encode(cursor)),
            None => url.clone(),
        };
        let page = fetch_page(client, &credential, custom_headers, &page_url, timeout).await;
        let body = match page {
            Ok(body) => body,
            Err(error) => {
                if pages == 0 {
                    // The first page failing is an error (`docs/12` §3.1).
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
                // The page cap was reached: the list is partial by policy,
                // and the rows already read are kept (`docs/12` §3.1).
                partial = true;
                warning = Some(format!(
                    "the provider advertised more models than the {}-page discovery limit",
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
    credential: &Option<String>,
    custom_headers: &[(String, String)],
    url: &str,
    timeout: Duration,
) -> Result<Value, ProviderError> {
    let mut builder = client.get(url);
    // The family's recorded static credential is a bearer token
    // (`docs/09` §2.2 "static bearer"); it is the same material the runtime
    // surface uses. An absent credential lets the request answer on its own
    // — the caller gates credentials first.
    if let Some(token) = credential {
        builder = builder.header("Authorization", format!("Bearer {token}"));
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
            message: format!("the provider could not be reached: {error}"),
        })?;
    let status = response.status();
    if !status.is_success() {
        // The status is reported; the body is not — it can carry a token,
        // and the error message carries no raw body (`docs/18` §5.4).
        return Err(ProviderError {
            class: crate::messages::status_class(status.as_u16()),
            status: Some(status.as_u16()),
            message: format!("the provider answered with status {status}"),
        });
    }
    response
        .json::<Value>()
        .await
        .map_err(|error| ProviderError {
            class: ErrorClass::InvalidRequest,
            status: Some(status.as_u16()),
            message: format!("the provider answer could not be read: {error}"),
        })
}

/// The model rows of a page, and the cursor of the next page.
///
/// An exact capture was never made (`docs/14` §9), so the parse is
/// shape-tolerant — a `modelSummaries` / `models` / `data` array of objects
/// with a `modelId` / `id` / `model` string on each row, and the cursor
/// under `nextToken`. Nothing is invented.
fn parse_page(body: &Value) -> (Vec<Value>, Option<String>) {
    let rows = body
        .get("modelSummaries")
        .or_else(|| body.get("models"))
        .or_else(|| body.get("data"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let cursor = ["nextToken", "next_token", "nextCursor"]
        .iter()
        .find_map(|key| {
            body.get(*key)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        });
    (rows, cursor)
}

fn model_id(row: &Value) -> Option<String> {
    ["modelId", "id", "model"]
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

/// Map a discovery row onto a descriptor (`docs/12` §2.3: rows may carry
/// capability fields; absent metadata stays unsupported, never optimistic).
pub(crate) fn descriptor_from_row(
    provider_id: &str,
    row: &Value,
) -> Option<myharness_provider_sdk::ModelDescriptor> {
    let id = model_id(row)?;
    let tools_supported = row
        .get("capabilities")
        .and_then(|capabilities| {
            capabilities
                .get("tool_use")
                .or_else(|| capabilities.get("tools"))
        })
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || row
            .get("supports_tools")
            .and_then(Value::as_bool)
            .unwrap_or(false);
    let capabilities = myharness_provider_sdk::ModelCapabilities {
        // The rows' capability fields were not captured in any spelling the
        // rebuild can cite (`docs/14` §9); a row carries what it carries, and
        // unknown metadata stays unsupported rather than optimistic.
        tools_supported,
        ..Default::default()
    };
    Some(myharness_provider_sdk::ModelDescriptor {
        key: myharness_provider_sdk::ModelKey {
            provider_id: provider_id.to_string(),
            model_id: id,
            variant: None,
            revision: None,
        },
        display_name: ["modelName", "display_name", "displayName"]
            .iter()
            .find_map(|key| row.get(*key).and_then(Value::as_str))
            .map(str::to_string),
        capabilities,
        unmodeled: Some(row.clone()),
    })
}
