//! Provider form schemas and validation — roadmap Phase 2 (docs/13 §4 Phase 2;
//! docs/09 §2.2; docs/12 §2.2; docs/features/third-party-setup.md "Validation and
//! error states").
//!
//! One schema per supported provider family (the shipped set is six — `docs/12` §1 —
//! the rebuild carries five; the Vertex family was removed from rebuild scope, owner
//! decision 2026-10-23, `docs/04` §3.1), built from the
//! observed flat `inference*` / per-provider key names (`docs/20` §2.1 — the key
//! index is the naming ground truth). Each [`FieldDef`] carries the metadata the setup
//! window's renderer needs (label, kind, required marker, options, help text), and
//! [`validate_connection`] carries the semantic rules:
//!
//! - **Required fields block Apply** with `*` markers (`docs/screens` "Required fields
//!   marked `*`; Apply blocks/shows errors").
//! - **Model ids are freeform** (owner decision 2026-10-23 — `docs/14` §9, "Open
//!   harness pivot"): every family accepts any model id the user names. The
//!   per-family Anthropic/Bedrock policies of `docs/12` §2.2 (the shipped app's
//!   gatekeeping) were withdrawn for the rebuild; the validator keeps only the
//!   structural gates — a non-empty id, and `docs/12`'s shape warnings where a
//!   family's listing quality is unknown. The shipped-app policy text in
//!   `docs/12` §2.2 stays as the preserved baseline; it no longer governs the
//!   rebuild (`docs/13` §C, "Open model layer").
//! - **Custom headers are routing/tenant only** — credential-shaped names and values
//!   are rejected (`TEST-SEC-004`; `docs/09` §2.2 / §4; `docs/08` "Authorization is
//!   treated as a credential").
//! - **Credential kinds are exclusive** (`REQ-SETUP-05`) — the kind enum is singular
//!   by construction; per-provider offered subsets validated here.
//! - **Discovery gating**: disabled for mantle static/helper (`docs/09` §2.2
//!   provider bullets); Foundry's
//!   exact disabled combinations were not captured (`docs/14` §9) and are not
//!   enforced here.
//!
//! Deliberately NOT here (bridge/renderer slices): fetch of model catalogs, live
//! reachability probes (health), and the helper-script process runner (lives in
//! `crates/secrets`).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{CredentialKind, ProfileDocument, ProviderKind};

/// Field severity on the form. Errors block Apply; warnings render but do not
/// (`docs/screens` states: "Invalid — inline errors; Apply blocked or fails
/// atomically").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormIssue {
    pub field: String,
    pub severity: Severity,
    pub message: String,
}

fn issue(field: &str, severity: Severity, message: String) -> FormIssue {
    FormIssue {
        field: field.to_string(),
        severity,
        message,
    }
}

fn error(field: &str, message: String) -> FormIssue {
    issue(field, Severity::Error, message)
}

fn warning(field: &str, message: String) -> FormIssue {
    issue(field, Severity::Warning, message)
}

/// Renderer-facing field kinds (the setup form's control vocabulary; `docs/screens`
/// "Two-column label/control layout", password reveal/lock, native-looking selects,
/// blue switches).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FieldKind {
    Text,
    Url,
    /// Password-style control: reveal + lock icons, value stays an opaque handle in
    /// the renderer (`docs/18` §10; `DATA-CONFIG-02`).
    Secret,
    Select,
    Number,
    Switch,
    /// Tag-style list input (egress hosts, workspace folders, allowed domains).
    TagList,
    /// Key/value map input (custom inference headers, OIDC blobs) — `docs/20` records
    /// these as `object` without the inner grammar.
    KeyValue,
}

/// Logical group inside a provider form, in capture order (`docs/screens` "Connection
/// tab": credential section top, endpoint, models below the fold, proxy/VM in the
/// same scroll).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FieldGroup {
    Credentials,
    Endpoint,
    Models,
    Proxy,
    Advanced,
}

/// One option of a select control.
///
/// The **value** is what the profile stores (the wire spelling) and the **label**
/// is what the user reads. They differ - the credential kind is stored as `static`
/// and shown as "Static API key" - so a select that carried only labels could
/// never show the stored value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldOption {
    pub value: &'static str,
    pub label: &'static str,
}

impl FieldOption {
    /// An option whose value and label are the same word.
    pub const fn plain(value: &'static str) -> Self {
        FieldOption {
            value,
            label: value,
        }
    }
}

/// One provider form field. `key` is the flat configuration key (`docs/20` §2.1),
/// carried verbatim so the renderer edits the real document shape and the bridge can
/// map drafts 1:1.
// Serialize only: a form schema is produced by this crate and shipped to the
// renderer as a descriptor. The renderer never constructs one, so there is nothing
// to parse back — and `&'static str` keys cannot borrow from a deserializer.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldDef {
    pub key: &'static str,
    pub label: &'static str,
    pub kind: FieldKind,
    pub group: FieldGroup,
    /// The shipped `*` marker: the field is required in the shipped form.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub required: bool,
    /// Credential kinds under which this field is required. Empty means "always".
    /// The shipped UI marks the field `*` and the kind selector narrows which fields
    /// are meaningful, so this is a rebuild contract, not an observed per-kind
    /// marker set (`docs/14` §9 discipline).
    #[serde(default, skip_serializing_if = "<[CredentialKind]>::is_empty")]
    pub required_when: &'static [CredentialKind],
    /// Select options, when [`FieldKind::Select`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<FieldOption>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub help: Option<&'static str>,
}

/// Schema for one provider's Connection card (`docs/features/third-party-setup.md`
/// "Provider cards").
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormSchema {
    pub provider: ProviderKind,
    pub fields: Vec<FieldDef>,
}

impl FormSchema {
    /// Credential kinds this family offers (`docs/12` §1 per-row "Credential kinds";
    /// `docs/09` §2.2 kind vocabulary).
    pub fn credential_kinds(&self) -> &'static [(&'static str, CredentialKind)] {
        schema_kinds(self.provider)
    }
}

