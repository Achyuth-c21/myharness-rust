//! Claude API model discovery — the native `/v1/models` listing, or the
//! profile's static list (`docs/18` §6: "Claude API | … | `/v1/models` or
//! static"; `docs/09` §2.2 "Claude API"; `docs/10` §9 assigns discovery to
//! `providers`).
//!
//! What is recorded, and therefore sent:
//!
//! - The listing is a `GET {base}/v1/models` with `x-api-key` auth and
//!   `anthropic-version: 2023-06-01` (`docs/18` §6; `docs/09` §2.2 records
//!   the shared Anthropic-compatible contract statement for the family set).
//! - **No `limit` parameter is injected.** The gateway's `limit=1000` is
//!   recorded for the *gateway* listing specifically (`docs/12` §3.1's URL
//!   shape); nothing records a first-party limit, so the rebuild sends no
//!   parameter it cannot cite. Recorded in `docs/14` §9.
//! - The app's built-in listing policy applies as policy: an `after_id`
//!   cursor followed for at most three pages, ten seconds per page, a
//!   later-page failure keeping the rows already read, and de-duplication by
//!   stable id (`docs/12` §3.1).
//! - The page shape is shape-tolerant (a `data`/`models` array of objects
//!   with an `id`/`model`/`name` string) because an exact first-party
//!   response was never captured; nothing is invented. Recorded in
//!   `docs/14` §9.
//!
//! This is a **probe of a provider-native endpoint**. It is not a translation
//! layer: it speaks the Anthropic listing contract and nothing else.

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

/// Run the native listing against the first-party host (or the configured
/// base URL). The credential material exists only as the requests' header
/// content (`AI-PROVIDER-03`) — never logged, never persisted.
pub async fn discover_models(
    base_url: &str,
    credential: Option<String>,
    custom_headers: &[(String, String)],
    client: &reqwest::Client,
) -> Result<DiscoveryOutcome, ProviderError> {
    discover_pages(
        base_url,
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
    base_url: &str,
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
            message: "the Anthropic base URL is required for model discovery".to_string(),
        });
    }
    let url = format!("{base}/v1/models");
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
        // The first page carries no query parameters: no `limit` is invented
        // for this family (`docs/14` §9), and the native default applies.
        let page_url = match &cursor {
            Some(cursor) => format!("{url}?after_id={}", encode(cursor)),
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
    let mut builder = client
        .get(url)
        .header("anthropic-version", crate::ANTHROPIC_VERSION);
    // The family's auth is `x-api-key` (`docs/18` §6); there is no scheme
    // field to interpret. An absent credential lets the request answer on
    // its own — the caller gates credentials first.
    if let Some(key) = credential {
        builder = builder.header("x-api-key", key);
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
        // The status is reported; the body is not — it can carry a token, and
        // the error message carries no raw body (`docs/18` §5.4 discipline).
        return Err(ProviderError {
            class: status_class(status.as_u16()),
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
/// An exact first-party page was never captured (`docs/14` §9), so the parse
/// is shape-tolerant — the same reading the contract allows: a `data` or
/// `models` array of objects, an `id` / `model` / `name` string on each row,
/// and a cursor under `after_id` / `next_cursor` / `last_id`. Nothing is
/// invented; `has_more: false` ends the walk.
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
        context_window: row.get("context_window").and_then(Value::as_u64),
        max_output_tokens: row.get("max_output_tokens").and_then(Value::as_u64),
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
        display_name: row
            .get("display_name")
            .and_then(Value::as_str)
            .map(str::to_string),
        capabilities,
        unmodeled: Some(row.clone()),
    })
}
