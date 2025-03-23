use crate::registry::resource::FixedResourceUri;
use crate::{
    Error, Prompt, PromptRegistry, Resource, ResourceRegistry, Service, Tool, ToolRegistry,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::task::JoinHandle;

pub struct BasicService<State> {
    state: Option<State>,

    name: String,
    version: String,

    instructions: Option<String>,
    tool_registry: ToolRegistry<State>,
    prompt_registry: PromptRegistry<State>,
    resource_registry: ResourceRegistry<State>,

    notification_handler: Option<Arc<dyn Fn(mcp_schema::ServerNotification) + Send + Sync>>,
    resource_subscriptions: Mutex<HashMap<String, JoinHandle<()>>>,
}

impl BasicService<()> {}

impl<State> Default for BasicService<State> {
    fn default() -> Self {
        Self::new()
    }
}

impl<State> BasicService<State> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: None,
            name: "unnamed".to_string(),
            version: "0.1.0".to_string(),
            instructions: None,
            tool_registry: ToolRegistry::default(),
            prompt_registry: PromptRegistry::default(),
            resource_registry: ResourceRegistry::default(),
            notification_handler: None,
            resource_subscriptions: Mutex::new(HashMap::new()),
        }
    }

    #[must_use]
    pub fn state(mut self, state: State) -> Self {
        self.state = Some(state);
        self
    }

    #[must_use]
    pub fn version(mut self, version: String) -> Self {
        self.version = version;
        self
    }

    #[must_use]
    pub fn name(mut self, name: String) -> Self {
        self.name = name;
        self
    }

    pub const fn tool_registry(&self) -> &ToolRegistry<State> {
        &self.tool_registry
    }

    pub const fn tool_registry_mut(&mut self) -> &mut ToolRegistry<State> {
        &mut self.tool_registry
    }

    #[must_use]
    pub fn tool(mut self, tool: Tool<State>) -> Self {
        let registry = self.tool_registry_mut();
        registry.register(tool);
        self
    }

    #[must_use]
    pub fn prompt(mut self, prompt: Prompt<State>) -> Self {
        let registry = self.prompt_registry_mut();
        registry.register(prompt);
        self
    }

    pub const fn prompt_registry(&self) -> &PromptRegistry<State> {
        &self.prompt_registry
    }

    pub const fn prompt_registry_mut(&mut self) -> &mut PromptRegistry<State> {
        &mut self.prompt_registry
    }

    pub const fn resource_registry(&self) -> &ResourceRegistry<State> {
        &self.resource_registry
    }

    pub fn resource_registry_mut(&mut self) -> &mut ResourceRegistry<State> {
        &mut self.resource_registry
    }

    #[must_use]
    pub fn fixed_resource(mut self, resource: Resource<State, FixedResourceUri>) -> Self {
        let registry = self.resource_registry_mut();
        registry.register_fixed(resource);
        self
    }
}

