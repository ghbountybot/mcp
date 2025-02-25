//! # Procedural Macros for MCP
//! 
//! This crate provides procedural macros for the Model Context Protocol (MCP).
//! These macros make it easier to define and implement tools for use with MCP.
//! 
//! ## Available Macros
//! 
//! - [`define_tool!`]: A declarative macro for creating anonymous tool implementations with minimal boilerplate
//! - [`tool`]: An attribute macro for implementing the `Tool` trait on a struct with state
//! 
//! ## When to Use Each Macro
//! 
//! - Use [`define_tool!`] for simple, stateless tools that can be defined inline
//! - Use [`tool`] for complex tools that need to maintain state or have multiple methods
//! 
//! ## Examples
//! 
//! Using the `define_tool!` macro:
//! 
//! ```ignore
//! use mcp_macros::define_tool;
//! use mcp::tool::text_content;
//! use mcp::message::CallToolResult;
//! use schemars::JsonSchema;
//! use serde::Deserialize;
//! use serde_json::json;
//! 
//! // Define the input type with serde for deserialization and JsonSchema for schema generation
//! #[derive(Deserialize, JsonSchema)]
//! struct EchoInput {
//!     message: String,
//! }
//! 
//! // Create a tool with a name, description, input type, and handler function
//! let echo_tool = define_tool! {
//!     name: "echo",
//!     description: "Echo back the input message",
//!     input: EchoInput,
//!     handler: |args| async move {
//!         // Parse the JSON arguments into our strongly-typed input
//!         let input: EchoInput = serde_json::from_value(args)?;
//!         
//!         // Create a text content response
//!         let content = vec![text_content(input.message)];
//!         
//!         // Return a successful result
//!         Ok(CallToolResult {
//!             content,
//!             is_error: false,
//!         })
//!     }
//! };
//! 
//! // Register the tool with a ToolRegistry
//! let mut registry = mcp::tool::ToolRegistry::new();
//! registry.register(echo_tool);
//! ```
//! 
//! Using the `tool` attribute macro:
//! 
//! ```ignore
//! use mcp_macros::tool;
//! use async_trait::async_trait;
//! use eyre::Result;
//! use mcp::tool::ToolHandler;
//! use mcp::message::CallToolResult;
//! use mcp::tool::text_content;
//! use schemars::JsonSchema;
//! use serde::Deserialize;
//! 
//! // Define the input type with serde for deserialization and JsonSchema for schema generation
//! #[derive(Deserialize, JsonSchema)]
//! struct EchoInput {
//!     message: String,
//! }
//!
//! // Apply the tool attribute to a struct that will hold any state needed by the tool
//! #[tool(name = "echo", description = "Echo back the input message")]
//! struct EchoTool {
//!     // Tool state can be stored here
//!     prefix: String,
//! }
//!
//! // Implement the ToolHandler trait to handle tool calls
//! #[async_trait]
//! impl ToolHandler for EchoTool {
//!     // Specify the input type for this tool
//!     type Input = EchoInput;
//!     
//!     // Implement the handle method to process tool calls
//!     async fn handle(&self, input: Self::Input) -> Result<CallToolResult> {
//!         // Create a response with the tool's state and the input
//!         let response = format!("{}: {}", self.prefix, input.message);
//!         
//!         // Return a successful result
//!         Ok(CallToolResult {
//!             content: vec![text_content(response)],
//!             is_error: false,
//!         })
//!     }
//! }
//!
//! // Create and register the tool
//! let echo_tool = EchoTool { prefix: "Echo".to_string() };
//! let mut registry = mcp::tool::ToolRegistry::new();
//! registry.register(echo_tool);
//! ```

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{quote, ToTokens};
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    spanned::Spanned,
    token::Comma,
    Expr, ExprClosure, Ident, LitStr, Token, Type,
};

/// A struct to parse the input to the define_tool macro
struct ToolDefinition {
    name: LitStr,
    description: LitStr,
    input_type: Type,
    handler: ExprClosure,
}

impl Parse for ToolDefinition {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut name = None;
        let mut description = None;
        let mut input_type = None;
        let mut handler = None;

        // Parse the fields in any order
        while !input.is_empty() {
            let field_name: Ident = input.parse()?;
            input.parse::<Token![:]>()?;

            match field_name.to_string().as_str() {
                "name" => {
                    name = Some(input.parse()?);
                },
                "description" => {
                    description = Some(input.parse()?);
                },
                "input" => {
                    input_type = Some(input.parse()?);
                },
                "handler" => {
                    handler = Some(input.parse()?);
                },
                _ => {
                    return Err(syn::Error::new(
                        field_name.span(),
                        format!("Unknown field: {}", field_name),
                    ));
                }
            }

            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }

