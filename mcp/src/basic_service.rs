use crate::{Error, Service, builder, builder::ToolRegistry, schema};

struct BasicService<State> {
    tool_registry: ToolRegistry<State>,
}

impl<State> Service for BasicService<State> {
    fn init(&self, request: schema::InitializeRequest) -> Result<schema::InitializeResult, Error> {
        todo!()
    }

    fn ping(&self, _: schema::PingRequest) -> Result<schema::Result, Error> {
        todo!()
    }

    fn list_resources(
        &self,
        request: schema::ListResourcesRequest,
    ) -> Result<schema::ListResourcesResult, Error> {
        todo!()
    }

    fn list_resource_templates(
        &self,
        request: schema::ListResourceTemplatesRequest,
    ) -> Result<schema::ListResourceTemplatesResult, Error> {
        todo!()
    }

    fn read_resource(
        &self,
        request: schema::ReadResourceRequest,
    ) -> Result<schema::ReadResourceResult, Error> {
        todo!()
    }

    fn subscribe(&self, request: schema::SubscribeRequest) -> Result<schema::Result, Error> {
        todo!()
    }

    fn unsubscribe(&self, request: schema::UnsubscribeRequest) -> Result<schema::Result, Error> {
        todo!()
    }

    fn list_prompts(
        &self,
        request: schema::ListPromptsRequest,
    ) -> Result<schema::ListPromptsResult, Error> {
        todo!()
    }

    fn get_prompt(
        &self,
        request: schema::GetPromptRequest,
    ) -> Result<schema::GetPromptResult, Error> {
        todo!()
    }

    fn list_tools(
        &self,
        request: schema::ListToolsRequest,
    ) -> Result<schema::ListToolsResult, Error> {
        let result = schema::ListToolsResult {
            meta: serde_json::Map::new(),
            next_cursor: None,
            tools: self
                .tool_registry
                .tools_iter()
                .map(|(_, tool)| schema::Tool::try_from(tool))
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(result)
    }

    fn call_tool(&self, request: schema::CallToolRequest) -> Result<schema::CallToolResult, Error> {
        todo!()
    }

    fn set_level(&self, request: schema::SetLevelRequest) -> Result<schema::Result, Error> {
        todo!()
    }
}
