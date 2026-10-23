//! Egress allowlist derivation — roadmap Phase 2 (docs/13 §4 Phase 2; docs/09 §2.9
//! Egress tab; docs/12 §1 provider host shapes; docs/20 §6 "Egress | Recompute from
//! active config"; `TEST-CFG-008`).
//!
//! The Egress tab is a **read-only derived allowlist**: it is recomputed from the
//! active configuration, never edited, and it is grouped by purpose
//! (`docs/09` §2.9). Unapproved hosts are blocked at request time
//! (`docs/20` §6) — the derivation is the list, not the enforcement.
//!
//! What the docs pin exactly:
//!
//! - the group names and the hosts they name verbatim (`downloads.claude.ai`,
//!   `claude.ai`, `api.anthropic.com`, `claudeusercontent.com`,
//!   `claudemcpcontent.com`, `assets.claude.ai`);
//! - the provider default host shapes and the Bedrock control-plane hosts
//!   (`docs/12` §1);
//! - that the group boundaries are "core downloads / auto-updates / essential
//!   telemetry / nonessential services / gateway base URL / plugin marketplaces /
//!   managed MCP servers / user MCP + Python runtime / SSH and identity endpoints".
//!
//! What the docs do **not** pin: the hostnames of the Sentry and Datadog intake,
//! the Google static/font hosts, the cdnjs and jsDelivr hosts, and the Python
//! package hosts. Those groups are emitted with [`EgressGroup::hosts_unobserved`]
//! set, so the gap is visible in the product instead of being filled with invented
//! hostnames (`RULES.md` §1 evidence honesty; `docs/14` §9 item 16).
//!
//! The list can reveal private infrastructure (`docs/09` §2.9), so the export
//! path redacts it — see [`crate::export`].

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::overlay::MergedConfig;
use crate::{ProfileDocument, ProviderKind};

/// The egress groups (`docs/09` §2.9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EgressGroupKind {
    CoreDownloads,
    AutoUpdates,
    EssentialTelemetry,
    NonessentialServices,
    ProviderEndpoint,
    PluginMarketplaces,
    ManagedMcpServers,
    UserMcpRuntime,
    SshIdentityEndpoints,
}

/// One group of the derived allowlist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EgressGroup {
    pub group: EgressGroupKind,
    /// The hosts in this group that are known.
    pub hosts: Vec<String>,
    /// True when this group also contains hosts whose names were never captured.
    /// The group is present; its missing members are not invented.
    #[serde(default)]
    pub hosts_unobserved: bool,
}

/// The derived egress allowlist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EgressAllowlist {
    pub groups: Vec<EgressGroup>,
}

impl EgressAllowlist {
    /// Every known host, sorted and de-duplicated.
    pub fn hosts(&self) -> Vec<String> {
        let mut all: BTreeSet<String> = BTreeSet::new();
        for group in &self.groups {
            for host in &group.hosts {
                all.insert(host.clone());
            }
        }
        all.into_iter().collect()
    }

    /// The hosts of one group.
    pub fn group(&self, kind: EgressGroupKind) -> Option<&EgressGroup> {
        self.groups.iter().find(|g| g.group == kind)
    }

    /// Whether the list contains a host. `*` is the special "unrestricted" entry
    /// (`docs/09` §3.4) and matches any host.
    pub fn allows(&self, host: &str) -> bool {
        let host = host.trim().to_ascii_lowercase();
        self.hosts()
            .iter()
            .any(|allowed| allowed == "*" || allowed.eq_ignore_ascii_case(&host))
    }

    /// The flat `.txt` form the "Download .txt" action produces
    /// (`docs/09` §2.9) — one host per line, sorted.
    pub fn to_text(&self) -> String {
        self.hosts().join("\n")
    }
}

