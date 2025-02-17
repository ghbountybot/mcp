pub mod builder;
pub mod rpc;
pub mod schema;
pub use builder::ToolRegistry;
pub use mcp_macros::tool;

#[derive(Debug)]
pub struct Error {
    pub message: String,
    pub code: i32,
}

trait Service {
    fn init(&self, request: schema::InitializeRequest) -> Result<schema::InitializeResult, Error>;
    fn ping(&self, _: schema::PingRequest) -> Result<schema::Result, Error> {
        Ok(schema::Result::default())
    }

    fn list_resources(
        &self,
        request: schema::ListResourcesRequest,
    ) -> Result<schema::ListResourcesResult, Error>;

    fn list_resource_templates(
        &self,
        request: schema::ListResourceTemplatesRequest,
    ) -> Result<schema::ListResourceTemplatesResult, Error>;

    fn read_resource(
        &self,
        request: schema::ReadResourceRequest,
    ) -> Result<schema::ReadResourceResult, Error>;

    fn subscribe(&self, request: schema::SubscribeRequest) -> Result<schema::Result, Error>;

    fn unsubscribe(&self, request: schema::UnsubscribeRequest) -> Result<schema::Result, Error> {
        Ok(schema::Result::default())
    }

    fn list_prompts(
        &self,
        request: schema::ListPromptsRequest,
    ) -> Result<schema::ListPromptsResult, Error>;

    fn get_prompt(
        &self,
        request: schema::GetPromptRequest,
    ) -> Result<schema::GetPromptResult, Error>;

    fn list_tools(
        &self,
        request: schema::ListToolsRequest,
    ) -> Result<schema::ListToolsResult, Error>;

    fn call_tool(&self, request: schema::CallToolRequest) -> Result<schema::CallToolResult, Error>;

    fn set_level(&self, request: schema::SetLevelRequest) -> Result<schema::Result, Error>;
}

fn handle_request(
    service: &impl Service,
    request: schema::ClientRequest,
) -> Result<schema::ServerResult, Error> {
    match request {
        schema::ClientRequest::InitializeRequest(r) => {
            service.init(r).map(schema::ServerResult::InitializeResult)
        }
        schema::ClientRequest::PingRequest(r) => service.ping(r).map(schema::ServerResult::Result),
        schema::ClientRequest::ListResourcesRequest(r) => service
            .list_resources(r)
            .map(schema::ServerResult::ListResourcesResult),
        schema::ClientRequest::ListResourceTemplatesRequest(r) => service
            .list_resource_templates(r)
            .map(schema::ServerResult::ListResourceTemplatesResult),
        schema::ClientRequest::ReadResourceRequest(r) => service
            .read_resource(r)
            .map(schema::ServerResult::ReadResourceResult),
        schema::ClientRequest::SubscribeRequest(r) => {
            service.subscribe(r).map(schema::ServerResult::Result)
        }
        schema::ClientRequest::UnsubscribeRequest(r) => {
            service.unsubscribe(r).map(schema::ServerResult::Result)
        }
        schema::ClientRequest::ListPromptsRequest(r) => service
            .list_prompts(r)
            .map(schema::ServerResult::ListPromptsResult),
        schema::ClientRequest::GetPromptRequest(r) => service
            .get_prompt(r)
            .map(schema::ServerResult::GetPromptResult),
        schema::ClientRequest::ListToolsRequest(r) => service
            .list_tools(r)
            .map(schema::ServerResult::ListToolsResult),
        schema::ClientRequest::CallToolRequest(r) => service
            .call_tool(r)
            .map(schema::ServerResult::CallToolResult),
        schema::ClientRequest::SetLevelRequest(r) => {
            service.set_level(r).map(schema::ServerResult::Result)
        }
        schema::ClientRequest::CompleteRequest(_) => {
            Ok(schema::ServerResult::Result(schema::Result::default()))
        }
    }
}
