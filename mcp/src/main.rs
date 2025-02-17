#![allow(clippy::unused_async)]

use axum::{
    Router,
    routing::{get, post},
};
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;
use tracing_subscriber::{fmt, prelude::*};

use crate::rpc::McpImpl;

mod rpc;
mod schema;

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(tracing_subscriber::filter::LevelFilter::TRACE)
        .init();

    let state = McpImpl::default();

    let app = Router::new()
        .route("/api/message", post(McpImpl::message_handler))
        .route("/api/events", get(McpImpl::sse_handler))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::info!("listening on {}", addr);
    axum::serve(tokio::net::TcpListener::bind(addr).await.unwrap(), app)
        .await
        .unwrap();
}
