pub mod prompt;
pub mod tool;

use crate::Error;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;

pub use prompt::{Prompt, PromptRegistry};
pub use tool::{Tool, ToolRegistry};

pub type HandlerArgs = HashMap<String, serde_json::Value>;

pub trait HandlerFn<State, O> {
    fn run(
        &self,
        state: State,
        input: HandlerArgs,
    ) -> Pin<Box<dyn Future<Output = Result<O, Error>> + Send>>;
}

pub trait AsyncFnExt<State, I, O> {
    fn handler<'a>(self) -> impl HandlerFn<State, O> + Send + Sync + 'a
    where
        Self: 'a,
        I: 'a;
}

impl<State, I, O, Fut, F> AsyncFnExt<State, I, O> for F
where
    State: Send + Sync + 'static,
    I: DeserializeOwned + Send,
    O: 'static,
    F: Fn(State, I) -> Fut + Send + Sync + Sized,
    Fut: Future<Output = Result<O, Error>> + Send + 'static,
{
    fn handler<'a>(self) -> impl HandlerFn<State, O> + Send + Sync + 'a
    where
        Self: 'a,
        I: 'a,
    {
        WrappedAsyncFn {
            handler: self,
            phantom: PhantomData,
        }
    }
}

/// This wrapper is used to wrap an [`AsyncFn`] and implement [`HandlerFn`]. This is needed to
/// store the I generic
struct WrappedAsyncFn<F, I> {
    handler: F,
    phantom: PhantomData<fn() -> I>,
}

impl<State, I, O, Fut, F> HandlerFn<State, O> for WrappedAsyncFn<F, I>
where
    State: Send + Sync + 'static,
    I: DeserializeOwned + Send,
    O: 'static,
    F: Fn(State, I) -> Fut + Send + Sync + Sized,
    Fut: Future<Output = Result<O, Error>> + Send + 'static,
{
    fn run(
        &self,
        state: State,
        args: HandlerArgs,
    ) -> Pin<Box<dyn Future<Output = Result<O, Error>> + Send>> {
        let input = serde_json::from_value(serde_json::Value::Object(args.into_iter().collect()))
            .map_err(|e| Error {
                message: format!("Failed to deserialize arguments: {e}"),
                code: 400,
            });

        let result = input.map(|input| (self.handler)(state, input));

        Box::pin(async move { result?.await })
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
    pub fn call<State, O>(
        &self,
        state: State,
        name: &str,
        args: HashMap<String, serde_json::Value>,
    ) -> impl Future<Output = Result<O, Error>> + use<Handler, State, O> + Send + 'static
    where
        State: Sync,
        O: 'static,
        Handler: HandlerFn<State, O>,
    {
        let handler = self
            .handlers
            .get(name)
            .ok_or_else(|| Error {
                message: format!("Handler '{}' not found", name),
                code: 404,
            })
            .map(|handler| handler.run(state, args));

        Box::pin(async move { handler?.await })
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
