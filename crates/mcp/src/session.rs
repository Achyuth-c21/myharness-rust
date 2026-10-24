//! Transport connectors and the typed session abstraction.
//!
//! The manager depends on [`McpSession`], not on `rmcp`, so lifecycle and
//! namespace tests use a scripted session while [`RmcpConnector`] provides the
//! real stdio and Streamable HTTP transports (`docs/13` Phase 5).

use std::collections::HashMap;

use async_trait::async_trait;
use http::{HeaderName, HeaderValue};
use rmcp::model::{
    CallToolRequestParams, ClientCapabilities, ClientConfig, ContentBlock, Implementation,
    ResourceContents,
};
use rmcp::service::RunningService;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::transport::{StreamableHttpClientTransport, TokioChildProcess};
use rmcp::{ClientHandler, RoleClient, ServiceExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::ServerConfig;
use crate::error::{McpError, McpResult};
use crate::tool::{ToolCallResult, ToolContent, ToolDefinition};

/// A redacted summary of the server's MCP identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfoSummary {
    /// Implementation name reported by the server.
    pub name: String,
    /// Optional implementation title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Implementation version.
    pub version: String,
    /// Negotiated MCP protocol version.
    pub protocol_version: String,
    /// Optional server instructions. Untrusted text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
}

impl ServerInfoSummary {
    /// The server did not report identity information.
    pub fn unknown() -> Self {
        Self {
            name: "unknown".to_string(),
            title: None,
            version: String::new(),
            protocol_version: String::new(),
            instructions: None,
        }
    }
}

/// A live MCP session.
///
/// Implementations own their transport. Dropping one without [`shutdown`]
/// still closes the SDK service, but callers should shut down explicitly so a
/// child process is reaped before the manager forgets it (`TEST-MCP-001`).
#[async_trait]
pub trait McpSession: Send + Sync {
    /// Redacted server identity captured during initialization.
    fn server_info(&self) -> ServerInfoSummary;

    /// Fetch the full tool catalog. Pagination is handled by the transport.
    async fn list_tools(&self) -> McpResult<Vec<ToolDefinition>>;

    /// Invoke one server-local tool with a JSON object of arguments.
    async fn call_tool(&self, tool: &str, arguments: Value) -> McpResult<ToolCallResult>;

    /// True when the transport has already closed.
    ///
    /// The manager uses this to reconnect instead of reporting a dead session
    /// as connected. Health probing beyond this flag is a later Phase 5 slice.
    fn is_closed(&self) -> bool;

    /// Close the transport and terminate a stdio child.
    async fn shutdown(&mut self) -> McpResult<()>;
}

/// Opens a transport for one configured server.
///
/// The server name is passed for diagnostics only; it never becomes part of
/// the wire handshake.
#[async_trait]
pub trait McpConnector: Send + Sync {
    /// Connect and initialize. A failure is returned to the manager, which
    /// records it without aborting other servers.
    async fn connect(&self, server: &str, config: &ServerConfig)
        -> McpResult<Box<dyn McpSession>>;
}

/// The production connector over `rmcp`.
#[derive(Clone, Copy, Debug, Default)]
pub struct RmcpConnector;

/// The client identity sent in the MCP initialize request.
#[derive(Clone, Copy, Debug, Default)]
pub struct MyHarnessClient;

impl ClientHandler for MyHarnessClient {
    fn get_info(&self) -> ClientConfig {
        ClientConfig::new(
            ClientCapabilities::default(),
            Implementation::new("myharness", env!("CARGO_PKG_VERSION")),
        )
    }
}

/// A real session backed by an `rmcp` running service.
pub struct RmcpSession {
    configured_name: String,
    service: Option<RunningService<RoleClient, MyHarnessClient>>,
    info: ServerInfoSummary,
}

impl RmcpSession {
    fn new(
        configured_name: String,
        service: RunningService<RoleClient, MyHarnessClient>,
    ) -> Self {
        let info = service
            .peer_info()
            .map(|peer| {
                let implementation = peer.server_info.as_ref();
                ServerInfoSummary {
                    name: implementation
                        .map(|implementation| implementation.name.clone())
                        .unwrap_or_else(|| "unknown".to_string()),
                    title: implementation.and_then(|implementation| implementation.title.clone()),
                    version: implementation
                        .map(|implementation| implementation.version.clone())
                        .unwrap_or_default(),
                    protocol_version: peer.protocol_version.to_string(),
                    instructions: peer.instructions.clone(),
                }
            })
            .unwrap_or_else(ServerInfoSummary::unknown);
        Self {
            configured_name,
            service: Some(service),
            info,
        }
    }
}

#[async_trait]
impl McpSession for RmcpSession {
    fn server_info(&self) -> ServerInfoSummary {
        self.info.clone()
    }

    async fn list_tools(&self) -> McpResult<Vec<ToolDefinition>> {
        let service = self.service.as_ref().ok_or_else(|| {
            McpError::Protocol("the MCP session is already closed".to_string())
        })?;
        let tools = service
            .list_all_tools()
            .await
            .map_err(|error| McpError::Protocol(error.to_string()))?;
        let mut definitions = Vec::with_capacity(tools.len());
        for tool in tools {
            let input_schema = serde_json::to_value(tool.input_schema.as_ref())
                .map_err(|error| McpError::Protocol(error.to_string()))?;
            let output_schema = match tool.output_schema.as_ref() {
                Some(schema) => Some(
                    serde_json::to_value(schema.as_ref())
                        .map_err(|error| McpError::Protocol(error.to_string()))?,
                ),
                None => None,
            };
            definitions.push(ToolDefinition {
                name: tool.name.into_owned(),
                title: tool.title,
                description: tool.description.map(|description| description.into_owned()),
                input_schema,
                output_schema,
            });
        }
        Ok(definitions)
    }

