pub mod basic_service;
pub mod error;
pub mod registry;
pub mod resources;
pub mod rpc;
pub mod service;

use axum::Router;
use axum::routing::{get, post};
pub use basic_service::BasicService;
pub use error::Error;
pub use registry::{Prompt, PromptRegistry, Resource, ResourceRegistry, Tool, ToolRegistry};
pub use rpc::McpImpl;
pub use service::Service;
use std::sync::Arc;
use tower_http::cors::CorsLayer;

pub async fn serve_over_stdio<S: Service + Send + Sync + 'static>(
    service: S,
) -> std::io::Result<()> {
    let service = Arc::new(McpImpl::new(service));
    service.serve_over_stdio().await
}

pub async fn serve_over_sse<S: Service + Send + Sync + 'static>(
    listener: tokio::net::TcpListener,
    service: S,
) -> std::io::Result<()> {
    let service = Arc::new(McpImpl::new(service));

    let app = Router::new()
        .route("/api/message", post(McpImpl::message_handler))
        .route("/api/events", get(McpImpl::sse_handler))
        .layer(CorsLayer::permissive())
        .with_state(service);

    axum::serve(listener, app).await
}
