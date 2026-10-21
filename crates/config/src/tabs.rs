//! Setup-window tab schemas and validation — roadmap Phase 2 (docs/13 §4 Phase 2;
//! docs/09 §2.1–§2.10, §3.4; docs/20 §2.1 key index; `docs/features/third-party-setup.md`).
//!
//! The setup window is a **separate window** (`docs/09` §2.1), not a Settings
//! tab and not a modal, with a left navigation of nine entries and a fixed
//! **Apply Changes** footer (`docs/screens/setup-desktop-3p.md`). This module owns
//! the **form shape of every tab except Connection**, which lives in
//! [`crate::forms`] because it varies per provider family.
//!
//! What each tab carries (docs/09 §2.3–§2.10) is recorded here as a schema, and
//! the semantic rules of `docs/09` §3.4 as validation:
//!
//! - "`0` has explicit meaning for approval lifetime and relaunch window; blank
//!   means default" — zero is a valid, meaningful value, not "unset".
//! - "Egress host entries must be parseable hosts; `*` is special-cased".
//! - "Retention fields accept days; legal hold overrides them".
//! - "Model discovery off + model list empty is invalid for providers that
//!   require a model" (enforced in [`crate::forms`]).
//!
//! Two policies are enforced here rather than merely displayed:
//!
//! - **`claudeAiImport` is policy-disabled in third-party mode** (`docs/09` §1.7
//!   records the shipped 3P value `{"enabled": false, "automatic3pImport": false,
//!   "exportEnabled": false}`). A profile that enables it is refused
//!   ([`RULES.md` §10](../../RULES.md): do not implement a policy-disabled
//!   feature).
//! - **Attachment content is never uploaded for telemetry** ([`RULES.md` §2](../../RULES.md),
//!   `docs/12` §6): `otlpContentCapture` may not be enabled.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::forms::{FieldDef, FieldGroup, FieldKind, FormIssue, Severity};

/// The nine setup-window tabs (`docs/09` §2.2–§2.10; the left navigation of
/// `docs/screens/setup-desktop-3p.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SetupTab {
    Connection,
    Capabilities,
    Connectors,
    Telemetry,
    Limits,
    Appearance,
    Plugins,
    Egress,
    Source,
}

impl SetupTab {
    /// The tab's display title.
    pub fn title(self) -> &'static str {
        match self {
            SetupTab::Connection => "Connection",
            SetupTab::Capabilities => "Capabilities",
            SetupTab::Connectors => "Connectors",
            SetupTab::Telemetry => "Telemetry & updates",
            SetupTab::Limits => "Limits",
            SetupTab::Appearance => "Appearance",
            SetupTab::Plugins => "Plugins",
            SetupTab::Egress => "Egress",
            SetupTab::Source => "Source",
        }
    }

    /// The profile section this tab edits.
    pub fn section(self) -> &'static str {
        match self {
            SetupTab::Connection => "connection",
            SetupTab::Capabilities => "capabilities",
            SetupTab::Connectors => "connectors",
            SetupTab::Telemetry => "telemetry",
            SetupTab::Limits => "limits",
            SetupTab::Appearance => "appearance",
            SetupTab::Plugins => "plugins",
            SetupTab::Egress => "egress",
            SetupTab::Source => "source",
        }
    }

    /// Every tab, in navigation order.
    pub fn all() -> [SetupTab; 9] {
        [
            SetupTab::Connection,
            SetupTab::Capabilities,
            SetupTab::Connectors,
            SetupTab::Telemetry,
            SetupTab::Limits,
            SetupTab::Appearance,
            SetupTab::Plugins,
            SetupTab::Egress,
            SetupTab::Source,
        ]
    }
}

/// The schema of one setup tab.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TabSchema {
    pub tab: SetupTab,
    pub title: &'static str,
    pub fields: Vec<FieldDef>,
    /// True when the tab cannot be edited — the Egress tab is a derived,
    /// read-only allowlist (`docs/09` §2.9).
    #[serde(default)]
    pub read_only: bool,
}

fn field(
    key: &'static str,
    label: &'static str,
    kind: FieldKind,
) -> FieldDef {
    FieldDef {
        key,
        label,
        kind,
        group: FieldGroup::Advanced,
        required: false,
        required_when: &[],
        options: vec![],
        help: None,
    }
}

fn text(key: &'static str, label: &'static str) -> FieldDef {
    field(key, label, FieldKind::Text)
}