/// Derive the egress allowlist from the active configuration.
///
/// `merged` is the merged view (`getMerged`): when a managed overlay supplies a
/// value, the managed value wins — the derivation must describe what the app
/// will actually reach.
pub fn derive(config: &ProfileDocument, merged: Option<&MergedConfig>) -> EgressAllowlist {
    // The effective value of a key: the managed one when present, the local one
    // otherwise.
    let effective = |section: &str, key: &str| -> Option<Value> {
        if let Some(merged) = merged {
            if let Some(field) = merged.get(section, key) {
                return Some(field.value.clone());
            }
        }
        section_value(config, section)
            .and_then(|s| s.get(key))
            .cloned()
    };

    let mut groups: Vec<EgressGroup> = Vec::new();

    // -- core downloads ----------------------------------------------------------
    groups.push(EgressGroup {
        group: EgressGroupKind::CoreDownloads,
        hosts: vec!["downloads.claude.ai".to_string()],
        hosts_unobserved: false,
    });

    // -- auto-updates ------------------------------------------------------------
    // "claude.ai, api.anthropic.com (unless update host is redirected)".
    let update_host = effective("telemetry", "updateViaUpdatesHost")
        .and_then(|v| v.as_str().map(str::trim).map(str::to_string))
        .filter(|s| !s.is_empty());
    let mut auto_update_hosts = vec!["claude.ai".to_string()];
    match update_host {
        Some(host) => {
            if let Some(h) = host_of(&Value::String(host)) {
                auto_update_hosts.push(h);
            }
        }
        None => auto_update_hosts.push("api.anthropic.com".to_string()),
    }
    groups.push(EgressGroup {
        group: EgressGroupKind::AutoUpdates,
        hosts: auto_update_hosts,
        hosts_unobserved: false,
    });

    // -- essential telemetry -----------------------------------------------------
    // "Sentry and Datadog intake hosts" — the categories are documented, the
    // hostnames were not captured.
    groups.push(EgressGroup {
        group: EgressGroupKind::EssentialTelemetry,
        hosts: Vec::new(),
        hosts_unobserved: true,
    });

    // -- nonessential services ---------------------------------------------------
    groups.push(EgressGroup {
        group: EgressGroupKind::NonessentialServices,
        hosts: vec![
            "claudeusercontent.com".to_string(),
            "claudemcpcontent.com".to_string(),
            "assets.claude.ai".to_string(),
        ],
        hosts_unobserved: true,
    });

    // -- provider endpoint -------------------------------------------------------
    let mut provider_hosts = provider_hosts(config, &effective);
    provider_hosts.sort();
    provider_hosts.dedup();
    groups.push(EgressGroup {
        group: EgressGroupKind::ProviderEndpoint,
        hosts: provider_hosts,
        hosts_unobserved: false,
    });

    // -- plugin marketplaces -----------------------------------------------------
    let mut marketplace_hosts = Vec::new();
    for key in ["allowedPluginMarketplaces", "organizationPluginsUrl"] {
        if let Some(value) = effective("plugins", key) {
            marketplace_hosts.extend(collect_hosts(&value));
        }
    }
    marketplace_hosts.sort();
    marketplace_hosts.dedup();
    groups.push(EgressGroup {
        group: EgressGroupKind::PluginMarketplaces,
        hosts: marketplace_hosts,
        hosts_unobserved: false,
    });

    // -- managed MCP servers -----------------------------------------------------
    let mut managed_mcp_hosts = Vec::new();
    if let Some(value) = effective("connectors", "managedMcpServers") {
        managed_mcp_hosts.extend(collect_hosts(&value));
    }
    managed_mcp_hosts.sort();
    managed_mcp_hosts.dedup();
    groups.push(EgressGroup {
        group: EgressGroupKind::ManagedMcpServers,
        hosts: managed_mcp_hosts,
        hosts_unobserved: false,
    });

    // -- user MCP / Python runtime ----------------------------------------------
    // "PyPI, GitHub objects, Python package hosts" — the categories are
    // documented, the hostnames were not captured.
    let mut user_runtime_hosts = Vec::new();
    for key in ["allowedPluginMcpServers", "deniedPluginMcpServers"] {
        if let Some(value) = effective("connectors", key) {
            user_runtime_hosts.extend(collect_hosts(&value));
        }
    }
    user_runtime_hosts.sort();
    user_runtime_hosts.dedup();
    groups.push(EgressGroup {
        group: EgressGroupKind::UserMcpRuntime,
        hosts: user_runtime_hosts,
        hosts_unobserved: true,
    });

    // -- SSH / identity endpoints ------------------------------------------------
    let mut ssh_hosts = Vec::new();
    if let Some(value) = effective("capabilities", "sshConfigs") {
        ssh_hosts.extend(collect_hosts(&value));
    }
    if let Some(value) = effective("capabilities", "sshHostAllowlist") {
        ssh_hosts.extend(collect_hosts(&value));
    }
    ssh_hosts.sort();
    ssh_hosts.dedup();
    groups.push(EgressGroup {
        group: EgressGroupKind::SshIdentityEndpoints,
        hosts: ssh_hosts,
        hosts_unobserved: false,
    });

    EgressAllowlist { groups }
}

