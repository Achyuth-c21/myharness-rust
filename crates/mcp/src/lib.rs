//! MCP runtime — roadmap Phase 5 (`docs/13` §4 Phase 5; `docs/12` §5.1;
//! `ARCH-PROC-08`).
//!
//! This slice is the transport-independent lifecycle foundation:
//!
//! - [`ServerConfig`] models stdio, Streamable HTTP, and legacy HTTP+SSE
//!   definitions, with a `Debug` implementation that redacts credentials
//!   (`docs/10` §8; `docs/11` §3).
//! - [`McpManager`] starts servers, caches their tools, namespaces them as
//!   `<server>:<tool>`, snapshots the enabled set per session, and shuts down
//!   cleanly (`TEST-MCP-001`, `TEST-MCP-002`).
//! - A startup failure is recorded per server and never aborts the remaining
//!   servers (`TEST-MCP-009`); the caller can run [`McpManager::start_all`]
//!   away from the app boot path.
//!
//! Deliberate limits of this slice, stated rather than implied:
//!
//! - stdio and Streamable HTTP use `rmcp` 3.7. The legacy HTTP+SSE transport is
//!   modelled but returns [`McpError::UnsupportedTransport`]: the current SDK
//!   carries SSE inside Streamable HTTP (`docs/14` §9).
//! - Tool execution, approval/policy, OAuth, skills, plugins, and renderer
//!   commands are later Phase 5 slices. No UI claims them yet.
//! - Tool descriptions and schemas are untrusted input (`docs/12` §5.1). They
//!   are size-limited here and never interpreted as instructions.
//!
//! Trust boundary: MCP servers are separate trust domains (`docs/10` §7).
//! Their output crosses into the app only through the typed values in this
//! crate.

mod config;
mod egress;
mod error;
mod manager;
mod session;
mod tool;

pub use config::{
    MAX_ERROR_MESSAGE_LEN, MAX_SERVER_NAME_LEN, MAX_TOOLS_PER_SERVER, MAX_TOOL_NAME_LEN,
    MAX_TOOL_SCHEMA_BYTES, ServerConfig, validate_server_name, validate_tool_name,
};
pub use egress::{EgressError, EgressPolicy};
pub use error::{McpError, McpResult};
pub use manager::{
    McpManager, ServerStatus, ServerView, ShutdownFailure, StartFailure, StartSummary,
};
pub use session::{
    McpConnector, McpSession, RmcpConnector, ServerInfoSummary,
};
pub use tool::{
    ToolCallResult, ToolContent, ToolDefinition, ToolDescriptor, ToolSnapshot, namespaced_name,
    split_namespaced,
};
