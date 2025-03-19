use crate::Error;
use crate::registry::{AsyncFnExt, HandlerArgs, HandlerFn, HandlerRegistry};
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

/// A registry for managing available tools with shared state
pub struct ToolRegistry<State> {
    registry: HandlerRegistry<Tool<State>>,
}

impl<State> ToolRegistry<State> {
    /// Register a new tool with the given name and handler
    pub fn register(&mut self, tool: Tool<State>) {
        self.registry.register(tool.name.clone(), tool);
    }
}

impl<State: Send + Sync + 'static> ToolRegistry<State> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Call a tool by name with the given arguments
    pub fn call_tool(
        &self,
        state: State,
        request: mcp_schema::CallToolParams,
    ) -> impl Future<Output = Result<mcp_schema::CallToolResult, Error>> + use<State> + Send + 'static
    {
        self.registry
            .call(state, &request.name, request.arguments.unwrap_or_default())
    }

    /// Iterate through all registered tools
    pub fn tools_iter(&self) -> impl Iterator<Item = (&String, &Tool<State>)> {
        self.registry.handlers_iter()
    }
}

impl<State> Default for ToolRegistry<State> {
    fn default() -> Self {
        Self {
            registry: HandlerRegistry::default(),
        }
    }
}

pub struct Tool<State> {
    name: String,
    description: Option<String>,
    schema: serde_json::Value,
    handler: Box<dyn HandlerFn<State, Vec<mcp_schema::PromptContent>> + Send + Sync>,
}

impl<State: Send + Sync + 'static> Tool<State> {
    #[must_use]
    pub fn builder() -> ToolBuilder<State> {
        ToolBuilder::new()
    }
}

impl<State: Send + Sync + 'static> HandlerFn<State, mcp_schema::CallToolResult> for Tool<State> {
    fn run(
        &self,
        state: State,
        args: HandlerArgs,
    ) -> Pin<Box<dyn Future<Output = Result<mcp_schema::CallToolResult, Error>> + Send>> {
        let content = self.handler.run(state, args);
        Box::pin(async move {
            let content = content.await?;

            Ok(mcp_schema::CallToolResult {
                meta: None,
                content,
                is_error: Some(false),
                extra: HashMap::new(),
            })
        })
    }
}

impl<State> TryFrom<&Tool<State>> for mcp_schema::Tool {
    type Error = serde_json::Error;

    fn try_from(tool: &Tool<State>) -> Result<Self, Self::Error> {
        Ok(Self {
            description: tool.description.clone(),
            input_schema: serde_json::from_value(tool.schema.clone())?,
            name: tool.name.clone(),
            extra: HashMap::new(),
        })
    }
}

/// A builder for constructing a tool with validation and metadata
pub struct ToolBuilder<State> {
    name: Option<String>,
    description: Option<String>,
    schema: Option<serde_json::Value>,
    handler: Option<Box<dyn HandlerFn<State, Vec<mcp_schema::PromptContent>> + Send + Sync>>,
}

impl<State: Send + Sync + 'static> ToolBuilder<State> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    #[must_use]
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    #[must_use]
    pub fn handler<I>(
        mut self,
        handler: impl AsyncFnExt<State, I, Vec<mcp_schema::PromptContent>>
        + Send
        + Sync
        + Copy
        + 'static,
    ) -> Self
    where
        I: DeserializeOwned + schemars::JsonSchema + Send + 'static,
    {
        self.schema = Some(serde_json::to_value(schemars::schema_for!(I)).unwrap());
        self.handler = Some(Box::new(handler.handler()));
        self
    }

    /// Builds a tool.
    ///
    /// # Errors
    /// If the name or handler was not set, this will error.
    pub fn build(self) -> Result<Tool<State>, Error> {
        Ok(Tool {
            name: self.name.unwrap_or_else(|| "unnamed tool".to_string()),
            description: self.description,
            schema: self.schema.ok_or_else(|| Error {
                message: "missing handler input schema".to_string(),
                code: 500,
            })?,
            handler: self.handler.ok_or_else(|| Error {
                message: "missing handler".to_string(),
                code: 500,
            })?,
        })
    }
}

impl<State> Default for ToolBuilder<State> {
    fn default() -> Self {
        Self {
            name: None,
            description: None,
            schema: None,
            handler: None,
        }
    }
}