/// The hosts the active provider family reaches.
///
/// The default host shapes are documented in `docs/12` §1; a configured base URL
/// replaces the default. Bedrock additionally reaches the control-plane hosts
/// `docs/12` §1 names, derived from the region.
fn provider_hosts(
    config: &ProfileDocument,
    effective: &dyn Fn(&str, &str) -> Option<Value>,
) -> Vec<String> {
    let mut hosts: Vec<String> = Vec::new();
    let provider = config.provider;

    let base_url = |key: &str| -> Option<String> {
        effective("connection", key)
            .and_then(|v| v.as_str().map(str::trim).map(str::to_string))
            .filter(|s| !s.is_empty())
    };

    match provider {
        Some(ProviderKind::ClaudeApi) => {
            if let Some(url) = base_url("inferenceAnthropicBaseUrl") {
                hosts.extend(host_of(&Value::String(url)));
            } else {
                hosts.push("api.anthropic.com".to_string());
            }
        }
        Some(ProviderKind::Gateway) => {
            if let Some(url) = base_url("inferenceGatewayBaseUrl") {
                hosts.extend(host_of(&Value::String(url)));
            }
        }
        Some(ProviderKind::Bedrock) => {
            let region = effective("connection", "inferenceBedrockRegion")
                .and_then(|v| v.as_str().map(str::trim).map(str::to_string))
                .filter(|s| !s.is_empty());
            if let Some(url) = base_url("inferenceBedrockBaseUrl") {
                hosts.extend(host_of(&Value::String(url)));
            }
            if let Some(region) = region {
                hosts.push(format!("bedrock-runtime.{region}.amazonaws.com"));
                hosts.push(format!("bedrock.{region}.amazonaws.com"));
                hosts.push("sts.amazonaws.com".to_string());
                hosts.push(format!("sts.{region}.amazonaws.com"));
                hosts.push(format!("portal.sso.{region}.amazonaws.com"));
                hosts.push(format!("oidc.{region}.amazonaws.com"));
            }
        }
        Some(ProviderKind::BedrockMantle) => {
            let region = effective("connection", "inferenceBedrockMantleRegion")
                .and_then(|v| v.as_str().map(str::trim).map(str::to_string))
                .filter(|s| !s.is_empty());
            if let Some(url) = base_url("inferenceBedrockMantleBaseUrl") {
                hosts.extend(host_of(&Value::String(url)));
            }
            if let Some(region) = region {
                hosts.push(format!("bedrock-mantle.{region}.api.aws"));
            }
        }
        Some(ProviderKind::Foundry) => {
            let resource = effective("connection", "inferenceFoundryResource")
                .and_then(|v| v.as_str().map(str::trim).map(str::to_string))
                .filter(|s| !s.is_empty());
            if let Some(url) = base_url("inferenceFoundryBaseUrl") {
                hosts.extend(host_of(&Value::String(url)));
            }
            if let Some(resource) = resource {
                hosts.push(format!("{resource}.services.ai.azure.com"));
            }
        }
        Some(ProviderKind::OpenAi) => {
            // Override when the user set one; otherwise the first-party host
            // (`docs/13` §C open model layer; the default is the adapter's).
            match base_url("inferenceOpenAiBaseUrl") {
                Some(url) => hosts.extend(host_of(&Value::String(url))),
                None => hosts.push("api.openai.com".to_string()),
            }
        }
        Some(ProviderKind::Custom) => {
            if let Some(url) = base_url("inferenceCustomBaseUrl") {
                hosts.extend(host_of(&Value::String(url)));
            }
        }
        None => {}
    }
    hosts
}

