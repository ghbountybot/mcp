use eyre::Result;
use mcp::client::{ClientConfig, McpClient};
use serde_json::json;
use std::env;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing for logging
    tracing_subscriber::fmt::init();
    
    // Get the server URL from environment or use default
    let server_url = env::var("MCP_SERVER_URL")
        .unwrap_or_else(|_| "http://localhost:3000".to_string());
    
    // Create a client configuration
    let config = ClientConfig {
        server_url,
        protocol_version: "2024-11-05".to_string(),
    };
    
    // Create a client
    let client = McpClient::new(config);
    
    // Initialize the client
    println!("Initializing client...");
    client.initialize().await?;
    println!("Client initialized successfully!");
    
    // List available tools
    println!("\nListing available tools:");
    let tools = client.list_tools().await?;
    
    for tool in &tools {
        println!("- {} : {}", tool.name, tool.description);
    }
    
    // Check if the weather tool is available
    if tools.iter().any(|t| t.name == "weather") {
        // Call the weather tool
        println!("\nCalling weather tool for London...");
        
        let result = client.call_tool("weather", Some(json!({
            "city": "London"
        }))).await?;
        
        println!("Weather tool result:");
        for content in result.content {
            match content {
                mcp::message::Content::Text(text) => {
                    println!("{}", text.text);
                },
                _ => println!("Non-text content received"),
            }
        }
    } else {
        println!("\nWeather tool not available on this server.");
    }
    
    // Send a ping
    println!("\nSending ping...");
    client.ping().await?;
    println!("Ping successful!");
    
    Ok(())
} 