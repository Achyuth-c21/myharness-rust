//! Export / support bundle with redaction — roadmap Phase 2 (docs/13 §4 Phase 2;
//! docs/18 §10 config contract (`export` operation), §11 error envelope;
//! docs/09 §4 sensitive data and security; `TEST-CFG-007`, `TEST-CFG-008`).
//!
//! The shipped setup window has an **Export** action (`docs/09` §2.1). It is
//! sensitive: "The setup Export action can contain secrets or embedded tokens;
//! warn and require explicit action" (`docs/09` §4), and "The diagnostic bundle
//! must enumerate its contents and require explicit consent before upload"
//! (`docs/09` §4).
//!
//! This module implements the bundle. Three properties hold:
//!
//! 1. **Redacted by default** (`TEST-CFG-007`). Known secret-bearing keys are
//!    replaced wholesale; credential-shaped header entries and values are
//!    replaced individually; URL query strings are stripped, because a token can
//!    ride in a query string.
//! 2. **Contents enumerated** (`docs/09` §4). The bundle lists every file it
//!    contains and carries the warnings the UI must show before the user acts.
//! 3. **Private infrastructure excluded** (`TEST-CFG-008`). Hosts that are
//!    private, loopback, link-local or the cloud metadata address are replaced,
//!    because "the list can reveal private infrastructure" (`docs/09` §2.9) and
//!    "Documentation must never publish a private hostname".
//!
//! The consent step itself is a UI action: the bundle exposes
//! [`ExportBundle::contents`] and [`ExportBundle::warnings`] so the setup window
//! can show them and require an explicit confirmation.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::egress::{derive, EgressAllowlist};
use crate::overlay::MergedConfig;
use crate::{ProfileDocument, ProviderKind};

/// The marker that replaces a redacted value.
pub const REDACTED: &str = "<redacted>";

/// The marker that replaces a private-infrastructure host.
pub const REDACTED_HOST: &str = "<private-host>";

/// Keys whose whole value is a credential (`docs/20` §2.1 marks them "secret
/// string"). Their values never appear in an export, and the setup window
/// preserves them across a renderer write (`docs/18` §10).
pub const SECRET_KEYS: [&str; 6] = [
    "inferenceGatewayApiKey",
    "inferenceAnthropicApiKey",
    "inferenceBedrockBearerToken",
    "inferenceFoundryApiKey",
    "inferenceVertexOAuthClientSecret",
    "otlpHeaders",
];

/// Header names that carry a credential (`docs/08`: "Authorization is treated as a
/// credential"; `TEST-SEC-004`).
const CREDENTIAL_HEADER_NAMES: [&str; 4] = [
    "authorization",
    "proxy-authorization",
    "x-api-key",
    "anthropic-api-key",
];

/// What one file in the bundle is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportEntry {
    /// The file name inside the bundle.
    pub name: String,
    /// A one-line description of what it contains — the enumeration the consent
    /// step shows (`docs/09` §4).
    pub description: String,
    /// Whether the file can disclose private infrastructure.
    pub may_contain_private_hosts: bool,
}

/// The export bundle: the redacted files, the enumeration of what they contain,
/// and the warnings the UI must show before the user acts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportBundle {
    /// Every file in the bundle, with a description.
    pub contents: Vec<ExportEntry>,
    /// The redacted file contents, keyed by file name.
    pub files: BTreeMap<String, Value>,
    /// The warnings to show before exporting.
    pub warnings: Vec<String>,
    /// Explicit consent is required before the bundle is uploaded anywhere.
    pub consent_required: bool,
}

impl ExportBundle {
    /// The names of the files in the bundle.
    pub fn file_names(&self) -> Vec<&str> {
        self.files.keys().map(String::as_str).collect()
    }
}

