//! OpenAI-family model discovery — `GET {base}/models` (the open model
//! layer's native listing; owner decision recorded 2026-10-23, `docs/14`
//! §9).
//!
//! The listing is a **single page**: this wire orders no cursor in its
//! compat surface, and a fragmenting cursor would be invented structure
//! (`docs/01` fail-closed rule). Ten-second timeout
//! ([`OPENAI_DISCOVERY_TIMEOUT`]), trailing slashes normalized, models
//! de-duplicated by their stable id.
//!
//! The parse is shape-tolerant — OpenAI-compatible servers genuinely disagree
//! on the listing shape, so the reader accepts the shapes observed across
//! the surface and nothing further (`docs/14` §9 per-wire readings):
//!
//! - a `data` **array** of rows (first-party OpenAI's shape), a `models`
//!   **object** keyed by id (some compatible servers), or a bare top-level
//!   array (others);
//! - rows are objects with an id under `id`/`model`/`name`, or bare strings;
//! - a row's context size is read from `context_window` or
//!   `context_length`.
//!
//! A row with no readable id is skipped, never invented; an empty listing is
//! an empty answer, not an error.

use std::collections::BTreeSet;
use std::time::Duration;

use myharness_provider_sdk::{
    ErrorClass, ModelCapabilities, ModelDescriptor, ModelKey, ProviderError,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::adapter::{status_class, AdapterKind, normalize_base};

/// The single listing's timeout (the shipped discovery timeout; the gateway
/// family documents 10 s per page — `docs/12` §3.1 — and the tolerance
/// reading keeps the same bound here).
pub const OPENAI_DISCOVERY_TIMEOUT: Duration = Duration::from_secs(10);

/// A model discovery request. A wire type: the setup window sends it across
/// the bridge, and the adapter runs the same listing against a resolved
/// credential — see [`discover_with`], which keeps credential material out
/// of wire types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryRequest {
    /// Which family the request targets (its id travels onto descriptors).
    pub kind: AdapterKind,
    /// The base URL. Trailing slashes are normalized here.
    pub base_url: String,
    /// The credential, when the endpoint needs one. The user typed it; it is
    /// never logged and never stored by the probe (`docs/09` §4). A `custom`
    /// endpoint may need none (`docs/14` §9 owner decision).
    #[serde(default)]
    pub api_key: Option<String>,
    /// Extra headers the profile forwards.
    #[serde(default)]
    pub custom_headers: Vec<(String, String)>,
    /// The listing timeout, in seconds. The documented bound is 10.
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
}

fn default_timeout_secs() -> u64 {
    OPENAI_DISCOVERY_TIMEOUT.as_secs()
}

/// One discovered model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredModel {
    /// The stable id — the deduplication key.
    pub id: String,
    /// The raw row, kept so catalog metadata can join onto it later.
    pub raw: Value,
}

/// The outcome of a discovery run (single page by this wire's contract).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryOutcome {
    /// The models, de-duplicated by id.
    pub models: Vec<DiscoveredModel>,
    /// How many pages were read (always 1; the field is the shared shape).
    pub pages: usize,
    /// False here — the listing has no cursor to truncate.
    pub partial: bool,
    /// Always absent here; the shared shape keeps the slot.
    pub warning: Option<String>,
}

/// Run model discovery against an OpenAI-compatible endpoint.
///
/// This is the wire-type entry point (the setup-window probe path). No
/// credential material travels inside a wire type.
pub async fn discover_models(
    request: &DiscoveryRequest,
    client: &reqwest::Client,
) -> Result<DiscoveryOutcome, ProviderError> {
    discover_with(
        request.kind,
        &request.base_url,
        request.api_key.clone(),
        &request.custom_headers,
        client,
        request.timeout_secs,
    )
    .await
}

