//! MCP lifecycle and namespace tests (`TEST-MCP-001`, `TEST-MCP-002`,
//! `TEST-MCP-009`).
//!
//! The scripted connector isolates manager behavior from process/network
//! flakiness; the real stdio path is exercised separately in `stdio.rs`.

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use myharness_mcp::{
    EgressPolicy, MAX_TOOL_SCHEMA_BYTES, McpConnector, McpError, McpManager, McpResult,
    McpSession, ServerConfig, ServerInfoSummary, ToolCallResult, ToolContent, ToolDefinition,
    namespaced_name,
};
use serde_json::{Value, json};

#[derive(Clone)]
enum MockResult {
    Connected {
        tools: Vec<ToolDefinition>,
        info: ServerInfoSummary,
    },
    Failed(String),
}

struct MockConnector {
    results: Mutex<HashMap<String, MockResult>>,
    shutdowns: Arc<AtomicUsize>,
    connects: Arc<AtomicUsize>,
    calls: Arc<AtomicUsize>,
    call_result: Mutex<ToolCallResult>,
    call_delay: Mutex<Option<Duration>>,
    connect_delays: Mutex<HashMap<String, Duration>>,
}

impl Default for MockConnector {
    fn default() -> Self {
        Self {
            results: Mutex::new(HashMap::new()),
            shutdowns: Arc::new(AtomicUsize::new(0)),
            connects: Arc::new(AtomicUsize::new(0)),
            calls: Arc::new(AtomicUsize::new(0)),
            call_result: Mutex::new(ToolCallResult::text("ok")),
            call_delay: Mutex::new(None),
            connect_delays: Mutex::new(HashMap::new()),
        }
    }
}

impl MockConnector {
    fn add(&self, server: &str, result: MockResult) {
        self.results
            .lock()
            .expect("mock results lock")
            .insert(server.to_string(), result);
    }
}

struct MockSession {
    tools: Vec<ToolDefinition>,
    info: ServerInfoSummary,
    shutdowns: Arc<AtomicUsize>,
    calls: Arc<AtomicUsize>,
    call_result: ToolCallResult,
    call_delay: Option<Duration>,
}

#[async_trait]
impl McpConnector for MockConnector {
    async fn connect(
        &self,
        server: &str,
        _config: &ServerConfig,
    ) -> McpResult<Box<dyn McpSession>> {
        self.connects.fetch_add(1, Ordering::SeqCst);
        let delay = self
            .connect_delays
            .lock()
            .expect("connect delays lock")
            .get(server)
            .copied();
        if let Some(delay) = delay {
            tokio::time::sleep(delay).await;
        }
        let result = self
            .results
            .lock()
            .expect("mock results lock")
            .remove(server);
        match result {
            Some(MockResult::Connected { tools, info }) => Ok(Box::new(MockSession {
                tools,
                info,
                shutdowns: Arc::clone(&self.shutdowns),
                calls: Arc::clone(&self.calls),
                call_result: self.call_result.lock().expect("call result lock").clone(),
                call_delay: *self.call_delay.lock().expect("call delay lock"),
            })),
            Some(MockResult::Failed(message)) => Err(McpError::Protocol(message)),
            None => Err(McpError::Protocol(format!(
                "no scripted result for {server}"
            ))),
        }
    }
}

#[async_trait]
impl McpSession for MockSession {
    fn server_info(&self) -> ServerInfoSummary {
        self.info.clone()
    }

    async fn list_tools(&self) -> McpResult<Vec<ToolDefinition>> {
        Ok(self.tools.clone())
    }

