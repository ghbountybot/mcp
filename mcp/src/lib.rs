#![feature(async_fn_traits, unboxed_closures)]

pub mod basic_service;
pub mod error;
pub mod registry;
pub mod rpc;
pub mod service;

pub use basic_service::BasicService;
pub use error::Error;
pub use registry::{Prompt, PromptRegistry, Tool, ToolRegistry};
pub use rpc::McpImpl;
pub use service::Service;