    async fn call_tool(&self, tool: &str, arguments: Value) -> McpResult<ToolCallResult> {
        let service = self.service.as_ref().ok_or_else(|| {
            McpError::Protocol("the MCP session is already closed".to_string())
        })?;
        let arguments = match arguments {
            Value::Null => serde_json::Map::new(),
            Value::Object(arguments) => arguments,
            _ => {
                return Err(McpError::Protocol(
                    "MCP tool arguments must be a JSON object".to_string(),
                ));
            }
        };
        let result = service
            .call_tool(
                CallToolRequestParams::new(tool.to_string()).with_arguments(arguments),
            )
            .await
            .map_err(|error| McpError::Protocol(error.to_string()))?;
        let content = result
            .content
            .into_iter()
            .map(|block| map_content(tool, block))
            .collect::<McpResult<Vec<_>>>()?;
        Ok(ToolCallResult {
            content,
            structured_content: result.structured_content,
            is_error: result.is_error.unwrap_or(false),
        })
    }

    fn is_closed(&self) -> bool {
        self.service
            .as_ref()
            .map(|service| service.is_closed())
            .unwrap_or(true)
    }

    async fn shutdown(&mut self) -> McpResult<()> {
        let Some(service) = self.service.take() else {
            return Ok(());
        };
        service.cancel().await.map_err(|error| McpError::Shutdown {
            server: self.configured_name.clone(),
            message: error.to_string(),
        })?;
        Ok(())
    }
}

fn map_content(tool: &str, block: ContentBlock) -> McpResult<ToolContent> {
    match block {
        ContentBlock::Text(text) => Ok(ToolContent::Text { text: text.text }),
        ContentBlock::Image(image) => Ok(ToolContent::Image {
            data: image.data,
            mime_type: image.mime_type,
        }),
        ContentBlock::Audio(audio) => Ok(ToolContent::Audio {
            data: audio.data,
            mime_type: audio.mime_type,
        }),
        ContentBlock::Resource(embedded) => match embedded.resource {
            ResourceContents::TextResourceContents {
                uri,
                mime_type,
                text,
                ..
            } => Ok(ToolContent::Resource {
                uri,
                mime_type,
                text: Some(text),
                blob: None,
            }),
            ResourceContents::BlobResourceContents {
                uri,
                mime_type,
                blob,
                ..
            } => Ok(ToolContent::Resource {
                uri,
                mime_type,
                text: None,
                blob: Some(blob),
            }),
            _ => Err(McpError::UnsupportedContent {
                name: tool.to_string(),
                kind: "resource".to_string(),
            }),
        },
        ContentBlock::ResourceLink(link) => Ok(ToolContent::ResourceLink {
            uri: link.uri,
            name: link.name,
            mime_type: link.mime_type,
        }),
        _ => Err(McpError::UnsupportedContent {
            name: tool.to_string(),
            kind: "content block".to_string(),
        }),
    }
}

#[async_trait]
impl McpConnector for RmcpConnector {
    async fn connect(
        &self,
        server: &str,
        config: &ServerConfig,
    ) -> McpResult<Box<dyn McpSession>> {
        match config {
            ServerConfig::Stdio {
                command,
                args,
                env,
            } => {
                let mut child = tokio::process::Command::new(command);
                child.args(args);
                for (key, value) in env {
                    child.env(key, value);
                }
                let transport = TokioChildProcess::new(child).map_err(|error| {
                    McpError::Protocol(format!("the stdio server could not be spawned: {error}"))
                })?;
                let service = MyHarnessClient.serve(transport).await.map_err(|error| {
                    McpError::Protocol(format!("the stdio server did not initialize: {error}"))
                })?;
                Ok(Box::new(RmcpSession::new(server.to_string(), service)))
            }
            ServerConfig::Http { url, headers } => {
                let custom_headers = header_map(headers)?;
                let mut transport_config =
                    StreamableHttpClientTransportConfig::with_uri(url.clone());
                transport_config.custom_headers = custom_headers;
                let transport = StreamableHttpClientTransport::from_config(transport_config);
                let service = MyHarnessClient.serve(transport).await.map_err(|error| {
                    McpError::Protocol(format!(
                        "the Streamable HTTP server did not initialize: {error}"
                    ))
                })?;
                Ok(Box::new(RmcpSession::new(server.to_string(), service)))
            }
            ServerConfig::Sse { .. } => Err(McpError::UnsupportedTransport(
                "legacy HTTP+SSE; use the Streamable HTTP transport",
            )),
        }
    }
}

fn header_map(
    headers: &std::collections::BTreeMap<String, String>,
) -> McpResult<HashMap<HeaderName, HeaderValue>> {
    headers
        .iter()
        .map(|(name, value)| {
            let name = HeaderName::from_bytes(name.as_bytes()).map_err(|error| {
                McpError::InvalidConfig(format!("invalid header name {name:?}: {error}"))
            })?;
            let value = HeaderValue::from_str(value).map_err(|error| {
                McpError::InvalidConfig(format!("invalid header value: {error}"))
            })?;
            Ok((name, value))
        })
        .collect()
}