    async fn call_tool(&self, _tool: &str, _arguments: Value) -> McpResult<ToolCallResult> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if let Some(delay) = self.call_delay {
            tokio::time::sleep(delay).await;
        }
        Ok(self.call_result.clone())
    }

    fn is_closed(&self) -> bool {
        false
    }

    async fn shutdown(&mut self) -> McpResult<()> {
        self.shutdowns.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

fn stdio_config() -> ServerConfig {
    ServerConfig::Stdio {
        command: "mock-server".to_string(),
        args: Vec::new(),
        env: BTreeMap::new(),
    }
}

fn http_config(url: &str) -> ServerConfig {
    ServerConfig::Http {
        url: url.to_string(),
        headers: BTreeMap::new(),
    }
}

fn info(name: &str) -> ServerInfoSummary {
    ServerInfoSummary {
        name: name.to_string(),
        title: None,
        version: "1.0.0".to_string(),
        protocol_version: "2026-07-28".to_string(),
        instructions: None,
    }
}

fn tool(name: &str) -> ToolDefinition {
    ToolDefinition {
        name: name.to_string(),
        title: None,
        description: Some(format!("The {name} tool")),
        input_schema: json!({"type": "object", "properties": {}}),
        output_schema: None,
    }
}

fn connected(tools: Vec<ToolDefinition>) -> MockResult {
    MockResult::Connected {
        tools,
        info: info("mock"),
    }
}

fn manager() -> (McpManager, Arc<MockConnector>) {
    let connector = Arc::new(MockConnector::default());
    let manager = McpManager::with_connector(connector.clone());
    (manager, connector)
}

#[test]
fn duplicate_and_invalid_server_names_are_refused() {
    let (mut manager, _) = manager();
    manager.add_server("alpha", stdio_config()).unwrap();
    assert!(matches!(
        manager.add_server("alpha", stdio_config()),
        Err(McpError::DuplicateServer(_))
    ));
    assert!(matches!(
        manager.add_server("bad:name", stdio_config()),
        Err(McpError::InvalidServerName(_))
    ));
    assert!(matches!(
        manager.add_server("", stdio_config()),
        Err(McpError::InvalidServerName(_))
    ));
}

#[tokio::test]
async fn start_namespaces_and_sorts_tools() {
    let (mut manager, connector) = manager();
    manager.add_server("alpha", stdio_config()).unwrap();
    manager.add_server("beta", stdio_config()).unwrap();
    connector.add("alpha", connected(vec![tool("z"), tool("a")]));
    connector.add("beta", connected(vec![tool("m")]));

    manager.start("alpha").await.unwrap();
    manager.start("beta").await.unwrap();

    let names: Vec<String> = manager
        .list_tools()
        .into_iter()
        .map(|tool| tool.name)
        .collect();
    assert_eq!(names, vec!["alpha:a", "alpha:z", "beta:m"]);
    assert_eq!(namespaced_name("alpha", "a").unwrap(), "alpha:a");

    let alpha = manager
        .statuses()
        .into_iter()
        .find(|view| view.name == "alpha")
        .unwrap();
    assert!(matches!(
        alpha.status,
        myharness_mcp::ServerStatus::Connected { tool_count: 2, .. }
    ));
}

#[tokio::test]
async fn start_all_records_failure_and_continues() {
    let (mut manager, connector) = manager();
    let mut beta_config = stdio_config();
    if let ServerConfig::Stdio { env, .. } = &mut beta_config {
        env.insert("BETA_TOKEN".to_string(), "super-secret-token".to_string());
    }
    manager.add_server("alpha", stdio_config()).unwrap();
    manager.add_server("beta", beta_config).unwrap();
    connector.add("alpha", connected(vec![tool("a")]));
    connector.add(
        "beta",
        MockResult::Failed("the server rejected super-secret-token".to_string()),
    );

    let summary = manager.start_all().await;
    assert_eq!(summary.started, vec!["alpha"]);
    assert_eq!(summary.failed.len(), 1);
    assert!(summary.failed[0].message.contains("[redacted]"));
    assert!(!summary.failed[0].message.contains("super-secret-token"));
    assert!(!summary.all_started());

    let beta = manager
        .statuses()
        .into_iter()
        .find(|view| view.name == "beta")
        .unwrap();
    let myharness_mcp::ServerStatus::Failed { message } = beta.status else {
        panic!("beta should be failed");
    };
    assert!(message.contains("[redacted]"));
    assert!(!message.contains("super-secret-token"));

    let names: Vec<String> = manager
        .list_tools()
        .into_iter()
        .map(|tool| tool.name)
        .collect();
    assert_eq!(names, vec!["alpha:a"]);
}

#[tokio::test]
async fn snapshot_is_stable_and_reports_missing_tools() {
    let (mut manager, connector) = manager();
    manager.add_server("alpha", stdio_config()).unwrap();
    connector.add("alpha", connected(vec![tool("z"), tool("a")]));
    manager.start("alpha").await.unwrap();

    let snapshot = manager.snapshot(&[
        "alpha:z".to_string(),
        "alpha:a".to_string(),
        "missing".to_string(),
        "alpha:a".to_string(),
    ]);
    assert_eq!(
        snapshot.names().collect::<Vec<_>>(),
        vec!["alpha:a", "alpha:z"]
    );
    assert_eq!(snapshot.missing, vec!["missing"]);

    let empty = manager.snapshot(&[]);
    assert!(empty.tools.is_empty());
    assert!(empty.missing.is_empty());
}

#[tokio::test]
async fn shutdown_stops_every_session_once() {
    let (mut manager, connector) = manager();
    manager.add_server("alpha", stdio_config()).unwrap();
    connector.add("alpha", connected(vec![tool("a")]));
    manager.start("alpha").await.unwrap();

    let failures = manager.shutdown().await;
    assert!(failures.is_empty());
    assert_eq!(connector.shutdowns.load(Ordering::SeqCst), 1);
    assert!(manager.list_tools().is_empty());
    assert!(matches!(
        manager.statuses()[0].status,
        myharness_mcp::ServerStatus::Stopped
    ));

    let failures = manager.shutdown().await;
    assert!(failures.is_empty());
    assert_eq!(connector.shutdowns.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn invalid_tool_name_fails_the_server_and_shuts_it_down() {
    let (mut manager, connector) = manager();
    manager.add_server("alpha", stdio_config()).unwrap();
    connector.add("alpha", connected(vec![tool("bad\nname")]));

    let error = manager.start("alpha").await.unwrap_err();
    assert!(error.to_string().contains("tool name"));
    assert_eq!(connector.shutdowns.load(Ordering::SeqCst), 1);
    assert!(matches!(
        manager.statuses()[0].status,
        myharness_mcp::ServerStatus::Failed { .. }
    ));
}

#[tokio::test]
async fn oversized_schema_fails_the_server() {
    let (mut manager, connector) = manager();
    manager.add_server("alpha", stdio_config()).unwrap();
    let mut definition = tool("huge");
    definition.input_schema = json!({
        "type": "object",
        "description": "x".repeat(MAX_TOOL_SCHEMA_BYTES + 1),
    });
    connector.add("alpha", connected(vec![definition]));

    let error = manager.start("alpha").await.unwrap_err();
    assert!(error.to_string().contains("input schema"));
    assert_eq!(connector.shutdowns.load(Ordering::SeqCst), 1);
}

#[test]
fn debug_and_redaction_never_expose_env_or_header_values() {
    let mut env = BTreeMap::new();
    env.insert("API_TOKEN".to_string(), "super-secret-token".to_string());
    let stdio = ServerConfig::Stdio {
        command: "mock-server".to_string(),
        args: vec!["--verbose".to_string()],
        env,
    };
    let debug = format!("{stdio:?}");
    assert!(debug.contains("API_TOKEN"));
    assert!(!debug.contains("super-secret-token"));
    assert_eq!(stdio.redact("boom super-secret-token"), "boom [redacted]");

    let mut headers = BTreeMap::new();
    headers.insert(
        "Authorization".to_string(),
        "Bearer another-secret".to_string(),
    );
    let http = ServerConfig::Http {
        url: "https://user:password@example.com/mcp?token=secret".to_string(),
        headers,
    };
    let debug = format!("{http:?}");
    assert!(debug.contains("Authorization"));
    assert!(!debug.contains("another-secret"));
    assert!(!debug.contains("password"));
    assert!(!debug.contains("secret"));
    assert_eq!(
        http.redact("failed at https://example.com/mcp?token=secret"),
        "failed at https://example.com/mcp?[redacted]"
    );
}

#[test]
fn http_configuration_is_validated_before_storage() {
    let (mut manager, _) = manager();
    assert!(matches!(
        manager.add_server(
            "bad-scheme",
            ServerConfig::Http {
                url: "ftp://example.com/mcp".to_string(),
                headers: BTreeMap::new(),
            }
        ),
        Err(McpError::InvalidConfig(_))
    ));

    let mut headers = BTreeMap::new();
    headers.insert("X-Test".to_string(), "bad\nvalue".to_string());
    assert!(matches!(
        manager.add_server(
            "bad-header",
            ServerConfig::Http {
                url: "https://example.com/mcp".to_string(),
                headers,
            }
        ),
        Err(McpError::InvalidConfig(_))
    ));
}

#[tokio::test]
async fn legacy_sse_is_explicitly_unsupported() {
    let mut manager = McpManager::new();
    manager
        .add_server(
            "legacy",
            ServerConfig::Sse {
                url: "https://example.com/sse".to_string(),
                headers: BTreeMap::new(),
            },
        )
        .unwrap();
    let error = manager.start("legacy").await.unwrap_err();
    assert!(error.to_string().contains("legacy HTTP+SSE"));
    assert!(matches!(
        manager.statuses()[0].status,
        myharness_mcp::ServerStatus::Failed { .. }
    ));
}

#[tokio::test]
async fn egress_denies_an_unapproved_http_host_before_connect() {
    let connector = Arc::new(MockConnector::default());
    connector.add("blocked", connected(vec![tool("a")]));
    let mut manager = McpManager::with_connector(connector.clone())
        .with_egress(EgressPolicy::allow_hosts(["allowed.example"]));
    manager
        .add_server("blocked", http_config("https://blocked.example/mcp"))
        .unwrap();

    let error = manager.start("blocked").await.unwrap_err();
    assert!(error.to_string().contains("egress allowlist"));
    assert_eq!(connector.connects.load(Ordering::SeqCst), 0);
    assert!(matches!(
        manager.statuses()[0].status,
        myharness_mcp::ServerStatus::Failed { .. }
    ));
}

#[tokio::test]
async fn egress_allows_an_approved_http_host() {
    let connector = Arc::new(MockConnector::default());
    connector.add("allowed", connected(vec![tool("a")]));
    let mut manager = McpManager::with_connector(connector.clone())
        .with_egress(EgressPolicy::allow_hosts(["allowed.example"]));
    manager
        .add_server("allowed", http_config("https://allowed.example/mcp"))
        .unwrap();

    manager.start("allowed").await.unwrap();
    assert_eq!(connector.connects.load(Ordering::SeqCst), 1);
    assert_eq!(manager.list_tools()[0].name, "allowed:a");
}

#[tokio::test]
async fn egress_does_not_inspect_stdio() {
    let connector = Arc::new(MockConnector::default());
    connector.add("local", connected(vec![tool("a")]));
    let mut manager = McpManager::with_connector(connector.clone())
        .with_egress(EgressPolicy::deny_all());
    manager.add_server("local", stdio_config()).unwrap();

    manager.start("local").await.unwrap();
    assert_eq!(connector.connects.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn start_timeout_does_not_block_the_next_server() {
    let connector = Arc::new(MockConnector::default());
    connector
        .connect_delays
        .lock()
        .unwrap()
        .insert("slow".to_string(), Duration::from_millis(100));
    connector.add("fast", connected(vec![tool("b")]));
    connector.add("slow", connected(vec![tool("a")]));
    let mut manager = McpManager::with_connector(connector)
        .with_start_timeout(Some(Duration::from_millis(10)));
    manager.add_server("fast", stdio_config()).unwrap();
    manager.add_server("slow", stdio_config()).unwrap();

    let summary = manager.start_all().await;
    assert_eq!(summary.started, vec!["fast"]);
    assert_eq!(summary.failed.len(), 1);
    assert_eq!(summary.failed[0].server, "slow");
    assert!(summary.failed[0].message.contains("did not start within"));
    assert!(summary.failed[0].message.contains("10 ms"));
}

#[tokio::test]
async fn call_tool_is_gated_by_the_frozen_snapshot() {
    let (mut manager, connector) = manager();
    manager.add_server("alpha", stdio_config()).unwrap();
    connector.add("alpha", connected(vec![tool("a")]));
    manager.start("alpha").await.unwrap();
    let snapshot = manager.snapshot(&["alpha:a".to_string()]);

    let result = manager
        .call_tool(
            &snapshot,
            "alpha:a",
            json!({"value": 1}),
            Some(Duration::from_secs(1)),
        )
        .await
        .unwrap();
    assert_eq!(result, ToolCallResult::text("ok"));
    assert_eq!(connector.calls.load(Ordering::SeqCst), 1);

    let error = manager
        .call_tool(&snapshot, "alpha:b", json!({}), None)
        .await
        .unwrap_err();
    assert!(matches!(error, McpError::ToolNotInSnapshot { .. }));
    assert_eq!(connector.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn call_tool_preserves_images_and_structured_content() {
    let (mut manager, connector) = manager();
    manager.add_server("alpha", stdio_config()).unwrap();
    *connector.call_result.lock().unwrap() = ToolCallResult {
        content: vec![ToolContent::Image {
            data: "aGk=".to_string(),
            mime_type: "image/png".to_string(),
        }],
        structured_content: Some(json!({"ok": true})),
        is_error: false,
    };
    connector.add("alpha", connected(vec![tool("a")]));
    manager.start("alpha").await.unwrap();
    let snapshot = manager.snapshot_all();

    let result = manager
        .call_tool(&snapshot, "alpha:a", json!({}), None)
        .await
        .unwrap();
    assert_eq!(
        result.content,
        vec![ToolContent::Image {
            data: "aGk=".to_string(),
            mime_type: "image/png".to_string(),
        }]
    );
    assert_eq!(result.structured_content, Some(json!({"ok": true})));
    assert!(!result.is_error);
}

#[tokio::test]
async fn call_tool_timeout_is_reported() {
    let (mut manager, connector) = manager();
    manager.add_server("alpha", stdio_config()).unwrap();
    *connector.call_delay.lock().unwrap() = Some(Duration::from_millis(50));
    connector.add("alpha", connected(vec![tool("a")]));
    manager.start("alpha").await.unwrap();
    let snapshot = manager.snapshot_all();

    let error = manager
        .call_tool(
            &snapshot,
            "alpha:a",
            json!({}),
            Some(Duration::from_millis(1)),
        )
        .await
        .unwrap_err();
    assert!(matches!(error, McpError::ToolTimeout { .. }));
    assert_eq!(connector.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn unknown_and_removed_servers_are_reported() {
    let (mut manager, connector) = manager();
    assert!(matches!(
        manager.start("missing").await,
        Err(McpError::UnknownServer(_))
    ));

    manager.add_server("alpha", stdio_config()).unwrap();
    connector.add("alpha", connected(vec![tool("a")]));
    manager.start("alpha").await.unwrap();
    manager.remove_server("alpha").await.unwrap();
    assert_eq!(connector.shutdowns.load(Ordering::SeqCst), 1);
    assert_eq!(manager.server_names().count(), 0);
    assert!(matches!(
        manager.remove_server("alpha").await,
        Err(McpError::UnknownServer(_))
    ));
}
