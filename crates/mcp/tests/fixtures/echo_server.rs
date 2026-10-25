//! Test-only stdio MCP server for `tests/stdio.rs`.
//!
//! Built only with `--features fixture-server`; it is not part of a normal
//! `myharness-mcp` build. It exposes one deterministic tool so the real
//! `rmcp` child-process path can be exercised without network access.

use rmcp::{
    handler::server::wrapper::Parameters, schemars, tool, tool_router, transport::stdio,
    ServiceExt,
};

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct EchoParams {
    /// Text to echo.
    text: String,
}

#[derive(Debug, Clone)]
struct EchoServer;

#[tool_router(server_handler)]
impl EchoServer {
    #[tool(description = "Echo the supplied text")]
    fn echo(&self, Parameters(EchoParams { text }): Parameters<EchoParams>) -> String {
        text
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = EchoServer.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
