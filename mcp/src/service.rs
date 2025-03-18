use crate::Error;

pub trait Service {
    fn set_notification_handler(
        &mut self,
        handler: Box<dyn Fn(mcp_schema::ServerNotification) + Send + Sync>,
    );

    fn init(
        &self,
        request: mcp_schema::InitializeParams,
    ) -> impl Future<Output = Result<mcp_schema::InitializeResult, Error>> + Send;

    fn ping(
        &self,
        _: mcp_schema::PingParams,
    ) -> impl Future<Output = Result<mcp_schema::EmptyResult, Error>> + Send;

    fn list_resources(
        &self,
        request: mcp_schema::PaginatedParams,
    ) -> impl Future<Output = Result<mcp_schema::ListResourcesResult, Error>> + Send;

    fn list_resource_templates(
        &self,
        request: mcp_schema::PaginatedParams,
    ) -> impl Future<Output = Result<mcp_schema::ListResourceTemplatesResult, Error>> + Send;

    fn read_resource(
        &self,
        request: mcp_schema::ReadResourceParams,
    ) -> impl Future<Output = Result<mcp_schema::ReadResourceResult, Error>> + Send;

    fn subscribe(
        &self,
        request: mcp_schema::SubscribeParams,
    ) -> impl Future<Output = Result<mcp_schema::EmptyResult, Error>> + Send;

    fn unsubscribe(
        &self,
        request: mcp_schema::UnsubscribeParams,
    ) -> impl Future<Output = Result<mcp_schema::EmptyResult, Error>> + Send;

    fn list_prompts(
        &self,
        request: mcp_schema::PaginatedParams,
    ) -> impl Future<Output = Result<mcp_schema::ListPromptsResult, Error>> + Send;

    fn get_prompt(
        &self,
        request: mcp_schema::GetPromptParams,
    ) -> impl Future<Output = Result<mcp_schema::GetPromptResult, Error>> + Send;

    fn list_tools(
        &self,
        request: mcp_schema::PaginatedParams,
    ) -> impl Future<Output = Result<mcp_schema::ListToolsResult, Error>> + Send;

    fn call_tool(
        &self,
        request: mcp_schema::CallToolParams,
    ) -> impl Future<Output = Result<mcp_schema::CallToolResult, Error>> + Send;

    fn set_level(
        &self,
        request: mcp_schema::SetLevelParams,
    ) -> impl Future<Output = Result<mcp_schema::EmptyResult, Error>> + Send;
}
