pub mod prompt;
pub mod tool;

use crate::{Error, schema};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Map;
use std::collections::HashMap;
use std::future::Future;
use std::marker::PhantomData;
use std::ops::AsyncFn;
use std::pin::Pin;

pub use prompt::{Prompt, PromptRegistry};
pub use tool::{Tool, ToolRegistry};

pub type HandlerArgs = Map<String, serde_json::Value>;

pub trait HandlerFn<State, O> {
    fn run<'a>(
        &'a self,
        state: &'a State,
        input: HandlerArgs,
    ) -> Pin<Box<dyn Future<Output = Result<O, Error>> + 'a>>;
}

pub trait AsyncFnExt<State, I, O>: AsyncFn(&State, I) -> Result<O, Error> {
    fn handler<'a>(self) -> impl HandlerFn<State, O> + 'a
    where
        Self: Sized + 'a,
        I: DeserializeOwned + 'a,
    {
        WrappedAsyncFn {
            handler: self,
            phantom: PhantomData,
        }
    }
}

impl<State, I, O, F> AsyncFnExt<State, I, O> for F where F: AsyncFn(&State, I) -> Result<O, Error> {}

/// This wrapper is used to wrap an [`AsyncFn`] and implement [`HandlerFn`]. This is needed to
/// store the I generic
struct WrappedAsyncFn<F, I> {
    handler: F,
    phantom: PhantomData<I>,
}

impl<State, F, I, O> HandlerFn<State, O> for WrappedAsyncFn<F, I>
where
    F: AsyncFn(&State, I) -> Result<O, Error>,
    I: DeserializeOwned,
{
    fn run<'a>(
        &'a self,
        state: &'a State,
        args: HandlerArgs,
    ) -> Pin<Box<dyn Future<Output = Result<O, Error>> + 'a>> {
        Box::pin(async move {
            let input =
                serde_json::from_value(serde_json::Value::Object(args)).map_err(|e| Error {
                    message: format!("Failed to deserialize arguments: {e}"),
                    code: 400,
                })?;

            (self.handler)(state, input).await
        })
    }
}

/// A registry for managing available handlers
pub(crate) struct HandlerRegistry<Handler> {
    handlers: HashMap<String, Handler>,
}

impl<Handler> HandlerRegistry<Handler> {
    /// Register a new handler with the given name and handler
    pub fn register(&mut self, name: String, handler: Handler) {
        self.handlers.insert(name, handler);
    }

    /// Call a handler by name with the given arguments
    pub async fn call<State, O>(
        &self,
        state: &State,
        name: &str,
        args: Map<String, serde_json::Value>,
    ) -> Result<O, Error>
    where
        Handler: HandlerFn<State, O>,
    {
        let handler = self.handlers.get(name).ok_or_else(|| Error {
            message: format!("Handler '{}' not found", name),
            code: 404,
        })?;

        handler.run(state, args).await
    }

    /// Iterate through all registered handlers
    pub fn handlers_iter(&self) -> impl Iterator<Item = (&String, &Handler)> {
        self.handlers.iter()
    }
}

impl<Handler> Default for HandlerRegistry<Handler> {
    fn default() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }
}
