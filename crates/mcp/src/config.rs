//! MCP server definitions and their trust-boundary rules.
//!
//! Sources: `docs/12` §5.1 (stdio/HTTP/SSE, tool namespacing, untrusted
//! schemas), `docs/10` §7–§8 (separate trust domain, no secret in a renderer
//! payload), `docs/11` §3 (secret handling).

use std::collections::BTreeMap;
use std::fmt;

use http::{HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::{McpError, McpResult};

/// Maximum configured server-name length, in bytes.
pub const MAX_SERVER_NAME_LEN: usize = 64;

/// Maximum MCP tool-name length, in bytes.
pub const MAX_TOOL_NAME_LEN: usize = 128;

/// Maximum stored error-message length, in characters.
pub const MAX_ERROR_MESSAGE_LEN: usize = 2_048;

/// Maximum tools accepted from one server.
///
/// Rebuild-defined resource bound (`docs/14` §9): the shipped app's exact cap
/// was not captured, but an unbounded catalog is untrusted input and cannot be
/// allowed to grow memory without limit. Tool search/on-demand schemas remain
/// the product behavior (`docs/12` §5.1).
pub const MAX_TOOLS_PER_SERVER: usize = 4_096;

/// Maximum serialized size of one input or output schema, in bytes.
///
/// Rebuild-defined resource bound (`docs/14` §9), for the same reason as
/// [`MAX_TOOLS_PER_SERVER`].
pub const MAX_TOOL_SCHEMA_BYTES: usize = 256 * 1024;

/// One MCP server definition.
///
/// The `Debug` implementation deliberately never prints environment values or
/// header values: those are the two places the shipped configuration carries
/// credentials (`docs/09` §4; `docs/12` §5.1).
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "transport", rename_all = "camelCase")]
pub enum ServerConfig {
    /// A child process speaking MCP over stdin/stdout.
    Stdio {
        /// Executable to spawn.
        command: String,
        /// Arguments passed to the executable.
        #[serde(default)]
        args: Vec<String>,
        /// Environment additions. Values are secrets.
        #[serde(default)]
        env: BTreeMap<String, String>,
    },
    /// Streamable HTTP (the current MCP HTTP transport).
    Http {
        /// Endpoint URL.
        url: String,
        /// Extra headers. Values are secrets.
        #[serde(default)]
        headers: BTreeMap<String, String>,
    },
    /// Legacy HTTP+SSE. Modelled for imported/managed definitions; the current
    /// SDK exposes SSE inside Streamable HTTP instead.
    Sse {
        /// Endpoint URL.
        url: String,
        /// Extra headers. Values are secrets.
        #[serde(default)]
        headers: BTreeMap<String, String>,
    },
}

impl fmt::Debug for ServerConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stdio {
                command,
                args,
                env,
            } => formatter
                .debug_struct("Stdio")
                .field("command", command)
                .field("args", args)
                .field("env_keys", &env.keys().collect::<Vec<_>>())
                .finish(),
            Self::Http { url, headers } => formatter
                .debug_struct("Http")
                .field("url", &redacted_url(url))
                .field("header_keys", &headers.keys().collect::<Vec<_>>())
                .finish(),
            Self::Sse { url, headers } => formatter
                .debug_struct("Sse")
                .field("url", &redacted_url(url))
                .field("header_keys", &headers.keys().collect::<Vec<_>>())
                .finish(),
        }
    }
}

impl ServerConfig {
    /// Validate the transport shape before a server is added to a manager.
    pub fn validate(&self) -> McpResult<()> {
        match self {
            Self::Stdio {
                command,
                args,
                env,
            } => {
                if command.trim().is_empty() {
                    return Err(McpError::InvalidConfig(
                        "the stdio command is empty".to_string(),
                    ));
                }
                if command.contains('\0') {
                    return Err(McpError::InvalidConfig(
                        "the stdio command contains a NUL byte".to_string(),
                    ));
                }
                for argument in args {
                    if argument.contains('\0') {
                        return Err(McpError::InvalidConfig(
                            "a stdio argument contains a NUL byte".to_string(),
                        ));
                    }
                }
                for (key, value) in env {
                    if key.is_empty() || key.contains('=') || key.contains('\0') {
                        return Err(McpError::InvalidConfig(format!(
                            "invalid stdio environment key: {key:?}"
                        )));
                    }
                    if value.contains('\0') {
                        return Err(McpError::InvalidConfig(format!(
                            "the value of stdio environment key {key:?} contains a NUL byte"
                        )));
                    }
                }
            }
            Self::Http { url, headers } | Self::Sse { url, headers } => {
                let parsed = Url::parse(url)
                    .map_err(|error| McpError::InvalidConfig(format!("invalid URL: {error}")))?;
                if !matches!(parsed.scheme(), "http" | "https") {
                    return Err(McpError::InvalidConfig(
                        "the URL scheme must be http or https".to_string(),
                    ));
                }
                if parsed.host_str().is_none() {
                    return Err(McpError::InvalidConfig(
                        "the URL has no host".to_string(),
                    ));
                }
                for (name, value) in headers {
                    HeaderName::from_bytes(name.as_bytes()).map_err(|error| {
                        McpError::InvalidConfig(format!("invalid header name {name:?}: {error}"))
                    })?;
                    HeaderValue::from_str(value).map_err(|error| {
                        McpError::InvalidConfig(format!("invalid header value: {error}"))
                    })?;
                }
            }
        }
        Ok(())
    }

