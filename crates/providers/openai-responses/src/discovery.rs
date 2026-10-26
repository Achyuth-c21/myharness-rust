//! Model discovery for the Responses wall: the platform's shared
//! `GET {base}/models` listing, read tolerantly (the same surface the Chat
//! Completions wall discovers through — the platform has one listing
//! endpoint per base). This module is this crate's own implementation: the
//! sibling holds its own, by design (`AI-PROVIDER-02` — walls share a
//! design, not code).

use std::time::Duration;

use myharness_provider_sdk::{
    DescriptorSource, ErrorClass, ModelCapabilities, ModelDescriptor, ModelKey, ProviderError,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The documented discovery window (the same 10 s per page the setup
/// discovery protocol uses — `docs/18` §5; `docs/14` §9).
pub const RESPONSES_DISCOVERY_TIMEOUT: Duration = Duration::from_secs(10);

/// The setup discovery request (Phase 8 slice 2): the endpoint to read and
/// the credential the user typed — never logged, never stored by the probe
/// (`docs/09` §4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryRequest {
    /// The base URL; empty falls to the first-party host (this family is
    /// first-party only — `docs/14` §9 item 71).
    pub base_url: String,
    /// The credential, when the user typed one. The family's rule makes it
    /// required; the field stays optional so the refusal is the adapter's
    /// own empty-credential gate, never a substituted value
    /// (`AI-PROVIDER-03`, `AI-ADAPTER-12`).
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
    RESPONSES_DISCOVERY_TIMEOUT.as_secs()
}

/// Run the setup discovery against this family's listing surface
/// (`GET {base}/models`, single page). The base URL is normalized here
/// (empty → the first-party host) exactly as the adapter normalizes it.
pub async fn discover_models(
    request: &DiscoveryRequest,
    client: &reqwest::Client,
) -> Result<DiscoveryOutcome, ProviderError> {
    discover_with(
        &crate::adapter::normalize_base(&request.base_url),
        request.api_key.clone().unwrap_or_default(),
        &request.custom_headers,
        client,
        request.timeout_secs,
    )
    .await
}

/// One discovered row: the raw JSON (descriptors read from it later) plus
/// the id that named it.
#[derive(Debug, Clone)]
pub struct DiscoveredModel {
    pub id: String,
    pub raw: Value,
}

/// The discovery outcome: what was read, and whether the read was partial
/// (a later-page failure keeps what was read and warns — the shipped
/// protocol's shape; single-page here, so `warnings` carries only the
/// tolerant-parse notes).
#[derive(Debug, Clone)]
pub struct DiscoveryOutcome {
    pub models: Vec<DiscoveredModel>,
    pub warnings: Vec<String>,
}

/// One tolerant row: the id may sit under `id`, `model`, or `name`.
fn row_id(row: &Value) -> Option<String> {
    ["id", "model", "name"]
        .iter()
        .find_map(|key| row.get(*key).and_then(Value::as_str))
        .map(str::to_string)
        .filter(|id| !id.is_empty())
}

/// Read the listing body tolerantly: a `data` array, a bare top-level
/// array, or a keyed `models` object (the three shapes compatible
/// deployments answer with — `docs/14` §9).
fn rows_from_listing(body: &Value, warnings: &mut Vec<String>) -> Vec<Value> {
    if let Some(rows) = body.get("data").and_then(Value::as_array) {
        return rows.clone();
    }
    if let Some(rows) = body.as_array() {
        return rows.clone();
    }
    if let Some(rows) = body.get("models").and_then(Value::as_array) {
        return rows.clone();
    }
    warnings.push("the listing answered in an unhandled shape; nothing was read".to_string());
    Vec::new()
}

/// Run the single-page discovery against `base`.
pub(crate) async fn discover_with(
    base: &str,
    credential: String,
    custom_headers: &[(String, String)],
    client: &reqwest::Client,
    timeout_secs: u64,
) -> Result<DiscoveryOutcome, ProviderError> {
    let mut builder = client
        .get(format!("{base}/models"))
        .timeout(Duration::from_secs(timeout_secs.max(1)));
    if !credential.trim().is_empty() {
        builder = builder.header("authorization", format!("Bearer {credential}"));
    }
    for (name, value) in custom_headers {
        builder = builder.header(name, value);
    }
    let response = builder.send().await.map_err(|error| ProviderError {
        class: ErrorClass::Unreachable,
        status: None,
        message: format!("the provider could not be reached: {error}"),
    })?;
    let status = response.status();
    if !status.is_success() {
        let status = status.as_u16();
        let body = response.json::<Value>().await.ok();
        return Err(match body {
            Some(body) => crate::responses::error_from_envelope(Some(status), &body),
            None => crate::responses::error_from_status(status),
        });
    }
    let listing: Value = response.json().await.map_err(|error| ProviderError {
        class: ErrorClass::InvalidRequest,
        status: None,
        message: format!("the discovery answer could not be read: {error}"),
    })?;
    let mut warnings = Vec::new();
    let models = rows_from_listing(&listing, &mut warnings)
        .into_iter()
        .filter_map(|raw| {
            let id = row_id(&raw)?;
            Some(DiscoveredModel { id, raw })
        })
        .collect();
    Ok(DiscoveryOutcome { models, warnings })
}

/// Map one raw listing row to a descriptor. Absent capability metadata
/// stays unsupported (`AI-PROVIDER-05`); the row's `display_name`/`name`
/// travels when the provider reports one.
pub(crate) fn descriptor_from_row(provider_id: &str, raw: &Value) -> Option<ModelDescriptor> {
    let id = ["id", "model", "name"]
        .iter()
        .find_map(|key| raw.get(*key).and_then(Value::as_str))
        .filter(|id| !id.is_empty())?
        .to_string();
    let display_name = ["display_name", "name"]
        .iter()
        .find_map(|key| raw.get(*key).and_then(Value::as_str))
        .map(str::to_string);
    Some(ModelDescriptor {
        key: ModelKey {
            provider_id: provider_id.to_string(),
            model_id: id,
            variant: None,
            revision: None,
        },
        display_name,
        capabilities: ModelCapabilities {
            source: Some(DescriptorSource::Provider),
            ..ModelCapabilities::default()
        },
        unmodeled: None,
    })
}
