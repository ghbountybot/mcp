use crate::Error;
use crate::registry::{AsyncFnExt, HandlerArgs, HandlerFn, HandlerRegistry};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::future::Future;
use std::marker::PhantomData;
use std::ops::AsyncFn;
use std::pin::Pin;

/// A registry for managing available tools with shared state
pub struct ToolRegistry<State> {
    registry: HandlerRegistry<Tool<State>>,
}

impl<State> ToolRegistry<State> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new tool with the given name and handler
    pub fn register<I, O>(
        &mut self,
        name: impl Into<String>,
        handler: impl AsyncFn(&State, I) -> Result<O, Error> + AsyncFnExt<State, I, O> + Copy + 'static,
    ) where
        I: DeserializeOwned + schemars::JsonSchema + 'static,
        O: Serialize + 'static,
    {
        let name = name.into();
        let schema = serde_json::to_value(schemars::schema_for!(I)).unwrap();

        self.registry.register(
            name.clone(),
            Tool {
                name,
                schema,
                handler: Box::new(ToolHandler {
                    handler: handler.handler(),
                    phantom: PhantomData,
                }),
            },
        );
    }

    /// Call a tool by name with the given arguments
    pub async fn call_tool(
        &self,
        state: &State,
        request: &mcp_schema::CallToolParams,
    ) -> Result<mcp_schema::CallToolResult, Error> {
        self.registry
            .call(
                state,
                &request.name,
                request.arguments.clone().unwrap_or_default(),
            )
            .await
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
    phantom: PhantomData<O>,
}

impl<State, F, O> HandlerFn<State, String> for ToolHandler<F, O>
where
    F: HandlerFn<State, O>,
    O: Serialize,
{
    fn run<'a>(
        &'a self,
        state: &'a State,
        args: HandlerArgs,
    ) -> Pin<Box<dyn Future<Output = Result<String, Error>> + 'a>> {
        Box::pin(async move {
            let result = self.handler.run(state, args).await?;
            let result = serde_json::to_string(&result).unwrap();
            Ok(result)
        })
    }
}

pub struct Tool<State> {
    pub(crate) name: String,
    pub(crate) schema: serde_json::Value,
    handler: Box<dyn HandlerFn<State, String>>,
}

impl<State> HandlerFn<State, mcp_schema::CallToolResult> for Tool<State> {
    fn run<'a>(
        &'a self,
        state: &'a State,
        args: HandlerArgs,
    ) -> Pin<Box<dyn Future<Output = Result<mcp_schema::CallToolResult, Error>> + 'a>> {
        Box::pin(async move {
            let result = self.handler.run(state, args).await?;
            let result = serde_json::to_string(&result).unwrap();
            let result = mcp_schema::TextContent {
                kind: "json".to_string(),
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
            description: todo!(),
            input_schema: serde_json::from_value(tool.schema.clone())?,
            name: tool.name.clone(),
            extra: HashMap::new(),
        })
    }
}

/// A builder for constructing a tool with validation and metadata
pub struct ToolBuilder {
    name: String,
    description: Option<String>,
    required_args: Vec<String>,
    handler: Option<
        Box<
            dyn Fn(
                &HashMap<String, serde_json::Value>,
            ) -> Result<mcp_schema::CallToolResult, Error>,
        >,
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
        F: Fn(&HashMap<String, serde_json::Value>) -> Result<mcp_schema::CallToolResult, Error>
            + 'static,
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
