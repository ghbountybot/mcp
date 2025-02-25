use async_trait::async_trait;
use eyre::Result;
use mcp::{
    define_tool, tool,
    message::CallToolResult,
    tool::{ToolHandler, ToolRegistry, text_content},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Deserialize, JsonSchema)]
struct EchoInput {
    message: String,
}

#[derive(Serialize)]
#[allow(dead_code)]
struct EchoOutput {
    response: String,
}

// Test the define_tool macro
#[tokio::test]
async fn test_define_tool_macro() {
    let echo_tool = define_tool! {
        name: "echo",
        description: "Echo back the input message",
        input: EchoInput,
        handler: |args| async move {
            let input: EchoInput = serde_json::from_value(args)?;
            
            let content = vec![text_content(format!("Echo: {}", input.message))];
            
            Ok(CallToolResult {
                content,
                is_error: false,
            })
        }
    };
    
    let mut registry = ToolRegistry::new();
    registry.register(echo_tool);
    
    let args = json!({
        "message": "Hello, world!"
    });
    
    let result = registry.call_tool("echo", Some(args)).await.unwrap();
    
    match &result.content[0] {
        mcp::message::Content::Text(text) => assert_eq!(text.text, "Echo: Hello, world!"),
        _ => panic!("Expected text content"),
    }
}

// Test the tool attribute macro
#[tool(name = "greeting", description = "Generate a greeting message")]
struct GreetingTool;

#[async_trait]
impl ToolHandler for GreetingTool {
    type Input = EchoInput;
    
    async fn handle(&self, input: Self::Input) -> Result<CallToolResult> {
        let content = vec![text_content(format!("Greeting: {}", input.message))];
        
        Ok(CallToolResult {
            content,
            is_error: false,
        })
    }
}

#[tokio::test]
async fn test_tool_attribute_macro() {
    let mut registry = ToolRegistry::new();
    registry.register(GreetingTool);
    
    let args = json!({
        "message": "Hello from attribute macro!"
    });
    
    let result = registry.call_tool("greeting", Some(args)).await.unwrap();
    
    match &result.content[0] {
        mcp::message::Content::Text(text) => {
            assert_eq!(text.text, "Greeting: Hello from attribute macro!")
        },
        _ => panic!("Expected text content"),
    }
}
