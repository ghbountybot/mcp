#![allow(clippy::unused_async)]

use axum::{
    Router,
    routing::{get, post},
};
use futures::future::pending;
use mcp::registry::resource::Source;
use mcp::rpc::McpImpl;
use rand::Rng;
use schemars::JsonSchema;
use serde::Deserialize;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing_subscriber::{fmt, prelude::*};

#[derive(Clone, Default)]
struct Resource {
    inner: Arc<tokio::sync::Mutex<mcp::resources::MemoryResource>>,
}

impl<State: Send + Sync + 'static> Source<State> for Resource {
    fn read(
        &self,
        state: State,
        uri: String,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<mcp_schema::ResourceContents>, mcp::Error>> + Send>>
    {
        let inner = self.inner.clone().lock_owned();
        Box::pin(async move {
            let future = inner.await.read(state, uri);
            future.await
        })
    }

    fn wait_for_change(
        &self,
        state: State,
        uri: String,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        let inner = self.inner.clone().lock_owned();
        Box::pin(async move {
            let future = inner.await.wait_for_change(state, uri);
            future.await;
        })
    }
}

#[derive(Default, Clone)]
struct State {
    resource: Resource,
    history: Vec<f32>,
}

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

    let text;
    let resource;
    {
        let mut state = state.lock().unwrap();
        state.history.push(temperature);
        text = format!("{:?}", state.history);
        resource = state.resource.inner.clone();
    }
    resource
        .lock_owned()
        .await
        .set(vec![mcp_schema::ResourceContents::Text(
            mcp_schema::TextResourceContents {
                uri: "history://weather".to_string(),
                mime_type: None,
                text,
            },
        )]);

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
async fn main() {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(tracing_subscriber::filter::LevelFilter::TRACE)
        .init();

    let resource = Resource::default();

    let mut service = mcp::BasicService::new(
        Arc::new(std::sync::Mutex::new(State {
            resource: resource.clone(),
            history: Vec::new(),
        })),
        "weather".to_string(),
        "0.1.0".to_string(),
    );

    let forecast_tool = mcp::Tool::builder()
        .name("get_forecast")
        .description("Get weather forecast for a location")
        .handler(get_forecast)
        .build()
        .unwrap();

    let do_nothing_tool = mcp::Tool::builder()
        .name("do_nothing")
        .description("Do absolutely nothing")
        .handler(do_nothing)
        .build()
        .unwrap();

    let forecast_prompt = mcp::Prompt::builder()
        .name("forecast")
        .description("Get the forecaster prompt")
        .handler(get_forecast_prompt)
        .build()
        .unwrap();

    let resource = mcp::Resource::builder()
        .name("history")
        .fixed_uri("history://temperature")
        .description("Temperature history")
        .source(Box::new(resource))
        .build()
        .unwrap();

    let tool_registry = service.tool_registry_mut().get_mut().unwrap();
    tool_registry.register(forecast_tool);
    tool_registry.register(do_nothing_tool);

    let prompt_registry = service.prompt_registry_mut().get_mut().unwrap();
    prompt_registry.register(forecast_prompt);

    {
        let mut resource_registry = service.resource_registry().lock().unwrap();
        resource_registry.register(resource);
    }

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