/// Build the redacted export bundle for a set of profiles.
///
/// `profiles` are the profile documents to include; `merged` is the merged view
/// of the applied profile, used to derive the egress allowlist.
pub fn export_profiles(
    profiles: &[(String, ProfileDocument)],
    merged: Option<&MergedConfig>,
) -> ExportBundle {
    let mut contents: Vec<ExportEntry> = Vec::new();
    let mut files: BTreeMap<String, Value> = BTreeMap::new();

    for (id, profile) in profiles {
        let name = format!("profiles/{id}.json");
        files.insert(name.clone(), redact_document_for_export(profile));
        contents.push(ExportEntry {
            name,
            description: "A third-party inference profile (redacted).".to_string(),
            may_contain_private_hosts: true,
        });
    }

    // The egress allowlist is derived, not stored (`docs/20` §6). It is useful to
    // a support request, and it is where private infrastructure would appear, so
    // its hosts are redacted.
    if let Some(merged) = merged {
        let Some(profile) = profiles
            .iter()
            .find(|(id, _)| Some(id.as_str()) == merged.profile_id.as_deref())
            .map(|(_, profile)| profile)
        else {
            return bundle(contents, files);
        };
        let allowlist = derive(profile, Some(merged));
        let name = "egress.json".to_string();
        files.insert(name.clone(), redact_allowlist(&allowlist));
        contents.push(ExportEntry {
            name,
            description: "The derived egress allowlist (private hosts redacted).".to_string(),
            may_contain_private_hosts: true,
        });
    }

    bundle(contents, files)
}

fn bundle(contents: Vec<ExportEntry>, files: BTreeMap<String, Value>) -> ExportBundle {
    ExportBundle {
        contents,
        files,
        warnings: vec![
            "This export can contain configuration that identifies your organization or \
             deployment. Secrets and private hosts are redacted, but review the file list \
             before sharing it."
                .to_string(),
            "Explicit confirmation is required before a diagnostic bundle is uploaded.".to_string(),
        ],
        consent_required: true,
    }
}

/// Where a redacted document is going.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedactTarget {
    /// The renderer: a keychain **handle** is safe to show (it names an account,
    /// not a value) and must be visible so the setup window can show that a
    /// credential is stored and offer to replace it (`docs/18` §10).
    Renderer,
    /// The export bundle: nothing that names a stored credential leaves.
    Export,
}

/// Redact a profile document for the renderer: secret keys keep their handle,
/// credential-shaped values and URL query strings are stripped, and private
/// hosts are replaced.
pub fn redact_document(profile: &ProfileDocument) -> Value {
    redact_document_for(profile, RedactTarget::Renderer)
}

/// Redact a profile document for an export bundle: handles are replaced too,
/// because a bundle can be shared.
pub fn redact_document_for_export(profile: &ProfileDocument) -> Value {
    redact_document_for(profile, RedactTarget::Export)
}

/// Redact a profile document: secret keys, credential-shaped values, URL query
/// strings and private hosts.
pub fn redact_document_for(profile: &ProfileDocument, target: RedactTarget) -> Value {
    let mut document = match serde_json::to_value(profile) {
        Ok(value) => value,
        Err(_) => Value::Null,
    };
    if let Some(object) = document.as_object_mut() {
        for (_key, value) in object.iter_mut() {
            // The unknown-key map is flattened into the document by the wire
            // format, so it is redacted like any other map.
            *value = redact_value_for(value, target);
        }
    }
    redact_private_hosts_in(&mut document);
    document
}

/// Redact an arbitrary JSON value: secret keys, credential-shaped entries and
/// values, and URL query strings.
pub fn redact_value(value: &Value) -> Value {
    redact_value_for(value, RedactTarget::Export)
}

/// Redact an arbitrary JSON value for a target.
pub fn redact_value_for(value: &Value, target: RedactTarget) -> Value {
    match value {
        Value::Object(map) => {
            let mut out = Map::new();
            for (key, item) in map {
                if SECRET_KEYS.contains(&key.as_str()) {
                    // A handle is safe for the renderer; a value never is. The
                    // export replaces both.
                    let keep_handle = target == RedactTarget::Renderer && item.is_object();
                    out.insert(
                        key.clone(),
                        if keep_handle {
                            item.clone()
                        } else {
                            Value::String(REDACTED.to_string())
                        },
                    );
                    continue;
                }
                if CREDENTIAL_HEADER_NAMES.contains(&key.to_ascii_lowercase().as_str()) {
                    // The header name is routing metadata, not a secret; the value
                    // is. The name stays so the UI can show the header exists.
                    out.insert(key.clone(), Value::String(REDACTED.to_string()));
                    continue;
                }
                out.insert(key.clone(), redact_value_for(item, target));
            }
            Value::Object(out)
        }
        Value::Array(items) => {
            Value::Array(items.iter().map(|i| redact_value_for(i, target)).collect())
        }
        Value::String(text) => {
            if looks_like_credential(text) {
                return Value::String(REDACTED.to_string());
            }
            Value::String(strip_query_string(text))
        }
        other => other.clone(),
    }
}

