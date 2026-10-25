//! Code session permission modes — roadmap Phase 6 (`docs/13` Phase 6;
//! `docs/features/code.md` §Permission modes; `REQ-CODE-02`,
//! `TEST-CODE-004`).
//!
//! The mode is stored on the session and enforced by the agent tool loop, not
//! only by the renderer. The shipped UI wording is observed; the exact policy
//! precedence and the classification of a tool as an "edit" were not captured
//! (`docs/14` §9). This module therefore records the rebuild-defined rules:
//!
//! - `Default` — the existing tool policy decides (deny / ask / allow).
//! - `AcceptEdits` — a host-declared edit tool whose policy says `Ask` may run
//!   without a prompt; every other call keeps the existing policy. This slice
//!   has no built-in file-edit tool, so production passes no edit set and the
//!   mode never weakens an MCP approval. The seam exists so the mode is not a
//!   renderer-only label when file tools land.
//! - `Plan` — no tools are advertised, and a stale/hallucinated call is
//!   refused before the executor. The model can still plan from the
//!   conversation; it cannot execute changes.
//! - `Bypass` — an `Ask` decision runs without a prompt, but a policy `Deny`
//!   still wins. The mode is refused before creation/send unless the applied
//!   configuration explicitly permits it (`disableBypassPermissionsMode`).
//!
//! A mode never grants an approval the policy denied, never changes the model,
//! and never weakens the approval path for a mode it does not name.

use serde::{Deserialize, Serialize};

/// The four documented Code permission modes (`docs/features/code.md`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionMode {
    /// Ask for risky tools/edits.
    #[default]
    Default,
    /// Auto-approve host-declared file edits; still gate higher-risk actions.
    AcceptEdits,
    /// Read/plan without executing changes.
    Plan,
    /// Broad auto-approval, only when policy explicitly permits it.
    Bypass,
}

impl PermissionMode {
    /// The stored spelling used by the session row (`permission_mode`).
    pub fn as_stored(self) -> &'static str {
        match self {
            PermissionMode::Default => "default",
            PermissionMode::AcceptEdits => "acceptEdits",
            PermissionMode::Plan => "plan",
            PermissionMode::Bypass => "bypass",
        }
    }

    /// Parse a stored spelling. Unknown values are `None`; callers fail closed
    /// instead of treating a corrupt mode as `Default`.
    pub fn parse_stored(value: &str) -> Option<Self> {
        match value {
            "default" => Some(PermissionMode::Default),
            "acceptEdits" => Some(PermissionMode::AcceptEdits),
            "plan" => Some(PermissionMode::Plan),
            "bypass" => Some(PermissionMode::Bypass),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PermissionMode;

    #[test]
    fn stored_spellings_round_trip_and_unknown_values_fail_closed() {
        for mode in [
            PermissionMode::Default,
            PermissionMode::AcceptEdits,
            PermissionMode::Plan,
            PermissionMode::Bypass,
        ] {
            assert_eq!(
                PermissionMode::parse_stored(mode.as_stored()),
                Some(mode)
            );
        }
        assert_eq!(PermissionMode::parse_stored("auto"), None);
        assert_eq!(PermissionMode::parse_stored(""), None);
    }
}