/// Credential kinds offered per provider family (`docs/12` §1 table; `docs/09` §2.2
/// "Credential kinds" table).
fn schema_kinds(provider: ProviderKind) -> &'static [(&'static str, CredentialKind)] {
    match provider {
        ProviderKind::ClaudeApi => &[
            ("Static API key", CredentialKind::Static),
            ("Interactive sign-in", CredentialKind::Interactive),
        ],
        ProviderKind::Gateway => &[
            ("Static API key", CredentialKind::Static),
            ("Interactive sign-in", CredentialKind::Interactive),
            ("Helper script", CredentialKind::HelperScript),
        ],
        ProviderKind::OpenAi => &[("Static API key", CredentialKind::Static)],
        // Phase 8's Responses extension (`docs/13` Phase 8; `docs/14` §9
        // item 71): first-party only — a static key is the family's only
        // offered credential kind, like the Chat Completions `openai` one.
        ProviderKind::OpenAiResponses => &[("Static API key", CredentialKind::Static)],
        ProviderKind::Custom => &[
            ("Static API key", CredentialKind::Static),
            // A company gateway resolves its key through the family's helper
            // script — the same offered-kind set the Gateway family carries.
            ("Helper script", CredentialKind::HelperScript),
        ],
        ProviderKind::Bedrock => &[
            ("Static bearer token", CredentialKind::Static),
            ("AWS profile / SSO", CredentialKind::Sso),
            ("Web identity", CredentialKind::ExternalIdp),
            ("Helper script", CredentialKind::HelperScript),
            ("Vendor profile", CredentialKind::VendorProfile),
        ],
        ProviderKind::BedrockMantle => &[
            ("Static key", CredentialKind::Static),
            ("Helper script", CredentialKind::HelperScript),
            ("Vendor profile", CredentialKind::VendorProfile),
        ],
        ProviderKind::Foundry => &[
            ("Static API key", CredentialKind::Static),
            ("Interactive sign-in", CredentialKind::Interactive),
            ("Helper script", CredentialKind::HelperScript),
        ],
    }
}

// ---------------------------------------------------------------------------
// Model-ID policy (open model layer — owner decision 2026-10-23, `docs/14` §9)
// ---------------------------------------------------------------------------

/// The open model-ID rule. The shipped app's per-family predicates
/// (`docs/12` §2.2: the Anthropic denylist/accept-list and the Bedrock
/// ARN shape) gated which models a user could name; the owner withdrew that
/// gatekeeping for the rebuild (the "open harness" decision, recorded in
/// `docs/14` §9 and mirrored in `docs/13` §C). What remains is the structural
/// minimum both policies shared: an id must exist after trimming. Family ids
/// travel verbatim (`AI-ADAPTER-11`) — OpenAI-compat tags (`llama3:70b`),
/// Bedrock ARNs, deployment names, and Claude ids are all just ids now.
pub fn validate_open_model_id(id: &str) -> Result<(), String> {
    let id = id.trim();
    if id.is_empty() {
        return Err("model id must be non-empty".to_string());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Credential / header shape
// ---------------------------------------------------------------------------

/// Header names that smuggle credentials (`docs/08` interaction catalog: an
/// "Authorization" header is a credential; `TEST-SEC-004`).
///
/// `anthropic-version` is deliberately absent: it is version metadata, not a
/// credential, and the gateway contract sends it on every request (`docs/09` §2.2).
const CREDENTIAL_HEADER_NAMES: [&str; 4] = [
    "authorization",
    "proxy-authorization",
    "x-api-key",
    "anthropic-api-key",
];

fn is_credential_header_name(name: &str) -> bool {
    let name = name.trim().to_ascii_lowercase();
    CREDENTIAL_HEADER_NAMES.contains(&name.as_str())
}

/// Values that look like credentials (`sk-ant-…` keys/tokens; "Bearer " prefix carry).
fn is_credential_shaped_value(value: &str) -> bool {
    let v = value.trim();
    v.starts_with("sk-ant-") || v.to_ascii_lowercase().starts_with("bearer ")
}

/// A SecretSlot-shaped value: plaintext string (legacy) or keychain reference object
/// (`crates/secrets` SecretSlot wire shapes; both accepted here so validation sees
/// foreign/legacy profiles).
fn secret_slot_present(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::String(s) => !s.trim().is_empty(),
        Value::Object(map) => {
            map.contains_key("kind") && map.contains_key("account") || map.contains_key("secretRef")
        }
        _ => false,
    }
}

fn url_value(value: Option<&Value>) -> Result<Option<&str>, String> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => {
            let s = s.trim();
            if s.is_empty() {
                Ok(None)
            } else if s == "http://" || s == "https://" {
                Err(format!("URL {s:?} has no host"))
            } else {
                Ok(Some(s))
            }
        }
        Some(other) => Err(format!("expected a URL string, found {other:?}")),
    }
}

fn text_present(value: Option<&Value>) -> bool {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|s| !s.is_empty())
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

