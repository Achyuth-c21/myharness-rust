//! Code placement and permission policy — roadmap Phase 6 (`docs/13` Phase 6;
//! `docs/features/code.md`; `docs/09` §2.3; `REQ-CODE-01/02`,
//! `TEST-CODE-002/004`).
//!
//! The policy is read from the effective (merged) profile, never from the
//! renderer. A missing key keeps the rebuild's documented default; a malformed
//! value fails closed. The only path a renderer supplies is the folder string,
//! and it is canonicalized and checked here before it becomes session
//! metadata.
//!
//! Shipped shapes not observed (`docs/14` §9):
//! - `allowedWorkspaceFolders` is documented as array/object but its exact
//!   object shape was not captured. This implementation accepts an array of
//!   strings, an object with `folders`/`paths` arrays, or an object mapping
//!   path strings to booleans. Anything else grants nothing.
//! - `disableBypassPermissionsMode` is treated fail-closed: Bypass is allowed
//!   only when the key is explicitly `false`. Absence, `true`, or a malformed
//!   value all disable it.
//! - `disableDesktopLocalSessions` (if present) blocks local Code creation.
//!   Remote/SSH/WSL placement is deferred; this slice never falls back to it.

use std::path::{Path, PathBuf};

use myharness_config::overlay::MergedConfig;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::setup::BridgeError;

/// The effective Code policy for this process.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodePolicy {
    /// `capabilities/isClaudeCodeForDesktopEnabled`; absent means enabled.
    pub code_enabled: bool,
    /// False when `capabilities/disableDesktopLocalSessions` is explicitly
    /// true. Local is the only placement this slice implements.
    pub local_sessions_enabled: bool,
    /// True only when `capabilities/disableBypassPermissionsMode` is
    /// explicitly false.
    pub bypass_allowed: bool,
    /// `None` means unrestricted (the documented unset default). `Some([])`
    /// means "allow none"; `Some(roots)` confines selection to those roots.
    pub allowed_workspace_folders: Option<Vec<PathBuf>>,
}

impl Default for CodePolicy {
    fn default() -> Self {
        CodePolicy {
            code_enabled: true,
            local_sessions_enabled: true,
            bypass_allowed: false,
            allowed_workspace_folders: None,
        }
    }
}

impl CodePolicy {
    /// Read the effective policy from a merged profile. No active profile
    /// yields the defaults; Code remains enabled because the shipped gate is
    /// a capability flag, not an authentication fact.
    pub fn from_merged(merged: &MergedConfig) -> CodePolicy {
        // Absence keeps the documented default; a present but malformed value
        // is not a grant, so both capability gates fail closed.
        let code_enabled = match merged.value("capabilities", "isClaudeCodeForDesktopEnabled") {
            None | Some(Value::Null) => true,
            Some(Value::Bool(value)) => *value,
            Some(_) => false,
        };
        let local_sessions_enabled = match merged.value("capabilities", "disableDesktopLocalSessions")
        {
            None | Some(Value::Null) | Some(Value::Bool(false)) => true,
            Some(Value::Bool(true)) => false,
            Some(_) => false,
        };
        // Fail closed: only an explicit `false` enables Bypass.
        let bypass_allowed = matches!(
            merged.value("capabilities", "disableBypassPermissionsMode"),
            Some(Value::Bool(false))
        );
        let allowed_workspace_folders = match merged.value("capabilities", "allowedWorkspaceFolders")
        {
            None | Some(Value::Null) => None,
            Some(value) => Some(parse_workspace_folders(value)),
        };
        CodePolicy {
            code_enabled,
            local_sessions_enabled,
            bypass_allowed,
            allowed_workspace_folders,
        }
    }

    /// The renderer-safe subset. Absolute allowed roots stay in the core.
    pub fn view(&self) -> CodePolicyView {
        CodePolicyView {
            code_enabled: self.code_enabled,
            local_sessions_enabled: self.local_sessions_enabled,
            bypass_allowed: self.bypass_allowed,
            workspace_restricted: self.allowed_workspace_folders.is_some(),
        }
    }
}

/// Policy facts the dashboard/composer may render. Never includes folder paths.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodePolicyView {
    pub code_enabled: bool,
    pub local_sessions_enabled: bool,
    pub bypass_allowed: bool,
    pub workspace_restricted: bool,
}

/// The effective memory policy. The captured default is on (`docs/11`
/// lists `memoryEnabled` among the capability keys and the capture shows
/// memory on), so absence keeps it enabled; only an explicit `false`
/// disables it, and a malformed value fails closed like every other
/// capability gate (`crates/bridge/src/policy.rs` conventions).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryPolicy {
    pub enabled: bool,
}

