//! # JSON-RPC Implementation for MCP
//! 
//! This module provides a JSON-RPC implementation for the Model Context Protocol (MCP).
//! It handles the communication between clients and the server using the JSON-RPC 2.0
//! protocol specification, with extensions for Server-Sent Events (SSE) for real-time
//! notifications.
//!
//! ## Overview
//!
//! The JSON-RPC implementation consists of:
//!
//! 1. Message types for requests, responses, notifications, and errors
//! 2. An implementation of the MCP protocol using JSON-RPC
//! 3. Handlers for HTTP endpoints (message and SSE)
//! 4. A broadcast channel for sending notifications to all connected clients
//!
//! ## JSON-RPC Protocol
//!
//! This implementation follows the [JSON-RPC 2.0 specification](https://www.jsonrpc.org/specification)
//! with the following MCP-specific methods:
//!
//! - `initialize`: Initialize the connection with the server
//! - `tools/list`: List available tools
//! - `tools/call`: Call a specific tool with arguments
//!
//! ## Server-Sent Events (SSE)
//!
//! The implementation uses Server-Sent Events (SSE) to push notifications to clients.
//! When a client connects to the SSE endpoint, it receives:
//!
//! 1. An initial `endpoint` event with the message endpoint URL
//! 2. Subsequent `message` events containing JSON-RPC messages
//!
//! ## Example Usage
//!
//! ```rust,no_run
//! use axum::{Router, routing::{get, post}};
//! use mcp::rpc::{McpImpl, McpHandler, JsonRpcResponse};
//! use serde_json::Value;
//! use std::collections::HashMap;
//! use std::net::SocketAddr;
//! use tower_http::cors::CorsLayer;
//!
//! #[tokio::main]
//! async fn main() {
//!     // Create a custom handler for a tool
//!     let weather_handler = McpHandler::new("tools/call", |params| {
//!         Box::pin(async move {
//!             // Extract parameters and perform the operation
//!             let city = params.get("city").and_then(|v| v.as_str()).unwrap_or("Unknown");
//!             
//!             // Return a successful response
//!             JsonRpcResponse {
//!                 jsonrpc: "2.0".to_string(),
//!                 id: Some(1),
//!                 result: Some(serde_json::json!({
//!                     "content": [{
//!                         "type": "text",
//!                         "text": format!("Weather for {}: Sunny, 72°F", city)
//!                     }],
//!                     "isError": false
//!                 })),
//!                 error: None,
//!             }
//!         })
//!     });
//!     
//!     // Create a map of handlers
//!     let mut handlers = HashMap::new();
//!     handlers.insert("tools/call".to_string(), weather_handler.handler);
//!     
//!     // Create the MCP implementation with the handlers
//!     let state = McpImpl::new(handlers);
//!     
//!     // Create the router with the MCP endpoints
//!     let app = Router::new()
//!         .route("/api/message", post(McpImpl::message_handler))
//!         .route("/api/events", get(McpImpl::sse_handler))
//!         .layer(CorsLayer::permissive())
//!         .with_state(state);
//!     
//!     // Start the server
//!     let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
//!     println!("Server listening on {}", addr);
//!     axum::serve(tokio::net::TcpListener::bind(addr).await.unwrap(), app)
//!         .await
//!         .unwrap();
//! }
//! ```

use axum::{
    Json,
    extract::State,
    response::sse::{Event, Sse},
};
use futures::stream::{self, Stream};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, convert::Infallible, future::Future, pin::Pin, sync::Arc};
use tokio::sync::broadcast;
use tokio_stream::StreamExt;
use tracing::{debug, info, warn};

/// A future that resolves to a JSON-RPC response
///
/// This type alias represents an asynchronous function that returns a JSON-RPC response.
/// It's used for handlers that process JSON-RPC requests.
type JsonRpcFuture = Pin<Box<dyn Future<Output = JsonRpcResponse> + Send>>;

