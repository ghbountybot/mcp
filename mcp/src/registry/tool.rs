use crate::Error;
use crate::registry::{AsyncFnExt, HandlerArgs, HandlerFn, HandlerRegistry};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;

/// A registry for managing available tools with shared state
pub struct ToolRegistry<State> {
    registry: HandlerRegistry<Tool<State>>,
}

impl<State: Send + Sync + 'static> ToolRegistry<State> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new tool with the given name and handler
    pub fn register(&mut self, tool: Tool<State>) {
        self.registry.register(tool.name.clone(), tool);
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
            registry: Default::default(),
        }
    }
}

struct ToolHandler<F, O> {
    handler: F,
    phantom: PhantomData<fn() -> O>,
}

impl<State, F, O> HandlerFn<State, String> for ToolHandler<F, O>
where
    State: Send + Sync + 'static,
    F: HandlerFn<State, O>,
    O: Serialize + 'static,
{
    fn run(
        &self,
        state: State,
        args: HandlerArgs,
    ) -> Pin<Box<dyn Future<Output = Result<String, Error>> + Send>> {
        let result = self.handler.run(state, args);
        Box::pin(async move {
            let result = result.await?;
            let result = serde_json::to_string(&result).unwrap();
            Ok(result)
        })
    }
}

pub struct Tool<State> {
    name: String,
    description: Option<String>,
    schema: serde_json::Value,
    handler: Box<dyn HandlerFn<State, String> + Send + Sync>,
}

impl<State: Send + Sync + 'static> Tool<State> {
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
        let result = self.handler.run(state, args);
        Box::pin(async move {
            let result = result.await?;
            let result = serde_json::to_string(&result).unwrap();
            let result = mcp_schema::TextContent {
                kind: "text".to_string(),
                text: result,
                annotated: mcp_schema::Annotated {
                    annotations: None,
                    extra: HashMap::new(),
                },
            };

            let result = mcp_schema::PromptContent::Text(result);
            Ok(mcp_schema::CallToolResult {
                meta: None,
                content: vec![result],
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
    required_args: Vec<String>,
    schema: Option<serde_json::Value>,
    handler: Option<Box<dyn HandlerFn<State, String> + Send + Sync>>,
}

impl<State: Send + Sync + 'static> ToolBuilder<State> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn required_arg(mut self, arg_name: impl Into<String>) -> Self {
        self.required_args.push(arg_name.into());
        self
    }

    pub fn handler<I, O>(
        mut self,
        handler: impl AsyncFnExt<State, I, O> + Send + Sync + Copy + 'static,
    ) -> Self
    where
        I: DeserializeOwned + schemars::JsonSchema + Send + 'static,
        O: Serialize + 'static,
    {
        self.schema = Some(serde_json::to_value(schemars::schema_for!(I)).unwrap());
        self.handler = Some(Box::new(ToolHandler {
            handler: handler.handler(),
            phantom: PhantomData,
        }));
        self
    }

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
            required_args: Vec::new(),
            schema: None,
            handler: None,
        }
    }
}
