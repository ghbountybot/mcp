use eyre::{Result, WrapErr};
use mcp::{
    message::CallToolResult,
    server::{McpServer, ServerConfig},
    tool::{Tool, ToolRegistry, text_content},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::env;
use async_trait::async_trait;

/// Weather input parameters
#[derive(Deserialize, JsonSchema)]
struct WeatherInput {
    city: String,
}

/// Weather output
#[derive(Serialize)]
struct WeatherOutput {
    temperature: f64,
    description: String,
    city: String,
}

/// Application state
#[derive(Clone)]
struct AppState {
    api_key: String,
    client: reqwest::Client,
}

/// Weather tool implementation
struct WeatherTool {
    state: AppState,
}

#[async_trait]
impl Tool for WeatherTool {
    fn name(&self) -> &str {
        "weather"
    }
    
    fn description(&self) -> &str {
        "Get the current weather for a city"
    }
    
    fn input_schema(&self) -> serde_json::Value {
        let schema = schemars::schema_for!(WeatherInput);
        serde_json::to_value(schema).unwrap_or_default()
    }
    
    async fn call(&self, args: serde_json::Value) -> Result<CallToolResult> {
        let input: WeatherInput = serde_json::from_value(args)?;
        let result = get_weather(&self.state, input).await?;
        
        // Convert the result to a CallToolResult
        let content = vec![text_content(
            format!("Weather in {}: {}°C, {}", 
                result.city, 
                result.temperature, 
                result.description
            )
        )];
        
        Ok(CallToolResult {
            content,
            is_error: false,
        })
    }
}

/// Main function
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing for logging
    tracing_subscriber::fmt::init();
    
    // Get the OpenWeather API key from environment
    let api_key = env::var("OPENWEATHER_API_KEY")
        .expect("OPENWEATHER_API_KEY environment variable not set");
    
    // Create the application state
    let state = AppState {
        api_key,
        client: reqwest::Client::new(),
    };
    
    // Create a tool registry
    let mut registry = ToolRegistry::new();
    
    // Create and register the weather tool
    let weather_tool = WeatherTool { state };
    registry.register(weather_tool);
    
    // Create and start the server
    let config = ServerConfig {
        name: "weather-server".to_string(),
        version: "0.1.0".to_string(),
        protocol_version: "2024-11-05".to_string(),
        host: "127.0.0.1".to_string(),
        port: 3000,
    };
    
    let server = McpServer::new(config, registry);
    
    println!("Starting weather server on http://127.0.0.1:3000");
    println!("Use the following endpoints:");
    println!("  - POST /api/message: Send MCP messages");
    println!("  - GET /api/events: Connect to SSE events");
    println!();
    println!("Example tool call:");
    println!("{{\"method\":\"tools/call\",\"params\":{{\"name\":\"weather\",\"arguments\":{{\"city\":\"London\"}}}}}}");
    
    server.start().await?;
    
    Ok(())
}

/// Get weather for a city
async fn get_weather(state: &AppState, input: WeatherInput) -> Result<WeatherOutput> {
    let api_key = &state.api_key;
    let url = format!(
        "https://api.openweathermap.org/data/2.5/weather?q={city}&appid={api_key}&units=metric",
        city = input.city,
    );

    let response = state
        .client
        .get(&url)
        .send()
        .await
        .wrap_err("Failed to make weather API request")?;

    let data: serde_json::Value = response
        .json()
        .await
        .wrap_err("Failed to parse weather API response")?;

    let temp = data["main"]["temp"]
        .as_f64()
        .ok_or_else(|| eyre::eyre!("Temperature data not found"))?;
        
    let description = data["weather"][0]["description"]
        .as_str()
        .ok_or_else(|| eyre::eyre!("Weather description not found"))?
        .to_string();

    Ok(WeatherOutput {
        temperature: temp,
        description,
        city: input.city,
    })
} 