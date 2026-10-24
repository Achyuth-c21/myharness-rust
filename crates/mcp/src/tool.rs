//! Tool names, descriptors, and per-session snapshots.
//!
//! `docs/12` §5.1: tool names are namespaced as `<server>:<tool>` in the
//! session enablement map, and the enabled set is snapshotted per session so a
//! later server refresh cannot silently change what a running turn may call
//! (`TEST-MCP-002`).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::{validate_server_name, validate_tool_name};
use crate::error::{McpError, McpResult};

/// One tool as reported by an MCP server, before namespacing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDefinition {
    /// Server-local tool name.
    pub name: String,
    /// Optional human-readable title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Optional description. Untrusted text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// JSON Schema for the arguments. Untrusted input.
    pub input_schema: Value,
    /// Optional JSON Schema for the result. Untrusted input.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<Value>,
}

/// One namespaced tool in the manager's cache.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDescriptor {
    /// `<server>:<tool>`.
    pub name: String,
    /// Configured server name.
    pub server: String,
    /// Server-local tool name.
    pub tool: String,
    /// Optional human-readable title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Optional description. Untrusted text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// JSON Schema for the arguments. Untrusted input.
    pub input_schema: Value,
    /// Optional JSON Schema for the result. Untrusted input.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<Value>,
}

impl ToolDescriptor {
    /// Namespace a server-local definition.
    pub fn from_definition(server: &str, definition: ToolDefinition) -> McpResult<Self> {
        let name = namespaced_name(server, &definition.name)?;
        Ok(Self {
            name,
            server: server.to_string(),
            tool: definition.name,
            title: definition.title,
            description: definition.description,
            input_schema: definition.input_schema,
            output_schema: definition.output_schema,
        })
    }
}

/// A frozen view of the tools a session may use.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolSnapshot {
    /// Enabled tools, sorted by namespaced name.
    pub tools: Vec<ToolDescriptor>,
    /// Names that were requested but are not available from a connected
    /// server. They stay visible rather than being silently dropped.
    pub missing: Vec<String>,
}

impl ToolSnapshot {
    /// Namespaced names in snapshot order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.tools.iter().map(|tool| tool.name.as_str())
    }

    /// True when the frozen snapshot contains this exact namespaced name.
    pub fn contains(&self, name: &str) -> bool {
        self.tools.iter().any(|tool| tool.name == name)
    }
}

/// One content block returned by a tool.
///
/// Text, images, audio, and embedded resources are preserved rather than
/// flattened; `docs/13` Phase 5 requires tool-result images to survive.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ToolContent {
    /// Plain text.
    Text {
        /// The text.
        text: String,
    },
    /// Base64 image data.
    Image {
        /// Base64 data.
        data: String,
        /// MIME type.
        mime_type: String,
    },
    /// Base64 audio data.
    Audio {
        /// Base64 data.
        data: String,
        /// MIME type.
        mime_type: String,
    },
    /// An embedded text or blob resource.
    Resource {
        /// Resource URI.
        uri: String,
        /// Optional MIME type.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mime_type: Option<String>,
        /// Text body, when the resource is text.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
        /// Base64 body, when the resource is a blob.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        blob: Option<String>,
    },
    /// A link to a resource the server did not inline.
    ResourceLink {
        /// Resource URI.
        uri: String,
        /// Programmatic name.
        name: String,
        /// Optional MIME type.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mime_type: Option<String>,
    },
}

/// A tool result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallResult {
    /// Content blocks in server order.
    pub content: Vec<ToolContent>,
    /// Optional structured result.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured_content: Option<Value>,
    /// True when the server marked the result as an error.
    pub is_error: bool,
}

impl ToolCallResult {
    /// A text-only result, used by tests and the stdio fixture.
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent::Text { text: text.into() }],
            structured_content: None,
            is_error: false,
        }
    }
}

/// Build `<server>:<tool>` after validating both components.
pub fn namespaced_name(server: &str, tool: &str) -> McpResult<String> {
    validate_server_name(server)?;
    validate_tool_name(tool)?;
    Ok(format!("{server}:{tool}"))
}

/// Split a namespaced name on the first colon.
///
/// The server component may not contain `:`; a tool name may, and the rest of
/// the string is preserved as the tool name.
pub fn split_namespaced(name: &str) -> McpResult<(&str, &str)> {
    let (server, tool) = name
        .split_once(':')
        .ok_or_else(|| McpError::InvalidToolName(name.to_string()))?;
    validate_server_name(server)?;
    validate_tool_name(tool)?;
    Ok((server, tool))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namespacing_keeps_a_colon_in_the_tool_name() {
        assert_eq!(namespaced_name("github", "issues:read").unwrap(), "github:issues:read");
        assert_eq!(
            split_namespaced("github:issues:read").unwrap(),
            ("github", "issues:read")
        );
    }

    #[test]
    fn a_name_without_a_separator_is_invalid() {
        assert!(matches!(
            split_namespaced("github"),
            Err(McpError::InvalidToolName(_))
        ));
    }
}