        // Ensure all required fields are present
        let name = name.ok_or_else(|| syn::Error::new(Span::call_site(), "Missing 'name' field"))?;
        let description = description.ok_or_else(|| syn::Error::new(Span::call_site(), "Missing 'description' field"))?;
        let input_type = input_type.ok_or_else(|| syn::Error::new(Span::call_site(), "Missing 'input' field"))?;
        let handler = handler.ok_or_else(|| syn::Error::new(Span::call_site(), "Missing 'handler' field"))?;

        Ok(ToolDefinition {
            name,
            description,
            input_type,
            handler,
        })
    }
}

/// The `define_tool!` macro creates an anonymous implementation of the `Tool` trait.
///
/// This macro provides a concise way to define tools without creating a separate struct
/// and implementing the `Tool` trait manually. It's ideal for simple, stateless tools
/// that can be defined inline.
///
/// # Parameters
///
/// * `name`: A string literal specifying the name of the tool. This should be a unique
///   identifier that clients can use to call the tool.
///
/// * `description`: A string literal describing what the tool does and how to use it.
///   This description will be provided to clients to help them understand the tool's purpose.
///
/// * `input`: The type that represents the tool's input parameters. This type must implement
///   `serde::Deserialize` for parsing JSON arguments and `schemars::JsonSchema` for generating
///   the JSON schema.
///
/// * `handler`: A closure that takes a `serde_json::Value` argument and returns a `Future`
///   that resolves to a `Result<CallToolResult>`. The closure is responsible for parsing
///   the input, performing the tool's action, and returning the result.
///
/// # Returns
///
/// An anonymous struct that implements the `Tool` trait, which can be registered with
/// a `ToolRegistry`.
///
/// # Example
///
/// ```ignore
/// use mcp_macros::define_tool;
/// use mcp::tool::text_content;
/// use mcp::message::CallToolResult;
/// use schemars::JsonSchema;
/// use serde::Deserialize;
/// 
/// #[derive(Deserialize, JsonSchema)]
/// struct CalculatorInput {
///     a: f64,
///     b: f64,
///     operation: String,
/// }
/// 
/// let calculator_tool = define_tool! {
///     name: "calculator",
///     description: "Perform basic arithmetic operations",
///     input: CalculatorInput,
///     handler: |args| async move {
///         let input: CalculatorInput = serde_json::from_value(args)?;
///         
///         let result = match input.operation.as_str() {
///             "add" => input.a + input.b,
///             "subtract" => input.a - input.b,
///             "multiply" => input.a * input.b,
///             "divide" => {
///                 if input.b == 0.0 {
///                     return Ok(CallToolResult {
///                         content: vec![text_content("Error: Division by zero")],
///                         is_error: true,
///                     });
///                 }
///                 input.a / input.b
///             },
///             _ => {
///                 return Ok(CallToolResult {
///                     content: vec![text_content(format!("Unknown operation: {}", input.operation))],
///                     is_error: true,
///                 });
///             }
///         };
///         
///         Ok(CallToolResult {
///             content: vec![text_content(result.to_string())],
///             is_error: false,
///         })
///     }
/// };
/// ```
#[proc_macro]
pub fn define_tool(input: TokenStream) -> TokenStream {
    let tool_def = parse_macro_input!(input as ToolDefinition);
    
    let name = tool_def.name;
    let description = tool_def.description;
    let input_type = tool_def.input_type;
    let handler = tool_def.handler;
    
    let expanded = quote! {
        {
            struct AnonymousTool;
            
            #[async_trait::async_trait]
            impl mcp::tool::Tool for AnonymousTool {
                fn name(&self) -> &str {
                    #name
                }
                
                fn description(&self) -> &str {
                    #description
                }
                
                fn input_schema(&self) -> serde_json::Value {
                    let schema = schemars::schema_for!(#input_type);
                    serde_json::to_value(schema).unwrap_or_default()
                }
                
                async fn call(&self, args: serde_json::Value) -> eyre::Result<mcp::message::CallToolResult> {
                    let handler = #handler;
                    handler(args).await
                }
            }
            
            AnonymousTool
        }
    };
    
    TokenStream::from(expanded)
}

/// A struct to parse the input to the tool attribute macro
struct ToolAttr {
    name: Option<LitStr>,
    description: Option<LitStr>,
}

impl Parse for ToolAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut name = None;
        let mut description = None;

        if input.is_empty() {
            return Ok(ToolAttr { name, description });
        }

        let vars = Punctuated::<Expr, Comma>::parse_terminated(input)?;

        for expr in vars {
            match expr {
                Expr::Assign(assign) => {
                    let key = assign.left.to_token_stream().to_string();
                    match key.as_str() {
                        "name" => {
                            if let Expr::Lit(lit) = *assign.right {
                                if let syn::Lit::Str(s) = lit.lit {
                                    name = Some(s);
                                }
                            }
                        }
                        "description" => {
                            if let Expr::Lit(lit) = *assign.right {
                                if let syn::Lit::Str(s) = lit.lit {
                                    description = Some(s);
                                }
                            }
                        }
                        _ => {
                            return Err(syn::Error::new(
                                assign.left.span(),
                                format!("Unknown attribute: {}", key),
                            ));
                        }
                    }
                }
                _ => {
                    return Err(syn::Error::new(
                        expr.span(),
                        "Expected key-value pair like name = \"tool_name\"",
                    ));
                }
            }
        }

        Ok(ToolAttr { name, description })
    }
}