/// The host of a string that may be a URL. Returns `None` when the string carries
/// no host.
fn host_of(value: &Value) -> Option<String> {
    let text = value.as_str()?.trim();
    if text.is_empty() {
        return None;
    }
    if let Some(rest) = text.split_once("://") {
        let authority = rest.1.split(['/', '?', '#']).next().unwrap_or("");
        return authority
            .rsplit('@')
            .next()
            .map(|host| host.split(':').next().unwrap_or("").to_ascii_lowercase())
            .filter(|host| !host.is_empty());
    }
    // A bare host (no scheme, no path).
    if text.contains('/') || text.contains(' ') {
        return None;
    }
    let host = text.split(':').next().unwrap_or("").to_ascii_lowercase();
    if host.is_empty() || !host.contains('.') {
        return None;
    }
    Some(host)
}

/// Collect every host reachable from an arbitrary configuration value.
///
/// The array/object keys carry no documented inner grammar (`docs/20` §2.1 records
/// "array/object"), so the walk is shape-agnostic: any string that resolves to a
/// host contributes it. Nothing is guessed about the structure.
fn collect_hosts(value: &Value) -> Vec<String> {
    let mut hosts = Vec::new();
    walk(value, &mut hosts);
    hosts.retain(|host| !host.is_empty());
    hosts
}

fn walk(value: &Value, hosts: &mut Vec<String>) {
    match value {
        Value::String(text) => {
            if let Some(host) = host_of(value) {
                hosts.push(host);
            } else if is_bare_host(text) {
                hosts.push(text.trim().to_ascii_lowercase());
            }
        }
        Value::Array(items) => {
            for item in items {
                walk(item, hosts);
            }
        }
        Value::Object(map) => {
            for item in map.values() {
                walk(item, hosts);
            }
        }
        _ => {}
    }
}

fn is_bare_host(text: &str) -> bool {
    let text = text.trim();
    !text.is_empty()
        && !text.contains(' ')
        && !text.contains('/')
        && !text.contains('@')
        && text.contains('.')
        && !text.starts_with('.')
        && !text.ends_with('.')
}