    /// The transport label used by status views and diagnostics.
    pub fn transport(&self) -> &'static str {
        match self {
            Self::Stdio { .. } => "stdio",
            Self::Http { .. } => "http",
            Self::Sse { .. } => "sse",
        }
    }

    /// Redact known credential values from a message and bound its length.
    ///
    /// Values shorter than three bytes are not substituted: replacing a
    /// one-character environment value would destroy the message without
    /// protecting a meaningful secret. The full value is still never logged
    /// by `Debug`.
    pub fn redact(&self, message: &str) -> String {
        let mut redacted = message.to_string();
        for secret in self.secret_values() {
            if secret.len() >= 3 {
                redacted = redacted.replace(&secret, "[redacted]");
            }
        }
        redacted.chars().take(MAX_ERROR_MESSAGE_LEN).collect()
    }

    fn secret_values(&self) -> Vec<String> {
        match self {
            Self::Stdio { env, .. } => env.values().cloned().collect(),
            Self::Http { url, headers } | Self::Sse { url, headers } => {
                let mut values: Vec<String> = headers.values().cloned().collect();
                // A transport error often echoes the endpoint. The userinfo
                // password and the entire query string are credential-bearing
                // in managed definitions (`docs/12` §5.1), so both are
                // redacted from a message even though `Debug` already hides
                // them.
                if let Ok(parsed) = Url::parse(url) {
                    if let Some(password) = parsed.password() {
                        values.push(password.to_string());
                    }
                    if let Some(query) = parsed.query() {
                        values.push(query.to_string());
                    }
                }
                values
            }
        }
    }
}

/// Validate a configured server name.
///
/// A server name is the left half of the `<server>:<tool>` namespace, so `:`
/// is reserved. The conservative character set is a rebuild-defined safety
/// rule (`docs/14` §9): it keeps names usable in logs, snapshots, and provider
/// tool identifiers without inventing a mapping. Tool names are validated
/// separately and may contain `:`.
pub fn validate_server_name(name: &str) -> McpResult<()> {
    if name.is_empty() {
        return Err(McpError::InvalidServerName(
            "the server name is empty".to_string(),
        ));
    }
    if name.len() > MAX_SERVER_NAME_LEN {
        return Err(McpError::InvalidServerName(format!(
            "the server name is longer than {MAX_SERVER_NAME_LEN} bytes"
        )));
    }
    if !name
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.'))
    {
        return Err(McpError::InvalidServerName(format!(
            "the server name {name:?} contains a character outside [A-Za-z0-9._-]"
        )));
    }
    Ok(())
}

/// Validate an MCP tool name as returned by a server.
///
/// Control characters are refused; `:` is allowed because the namespace is
/// split on the first colon, leaving the rest of the tool name intact.
pub fn validate_tool_name(name: &str) -> McpResult<()> {
    if name.is_empty() {
        return Err(McpError::InvalidToolName(
            "the tool name is empty".to_string(),
        ));
    }
    if name.len() > MAX_TOOL_NAME_LEN {
        return Err(McpError::InvalidToolName(format!(
            "the tool name is longer than {MAX_TOOL_NAME_LEN} bytes"
        )));
    }
    if name.chars().any(char::is_control) {
        return Err(McpError::InvalidToolName(format!(
            "the tool name {name:?} contains a control character"
        )));
    }
    Ok(())
}

fn redacted_url(url: &str) -> String {
    let Ok(mut parsed) = Url::parse(url) else {
        return "[invalid URL]".to_string();
    };
    if !parsed.username().is_empty() {
        let _ = parsed.set_username("redacted");
    }
    if parsed.password().is_some() {
        let _ = parsed.set_password(Some("redacted"));
    }
    if parsed.query().is_some() {
        parsed.set_query(Some("redacted"));
    }
    parsed.to_string()
}
