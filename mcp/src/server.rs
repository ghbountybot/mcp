//! # MCP Server Implementation
//! 
//! This module provides a server implementation for the Model Context Protocol (MCP).
//! The server allows AI models to expose tools that can be called by clients, enabling
//! models to interact with external systems and perform actions.
//!
//! ## Overview
//!
//! The MCP server consists of:
//!
//! 1. A configurable HTTP server that handles MCP messages
//! 2. A tool registry for managing available tools
//! 3. Server-Sent Events (SSE) for real-time notifications
//! 4. Message handlers for processing client requests
//!
//! ## Server Architecture
//!
//! The server follows a simple architecture:
//!
//! 1. Clients connect to the server via HTTP
//! 2. Clients initialize a connection with the server
//! 3. Clients can list available tools and call them
//! 4. The server processes tool calls and returns results
//! 5. All responses are also broadcast via SSE for real-time updates
//!
//! ## Example Usage
//!
//! ```rust,no_run
//! use mcp::{
//!     server::{McpServer, ServerConfig},
//!     tool::{Tool, ToolRegistry, text_content},
//!     message::CallToolResult,
//! };
//! use async_trait::async_trait;
//! use eyre::Result;
//! use serde_json::Value;
//!
//! // Define a simple tool
//! struct GreetingTool;
//!
//! #[async_trait]
//! impl Tool for GreetingTool {
//!     fn name(&self) -> &str {
//!         "greeting"
//!     }
//!     
//!     fn description(&self) -> &str {
//!         "Generate a greeting message"
//!     }
//!     
//!     fn input_schema(&self) -> Value {
//!         serde_json::json!({
//!             "type": "object",
//!             "properties": {
//!                 "name": {
//!                     "type": "string",
//!                     "description": "The name to greet"
//!                 }
//!             }
//!         })
//!     }
//!     
//!     async fn call(&self, args: Value) -> Result<CallToolResult> {
//!         let name = args.get("name")
//!             .and_then(|v| v.as_str())
//!             .unwrap_or("World");
//!             
//!         Ok(CallToolResult {
//!             content: vec![text_content(format!("Hello, {}!", name))],
//!             is_error: false,
//!         })
//!     }
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     // Create a tool registry and register our tool
//!     let mut registry = ToolRegistry::new();
//!     registry.register(GreetingTool);
//!     
//!     // Create a server configuration
//!     let config = ServerConfig {
//!         name: "greeting-server".to_string(),
//!         version: "0.1.0".to_string(),
//!         protocol_version: "2024-11-05".to_string(),
//!         host: "127.0.0.1".to_string(),
//!         port: 3000,
//!     };
//!     
//!     // Create and start the server
//!     let server = McpServer::new(config, registry);
//!     server.start().await?;
//!     
//!     Ok(())
//! }
//! ```

use std::sync::Arc;
use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use eyre::{Result, WrapErr};
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;
use tracing::{debug, error, info};

use crate::message::{
    CallToolParams, ErrorData, InitializeParams, ListToolsResult, McpMessage,
};
use crate::tool::ToolRegistry;
use crate::transport::SseTransport;

/// Server configuration
///
/// This struct contains the configuration parameters for an MCP server,
/// including the server name, version, protocol version, host, and port.
///
/// # Examples
///
/// ```rust
/// use mcp::server::ServerConfig;
///
/// // Create a configuration with custom values
/// let config = ServerConfig {
///     name: "my-server".to_string(),
///     version: "0.1.0".to_string(),
///     protocol_version: "2024-11-05".to_string(),
///     host: "127.0.0.1".to_string(),
///     port: 3000,
/// };
///
/// // Or use the default configuration
/// let default_config = ServerConfig::default();
/// ```
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Server name
    ///
    /// A human-readable name for the server.
    pub name: String,
    
    /// Server version
    ///
    /// The version of the server software.
    pub version: String,
    
    /// Protocol version
    ///
    /// The version of the MCP protocol that the server supports.
    pub protocol_version: String,
    
    /// Host address to bind to
    ///
    /// The IP address or hostname that the server will bind to.
    pub host: String,
    
    /// Port to listen on
    ///
    /// The TCP port that the server will listen on.
    pub port: u16,
}

impl Default for ServerConfig {
    /// Creates a default server configuration
    ///
    /// The default configuration uses:
    /// - Name: "mcp-server"
    /// - Version: The crate version
    /// - Protocol version: "2024-11-05"
    /// - Host: "127.0.0.1"
    /// - Port: 3000
    fn default() -> Self {
        Self {
            name: "mcp-server".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            protocol_version: "2024-11-05".to_string(),
            host: "127.0.0.1".to_string(),
            port: 3000,
        }
    }
}

