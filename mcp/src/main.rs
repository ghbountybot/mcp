#![feature(async_fn_traits, unboxed_closures)]
#![allow(clippy::unused_async)]

use axum::{
    Router,
    routing::{get, post},
};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing_subscriber::{fmt, prelude::*};

use crate::rpc::McpImpl;

mod basic_service;
mod error;
mod registry;
mod rpc;
mod service;

pub use error::Error;
pub use registry::{Prompt, PromptRegistry, Tool, ToolRegistry};
pub use service::Service;

// TODO: Remove clone requirement
#[derive(Copy, Clone)]
struct State;

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(tracing_subscriber::filter::LevelFilter::TRACE)
        .init();

    let service = basic_service::BasicService::new(State);

    let state = McpImpl::new(service);

    let app = Router::new()
        .route("/api/message", post(McpImpl::message_handler))
        .route("/api/events", get(McpImpl::sse_handler))
        .layer(CorsLayer::permissive())
        .with_state(Arc::new(state));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::info!("listening on {}", addr);
    axum::serve(tokio::net::TcpListener::bind(addr).await.unwrap(), app)
        .await
        .unwrap();
}