/// True when a string looks like a credential value.
fn looks_like_credential(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }
    trimmed.starts_with("sk-ant-") || trimmed.to_ascii_lowercase().starts_with("bearer ")
}

/// Strip the query string and fragment of a URL-shaped string. A token can ride
/// in a query string (`docs/09` §4: "Redact authorization headers and query
/// strings from logs").
fn strip_query_string(text: &str) -> String {
    let trimmed = text.trim();
    if !trimmed.contains("://") {
        return text.to_string();
    }
    let (head, tail) = match trimmed.split_once('?') {
        Some((head, tail)) => (head, tail),
        None => return trimmed.to_string(),
    };
    let head = head.split('#').next().unwrap_or(head);
    let _ = tail;
    head.to_string()
}

/// Replace private-infrastructure hosts anywhere in a document.
fn redact_private_hosts_in(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for item in map.values_mut() {
                redact_private_hosts_in(item);
            }
        }
        Value::Array(items) => {
            for item in items {
                redact_private_hosts_in(item);
            }
        }
        Value::String(text) if is_private_host(text) => {
            *text = REDACTED_HOST.to_string();
        }
        _ => {}
    }
}

/// True when a string is a private, loopback, link-local or metadata host.
///
/// Single-label names are included: an unqualified name is an internal host by
/// definition, and publishing one is exactly the leak `docs/09` §2.9 warns about.
fn is_private_host(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }
    // The redaction markers are not hosts.
    if trimmed == REDACTED || trimmed == REDACTED_HOST {
        return false;
    }
    let is_url = trimmed.contains("://");
    let host = trimmed
        .split("://")
        .last()
        .unwrap_or(trimmed)
        .split(['/', '?', '#'])
        .next()
        .unwrap_or(trimmed)
        .rsplit('@')
        .next()
        .unwrap_or(trimmed)
        .split(':')
        .next()
        .unwrap_or(trimmed)
        .to_ascii_lowercase();
    if host.is_empty() {
        return false;
    }
    if !host.contains('.') {
        // A single-label name is an internal host only in a host position - a URL
        // authority. A bare single-label value elsewhere in the document (a header
        // value, an organization name) is not a host.
        return is_url;
    }
    if host == "localhost" || host.ends_with(".localhost") || host.ends_with(".local") {
        return true;
    }
    if host == "169.254.169.254" || host == "metadata.google.internal" {
        return true;
    }
    let octets: Vec<u8> = host
        .split('.')
        .filter_map(|part| part.parse::<u8>().ok())
        .collect();
    if octets.len() == 4 {
        let [a, b, _, _] = [octets[0], octets[1], octets[2], octets[3]];
        return a == 10
            || a == 127
            || (a == 172 && (16..=31).contains(&b))
            || (a == 192 && b == 168)
            || (a == 169 && b == 254)
            || (a == 100 && (64..=127).contains(&b));
    }
    if host.starts_with("fd") || host.starts_with("fc") || host.starts_with("fe80:") {
        return true;
    }
    false
}

/// Redact the private hosts out of a derived egress allowlist.
fn redact_allowlist(allowlist: &EgressAllowlist) -> Value {
    let groups: Vec<Value> = allowlist
        .groups
        .iter()
        .map(|group| {
            let hosts: Vec<Value> = group
                .hosts
                .iter()
                .map(|host| {
                    if is_private_host(host) {
                        Value::String(REDACTED_HOST.to_string())
                    } else {
                        Value::String(host.clone())
                    }
                })
                .collect();
            serde_json::json!({
                "group": group.group,
                "hosts": hosts,
                "hostsUnobserved": group.hosts_unobserved,
            })
        })
        .collect();
    serde_json::json!({ "groups": groups })
}

