pub mod basic_service;
pub mod registry;
pub mod rpc;
pub use registry::{Prompt, PromptRegistry, Tool, ToolRegistry};

use std::collections::HashMap;

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
        request: mcp_schema::InitializeParams,
    ) -> Result<mcp_schema::InitializeResult, Error>;

    async fn ping(&self, _: mcp_schema::PingParams) -> Result<mcp_schema::EmptyResult, Error>;

    async fn list_resources(
        &self,
        request: mcp_schema::PaginatedParams,
    ) -> Result<mcp_schema::ListResourcesResult, Error>;

    async fn list_resource_templates(
        &self,
        request: mcp_schema::PaginatedParams,
    ) -> Result<mcp_schema::ListResourceTemplatesResult, Error>;

    async fn read_resource(
        &self,
        request: mcp_schema::ReadResourceParams,
    ) -> Result<mcp_schema::ReadResourceResult, Error>;

    async fn subscribe(
        &self,
        request: mcp_schema::SubscribeParams,
    ) -> Result<mcp_schema::EmptyResult, Error>;

    async fn unsubscribe(
        &self,
        request: mcp_schema::UnsubscribeParams,
    ) -> Result<mcp_schema::EmptyResult, Error>;

    async fn list_prompts(
        &self,
        request: mcp_schema::PaginatedParams,
    ) -> Result<mcp_schema::ListPromptsResult, Error>;

    async fn get_prompt(
        &self,
        request: mcp_schema::GetPromptParams,
    ) -> Result<mcp_schema::GetPromptResult, Error>;

    async fn list_tools(
        &self,
        request: mcp_schema::PaginatedParams,
    ) -> Result<mcp_schema::ListToolsResult, Error>;

    async fn call_tool(
        &self,
        request: mcp_schema::CallToolParams,
    ) -> Result<mcp_schema::CallToolResult, Error>;

    async fn set_level(
        &self,
        request: mcp_schema::SetLevelParams,
    ) -> Result<mcp_schema::EmptyResult, Error>;
}

async fn handle_request(
    service: &impl Service,
    request: mcp_schema::ClientRequest,
) -> Result<mcp_schema::ServerResult, Error> {
    match request {
        mcp_schema::ClientRequest::Initialize { params, .. } => service
            .init(params)
            .await
            .map(mcp_schema::ServerResult::Initialize),
        mcp_schema::ClientRequest::Ping { params, .. } => service
            .ping(params)
            .await
            .map(mcp_schema::ServerResult::Empty),
        mcp_schema::ClientRequest::ListResources { params, .. } => service
            .list_resources(params)
            .await
            .map(mcp_schema::ServerResult::ListResources),
        mcp_schema::ClientRequest::ListResourceTemplates { params, .. } => service
            .list_resource_templates(params)
            .await
            .map(mcp_schema::ServerResult::ListResourceTemplates),
        mcp_schema::ClientRequest::ReadResource { params, .. } => service
            .read_resource(params)
            .await
            .map(mcp_schema::ServerResult::ReadResource),
        mcp_schema::ClientRequest::Subscribe { params, .. } => service
            .subscribe(params)
            .await
            .map(mcp_schema::ServerResult::Empty),
        mcp_schema::ClientRequest::Unsubscribe { params, .. } => service
            .unsubscribe(params)
            .await
            .map(mcp_schema::ServerResult::Empty),
        mcp_schema::ClientRequest::ListPrompts { params, .. } => service
            .list_prompts(params)
            .await
            .map(mcp_schema::ServerResult::ListPrompts),
        mcp_schema::ClientRequest::GetPrompt { params, .. } => service
            .get_prompt(params)
            .await
            .map(mcp_schema::ServerResult::GetPrompt),
        mcp_schema::ClientRequest::ListTools { params, .. } => service
            .list_tools(params)
            .await
            .map(mcp_schema::ServerResult::ListTools),
        mcp_schema::ClientRequest::CallTool { params, .. } => service
            .call_tool(params)
            .await
            .map(mcp_schema::ServerResult::CallTool),
        mcp_schema::ClientRequest::SetLevel { params, .. } => service
            .set_level(params)
            .await
            .map(mcp_schema::ServerResult::Empty),
        mcp_schema::ClientRequest::Complete { .. } => {
            Ok(mcp_schema::ServerResult::Empty(mcp_schema::EmptyResult {
                meta: None,
                extra: HashMap::new(),
            }))
        }
    }
}
