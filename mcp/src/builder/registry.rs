use crate::builder::Tool;
use crate::{Error, schema};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Map;
use std::collections::HashMap;
use std::future::Future;
use std::ops::AsyncFn;
use std::pin::Pin;

/// A registry for managing available tools with shared state
pub struct ToolRegistry<State> {
    tools: HashMap<String, Tool<State>>,
    state: State,
}

pub trait HandlerFn<State, I, O> {
    fn run(
        &self,
        state: &State,
        args: Map<String, serde_json::Value>,
    ) -> impl Future<Output = Result<schema::CallToolResultContentItem, Error>>;

    fn schema(&self) -> serde_json::Value;
}

impl<State, F, I, O> HandlerFn<State, I, O> for F
where
    F: AsyncFn(&State, I) -> O,
    I: DeserializeOwned + schemars::JsonSchema,
    O: Serialize,
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
    pub fn register<F, I, O>(&mut self, name: impl Into<String>, handler: F)
    where
        F: HandlerFn<State, I, O> + Copy + 'static,
    {
        let name = name.into();
        let schema = handler.schema();
        let handler_fn: Box<
            dyn for<'a> Fn(
                &'a State,
                &'a Map<String, serde_json::Value>,
            ) -> Pin<
                Box<dyn Future<Output = Result<schema::CallToolResult, Error>> + 'a>,
            >,
        > = {
            Box::new(
                move |state: &State, args: &Map<String, serde_json::Value>| {
                    let args = args.clone();
                    Box::pin(async move {
                        let result = handler.run(state, args).await?;
                        Ok(schema::CallToolResult {
                            content: vec![result],
                            is_error: Some(false),
                            meta: serde_json::Map::new(),
                        })
                    })
                },
            )
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
        state: &State,
        request: &schema::CallToolRequest,
    ) -> Result<schema::CallToolResult, Error> {
        let tool = self.tools.get(&request.params.name).ok_or_else(|| Error {
            message: format!("Tool '{}' not found", request.params.name),
            code: 404,
        })?;

        (tool.handler)(state, &request.params.arguments).await
    }

    /// Iterate through all registered tools
    pub fn tools_iter(&self) -> impl Iterator<Item = (&String, &Tool<State>)> {
        self.tools.iter()
    }
}
