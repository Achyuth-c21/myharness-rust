//! Model discovery for the local wall (`AI-LOCAL-06`): the two runtimes'
//! listing shapes, read tolerantly — `GET {base}/api/tags` first (Ollama's
//! keyed `models` array, ids under `name`/`model`), and
//! `GET {base}/v1/models` (the OpenAI `data` array) when the first endpoint
//! is absent. A 404 on `/api/tags` is the llama.cpp posture, not an error;
//! only a failure on BOTH is one. Own implementation, sibling design
//! (`AI-PROVIDER-02`).

use std::time::Duration;

use myharness_provider_sdk::{
    DescriptorSource, ErrorClass, ModelCapabilities, ModelDescriptor, ModelKey, ProviderError,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::destination::Destination;

/// The documented discovery window (the same 10 s per page the setup
/// discovery protocol uses — `docs/18` §5; `docs/14` §9).
pub const LOCAL_DISCOVERY_TIMEOUT: Duration = Duration::from_secs(10);

/// The setup discovery request (Phase 8): the endpoint to read and the
/// credential the user typed — never logged, never stored by the probe
/// (`docs/09` §4). The destination gate applies here exactly as it does to
/// a turn: a non-loopback base refuses before any bytes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryRequest {
    /// The base URL; empty falls to the family's recorded default.
    pub base_url: String,
    /// The credential, when the user typed one; absent is the documented
    /// keyless send (`AI-LOCAL-05`).
    #[serde(default)]
    pub api_key: Option<String>,
    /// Extra headers the profile forwards.
    #[serde(default)]
    pub custom_headers: Vec<(String, String)>,
}

/// Run the setup discovery: validate and pin the destination, then read the
/// runtimes' listing shapes (`/api/tags` first, `/v1/models` on a 404).
pub async fn discover_models(
    request: &DiscoveryRequest,
    client: &reqwest::Client,
) -> Result<DiscoveryOutcome, ProviderError> {
    let resolver: crate::destination::ResolveFn =
        std::sync::Arc::new(crate::destination::resolve_system);
    let destination = Destination::resolve(&request.base_url, &resolver)?;
    discover_with(
        &destination,
        request.api_key.as_deref(),
        &request.custom_headers,
        client,
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

/// The discovery outcome: what was read, and which listing answered.
#[derive(Debug, Clone)]
pub struct DiscoveryOutcome {
    pub models: Vec<DiscoveredModel>,
    /// Which endpoint answered: `"api/tags"` or `"v1/models"`.
    pub source: &'static str,
    /// Tolerant-parse notes (never a silent drop).
    pub warnings: Vec<String>,
}

/// One tolerant row: the id may sit under `name`, `model`, or `id`.
fn row_id(row: &Value) -> Option<String> {
    ["name", "model", "id"]
        .iter()
        .find_map(|key| row.get(*key).and_then(Value::as_str))
        .map(str::to_string)
        .filter(|id| !id.is_empty())
}

/// Read one listing body tolerantly: a keyed array under any of the known
/// keys, or a bare top-level array (`AI-LOCAL-06`).
fn rows_from_listing(body: &Value, warnings: &mut Vec<String>) -> Vec<Value> {
    for key in ["models", "data"] {
        if let Some(rows) = body.get(key).and_then(Value::as_array) {
            return rows.clone();
        }
    }
    if let Some(rows) = body.as_array() {
        return rows.clone();
    }
    warnings.push("the listing answered in an unhandled shape; nothing was read".to_string());
    Vec::new()
}

fn listing_request(
    destination: &Destination,
    path: &str,
    credential: Option<&str>,
    custom_headers: &[(String, String)],
    client: &reqwest::Client,
) -> reqwest::RequestBuilder {
    let mut builder = client
        .get(destination.url(path))
        .timeout(LOCAL_DISCOVERY_TIMEOUT);
    if let Some(credential) = credential.filter(|value| !value.trim().is_empty()) {
        builder = builder.header("authorization", format!("Bearer {credential}"));
    }
    for (name, value) in custom_headers {
        builder = builder.header(name, value);
    }
    builder
}

async fn read_listing(
    destination: &Destination,
    path: &'static str,
    credential: Option<&str>,
    custom_headers: &[(String, String)],
    client: &reqwest::Client,
) -> Result<(Vec<DiscoveredModel>, Vec<String>), (Option<u16>, ProviderError)> {
    let builder = listing_request(destination, path, credential, custom_headers, client);
    let response = builder.send().await.map_err(|error| {
        (
            None,
            ProviderError {
                class: ErrorClass::Unreachable,
                status: None,
                message: format!("the local endpoint could not be reached: {error}"),
            },
        )
    })?;
    let status = response.status();
    if !status.is_success() {
        let status = status.as_u16();
        let body = response.json::<Value>().await.ok();
        return Err((
            Some(status),
            match body {
                Some(body) => crate::chat::error_from_envelope(Some(status), &body),
                None => crate::chat::error_from_status(status),
            },
        ));
    }
    let listing: Value = response.json().await.map_err(|error| {
        (
            None,
            ProviderError {
                class: ErrorClass::InvalidRequest,
                status: None,
                message: format!("the discovery answer could not be read: {error}"),
            },
        )
    })?;
    let mut warnings = Vec::new();
    let rows = rows_from_listing(&listing, &mut warnings)
        .into_iter()
        .filter_map(|raw| {
            let id = row_id(&raw)?;
            Some(DiscoveredModel { id, raw })
        })
        .collect();
    Ok((rows, warnings))
}

/// Run the discovery: `/api/tags` first, `/v1/models` on a 404 from the
/// first (`AI-LOCAL-06`). A network failure or a non-404 error on the first
/// endpoint is fatal — the endpoint was reached and refused, and falling
/// back would hide it.
pub(crate) async fn discover_with(
    destination: &Destination,
    credential: Option<&str>,
    custom_headers: &[(String, String)],
    client: &reqwest::Client,
) -> Result<DiscoveryOutcome, ProviderError> {
    match read_listing(destination, "/api/tags", credential, custom_headers, client).await {
        Ok((rows, warnings)) => Ok(DiscoveryOutcome {
            models: rows,
            source: "api/tags",
            warnings,
        }),
        Err((Some(404), _)) => {
            // The Ollama-only endpoint is absent: the llama.cpp posture.
            let (rows, mut warnings) = read_listing(
                destination,
                "/v1/models",
                credential,
                custom_headers,
                client,
            )
            .await
            .map_err(|(_, error)| error)?;
            warnings
                .push("the /api/tags listing was absent; the OpenAI listing answered".to_string());
            Ok(DiscoveryOutcome {
                models: rows,
                source: "v1/models",
                warnings,
            })
        }
        Err((_, error)) => Err(error),
    }
}

/// Map one raw listing row to a descriptor. Absent capability metadata
/// stays unsupported (`AI-PROVIDER-05`); the row's `name` travels when the
/// provider reports one.
pub(crate) fn descriptor_from_row(provider_id: &str, raw: &Value) -> Option<ModelDescriptor> {
    let id = ["name", "model", "id"]
        .iter()
        .find_map(|key| raw.get(*key).and_then(Value::as_str))
        .filter(|id| !id.is_empty())?
        .to_string();
    Some(ModelDescriptor {
        key: ModelKey {
            provider_id: provider_id.to_string(),
            model_id: id,
            variant: None,
            revision: None,
        },
        display_name: raw.get("name").and_then(Value::as_str).map(str::to_string),
        capabilities: ModelCapabilities {
            source: Some(DescriptorSource::Provider),
            ..ModelCapabilities::default()
        },
        unmodeled: None,
    })
}
