//! # MCP - Model Context Protocol
//! 
//! An idiomatic Rust implementation of the Model Context Protocol (MCP).
//! 
//! MCP is a protocol for communication between AI models and tools.
//! This library provides a robust, type-safe, and high-performance
//! implementation of the MCP protocol.
//! 
//! ## Features
//! 
//! - Type-safe message handling
//! - Multiple transport mechanisms (STDIO, SSE)
//! - Tool registry for managing available tools
//! - Server implementation for hosting MCP services
//! - Client implementation for consuming MCP services
//! 
//! ## Example
//! 
//! ```rust,no_run
//! use mcp::{
//!     server::{McpServer, ServerConfig},
//!     tool::{Tool, ToolRegistry, TypedTool},
//! };
//! use eyre::Result;
//! use schemars::JsonSchema;
//! use serde::{Deserialize, Serialize};
//! 
//! #[derive(Deserialize, JsonSchema)]
//! struct EchoInput {
//!     message: String,
//! }
//! 
//! #[derive(Serialize)]
//! struct EchoOutput {
//!     response: String,
//! }
//! 
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     // Create a tool registry
//!     let mut registry = ToolRegistry::new();
//!     
//!     // Register a tool
//!     let echo_tool = TypedTool::new(
//!         "echo",
//!         "Echo back the input message",
//!         |input: EchoInput| async move {
//!             Ok(EchoOutput {
//!                 response: input.message,
//!             })
//!         }
//!     );
//!     
//!     registry.register(echo_tool);
//!     
//!     // Create and start the server
//!     let server = McpServer::new(ServerConfig::default(), registry);
//!     server.start().await?;
//!     
//!     Ok(())
//! }
//! ```

// Public modules
pub mod client;
pub mod message;
pub mod server;
pub mod tool;
pub mod transport;

// Re-export commonly used types
pub use client::McpClient;
pub use message::McpMessage;
pub use server::McpServer;
pub use tool::ToolRegistry;

// Re-export macros from mcp-macros
pub use mcp_macros::{define_tool, tool};
