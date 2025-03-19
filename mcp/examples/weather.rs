#![allow(clippy::unused_async)]

use axum::{
    Router,
    routing::{get, post},
};
use futures::future::pending;
use mcp::Resource;
use mcp::registry::resource::{ErasedSource, Source};
use mcp::resources::MemoryResource;
use mcp::rpc::McpImpl;
use mcp_schema::ResourceContents;
use rand::Rng;
use schemars::JsonSchema;
use serde::Deserialize;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing_subscriber::{fmt, prelude::*};

#[derive(Default, Clone)]
struct State {
    resource: MemoryResource,
    history: Vec<f32>,
}

type SharedState = Arc<tokio::sync::Mutex<State>>;

#[derive(Deserialize, JsonSchema)]
struct ForecastParams {
    latitude: f32,
    longitude: f32,
    temperature_multiplier: Option<f32>,
}

#[derive(Deserialize, JsonSchema)]
struct DoNothingParams {}

#[derive(Deserialize, JsonSchema)]
struct ForecastPromptParams {
    city: Option<String>,
}

const WEATHER_URI: &str = "history://weather";

async fn get_forecast(
    state: Arc<std::sync::Mutex<State>>,
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

    let mut state = state.lock().unwrap();
    state.history.push(temperature);
    let text = format!("{:?}", state.history);

    state
        .resource
        .set([ResourceContents::Text(mcp_schema::TextResourceContents {
            uri: WEATHER_URI.to_string(),
            mime_type: None,
            text,
        })]);

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

async fn do_nothing(
    _state: Arc<std::sync::Mutex<State>>,
    _params: DoNothingParams,
) -> Result<Vec<mcp_schema::PromptContent>, mcp::Error> {
    pending().await
}

async fn get_forecast_prompt(
    _state: Arc<std::sync::Mutex<State>>,
    params: ForecastPromptParams,
) -> Result<Vec<mcp_schema::PromptMessage>, mcp::Error> {
    Ok(vec![mcp_schema::PromptMessage {
        role: mcp_schema::Role::Assistant,
        content: mcp_schema::PromptContent::Text(mcp_schema::TextContent {
            kind: "text".to_string(),
            text: if let Some(city) = params.city {
                format!("You are a meteorologist with access to weather forecasts from {city}.")
            } else {
                "You are a meteorologist with access to weather forecasts from any location"
                    .to_string()
            },
            annotated: mcp_schema::Annotated {
                annotations: None,
                extra: HashMap::new(),
            },
        }),
    }])
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    const USE_STDIO: bool = true;

    tracing_subscriber::registry()
        .with(fmt::layer().with_writer(std::io::stderr))
        .with(tracing_subscriber::filter::LevelFilter::TRACE)
        .init();

    let resource = MemoryResource::default();

    let state = Arc::new(std::sync::Mutex::new(State {
        resource: resource.clone(),
        history: Vec::new(),
    }));

    let forecast_tool = mcp::Tool::builder()
        .name("get_forecast")
        .description("Get weather forecast for a location")
        .handler(get_forecast)
        .build()?;

    let do_nothing_tool = mcp::Tool::builder()
        .name("do_nothing")
        .description("Do absolutely nothing")
        .handler(do_nothing)
        .build()?;

    let forecast_prompt = mcp::Prompt::builder()
        .name("forecast")
        .description("Get the forecaster prompt")
        .handler(get_forecast_prompt)
        .build()?;

    let resource = Resource::builder()
        .name("history")
        .fixed_uri("history://temperature")
        .description("Temperature history")
        .build()?;

    let service = mcp::BasicService::new()
        .tool(forecast_tool)
        .tool(do_nothing_tool)
        .prompt(forecast_prompt)
        .fixed_resource(resource)
        .state(state);

    if USE_STDIO {
        mcp::serve_over_stdio(service).await?;
    } else {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
        mcp::serve_over_sse(listener, service).await?;
    }

    Ok(())
}