/// Validate a profile's Connection section for one provider. Operates on the raw
/// section values (flat `inference*` keys) — draft validation reads exactly what the
/// renderer edits (`docs/features/third-party-setup.md` "Test/probe must not persist
/// a draft"; validation is read-only by construction).
pub fn validate_connection(provider: ProviderKind, connection: &Value) -> Vec<FormIssue> {
    let mut issues: Vec<FormIssue> = Vec::new();
    let conn = connection.as_object().cloned().unwrap_or_default();
    let get = |key: &str| conn.get(key);

    // -- credential kind: must be one the family offers --------------------------
    // An absent kind is the shipped form's default (the first option in the
    // selector) and is not itself an error; a present-but-unknown spelling is.
    let kind_raw = conn
        .get("inferenceCredentialKind")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let parsed_kind = kind_raw.and_then(|raw| {
        serde_json::from_value::<CredentialKind>(Value::String(raw.to_string())).ok()
    });
    let allowed: Vec<CredentialKind> = schema_kinds(provider).iter().map(|(_, k)| *k).collect();
    if let Some(raw) = kind_raw {
        match parsed_kind {
            Some(k) if allowed.contains(&k) => {}
            Some(k) => issues.push(error(
                "inferenceCredentialKind",
                format!(
                    "credential kind {:?} is not offered by this provider",
                    wire_kind(k)
                ),
            )),
            None => issues.push(error(
                "inferenceCredentialKind",
                format!("unknown credential kind {raw:?}"),
            )),
        }
    }
    let kind = parsed_kind.unwrap_or(CredentialKind::Static);
    let static_like = kind == CredentialKind::Static;
    let helper_kind = kind == CredentialKind::HelperScript;

    // -- helper-script fields (any family that offers the helper kind) ------------
    if helper_kind {
        if !text_present(get("inferenceCredentialHelper")) {
            issues.push(error(
                "inferenceCredentialHelper",
                "helper script is required for helper credential kind".to_string(),
            ));
        }
        if let Some(timeout) = get("inferenceCredentialHelperTimeoutSec") {
            if !timeout.as_u64().is_some_and(|secs| secs > 0) {
                issues.push(error(
                    "inferenceCredentialHelperTimeoutSec",
                    format!(
                        "helper timeout must be a positive number of seconds, found {timeout:?}"
                    ),
                ));
            }
        }
        if let Some(ttl) = get("inferenceCredentialHelperTtlSec") {
            if !ttl.as_u64().is_some_and(|secs| secs > 0) {
                issues.push(error(
                    "inferenceCredentialHelperTtlSec",
                    format!("helper TTL must be a positive number of seconds, found {ttl:?}"),
                ));
            }
        }
    }

    // -- per-provider required fields --------------------------------------------
    match provider {
        ProviderKind::Gateway => {
            match url_value(get("inferenceGatewayBaseUrl")) {
                Ok(None) => issues.push(error(
                    "inferenceGatewayBaseUrl",
                    "Gateway base URL is required".to_string(),
                )),
                Err(e) => issues.push(error("inferenceGatewayBaseUrl", e)),
                Ok(Some(u)) => {
                    if !u.starts_with("https://") && !u.starts_with("http://") {
                        issues.push(error(
                            "inferenceGatewayBaseUrl",
                            "base URL must start with http:// or https://".to_string(),
                        ));
                    }
                }
            }
            // Gateway API key — static credential only; helper/interactive have no
            // static key (the kind is the only source used, no fallback).
            if static_like
                && !secret_slot_present(get("inferenceGatewayApiKey").unwrap_or(&Value::Null))
            {
                issues.push(error(
                    "inferenceGatewayApiKey",
                    "Gateway API key is required for static credential kind".to_string(),
                ));
            }
            if let Some(s) = get("inferenceGatewayAuthScheme").and_then(Value::as_str) {
                if !matches!(s.trim(), "bearer" | "x-api-key") {
                    issues.push(error(
                        "inferenceGatewayAuthScheme",
                        format!("auth scheme {s:?} is not bearer or x-api-key"),
                    ));
                }
            }
            // Artifact preview origin: userContentRendererUrl (HTTPS origin; docs/09
            // "defaults to commercial host")
            match url_value(get("userContentRendererUrl")) {
                Ok(Some(u)) if !u.starts_with("https://") => issues.push(warning(
                    "userContentRendererUrl",
                    "artifact preview origins should be https://".to_string(),
                )),
                Err(e) => issues.push(error("userContentRendererUrl", e)),
                _ => {}
            }
            // Stream idle timeout: default 300 s, positive integer
            idle_timeout_check(&conn, &mut issues);
        }
        ProviderKind::ClaudeApi => {
            if static_like
                && !secret_slot_present(get("inferenceAnthropicApiKey").unwrap_or(&Value::Null))
            {
                issues.push(error(
                    "inferenceAnthropicApiKey",
                    "API key is required for static credential kind".to_string(),
                ));
            }
            // Optional base URL (docs/09 §2.2 "optional base URL"). The key name is
            // not in the captured key index — see `docs/14` §9.
            if let Some(u) = url_value(get(UNVERIFIED_ANTHROPIC_BASE_URL)).err() {
                issues.push(error(UNVERIFIED_ANTHROPIC_BASE_URL, u));
            }
        }
        ProviderKind::Bedrock => {
            if !text_present(get("inferenceBedrockRegion")) {
                issues.push(error(
                    "inferenceBedrockRegion",
                    "AWS region is required".to_string(),
                ));
            }
            if static_like
                && !secret_slot_present(get("inferenceBedrockBearerToken").unwrap_or(&Value::Null))
            {
                issues.push(error(
                    "inferenceBedrockBearerToken",
                    "bearer token is required for static credential kind".to_string(),
                ));
            }
            if let Some(u) = url_value(get("inferenceBedrockBaseUrl")).err() {
                issues.push(error("inferenceBedrockBaseUrl", u));
            }
            // Open model layer: the list is freeform (`docs/14` §9 owner
            // decision). The AWS-availability warning is user guidance, not policy.
            validate_model_list(get("inferenceModels"), &mut issues);
            let has_models = get("inferenceModels").is_some_and(model_list_present);
            if has_models {
                issues.push(warning(
                    "inferenceModels",
                    "model names are not verified as available with your AWS account; verify with the AWS console".to_string(),
                ));
            }
        }
        ProviderKind::BedrockMantle => {
            // Region/base URL; static or helper need credentials. The key names are
            // not in the captured key index — see `docs/14` §9.
            if !text_present(get(UNVERIFIED_MANTLE_REGION)) {
                issues.push(error(
                    UNVERIFIED_MANTLE_REGION,
                    "region is required".to_string(),
                ));
            }
            if let Some(u) = url_value(get(UNVERIFIED_MANTLE_BASE_URL)).err() {
                issues.push(error(UNVERIFIED_MANTLE_BASE_URL, u));
            }
            if static_like
                && !secret_slot_present(get(UNVERIFIED_MANTLE_API_KEY).unwrap_or(&Value::Null))
            {
                issues.push(error(
                    UNVERIFIED_MANTLE_API_KEY,
                    "API key is required for static credential kind".to_string(),
                ));
            }
            // Model discovery disabled for static/helper (docs/09 §2.2 Mantle).
            if discovery_enabled(get("modelDiscoveryEnabled")) && (static_like || helper_kind) {
                issues.push(warning(
                    "modelDiscoveryEnabled",
                    "model discovery is unavailable for static/helper-script credential kinds"
                        .to_string(),
                ));
            }
        }
        ProviderKind::Foundry => {
            if !text_present(get("inferenceFoundryResource")) {
                issues.push(error(
                    "inferenceFoundryResource",
                    "Azure resource name is required".to_string(),
                ));
            }
            if let Some(u) = url_value(get("inferenceFoundryBaseUrl")).err() {
                issues.push(error("inferenceFoundryBaseUrl", u));
            }
            if static_like
                && !secret_slot_present(get("inferenceFoundryApiKey").unwrap_or(&Value::Null))
            {
                issues.push(error(
                    "inferenceFoundryApiKey",
                    "API key is required for static credential kind".to_string(),
                ));
            }
            // Deployment names follow the open model rule (`docs/14` §9 owner
            // decision): any non-empty deployment id is accepted and travels
            // verbatim. The key name is not in the captured key index — see
            // `docs/14` §9.
            match get(UNVERIFIED_FOUNDRY_DEPLOYMENT)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                Some(name) => {
                    if let Err(e) = validate_open_model_id(name) {
                        issues.push(error(UNVERIFIED_FOUNDRY_DEPLOYMENT, e));
                    }
                }
                None => issues.push(error(
                    UNVERIFIED_FOUNDRY_DEPLOYMENT,
                    "deployment name is required".to_string(),
                )),
            }
        }
        ProviderKind::OpenAi => {
            // First-party OpenAI: a static key is the family's only offered
            // credential kind; the base URL is optional (empty = first-party
            // host; the field's spelling is owner-defined, `docs/14` §9).
            if static_like
                && !secret_slot_present(get("inferenceOpenAiApiKey").unwrap_or(&Value::Null))
            {
                issues.push(error(
                    "inferenceOpenAiApiKey",
                    "OpenAI API key is required for static credential kind".to_string(),
                ));
            }
            match url_value(get("inferenceOpenAiBaseUrl")) {
                Err(e) => issues.push(error("inferenceOpenAiBaseUrl", e)),
                Ok(Some(u)) => {
                    if !u.starts_with("https://") && !u.starts_with("http://") {
                        issues.push(error(
                            "inferenceOpenAiBaseUrl",
                            "base URL must start with http:// or https://".to_string(),
                        ));
                    }
                }
                Ok(None) => {}
            }
            idle_timeout_check(&conn, &mut issues);
        }
        ProviderKind::OpenAiResponses => {
            // First-party OpenAI over the Responses wire (Phase 8 slice 1's
            // extension, `docs/13` Phase 8; the readings are `docs/14` §9
            // item 71): a static key is required — the wire has no keyless
            // variant — and the base URL is optional (empty = the
            // first-party host). The field spellings are owner-defined
            // rebuild keys recorded like every open-layer family's.
            if static_like
                && !secret_slot_present(
                    get("inferenceOpenAiResponsesApiKey").unwrap_or(&Value::Null),
                )
            {
                issues.push(error(
                    "inferenceOpenAiResponsesApiKey",
                    "OpenAI API key is required for static credential kind".to_string(),
                ));
            }
            match url_value(get("inferenceOpenAiResponsesBaseUrl")) {
                Err(e) => issues.push(error("inferenceOpenAiResponsesBaseUrl", e)),
                Ok(Some(u)) => {
                    if !u.starts_with("https://") && !u.starts_with("http://") {
                        issues.push(error(
                            "inferenceOpenAiResponsesBaseUrl",
                            "base URL must start with http:// or https://".to_string(),
                        ));
                    }
                }
                Ok(None) => {}
            }
            idle_timeout_check(&conn, &mut issues);
        }
        ProviderKind::Custom => {
            // An OpenAI-compatible endpoint the user defines. The base URL is
            // required; the key is optional — self-hosted servers often serve
            // without one (`docs/13` §C open model layer).
            match url_value(get("inferenceCustomBaseUrl")) {
                Ok(None) => issues.push(error(
                    "inferenceCustomBaseUrl",
                    "custom base URL is required".to_string(),
                )),
                Err(e) => issues.push(error("inferenceCustomBaseUrl", e)),
                Ok(Some(u)) => {
                    if !u.starts_with("https://") && !u.starts_with("http://") {
                        issues.push(error(
                            "inferenceCustomBaseUrl",
                            "base URL must start with http:// or https://".to_string(),
                        ));
                    }
                }
            }
            idle_timeout_check(&conn, &mut issues);
        }
    }

    // -- shared model-list rule: freeform ids for every family ---------------------
    // (`docs/14` §9 owner decision: the model policy gatekeeping was withdrawn;
    // only empty entries are structural errors.)

    // -- custom headers: routing/tenant only; credential-shaped values rejected ---
    if let Some(headers) = get("inferenceCustomHeaders").and_then(Value::as_object) {
        for (name, value) in headers {
            if is_credential_header_name(name) {
                issues.push(error(
                    "inferenceCustomHeaders",
                    format!("custom header {name:?} carries credentials; use the credential section instead"),
                ));
            }
            if let Some(s) = value.as_str() {
                if is_credential_shaped_value(s) {
                    issues.push(error(
                        "inferenceCustomHeaders",
                        format!("custom header {name:?} value looks like a credential; routing metadata only"),
                    ));
                }
            }
        }
    }

    issues.sort_by(|a, b| (&a.field, &a.message).cmp(&(&b.field, &b.message)));
    issues
}

