//! The update lifecycle's policy, state machine, and fail-closed
//! verification (roadmap Phase 7's update slice; `docs/13` Phase 7;
//! `docs/features/updates-telemetry-privacy.md` §4, §8; `REQ-UPDATE-01/02`;
//! `TEST-UPDATE-001/002`).
//!
//! The shipped lifecycle is `check → verify metadata/signature → download →
//! verify → stage → prompt/enforce → restart → health check → rollback on
//! failure`. What this crate implements honestly:
//!
//! - **The policy** (`docs/09` §2.5): `disableAutoUpdates`, the enforcement
//!   window (`autoUpdaterEnforcementHours`, default 72, `0` = immediately
//!   enforceable — `0` has explicit meaning), the release channel, and the
//!   updates-host routing flag.
//! - **The lifecycle state machine** with the session guard ("do not
//!   force-install while a task is streaming") and the enforcement window.
//! - **Fail-closed signature verification.** The shipped feed's wire format
//!   and public key were never captured (the banner offered `2.42304.2` and
//!   was not applied), so there is nothing to verify against yet. The
//!   production verifier therefore refuses *every* bundle — and the state
//!   machine treats a refusal as terminal for that bundle while the running
//!   version stays runnable. That is the Phase 7 exit criterion ("update
//!   failure leaves the old version runnable") held by construction, and it
//!   is the same posture the catalog signature check takes (`docs/14` §9
//!   item 20). Capturing the feed format moves [`NoTrustAnchor`] aside; it
//!   does not weaken the gate: **never install without verification**.
//!
//! The crate is deliberately free of network and process code. A feed
//! client cannot be written against a wire nobody has seen; when one is
//! captured, it lands behind the same seam and the state machine here stays
//! the authority for what may install.

use myharness_config::overlay::MergedConfig;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The documented enforcement-window default (`docs/09` §2.5).
pub const DEFAULT_ENFORCEMENT_HOURS: u32 = 72;
/// The configuration relaunch window's default (`docs/09` §2.5: "hours;
/// default 24; 0 = immediate").
pub const DEFAULT_RELAUNCH_HOURS: u32 = 24;

const HOUR_MS: i64 = 3_600_000;

/// The effective update policy, read fail-closed from the merged profile's
/// `telemetry` section (`docs/09` §2.5). A malformed window falls back to
/// the documented default rather than to `0` — an unparseable value must not
/// *shorten* the window.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePolicy {
    /// `disableAutoUpdates` — no check, download, or install.
    pub auto_updates_blocked: bool,
    /// `autoUpdaterEnforcementHours` — how long an offered update may wait
    /// before enforcing. `0` enforces immediately.
    pub enforcement_hours: u32,
    /// `releaseChannel`, verbatim.
    pub release_channel: Option<String>,
    /// `updateViaUpdatesHost` — the update host is rerouted so
    /// `api.anthropic.com` can stay blocked (egress honors this in
    /// `crates/config::egress`).
    pub update_via_updates_host: bool,
    /// `relaunchEnforcementHours` — the configuration relaunch window;
    /// `0` = immediate.
    pub relaunch_hours: u32,
}

impl Default for UpdatePolicy {
    fn default() -> Self {
        UpdatePolicy {
            auto_updates_blocked: false,
            enforcement_hours: DEFAULT_ENFORCEMENT_HOURS,
            release_channel: None,
            update_via_updates_host: false,
            relaunch_hours: DEFAULT_RELAUNCH_HOURS,
        }
    }
}

fn read_hours(merged: &MergedConfig, key: &str, default: u32) -> u32 {
    match merged.value("telemetry", key) {
        // `0` has explicit meaning (immediate); it is honored, not defaulted.
        Some(Value::Number(number)) => match number.as_u64() {
            Some(hours) if hours <= u64::from(u32::MAX) => hours as u32,
            _ => default,
        },
        _ => default,
    }
}

impl UpdatePolicy {
    pub fn from_merged(merged: &MergedConfig) -> UpdatePolicy {
        UpdatePolicy {
            auto_updates_blocked: matches!(
                merged.value("telemetry", "disableAutoUpdates"),
                Some(Value::Bool(true))
            ),
            enforcement_hours: read_hours(merged, "autoUpdaterEnforcementHours", DEFAULT_ENFORCEMENT_HOURS),
            release_channel: match merged.value("telemetry", "releaseChannel") {
                Some(Value::String(channel)) if !channel.trim().is_empty() => {
                    Some(channel.trim().to_string())
                }
                _ => None,
            },
            update_via_updates_host: matches!(
                merged.value("telemetry", "updateViaUpdatesHost"),
                Some(Value::Bool(true))
            ),
            relaunch_hours: read_hours(merged, "relaunchEnforcementHours", DEFAULT_RELAUNCH_HOURS),
        }
    }

