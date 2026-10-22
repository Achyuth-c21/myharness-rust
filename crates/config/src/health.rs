//! Managed-configuration health and re-check — roadmap Phase 2 (docs/13 §4 Phase 2;
//! docs/09 §3.2 profile lifecycle, §2.5 Telemetry & updates, §2.10 Source tab;
//! `DATA-CONFIG-09`).
//!
//! The shipped app re-checks its managed (bootstrap) configuration on a configured
//! interval and surfaces the result: a failed check can mark a landed remote
//! configuration **tainted**, and `recheckConfigHealth` repairs or retries it
//! (`docs/09` §3.2; `DATA-CONFIG-09` "Re-check managed configuration on the
//! configured interval and surface health").
//!
//! The fetch itself is a network call and belongs to a later slice (it also needs
//! the trust policy for bootstrap delivery, which was never captured — `docs/14`
//! §9). This module owns the **state machine** and the **settings** it reads, so
//! the transport can be swapped without changing the semantics:
//!
//! - [`ManagedSettings::from_sections`] reads the Source-tab and Telemetry-tab keys
//!   that drive the behavior.
//! - [`ManagedConfig::apply_result`] folds one fetch outcome into the state
//!   (healthy / tainted, overlay replaced or retained, local-fallback decision).
//! - [`ManagedConfig::is_recheck_due`] answers whether the interval has elapsed.
//!
//! Precedence note: a tainted managed configuration does **not** silently drop the
//! managed restrictions. The overlay is retained (it is the last known managed
//! state) and the failure is surfaced; whether the *local* copy may stand in for
//! the unreachable managed source is decided by `disableLocalConfigCache`
//! (`docs/09` §3.2).

use serde::{Deserialize, Serialize};

use crate::overlay::Overlay;
use crate::Section;

/// Default configuration re-check interval in minutes (`docs/09` §2.5: "default 10").
pub const DEFAULT_RECHECK_INTERVAL_MINUTES: u32 = 10;

/// Health of the managed configuration source.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HealthStatus {
    /// The managed source is reachable (or no managed source is configured).
    #[default]
    Healthy,
    /// The managed source could not be reached. A landed remote configuration is
    /// now suspect (`docs/09` §3.2).
    Tainted,
}

/// The health surface the setup window shows (`DATA-CONFIG-09`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigHealth {
    pub status: HealthStatus,
    /// Whether the app may keep using the last known local configuration while the
    /// managed source is unreachable. `false` when `disableLocalConfigCache` is
    /// set: a stale local copy must not be used when managed config is unreachable
    /// (`docs/09` §3.2).
    pub local_fallback_allowed: bool,
    /// Whether a managed overlay has ever landed.
    pub overlay_present: bool,
    /// Epoch seconds of the last check, when one has run.
    pub last_checked: Option<u64>,
    /// A user-safe detail line. Never a raw body, token, or URL with a query
    /// string (`docs/18` §11).
    pub detail: Option<String>,
}

/// The settings that drive managed-configuration behavior, read from the active
/// profile's Source and Telemetry sections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedSettings {
    /// `bootstrapEnabled` — use bootstrap config.
    pub enabled: bool,
    /// `bootstrapUrl` — the HTTPS endpoint returning the per-user overlay.
    pub url: Option<String>,
    /// `trustBootstrapDelivery` — trust bootstrap-delivered settings. A
    /// security-sensitive opt-in (`docs/20`).
    pub trust_delivery: bool,
    /// `configRecheckIntervalMinutes` — default 10 (`docs/09` §2.5).
    pub recheck_interval_minutes: u32,
    /// `disableLocalConfigCache` — keep only the organization identity and
    /// restrictions on disk, and do not use a stale local copy when the managed
    /// source is unreachable (`docs/09` §3.2; `docs/20`).
    pub disable_local_cache: bool,
}

impl Default for ManagedSettings {
    fn default() -> Self {
        ManagedSettings {
            enabled: false,
            url: None,
            trust_delivery: false,
            recheck_interval_minutes: DEFAULT_RECHECK_INTERVAL_MINUTES,
            disable_local_cache: false,
        }
    }
}

