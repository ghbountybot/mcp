use eyre::Result;
use mcp::client::{ClientConfig, McpClient};
use serde_json::Value;
use std::env;
use tokio::io::{AsyncBufReadExt, BufReader, AsyncWriteExt};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing for logging
    tracing_subscriber::fmt::init();
    
    // Get the server URL from environment or use default
    let server_url = env::var("MCP_SERVER_URL")
        .unwrap_or_else(|_| "http://localhost:3000".to_string());
    
    // Create a client configuration
    let config = ClientConfig {
        server_url: server_url.clone(),
        protocol_version: "2024-11-05".to_string(),
    };
    
    // Create a client
    let client = McpClient::new(config);
    
    // Initialize the client
    println!("Connecting to MCP server at {}...", server_url);
    client.initialize().await?;
    println!("Connected successfully!");
    
    // List available tools
    let tools = client.list_tools().await?;
    
    println!("\nAvailable tools:");
    for tool in &tools {
        println!("- {} : {}", tool.name, tool.description);
    }
    
    // Interactive CLI loop
    println!("\nMCP CLI - Enter commands or 'help' for assistance (Ctrl+C to exit)");
    
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let reader = BufReader::new(stdin);
    let mut lines = reader.lines();
    
    loop {
        stdout.write_all(b"> ").await?;
        stdout.flush().await?;
        
        let line = match lines.next_line().await {
            Ok(Some(line)) => line.trim().to_string(),
            Ok(None) => break, // EOF
            Err(e) => {
                eprintln!("Error reading input: {}", e);
                continue;
            }
        };
        
        if line.is_empty() {
            continue;
        }
        
        match line.as_str() {
            "help" => {
                println!("Available commands:");
                println!("  help                - Show this help message");
                println!("  list                - List available tools");
                println!("  call <tool> <args>  - Call a tool with JSON arguments");
                println!("  ping                - Send a ping request");
                println!("  exit                - Exit the CLI");
            },
            "list" => {
                let tools = client.list_tools().await?;
                
                println!("Available tools:");
                for tool in &tools {
                    println!("- {} : {}", tool.name, tool.description);
                    println!("  Schema: {}", tool.input_schema);
                }
            },
            "ping" => {
                println!("Sending ping...");
                client.ping().await?;
                println!("Ping successful!");
            },
            "exit" => {
                println!("Exiting...");
                break;
            },
            cmd if cmd.starts_with("call ") => {
                let parts: Vec<&str> = cmd.splitn(3, ' ').collect();
                
                if parts.len() < 2 {
                    println!("Usage: call <tool> <args>");
                    continue;
                }
                
                let tool_name = parts[1];
                let args_str = parts.get(2).unwrap_or(&"{}");
                
                // Parse arguments as JSON
                let args: Value = match serde_json::from_str(args_str) {
                    Ok(args) => args,
                    Err(e) => {
                        println!("Error parsing arguments: {}", e);
                        println!("Arguments should be valid JSON, e.g. {{\"city\": \"London\"}}");
                        continue;
                    }
                };
                
                println!("Calling tool '{}' with arguments: {}", tool_name, args);
                
                match client.call_tool(tool_name, Some(args)).await {
                    Ok(result) => {
                        println!("Result:");
                        for content in result.content {
                            match content {
                                mcp::message::Content::Text(text) => {
                                    println!("{}", text.text);
                                },
                                mcp::message::Content::Image(image) => {
                                    println!("Image: {} ({})", image.url, image.mime_type);
                                },
                                mcp::message::Content::Resource(resource) => {
                                    println!("Resource: {} ({})", resource.url, 
                                        resource.mime_type.unwrap_or_else(|| "unknown type".to_string()));
                                },
                            }
                        }
                    },
                    Err(e) => {
                        println!("Error calling tool: {}", e);
                    }
                }
            },
            _ => {
                println!("Unknown command. Type 'help' for assistance.");
            }
        }
    }
    
    Ok(())
} 