impl<State: Clone + Send + Sync + 'static> Service for BasicService<State> {
    fn set_notification_handler(
        &mut self,
        handler: Box<dyn Fn(mcp_schema::ServerNotification) + Send + Sync>,
    ) {
        self.notification_handler = Some(handler.into());
    }

    fn init(
        &self,
        _request: mcp_schema::InitializeParams,
    ) -> impl Future<Output = Result<mcp_schema::InitializeResult, Error>> + Send {
        let result = mcp_schema::InitializeResult {
            capabilities: mcp_schema::ServerCapabilities {
                experimental: None,
                logging: None,
                prompts: Some(mcp_schema::PromptsCapability {
                    list_changed: Some(false),
                }),
                resources: Some(mcp_schema::ResourcesCapability {
                    subscribe: Some(true),
                    list_changed: Some(false),
                }),
                tools: Some(mcp_schema::ToolsCapability {
                    list_changed: Some(false),
                }),
                extra: HashMap::new(),
            },
            instructions: self.instructions.clone(),
            meta: None,
            protocol_version: mcp_schema::LATEST_PROTOCOL_VERSION.to_string(),
            server_info: mcp_schema::Implementation {
                name: self.name.clone(),
                version: self.version.clone(),
                extra: HashMap::new(),
            },
            extra: HashMap::new(),
        };

        async move { Ok(result) }
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
        _request: mcp_schema::PaginatedParams,
    ) -> impl Future<Output = Result<mcp_schema::ListResourcesResult, Error>> + Send {
        let result = || {
            let resources: Result<Vec<_>, serde_json::Error> = self
                .resource_registry
                .fixed_resources_iter()
                .map(mcp_schema::Resource::try_from)
                .collect();

            let resources = resources?;

            let result = mcp_schema::ListResourcesResult {
                meta: None,
                next_cursor: None,
                resources,
                extra: HashMap::new(),
            };

            Ok::<_, serde_json::Error>(result)
        };

        let result = result();

        async move { Ok(result?) }
    }

    fn list_resource_templates(
        &self,
        _request: mcp_schema::PaginatedParams,
    ) -> impl Future<Output = Result<mcp_schema::ListResourceTemplatesResult, Error>> + Send {
        let result = || {
            let resource_templates: Result<Vec<_>, serde_json::Error> = self
                .resource_registry
                .template_resource_iter()
                .map(mcp_schema::ResourceTemplate::try_from)
                .collect();

            let resource_templates = resource_templates?;

            let result = mcp_schema::ListResourceTemplatesResult {
                meta: None,
                next_cursor: None,
                resource_templates,
                extra: HashMap::new(),
            };

            Ok::<_, serde_json::Error>(result)
        };

        let result = result();

        async move { Ok(result?) }
    }

    fn read_resource(
        &self,
        request: mcp_schema::ReadResourceParams,
    ) -> impl Future<Output = Result<mcp_schema::ReadResourceResult, Error>> + Send {
        let result = &self.resource_registry;
        result.read_resource(self.state.clone().expect("state must be set"), request.uri)
    }

    fn subscribe(
        &self,
        request: mcp_schema::SubscribeParams,
    ) -> impl Future<Output = Result<mcp_schema::EmptyResult, Error>> + Send {
        let notification_handler = self
            .notification_handler
            .clone()
            .expect("service notification handler must be set");
        let state = self.state.clone().expect("state must be set");
        let uri = request.uri;
        let source = self.resource_registry.get_source(&uri);
        let mut error = None;
        match source {
            Ok(source) => {
                let uri_clone = uri.clone();
                let handle = tokio::spawn(async move {
                    loop {
                        source
                            .wait_for_change_erased(state.clone(), uri.clone())
                            .await;
                        (notification_handler)(mcp_schema::ServerNotification::ResourceUpdated {
                            json_rpc: mcp_schema::JSONRPC_VERSION.to_string(),
                            params: mcp_schema::ResourceUpdatedParams {
                                uri: uri.clone(),
                                extra: HashMap::new(),
                            },
                        });
                    }
                });
                self.resource_subscriptions
                    .lock()
                    .unwrap()
                    .insert(uri_clone, handle);
            }
            Err(e) => {
                error = Some(e);
            }
        }
        async move {
            error.map_or_else(
                || {
                    Ok(mcp_schema::EmptyResult {
                        meta: None,
                        extra: HashMap::new(),
                    })
                },
                Err,
            )
        }
    }

    fn unsubscribe(
        &self,
        request: mcp_schema::UnsubscribeParams,
    ) -> impl Future<Output = Result<mcp_schema::EmptyResult, Error>> + Send {
        let subscription = self
            .resource_subscriptions
            .lock()
            .unwrap()
            .remove(&request.uri);

        if let Some(subscription) = subscription {
            subscription.abort();
        }

        async move {
            Ok(mcp_schema::EmptyResult {
                meta: None,
                extra: HashMap::new(),
            })
        }
    }

    fn list_prompts(
        &self,
        _request: mcp_schema::PaginatedParams,
    ) -> impl Future<Output = Result<mcp_schema::ListPromptsResult, Error>> + Send {
        let result = || {
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
        };

        let result = result();

        async move { result }
    }

    fn get_prompt(
        &self,
        request: mcp_schema::GetPromptParams,
    ) -> impl Future<Output = Result<mcp_schema::GetPromptResult, Error>> + Send {
        let result = &self.prompt_registry;
        result.get_prompt(self.state.clone().expect("state must be set"), request)
    }

    fn list_tools(
        &self,
        _request: mcp_schema::PaginatedParams,
    ) -> impl Future<Output = Result<mcp_schema::ListToolsResult, Error>> + Send {
        let tools = self
            .tool_registry
            .tools_iter()
            .map(|(_, tool): (_, &Tool<State>)| mcp_schema::Tool::try_from(tool))
            .collect::<Result<Vec<_>, _>>();
        async move {
            let result = mcp_schema::ListToolsResult {
                meta: None,
                next_cursor: None,
                tools: tools?,
                extra: HashMap::new(),
            };
            Ok(result)
        }
    }

    fn call_tool(
        &self,
        request: mcp_schema::CallToolParams,
    ) -> impl Future<Output = Result<mcp_schema::CallToolResult, Error>> + Send {
        let result = &self.tool_registry;
        result.call_tool(self.state.clone().expect("state must be set"), request)
    }

    fn set_level(
        &self,
        _request: mcp_schema::SetLevelParams,
    ) -> impl Future<Output = Result<mcp_schema::EmptyResult, Error>> + Send {
        async move { todo!() }
    }
}