impl ManagedSettings {
    /// Read the managed settings from the profile's `source` and `telemetry`
    /// sections. A missing or malformed key falls back to the documented default
    /// rather than failing the whole read — a broken key must not make the
    /// configuration unreadable (`docs/14` §9 discipline: unknown capability is
    /// disabled, not assumed present).
    pub fn from_sections(source: &Section, telemetry: &Section) -> ManagedSettings {
        let source_obj = source.0.as_object();
        let telemetry_obj = telemetry.0.as_object();
        fn read<'a>(
            obj: Option<&'a serde_json::Map<String, serde_json::Value>>,
            key: &str,
        ) -> Option<&'a serde_json::Value> {
            obj.and_then(|o| o.get(key))
        }

        let url = read(source_obj, "bootstrapUrl")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);

        ManagedSettings {
            enabled: read(source_obj, "bootstrapEnabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            url,
            trust_delivery: read(source_obj, "trustBootstrapDelivery")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            recheck_interval_minutes: read(telemetry_obj, "configRecheckIntervalMinutes")
                .and_then(|v| v.as_u64())
                .filter(|minutes| *minutes > 0)
                .map(|minutes| minutes as u32)
                .unwrap_or(DEFAULT_RECHECK_INTERVAL_MINUTES),
            disable_local_cache: read(telemetry_obj, "disableLocalConfigCache")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        }
    }

    /// Whether a managed source is configured at all. A managed source needs a
    /// URL; `bootstrapEnabled` alone is not enough to reach anything.
    pub fn has_source(&self) -> bool {
        self.enabled && self.url.is_some()
    }
}

/// The managed-configuration state: the landed overlay, its health, and the
/// re-check schedule.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ManagedConfig {
    overlay: Overlay,
    overlay_present: bool,
    status: HealthStatus,
    last_checked: Option<u64>,
    settings: ManagedSettings,
}

impl ManagedConfig {
    /// A state with no managed source configured.
    pub fn new(settings: ManagedSettings) -> Self {
        ManagedConfig {
            overlay: Overlay::empty(),
            overlay_present: false,
            status: HealthStatus::Healthy,
            last_checked: None,
            settings,
        }
    }

    /// The overlay that landed, if any.
    pub fn overlay(&self) -> &Overlay {
        &self.overlay
    }

    /// The current health surface.
    pub fn health(&self) -> ConfigHealth {
        ConfigHealth {
            status: self.status,
            local_fallback_allowed: !self.settings.disable_local_cache,
            overlay_present: self.overlay_present,
            last_checked: self.last_checked,
            detail: match self.status {
                HealthStatus::Healthy => None,
                HealthStatus::Tainted => {
                    Some("the managed configuration source could not be reached".to_string())
                }
            },
        }
    }

    /// Fold one fetch outcome into the state.
    ///
    /// A success replaces the landed overlay and clears the taint. A failure
    /// marks the state tainted — including when an overlay had landed, which is
    /// the case `docs/09` §3.2 calls out ("a failed health check can mark a
    /// landed remote configuration as tainted") — and **retains** the last known
    /// overlay, so the managed restrictions do not silently disappear.
    pub fn apply_result(
        &mut self,
        result: Result<Overlay, String>,
        now_epoch_secs: u64,
    ) -> ConfigHealth {
        self.last_checked = Some(now_epoch_secs);
        match result {
            Ok(overlay) => {
                self.overlay_present = !overlay.is_empty();
                self.overlay = overlay;
                self.status = HealthStatus::Healthy;
            }
            Err(_) => {
                self.status = HealthStatus::Tainted;
            }
        }
        self.health()
    }

    /// Whether a re-check is due, given the current epoch time. The interval is
    /// `configRecheckIntervalMinutes` (default 10) — `DATA-CONFIG-09`.
    pub fn is_recheck_due(&self, now_epoch_secs: u64) -> bool {
        if !self.settings.has_source() {
            return false;
        }
        let Some(last) = self.last_checked else {
            return true;
        };
        let interval = u64::from(self.settings.recheck_interval_minutes) * 60;
        now_epoch_secs.saturating_sub(last) >= interval
    }