    /// True when the enforcement window has expired for an update offered at
    /// `offered_at_ms`.
    pub fn enforcement_due(&self, offered_at_ms: i64, now_ms: i64) -> bool {
        let window = i64::from(self.enforcement_hours).saturating_mul(HOUR_MS);
        now_ms >= offered_at_ms.saturating_add(window)
    }

    /// True when the configuration relaunch window has expired for a
    /// configuration change at `changed_at_ms`.
    pub fn relaunch_due(&self, changed_at_ms: i64, now_ms: i64) -> bool {
        let window = i64::from(self.relaunch_hours).saturating_mul(HOUR_MS);
        now_ms >= changed_at_ms.saturating_add(window)
    }
}

/// One update bundle's metadata, as the feed offered it. The wire spelling
/// of the real feed is unknown (`docs/14` §9); this is the state machine's
/// input shape, not a wire contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateManifest {
    pub version: String,
    pub offered_at_ms: i64,
}

/// The verdict of a verification attempt. There is no `Unverified`-but-
/// proceed: **never install without signature verification**
/// (`docs/features/updates-telemetry-privacy.md` §4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verification {
    Verified,
    Refused { reason: String },
}

/// The signature-verification seam. The production implementation is
/// [`NoTrustAnchor`] until a feed format and a public key are actually
/// captured; a test verifier can grant `Verified`.
pub trait BundleVerifier: Send + Sync {
    fn verify(&self, manifest: &UpdateManifest) -> Verification;
}

/// The production verifier: refuses every bundle with the recorded reason.
/// The shipped public key was never captured, so there is nothing to verify
/// against — and a bundle nobody verified is a bundle nobody installs.
pub struct NoTrustAnchor;

impl BundleVerifier for NoTrustAnchor {
    fn verify(&self, _manifest: &UpdateManifest) -> Verification {
        Verification::Refused {
            reason: "no trust anchor is configured for the update feed".to_string(),
        }
    }
}

/// What the session guard and the enforcement window decide.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstallDecision {
    /// Inside the window and no task is streaming: prompt the user.
    OfferRestart,
    /// The window expired and no task is streaming: enforce.
    EnforceNow,
    /// A turn is streaming. The guard delays *any* install — prompt or
    /// enforce — until the task ends (`docs/features/updates-telemetry-
    /// privacy.md` §4: do not force-install while a task is streaming).
    SessionGuardBlocks,
    /// The policy blocks auto-updates outright.
    PolicyBlocks,
}

/// The lifecycle states (`docs/features/updates-telemetry-privacy.md` §8).
/// `Running` is the app's own version; every failed or refused transition
/// returns to it — the old version never stops being runnable.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum UpdateState {
    Running,
    Available { version: String },
    Downloading { version: String },
    /// Verified and staged; awaiting the prompt/enforce decision.
    Staged { version: String },
    /// A restart to `version` happened and the health check passed.
    Updated { version: String },
    /// The health check failed after a restart; the previous version is
    /// running again.
    RolledBack { version: String, reason: String },
    /// The bundle failed verification, download, or staging. The running
    /// version is untouched.
    Failed { version: String, reason: String },
}

/// One lifecycle. Transitions that the spec forbids are refused with the
/// state unchanged, so a caller cannot walk the machine into installing an
/// unverified bundle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateLifecycle {
    pub state: UpdateState,
}

impl Default for UpdateLifecycle {
    fn default() -> Self {
        UpdateLifecycle {
            state: UpdateState::Running,
        }
    }
}

impl UpdateLifecycle {
    /// An update was offered. Blocked policies still record the offer (the
    /// shipped banner was informational); the install path is what the
    /// policy gates.
    pub fn offer(&mut self, manifest: &UpdateManifest) -> Result<(), String> {
        match self.state {
            UpdateState::Running | UpdateState::Failed { .. } | UpdateState::RolledBack { .. } => {
                self.state = UpdateState::Available {
                    version: manifest.version.clone(),
                };
                Ok(())
            }
            _ => Err("an update is already in flight".to_string()),
        }
    }

    /// Download accepted; staging verifies. A refused verification is
    /// terminal for this bundle: the machine returns to `Running` (the old
    /// version stays runnable) and the reason is kept for display.
    pub fn stage(&mut self, manifest: &UpdateManifest, verifier: &dyn BundleVerifier) {
        self.state = UpdateState::Downloading {
            version: manifest.version.clone(),
        };
        match verifier.verify(manifest) {
            Verification::Verified => {
                self.state = UpdateState::Staged {
                    version: manifest.version.clone(),
                };
            }
            Verification::Refused { reason } => {
                self.state = UpdateState::Failed {
                    version: manifest.version.clone(),
                    reason,
                };
            }
        }
    }

