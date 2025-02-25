//! # Tool System for MCP
//! 
//! This module provides the core abstractions for defining and working with tools
//! in the Model Context Protocol (MCP). Tools are the primary way for AI models
//! to interact with external systems and perform actions.
//! 
//! ## Tool Traits
//! 
//! - [`Tool`]: The core trait that all tools must implement
//! - [`ToolHandler`]: A trait for handling tool calls with strongly typed inputs
//! 
//! ## Tool Types
//! 
//! - [`TypedTool`]: A tool implementation that handles strongly typed inputs and outputs
//! - [`ToolRegistry`]: A registry for managing and calling tools
//! 
//! ## Helper Functions
//! 
//! - [`text_content`]: Create text content for tool responses
//! - [`image_content`]: Create image content for tool responses
//! - [`resource_content`]: Create resource content for tool responses

use async_trait::async_trait;
use eyre::{Result, WrapErr};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

use crate::message::ToolDefinition;
use crate::message::{CallToolResult, Content, TextContent, ImageContent, ResourceContent};

/// Trait representing a tool that can be called via MCP
///
/// This is the core trait that all tools must implement. It defines the
/// interface for getting information about a tool and calling it with
/// arguments.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Get the name of the tool
    ///
    /// The name should be a unique identifier for the tool.
    fn name(&self) -> &str;
    
    /// Get the description of the tool
    ///
    /// The description should explain what the tool does and how to use it.
    fn description(&self) -> &str;
    
    /// Get the JSON schema for the tool's input
    ///
    /// This schema is used to validate input arguments and provide
    /// documentation for the tool's parameters.
    fn input_schema(&self) -> Value;
    
    /// Call the tool with the given arguments
    ///
    /// This method is called when a client requests the tool to be executed.
    /// The arguments are provided as a JSON value, which should be validated
    /// and parsed according to the tool's input schema.
    async fn call(&self, args: Value) -> Result<CallToolResult>;
}

/// Trait for handling tool calls with a strongly typed input
#[async_trait]
pub trait ToolHandler {
    /// The input type for this tool
    type Input: for<'de> serde::Deserialize<'de> + Send;
    
    /// Handle a tool call with the typed input
    async fn handle(&self, input: Self::Input) -> eyre::Result<crate::message::CallToolResult>;
}

/// A registry for managing available tools
///
/// The `ToolRegistry` is a central repository for all tools available in an MCP server.
/// It provides methods for registering tools, calling them by name, and listing all
/// available tools.
///
/// # Example
///
/// ```rust,no_run
/// use mcp::tool::{Tool, ToolRegistry, TypedTool};
/// use schemars::JsonSchema;
/// use serde::Deserialize;
///
/// #[derive(Deserialize, JsonSchema)]
/// struct GreetingInput {
///     name: String,
/// }
///
/// let mut registry = ToolRegistry::new();
///
/// // Register a typed tool
/// let greeting_tool = TypedTool::new(
///     "greeting",
///     "Generate a greeting message",
///     |input: GreetingInput| async move {
///         Ok(format!("Hello, {}!", input.name))
///     }
/// );
///
/// registry.register(greeting_tool);
/// ```
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    /// Create a new empty tool registry
    #[must_use]
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }
    
    /// Register a tool with the registry
    ///
    /// This method adds a tool to the registry, making it available for clients to call.
    /// If a tool with the same name already exists, it will be replaced.
    ///
    /// # Arguments
    ///
    /// * `tool` - The tool to register
    pub fn register<T: Tool + 'static>(&mut self, tool: T) {
        let name = tool.name().to_string();
        self.tools.insert(name, Arc::new(tool));
    }
    
    /// Call a tool by name with the given arguments
    ///
    /// This method looks up a tool by name and calls it with the provided arguments.
    /// If the tool is not found, an error is returned.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the tool to call
    /// * `args` - The arguments to pass to the tool, or `None` for no arguments
    ///
    /// # Returns
    ///
    /// The result of the tool call, or an error if the tool is not found or the call fails
    pub async fn call_tool(&self, name: &str, args: Option<Value>) -> Result<CallToolResult> {
        let tool = self.tools.get(name).ok_or_else(|| {
            eyre::eyre!("Tool not found: {}", name)
        })?;
        
        let args = args.unwrap_or(Value::Null);
        tool.call(args).await
    }
    
    /// List all registered tools
    pub fn list_tools(&self) -> Vec<ToolDefinition> {
        self.tools
            .values()
            .map(|tool| ToolDefinition {
                name: tool.name().to_string(),
                description: tool.description().to_string(),
                input_schema: tool.input_schema(),
            })
            .collect()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// A tool implementation that handles strongly typed inputs and outputs
///
/// `TypedTool` provides a convenient way to create tools with strongly typed
/// inputs and outputs. It handles serialization and deserialization of inputs
/// and outputs automatically.
///
/// # Type Parameters
///
/// * `I` - The input type, which must be deserializable from JSON and have a JSON schema
/// * `O` - The output type, which must be serializable to JSON
/// * `F` - The function type that processes inputs and produces outputs
///
/// # Example
///
/// ```rust,no_run
/// use mcp::tool::TypedTool;
/// use schemars::JsonSchema;
/// use serde::Deserialize;
///
/// #[derive(Deserialize, JsonSchema)]
/// struct CalculatorInput {
///     a: i32,
///     b: i32,
///     operation: String,
/// }
///
/// let calculator = TypedTool::new(
///     "calculator",
///     "Perform basic arithmetic operations",
///     |input: CalculatorInput| async move {
///         match input.operation.as_str() {
///             "add" => Ok(input.a + input.b),
///             "subtract" => Ok(input.a - input.b),
///             "multiply" => Ok(input.a * input.b),
///             "divide" => {
///                 if input.b == 0 {
///                     Err(eyre::eyre!("Division by zero"))
///                 } else {
///                     Ok(input.a / input.b)
///                 }
///             },
///             _ => Err(eyre::eyre!("Unknown operation: {}", input.operation)),
///         }
///     }
/// );
/// ```
pub struct TypedTool<I, O, F, Fut>
where
    I: DeserializeOwned + JsonSchema + Send + Sync + 'static,
    O: Serialize + Send + Sync + 'static,
    F: Fn(I) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<O>> + Send,
{
    name: String,
    description: String,
    handler: F,
    _phantom: std::marker::PhantomData<(I, O)>,
}

impl<I, O, F, Fut> TypedTool<I, O, F, Fut>
where
    I: DeserializeOwned + JsonSchema + Send + Sync + 'static,
    O: Serialize + Send + Sync + 'static,
    F: Fn(I) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<O>> + Send,
{
    /// Create a new typed tool
    pub fn new(name: impl Into<String>, description: impl Into<String>, handler: F) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            handler,
            _phantom: std::marker::PhantomData,
        }
    }
}

