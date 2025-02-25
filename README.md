# MCP - Model Context Protocol for Rust

An idiomatic Rust implementation of the Model Context Protocol (MCP), providing a robust, type-safe, and high-performance library for AI tool integration. Types generated from the [Model Context Protocol (MCP) specification](https://spec.modelcontextprotocol.io/specification/2024-11-05/).

## Features

- **Type-safe Message Handling**: Strongly typed message structures for compile-time safety
- **Multiple Transport Mechanisms**: Support for STDIO and Server-Sent Events (SSE)
- **Tool Registry**: Easy registration and management of tools
- **Server Implementation**: Host MCP services with Axum
- **Client Implementation**: Consume MCP services with a simple API
- **Async/Await**: Built on Tokio for high-performance asynchronous I/O
- **JSON-RPC**: Implements the protocol using JSON-RPC 2.0 with extensions for SSE

## Schema Source

The types are generated from the official MCP schema version 2024-11-05:

- Schema URL: https://github.com/modelcontextprotocol/specification/blob/main/schema/2024-11-05/schema.json
- Specification Documentation: https://spec.modelcontextprotocol.io/specification/2024-11-05/

The types are generated using [typify](https://github.com/oxidecomputer/typify), which converts JSON Schema into idiomatic Rust types with full serde support.

## Installation

Add the following to your `Cargo.toml`:

```toml
[dependencies]
mcp = "0.1.0"
```

## Quick Start

### Creating a Tool

```rust
use mcp::{define_tool, tool::ToolRegistry};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, JsonSchema)]
struct EchoInput {
    message: String,
}

// Create a tool registry
let mut registry = ToolRegistry::new();

// Define and register a tool
let echo_tool = define_tool! {
    name: "echo",
    description: "Echo back the input message",
    input: EchoInput,
    handler: |args| async move {
        let input: EchoInput = serde_json::from_value(args)?;

        // Create a response
        let content = vec![mcp::tool::text_content(input.message)];

        Ok(mcp::message::CallToolResult {
            content,
            is_error: false,
        })
    }
};

registry.register(echo_tool);
```

### Starting a Server

```rust
use mcp::{server::{McpServer, ServerConfig}, tool::ToolRegistry};

// Create a server with the tool registry
let server = McpServer::new(ServerConfig::default(), registry);

// Start the server
server.start().await?;
```

### Using the Client

```rust
use mcp::{client::{McpClient, ClientConfig}};
use serde_json::json;

// Create a client
let client = McpClient::new(ClientConfig::default());

// Initialize the client
client.initialize().await?;

// Call a tool
let result = client.call_tool("echo", Some(json!({
    "message": "Hello, world!"
}))).await?;

println!("Result: {:?}", result);
```

## Architecture

The MCP library consists of several key components:

### Client

The client component (`mcp::client`) provides a high-level API for connecting to MCP servers, discovering available tools, and calling them. It handles:

- Connection initialization
- Tool discovery
- Tool invocation
- Error handling

```rust
// Example of client usage with error handling
let client = McpClient::new(ClientConfig::default());

match client.initialize().await {
    Ok(_) => println!("Connected to server"),
    Err(e) => eprintln!("Failed to connect: {}", e),
}

// List available tools
let tools = client.list_tools().await?;
for tool in &tools {
    println!("Tool: {} - {}", tool.name, tool.description);
}
```

### Server

The server component (`mcp::server`) provides a framework for hosting MCP services. It includes:

- HTTP server with Axum
- Tool registry management
- Message handling
- Server-Sent Events (SSE) for real-time updates

```rust
// Example of server with custom configuration
let config = ServerConfig {
    name: "my-mcp-server",
    version: "1.0.0",
    protocol_version: "2024-11-05",
    host: "0.0.0.0", // Listen on all interfaces
    port: 8080,      // Custom port
};

let server = McpServer::new(config, registry);
server.start().await?;
```

### Tool System

The tool system (`mcp::tool`) provides multiple ways to define and implement tools:

1. **Using the `define_tool!` macro** (simplest approach):

```rust
let tool = define_tool! {
    name: "calculator",
    description: "Perform basic calculations",
    input: CalculatorInput,
    handler: |args| async move {
        // Implementation
    }
};
```

2. **Using the `#[tool]` attribute** (for struct-based tools):

```rust
#[tool(name = "calculator", description = "Perform basic calculations")]
struct CalculatorTool;

#[async_trait]
impl ToolHandler for CalculatorTool {
    type Input = CalculatorInput;

    async fn handle(&self, input: Self::Input) -> Result<CallToolResult> {
        // Implementation
    }
}
```

3. **Implementing the `Tool` trait directly** (for maximum flexibility):

```rust
struct CalculatorTool;

#[async_trait]
impl Tool for CalculatorTool {
    fn name(&self) -> &str {
        "calculator"
    }

    fn description(&self) -> &str {
        "Perform basic calculations"
    }

    fn input_schema(&self) -> Value {
        // Define schema
    }

    async fn call(&self, args: Value) -> Result<CallToolResult> {
        // Implementation
    }
}
```

### Transport Layer

The transport layer (`mcp::transport`) provides abstractions for communication between clients and servers:

- **StdioTransport**: Uses standard input/output for communication
- **SseTransport**: Uses Server-Sent Events (SSE) for real-time communication

```rust
// Example of using StdioTransport
let mut transport = StdioTransport::new();
transport.write_message(message).await?;
```

### JSON-RPC Implementation

The JSON-RPC implementation (`mcp::rpc`) provides the underlying protocol for MCP:

- JSON-RPC 2.0 message types
- Request/response handling
- Error handling
- SSE extensions for real-time updates

## Error Handling

MCP uses [eyre](https://docs.rs/eyre) for error handling, providing rich context for errors. All public methods return `Result<T, eyre::Error>`, allowing for easy error handling with the `?` operator.

```rust
// Example of error handling
match client.call_tool("calculator", Some(args)).await {
    Ok(result) => {
        // Process result
    },
    Err(e) => {
        eprintln!("Error calling tool: {}", e);
        // Handle specific error types if needed
    }
}
```

## Examples

Check out the examples directory for complete working examples:

- `weather`: A weather service that demonstrates tool registration and server setup
- `client`: A simple client that connects to an MCP server
- `cli`: A command-line interface for interacting with MCP servers

To run the weather example:

```bash
export OPENWEATHER_API_KEY=your_api_key_here
cargo run --example weather
```

To run the CLI example:

```bash
# Start the weather server in one terminal
export OPENWEATHER_API_KEY=your_api_key_here
cargo run --example weather

# Run the CLI in another terminal
cargo run --example cli
```

## Project Structure

The project is organized as a Rust workspace with the following structure:

- `/mcp/`: Main library crate

  - `/src/`: Core library code
    - `lib.rs`: Library entry point and exports
    - `client.rs`: Client implementation for connecting to MCP servers
    - `server.rs`: Server implementation for hosting MCP services
    - `tool.rs`: Tool system for defining and managing tools
    - `message.rs`: Message types and serialization
    - `transport.rs`: Transport abstractions (STDIO, SSE)
    - `rpc.rs`: JSON-RPC implementation
    - `builder.rs`: Builder patterns for constructing MCP components
    - `schema.rs`: Schema-related utilities
  - `/examples/`: Example applications
    - `/weather/`: Weather service example
    - `/client/`: Client example
    - `/cli/`: Command-line interface example
  - `/tests/`: Integration tests
  - `build.rs`: Build script for code generation
  - `schema.json`: MCP schema definition

- `/mcp-macros/`: Procedural macros for tool definition

  - `/src/`: Macro implementation
  - `/tests/`: Macro tests

- `Cargo.toml`: Workspace definition
- `download-schema.sh`: Script to download the latest MCP schema

## Documentation

Comprehensive documentation is available for all components:

```bash
cargo doc --open
```

## License

This project is licensed under the MIT License - see the LICENSE.md file for details.