/// MCP Server
///
/// This struct provides the core implementation of an MCP server.
/// It manages:
/// - A tool registry for available tools
/// - A server configuration
/// - A transport layer for real-time communication
///
/// # Examples
///
/// ```rust,no_run
/// use mcp::{
///     server::{McpServer, ServerConfig},
///     tool::ToolRegistry,
/// };
/// use eyre::Result;
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     // Create a tool registry
///     let registry = ToolRegistry::new();
///     
///     // Create a server configuration
///     let config = ServerConfig::default();
///     
///     // Create and start the server
///     let server = McpServer::new(config, registry);
///     server.start().await?;
///     
///     Ok(())
/// }
/// ```
pub struct McpServer {
    config: ServerConfig,
    registry: Arc<Mutex<ToolRegistry>>,
    transport: SseTransport,
}

impl McpServer {
    /// Create a new MCP server with the given configuration and tool registry
    ///
    /// # Arguments
    ///
    /// * `config` - The server configuration
    /// * `registry` - The tool registry containing available tools
    ///
    /// # Returns
    ///
    /// A new `McpServer` instance
    ///
    /// # Examples
    ///
    /// ```rust
    /// use mcp::{
    ///     server::{McpServer, ServerConfig},
    ///     tool::ToolRegistry,
    /// };
    ///
    /// let config = ServerConfig::default();
    /// let registry = ToolRegistry::new();
    /// let server = McpServer::new(config, registry);
    /// ```
    pub fn new(config: ServerConfig, registry: ToolRegistry) -> Self {
        Self {
            config,
            registry: Arc::new(Mutex::new(registry)),
            transport: SseTransport::new(100), // Channel capacity of 100 messages
        }
    }
    
    /// Start the server
    ///
    /// This method starts the HTTP server and begins listening for connections.
    /// It blocks until the server is shut down.
    ///
    /// # Returns
    ///
    /// * `Ok(())` if the server was started and shut down gracefully
    /// * `Err(...)` if there was an error starting or running the server
    ///
    /// # Errors
    ///
    /// This method can fail if:
    /// - The server address is invalid
    /// - The port is already in use
    /// - There is an error binding to the address
    /// - There is an error during server operation
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use mcp::{
    ///     server::{McpServer, ServerConfig},
    ///     tool::ToolRegistry,
    /// };
    /// use eyre::Result;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<()> {
    ///     let config = ServerConfig::default();
    ///     let registry = ToolRegistry::new();
    ///     let server = McpServer::new(config, registry);
    ///     
    ///     // Start the server (this will block until the server is shut down)
    ///     server.start().await?;
    ///     
    ///     Ok(())
    /// }
    /// ```
    pub async fn start(&self) -> Result<()> {
        let registry = self.registry.clone();
        let transport_sender = self.transport.sender();
        
        // Create the router with our routes
        let app = Router::new()
            .route("/api/message", post(Self::handle_message))
            .route("/api/events", get({
                let tx = transport_sender.clone();
                move || async move { 
                    let transport = SseTransport::new_with_sender(tx.clone());
                    transport.sse_handler() 
                }
            }))
            .layer(CorsLayer::permissive())
            .with_state(AppState {
                config: self.config.clone(),
                registry,
                transport: transport_sender,
            });
        
        // Build the server address
        let addr = format!("{}:{}", self.config.host, self.config.port)
            .parse::<std::net::SocketAddr>()
            .wrap_err("Failed to parse server address")?;
        
        info!("MCP server listening on {}", addr);
        
        // Start the server
        axum::serve(tokio::net::TcpListener::bind(addr).await?, app)
            .await
            .wrap_err("Server error")?;
        
        Ok(())
    }
    
    /// Handle an incoming message
    ///
    /// This method processes incoming MCP messages, dispatches them to the
    /// appropriate handler, and returns the response. It also broadcasts the
    /// response to all connected SSE clients.
    ///
    /// # Arguments
    ///
    /// * `state` - The application state
    /// * `message` - The incoming MCP message
    ///
    /// # Returns
    ///
    /// The MCP response message
    async fn handle_message(
        State(state): State<AppState>,
        Json(message): Json<McpMessage>,
    ) -> Json<McpMessage> {
        debug!("Received message: {:?}", message);
        
        let response = match message {
            McpMessage::Initialize(params) => Self::handle_initialize(&state, params).await,
            McpMessage::CallTool(params) => Self::handle_call_tool(&state, params).await,
            McpMessage::ListTools => Self::handle_list_tools(&state).await,
            McpMessage::Ping => McpMessage::PingResponse,
            _ => {
                error!("Unsupported message type: {:?}", message);
                McpMessage::Error(ErrorData {
                    code: -32601,
                    message: "Method not supported".to_string(),
                    data: None,
                })
            }
        };
        
        // Broadcast the response to all SSE clients
        if let Err(e) = state.transport.send(response.clone()) {
            error!("Failed to broadcast response: {}", e);
        }
        
        Json(response)
    }
    
    /// Handle an initialize request
    ///
    /// This method processes an initialize request from a client.
    /// It checks if the protocol version is supported and returns
    /// an appropriate response.
    ///
    /// # Arguments
    ///
    /// * `state` - The application state
    /// * `params` - The initialize parameters
    ///
    /// # Returns
    ///
    /// An MCP message indicating success or failure
    async fn handle_initialize(state: &AppState, params: InitializeParams) -> McpMessage {
        info!("Initializing with protocol version: {}", params.protocol_version);
        
        // Check if the protocol version is supported
        if params.protocol_version != state.config.protocol_version {
            return McpMessage::Error(ErrorData {
                code: -32000,
                message: format!(
                    "Unsupported protocol version: {}. Server supports: {}",
                    params.protocol_version, state.config.protocol_version
                ),
                data: None,
            });
        }
        
        McpMessage::Initialized
    }
    
    /// Handle a call tool request
    ///
    /// This method processes a request to call a tool.
    /// It looks up the tool in the registry, calls it with the provided
    /// arguments, and returns the result.
    ///
    /// # Arguments
    ///
    /// * `state` - The application state
    /// * `params` - The call tool parameters
    ///
    /// # Returns
    ///
    /// An MCP message containing the tool result or an error
    async fn handle_call_tool(state: &AppState, params: CallToolParams) -> McpMessage {
        info!("Calling tool: {}", params.name);
        
        let registry = state.registry.lock().await;
        
        match registry.call_tool(&params.name, params.arguments).await {
            Ok(result) => McpMessage::CallToolResponse(result),
            Err(e) => {
                error!("Tool call error: {}", e);
                McpMessage::Error(ErrorData {
                    code: -32000,
                    message: format!("Tool call error: {}", e),
                    data: None,
                })
            }
        }
    }
    
    /// Handle a list tools request
    ///
    /// This method processes a request to list available tools.
    /// It retrieves the list of tools from the registry and returns it.
    ///
    /// # Arguments
    ///
    /// * `state` - The application state
    ///
    /// # Returns
    ///
    /// An MCP message containing the list of available tools
    async fn handle_list_tools(state: &AppState) -> McpMessage {
        info!("Listing tools");
        
        let registry = state.registry.lock().await;
        let tools = registry.list_tools();
        
        McpMessage::ListToolsResponse(ListToolsResult { tools })
    }
}

/// Application state shared between handlers
///
/// This struct contains the shared state that is passed to all request handlers.
/// It includes the server configuration, tool registry, and broadcast channel
/// for SSE messages.
#[derive(Clone)]
struct AppState {
    /// Server configuration
    config: ServerConfig,
    
