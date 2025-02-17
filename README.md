# Model Context Protocol (MCP) Types

This repository contains Rust types generated from the [Model Context Protocol (MCP) specification](https://spec.modelcontextprotocol.io/specification/2024-11-05/).

## About

The Model Context Protocol (MCP) is a specification for standardizing context exchange between AI models and their runtime environments. This crate provides strongly-typed Rust bindings generated from the official MCP JSON Schema.

## Schema Source

The types are generated from the official MCP schema version 2024-11-05:
- Schema URL: https://github.com/modelcontextprotocol/specification/blob/main/schema/2024-11-05/schema.json
- Specification Documentation: https://spec.modelcontextprotocol.io/specification/2024-11-05/

## Usage

The generated types are available through the `types` module. Import them in your code:

```rust
use mcp::types::*;
```

## Generation

The types are generated using [typify](https://github.com/oxidecomputer/typify), which converts JSON Schema into idiomatic Rust types with full serde support. 