fn url(key: &'static str, label: &'static str) -> FieldDef {
    field(key, label, FieldKind::Url)
}

fn number(key: &'static str, label: &'static str) -> FieldDef {
    field(key, label, FieldKind::Number)
}

fn switch(key: &'static str, label: &'static str) -> FieldDef {
    field(key, label, FieldKind::Switch)
}

fn tags(key: &'static str, label: &'static str) -> FieldDef {
    field(key, label, FieldKind::TagList)
}

fn map(key: &'static str, label: &'static str) -> FieldDef {
    field(key, label, FieldKind::KeyValue)
}

/// The schema for a tab. The Connection tab is per provider family and lives in
/// [`crate::forms::connection_schema`]; the Egress tab has no fields because it
/// is derived.
pub fn tab_schema(tab: SetupTab) -> TabSchema {
    let fields: Vec<FieldDef> = match tab {
        SetupTab::Connection => Vec::new(),
        SetupTab::Capabilities => vec![
            switch("chatTabEnabled", "Chat"),
            switch("coworkTabEnabled", "Cowork"),
            switch("isClaudeCodeForDesktopEnabled", "Code"),
            switch("scheduledTasksEnabled", "Scheduled tasks"),
            switch("quickWriteEnabled", "Quick write"),
            switch("developerModeEnabled", "Developer mode"),
            tags("coworkEgressAllowedHosts", "Allowed egress hosts"),
            tags("allowedWorkspaceFolders", "Allowed workspace folders"),
            switch(
                "blockReadsOutsideWorkingDirectories",
                "Block reads outside working directories",
            ),
            switch("autoModeEnabled", "Allow Auto mode"),
            switch("disableBypassPermissionsMode", "Bypass permissions"),
            text("builtinToolPolicy", "Built-in tool policy"),
            tags("disabledBuiltinTools", "Disabled built-in tools"),
            switch("toolSearchEnabled", "Tool search"),
            switch("skipWebFetchPreflight", "Skip web-fetch preflight"),
            switch("coworkFileReadCheck", "Cowork file read check"),
            url("toolPolicyEndpoint", "Tool policy endpoint"),
            switch("builtinBrowserEnabled", "Built-in browser"),
            text(
                "builtinBrowserDefaultDomainPolicy",
                "Built-in browser default domain policy",
            ),
            tags("builtinBrowserAllowedDomains", "Built-in browser allowed domains"),
            tags("builtinBrowserBlockedDomains", "Built-in browser blocked domains"),
            switch("claudeInChromeEnabled", "Claude in Chrome"),
            switch(
                "claudeInChromePasswordManagersEnabled",
                "Claude in Chrome password managers",
            ),
            switch("dictationEnabled", "Dictation"),
            switch("disableFeatureDiscovery", "Feature announcements"),
            text("organizationInstructions", "Organization instructions"),
            switch("disableDesktopLocalSessions", "Desktop local sessions"),
            switch("disableWslSessions", "WSL sessions"),
            map("sshConfigs", "SSH connections"),
            tags("sshHostAllowlist", "SSH host allowlist"),
            text("sshTransport", "SSH transport"),
            text("sshClientPath", "SSH client path"),
            switch("relocateUncUserData", "Relocate UNC user data"),
            switch("requireCoworkFullVmSandbox", "Require the full Cowork VM sandbox"),
            switch("secureVmFeaturesEnabled", "Secure VM features"),
            text("coworkEDRIntegration", "Cowork EDR integration"),
            text("coworkEDRIntegrationVendor", "Cowork EDR vendor"),
            switch("coworkVmIpv6Enabled", "Workspace VM IPv6"),
            switch("disableBundledSkills", "Bundled skills"),
            switch("skillCreationEnabled", "Skill creation"),
            switch("disableDeepLinkRegistration", "Deep-link registration"),
            switch(
                "claudeCodeProcessWrapperEnabled",
                "Claude Code process wrapper",
            ),
            map("claudeCodeSettings", "Claude Code settings"),
            tags("codeAllowedRepositories", "Allowed repositories"),
            map("officeAccessPolicies", "Office access policies"),
            tags("officeDisabledFeatures", "Disabled Office features"),
            tags("officeDisabledHosts", "Disabled Office hosts"),
            switch("hardwareBuddyEnabled", "Hardware buddy"),
            switch("keepAwakeEnabled", "Keep awake"),
            switch(
                "chatAdvancedFileAnalysisEnabled",
                "Advanced file analysis",
            ),
            text("desktopHome", "Desktop home"),
        ],
        SetupTab::Connectors => vec![
            map("managedMcpServers", "Managed MCP servers"),
            switch(
                "mcpPersistentAlwaysAllowEnabled",
                "Allow persistent tool approvals",
            ),
            number(
                "mcpScheduledTaskApprovalLifetimeDays",
                "Scheduled-task tool approval lifetime (days)",
            ),
            switch("isLocalDevMcpEnabled", "Allow user-added MCP servers"),
            tags("allowedPluginMcpServers", "Allowed plugin MCP servers"),
            tags("deniedPluginMcpServers", "Denied plugin MCP servers"),
            number("mcpToolTimeoutSec", "MCP tool timeout (seconds)"),
            switch("isDesktopExtensionEnabled", "Desktop extensions"),
            switch(
                "isDesktopExtensionDirectoryEnabled",
                "Desktop extension directory",
            ),
            switch(
                "isDesktopExtensionSignatureRequired",
                "Require extension signatures",
            ),
            text(
                "importThirdPartyHistoryIntoOrgUUID",
                "Import third-party history into org UUID",
            ),
            map("claudeAiImport", "Claude.ai import"),
            switch("disableConnectorSignInInApp", "Connector sign-in in app"),
            switch("claudeInChromeEnabled", "Claude in Chrome"),
            switch(
                "claudeInChromePasswordManagersEnabled",
                "Claude in Chrome password managers",
            ),
            switch("microsoftAuthBroker", "Microsoft auth broker"),
            text("microsoftAuthDefaultAccount", "Microsoft default account"),
            switch("hardwareBuddyEnabled", "Hardware buddy"),
        ],
        SetupTab::Telemetry => vec![
            text(
                "deploymentOrganizationUuid",
                "Organization UUID",
            ),
            switch("disableEssentialTelemetry", "Block essential telemetry"),
            switch(
                "disableNonessentialTelemetry",
                "Block nonessential telemetry",
            ),
            switch("usageMetricsEnabled", "Usage metrics"),
            tags("disableNonessentialServices", "Blocked nonessential services"),
            url("otlpEndpoint", "OpenTelemetry endpoint"),
            text("otlpProtocol", "OpenTelemetry protocol"),
            map("otlpHeaders", "OpenTelemetry headers"),
            map("otlpHeadersHelper", "OpenTelemetry headers helper"),
            text("otlpAuthMode", "OpenTelemetry auth mode"),
            map("otlpResourceAttributes", "OpenTelemetry resource attributes"),
            switch("otlpContentCapture", "OpenTelemetry content capture"),
            text("otlpDesktopLogLevel", "OpenTelemetry desktop log level"),
            switch("otlpTracesEnabled", "OpenTelemetry traces"),
            number("otlpAttrMaxChars", "OpenTelemetry attribute max characters"),
            switch("disableAutoUpdates", "Block auto-updates"),
            number(
                "autoUpdaterEnforcementHours",
                "Auto-update enforcement window (hours)",
            ),
            text("releaseChannel", "Release channel"),
            text("updateViaUpdatesHost", "Update host"),
            text("dangerousMaxVersion", "Maximum version"),
            number("relaunchEnforcementHours", "Relaunch enforcement window (hours)"),
            number(
                "configRecheckIntervalMinutes",
                "Configuration re-check interval (minutes)",
            ),
            switch("disableLocalConfigCache", "Disable the local config cache"),
        ],
        SetupTab::Limits => vec![
            number("chatSessionRetentionDays", "Chat retention (days)"),
            number("coworkSessionRetentionDays", "Cowork retention (days)"),
            number("codeSessionRetentionDays", "Code retention (days)"),
            switch("sessionRetentionHold", "Suspend session deletion (legal hold)"),
            number("inferenceMaxTokensPerWindow", "Max tokens per window"),
            number("inferenceTokenWindowHours", "Token window (hours)"),
        ],
        SetupTab::Appearance => vec![
            switch("disableFeatureDiscovery", "Hide feature announcements"),
            text("endUserAttribution", "End-user attribution"),
            text("deploymentDisplayName", "Display name"),
            text("deploymentDisplaySubtitle", "Display subtitle"),
            map("banner", "Banner"),
            switch(
                "disableConfigDeprecationWarnings",
                "Hide configuration deprecation warnings",
            ),
        ],
        SetupTab::Plugins => vec![
            tags("allowedPluginMarketplaces", "Plugin marketplaces"),
            tags("organizationPluginsUrl", "Organization plugins endpoint"),
            map("orgPluginSettings", "Per-server tool policies"),
            switch(
                "userPluginMarketplacesEnabled",
                "User plugin marketplaces",
            ),
            switch("userPluginUploadsEnabled", "User plugin uploads"),
            map("skillBundles", "Skill bundles"),
            url("sharingEndpoint", "Sharing endpoint"),
        ],
        // The Egress tab is a derived, read-only allowlist (`docs/09` §2.9); it has
        // no editable fields.
        SetupTab::Egress => Vec::new(),
        SetupTab::Source => vec![
            url("bootstrapUrl", "Bootstrap config URL"),
            switch("bootstrapEnabled", "Use bootstrap config"),
            map("bootstrapOidc", "Bootstrap OIDC parameters"),
            map("bootstrapHeaders", "Bootstrap request headers"),
            map("bootstrapHeadersHelper", "Bootstrap headers helper"),
            switch("trustBootstrapDelivery", "Trust bootstrap-delivered settings"),
        ],
    };

    TabSchema {
        tab,
        title: tab.title(),
        fields,
        read_only: tab == SetupTab::Egress,
    }
}