impl Default for MemoryPolicy {
    fn default() -> Self {
        MemoryPolicy { enabled: true }
    }
}

impl MemoryPolicy {
    pub fn from_merged(merged: &MergedConfig) -> MemoryPolicy {
        let enabled = match merged.value("capabilities", "memoryEnabled") {
            None | Some(Value::Null) | Some(Value::Bool(true)) => true,
            Some(Value::Bool(false)) => false,
            Some(_) => false,
        };
        MemoryPolicy { enabled }
    }

    /// The renderer-safe subset (a single fact; nothing sensitive).
    pub fn view(&self) -> MemoryPolicyView {
        MemoryPolicyView {
            memory_enabled: self.enabled,
        }
    }
}

/// Policy facts the memory panel may render.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryPolicyView {
    pub memory_enabled: bool,
}

/// Validate and canonicalize a folder selected by the renderer.
///
/// Rules:
/// - absolute, non-empty, bounded, no control characters;
/// - must exist and be a directory;
/// - symlinks are resolved before the allowlist check, so a symlink cannot
///   escape an allowed root;
/// - when an allowlist is configured, the canonical folder must equal or sit
///   below a canonical root.
///
/// The returned path is the canonical spelling stored in the session. There is
/// no fallback to another folder and no directory creation.
pub fn validate_workspace_folder(
    raw: &str,
    policy: &CodePolicy,
) -> Result<PathBuf, BridgeError> {
    if raw.is_empty() || raw.chars().count() > 4096 || raw.contains('\0') {
        return Err(BridgeError::new(
            "CONFIG_VALIDATION",
            "the folder path is empty, too long, or contains an unsafe character",
        ));
    }
    if raw.chars().any(char::is_control) {
        return Err(BridgeError::new(
            "CONFIG_VALIDATION",
            "the folder path contains a control character",
        ));
    }
    let path = Path::new(raw);
    if !path.is_absolute() {
        return Err(BridgeError::new(
            "CONFIG_VALIDATION",
            "choose an absolute folder path",
        ));
    }
    let canonical = std::fs::canonicalize(path).map_err(|_| {
        BridgeError::new(
            "FOLDER_UNAVAILABLE",
            "the selected folder is not available; choose another folder",
        )
    })?;
    let metadata = std::fs::metadata(&canonical).map_err(|_| {
        BridgeError::new(
            "FOLDER_UNAVAILABLE",
            "the selected folder is not available; choose another folder",
        )
    })?;
    if !metadata.is_dir() {
        return Err(BridgeError::new(
            "FOLDER_UNAVAILABLE",
            "the selected path is not a folder",
        ));
    }
    let canonical_text = canonical.to_str().ok_or_else(|| {
        BridgeError::new(
            "CONFIG_VALIDATION",
            "the selected folder path is not valid UTF-8",
        )
    })?;
    if canonical_text.is_empty() {
        return Err(BridgeError::new(
            "CONFIG_VALIDATION",
            "the selected folder path is invalid",
        ));
    }

    if let Some(roots) = &policy.allowed_workspace_folders {
        let allowed = roots.iter().any(|root| {
            std::fs::canonicalize(root)
                .ok()
                .filter(|canonical_root| canonical_root.is_dir())
                .is_some_and(|canonical_root| canonical.starts_with(canonical_root))
        });
        if !allowed {
            return Err(BridgeError::new(
                "FOLDER_NOT_ALLOWED",
                "the selected folder is outside the allowed workspace folders",
            ));
        }
    }
    Ok(canonical)
}