/// The adapter-side discovery entry: the same native listing run with a
/// credential the caller resolved per request (rotation). The credential
/// exists only as this request's header content (`AI-PROVIDER-03`) — never
/// logged, never persisted. A `custom` endpoint runs keyless when the
/// credential is empty (`docs/14` §9 owner decision).
pub async fn discover_with(
    kind: AdapterKind,
    base_url: &str,
    credential: Option<String>,
    custom_headers: &[(String, String)],
    client: &reqwest::Client,
    timeout_secs: u64,
) -> Result<DiscoveryOutcome, ProviderError> {
    let base = normalize_base(kind, base_url);
    if base.is_empty() {
        return Err(ProviderError {
            class: ErrorClass::Configuration,
            status: None,
            message: "the base URL is required for model discovery".to_string(),
        });
    }
    let url = format!("{base}/models");
    let timeout = if timeout_secs == 0 {
        OPENAI_DISCOVERY_TIMEOUT
    } else {
        Duration::from_secs(timeout_secs)
    };

    let mut builder = client.get(&url).timeout(timeout);
    if let Some(key) = credential.filter(|key| !key.trim().is_empty()) {
        // The header goes on only when a credential exists — keyless
        // endpoints get no `authorization` at all (`docs/14` §9).
        builder = builder.header("authorization", format!("Bearer {key}"));
    }
    for (name, value) in custom_headers {
        builder = builder.header(name, value);
    }
    let response = builder
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
        // and the error message carries no raw body (`docs/18` §11).
        return Err(ProviderError {
            class: status_class(status.as_u16()),
            status: Some(status.as_u16()),
            message: format!("the provider answered with status {status}"),
        });
    }
    let body = response
        .json::<Value>()
        .await
        .map_err(|error| ProviderError {
            class: ErrorClass::InvalidRequest,
            status: Some(status.as_u16()),
            message: format!("the provider answer could not be read: {error}"),
        })?;

    let mut models = Vec::new();
    let mut seen = BTreeSet::new();
    for (id, raw) in listing_rows(&body) {
        if seen.insert(id.clone()) {
            models.push(DiscoveredModel { id, raw });
        }
    }
    Ok(DiscoveryOutcome {
        models,
        pages: 1,
        partial: false,
        warning: None,
    })
}

/// The listing rows of one `GET {base}/models` answer, tolerant of the
/// shapes the wire's servers actually send (the module header's three).
/// Returns `(id, raw-row)` pairs; raw rows stay whole for the catalog.
fn listing_rows(body: &Value) -> Vec<(String, Value)> {
    let rows: Vec<Value> = match body {
        // A bare top-level array, or `data` as an array.
        Value::Array(rows) => Some(rows.clone()),
        Value::Object(_) => body.get("data").and_then(Value::as_array).cloned(),
        _ => None,
    }
    .map(|rows| {
        rows.into_iter()
            .filter_map(row_from_value)
            .collect::<Vec<_>>()
    })
    .unwrap_or_default();
    let keyed: Vec<Value> = match body.get("models").and_then(Value::as_object) {
        // Keyed form: the key is the id; the value (when an object) is the
        // row (with the key injected so the raw row stays self-describing).
        Some(map) => map
            .iter()
            .filter_map(|(key, value)| {
                let id = key.trim();
                if id.is_empty() {
                    return None;
                }
                let mut row = if value.is_object() {
                    value.clone()
                } else {
                    json!({})
                };
                row["id"] = json!(key);
                Some(row)
            })
            .collect(),
        None => Vec::new(),
    };
    rows.into_iter()
        .chain(keyed)
        .filter_map(|row| row_id(&row).map(|id| (id, row)))
        .collect()
}

/// Normalize one row entry: objects stay whole, bare strings become
/// `{"id": …}` rows (their string is the id — a documented shape).
fn row_from_value(value: Value) -> Option<Value> {
    match value {
        object @ Value::Object(_) => Some(object),
        Value::String(id) if !id.trim().is_empty() => Some(json!({ "id": id.trim() })),
        _ => None,
    }
}

/// A row's id: the first readable key of `id`/`model`/`name` (`docs/14` §9
/// per-wire readings; the bare-string rows carry `id` from [`row_from_value`]).
/// Nothing is invented.
fn row_id(row: &Value) -> Option<String> {
    ["id", "model", "name"]
        .iter()
        .find_map(|key| row.get(*key).and_then(Value::as_str))
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
}

/// Map one listing row onto a descriptor: native id, any reported display
/// name, and a context size under either shape the wire names. Absent
/// capability metadata stays unsupported, never optimistic
/// (`AI-PROVIDER-05`).
pub(crate) fn descriptor_from_row(provider: &str, row: &Value) -> Option<ModelDescriptor> {
    let id = row_id(row)?;
    let capabilities = ModelCapabilities {
        context_window: row
            .get("context_window")
            .and_then(Value::as_u64)
            .or_else(|| row.get("context_length").and_then(Value::as_u64)),
        ..Default::default()
    };
    Some(ModelDescriptor {
        key: ModelKey {
            provider_id: provider.to_string(),
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