#[async_trait]
impl<I, O, F, Fut> Tool for TypedTool<I, O, F, Fut>
where
    I: DeserializeOwned + JsonSchema + Send + Sync + 'static,
    O: Serialize + Send + Sync + 'static,
    F: Fn(I) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<O>> + Send,
{
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn input_schema(&self) -> Value {
        let schema = schemars::schema_for!(I);
        serde_json::to_value(schema).unwrap_or_default()
    }

    async fn call(&self, args: Value) -> Result<CallToolResult> {
        let input: I = serde_json::from_value(args)
            .wrap_err_with(|| format!("Failed to parse arguments for tool '{}'", self.name))?;
        
        let result = (self.handler)(input).await?;
        let json = serde_json::to_string(&result)
            .wrap_err("Failed to serialize tool result")?;
        
        Ok(CallToolResult {
            content: vec![text_content(json)],
            is_error: false,
        })
    }
}

/// Create text content for tool responses
///
/// This helper function creates a `Content::Text` variant with the provided text.
///
/// # Arguments
///
/// * `text` - The text content to include in the response
///
/// # Returns
///
/// A `Content::Text` variant with the text data
#[must_use]
pub fn text_content(text: impl Into<String>) -> Content {
    Content::Text(TextContent {
        text: text.into(),
    })
}

/// Create image content for tool responses
///
/// This helper function creates a `Content::Image` variant with image data.
///
/// # Arguments
///
/// * `url` - The URL of the image
/// * `mime_type` - The MIME type of the image (e.g., "image/png", "image/jpeg")
///
/// # Returns
///
/// A `Content::Image` variant with the image data
#[must_use]
pub fn image_content(url: impl Into<String>, mime_type: impl Into<String>) -> Content {
    Content::Image(ImageContent {
        url: url.into(),
        mime_type: mime_type.into(),
    })
}

/// Create resource content for tool responses
///
/// This helper function creates a `Content::Resource` variant with resource data.
///
/// # Arguments
///
/// * `url` - The URL of the resource
/// * `mime_type` - Optional MIME type of the resource
///
/// # Returns
///
/// A `Content::Resource` variant with the resource data
#[must_use]
pub fn resource_content(url: impl Into<String>, mime_type: Option<String>) -> Content {
    Content::Resource(ResourceContent {
        url: url.into(),
        mime_type,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    
    #[derive(Deserialize, JsonSchema)]
    struct EchoInput {
        message: String,
    }
    
    struct EchoTool;
    
    #[async_trait]
    impl Tool for EchoTool {
        fn name(&self) -> &str {
            "echo"
        }
        
        fn description(&self) -> &str {
            "Echoes back the input message"
        }
        
        fn input_schema(&self) -> Value {
            let schema = schemars::schema_for!(EchoInput);
            serde_json::to_value(schema).unwrap()
        }
        
        async fn call(&self, args: Value) -> Result<CallToolResult> {
            let input: EchoInput = serde_json::from_value(args)?;
            
            Ok(CallToolResult {
                content: vec![text_content(input.message)],
                is_error: false,
            })
        }
    }
    
    #[tokio::test]
    async fn test_tool_registry() {
        let mut registry = ToolRegistry::new();
        registry.register(EchoTool);
        
        let args = serde_json::json!({
            "message": "Hello, world!"
        });
        
        let result = registry.call_tool("echo", Some(args)).await.unwrap();
        
        match &result.content[0] {
            Content::Text(text) => assert_eq!(text.text, "Hello, world!"),
            _ => panic!("Expected text content"),
        }
    }
    
    #[tokio::test]
    async fn test_typed_tool() {
        let mut registry = ToolRegistry::new();
        
        let echo_tool = TypedTool::new(
            "typed_echo",
            "Typed echo tool",
            |input: EchoInput| async move { Ok(input.message) }
        );
        
        registry.register(echo_tool);
        
        let args = serde_json::json!({
            "message": "Hello from typed tool!"
        });
        
        let result = registry.call_tool("typed_echo", Some(args)).await.unwrap();
        
        match &result.content[0] {
            Content::Text(text) => assert!(text.text.contains("Hello from typed tool!")),
            _ => panic!("Expected text content"),
        }
    }
} 