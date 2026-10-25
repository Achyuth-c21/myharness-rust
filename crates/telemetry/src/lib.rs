//! Telemetry policy and write-time redaction (roadmap Phase 7; `docs/12` §6;
//! `docs/features/updates-telemetry-privacy.md` §1–§3; `docs/09` §1.3, §2.5;
//! `REQ-PRIV-01/02`, `TEST-PRIV-001/002/004`).
//!
//! What this crate is, and deliberately is not:
//!
//! - **No emitter ships.** The rebuild has no crash reporter, no
//!   product-analytics pipeline, and no OpenTelemetry exporter. The shipped
//!   app's Sentry/Datadog intake hosts are recorded baseline evidence, not
//!   something to reproduce — the feature spec's own trust-boundary note
//!   forbids adding hidden telemetry.
//! - **Prompts/completions are never telemetry** (`REQ-PRIV-01`). The only
//!   usage data the rebuild keeps is the device-local ledger
//!   (`crates/store` `usage_events`), whose rows carry token counts and an
//!   error string — never message content. The redaction here is the second
//!   line of defense for the free text a diagnostic bundle may legitimately
//!   include (an error string, an endpoint URL), not a license to capture
//!   content anywhere.
//! - **The controls are independent and enforced** (`REQ-PRIV-02`, `docs/09`
//!   §2.5): each switch is read separately from the merged profile and a
//!   malformed value fails closed. `otlpContentCapture` cannot be enabled at
//!   apply time (`crates/config::tabs` refuses it); the policy still reports
//!   what it reads, so a future emitter cannot mistake a managed overlay for
//!   permission.
//!
//! Nonessential *services* (connector favicons, artifact/MCP-App iframes,
//! the Linux spellcheck dictionary) are recorded as a block list. The
//! rebuild does not yet ship those fetchers, so the list's enforcement point
//! does not exist either; the policy carries the decision so the Privacy
//! pane can state it and any future fetcher must consult it before its
//! first request.

use myharness_config::overlay::MergedConfig;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The nonessential services the capture names (`docs/09` §1.3). A malformed
/// block list fails closed to exactly these.
pub const KNOWN_NONESSENTIAL_SERVICES: [&str; 3] = [
    "connector-favicons",
    "artifact-iframes",
    "spellcheck-dictionary",
];

/// The effective telemetry policy, read fail-closed from the merged
/// profile's `telemetry` section (`docs/09` §2.5). Nothing in this build
/// emits; the policy is what the Privacy pane displays and what the
/// diagnostic bundle consults before inclusion.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryPolicy {
    /// `disableEssentialTelemetry` — crash/performance reports would be blocked.
    pub essential_blocked: bool,
    /// `disableNonessentialTelemetry` — usage analytics and diagnostic
    /// uploads would be blocked.
    pub nonessential_blocked: bool,
    /// The `disableNonessentialServices` block list (service names).
    pub blocked_nonessential_services: Vec<String>,
    /// True when the block list was malformed and failed closed to every
    /// known service (`docs/14` §9 rule 4).
    pub nonessential_services_fail_closed: bool,
    /// `usageMetricsEnabled` — the local ledger itself is device-local data
    /// (`REQ-SET-04`), not telemetry; this records the switch's effective
    /// value for display. Absent keeps the opt-out default (on); malformed
    /// fails closed (off).
    pub usage_metrics_enabled: bool,
    /// `otlpEndpoint` — configured, never dialed by this build.
    pub otlp_endpoint: Option<String>,
    /// `otlpContentCapture` — refused at apply time; surfaced for defense in
    /// depth (`docs/12` §6).
    pub otlp_content_capture: bool,
}

impl Default for TelemetryPolicy {
    fn default() -> Self {
        TelemetryPolicy {
            essential_blocked: false,
            nonessential_blocked: false,
            blocked_nonessential_services: Vec::new(),
            nonessential_services_fail_closed: false,
            usage_metrics_enabled: true,
            otlp_endpoint: None,
            otlp_content_capture: false,
        }
    }
}

/// Read one boolean switch. `absent` is the documented default; a malformed
/// value fails closed — for a block switch that means blocked, for
/// `usageMetricsEnabled` it means off.
fn read_switch(merged: &MergedConfig, key: &str, absent: bool) -> bool {
    match merged.value("telemetry", key) {
        None | Some(Value::Null) => absent,
        Some(Value::Bool(value)) => *value,
        Some(_) => !absent,
    }
}