/// A function that handles a JSON-RPC request
///
/// This type alias represents a function that takes JSON parameters and returns
/// a future that resolves to a JSON-RPC response. It's used to register handlers
/// for specific JSON-RPC methods.
type HandlerFn = Box<dyn Fn(Value) -> JsonRpcFuture + Send + Sync>;

/// MCP implementation using JSON-RPC
///
/// This struct provides the core implementation of the MCP protocol using JSON-RPC.
/// It manages:
/// - A broadcast channel for sending messages to all connected clients
/// - A map of handlers for different JSON-RPC methods
///
/// # Examples
///
/// ```rust,no_run
/// use mcp::rpc::McpImpl;
/// use std::collections::HashMap;
///
/// // Create a default implementation with no custom handlers
/// let default_impl = McpImpl::default();
///
/// // Or create an implementation with custom handlers
/// let handlers = HashMap::new();
/// let custom_impl = McpImpl::new(handlers);
/// ```
#[derive(Clone)]
pub struct McpImpl {
    tx: Arc<broadcast::Sender<JsonRpcMessage>>,
    handlers: Arc<HashMap<String, HandlerFn>>,
}

/// JSON-RPC request
///
/// This struct represents a JSON-RPC request message according to the JSON-RPC 2.0
/// specification. It includes:
/// - The JSON-RPC version (always "2.0")
/// - An optional request ID
/// - The method name
/// - Optional parameters
///
/// # Examples
///
/// ```rust
/// use mcp::rpc::JsonRpcRequest;
/// use serde_json::json;
///
/// let request = JsonRpcRequest {
///     jsonrpc: "2.0".to_string(),
///     id: Some(1),
///     method: "tools/list".to_string(),
///     params: Some(json!({})),
/// };
/// ```
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct JsonRpcRequest {
    /// The JSON-RPC version (always "2.0")
    pub jsonrpc: String,
    
    /// The request ID (optional for notifications)
    pub id: Option<i32>,
    
    /// The method name
    pub method: String,
    
    /// The method parameters (optional)
    pub params: Option<Value>,
}