    /// The install decision for a staged update: policy first, then the
    /// session guard, then the window.
    pub fn install_decision(
        &self,
        policy: &UpdatePolicy,
        manifest: &UpdateManifest,
        now_ms: i64,
        streaming: bool,
    ) -> InstallDecision {
        if !matches!(self.state, UpdateState::Staged { .. }) {
            return InstallDecision::PolicyBlocks;
        }
        if policy.auto_updates_blocked {
            return InstallDecision::PolicyBlocks;
        }
        if streaming {
            return InstallDecision::SessionGuardBlocks;
        }
        if policy.enforcement_due(manifest.offered_at_ms, now_ms) {
            InstallDecision::EnforceNow
        } else {
            InstallDecision::OfferRestart
        }
    }

    /// A restart to the staged version succeeded and the health check
    /// passed. Only a staged update can commit.
    pub fn commit(&mut self, version: &str) -> Result<(), String> {
        match &self.state {
            UpdateState::Staged { version: staged } if staged == version => {
                self.state = UpdateState::Updated {
                    version: version.to_string(),
                };
                Ok(())
            }
            _ => Err("only a staged update can be committed".to_string()),
        }
    }

    /// The health check failed after the restart: the previous version is
    /// running again, with the reason recorded for display
    /// ("Failed | Error + retry; old version remains").
    pub fn rollback(&mut self, version: &str, reason: &str) -> Result<(), String> {
        match &self.state {
            UpdateState::Staged { version: staged } | UpdateState::Updated { version: staged }
                if staged == version =>
            {
                self.state = UpdateState::RolledBack {
                    version: version.to_string(),
                    reason: reason.to_string(),
                };
                Ok(())
            }
            _ => Err("nothing was installed to roll back".to_string()),
        }
    }
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

    struct Granting;
    impl BundleVerifier for Granting {
        fn verify(&self, _manifest: &UpdateManifest) -> Verification {
            Verification::Verified
        }
    }

    fn manifest(now: i64) -> UpdateManifest {
        UpdateManifest {
            version: "2.42304.2".to_string(),
            offered_at_ms: now,
        }
    }

    // ---------------------------------------------------------------
    // Policy (`docs/09` §2.5)
    // ---------------------------------------------------------------

    #[test]
    fn the_policy_defaults_match_the_documented_windows() {
        let policy = UpdatePolicy::from_merged(&merged(json!({})));
        assert!(!policy.auto_updates_blocked);
        assert_eq!(policy.enforcement_hours, 72);
        assert_eq!(policy.relaunch_hours, 24);
        assert_eq!(policy.release_channel, None);
        assert!(!policy.update_via_updates_host);
    }

    #[test]
    fn zero_windows_mean_immediate_and_malformed_keeps_the_default() {
        let policy = UpdatePolicy::from_merged(&merged(json!({
            "autoUpdaterEnforcementHours": 0,
            "relaunchEnforcementHours": 0,
        })));
        assert!(policy.enforcement_due(1_000, 1_000));
        assert!(policy.relaunch_due(1_000, 1_000));

        let policy = UpdatePolicy::from_merged(&merged(json!({
            "autoUpdaterEnforcementHours": "soon",
        })));
        assert_eq!(policy.enforcement_hours, 72, "malformed must not shorten");
    }

    #[test]
    fn the_enforcement_window_expires_after_the_configured_hours() {
        let policy = UpdatePolicy::from_merged(&merged(json!({
            "autoUpdaterEnforcementHours": 72,
        })));
        let offered = 1_000_000;
        assert!(!policy.enforcement_due(offered, offered + 71 * HOUR_MS));
        assert!(policy.enforcement_due(offered, offered + 72 * HOUR_MS));
    }

    #[test]
    fn the_blocked_switch_is_read_and_the_channel_recorded() {
        let policy = UpdatePolicy::from_merged(&merged(json!({
            "disableAutoUpdates": true,
            "releaseChannel": "stable",
            "updateViaUpdatesHost": true,
        })));
        assert!(policy.auto_updates_blocked);
        assert_eq!(policy.release_channel.as_deref(), Some("stable"));
        assert!(policy.update_via_updates_host);
    }

    // ---------------------------------------------------------------
    // Lifecycle (`TEST-UPDATE-001`): verify gate and rollback
    // ---------------------------------------------------------------