/// The provider family of a profile — used by the export to label the bundle.
pub fn provider_label(profile: &ProfileDocument) -> &'static str {
    match profile.provider {
        Some(ProviderKind::ClaudeApi) => "claude-api",
        Some(ProviderKind::Gateway) => "gateway",
        Some(ProviderKind::Bedrock) => "bedrock",
        Some(ProviderKind::BedrockMantle) => "bedrock-mantle",
        Some(ProviderKind::Vertex) => "vertex",
        Some(ProviderKind::Foundry) => "foundry",
        None => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::overlay::Overlay;
    use serde_json::json;

    fn profile() -> ProfileDocument {
        ProfileDocument {
            schema_version: 1,
            id: Some("11111111-1111-4111-8111-111111111111".to_string()),
            name: Some("Profile".to_string()),
            provider: Some(ProviderKind::Gateway),
            connection: crate::Section(json!({
                "inferenceGatewayBaseUrl": "https://gw.example/v1",
                "inferenceGatewayApiKey": "sk-ant-EXAMPLE-INLINE",
                "inferenceCustomHeaders": {
                    "X-Tenant": "acme",
                    "Authorization": "Bearer example",
                },
            })),
            ..ProfileDocument::default()
        }
    }

    /// TEST-CFG-007: a secret-valued key is redacted wholesale.
    #[test]
    fn secret_keys_are_redacted() {
        let redacted = redact_document(&profile());
        assert_eq!(
            redacted["connection"]["inferenceGatewayApiKey"],
            json!(REDACTED)
        );
        assert!(!redacted.to_string().contains("sk-ant-EXAMPLE-INLINE"));
    }

    /// A credential-shaped header entry is redacted; ordinary routing metadata is
    /// kept.
    #[test]
    fn credential_headers_are_redacted_but_routing_headers_are_kept() {
        let redacted = redact_document(&profile());
        assert_eq!(
            redacted["connection"]["inferenceCustomHeaders"]["Authorization"],
            json!(REDACTED)
        );
        assert_eq!(
            redacted["connection"]["inferenceCustomHeaders"]["X-Tenant"],
            json!("acme")
        );
    }

    /// A URL query string is stripped — a token can ride in one.
    #[test]
    fn query_strings_are_stripped() {
        let mut config = profile();
        config.connection = crate::Section(json!({
            "inferenceGatewayBaseUrl": "https://gw.example/v1?token=secret-value",
        }));
        let redacted = redact_document(&config);
        assert_eq!(
            redacted["connection"]["inferenceGatewayBaseUrl"],
            json!("https://gw.example/v1")
        );
        assert!(!redacted.to_string().contains("secret-value"));
    }

    /// TEST-CFG-008: private infrastructure never leaves in the bundle.
    #[test]
    fn private_hosts_are_redacted() {
        let mut config = profile();
        config.connection = crate::Section(json!({
            "inferenceGatewayBaseUrl": "https://gateway/v1",
            "inferenceBedrockBaseUrl": "https://10.0.0.5/bedrock",
        }));
        let redacted = redact_document(&config);
        assert!(!redacted.to_string().contains("10.0.0.5"));
        assert!(!redacted.to_string().contains("https://gateway/v1"));
        assert!(redacted.to_string().contains(REDACTED_HOST));
    }

    /// The bundle enumerates its contents and requires consent.
    #[test]
    fn bundle_enumerates_contents_and_requires_consent() {
        let config = profile();
        let id = config.id.clone().unwrap();
        let merged = crate::overlay::merge(&config, None);
        let bundle = export_profiles(&[(id.clone(), config)], Some(&merged));
        assert!(bundle.consent_required);
        assert!(!bundle.warnings.is_empty());
        assert!(bundle
            .contents
            .iter()
            .any(|c| c.name == format!("profiles/{id}.json")));
        assert!(bundle.contents.iter().any(|c| c.name == "egress.json"));
        for entry in &bundle.contents {
            assert!(bundle.files.contains_key(&entry.name));
        }
    }

    /// The exported egress list has its private hosts redacted.
    #[test]
    fn exported_egress_redacts_private_hosts() {
        let mut config = profile();
        config.connection = crate::Section(json!({
            "inferenceGatewayBaseUrl": "https://10.1.2.3/v1",
        }));
        let id = config.id.clone().unwrap();
        let overlay = Overlay::parse(&json!({
            "connection": { "inferenceGatewayBaseUrl": "https://10.1.2.3/v1" },
        }));
        let merged = crate::overlay::merge(&config, Some(&overlay));
        let bundle = export_profiles(&[(id, config)], Some(&merged));
        let egress = bundle.files.get("egress.json").unwrap().to_string();
        assert!(!egress.contains("10.1.2.3"), "{egress}");
        assert!(egress.contains(REDACTED_HOST));
    }

    /// The redaction never touches the library's own copy of the document.
    #[test]
    fn redaction_does_not_mutate_the_profile() {
        let config = profile();
        let _ = redact_document(&config);
        assert_eq!(
            config.connection.0["inferenceGatewayApiKey"],
            json!("sk-ant-EXAMPLE-INLINE")
        );
    }
}