fn section_value<'a>(
    doc: &'a ProfileDocument,
    section: &str,
) -> Option<&'a serde_json::Map<String, Value>> {
    match section {
        "connection" => doc.connection.0.as_object(),
        "capabilities" => doc.capabilities.0.as_object(),
        "connectors" => doc.connectors.0.as_object(),
        "telemetry" => doc.telemetry.0.as_object(),
        "limits" => doc.limits.0.as_object(),
        "appearance" => doc.appearance.0.as_object(),
        "plugins" => doc.plugins.0.as_object(),
        "egress" => doc.egress.0.as_object(),
        "source" => doc.source.0.as_object(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn profile(provider: ProviderKind, connection: Value) -> ProfileDocument {
        ProfileDocument {
            schema_version: 1,
            id: Some("11111111-1111-4111-8111-111111111111".to_string()),
            name: Some("Profile".to_string()),
            provider: Some(provider),
            connection: crate::Section(connection),
            ..ProfileDocument::default()
        }
    }

    /// docs/09 §2.9: the documented groups and their named hosts.
    #[test]
    fn documented_groups_and_hosts_are_present() {
        let allowlist = derive(&profile(ProviderKind::ClaudeApi, json!({})), None);
        assert!(allowlist
            .hosts()
            .contains(&"downloads.claude.ai".to_string()));
        assert!(allowlist.hosts().contains(&"claude.ai".to_string()));
        assert!(allowlist.hosts().contains(&"api.anthropic.com".to_string()));
        assert!(allowlist
            .hosts()
            .contains(&"claudeusercontent.com".to_string()));
        assert!(allowlist.hosts().contains(&"assets.claude.ai".to_string()));
        assert!(allowlist.group(EgressGroupKind::CoreDownloads).is_some());
        assert!(allowlist.group(EgressGroupKind::AutoUpdates).is_some());
        assert!(allowlist
            .group(EgressGroupKind::EssentialTelemetry)
            .is_some());
        assert!(allowlist
            .group(EgressGroupKind::NonessentialServices)
            .is_some());
    }

    /// The groups whose hostnames were never captured are marked, not invented.
    #[test]
    fn unobserved_host_groups_are_marked_not_invented() {
        let allowlist = derive(&profile(ProviderKind::ClaudeApi, json!({})), None);
        let telemetry = allowlist
            .group(EgressGroupKind::EssentialTelemetry)
            .unwrap();
        assert!(telemetry.hosts_unobserved);
        assert!(telemetry.hosts.is_empty(), "no hostname may be invented");
    }

    /// The update host redirects the auto-update group.
    #[test]
    fn redirected_update_host_replaces_the_default() {
        let mut config = profile(ProviderKind::ClaudeApi, json!({}));
        config.telemetry = crate::Section(json!({
            "updateViaUpdatesHost": "releases.claude.com",
        }));
        let allowlist = derive(&config, None);
        let auto = allowlist.group(EgressGroupKind::AutoUpdates).unwrap();
        assert!(auto.hosts.contains(&"releases.claude.com".to_string()));
        assert!(!auto.hosts.contains(&"api.anthropic.com".to_string()));
    }

    /// docs/12 §1: the Bedrock control-plane hosts are derived from the region.
    #[test]
    fn bedrock_control_plane_hosts_come_from_the_region() {
        let config = profile(
            ProviderKind::Bedrock,
            json!({ "inferenceBedrockRegion": "us-east-1" }),
        );
        let allowlist = derive(&config, None);
        let hosts = allowlist.hosts();
        for expected in [
            "bedrock-runtime.us-east-1.amazonaws.com",
            "bedrock.us-east-1.amazonaws.com",
            "sts.amazonaws.com",
            "sts.us-east-1.amazonaws.com",
            "portal.sso.us-east-1.amazonaws.com",
            "oidc.us-east-1.amazonaws.com",
        ] {
            assert!(hosts.contains(&expected.to_string()), "missing {expected}");
        }
    }

    /// docs/12 §1: the other families' default host shapes.
    #[test]
    fn provider_default_host_shapes() {
        let foundry = derive(
            &profile(
                ProviderKind::Foundry,
                json!({ "inferenceFoundryResource": "myresource" }),
            ),
            None,
        );
        assert!(foundry
            .hosts()
            .contains(&"myresource.services.ai.azure.com".to_string()));

        let mantle = derive(
            &profile(
                ProviderKind::BedrockMantle,
                json!({ "inferenceBedrockMantleRegion": "us-east-1" }),
            ),
            None,
        );
        assert!(mantle
            .hosts()
            .contains(&"bedrock-mantle.us-east-1.api.aws".to_string()));
    }

    /// The configured gateway base URL replaces the default endpoint.
    #[test]
    fn configured_gateway_url_is_derived() {
        let config = profile(
            ProviderKind::Gateway,
            json!({ "inferenceGatewayBaseUrl": "https://gw.internal.example/v1" }),
        );
        let allowlist = derive(&config, None);
        assert!(allowlist
            .hosts()
            .contains(&"gw.internal.example".to_string()));
    }

    /// The managed overlay wins: the derivation describes what the app reaches.
    #[test]
    fn managed_values_win_in_the_derivation() {
        let config = profile(
            ProviderKind::Gateway,
            json!({ "inferenceGatewayBaseUrl": "https://local.example" }),
        );
        let overlay = crate::overlay::Overlay::parse(&json!({
            "connection": { "inferenceGatewayBaseUrl": "https://managed.example" },
        }));
        let merged = crate::overlay::merge(&config, Some(&overlay));
        let allowlist = derive(&config, Some(&merged));
        assert!(allowlist.hosts().contains(&"managed.example".to_string()));
        assert!(!allowlist.hosts().contains(&"local.example".to_string()));
    }

    /// The `*` entry is the special "unrestricted" value (`docs/09` §3.4).
    #[test]
    fn wildcard_allows_any_host() {
        let allowlist = EgressAllowlist {
            groups: vec![EgressGroup {
                group: EgressGroupKind::CoreDownloads,
                hosts: vec!["*".to_string()],
                hosts_unobserved: false,
            }],
        };
        assert!(allowlist.allows("anything.example"));
    }

    /// The `.txt` download form is one host per line.
    #[test]
    fn text_form_is_one_host_per_line() {
        let allowlist = derive(&profile(ProviderKind::ClaudeApi, json!({})), None);
        let text = allowlist.to_text();
        assert!(text.contains('\n'));
        assert!(text.lines().all(|line| !line.contains(' ')));
    }
}
