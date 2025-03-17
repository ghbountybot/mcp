use crate::Error;
use crate::registry::{AsyncFnExt, HandlerArgs, HandlerFn, HandlerRegistry};
use schemars::schema::{InstanceType, Schema, SingleOrVec};
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

/// A registry for managing available prompts with shared state
pub struct PromptRegistry<State> {
    registry: HandlerRegistry<Prompt<State>>,
}

impl<State: Send + Sync + 'static> PromptRegistry<State> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new tool with the given name and handler
    pub fn register(&mut self, tool: Prompt<State>) {
        self.registry.register(tool.name.clone(), tool);
    }

    /// Call a tool by name with the given arguments
    pub fn get_prompt(
        &self,
        state: State,
        request: mcp_schema::GetPromptParams,
    ) -> impl Future<Output = Result<mcp_schema::GetPromptResult, Error>> + use<State> + Send + 'static
    {
        self.registry.call(
            state,
            &request.name,
            request
                .arguments
                .unwrap_or_default()
                .into_iter()
                .map(|(name, value)| (name, serde_json::Value::String(value)))
                .collect(),
        )
    }

    /// Iterate through all registered prompts
    pub fn prompts_iter(&self) -> impl Iterator<Item = (&String, &Prompt<State>)> {
        self.registry.handlers_iter()
    }
}

impl<State> Default for PromptRegistry<State> {
    fn default() -> Self {
        Self {
            registry: Default::default(),
        }
    }
}

pub struct Prompt<State> {
    name: String,
    description: Option<String>,
    schema: Vec<mcp_schema::PromptArgument>,
    handler: Box<dyn HandlerFn<State, Vec<mcp_schema::PromptMessage>> + Send + Sync>,
}

impl<State: Send + Sync + 'static> Prompt<State> {
    pub fn builder() -> PromptBuilder<State> {
        PromptBuilder::new()
    }
}

impl<State: Send + Sync + 'static> HandlerFn<State, mcp_schema::GetPromptResult> for Prompt<State> {
    fn run(
        &self,
        state: State,
        args: HandlerArgs,
    ) -> Pin<Box<dyn Future<Output = Result<mcp_schema::GetPromptResult, Error>> + Send>> {
        let description = self.description.clone();
        let messages = self.handler.run(state, args);
        Box::pin(async move {
            let messages = messages.await?;

            Ok(mcp_schema::GetPromptResult {
                meta: None,
                description,
                messages,
                extra: HashMap::new(),
            })
        })
    }
}

impl<State> TryFrom<&Prompt<State>> for mcp_schema::Prompt {
    type Error = serde_json::Error;

    fn try_from(tool: &Prompt<State>) -> Result<Self, Self::Error> {
        Ok(Self {
            description: tool.description.clone(),
            arguments: Some(tool.schema.clone()),
            name: tool.name.clone(),
            extra: HashMap::new(),
        })
    }
}

/// A builder for constructing a tool with validation and metadata
pub struct PromptBuilder<State> {
    name: Option<String>,
    description: Option<String>,
    schema: Option<Vec<mcp_schema::PromptArgument>>,
    handler: Option<Box<dyn HandlerFn<State, Vec<mcp_schema::PromptMessage>> + Send + Sync>>,
}

impl<State: Send + Sync + 'static> PromptBuilder<State> {
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

    pub fn handler<I>(
        mut self,
        handler: impl AsyncFnExt<State, I, Vec<mcp_schema::PromptMessage>>
        + Send
        + Sync
        + Copy
        + 'static,
    ) -> Self
    where
        I: DeserializeOwned + schemars::JsonSchema + Send + 'static,
    {
        self.schema = Some(
            schemars::schema_for!(I)
                .schema
                .object
                .map(|object| {
                    object
                        .properties
                        .into_iter()
                        .flat_map(|(name, schema)| match schema {
                            Schema::Bool(_) => None,
                            Schema::Object(object) => {
                                let (valid, required) = match object.instance_type {
                                    Some(SingleOrVec::Single(x)) => {
                                        (*x == InstanceType::String, true)
                                    }
                                    Some(SingleOrVec::Vec(x)) => (
                                        matches!(
                                            x.as_slice(),
                                            &[InstanceType::String, InstanceType::Null]
                                        ),
                                        false,
                                    ),
                                    _ => (false, false),
                                };

                                assert!(
                                    valid,
                                    "prompt parameter '{name}' must be String or Option<String>"
                                );

                                Some(mcp_schema::PromptArgument {
                                    name,
                                    description: None,
                                    required: Some(required),
                                    extra: HashMap::new(),
                                })
                            }
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or(Vec::new()),
        );
        self.handler = Some(Box::new(handler.handler()));
        self
    }

    pub fn build(self) -> Result<Prompt<State>, Error> {
        Ok(Prompt {
            name: self.name.unwrap_or_else(|| "unnamed prompt".to_string()),
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

impl<State> Default for PromptBuilder<State> {
    fn default() -> Self {
        Self {
            name: None,
            description: None,
            schema: None,
            handler: None,
        }
    }
}