/// The wire spelling of a credential kind (`docs/09` §2.2 kind table).
pub fn wire_kind(kind: CredentialKind) -> &'static str {
    match kind {
        CredentialKind::Static => "static",
        CredentialKind::HelperScript => "helper-script",
        CredentialKind::Interactive => "interactive",
        CredentialKind::VendorProfile => "vendor-profile",
        CredentialKind::Workforce => "workforce",
        CredentialKind::ExternalIdp => "external-idp",
        CredentialKind::Sso => "sso",
    }
}

/// True when a model list holds at least one entry (an absent or empty list means
/// "no explicit models", which is only invalid when discovery is also off).
fn model_list_present(value: &Value) -> bool {
    match value {
        Value::Array(entries) => !entries.is_empty(),
        Value::Object(_) => true,
        _ => false,
    }
}

fn discovery_enabled(value: Option<&Value>) -> bool {
    value.map(|v| v == &Value::Bool(true)).unwrap_or(false)
}

/// Validate every entry of `inferenceModels` (array; legacy {"name":…} objects pass
/// through untouched — `docs/20` records "array/object" without the object grammar).
/// The open model rule (`validate_open_model_id`) applies to every family since the
/// owner decision (`docs/14` §9).
fn validate_model_list(list: Option<&Value>, issues: &mut Vec<FormIssue>) {
    match list {
        Some(Value::Array(entries)) => {
            if entries.is_empty() {
                return;
            }
            for (index, entry) in entries.iter().enumerate() {
                let Some(id) = entry.as_str() else {
                    // Object entries were not captured in a form edit; keep them but
                    // flag so the UI surfaces the unknown shape instead of silently
                    // dropping state (docs/14 §9 discipline).
                    issues.push(warning(
                        "inferenceModels",
                        format!("model list entry {index} is not a plain string; its object shape is unobserved"),
                    ));
                    continue;
                };
                if let Err(e) = validate_open_model_id(id) {
                    issues.push(error("inferenceModels", format!("model {index}: {e}")));
                }
            }
        }
        Some(Value::Object(_)) => {
            issues.push(warning(
                "inferenceModels",
                "model list uses the object shape; its inner grammar is unobserved".to_string(),
            ));
        }
        Some(other) => issues.push(error(
            "inferenceModels",
            format!("model list must be an array, found {other:?}"),
        )),
        None => {}
    }
}

