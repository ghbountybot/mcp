use crate::builder::Tool;
use crate::{Error, schema};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Map;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

/// A registry for managing available tools with shared state
pub struct ToolRegistry<State> {
    tools: HashMap<String, Tool<State>>,
    state: State,
}

pub trait HandlerFn<State> {
    fn run(
        &self,
        state: &State,
        args: Map<String, serde_json::Value>,
    ) -> impl Future<Output = Result<schema::CallToolResultContentItem, Error>>;

    fn schema(&self) -> serde_json::Value;
}

impl<State, I, F, O> HandlerFn<State> for fn(&State, I) -> F
where
    I: DeserializeOwned + schemars::JsonSchema,
    O: Serialize,
    F: Future<Output = O>,
{
    fn run(
        &self,
        state: &State,
        args: Map<String, serde_json::Value>,
    ) -> impl Future<Output = Result<schema::CallToolResultContentItem, Error>> {
        let args = serde_json::from_value(serde_json::Value::Object(args))
            .map_err(|e| Error {
                message: format!("Failed to deserialize arguments: {e}"),
                code: 400,
            })
            .unwrap();

        async move {
            let result = self(state, args).await;
            let result = serde_json::to_string(&result).unwrap();
            let result = schema::TextContent {
                annotations: None,
                text: result,
                type_: "json".to_string(),
            };

            let result = schema::CallToolResultContentItem::TextContent(result);
            Ok(result)
        }
    }

    fn schema(&self) -> serde_json::Value {
        let schema = schemars::schema_for!(I);
        serde_json::to_value(schema).unwrap()
    }
}

impl<State> ToolRegistry<State> {
    #[must_use]
    pub fn new(state: State) -> Self {
        Self {
            tools: HashMap::new(),
            state,
        }
    }

    /// Register a new tool with the given name and handler
    pub fn register<I, O>(&mut self, name: impl Into<String>, handler: fn(&State, I) -> O)
    where
        fn(&State, I) -> O: HandlerFn<State> + 'static,
    {
        let name = name.into();
        let schema = handler.schema();
        let handler_fn: Box<
            dyn Fn(
                &Map<String, serde_json::Value>,
            )
                -> Pin<Box<dyn Future<Output = Result<schema::CallToolResult, Error>>>>,
        > = {
            let state = &self.state;
            Box::new(move |args: &Map<String, serde_json::Value>| {
                let args = args.clone();
                Box::pin(async move {
                    let result = handler.run(state, args).await?;
                    Ok(schema::CallToolResult {
                        content: vec![result],
                        is_error: Some(false),
                        meta: serde_json::Map::new(),
                    })
                })
                    as Pin<Box<dyn Future<Output = Result<schema::CallToolResult, Error>>>>
            })
        };

        self.tools.insert(
            name.clone(),
            Tool {
                name,
                schema,
                handler: handler_fn,
            },
        );
    }

    /// Call a tool by name with the given arguments
    pub async fn call_tool(
        &self,
        request: &schema::CallToolRequest,
    ) -> Result<schema::CallToolResult, Error> {
        let tool = self.tools.get(&request.params.name).ok_or_else(|| Error {
            message: format!("Tool '{}' not found", request.params.name),
            code: 404,
        })?;

        (tool.handler)(&request.params.arguments).await
    }

    /// List all registered tools
    pub fn list_tools(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }
}
