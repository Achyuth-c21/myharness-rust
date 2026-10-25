//! Typed MCP errors.
//!
//! Messages are redacted by the manager before they reach a status or a
//! renderer (`docs/18` §11: no body text, no secret material). A connector may
//! still return an error containing server output; [`crate::McpManager`]
//! applies the owning [`crate::ServerConfig::redact`] before storing it.

use thiserror::Error;

/// Result alias for the MCP crate.
pub type McpResult<T> = Result<T, McpError>;

/// One MCP failure.
#[derive(Debug, Error)]
pub enum McpError {
    /// The manager has no server with this configured name.
    #[error("unknown MCP server: {0}")]
    UnknownServer(String),

    /// A server with this configured name already exists.
    #[error("duplicate MCP server: {0}")]
    DuplicateServer(String),

    /// The configured server name is not a safe namespace component.
    #[error("invalid MCP server name: {0}")]
    InvalidServerName(String),

    /// The transport configuration is structurally invalid.
    #[error("invalid MCP server configuration: {0}")]
    InvalidConfig(String),

    /// The transport is modelled but not implemented in this build.
    #[error("MCP transport is not supported in this build: {0}")]
    UnsupportedTransport(&'static str),

    /// A server failed to start, list tools, or validate its tool catalog.
    #[error("MCP server {server} failed: {message}")]
    Server {
        /// Configured server name.
        server: String,
        /// Redacted, bounded message.
        message: String,
    },

    /// The server needs OAuth authorization the app does not have.
    #[error("MCP server {server} needs authorization: {message}")]
    NeedsAuth {
        /// Configured server name.
        server: String,
        /// Redacted, bounded message.
        message: String,
    },

    /// The SDK reported a protocol or transport failure.
    #[error("MCP protocol error: {0}")]
    Protocol(String),

    /// A tool name is not a safe namespace component.
    #[error("MCP tool name is invalid: {0}")]
    InvalidToolName(String),

    /// The tool is not present in the session's frozen snapshot.
    #[error("MCP tool {name} is not in the session snapshot")]
    ToolNotInSnapshot {
        /// Namespaced tool name.
        name: String,
    },

    /// A tool call exceeded the configured timeout.
    #[error("MCP tool {name} timed out after {millis} ms")]
    ToolTimeout {
        /// Namespaced tool name.
        name: String,
        /// Timeout in milliseconds.
        millis: u64,
    },

    /// The tool result contains a content type this slice cannot preserve.
    #[error("MCP tool {name} returned unsupported content: {kind}")]
    UnsupportedContent {
        /// Namespaced tool name.
        name: String,
        /// Content kind reported by the server.
        kind: String,
    },

    /// A server did not shut down cleanly.
    #[error("MCP server {server} did not shut down cleanly: {message}")]
    Shutdown {
        /// Configured server name.
        server: String,
        /// Redacted, bounded message.
        message: String,
    },
}
