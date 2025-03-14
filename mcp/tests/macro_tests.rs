use eyre::WrapErr;
use mcp::ToolRegistry;
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Deserialize, JsonSchema)]
struct WeatherInput {
    city: String,
}

#[derive(Clone)]
struct AppState {
    api_key: String,
    client: reqwest::Client,
}

async fn weather(
    state: &AppState,
    WeatherInput { city }: WeatherInput,
) -> Result<String, mcp::Error> {
    let api_key = &state.api_key;
    let url = format!(
        "https://api.openweathermap.org/data/2.5/weather?q={city}&appid={api_key}&units=metric",
    );

    let response = state
        .client
        .get(&url)
        .send()
        .await
        .wrap_err("Failed to make weather API request")
        .unwrap();

    let data: serde_json::Value = response
        .json()
        .await
        .wrap_err("Failed to parse weather API response")
        .unwrap();

    let temp = data["main"]["temp"]
        .as_f64()
        .ok_or_else(|| eyre::eyre!("Temperature data not found"))
        .unwrap();
    let description = data["weather"][0]["description"]
        .as_str()
        .ok_or_else(|| eyre::eyre!("Weather description not found"))
        .unwrap();

    Ok(format!("Weather in {}: {}°C, {}", city, temp, description))
}

async fn create_registry() {
    let state = AppState {
        api_key: std::env::var("OPENWEATHER_API_KEY")
            .expect("OPENWEATHER_API_KEY environment variable not set"),
        client: reqwest::Client::new(),
    };
    let mut registry = ToolRegistry::new();
    registry.register("weather", weather);
}
