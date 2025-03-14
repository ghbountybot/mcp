use crate::{Error, Prompt, PromptRegistry, Service, Tool, ToolRegistry, schema};
use std::collections::HashMap;

struct BasicService<State> {
    state: State,
    instructions: Option<String>,
    tool_registry: ToolRegistry<State>,
    prompt_registry: PromptRegistry<State>,
}

impl<State> Service for BasicService<State> {
    async fn init(
        &self,
        request: schema::InitializeRequest,
    ) -> Result<schema::InitializeResult, Error> {
        // TODO: check for compatible MCP version
        // TODO: Implement more capabilities
        Ok(schema::InitializeResult {
            capabilities: schema::ServerCapabilities {
                experimental: HashMap::new(),
                logging: serde_json::Map::new(),
                prompts: None,
                resources: None,
                tools: Some(schema::ServerCapabilitiesTools {
                    list_changed: Some(false),
                }),
            },
            instructions: self.instructions.clone(),
            meta: serde_json::Map::new(),
            protocol_version: "1.0.0".to_string(),
            server_info: schema::Implementation {
                name: "BasicServer".to_string(),
                version: "0.1.0".to_string(),
            },
        })
    }

    async fn ping(&self, _: schema::PingRequest) -> Result<schema::Result, Error> {
        Ok(schema::Result::default())
    }

    async fn list_resources(
        &self,
        request: schema::ListResourcesRequest,
    ) -> Result<schema::ListResourcesResult, Error> {
        todo!()
    }

    async fn list_resource_templates(
        &self,
        request: schema::ListResourceTemplatesRequest,
    ) -> Result<schema::ListResourceTemplatesResult, Error> {
        todo!()
    }

    async fn read_resource(
        &self,
        request: schema::ReadResourceRequest,
    ) -> Result<schema::ReadResourceResult, Error> {
        todo!()
    }

    async fn subscribe(&self, request: schema::SubscribeRequest) -> Result<schema::Result, Error> {
        todo!()
    }

    async fn unsubscribe(
        &self,
        request: schema::UnsubscribeRequest,
    ) -> Result<schema::Result, Error> {
        todo!()
    }

    async fn list_prompts(
        &self,
        request: schema::ListPromptsRequest,
    ) -> Result<schema::ListPromptsResult, Error> {
        let result = schema::ListPromptsResult {
            meta: serde_json::Map::new(),
            next_cursor: None,
            prompts: self
                .prompt_registry
                .prompts_iter()
                .map(|(_, prompt)| schema::Prompt::try_from(prompt))
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(result)
    }

    async fn get_prompt(
        &self,
        request: schema::GetPromptRequest,
    ) -> Result<schema::GetPromptResult, Error> {
        self.prompt_registry.get_prompt(&self.state, &request).await
    }

    async fn list_tools(
        &self,
        request: schema::ListToolsRequest,
    ) -> Result<schema::ListToolsResult, Error> {
        let result = schema::ListToolsResult {
            meta: serde_json::Map::new(),
            next_cursor: None,
            tools: self
                .tool_registry
                .tools_iter()
                .map(|(_, tool): (_, &Tool<State>)| schema::Tool::try_from(tool))
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(result)
    }

    async fn call_tool(
        &self,
        request: schema::CallToolRequest,
    ) -> Result<schema::CallToolResult, Error> {
        self.tool_registry.call_tool(&self.state, &request).await
    }

    async fn set_level(&self, request: schema::SetLevelRequest) -> Result<schema::Result, Error> {
        todo!()
    }
}