/// The `tool` attribute macro implements the `Tool` trait for a struct.
///
/// This macro is designed for creating tools that need to maintain state or have
/// complex behavior that's better expressed as a struct with methods. It automatically
/// implements the `Tool` trait for the struct, connecting it to the `ToolHandler` trait
/// that you must implement manually.
///
/// # Parameters
///
/// * `name` (optional): A string literal specifying the name of the tool. If not provided,
///   the macro will call a `name()` method on the struct, which you must implement.
///
/// * `description` (optional): A string literal describing what the tool does. If not provided,
///   the macro will call a `description()` method on the struct, which you must implement.
///
/// # Requirements
///
/// After applying this attribute, you must implement the `ToolHandler` trait for your struct:
///
/// ```ignore
/// #[async_trait]
/// impl ToolHandler for YourTool {
///     type Input = YourInputType; // Must implement Deserialize + JsonSchema
///     
///     async fn handle(&self, input: Self::Input) -> Result<CallToolResult> {
///         // Your implementation here
///     }
/// }
/// ```
///
/// # Example
///
/// ```ignore
/// use mcp_macros::tool;
/// use async_trait::async_trait;
/// use eyre::Result;
/// use mcp::tool::{ToolHandler, text_content};
/// use mcp::message::CallToolResult;
/// use schemars::JsonSchema;
/// use serde::Deserialize;
/// 
/// // Define the input type
/// #[derive(Deserialize, JsonSchema)]
/// struct WeatherInput {
///     location: String,
///     units: Option<String>,
/// }
/// 
/// // Create a tool with state (API client)
/// #[tool(name = "weather", description = "Get the current weather for a location")]
/// struct WeatherTool {
///     api_key: String,
///     client: reqwest::Client,
/// }
/// 
/// impl WeatherTool {
///     // Constructor for the tool
///     pub fn new(api_key: String) -> Self {
///         Self {
///             api_key,
///             client: reqwest::Client::new(),
///         }
///     }
///     
///     // Helper method for the tool
///     async fn fetch_weather(&self, location: &str, units: &str) -> Result<String> {
///         // Implementation details...
///         Ok("Weather data here".to_string())
///     }
/// }
/// 
/// // Implement the ToolHandler trait
/// #[async_trait]
/// impl ToolHandler for WeatherTool {
///     type Input = WeatherInput;
///     
///     async fn handle(&self, input: Self::Input) -> Result<CallToolResult> {
///         let units = input.units.unwrap_or_else(|| "metric".to_string());
///         let weather = self.fetch_weather(&input.location, &units).await?;
///         
///         Ok(CallToolResult {
///             content: vec![text_content(weather)],
///             is_error: false,
///         })
///     }
/// }
/// ```
#[proc_macro_attribute]
pub fn tool(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attr = parse_macro_input!(attr as ToolAttr);
    let input = parse_macro_input!(item as syn::ItemStruct);
    
    let struct_name = &input.ident;
    
    let name = attr.name.map_or_else(
        || quote! { self.name() },
        |n| quote! { #n }
    );
    
    let description = attr.description.map_or_else(
        || quote! { self.description() },
        |d| quote! { #d }
    );
    
    let expanded = quote! {
        #input
        
        #[async_trait::async_trait]
        impl mcp::tool::Tool for #struct_name {
            fn name(&self) -> &str {
                #name
            }
            
            fn description(&self) -> &str {
                #description
            }
            
            fn input_schema(&self) -> serde_json::Value {
                let schema = schemars::schema_for!(<Self as mcp::tool::ToolHandler>::Input);
                serde_json::to_value(schema).unwrap_or_default()
            }
            
            async fn call(&self, args: serde_json::Value) -> eyre::Result<mcp::message::CallToolResult> {
                let input = serde_json::from_value(args)?;
                self.handle(input).await
            }
        }
    };
    
    TokenStream::from(expanded)
} 