impl TelemetryPolicy {
    pub fn from_merged(merged: &MergedConfig) -> TelemetryPolicy {
        let raw_services = merged.value("telemetry", "disableNonessentialServices");
        let blocked_nonessential_services = match raw_services {
            None | Some(Value::Null) => Vec::new(),
            Some(Value::Array(items)) => items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect(),
            // Malformed: block every known service rather than none.
            Some(_) => KNOWN_NONESSENTIAL_SERVICES
                .iter()
                .map(|name| (*name).to_string())
                .collect(),
        };
        let nonessential_services_fail_closed = !matches!(
            raw_services,
            None | Some(Value::Null) | Some(Value::Array(_))
        );
        TelemetryPolicy {
            essential_blocked: read_switch(merged, "disableEssentialTelemetry", false),
            nonessential_blocked: read_switch(merged, "disableNonessentialTelemetry", false),
            blocked_nonessential_services,
            nonessential_services_fail_closed,
            usage_metrics_enabled: read_switch(merged, "usageMetricsEnabled", true),
            otlp_endpoint: match merged.value("telemetry", "otlpEndpoint") {
                Some(Value::String(text)) if !text.trim().is_empty() => {
                    Some(text.trim().to_string())
                }
                _ => None,
            },
            otlp_content_capture: matches!(
                merged.value("telemetry", "otlpContentCapture"),
                Some(Value::Bool(true))
            ),
        }
    }

    /// True when a nonessential service may issue its fetch. An absent list
    /// blocks nothing (the documented default); a named service is blocked;
    /// a malformed list blocked everything.
    pub fn nonessential_service_allowed(&self, service: &str) -> bool {
        !self
            .blocked_nonessential_services
            .iter()
            .any(|name| name == service)
    }
}

/// Redact credential-shaped runs in free text destined for a diagnostic
/// bundle (`TEST-PRIV-004`). Recognized shapes: `Bearer`/`Basic` schemes,
/// and token prefixes `sk-ant-`, `sk-`, `ghp_`, `github_pat_`, `xoxb-`,
/// `xoxa-`, `AKIA`, and base64/JWT runs starting `eyJ`. The prefix stays so
/// a reviewer can tell what was removed; the secret does not.
///
/// This is additive redaction, not a content filter: ordinary prose
/// (including prompt-like text) passes through unchanged, because free text
/// is included only where the bundle's manifest says it may be.
pub fn redact_text(input: &str) -> String {
    const PREFIXES: [&str; 8] = [
        "sk-ant-", "github_pat_", "ghp_", "xoxb-", "xoxa-", "AKIA", "sk-", "eyJ",
    ];
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut index = 0;
    while index < input.len() {
        let rest = &input[index..];
        // A `Bearer`/`Basic` scheme: the next whitespace-delimited run is the
        // credential, whatever its shape.
        let mut consumed = 0usize;
        for scheme in ["Bearer ", "bearer ", "Basic ", "basic "] {
            if let Some(after) = rest.strip_prefix(scheme) {
                let run = after.split_whitespace().next().unwrap_or("");
                if run.len() >= 8 && run.chars().all(is_token_char) {
                    out.push_str(scheme.trim_end());
                    out.push_str(" [REDACTED]");
                    consumed = scheme.len() + run.len();
                    break;
                }
            }
        }
        if consumed > 0 {
            index += consumed;
            continue;
        }
        for prefix in PREFIXES {
            if let Some(after) = rest.strip_prefix(prefix) {
                let boundary_ok = index == 0
                    || (!bytes[index - 1].is_ascii_alphanumeric()
                        && bytes[index - 1] != b'_'
                        && bytes[index - 1] != b'-');
                let run: String = after.chars().take_while(|c| is_token_char(*c)).collect();
                let minimum = if prefix == "eyJ" { 20 } else { 8 };
                if boundary_ok && run.len() >= minimum {
                    out.push_str(prefix);
                    out.push_str("[REDACTED]");
                    index += prefix.len() + run.len();
                    consumed = 1;
                    break;
                }
            }
        }
        if consumed > 0 {
            continue;
        }
        let ch = rest.chars().next().expect("index inside the string");
        out.push(ch);
        index += ch.len_utf8();
    }
    out
}

fn is_token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '~')
}