/// Validate a tab's section.
///
/// Only rules the docs state are enforced. Where a value's meaning was never
/// captured — `endUserAttribution`'s enum values, for instance — the value is
/// accepted and the gap is recorded in `docs/14` §9 rather than guessed at.
pub fn validate_tab(tab: SetupTab, section: &Value) -> Vec<FormIssue> {
    let mut issues: Vec<FormIssue> = Vec::new();
    let object = match section.as_object() {
        Some(object) => object,
        None => return issues,
    };
    let get = |key: &str| object.get(key);

    match tab {
        SetupTab::Connection | SetupTab::Egress => {}
        SetupTab::Capabilities => {
            // "Egress host entries must be parseable hosts; `*` is special-cased".
            for key in [
                "coworkEgressAllowedHosts",
                "builtinBrowserAllowedDomains",
                "builtinBrowserBlockedDomains",
                "sshHostAllowlist",
            ] {
                if let Some(entries) = get(key).and_then(Value::as_array) {
                    for (index, entry) in entries.iter().enumerate() {
                        let Some(host) = entry.as_str() else {
                            issues.push(warning(
                                key,
                                format!("entry {index} is not a string; its shape is unobserved"),
                            ));
                            continue;
                        };
                        let host = host.trim();
                        if host == "*" || host.is_empty() {
                            continue;
                        }
                        if !is_parseable_host(host) {
                            issues.push(error(
                                key,
                                format!("{host:?} is not a parseable host"),
                            ));
                        }
                    }
                }
            }
            if let Some(endpoint) = get("toolPolicyEndpoint") {
                if let Some(text) = endpoint.as_str() {
                    if !text.trim().is_empty() && !text.trim().starts_with("http") {
                        issues.push(error(
                            "toolPolicyEndpoint",
                            "the tool policy endpoint must be an http(s) URL".to_string(),
                        ));
                    }
                }
            }
        }
        SetupTab::Connectors => {
            // "`0` removes the lasting option" — zero is meaningful, not unset.
            if let Some(days) = get("mcpScheduledTaskApprovalLifetimeDays").and_then(Value::as_i64)
            {
                if days < 0 {
                    issues.push(error(
                        "mcpScheduledTaskApprovalLifetimeDays",
                        "the approval lifetime in days cannot be negative".to_string(),
                    ));
                }
            }
            if let Some(timeout) = get("mcpToolTimeoutSec").and_then(Value::as_i64) {
                if timeout <= 0 {
                    issues.push(error(
                        "mcpToolTimeoutSec",
                        "the MCP tool timeout must be a positive number of seconds".to_string(),
                    ));
                }
            }
            // `claudeAiImport` is policy-disabled in third-party mode
            // (`docs/09` §1.7; RULES §10).
            if let Some(import) = get("claudeAiImport") {
                if import.get("enabled").and_then(Value::as_bool) == Some(true) {
                    issues.push(error(
                        "claudeAiImport",
                        "claude.ai import is policy-disabled in third-party mode".to_string(),
                    ));
                }
            }
        }
        SetupTab::Telemetry => {
            // Attachment content is never uploaded for telemetry (RULES §2).
            if get("otlpContentCapture").and_then(Value::as_bool) == Some(true) {
                issues.push(error(
                    "otlpContentCapture",
                    "telemetry content capture may not be enabled".to_string(),
                ));
            }
            if let Some(endpoint) = get("otlpEndpoint").and_then(Value::as_str) {
                if !endpoint.trim().is_empty() && !endpoint.trim().starts_with("http") {
                    issues.push(error(
                        "otlpEndpoint",
                        "the OpenTelemetry endpoint must be an http(s) URL".to_string(),
                    ));
                }
            }
            if let Some(minutes) = get("configRecheckIntervalMinutes").and_then(Value::as_i64) {
                if minutes <= 0 {
                    issues.push(error(
                        "configRecheckIntervalMinutes",
                        "the configuration re-check interval must be a positive number of minutes"
                            .to_string(),
                    ));
                }
            }
            if let Some(hours) = get("relaunchEnforcementHours").and_then(Value::as_i64) {
                // "0 = immediate" is an explicit meaning, so only negatives fail.
                if hours < 0 {
                    issues.push(error(
                        "relaunchEnforcementHours",
                        "the relaunch enforcement window cannot be negative".to_string(),
                    ));
                }
            }
            if let Some(hours) = get("autoUpdaterEnforcementHours").and_then(Value::as_i64) {
                if hours < 0 {
                    issues.push(error(
                        "autoUpdaterEnforcementHours",
                        "the auto-update enforcement window cannot be negative".to_string(),
                    ));
                }
            }
        }
        SetupTab::Limits => {
            for key in [
                "chatSessionRetentionDays",
                "coworkSessionRetentionDays",
                "codeSessionRetentionDays",
                "inferenceMaxTokensPerWindow",
                "inferenceTokenWindowHours",
            ] {
                if let Some(value) = get(key).and_then(Value::as_i64) {
                    if value < 0 {
                        issues.push(error(
                            key,
                            "the value cannot be negative".to_string(),
                        ));
                    }
                }
            }
        }
        SetupTab::Appearance => {}
        SetupTab::Plugins => {
            if let Some(endpoint) = get("sharingEndpoint").and_then(Value::as_str) {
                if !endpoint.trim().is_empty() && !endpoint.trim().starts_with("http") {
                    issues.push(error(
                        "sharingEndpoint",
                        "the sharing endpoint must be an http(s) URL".to_string(),
                    ));
                }
            }
        }
        SetupTab::Source => {
            // The bootstrap URL is an HTTPS endpoint (`docs/09` §2.10).
            if let Some(url) = get("bootstrapUrl").and_then(Value::as_str) {
                let url = url.trim();
                if !url.is_empty() && !url.starts_with("https://") {
                    issues.push(error(
                        "bootstrapUrl",
                        "the bootstrap config URL must be https".to_string(),
                    ));
                }
            }
            if get("trustBootstrapDelivery").and_then(Value::as_bool) == Some(true) {
                issues.push(warning(
                    "trustBootstrapDelivery",
                    "trusting bootstrap-delivered settings makes this profile organization-managed"
                        .to_string(),
                ));
            }
        }
    }

    issues.sort_by(|a, b| (&a.field, &a.message).cmp(&(&b.field, &b.message)));
    issues
}

