//! Egress policy for MCP hosts — roadmap Phase 5 (`docs/12` §5.1;
//! `docs/features/mcp-skills-tools.md` §4; `TEST-AI-020`).
//!
//! The policy is fail-closed **when it is configured**: an empty allowlist
//! denies every host, and only an explicit [`EgressPolicy::allow_all`] opens
//! the gate. The manager's `None` means "no policy was supplied by the host",
//! not "allow everything"; the app shell must supply the managed/developer
//! policy before it starts HTTP servers.
//!
//! Limits, stated rather than implied:
//!
//! - Hosts match **exactly** (case-insensitive). `example.com` does not allow
//!   `api.example.com`; a wildcard is not invented. A configured host entry
//!   covers every port on that host; the shipped port granularity was not
//!   observed (`docs/14` §9).
//! - Only `http` and `https` are accepted.
//! - **stdio servers are not inspected.** A child process can open its own
//!   sockets, and this layer cannot see them. Egress enforcement for stdio
//!   belongs to the OS/proxy boundary; claiming otherwise here would be false.
//! - Redirects are not part of this check. The pinned `rmcp` 3.7 Streamable
//!   HTTP client follows only same-origin 307/308 redirects (up to 10) and
//!   returns 301/302/303 or cross-origin redirects to the caller, so the
//!   checked host cannot be swapped for another host by a redirect. This
//!   relies on the default client; a custom transport client must re-check
//!   every hop.

use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;

use crate::config::ServerConfig;

/// Host allowlist for outbound MCP connections.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EgressPolicy {
    /// Exact hostnames (case-insensitive). Empty denies every host.
    #[serde(default)]
    pub allowed_hosts: std::collections::BTreeSet<String>,
    /// Explicit development escape hatch. Never set this from an untrusted
    /// config; it is a deliberate owner decision.
    #[serde(default)]
    pub allow_all: bool,
}

impl EgressPolicy {
    /// Deny every host.
    pub fn deny_all() -> Self {
        Self::default()
    }

    /// Allow exactly these hosts.
    pub fn allow_hosts(hosts: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            allowed_hosts: hosts
                .into_iter()
                .map(|host| host.into().trim().to_ascii_lowercase())
                .filter(|host| !host.is_empty())
                .collect(),
            allow_all: false,
        }
    }

    /// Allow every `http`/`https` host. Explicit, never a default.
    pub fn allow_every_host() -> Self {
        Self {
            allowed_hosts: Default::default(),
            allow_all: true,
        }
    }

    /// True when the host is allowed.
    pub fn allows_host(&self, host: &str) -> bool {
        if self.allow_all {
            return true;
        }
        self.allowed_hosts
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(host))
    }

    /// True when the URL is `http`/`https`, has a host, and that host is
    /// allowed.
    pub fn allows_url(&self, url: &Url) -> bool {
        if !matches!(url.scheme(), "http" | "https") {
            return false;
        }
        url.host_str().is_some_and(|host| self.allows_host(host))
    }

    /// Check a raw URL.
    pub fn check_url(&self, url: &str) -> Result<(), EgressError> {
        let parsed = Url::parse(url).map_err(|error| EgressError::InvalidUrl(error.to_string()))?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err(EgressError::UnsupportedScheme);
        }
        let host = parsed
            .host_str()
            .ok_or_else(|| EgressError::InvalidUrl("the URL has no host".to_string()))?;
        if self.allows_host(host) {
            Ok(())
        } else {
            Err(EgressError::HostNotAllowed {
                host: host.to_string(),
            })
        }
    }

    /// Check a server definition. Stdio is intentionally a no-op here; see the
    /// module limits.
    pub fn check_server(&self, config: &ServerConfig) -> Result<(), EgressError> {
        match config {
            ServerConfig::Stdio { .. } => Ok(()),
            ServerConfig::Http { url, .. } | ServerConfig::Sse { url, .. } => self.check_url(url),
        }
    }
}

/// Egress refusal.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum EgressError {
    /// The host is not on the allowlist. The host is retained for
    /// programmatic use but deliberately omitted from `Display`: it may be a
    /// private/internal name (`docs/features/mcp-skills-tools.md` §4).
    #[error("the MCP host is not on the egress allowlist")]
    HostNotAllowed {
        /// Denied host.
        host: String,
    },
    /// The URL could not be parsed.
    #[error("the MCP URL is invalid: {0}")]
    InvalidUrl(String),
    /// Only `http` and `https` are supported.
    #[error("the MCP URL scheme must be http or https")]
    UnsupportedScheme,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn http(url: &str) -> ServerConfig {
        ServerConfig::Http {
            url: url.to_string(),
            headers: BTreeMap::new(),
        }
    }

    #[test]
    fn the_default_denies_every_host() {
        let policy = EgressPolicy::default();
        assert!(!policy.allows_host("example.com"));
        assert_eq!(
            policy.check_url("https://example.com/mcp"),
            Err(EgressError::HostNotAllowed {
                host: "example.com".to_string()
            })
        );
    }

    #[test]
    fn hosts_match_exactly_and_case_insensitively() {
        let policy = EgressPolicy::allow_hosts(["Example.COM"]);
        assert!(policy.allows_host("example.com"));
        assert!(policy.allows_host("EXAMPLE.com"));
        assert!(!policy.allows_host("api.example.com"));
        assert!(!policy.allows_host("example.com.evil"));
        assert!(policy.check_url("https://example.com:8443/mcp").is_ok());
    }

    #[test]
    fn only_http_and_https_are_allowed() {
        let policy = EgressPolicy::allow_every_host();
        assert_eq!(
            policy.check_url("ftp://example.com/mcp"),
            Err(EgressError::UnsupportedScheme)
        );
        assert_eq!(
            policy.check_url("file:///tmp/mcp"),
            Err(EgressError::UnsupportedScheme)
        );
    }

    #[test]
    fn allow_all_is_explicit_and_still_requires_http() {
        let policy = EgressPolicy::allow_every_host();
        assert!(policy.check_url("https://anywhere.example/mcp").is_ok());
        assert!(policy.check_url("http://127.0.0.1:3000/mcp").is_ok());
        assert!(policy.check_url("not a url").is_err());
    }

    #[test]
    fn stdio_is_not_inspected() {
        let policy = EgressPolicy::deny_all();
        let stdio = ServerConfig::Stdio {
            command: "server".to_string(),
            args: Vec::new(),
            env: BTreeMap::new(),
        };
        assert_eq!(policy.check_server(&stdio), Ok(()));
        assert!(policy
            .check_server(&http("https://example.com/mcp"))
            .is_err());
    }

    #[test]
    fn a_denied_host_is_not_echoed_in_display() {
        let error = EgressError::HostNotAllowed {
            host: "internal.corp.example".to_string(),
        };
        assert!(!error.to_string().contains("internal.corp.example"));
    }
}
