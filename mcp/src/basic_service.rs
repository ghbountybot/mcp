use crate::{Error, Prompt, PromptRegistry, Service, Tool, ToolRegistry};
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
        request: mcp_schema::InitializeParams,
    ) -> Result<mcp_schema::InitializeResult, Error> {
        // TODO: check for compatible MCP version
        // TODO: Implement more capabilities
        Ok(mcp_schema::InitializeResult {
            capabilities: mcp_schema::ServerCapabilities {
                experimental: None,
                logging: None,
                prompts: None,
                resources: None,
                tools: None,
                extra: HashMap::new(),
            },
            instructions: self.instructions.clone(),
            meta: None,
            protocol_version: mcp_schema::LATEST_PROTOCOL_VERSION.to_string(),
            server_info: mcp_schema::Implementation {
                name: "BasicServer".to_string(),
                version: "0.1.0".to_string(),
                extra: HashMap::new(),
            },
            extra: HashMap::new(),
        })
    }

    async fn ping(&self, _: mcp_schema::PingParams) -> Result<mcp_schema::EmptyResult, Error> {
        Ok(mcp_schema::EmptyResult {
            meta: None,
            extra: HashMap::new(),
        })
    }

    async fn list_resources(
        &self,
        request: mcp_schema::PaginatedParams,
    ) -> Result<mcp_schema::ListResourcesResult, Error> {
        todo!()
    }

    async fn list_resource_templates(
        &self,
        request: mcp_schema::PaginatedParams,
    ) -> Result<mcp_schema::ListResourceTemplatesResult, Error> {
        todo!()
    }

    async fn read_resource(
        &self,
        request: mcp_schema::ReadResourceParams,
    ) -> Result<mcp_schema::ReadResourceResult, Error> {
        todo!()
    }

    async fn subscribe(
        &self,
        request: mcp_schema::SubscribeParams,
    ) -> Result<mcp_schema::EmptyResult, Error> {
        todo!()
    }

    async fn unsubscribe(
        &self,
        request: mcp_schema::UnsubscribeParams,
    ) -> Result<mcp_schema::EmptyResult, Error> {
        todo!()
    }

    async fn list_prompts(
        &self,
        request: mcp_schema::PaginatedParams,
    ) -> Result<mcp_schema::ListPromptsResult, Error> {
        let result = mcp_schema::ListPromptsResult {
            meta: None,
            next_cursor: None,
            prompts: self
                .prompt_registry
                .prompts_iter()
                .map(|(_, prompt)| mcp_schema::Prompt::try_from(prompt))
                .collect::<Result<Vec<_>, _>>()?,
            extra: HashMap::new(),
        };
        Ok(result)
    }

    async fn get_prompt(
        &self,
        request: mcp_schema::GetPromptParams,
    ) -> Result<mcp_schema::GetPromptResult, Error> {
        self.prompt_registry.get_prompt(&self.state, &request).await
    }

    async fn list_tools(
        &self,
        request: mcp_schema::PaginatedParams,
    ) -> Result<mcp_schema::ListToolsResult, Error> {
        let result = mcp_schema::ListToolsResult {
            meta: None,
            next_cursor: None,
            tools: self
                .tool_registry
                .tools_iter()
                .map(|(_, tool): (_, &Tool<State>)| mcp_schema::Tool::try_from(tool))
                .collect::<Result<Vec<_>, _>>()?,
            extra: HashMap::new(),
        };
        Ok(result)
    }

    async fn call_tool(
        &self,
        request: mcp_schema::CallToolParams,
    ) -> Result<mcp_schema::CallToolResult, Error> {
        self.tool_registry.call_tool(&self.state, &request).await
    }

    async fn set_level(
        &self,
        request: mcp_schema::SetLevelParams,
    ) -> Result<mcp_schema::EmptyResult, Error> {
        todo!()
    }
}