/// Redact a URL for diagnostic inclusion: userinfo, query string, and
/// fragment are dropped; scheme, host, port, and path stay. Anything that
/// does not parse as a URL is redacted whole — for a field that is supposed
/// to be a URL, showing unparseable input is the riskier choice.
pub fn redact_url(input: &str) -> String {
    let trimmed = input.trim();
    let (scheme, rest) = match trimmed.split_once("://") {
        Some((scheme, rest)) if !scheme.is_empty() => (scheme, rest),
        _ => return "[REDACTED]".to_string(),
    };
    let (authority, path) = match rest.find(['/', '?', '#']) {
        Some(cut) => (&rest[..cut], &rest[cut..]),
        None => (rest, ""),
    };
    let host_port = match authority.rsplit_once('@') {
        Some((_userinfo, host)) => host,
        None => authority,
    };
    if host_port.is_empty() {
        return "[REDACTED]".to_string();
    }
    let path = match path.find(['?', '#']) {
        Some(cut) => &path[..cut],
        None => path,
    };
    format!("{scheme}://{host_port}{path}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use myharness_config::{ProfileDocument, Section};
    use serde_json::json;

    fn merged(telemetry: Value) -> MergedConfig {
        let doc = ProfileDocument {
            telemetry: Section(telemetry),
            ..ProfileDocument::default()
        };
        myharness_config::overlay::merge(&doc, None)
    }

    // ---------------------------------------------------------------
    // Policy independence and fail-closed reading (TEST-PRIV-002)
    // ---------------------------------------------------------------

    #[test]
    fn the_block_switches_are_independent() {
        let policy = TelemetryPolicy::from_merged(&merged(json!({
            "disableEssentialTelemetry": true,
        })));
        assert!(policy.essential_blocked);
        assert!(!policy.nonessential_blocked);

        let policy = TelemetryPolicy::from_merged(&merged(json!({
            "disableNonessentialTelemetry": true,
        })));
        assert!(!policy.essential_blocked);
        assert!(policy.nonessential_blocked);
    }

    #[test]
    fn absent_switches_keep_the_documented_defaults() {
        let policy = TelemetryPolicy::from_merged(&merged(json!({})));
        assert!(!policy.essential_blocked);
        assert!(!policy.nonessential_blocked);
        assert!(policy.usage_metrics_enabled);
        assert!(policy.otlp_endpoint.is_none());
        assert!(!policy.otlp_content_capture);
        assert!(policy.blocked_nonessential_services.is_empty());
        assert!(!policy.nonessential_services_fail_closed);
    }

    #[test]
    fn a_malformed_switch_fails_closed() {
        let policy = TelemetryPolicy::from_merged(&merged(json!({
            "disableEssentialTelemetry": "sometimes",
        })));
        assert!(policy.essential_blocked);

        let policy = TelemetryPolicy::from_merged(&merged(json!({
            "usageMetricsEnabled": 7,
        })));
        assert!(!policy.usage_metrics_enabled);
    }

    #[test]
    fn the_service_block_list_reads_arrays_and_fails_closed_on_malformed() {
        let policy = TelemetryPolicy::from_merged(&merged(json!({
            "disableNonessentialServices": ["connector-favicons"],
        })));
        assert_eq!(
            policy.blocked_nonessential_services,
            vec!["connector-favicons"]
        );
        assert!(!policy.nonessential_services_fail_closed);
        assert!(policy.nonessential_service_allowed("spellcheck-dictionary"));
        assert!(!policy.nonessential_service_allowed("connector-favicons"));

        let policy = TelemetryPolicy::from_merged(&merged(json!({
            "disableNonessentialServices": "favicons",
        })));
        assert_eq!(
            policy.blocked_nonessential_services,
            KNOWN_NONESSENTIAL_SERVICES
        );
        assert!(policy.nonessential_services_fail_closed);
        assert!(!policy.nonessential_service_allowed("spellcheck-dictionary"));
    }

    #[test]
    fn the_otlp_endpoint_is_recorded_never_dialed() {
        let policy = TelemetryPolicy::from_merged(&merged(json!({
            "otlpEndpoint": "https://collector.example.com/v1/logs",
            "otlpContentCapture": true,
        })));
        assert_eq!(
            policy.otlp_endpoint.as_deref(),
            Some("https://collector.example.com/v1/logs")
        );
        // The apply-time gate refuses this value; if a managed overlay
        // carried it anyway, the policy surfaces it so no emitter can
        // mistake it for permission (docs/12 §6).
        assert!(policy.otlp_content_capture);
    }

    // ---------------------------------------------------------------
    // Redaction (TEST-PRIV-004)
    // ---------------------------------------------------------------

    #[test]
    fn a_bearer_credential_is_redacted_but_the_scheme_stays() {
        assert_eq!(
            redact_text("login failed: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9 denied"),
            "login failed: Bearer [REDACTED] denied"
        );
    }

    #[test]
    fn known_token_prefixes_are_redacted_from_anywhere_in_the_text() {
        assert_eq!(
            redact_text("probe used sk-ant-api03-AAAA0111222233334444 and ghp_0123456789abcdef"),
            "probe used sk-ant-[REDACTED] and ghp_[REDACTED]"
        );
        assert_eq!(
            redact_text("AKIAIOSFODNN7EXAMPLE refused"),
            "AKIA[REDACTED] refused"
        );
    }

    #[test]
    fn ordinary_words_that_merely_contain_a_prefix_survive() {
        assert_eq!(redact_text("the task-12345 detail"), "the task-12345 detail");
        assert_eq!(redact_text("risk-assessment done"), "risk-assessment done");
    }

    #[test]
    fn prompt_like_text_passes_through_unchanged() {
        let text = "Summarize the attached document in three bullets.";
        assert_eq!(redact_text(text), text);
    }

    #[test]
    fn a_url_loses_userinfo_query_and_fragment_but_keeps_the_path() {
        assert_eq!(
            redact_url("https://user:secret@collector.example.com/v1/logs?token=abc&x=1#frag"),
            "https://collector.example.com/v1/logs"
        );
        assert_eq!(
            redact_url("https://collector.example.com/v1/metrics"),
            "https://collector.example.com/v1/metrics"
        );
    }

    #[test]
    fn a_non_url_is_redacted_whole() {
        assert_eq!(redact_url("not a url"), "[REDACTED]");
        assert_eq!(redact_url(""), "[REDACTED]");
    }
}