/// JSON-RPC response
///
/// This struct represents a JSON-RPC response message according to the JSON-RPC 2.0
/// specification. It includes:
/// - The JSON-RPC version (always "2.0")
/// - The request ID (matching the request)
/// - An optional result (for successful responses)
/// - An optional error (for failed responses)
///
/// Either `result` or `error` should be present, but not both.
///
/// # Examples
///
/// ```rust
/// use mcp::rpc::{JsonRpcResponse, JsonRpcError};
/// use serde_json::json;
///
/// // Successful response
/// let success = JsonRpcResponse {
///     jsonrpc: "2.0".to_string(),
///     id: Some(1),
///     result: Some(json!({"status": "ok"})),
///     error: None,
/// };
///
/// // Error response
/// let error = JsonRpcResponse {
///     jsonrpc: "2.0".to_string(),
///     id: Some(1),
///     result: None,
///     error: Some(JsonRpcError {
///         code: -32601,
///         message: "Method not found".to_string(),
///         data: None,
///     }),
/// };
/// ```
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct JsonRpcResponse {
    /// The JSON-RPC version (always "2.0")
    pub jsonrpc: String,
    
    /// The request ID (matching the request)
    pub id: Option<i32>,
    
    /// The result of the request (for successful responses)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    
    /// The error details (for failed responses)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC error
///
/// This struct represents a JSON-RPC error object according to the JSON-RPC 2.0
/// specification. It includes:
/// - An error code
/// - An error message
/// - Optional additional data
///
/// # Standard Error Codes
///
/// - `-32700`: Parse error
/// - `-32600`: Invalid request
/// - `-32601`: Method not found
/// - `-32602`: Invalid params
/// - `-32603`: Internal error
/// - `-32000` to `-32099`: Server error
///
/// # Examples
///
/// ```rust
/// use mcp::rpc::JsonRpcError;
/// use serde_json::json;
///
/// let error = JsonRpcError {
///     code: -32601,
///     message: "Method not found".to_string(),
///     data: Some(json!({"method": "unknown_method"})),
/// };
/// ```
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct JsonRpcError {
    /// The error code
    pub code: i32,
    
    /// The error message
    pub message: String,
    
    /// Additional error data (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// JSON-RPC notification
///
/// This struct represents a JSON-RPC notification message according to the JSON-RPC 2.0
/// specification. It's similar to a request but has no ID and doesn't expect a response.
///
/// # Examples
///
/// ```rust
/// use mcp::rpc::JsonRpcNotification;
/// use serde_json::json;
///
/// let notification = JsonRpcNotification {
///     jsonrpc: "2.0".to_string(),
///     method: "system/status".to_string(),
///     params: Some(json!({"status": "ready"})),
/// };
/// ```
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct JsonRpcNotification {
    /// The JSON-RPC version (always "2.0")
    pub jsonrpc: String,
    
    /// The notification method
    pub method: String,
    
    /// The notification parameters (optional)
    pub params: Option<Value>,
}

/// JSON-RPC message
///
/// This enum represents any type of JSON-RPC message (request, response, or notification).
/// It's used for broadcasting messages to all connected clients.
///
/// The `untagged` attribute allows serde to automatically determine the message type
/// based on the presence or absence of certain fields.
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum JsonRpcMessage {
    /// A JSON-RPC request
    Request(JsonRpcRequest),
    
    /// A JSON-RPC response
    Response(JsonRpcResponse),
    
    /// A JSON-RPC notification
    Notification(JsonRpcNotification),
}

impl Default for McpImpl {
    /// Creates a default MCP implementation with no custom handlers
    ///
    /// This creates a broadcast channel with a capacity of 100 messages
    /// and an empty map of handlers.
    fn default() -> Self {
        let (tx, _) = broadcast::channel(100);
        Self {
            tx: Arc::new(tx),
            handlers: Arc::new(HashMap::new()),
        }
    }
}

impl McpImpl {
    /// Creates a new MCP implementation with the given handlers
    ///
    /// # Arguments
    ///
    /// * `handlers` - A map of method names to handler functions
    ///
    /// # Returns
    ///
    /// A new `McpImpl` instance with the given handlers
    #[must_use]
    #[allow(dead_code)]
    pub fn new(handlers: HashMap<String, HandlerFn>) -> Self {
        let (tx, _) = broadcast::channel(100);
        Self {
            tx: Arc::new(tx),
            handlers: Arc::new(handlers),
        }
    }

    /// Handles SSE connections
    ///
    /// This method sets up a Server-Sent Events (SSE) connection with a client.
    /// It sends an initial `endpoint` event with the message endpoint URL,
    /// then broadcasts all JSON-RPC messages to the client.
    ///
    /// # Arguments
    ///
    /// * `state` - The MCP implementation state
    ///
    /// # Returns
    ///
    /// An SSE stream that sends events to the client
    #[allow(clippy::unused_async)]
    pub async fn sse_handler(
        State(state): State<Self>,
    ) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
        info!("New SSE connection established");
        let rx = state.tx.subscribe();

        // Send initial endpoint event as required by MCP spec
        let endpoint_url = "/api/message";
        debug!("Sending initial endpoint URL: {}", endpoint_url);

        let initial =
            stream::once(async move { Ok(Event::default().event("endpoint").data(endpoint_url)) });

        let stream = stream::unfold(rx, |mut rx| async move {
            match rx.recv().await {
                Ok(msg) => {
                    debug!("Broadcasting message: {:?}", msg);
                    let event = Event::default().event("message").json_data(msg).ok()?;
                    Some((Ok(event), rx))
                }
                Err(e) => {
                    warn!("Error receiving message: {}", e);
                    None
                }
            }
        });

        Sse::new(initial.chain(stream))
    }

    /// Handles JSON-RPC message requests
    ///
    /// This method processes incoming JSON-RPC requests, dispatches them to the
    /// appropriate handler, and returns the response. It also broadcasts the
    /// response to all connected SSE clients.
    ///
    /// # Built-in Methods
    ///
    /// - `initialize`: Initialize the connection with the server
    /// - `tools/list`: List available tools
    ///
    /// Other methods are dispatched to registered handlers.
    ///
    /// # Arguments
    ///
    /// * `state` - The MCP implementation state
    /// * `request` - The JSON-RPC request
    ///
    /// # Returns
    ///
    /// The JSON-RPC response
    pub async fn message_handler(
        State(state): State<Self>,
        Json(request): Json<JsonRpcRequest>,
    ) -> Json<JsonRpcResponse> {
        info!("Received message request - method: {}", request.method);
        debug!("Message request details: {:?}", request);

        let response = match request.method.as_str() {
            "initialize" => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: Some(serde_json::json!({
                    "serverInfo": {
                        "name": "mcp-weather",
                        "version": "0.1.0"
                    },
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": {
                            "listChanged": false
                        }
                    }
                })),
                error: None,
            },
            "tools/list" => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: Some(serde_json::json!({
                    "tools": [
                        {
                            "name": "get_alerts",
                            "description": "Get weather alerts for a US state",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "state": {
                                        "type": "string",
                                        "description": "Two-letter US state code (e.g. CA, NY)"
                                    }
                                },
                                "required": ["state"]
                            }
                        },
                        {
                            "name": "get_forecast",
                            "description": "Get weather forecast for a location",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "latitude": {
                                        "type": "number",
                                        "description": "Latitude of the location"
                                    },
                                    "longitude": {
                                        "type": "number",
                                        "description": "Longitude of the location"
                                    }
                                },
                                "required": ["latitude", "longitude"]
                            }
                        }
                    ]
                })),
                error: None,
            },
            method => {
                if let Some(handler) = state.handlers.get(method) {
                    let params = request.params.unwrap_or(Value::Null);
                    handler(params).await
                } else {
                    JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: request.id,
                        result: None,
                        error: Some(JsonRpcError {
                            code: -32601,
                            message: format!("Method not found: {method}"),
                            data: None,
                        }),
                    }
                }
            }
        };

        if let Err(e) = state.tx.send(JsonRpcMessage::Response(response.clone())) {
            warn!("Failed to broadcast response: {}", e);
        } else {
            debug!("Successfully broadcast response");
        }

        Json(response)
    }
}

