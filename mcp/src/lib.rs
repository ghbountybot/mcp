pub mod basic_service;
pub mod registry;
pub mod rpc;
pub mod schema;
pub use registry::{Tool, ToolRegistry};

#[derive(Debug)]
pub struct Error {
    pub message: String,
    pub code: i32,
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self {
            message: format!("{error}"),
            code: 500,
        }
    }
}

trait Service {
    async fn init(
        &self,
        request: schema::InitializeRequest,
    ) -> Result<schema::InitializeResult, Error>;
    async fn ping(&self, _: schema::PingRequest) -> Result<schema::Result, Error> {
        Ok(schema::Result::default())
    }

    async fn list_resources(
        &self,
        request: schema::ListResourcesRequest,
    ) -> Result<schema::ListResourcesResult, Error>;

    async fn list_resource_templates(
        &self,
        request: schema::ListResourceTemplatesRequest,
    ) -> Result<schema::ListResourceTemplatesResult, Error>;

    async fn read_resource(
        &self,
        request: schema::ReadResourceRequest,
    ) -> Result<schema::ReadResourceResult, Error>;

    async fn subscribe(&self, request: schema::SubscribeRequest) -> Result<schema::Result, Error>;

    async fn unsubscribe(
        &self,
        request: schema::UnsubscribeRequest,
    ) -> Result<schema::Result, Error> {
        Ok(schema::Result::default())
    }

    async fn list_prompts(
        &self,
        request: schema::ListPromptsRequest,
    ) -> Result<schema::ListPromptsResult, Error>;

    async fn get_prompt(
        &self,
        request: schema::GetPromptRequest,
    ) -> Result<schema::GetPromptResult, Error>;

    async fn list_tools(
        &self,
        request: schema::ListToolsRequest,
    ) -> Result<schema::ListToolsResult, Error>;

    async fn call_tool(
        &self,
        request: schema::CallToolRequest,
    ) -> Result<schema::CallToolResult, Error>;

    async fn set_level(&self, request: schema::SetLevelRequest) -> Result<schema::Result, Error>;
}

async fn handle_request(
    service: &impl Service,
    request: schema::ClientRequest,
) -> Result<schema::ServerResult, Error> {
    match request {
        schema::ClientRequest::InitializeRequest(r) => service
            .init(r)
            .await
            .map(schema::ServerResult::InitializeResult),
        schema::ClientRequest::PingRequest(r) => {
            service.ping(r).await.map(schema::ServerResult::Result)
        }
        schema::ClientRequest::ListResourcesRequest(r) => service
            .list_resources(r)
            .await
            .map(schema::ServerResult::ListResourcesResult),
        schema::ClientRequest::ListResourceTemplatesRequest(r) => service
            .list_resource_templates(r)
            .await
            .map(schema::ServerResult::ListResourceTemplatesResult),
        schema::ClientRequest::ReadResourceRequest(r) => service
            .read_resource(r)
            .await
            .map(schema::ServerResult::ReadResourceResult),
        schema::ClientRequest::SubscribeRequest(r) => {
            service.subscribe(r).await.map(schema::ServerResult::Result)
        }
        schema::ClientRequest::UnsubscribeRequest(r) => service
            .unsubscribe(r)
            .await
            .map(schema::ServerResult::Result),
        schema::ClientRequest::ListPromptsRequest(r) => service
            .list_prompts(r)
            .await
            .map(schema::ServerResult::ListPromptsResult),
        schema::ClientRequest::GetPromptRequest(r) => service
            .get_prompt(r)
            .await
            .map(schema::ServerResult::GetPromptResult),
        schema::ClientRequest::ListToolsRequest(r) => service
            .list_tools(r)
            .await
            .map(schema::ServerResult::ListToolsResult),
        schema::ClientRequest::CallToolRequest(r) => service
            .call_tool(r)
            .await
            .map(schema::ServerResult::CallToolResult),
        schema::ClientRequest::SetLevelRequest(r) => {
            service.set_level(r).await.map(schema::ServerResult::Result)
        }
        schema::ClientRequest::CompleteRequest(_) => {
            Ok(schema::ServerResult::Result(schema::Result::default()))
        }
    }
}