    /// Tool registry
    registry: Arc<Mutex<ToolRegistry>>,
    
    /// Broadcast channel for SSE messages
    transport: tokio::sync::broadcast::Sender<McpMessage>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool::{Tool, text_content};
    use crate::message::CallToolResult;
    use async_trait::async_trait;
    use serde_json::Value;
    
    /// A simple test tool that echoes back input
    struct TestTool;
    
    #[async_trait]
    impl Tool for TestTool {
        fn name(&self) -> &str {
            "test"
        }
        
        fn description(&self) -> &str {
            "A test tool"
        }
        
        fn input_schema(&self) -> Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "echo": {
                        "type": "string"
                    }
                }
            })
        }
        
        async fn call(&self, args: Value) -> Result<CallToolResult> {
            let echo = args.get("echo")
                .and_then(|v| v.as_str())
                .unwrap_or("No input provided");
                
            Ok(CallToolResult {
                content: vec![text_content(format!("Echo: {}", echo))],
                is_error: false,
            })
        }
    }
    
    #[tokio::test]
    async fn test_handle_initialize() {
        let config = ServerConfig::default();
        let registry = ToolRegistry::new();
        let transport = tokio::sync::broadcast::channel(10).0;
        
        let state = AppState {
            config: config.clone(),
            registry: Arc::new(Mutex::new(registry)),
            transport,
        };
        
        let params = InitializeParams {
            protocol_version: config.protocol_version,
        };
        
        let response = McpServer::handle_initialize(&state, params).await;
        assert!(matches!(response, McpMessage::Initialized));
        
        // Test with unsupported version
        let params = InitializeParams {
            protocol_version: "unsupported".to_string(),
        };
        
        let response = McpServer::handle_initialize(&state, params).await;
        assert!(matches!(response, McpMessage::Error(_)));
    }
    
    #[tokio::test]
    async fn test_handle_call_tool() {
        let mut registry = ToolRegistry::new();
        registry.register(TestTool);
        
        let state = AppState {
            config: ServerConfig::default(),
            registry: Arc::new(Mutex::new(registry)),
            transport: tokio::sync::broadcast::channel(10).0,
        };
        
        let params = CallToolParams {
            name: "test".to_string(),
            arguments: Some(serde_json::json!({
                "echo": "Hello, world!"
            })),
        };
        
        let response = McpServer::handle_call_tool(&state, params).await;
        
        match response {
            McpMessage::CallToolResponse(result) => {
                match &result.content[0] {
                    crate::message::Content::Text(text) => {
                        assert_eq!(text.text, "Echo: Hello, world!");
                    },
                    _ => panic!("Expected text content"),
                }
            },
            _ => panic!("Expected CallToolResponse"),
        }
        
        // Test with non-existent tool
        let params = CallToolParams {
            name: "nonexistent".to_string(),
            arguments: None,
        };
        
        let response = McpServer::handle_call_tool(&state, params).await;
        assert!(matches!(response, McpMessage::Error(_)));
    }
} 