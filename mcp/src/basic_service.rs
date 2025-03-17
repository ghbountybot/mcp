use crate::{Error, Prompt, PromptRegistry, Service, Tool, ToolRegistry};
use std::collections::HashMap;
use std::sync::Mutex;

pub struct BasicService<State> {
    state: State,
    instructions: Option<String>,
    tool_registry: Mutex<ToolRegistry<State>>,
    prompt_registry: Mutex<PromptRegistry<State>>,
}

impl<State> BasicService<State> {
    pub fn new(state: State) -> Self {
        Self {
            state,
            instructions: None,
            tool_registry: Mutex::new(ToolRegistry::default()),
            prompt_registry: Mutex::new(PromptRegistry::default()),
        }
    }

    pub fn tool_registry(&self) -> &Mutex<ToolRegistry<State>> {
        &self.tool_registry
    }

    pub fn tool_registry_mut(&mut self) -> &mut Mutex<ToolRegistry<State>> {
        &mut self.tool_registry
    }

    pub fn prompt_registry(&self) -> &Mutex<PromptRegistry<State>> {
        &self.prompt_registry
    }

    pub fn prompt_registry_mut(&mut self) -> &mut Mutex<PromptRegistry<State>> {
        &mut self.prompt_registry
    }
}

impl<State: Clone + Send + Sync + 'static> Service for BasicService<State> {
    fn init(
        &self,
        request: mcp_schema::InitializeParams,
    ) -> impl Future<Output = Result<mcp_schema::InitializeResult, Error>> + Send {
        async move {
            // TODO: check for compatible MCP version
            // TODO: Implement more capabilities
            Ok(mcp_schema::InitializeResult {
                capabilities: mcp_schema::ServerCapabilities {
                    experimental: None,
                    logging: None,
                    prompts: None,
                    resources: None,
                    tools: Some(mcp_schema::ToolsCapability {
                        list_changed: Some(false),
                    }),
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
    }

    fn ping(
        &self,
        _: mcp_schema::PingParams,
    ) -> impl Future<Output = Result<mcp_schema::EmptyResult, Error>> + Send {
        async move {
            Ok(mcp_schema::EmptyResult {
                meta: None,
                extra: HashMap::new(),
            })
        }
    }

    fn list_resources(
        &self,
        request: mcp_schema::PaginatedParams,
    ) -> impl Future<Output = Result<mcp_schema::ListResourcesResult, Error>> + Send {
        async move { todo!() }
    }

    fn list_resource_templates(
        &self,
        request: mcp_schema::PaginatedParams,
    ) -> impl Future<Output = Result<mcp_schema::ListResourceTemplatesResult, Error>> + Send {
        async move { todo!() }
    }

    fn read_resource(
        &self,
        request: mcp_schema::ReadResourceParams,
    ) -> impl Future<Output = Result<mcp_schema::ReadResourceResult, Error>> + Send {
        async move { todo!() }
    }

    fn subscribe(
        &self,
        request: mcp_schema::SubscribeParams,
    ) -> impl Future<Output = Result<mcp_schema::EmptyResult, Error>> + Send {
        async move { todo!() }
    }

    fn unsubscribe(
        &self,
        request: mcp_schema::UnsubscribeParams,
    ) -> impl Future<Output = Result<mcp_schema::EmptyResult, Error>> + Send {
        async move { todo!() }
    }

    fn list_prompts(
        &self,
        request: mcp_schema::PaginatedParams,
    ) -> impl Future<Output = Result<mcp_schema::ListPromptsResult, Error>> + Send {
        async move {
            let result = mcp_schema::ListPromptsResult {
                meta: None,
                next_cursor: None,
                prompts: self
                    .prompt_registry
                    .lock()
                    .unwrap()
                    .prompts_iter()
                    .map(|(_, prompt)| mcp_schema::Prompt::try_from(prompt))
                    .collect::<Result<Vec<_>, _>>()?,
                extra: HashMap::new(),
            };
            Ok(result)
        }
    }

    fn get_prompt(
        &self,
        request: mcp_schema::GetPromptParams,
    ) -> impl Future<Output = Result<mcp_schema::GetPromptResult, Error>> + Send {
        let result = self.prompt_registry.lock().unwrap();
        result.get_prompt(self.state.clone(), request)
    }

    fn list_tools(
        &self,
        request: mcp_schema::PaginatedParams,
    ) -> impl Future<Output = Result<mcp_schema::ListToolsResult, Error>> + Send {
        async move {
            let result = mcp_schema::ListToolsResult {
                meta: None,
                next_cursor: None,
                tools: self
                    .tool_registry
                    .lock()
                    .unwrap()
                    .tools_iter()
                    .map(|(_, tool): (_, &Tool<State>)| mcp_schema::Tool::try_from(tool))
                    .collect::<Result<Vec<_>, _>>()?,
                extra: HashMap::new(),
            };
            Ok(result)
        }
    }

    fn call_tool(
        &self,
        request: mcp_schema::CallToolParams,
    ) -> impl Future<Output = Result<mcp_schema::CallToolResult, Error>> + Send {
        let result = self.tool_registry.lock().unwrap();
        result.call_tool(self.state.clone(), request)
    }

    fn set_level(
        &self,
        request: mcp_schema::SetLevelParams,
    ) -> impl Future<Output = Result<mcp_schema::EmptyResult, Error>> + Send {
        async move { todo!() }
    }
}
