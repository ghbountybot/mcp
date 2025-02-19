mod registry;

pub use registry::ToolRegistry;
use serde_json::Map;
use std::future::Future;
use std::pin::Pin;

use crate::Error;
use crate::schema;

/// A type representing a tool that can be called
pub struct Tool<State> {
    pub(crate) name: String,
    pub(crate) schema: serde_json::Value,
    pub(crate) handler: Box<
        dyn for<'a> Fn(
            &'a State,
            &'a Map<String, serde_json::Value>,
        )
            -> Pin<Box<dyn Future<Output = Result<schema::CallToolResult, Error>> + 'a>>,
    >,
}

/// A builder for constructing a tool with validation and metadata
pub struct ToolBuilder {
    name: String,
    description: Option<String>,
    required_args: Vec<String>,
    handler: Option<
        Box<dyn Fn(&Map<String, serde_json::Value>) -> Result<schema::CallToolResult, Error>>,
    >,
}

impl ToolBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            required_args: Vec::new(),
            handler: None,
        }
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn required_arg(mut self, arg_name: impl Into<String>) -> Self {
        self.required_args.push(arg_name.into());
        self
    }

    pub fn handler<F>(mut self, handler: F) -> Self
    where
        F: Fn(&Map<String, serde_json::Value>) -> Result<schema::CallToolResult, Error> + 'static,
    {
        let required_args = self.required_args.clone();
        self.handler = Some(Box::new(move |args| {
            // Validate required arguments
            for arg in &required_args {
                if !args.contains_key(arg) {
                    return Err(Error {
                        message: format!("Missing required argument: {}", arg),
                        code: 400,
                    });
                }
            }
            handler(args)
        }));
        self
    }

    // pub fn register(self, registry: &mut ToolRegistry) -> Result<(), Error> {
    //     let handler = self.handler.ok_or_else(|| Error {
    //         message: "Tool handler not set".to_string(),
    //         code: 500,
    //     })?;
    //
    //     registry.register(self.name, handler);
    //     Ok(())
    // }
}
