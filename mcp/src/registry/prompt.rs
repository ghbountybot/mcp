use crate::Error;
use crate::registry::{AsyncFnExt, HandlerArgs, HandlerFn, HandlerRegistry};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;

/// A registry for managing available prompts with shared state
pub struct PromptRegistry<State> {
    registry: HandlerRegistry<Prompt<State>>,
}

impl<State: Send + Sync + 'static> PromptRegistry<State> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new prompt with the given name and handler
    pub fn register<I, O>(
        &mut self,
        name: impl Into<String>,
        description: impl Into<String>,
        handler: impl AsyncFnExt<State, I, O> + Send + Sync + Copy + 'static,
    ) where
        I: DeserializeOwned + schemars::JsonSchema + Send + 'static,
        O: Serialize + 'static,
    {
        let name = name.into();
        let description = description.into();
        let schema = serde_json::to_value(schemars::schema_for!(I)).unwrap();

        self.registry.register(
            name.clone(),
            Prompt {
                name,
                description,
                schema,
                handler: Box::new(PromptHandler {
                    handler: handler.handler(),
                    phantom: PhantomData,
                }),
            },
        );
    }

    /// Gets a prompt by name with the given arguments
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
                .into_iter()
                .flatten()
                .map(|(key, value)| (key, serde_json::Value::String(value)))
                .collect::<HashMap<_, _>>(),
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

struct PromptHandler<F, O> {
    handler: F,
    phantom: PhantomData<fn() -> O>,
}

impl<State, F, O> HandlerFn<State, String> for PromptHandler<F, O>
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

pub struct Prompt<State> {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) schema: serde_json::Value,
    handler: Box<dyn HandlerFn<State, String> + Send + Sync>,
}

impl<State: Send + Sync + 'static> HandlerFn<State, mcp_schema::GetPromptResult> for Prompt<State> {
    fn run(
        &self,
        state: State,
        args: HandlerArgs,
    ) -> Pin<Box<dyn Future<Output = Result<mcp_schema::GetPromptResult, Error>> + Send>> {
        let description = self.description.clone();
        let result = self.handler.run(state, args);
        Box::pin(async move {
            let result = result.await?;
            let result = serde_json::to_string(&result).unwrap();
            let result = mcp_schema::TextContent {
                kind: "json".to_string(),
                text: result,
                annotated: mcp_schema::Annotated {
                    annotations: None,
                    extra: HashMap::new(),
                },
            };

            let result = mcp_schema::PromptMessage {
                content: mcp_schema::PromptContent::Text(result),
                role: mcp_schema::Role::Assistant,
            };

            Ok(mcp_schema::GetPromptResult {
                meta: None,
                description: Some(description),
                messages: vec![result],
                extra: HashMap::new(),
            })
        })
    }
}

impl<State> TryFrom<&Prompt<State>> for mcp_schema::Prompt {
    type Error = serde_json::Error;

    fn try_from(prompt: &Prompt<State>) -> Result<Self, Self::Error> {
        Ok(Self {
            name: prompt.name.clone(),
            description: Some(prompt.description.clone()),
            arguments: serde_json::from_value(prompt.schema.clone())?,
            extra: HashMap::new(),
        })
    }
}

/// A builder for constructing a prompt with validation and metadata
pub struct PromptBuilder {
    name: String,
    description: Option<String>,
    required_args: Vec<String>,
    handler: Option<
        Box<
            dyn Fn(
                &HashMap<String, serde_json::Value>,
            ) -> Result<mcp_schema::GetPromptResult, Error>,
        >,
    >,
}

impl PromptBuilder {
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
        F: Fn(&HashMap<String, serde_json::Value>) -> Result<mcp_schema::GetPromptResult, Error>
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

    // pub fn register(self, registry: &mut PromptRegistry) -> Result<(), Error> {
    //     let handler = self.handler.ok_or_else(|| Error {
    //         message: "Prompt handler not set".to_string(),
    //         code: 500,
    //     })?;
    //
    //     registry.register(self.name, handler);
    //     Ok(())
    // }
}