fn parse_workspace_folders(value: &Value) -> Vec<PathBuf> {
    match value {
        Value::Array(items) => items
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .filter(|item| Path::new(item).is_absolute())
            .map(PathBuf::from)
            .collect(),
        Value::Object(map) => {
            for key in ["folders", "paths"] {
                if let Some(Value::Array(items)) = map.get(key) {
                    return items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::trim)
                        .filter(|item| !item.is_empty())
                        .filter(|item| Path::new(item).is_absolute())
                        .map(PathBuf::from)
                        .collect();
                }
            }
            // A path -> boolean map. Only `true` grants a root.
            map.iter()
                .filter_map(|(path, enabled)| {
                    (enabled == &Value::Bool(true))
                        .then(|| path.trim())
                        .filter(|path| !path.is_empty())
                        .filter(|path| Path::new(path).is_absolute())
                        .map(PathBuf::from)
                })
                .collect()
        }
        // A malformed allowlist grants nothing.
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use myharness_config::overlay::MergedConfig;
    use serde_json::json;
    use tempfile::TempDir;

    use super::{validate_workspace_folder, CodePolicy};

    fn merged(capabilities: serde_json::Value) -> MergedConfig {
        let mut fields = std::collections::BTreeMap::new();
        if let serde_json::Value::Object(map) = capabilities {
            for (key, value) in map {
                fields.insert(
                    format!("capabilities/{key}"),
                    myharness_config::overlay::MergedField {
                        section: "capabilities".into(),
                        key: key.clone(),
                        value,
                        source: myharness_config::overlay::ConfigSource::Local,
                        lock: myharness_config::overlay::LockState::Writable,
                    },
                );
            }
        }
        MergedConfig {
            profile_id: Some("test".into()),
            fields,
            overlay_present: false,
        }
    }

    #[test]
    fn bypass_is_fail_closed_unless_explicitly_disabled() {
        assert!(!CodePolicy::from_merged(&merged(json!({}))).bypass_allowed);
        assert!(!CodePolicy::from_merged(&merged(
            json!({"disableBypassPermissionsMode": true})
        ))
        .bypass_allowed);
        assert!(CodePolicy::from_merged(&merged(
            json!({"disableBypassPermissionsMode": false})
        ))
        .bypass_allowed);
        assert!(!CodePolicy::from_merged(&merged(
            json!({"disableBypassPermissionsMode": "false"})
        ))
        .bypass_allowed);
    }

    #[test]
    fn malformed_capability_values_fail_closed() {
        let policy = CodePolicy::from_merged(&merged(json!({
            "isClaudeCodeForDesktopEnabled": "yes",
            "disableDesktopLocalSessions": "no",
        })));
        assert!(!policy.code_enabled);
        assert!(!policy.local_sessions_enabled);
    }

    #[test]
    fn relative_allowlist_roots_grant_nothing() {
        let policy = CodePolicy::from_merged(&merged(json!({
            "allowedWorkspaceFolders": ["relative/root", "/definitely/not/here"]
        })));
        assert_eq!(policy.allowed_workspace_folders, Some(vec![
            std::path::PathBuf::from("/definitely/not/here")
        ]));
    }

    #[test]
    fn folder_validation_canonicalizes_and_enforces_an_allowlist() {
        let dir = TempDir::new().expect("temp dir");
        let allowed = dir.path().join("allowed");
        let outside = dir.path().join("outside");
        fs::create_dir_all(&allowed).unwrap();
        fs::create_dir_all(&outside).unwrap();

        let unrestricted = CodePolicy::default();
        let canonical = validate_workspace_folder(
            allowed.to_str().expect("utf-8 path"),
            &unrestricted,
        )
        .expect("allowed");
        assert_eq!(canonical, fs::canonicalize(&allowed).unwrap());

        let restricted = CodePolicy {
            allowed_workspace_folders: Some(vec![allowed.clone()]),
            ..CodePolicy::default()
        };
        assert!(validate_workspace_folder(
            outside.to_str().expect("utf-8 path"),
            &restricted
        )
        .is_err());
        assert!(validate_workspace_folder(
            allowed.to_str().expect("utf-8 path"),
            &restricted
        )
        .is_ok());

        let none = CodePolicy {
            allowed_workspace_folders: Some(Vec::new()),
            ..CodePolicy::default()
        };
        assert!(validate_workspace_folder(
            allowed.to_str().expect("utf-8 path"),
            &none
        )
        .is_err());
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_cannot_escape_an_allowed_root() {
        use std::os::unix::fs::symlink;

        let dir = TempDir::new().expect("temp dir");
        let allowed = dir.path().join("allowed");
        let outside = dir.path().join("outside");
        fs::create_dir_all(&allowed).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let link = allowed.join("escape");
        symlink(&outside, &link).unwrap();

        let restricted = CodePolicy {
            allowed_workspace_folders: Some(vec![allowed.clone()]),
            ..CodePolicy::default()
        };
        let error = validate_workspace_folder(link.to_str().expect("utf-8 path"), &restricted)
            .expect_err("a symlink escape must be refused");
        assert_eq!(error.code, "FOLDER_NOT_ALLOWED");
    }

    #[test]
    fn relative_missing_and_control_paths_are_refused() {
        let policy = CodePolicy::default();
        assert!(validate_workspace_folder("relative/path", &policy).is_err());
        assert!(validate_workspace_folder("/definitely/missing/myharness", &policy).is_err());
        assert!(validate_workspace_folder("/tmp/\0bad", &policy).is_err());
    }
}