/// MCP handler
///
/// This struct represents a handler for a specific JSON-RPC method.
/// It contains the method name and a function that processes requests
/// for that method.
///
/// # Examples
///
/// ```rust
/// use mcp::rpc::{McpHandler, JsonRpcResponse};
/// use serde_json::Value;
///
/// let handler = McpHandler::new("tools/call", |params| {
///     Box::pin(async move {
///         // Process the parameters and return a response
///         JsonRpcResponse {
///             jsonrpc: "2.0".to_string(),
///             id: Some(1),
///             result: Some(serde_json::json!({"status": "ok"})),
///             error: None,
///         }
///     })
/// });
/// ```
#[allow(dead_code)]
pub struct McpHandler {
    pub(crate) name: String,
    pub(crate) handler: HandlerFn,
}

impl McpHandler {
    /// Creates a new MCP handler
    ///
    /// # Arguments
    ///
    /// * `name` - The method name
    /// * `f` - The handler function
    ///
    /// # Returns
    ///
    /// A new `McpHandler` instance
    #[allow(dead_code)]
    pub fn new<F>(name: &str, f: F) -> Self
    where
        F: Fn(Value) -> JsonRpcFuture + Send + Sync + 'static,
    {
        Self {
            name: name.to_string(),
            handler: Box::new(f),
        }
    }
}