    #[test]
    fn an_unverifiable_bundle_fails_and_the_old_version_stays_runnable() {
        let now = 1_000_000;
        let mut lifecycle = UpdateLifecycle::default();
        lifecycle.offer(&manifest(now)).expect("offer");
        // The production verifier has no trust anchor: the staged step
        // refuses, the machine returns to Running, and the reason is kept.
        lifecycle.stage(&manifest(now), &NoTrustAnchor);
        assert_eq!(
            lifecycle.state,
            UpdateState::Failed {
                version: "2.42304.2".to_string(),
                reason: "no trust anchor is configured for the update feed".to_string(),
            }
        );
        // A failed bundle cannot be committed, and the machine still runs.
        assert!(lifecycle.commit("2.42304.2").is_err());
        assert_eq!(lifecycle.state, UpdateState::Failed { version: "2.42304.2".to_string(), reason: "no trust anchor is configured for the update feed".to_string() });
    }

    #[test]
    fn a_verified_bundle_stages_prompts_commits_and_rolls_back_on_a_failed_health_check() {
        let now = 1_000_000;
        let mut lifecycle = UpdateLifecycle::default();
        lifecycle.offer(&manifest(now)).expect("offer");
        lifecycle.stage(&manifest(now), &Granting);
        assert_eq!(
            lifecycle.state,
            UpdateState::Staged {
                version: "2.42304.2".to_string()
            }
        );
        // Inside the window: a prompt, not an enforcement.
        let policy = UpdatePolicy::default();
        assert_eq!(
            lifecycle.install_decision(&policy, &manifest(now), now + HOUR_MS, false),
            InstallDecision::OfferRestart
        );
        // The restart succeeded and the health check passed.
        lifecycle.commit("2.42304.2").expect("commit");
        assert_eq!(
            lifecycle.state,
            UpdateState::Updated {
                version: "2.42304.2".to_string()
            }
        );
        // The health check failed after the restart: the old version runs.
        lifecycle.rollback("2.42304.2", "the health check did not pass").expect("rollback");
        assert!(matches!(lifecycle.state, UpdateState::RolledBack { .. }));
    }

    #[test]
    fn nothing_installs_outside_the_staged_state() {
        let mut lifecycle = UpdateLifecycle::default();
        let policy = UpdatePolicy::default();
        let now = 1_000_000;
        assert_eq!(
            lifecycle.install_decision(&policy, &manifest(now), now, false),
            InstallDecision::PolicyBlocks,
            "an offered-but-unstaged update must not install"
        );
        assert!(lifecycle.commit("2.42304.2").is_err());
        assert!(lifecycle.rollback("2.42304.2", "no reason").is_err());
    }

    // ---------------------------------------------------------------
    // Windows and the session guard (`TEST-UPDATE-002`)
    // ---------------------------------------------------------------

    #[test]
    fn the_expired_window_enforces_and_blocked_policy_refuses() {
        let now = 1_000_000;
        let mut lifecycle = UpdateLifecycle::default();
        lifecycle.offer(&manifest(now)).expect("offer");
        lifecycle.stage(&manifest(now), &Granting);

        let expired = UpdatePolicy::from_merged(&merged(json!({
            "autoUpdaterEnforcementHours": 72,
        })));
        assert_eq!(
            lifecycle.install_decision(&expired, &manifest(now), now + 73 * HOUR_MS, false),
            InstallDecision::EnforceNow
        );

        let blocked = UpdatePolicy::from_merged(&merged(json!({
            "disableAutoUpdates": true,
        })));
        assert_eq!(
            lifecycle.install_decision(&blocked, &manifest(now), now + 73 * HOUR_MS, false),
            InstallDecision::PolicyBlocks
        );
    }

    #[test]
    fn a_streaming_task_blocks_even_an_expired_window() {
        let now = 1_000_000;
        let mut lifecycle = UpdateLifecycle::default();
        lifecycle.offer(&manifest(now)).expect("offer");
        lifecycle.stage(&manifest(now), &Granting);
        let policy = UpdatePolicy::from_merged(&merged(json!({
            "autoUpdaterEnforcementHours": 72,
        })));
        assert_eq!(
            lifecycle.install_decision(&policy, &manifest(now), now + 73 * HOUR_MS, true),
            InstallDecision::SessionGuardBlocks,
            "do not force-install while a task is streaming"
        );
    }

    #[test]
    fn the_relaunch_window_honors_zero_and_the_default() {
        let policy = UpdatePolicy::from_merged(&merged(json!({})));
        assert!(!policy.relaunch_due(1_000, 1_000 + 23 * HOUR_MS));
        assert!(policy.relaunch_due(1_000, 1_000 + 24 * HOUR_MS));
        let immediate = UpdatePolicy::from_merged(&merged(json!({
            "relaunchEnforcementHours": 0,
        })));
        assert!(immediate.relaunch_due(1_000, 1_000));
    }
}