/// True when a string is a parseable host: a hostname with at least one label, or
/// an IP address. `*` is special-cased by the caller (`docs/09` §3.4).
fn is_parseable_host(host: &str) -> bool {
    let host = host.trim();
    if host.is_empty() || host.contains(' ') || host.contains('/') {
        return false;
    }
    if host.parse::<std::net::IpAddr>().is_ok() {
        return true;
    }
    let host = host.split(':').next().unwrap_or(host);
    !host.is_empty()
        && !host.starts_with('.')
        && !host.ends_with('.')
        && host
            .split('.')
            .all(|label| !label.is_empty() && !label.starts_with('-') && !label.ends_with('-'))
}

fn error(field: &str, message: String) -> FormIssue {
    FormIssue {
        field: field.to_string(),
        severity: Severity::Error,
        message,
    }
}

fn warning(field: &str, message: String) -> FormIssue {
    FormIssue {
        field: field.to_string(),
        severity: Severity::Warning,
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn errors(issues: &[FormIssue]) -> Vec<&str> {
        issues
            .iter()
            .filter(|i| i.severity == Severity::Error)
            .map(|i| i.field.as_str())
            .collect()
    }

    /// Every tab has a schema and a distinct section; the Egress tab is read-only.
    #[test]
    fn every_tab_has_a_schema() {
        let mut sections = Vec::new();
        for tab in SetupTab::all() {
            let schema = tab_schema(tab);
            assert_eq!(schema.title, tab.title());
            sections.push(schema.tab.section());
            if tab == SetupTab::Egress {
                assert!(schema.read_only);
                assert!(schema.fields.is_empty());
            } else {
                assert!(!schema.read_only);
            }
        }
        let unique: std::collections::BTreeSet<_> = sections.into_iter().collect();
        assert_eq!(unique.len(), 9, "each tab owns a distinct section");
    }

    /// docs/09 §3.4: egress host entries must be parseable hosts, and `*` is
    /// special-cased.
    #[test]
    fn egress_host_entries_must_be_parseable() {
        let issues = validate_tab(
            SetupTab::Capabilities,
            &json!({ "coworkEgressAllowedHosts": ["*", "example.com", "not a host"] }),
        );
        assert_eq!(errors(&issues), vec!["coworkEgressAllowedHosts"]);
    }

    /// docs/09 §3.4: "`0` has explicit meaning for approval lifetime and relaunch
    /// window; blank means default" — zero is valid.
    #[test]
    fn zero_is_a_meaningful_value_not_an_error() {
        let issues = validate_tab(
            SetupTab::Connectors,
            &json!({ "mcpScheduledTaskApprovalLifetimeDays": 0 }),
        );
        assert!(errors(&issues).is_empty(), "{issues:?}");

        let issues = validate_tab(
            SetupTab::Telemetry,
            &json!({ "relaunchEnforcementHours": 0, "autoUpdaterEnforcementHours": 0 }),
        );
        assert!(errors(&issues).is_empty(), "{issues:?}");
    }

    /// RULES §10: a policy-disabled feature is refused, not implemented.
    #[test]
    fn claude_ai_import_is_policy_disabled() {
        let issues = validate_tab(
            SetupTab::Connectors,
            &json!({ "claudeAiImport": { "enabled": true } }),
        );
        assert_eq!(errors(&issues), vec!["claudeAiImport"]);

        let issues = validate_tab(
            SetupTab::Connectors,
            &json!({
                "claudeAiImport": { "enabled": false, "automatic3pImport": false, "exportEnabled": false }
            }),
        );
        assert!(errors(&issues).is_empty(), "{issues:?}");
    }

    /// RULES §2: attachment content is never uploaded for telemetry.
    #[test]
    fn telemetry_content_capture_is_refused() {
        let issues = validate_tab(
            SetupTab::Telemetry,
            &json!({ "otlpContentCapture": true }),
        );
        assert_eq!(errors(&issues), vec!["otlpContentCapture"]);
    }

    /// docs/09 §2.10: the bootstrap config URL is an HTTPS endpoint, and trusting
    /// bootstrap delivery is a warned-about opt-in.
    #[test]
    fn source_tab_rules() {
        let issues = validate_tab(
            SetupTab::Source,
            &json!({ "bootstrapUrl": "http://bootstrap.example/config" }),
        );
        assert_eq!(errors(&issues), vec!["bootstrapUrl"]);

        let issues = validate_tab(
            SetupTab::Source,
            &json!({ "bootstrapUrl": "https://bootstrap.example/config" }),
        );
        assert!(errors(&issues).is_empty(), "{issues:?}");

        let issues = validate_tab(
            SetupTab::Source,
            &json!({ "trustBootstrapDelivery": true }),
        );
        assert!(errors(&issues).is_empty());
        assert!(issues
            .iter()
            .any(|i| i.field == "trustBootstrapDelivery" && i.severity == Severity::Warning));
    }

    /// Retention fields accept days; a negative value is refused.
    #[test]
    fn retention_days_are_validated() {
        let issues = validate_tab(
            SetupTab::Limits,
            &json!({ "chatSessionRetentionDays": -1, "coworkSessionRetentionDays": 30 }),
        );
        assert_eq!(errors(&issues), vec!["chatSessionRetentionDays"]);
    }
}
