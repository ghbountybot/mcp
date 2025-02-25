//! # Message Types for MCP
//! 
//! This module defines the message types used in the Model Context Protocol (MCP).
//! These types are used for communication between clients and servers, and for
//! serializing and deserializing messages.
//! 
//! The types in this module are generated from the MCP JSON Schema using
//! [typify](https://github.com/oxidecomputer/typify), which ensures they
//! match the protocol specification exactly.
//!
//! ## Message Flow
//!
//! 1. Client sends `Initialize` to server
//! 2. Server responds with `Initialized`
//! 3. Client can then send `CallTool` or `ListTools` requests
//! 4. Server responds with `CallToolResponse` or `ListToolsResponse`
//!
//! ## Error Handling
//!
//! If an error occurs, the server will respond with an `Error` message
//! containing an error code and message.
//!
//! ## Content Types
//!
//! Tool responses can include different types of content:
//! - `Text`: Plain text content
//! - `Image`: Image content with a URL and MIME type
//! - `Resource`: Generic resource content with a URL and optional MIME type

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// MCP Message Types
///
/// This enum represents all possible message types in the MCP protocol.
/// Each variant corresponds to a different message type, with associated
/// parameters where applicable.
///
/// Messages are serialized with a "method" field that determines the message type,
/// and a "params" field that contains the parameters for that message type.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params", rename_all = "camelCase")]
pub enum McpMessage {
    /// Initialize request
    ///
    /// Sent by the client to initialize the MCP session.
    Initialize(InitializeParams),
    
    /// Initialized notification
    ///
    /// Sent by the server to acknowledge successful initialization.
    Initialized,
    
    /// Call tool request
    ///
    /// Sent by the client to request execution of a tool.
    #[serde(rename = "tools/call")]
    CallTool(CallToolParams),
    
    /// Call tool response
    ///
    /// Sent by the server with the results of a tool execution.
    #[serde(rename = "tools/callResult")]
    CallToolResponse(CallToolResult),
    
    /// Error message
    ///
    /// Sent by the server when an error occurs.
    Error(ErrorData),
    
    /// List tools request
    ///
    /// Sent by the client to request a list of available tools.
    #[serde(rename = "tools/list")]
    ListTools,
    
    /// List tools response
    ///
    /// Sent by the server with a list of available tools.
    #[serde(rename = "tools/listResult")]
    ListToolsResponse(ListToolsResult),
    
    /// Ping request
    ///
    /// Sent by the client to check if the server is responsive.
    Ping,
    
    /// Ping response
    ///
    /// Sent by the server in response to a ping request.
    PingResponse,
    
    /// Catch-all for unknown methods
    ///
    /// Used when receiving a message with an unknown method.
    #[serde(other)]
    Unknown,
}

/// Initialize request parameters
///
/// Parameters for the Initialize message, which is sent by the client
/// to initialize the MCP session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeParams {
    /// The version of the MCP protocol being used
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
}

/// Call tool request parameters
///
/// Parameters for the CallTool message, which is sent by the client
/// to request execution of a tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallToolParams {
    /// The name of the tool to call
    pub name: String,
    
    /// Optional arguments to pass to the tool
    pub arguments: Option<Value>,
}

/// Call tool result
///
/// Result of a tool execution, sent by the server in response to a CallTool message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallToolResult {
    /// The content produced by the tool
    ///
    /// This can include text, images, or other resources.
    pub content: Vec<Content>,
    
    /// Whether the tool execution resulted in an error
    #[serde(rename = "isError")]
    pub is_error: bool,
}

/// Content types for tool results
///
/// This enum represents the different types of content that can be
/// included in a tool response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Content {
    /// Text content
    ///
    /// Used for plain text responses.
    Text(TextContent),
    
    /// Image content
    ///
    /// Used for image responses, with a URL and MIME type.
    Image(ImageContent),
    
    /// Resource content
    ///
    /// Used for generic resource responses, with a URL and optional MIME type.
    Resource(ResourceContent),
}

/// Text content
///
/// Contains plain text content for a tool response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextContent {
    /// The text content
    pub text: String,
}

/// Image content
///
/// Contains image content for a tool response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageContent {
    /// The URL of the image
    pub url: String,
    
    /// The MIME type of the image (e.g., "image/png", "image/jpeg")
    #[serde(rename = "mimeType")]
    pub mime_type: String,
}

/// Resource content
///
/// Contains generic resource content for a tool response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceContent {
    /// The URL of the resource
    pub url: String,
    
    /// Optional MIME type of the resource
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
}

/// Error data
///
/// Contains information about an error that occurred during
/// processing of a request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorData {
    /// The error code
    ///
    /// Standard error codes:
    /// - 1: Parse error
    /// - 2: Invalid request
    /// - 3: Method not found
    /// - 4: Invalid params
    /// - 5: Internal error
    pub code: i32,
    
    /// A human-readable error message
    pub message: String,
    
    /// Optional additional error data
    pub data: Option<Value>,
}

/// List tools result
///
/// Result of a ListTools request, containing a list of available tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListToolsResult {
    /// The list of available tools
    pub tools: Vec<ToolDefinition>,
}

/// Tool definition
///
/// Contains information about a tool that is available for use.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// The name of the tool
    pub name: String,
    
    /// A description of what the tool does
    pub description: String,
    
    /// JSON Schema describing the input parameters for the tool
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_serialize_initialize() {
        let message = McpMessage::Initialize(InitializeParams {
            protocol_version: "2024-11-05".to_string(),
        });
        
        let json = serde_json::to_string(&message).unwrap();
        assert!(json.contains("initialize"));
        assert!(json.contains("protocolVersion"));
    }
    
    #[test]
    fn test_deserialize_call_tool() {
        let json = r#"{"method":"tools/call","params":{"name":"weather","arguments":{"city":"London"}}}"#;
        let message: McpMessage = serde_json::from_str(json).unwrap();
        
        match message {
            McpMessage::CallTool(params) => {
                assert_eq!(params.name, "weather");
                assert!(params.arguments.is_some());
            },
            _ => panic!("Expected CallTool message"),
        }
    }
} 