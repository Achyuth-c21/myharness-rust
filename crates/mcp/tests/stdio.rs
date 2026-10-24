//! Real stdio lifecycle over `rmcp` (`TEST-MCP-001`, `TEST-MCP-009`).
//!
//! Gated behind `fixture-server` so a normal `cargo test -p myharness-mcp`
//! does not build a server binary. Run:
//!
//! ```text
//! cargo test -p myharness-mcp --features fixture-server
//! ```

#![cfg(feature = "fixture-server")]

use std::collections::BTreeMap;
use std::time::Duration;

use myharness_mcp::{McpManager, ServerConfig, ToolCallResult};
use serde_json::json;

fn stdio(command: &str) -> ServerConfig {
    ServerConfig::Stdio {
        command: command.to_string(),
        args: Vec::new(),
        env: BTreeMap::new(),
    }
}

#[tokio::test]
async fn stdio_child_starts_lists_tools_and_shuts_down() {
    let binary = env!("CARGO_BIN_EXE_mcp-echo-server");
    let mut manager = McpManager::new();
    manager.add_server("echo", stdio(binary)).unwrap();

    manager.start("echo").await.unwrap();
    let tools = manager.list_tools();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "echo:echo");
    assert_eq!(tools[0].tool, "echo");
    assert!(tools[0].description.as_deref().unwrap_or("").contains("Echo"));

    let statuses = manager.statuses();
    assert!(matches!(
        statuses[0].status,
        myharness_mcp::ServerStatus::Connected { tool_count: 1, .. }
    ));

    let snapshot = manager.snapshot_all();
    let result = manager
        .call_tool(
            &snapshot,
            "echo:echo",
            json!({"text": "hello from the child"}),
            Some(Duration::from_secs(5)),
        )
        .await
        .unwrap();
    assert_eq!(result, ToolCallResult::text("hello from the child"));

    let failures = manager.shutdown().await;
    assert!(failures.is_empty());
    assert!(manager.list_tools().is_empty());
}

#[tokio::test]
async fn a_broken_stdio_server_does_not_stop_the_healthy_one() {
    let binary = env!("CARGO_BIN_EXE_mcp-echo-server");
    let mut manager = McpManager::new();
    manager
        .add_server("broken", stdio("/definitely/not/a/real/mcp-server"))
        .unwrap();
    manager.add_server("echo", stdio(binary)).unwrap();

    let summary = manager.start_all().await;
    assert_eq!(summary.started, vec!["echo"]);
    assert_eq!(summary.failed.len(), 1);
    assert_eq!(summary.failed[0].server, "broken");
    assert!(summary.failed[0].message.contains("broken"));

    let names: Vec<String> = manager
        .list_tools()
        .into_iter()
        .map(|tool| tool.name)
        .collect();
    assert_eq!(names, vec!["echo:echo"]);

    manager.shutdown().await;
}
