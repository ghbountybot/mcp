#![feature(async_fn_traits, unboxed_closures)]
#![allow(clippy::unused_async)]

use axum::{
    Router,
    routing::{get, post},
};
use rand::Rng;
use schemars::JsonSchema;
use serde::Deserialize;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing_subscriber::{fmt, prelude::*};

use mcp::rpc::McpImpl;

#[derive(Copy, Clone)]
struct State;

#[derive(Deserialize, JsonSchema)]
struct ForecastParams {
    latitude: f32,
    longitude: f32,
    temperature_multiplier: Option<f32>,
}

async fn get_forecast(
    _state: State,
    params: ForecastParams,
) -> Result<Vec<mcp_schema::PromptContent>, mcp::Error> {
    let latitude = params.latitude;
    let longitude = params.longitude;
    let temperature =
        rand::rng().random_range(-50.0..120.0) * params.temperature_multiplier.unwrap_or(1.0);
    let description = if temperature < 50.0 {
        "very cold".to_string()
    } else {
        "a bit warm".to_string()
    };

    Ok(vec![mcp_schema::PromptContent::Text(
        mcp_schema::TextContent {
            kind: "text".to_string(),
            text: format!(
                "The temperature at {latitude} {longitude} is {temperature} degrees - {description}"
            ),
            annotated: mcp_schema::Annotated {
                annotations: None,
                extra: HashMap::new(),
            },
        },
    )])
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(tracing_subscriber::filter::LevelFilter::TRACE)
        .init();

    let mut service = mcp::BasicService::new(State);

    let forecast_tool = mcp::Tool::builder()
        .name("get_forecast")
        .description("Get weather forecast for a location")
        .handler(get_forecast)
        .build()
        .unwrap();

    service
        .tool_registry_mut()
        .get_mut()
        .unwrap()
        .register(forecast_tool);

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