    /// The settings in force.
    pub fn settings(&self) -> &ManagedSettings {
        &self.settings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn settings() -> ManagedSettings {
        ManagedSettings {
            enabled: true,
            url: Some("https://bootstrap.example/config".to_string()),
            trust_delivery: true,
            recheck_interval_minutes: DEFAULT_RECHECK_INTERVAL_MINUTES,
            disable_local_cache: false,
        }
    }

    /// docs/09 §3.2: a failed health check marks a landed remote configuration
    /// tainted, and the landed overlay is retained.
    #[test]
    fn failed_check_taints_a_landed_overlay() {
        let mut managed = ManagedConfig::new(settings());
        let overlay = Overlay::parse(&json!({
            "connection": { "inferenceGatewayBaseUrl": "https://managed.example" },
        }));
        managed.apply_result(Ok(overlay), 1_000);
        assert_eq!(managed.health().status, HealthStatus::Healthy);
        assert!(managed.health().overlay_present);

        let health = managed.apply_result(Err("unreachable".to_string()), 1_060);
        assert_eq!(health.status, HealthStatus::Tainted);
        assert!(health.overlay_present, "the landed overlay is still known");
        assert!(
            managed
                .overlay()
                .locks("connection", "inferenceGatewayBaseUrl"),
            "managed restrictions must not disappear on a failed check"
        );
    }

    /// A successful re-check repairs the taint and replaces the overlay.
    #[test]
    fn recheck_repairs_the_taint() {
        let mut managed = ManagedConfig::new(settings());
        managed.apply_result(Err("unreachable".to_string()), 1_000);
        assert_eq!(managed.health().status, HealthStatus::Tainted);

        let overlay = Overlay::parse(&json!({
            "connection": { "inferenceGatewayBaseUrl": "https://managed.example" },
        }));
        let health = managed.apply_result(Ok(overlay), 1_120);
        assert_eq!(health.status, HealthStatus::Healthy);
        assert!(health.detail.is_none());
    }

    /// `disableLocalConfigCache` decides whether a stale local copy may stand in
    /// for the unreachable managed source (`docs/09` §3.2).
    #[test]
    fn disable_local_cache_forbids_the_local_fallback() {
        let mut managed = ManagedConfig::new(ManagedSettings {
            disable_local_cache: true,
            ..settings()
        });
        let health = managed.apply_result(Err("unreachable".to_string()), 1_000);
        assert!(!health.local_fallback_allowed);

        let mut managed = ManagedConfig::new(settings());
        let health = managed.apply_result(Err("unreachable".to_string()), 1_000);
        assert!(health.local_fallback_allowed);
    }

    /// DATA-CONFIG-09: the re-check interval is `configRecheckIntervalMinutes`
    /// (default 10 minutes).
    #[test]
    fn recheck_interval_drives_the_schedule() {
        let mut managed = ManagedConfig::new(settings());
        assert!(managed.is_recheck_due(0), "never checked yet");
        managed.apply_result(Err("unreachable".to_string()), 1_000);
        assert!(!managed.is_recheck_due(1_100));
        assert!(!managed.is_recheck_due(1_599));
        assert!(managed.is_recheck_due(1_600));
    }

    /// No managed source configured means no re-check is scheduled.
    #[test]
    fn no_source_means_no_recheck() {
        let managed = ManagedConfig::new(ManagedSettings::default());
        assert!(!managed.is_recheck_due(100_000));
        assert_eq!(managed.health().status, HealthStatus::Healthy);
    }

    /// The Source and Telemetry keys drive the settings; missing keys fall back to
    /// the documented defaults.
    #[test]
    fn settings_are_read_from_the_sections() {
        let source = Section(json!({
            "bootstrapEnabled": true,
            "bootstrapUrl": "https://bootstrap.example/config",
            "trustBootstrapDelivery": true,
        }));
        let telemetry = Section(json!({
            "configRecheckIntervalMinutes": 30,
            "disableLocalConfigCache": true,
        }));
        let settings = ManagedSettings::from_sections(&source, &telemetry);
        assert!(settings.enabled);
        assert!(settings.has_source());
        assert!(settings.trust_delivery);
        assert_eq!(settings.recheck_interval_minutes, 30);
        assert!(settings.disable_local_cache);
    }

    /// Absent or malformed keys fall back to the defaults instead of failing the
    /// read.
    #[test]
    fn missing_settings_use_documented_defaults() {
        let settings = ManagedSettings::from_sections(&Section(json!({})), &Section(json!({})));
        assert!(!settings.enabled);
        assert!(!settings.has_source());
        assert_eq!(
            settings.recheck_interval_minutes,
            DEFAULT_RECHECK_INTERVAL_MINUTES
        );
        assert!(!settings.disable_local_cache);

        let settings = ManagedSettings::from_sections(
            &Section(json!({ "bootstrapEnabled": true })),
            &Section(json!({ "configRecheckIntervalMinutes": 0 })),
        );
        assert!(!settings.has_source(), "a URL is required to have a source");
        assert_eq!(
            settings.recheck_interval_minutes, DEFAULT_RECHECK_INTERVAL_MINUTES,
            "zero is not a valid interval"
        );
    }
}