/// The stream idle-timeout field's shared shape check (`docs/20`
/// `inferenceStreamIdleTimeoutSec`): present ⇒ a positive integer of seconds.
fn idle_timeout_check(conn: &serde_json::Map<String, Value>, issues: &mut Vec<FormIssue>) {
    if let Some(t) = conn.get("inferenceStreamIdleTimeoutSec") {
        if !t.as_u64().is_some_and(|secs| secs > 0) {
            issues.push(error(
                "inferenceStreamIdleTimeoutSec",
                format!("stream idle timeout must be a positive number of seconds, found {t:?}"),
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// Schemas
// ---------------------------------------------------------------------------

/// Key names for documented fields that were **not** captured in the key index
/// (`docs/20` §2.1). The fields themselves are documented in `docs/09` §2.2; only
/// their on-wire spelling is unknown. These are recorded in `docs/14` §9 — the
/// rebuild contract is that the setup window writes these keys and the key index is
/// corrected when the shipped spelling is observed.
const UNVERIFIED_ANTHROPIC_BASE_URL: &str = "inferenceAnthropicBaseUrl";
const UNVERIFIED_MANTLE_REGION: &str = "inferenceBedrockMantleRegion";
const UNVERIFIED_MANTLE_BASE_URL: &str = "inferenceBedrockMantleBaseUrl";
const UNVERIFIED_MANTLE_API_KEY: &str = "inferenceBedrockMantleApiKey";
const UNVERIFIED_FOUNDRY_DEPLOYMENT: &str = "inferenceFoundryDeploymentName";

/// Every key whose on-wire spelling is unverified (see `docs/14` §9).
pub const UNVERIFIED_KEYS: [&str; 5] = [
    UNVERIFIED_ANTHROPIC_BASE_URL,
    UNVERIFIED_MANTLE_REGION,
    UNVERIFIED_MANTLE_BASE_URL,
    UNVERIFIED_MANTLE_API_KEY,
    UNVERIFIED_FOUNDRY_DEPLOYMENT,
];

fn field(key: &'static str, label: &'static str, kind: FieldKind, group: FieldGroup) -> FieldDef {
    FieldDef {
        key,
        label,
        kind,
        group,
        required: false,
        required_when: &[],
        options: vec![],
        help: None,
    }
}

fn required(mut field: FieldDef) -> FieldDef {
    field.required = true;
    field
}

fn required_for(mut field: FieldDef, kinds: &'static [CredentialKind]) -> FieldDef {
    field.required = true;
    field.required_when = kinds;
    field
}

fn with_options(mut field: FieldDef, options: Vec<FieldOption>) -> FieldDef {
    field.options = options;
    field
}

fn with_help(mut field: FieldDef, help: &'static str) -> FieldDef {
    field.help = Some(help);
    field
}

/// Connection-tab field schema for the form renderer (`docs/screens` Connection tab:
/// credential card with Test, endpoint, models below the fold, proxy/VM same scroll).
pub fn connection_schema(provider: ProviderKind) -> FormSchema {
    let mut fields: Vec<FieldDef> = vec![with_help(
        with_options(
            required(field(
                "inferenceCredentialKind",
                "Credential kind",
                FieldKind::Select,
                FieldGroup::Credentials,
            )),
            schema_kinds(provider)
                .iter()
                .map(|(label, kind)| FieldOption {
                    value: wire_kind(*kind),
                    label,
                })
                .collect(),
        ),
        "When set, only that source is used (no fallback).",
    )];

    // -- provider-specific credential + endpoint fields ---------------------------
    match provider {
        ProviderKind::Gateway => {
            fields.push(required_for(
                field(
                    "inferenceGatewayApiKey",
                    "Gateway API key",
                    FieldKind::Secret,
                    FieldGroup::Credentials,
                ),
                &[CredentialKind::Static],
            ));
            fields.push(required(field(
                "inferenceGatewayBaseUrl",
                "Gateway base URL",
                FieldKind::Url,
                FieldGroup::Endpoint,
            )));
            fields.push(with_options(
                required(field(
                    "inferenceGatewayAuthScheme",
                    "Auth scheme",
                    FieldKind::Select,
                    FieldGroup::Credentials,
                )),
                vec![
                    FieldOption::plain("bearer"),
                    FieldOption::plain("x-api-key"),
                ],
            ));
            fields.push(with_help(
                field(
                    "userContentRendererUrl",
                    "Artifact preview iframe origin",
                    FieldKind::Url,
                    FieldGroup::Endpoint,
                ),
                "HTTPS origin of the user-content renderer; defaults to the commercial host.",
            ));
            fields.push(with_help(
                field(
                    "inferenceStreamIdleTimeoutSec",
                    "Stream idle timeout",
                    FieldKind::Number,
                    FieldGroup::Advanced,
                ),
                "Default 300 s.",
            ));
            fields.push(field(
                "inferenceGatewayManagedClaudeCode",
                "Gateway-managed Claude Code",
                FieldKind::Switch,
                FieldGroup::Advanced,
            ));
        }
        ProviderKind::OpenAi => {
            // First-party OpenAI: the family's credential is a static key and
            // the endpoint is first-party unless overridden. The field spellings
            // are owner-defined for the open model layer (`docs/13` §C; recorded
            // in `docs/14` §9 like every other rebuild-defined key).
            fields.push(required_for(
                field(
                    "inferenceOpenAiApiKey",
                    "OpenAI API key",
                    FieldKind::Secret,
                    FieldGroup::Credentials,
                ),
                &[CredentialKind::Static],
            ));
            fields.push(with_help(
                field(
                    "inferenceOpenAiBaseUrl",
                    "OpenAI base URL",
                    FieldKind::Url,
                    FieldGroup::Endpoint,
                ),
                "Leave empty for the first-party endpoint (https://api.openai.com/v1).",
            ));
            fields.push(with_help(
                field(
                    "inferenceStreamIdleTimeoutSec",
                    "Stream idle timeout",
                    FieldKind::Number,
                    FieldGroup::Advanced,
                ),
                "Default 300 s.",
            ));
        }
        ProviderKind::OpenAiResponses => {
            // The Responses extension's fields (Phase 8 slice 2; the readings
            // are `docs/14` §9 item 71): the family's own key spellings — a
            // profile that switches family resets the family-specific ones,
            // so the Chat Completions slot never leaks onto this wire.
            fields.push(required_for(
                field(
                    "inferenceOpenAiResponsesApiKey",
                    "OpenAI API key",
                    FieldKind::Secret,
                    FieldGroup::Credentials,
                ),
                &[CredentialKind::Static],
            ));
            fields.push(with_help(
                field(
                    "inferenceOpenAiResponsesBaseUrl",
                    "OpenAI base URL",
                    FieldKind::Url,
                    FieldGroup::Endpoint,
                ),
                "Leave empty for the first-party endpoint (https://api.openai.com/v1).",
            ));
            fields.push(with_help(
                field(
                    "inferenceStreamIdleTimeoutSec",
                    "Stream idle timeout",
                    FieldKind::Number,
                    FieldGroup::Advanced,
                ),
                "Default 300 s.",
            ));
        }
        ProviderKind::Custom => {
            // The user-defined OpenAI-compatible endpoint (the owner's "custom
            // provider"): base URL required, credential optional — self-hosted
            // servers often serve without one (`docs/13` §C).
            fields.push(with_help(
                field(
                    "inferenceCustomApiKey",
                    "API key",
                    FieldKind::Secret,
                    FieldGroup::Credentials,
                ),
                "Optional: self-hosted servers may serve without a credential.",
            ));
            fields.push(required(field(
                "inferenceCustomBaseUrl",
                "Custom base URL",
                FieldKind::Url,
                FieldGroup::Endpoint,
            )));
            fields.push(with_help(
                field(
                    "inferenceStreamIdleTimeoutSec",
                    "Stream idle timeout",
                    FieldKind::Number,
                    FieldGroup::Advanced,
                ),
                "Default 300 s.",
            ));
        }
        ProviderKind::ClaudeApi => {
            fields.push(required_for(
                field(
                    "inferenceAnthropicApiKey",
                    "API key",
                    FieldKind::Secret,
                    FieldGroup::Credentials,
                ),
                &[CredentialKind::Static],
            ));
            fields.push(field(
                UNVERIFIED_ANTHROPIC_BASE_URL,
                "Base URL",
                FieldKind::Url,
                FieldGroup::Endpoint,
            ));
        }
        ProviderKind::Bedrock => {
            fields.push(required(field(
                "inferenceBedrockRegion",
                "AWS region",
                FieldKind::Text,
                FieldGroup::Endpoint,
            )));
            fields.push(required_for(
                field(
                    "inferenceBedrockBearerToken",
                    "AWS bearer token",
                    FieldKind::Secret,
                    FieldGroup::Credentials,
                ),
                &[CredentialKind::Static],
            ));
            fields.push(field(
                "inferenceBedrockProfile",
                "AWS profile name",
                FieldKind::Text,
                FieldGroup::Credentials,
            ));
            fields.push(field(
                "inferenceBedrockSsoStartUrl",
                "AWS SSO start URL",
                FieldKind::Url,
                FieldGroup::Credentials,
            ));
            fields.push(field(
                "inferenceBedrockSsoAccountId",
                "AWS SSO account ID",
                FieldKind::Text,
                FieldGroup::Credentials,
            ));
            fields.push(field(
                "inferenceBedrockSsoRegion",
                "AWS SSO region",
                FieldKind::Text,
                FieldGroup::Credentials,
            ));
            fields.push(field(
                "inferenceBedrockSsoRoleName",
                "AWS SSO role name",
                FieldKind::Text,
                FieldGroup::Credentials,
            ));
            fields.push(field(
                "inferenceBedrockWebIdentityRoleArn",
                "Web identity role ARN",
                FieldKind::Text,
                FieldGroup::Credentials,
            ));
            fields.push(field(
                "inferenceBedrockBaseUrl",
                "Bedrock base URL",
                FieldKind::Url,
                FieldGroup::Endpoint,
            ));
            fields.push(field(
                "inferenceBedrockServiceTier",
                "Service tier",
                FieldKind::Text,
                FieldGroup::Advanced,
            ));
            fields.push(field(
                "inferenceBedrockAwsDir",
                "AWS config directory",
                FieldKind::Text,
                FieldGroup::Advanced,
            ));
            fields.push(field(
                "inferenceBedrockAwsCliPath",
                "AWS CLI path",
                FieldKind::Text,
                FieldGroup::Advanced,
            ));
        }
        ProviderKind::BedrockMantle => {
            fields.push(required(field(
                UNVERIFIED_MANTLE_REGION,
                "Region",
                FieldKind::Text,
                FieldGroup::Endpoint,
            )));
            fields.push(required_for(
                field(
                    UNVERIFIED_MANTLE_API_KEY,
                    "API key",
                    FieldKind::Secret,
                    FieldGroup::Credentials,
                ),
                &[CredentialKind::Static],
            ));
            fields.push(field(
                UNVERIFIED_MANTLE_BASE_URL,
                "Base URL",
                FieldKind::Url,
                FieldGroup::Endpoint,
            ));
        }
        ProviderKind::Foundry => {
            fields.push(required(field(
                "inferenceFoundryResource",
                "Azure resource name",
                FieldKind::Text,
                FieldGroup::Endpoint,
            )));
            fields.push(required_for(
                field(
                    "inferenceFoundryApiKey",
                    "API key",
                    FieldKind::Secret,
                    FieldGroup::Credentials,
                ),
                &[CredentialKind::Static],
            ));
            fields.push(field(
                "inferenceFoundryBaseUrl",
                "Foundry base URL",
                FieldKind::Url,
                FieldGroup::Endpoint,
            ));
            fields.push(field(
                "inferenceFoundryTenantId",
                "Entra ID tenant ID",
                FieldKind::Text,
                FieldGroup::Credentials,
            ));
            fields.push(field(
                "inferenceFoundryClientId",
                "Entra ID client ID",
                FieldKind::Text,
                FieldGroup::Credentials,
            ));
            fields.push(field(
                "inferenceFoundryAuthFlow",
                "Entra ID sign-in flow",
                FieldKind::Text,
                FieldGroup::Credentials,
            ));
            fields.push(required(field(
                UNVERIFIED_FOUNDRY_DEPLOYMENT,
                "Deployment name",
                FieldKind::Text,
                FieldGroup::Models,
            )));
        }
    }

    // -- helper-script fields (shown when the helper kind is selected) -------------
    fields.push(field(
        "inferenceCredentialHelper",
        "Helper script",
        FieldKind::Text,
        FieldGroup::Credentials,
    ));
    fields.push(field(
        "inferenceCredentialHelperArgs",
        "Helper script arguments",
        FieldKind::Text,
        FieldGroup::Credentials,
    ));
    fields.push(field(
        "inferenceCredentialHelperWindows",
        "Helper script (Windows)",
        FieldKind::Text,
        FieldGroup::Credentials,
    ));
    fields.push(field(
        "inferenceCredentialHelperTimeoutSec",
        "Helper script timeout",
        FieldKind::Number,
        FieldGroup::Advanced,
    ));
    fields.push(field(
        "inferenceCredentialHelperTtlSec",
        "Helper script TTL",
        FieldKind::Number,
        FieldGroup::Advanced,
    ));
    fields.push(field(
        "inferenceCredentialHelperSilentRefreshEnabled",
        "Re-run helper for silent refresh",
        FieldKind::Switch,
        FieldGroup::Advanced,
    ));

    // -- shared fields (docs/09 §2.2 "Shared fields") ------------------------------
    fields.push(field(
        "inferenceCustomHeaders",
        "Custom inference headers",
        FieldKind::KeyValue,
        FieldGroup::Advanced,
    ));
    fields.push(field(
        "modelDiscoveryEnabled",
        "Model discovery",
        FieldKind::Switch,
        FieldGroup::Models,
    ));
    fields.push(field(
        "inferenceModels",
        "Model list",
        FieldKind::TagList,
        FieldGroup::Models,
    ));
    fields.push(with_options(
        field(
            "defaultModelEffort",
            "Default model effort",
            FieldKind::Select,
            FieldGroup::Models,
        ),
        ["low", "medium", "high", "xhigh", "max"]
            .map(FieldOption::plain)
            .to_vec(),
    ));
    fields.push(field(
        "modelPrefer1mContext",
        "Use 1M wherever it's available",
        FieldKind::Switch,
        FieldGroup::Models,
    ));
    fields.push(field(
        "modelCatalogEnabled",
        "Model catalog metadata",
        FieldKind::Switch,
        FieldGroup::Models,
    ));
    fields.push(field(
        "modelCatalogUrl",
        "Model catalog URL",
        FieldKind::Url,
        FieldGroup::Models,
    ));
    fields.push(field(
        "inferenceModelPricingEnabled",
        "Show estimated cost",
        FieldKind::Switch,
        FieldGroup::Models,
    ));
    fields.push(field(
        "inferenceModelPricingMultiplier",
        "Price multiplier",
        FieldKind::Number,
        FieldGroup::Models,
    ));
    fields.push(field(
        "egressProxyUrl",
        "Proxy server URL",
        FieldKind::Url,
        FieldGroup::Proxy,
    ));
    fields.push(field(
        "egressProxyPacUrl",
        "Proxy auto-config (PAC) URL",
        FieldKind::Url,
        FieldGroup::Proxy,
    ));
    fields.push(field(
        "coworkVmIpv6Enabled",
        "Enable IPv6 in the workspace VM",
        FieldKind::Switch,
        FieldGroup::Proxy,
    ));

    FormSchema { provider, fields }
}

/// Validate a whole profile document at Apply time: structural gates (crate core),
/// then the provider form, then the eight non-Connection setup tabs
/// ([`crate::tabs`]). Errors block Apply; warnings travel for the renderer banner.
pub fn validate_document(doc: &ProfileDocument) -> Vec<FormIssue> {
    let Some(provider) = doc.provider else {
        return vec![error(
            "inferenceProvider",
            "profile has no provider family set".to_string(),
        )];
    };
    let structural = doc
        .validate_structural()
        .err()
        .map(|e| error("profile", e.to_string()));
    let mut issues = structural.into_iter().collect::<Vec<_>>();
    issues.extend(validate_connection(provider, &doc.connection.0));
    issues.extend(crate::tabs::validate_tab(
        crate::tabs::SetupTab::Capabilities,
        &doc.capabilities.0,
    ));
    issues.extend(crate::tabs::validate_tab(
        crate::tabs::SetupTab::Connectors,
        &doc.connectors.0,
    ));
    issues.extend(crate::tabs::validate_tab(
        crate::tabs::SetupTab::Telemetry,
        &doc.telemetry.0,
    ));
    issues.extend(crate::tabs::validate_tab(
        crate::tabs::SetupTab::Limits,
        &doc.limits.0,
    ));
    issues.extend(crate::tabs::validate_tab(
        crate::tabs::SetupTab::Appearance,
        &doc.appearance.0,
    ));
    issues.extend(crate::tabs::validate_tab(
        crate::tabs::SetupTab::Plugins,
        &doc.plugins.0,
    ));
    issues.extend(crate::tabs::validate_tab(
        crate::tabs::SetupTab::Source,
        &doc.source.0,
    ));
    issues
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn connection(provider: ProviderKind, section: Value) -> Vec<FormIssue> {
        validate_connection(provider, &section)
    }

    fn errors(issues: &[FormIssue]) -> Vec<&str> {
        issues
            .iter()
            .filter(|i| i.severity == Severity::Error)
            .map(|i| i.field.as_str())
            .collect()
    }

    /// TEST-CFG-004 (docs/13 §6.2: "All six providers validate required fields and
    /// model IDs"): each family refuses its own required fields.
    #[test]
    fn every_provider_requires_its_own_fields() {
        for provider in [
            ProviderKind::Gateway,
            ProviderKind::ClaudeApi,
            ProviderKind::Bedrock,
            ProviderKind::BedrockMantle,
            ProviderKind::Foundry,
            ProviderKind::OpenAi,
            ProviderKind::Custom,
            ProviderKind::OpenAiResponses,
        ] {
            let issues = connection(provider, json!({}));
            let fields = errors(&issues);
            assert!(
                !fields.is_empty(),
                "{provider:?} accepted an empty connection section"
            );
            // The credential-kind field must never be the only complaint: a missing
            // kind is not itself an error when it defaults to the family's first
            // option.
            assert!(
                !fields.contains(&"inferenceCredentialKind"),
                "{provider:?} rejected the default credential kind: {issues:?}"
            );
        }
    }

    /// The five supported families must not share a key set: the earlier draft emitted the
    /// Gateway keys for every provider, which contradicted `docs/20` §2.1.
    #[test]
    fn connection_schemas_are_provider_specific() {
        let gateway = connection_schema(ProviderKind::Gateway);
        let bedrock = connection_schema(ProviderKind::Bedrock);
        let foundry = connection_schema(ProviderKind::Foundry);

        assert!(gateway
            .fields
            .iter()
            .any(|f| f.key == "inferenceGatewayApiKey"));
        assert!(!bedrock
            .fields
            .iter()
            .any(|f| f.key == "inferenceGatewayApiKey"));
        assert!(!foundry
            .fields
            .iter()
            .any(|f| f.key == "inferenceGatewayApiKey"));

        assert!(bedrock
            .fields
            .iter()
            .any(|f| f.key == "inferenceBedrockRegion"));
        assert!(!gateway
            .fields
            .iter()
            .any(|f| f.key == "inferenceBedrockRegion"));

        assert!(foundry
            .fields
            .iter()
            .any(|f| f.key == "inferenceFoundryResource"));

        // The open families (`docs/13` §C): one endpoint/key shape per family,
        // no leakage from the shipped five or from each other.
        let openai = connection_schema(ProviderKind::OpenAi);
        let custom = connection_schema(ProviderKind::Custom);
        let responses = connection_schema(ProviderKind::OpenAiResponses);
        assert!(openai
            .fields
            .iter()
            .any(|f| f.key == "inferenceOpenAiApiKey"));
        assert!(custom
            .fields
            .iter()
            .any(|f| f.key == "inferenceCustomBaseUrl"));
        // The Responses extension's keys are its own — the Chat Completions
        // slot never leaks onto the Responses wire (`docs/14` §9 item 71).
        assert!(responses
            .fields
            .iter()
            .any(|f| f.key == "inferenceOpenAiResponsesApiKey"));
        assert!(responses
            .fields
            .iter()
            .any(|f| f.key == "inferenceOpenAiResponsesBaseUrl"));
        assert!(!openai
            .fields
            .iter()
            .any(|f| f.key == "inferenceGatewayApiKey"));
        assert!(!custom
            .fields
            .iter()
            .any(|f| f.key == "inferenceOpenAiApiKey"));
        assert!(!responses
            .fields
            .iter()
            .any(|f| f.key == "inferenceOpenAiApiKey"));
        assert!(!responses
            .fields
            .iter()
            .any(|f| f.key == "inferenceOpenAiBaseUrl"));
    }

    /// TEST-CFG-005 (credential kind is exclusive, no fallback): a kind the family
    /// does not offer is refused.
    #[test]
    fn credential_kind_subset_is_enforced() {
        let issues = connection(
            ProviderKind::ClaudeApi,
            json!({ "inferenceCredentialKind": "helper-script" }),
        );
        assert!(errors(&issues).contains(&"inferenceCredentialKind"));
    }

    // docs/12 §2.2 — the Anthropic predicate: denylist wins over the accept list.
    // docs/09 §2.2 — Bedrock accepts the Anthropic vendor prefix or an ARN.
    //
    // The shipped predicates were withdrawn for the rebuild (owner decision
    // 2026-10-23 — the open model layer, `docs/14` §9): every family accepts
    // every id the user names, and only empty ids are structural errors. The
    // shipped policy tests above were replaced by the open rule's own tests.
    #[test]
    fn open_model_id_rule() {
        assert!(validate_open_model_id("claude-opus-4-1").is_ok());
        assert!(validate_open_model_id("gpt-4o").is_ok());
        assert!(validate_open_model_id("gemini-2.0-flash").is_ok());
        assert!(validate_open_model_id("llama3:70b").is_ok());
        assert!(validate_open_model_id("minimax-m2").is_ok());
        assert!(validate_open_model_id(
            "arn:aws:bedrock:us-east-1:123456789012:inference-profile/us.anthropic.claude-3-5-sonnet-20241022-v2:0"
        )
        .is_ok());
        assert!(validate_open_model_id("").is_err());
        assert!(validate_open_model_id("   ").is_err());
    }

    /// The OpenAI family: the key is required for static, the base URL optional
    /// (empty = first-party), and the form carries no scheme select.
    #[test]
    fn openai_family_validation() {
        // Missing key on the static kind is the error.
        let issues = connection(ProviderKind::OpenAi, json!({}));
        assert!(errors(&issues).contains(&"inferenceOpenAiApiKey"));

        // Key + nothing else validates clean (first-party endpoint).
        let issues = connection(
            ProviderKind::OpenAi,
            json!({ "inferenceOpenAiApiKey": "placeholder-credential" }),
        );
        assert!(errors(&issues).is_empty(), "{issues:?}");

        // A nonsense host is an error.
        let issues = connection(
            ProviderKind::OpenAi,
            json!({
                "inferenceOpenAiApiKey": "placeholder-credential",
                "inferenceOpenAiBaseUrl": "not-a-url",
            }),
        );
        assert!(errors(&issues).contains(&"inferenceOpenAiBaseUrl"));
    }

    /// The Responses extension's family (Phase 8): the key is required for
    /// static (the wire has no keyless variant), the base URL is optional
    /// (empty = first-party), and a nonsense host is refused.
    #[test]
    fn openai_responses_family_validation() {
        // Missing key on the static kind is the error.
        let issues = connection(ProviderKind::OpenAiResponses, json!({}));
        assert!(errors(&issues).contains(&"inferenceOpenAiResponsesApiKey"));

        // Key + nothing else validates clean (first-party endpoint).
        let issues = connection(
            ProviderKind::OpenAiResponses,
            json!({ "inferenceOpenAiResponsesApiKey": "placeholder-credential" }),
        );
        assert!(errors(&issues).is_empty(), "{issues:?}");

        // A nonsense host is an error.
        let issues = connection(
            ProviderKind::OpenAiResponses,
            json!({
                "inferenceOpenAiResponsesApiKey": "placeholder-credential",
                "inferenceOpenAiResponsesBaseUrl": "not-a-url",
            }),
        );
        assert!(errors(&issues).contains(&"inferenceOpenAiResponsesBaseUrl"));
    }

    /// The Custom family: base URL required, credential optional (self-hosted
    /// servers may serve without one), and a nonsense host is refused.
    #[test]
    fn custom_family_validation() {
        // Missing base URL is the error; the credential is NOT required.
        let issues = connection(ProviderKind::Custom, json!({}));
        assert!(errors(&issues).contains(&"inferenceCustomBaseUrl"));
        assert!(!errors(&issues).contains(&"inferenceCustomApiKey"));

        // Base URL alone validates clean.
        let issues = connection(
            ProviderKind::Custom,
            json!({ "inferenceCustomBaseUrl": "https://llm.selfhosted.example/v1" }),
        );
        assert!(errors(&issues).is_empty(), "{issues:?}");

        let issues = connection(
            ProviderKind::Custom,
            json!({ "inferenceCustomBaseUrl": "ftp://llm.selfhosted.example" }),
        );
        assert!(errors(&issues).contains(&"inferenceCustomBaseUrl"));
    }

    /// TEST-SEC-004 — custom headers carry routing metadata, not credentials.
    #[test]
    fn custom_headers_reject_credentials() {
        let issues = connection(
            ProviderKind::Gateway,
            json!({
                "inferenceGatewayBaseUrl": "https://gw.example",
                "inferenceGatewayApiKey": "placeholder-credential",
                "inferenceCustomHeaders": { "Authorization": "Bearer abc" },
            }),
        );
        assert!(errors(&issues).contains(&"inferenceCustomHeaders"));

        // `anthropic-version` is metadata, not a credential, and must be allowed.
        let issues = connection(
            ProviderKind::Gateway,
            json!({
                "inferenceGatewayBaseUrl": "https://gw.example",
                "inferenceGatewayApiKey": "placeholder-credential",
                "inferenceCustomHeaders": { "anthropic-version": "2023-06-01" },
            }),
        );
        assert!(!errors(&issues).contains(&"inferenceCustomHeaders"));
    }

    /// docs/09 §2.2 — discovery is unavailable for the listed credential kinds.
    #[test]
    fn discovery_gating_warns() {
        let issues = connection(
            ProviderKind::BedrockMantle,
            json!({
                "inferenceCredentialKind": "static",
                UNVERIFIED_MANTLE_REGION: "us-east-1",
                UNVERIFIED_MANTLE_API_KEY: "placeholder-credential",
                "modelDiscoveryEnabled": true,
            }),
        );
        assert!(issues
            .iter()
            .any(|i| i.field == "modelDiscoveryEnabled" && i.severity == Severity::Warning));
    }

    /// Helper kinds require the helper script and reject non-positive timeouts.
    #[test]
    fn helper_kind_requires_script() {
        let issues = connection(
            ProviderKind::Gateway,
            json!({
                "inferenceCredentialKind": "helper-script",
                "inferenceGatewayBaseUrl": "https://gw.example",
            }),
        );
        assert!(errors(&issues).contains(&"inferenceCredentialHelper"));

        let issues = connection(
            ProviderKind::Gateway,
            json!({
                "inferenceCredentialKind": "helper-script",
                "inferenceGatewayBaseUrl": "https://gw.example",
                "inferenceCredentialHelper": "/usr/bin/creds",
                "inferenceCredentialHelperTimeoutSec": 0,
            }),
        );
        assert!(errors(&issues).contains(&"inferenceCredentialHelperTimeoutSec"));
    }

    /// A complete gateway profile produces no error-severity issue.
    #[test]
    fn complete_gateway_profile_validates_clean() {
        let issues = connection(
            ProviderKind::Gateway,
            json!({
                "inferenceCredentialKind": "static",
                "inferenceGatewayBaseUrl": "https://gw.example/",
                "inferenceGatewayApiKey": "placeholder-credential",
                "inferenceGatewayAuthScheme": "bearer",
                "inferenceModels": ["claude-opus-4-1"],
            }),
        );
        assert!(errors(&issues).is_empty(), "{issues:?}");
    }
}
