//! Typed bridge surface between the renderer and the Rust core (docs/18-api-contracts.md).
//!
//! Phase 1 slice: only the envelope primitives and the API-path classification that the
//! `app://` layer needs. Privileged namespaces arrive one roadmap phase at a time; every
//! added method must carry sender, argument, and policy validation (docs/10 §8,
//! ARCH-PROC-03) and get a row in docs/18-api-contracts.md.
//!
//! Phase 2 adds the `settings.Custom3pSetup` namespace ([`setup`]): profiles, config
//! read/write/apply, the merged view, health, the export bundle, and the setup-tab
//! form schemas. Secrets cross this boundary as opaque handles, never as values.

use serde::{Deserialize, Serialize};

pub mod helper;
pub mod setup;

/// Commands the renderer may invoke. The set grows phase by phase and must never become
/// a generic CRUD surface (docs/10 §4.3).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Command {
    /// Liveness probe (boot diagnostics, TEST-ARCH-001 derived checks).
    Ping,
}

/// Response envelope mirroring [`Command`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Response {
    Pong,
}

pub fn dispatch(command: Command) -> Response {
    match command {
        Command::Ping => Response::Pong,
    }
}

/// Path prefixes routed through the third-party endpoint table (docs/10 §5.2).
/// Precondition (report-bug-hunt C5): `path` must ALREADY be percent-decoded and
/// contain no backslashes — `app-shell::protocol` decodes exactly once
/// (`percent_decode_path`) before calling this, so encoded prefixes like
/// `/%61pi/x` are classified correctly and double-decoding is impossible.
/// Everything else under `/` is either a UI asset or a client route with SPA fallback.
pub fn is_api_path(path: &str) -> bool {
    const PREFIXES: [&str; 7] = [
        "/api", "/edge-api", "/v1", "/v2", "/mcp-registry", "/sandbox", "/claudeai-rpc",
    ];
    // Strip any query string first.
    let path = path.split('?').next().unwrap_or(path);
    let path = path.trim_end_matches('/');
    PREFIXES.iter().any(|p| path == *p || path.starts_with(&format!("{p}/")))
}

/// Typed 503 body for paths the endpoint table neither stubs nor forwards (TEST-ARCH-004,
/// ARCH-PROC-07). Body shape is contractual; do not localize it here.
pub const NOT_AVAILABLE_BODY: &str = r#"{"error":{"type":"custom_3p_not_available","message":"This functionality is not available in third-party mode."}}"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_path_classification() {
        assert!(is_api_path("/api/bootstrap"));
        assert!(is_api_path("/api"));
        assert!(is_api_path("/edge-api/a/b"));
        assert!(is_api_path("/v1/messages"));
        assert!(is_api_path("/v1"));
        assert!(is_api_path("/v2/cache"));
        assert!(is_api_path("/mcp-registry/x"));
        assert!(is_api_path("/sandbox/x"));
        assert!(is_api_path("/claudeai-rpc/y?z=1"));
        assert!(!is_api_path("/v1xx"));
        assert!(!is_api_path("/cowork"));
        assert!(!is_api_path("/"));
        assert!(!is_api_path("/router.js"));
    }

    #[test]
    fn not_available_body_shape() {
        assert!(NOT_AVAILABLE_BODY.contains("\"type\":\"custom_3p_not_available\""));
    }

    #[test]
    fn command_roundtrip() {
        let raw = serde_json::to_string(&Command::Ping).unwrap();
        assert_eq!(raw, r#"{"type":"ping"}"#);
        let back: Command = serde_json::from_str(&raw).unwrap();
        assert_eq!(dispatch(back), Response::Pong);
    }
}
