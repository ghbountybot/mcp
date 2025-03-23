use crate::Error;
use futures::FutureExt;
use mcp_schema::ResourceContents;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

fn template_uri_matches(_template: &str, _uri: &str) -> bool {
    todo!()
}

/// A registry for managing available resources with shared state
pub struct ResourceRegistry<State> {
    fixed_resources: HashMap<String, Resource<State, FixedResourceUri>>,
    template_resources: Vec<Resource<State, TemplateResourceUri>>,
}

impl<State> ResourceRegistry<State> {
    /// Register a new resource with a fixed uri
    pub fn register_fixed(&mut self, resource: Resource<State, FixedResourceUri>) {
        self.fixed_resources
            .insert(resource.uri.0.clone(), resource);
    }

    /// Register a new resource with a template uri
    pub fn register_template(&mut self, resource: Resource<State, TemplateResourceUri>) {
        self.template_resources.push(resource);
    }
}

impl<State: Send + Sync + 'static> ResourceRegistry<State> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Gets a source from a uri.
    ///
    /// # Errors
    /// If the uri does not match any of the registered resources, this will error.
    pub fn get_source(
        &self,
        uri: &str,
    ) -> Result<Arc<dyn ErasedSource<State> + Send + Sync>, Error> {
        self.fixed_resources
            .get(uri)
            .map(|resource| resource.source.clone())
            .or_else(|| {
                self.template_resources
                    .iter()
                    .find(|resource| template_uri_matches(&resource.uri.0, uri))
                    .map(|resource| resource.source.clone())
            })
            .ok_or_else(|| Error {
                message: format!("Resource at uri '{uri}' not found"),
                code: 404,
            })
    }

    /// Read a resource from a URI
    ///
    /// # Errors
    /// If the uri does not match any of the registered resources or the underlying source
    /// encounters an error, this will error.
    pub fn read_resource(
        &self,
        state: State,
        uri: String,
    ) -> impl Future<Output = Result<mcp_schema::ReadResourceResult, Error>> + use<State> + Send + 'static
    {
        let source = self.get_source(&uri);
        let contents = source.map(|source| source.read_erased(state, uri));

        async move {
            let contents = contents?.await?;

            Ok(mcp_schema::ReadResourceResult {
                meta: None,
                contents,
                extra: HashMap::new(),
            })
        }
    }

    /// Waits for a change in a resource from a URI
    ///
    /// # Errors
    /// If the uri does not match any of the registered resources, this will error.
    pub fn wait_for_change(
        &self,
        state: State,
        uri: String,
    ) -> Result<impl Future<Output = ()> + use<State> + Send + 'static, Error> {
        Ok(self.get_source(&uri)?.wait_for_change_erased(state, uri))
    }

    /// Iterate through all registered fixed resources
    pub fn fixed_resources_iter(&self) -> impl Iterator<Item = &Resource<State, FixedResourceUri>> {
        self.fixed_resources.values()
    }

    /// Iterate through all registered resource templates
    pub fn template_resource_iter(
        &self,
    ) -> impl Iterator<Item = &Resource<State, TemplateResourceUri>> {
        self.template_resources.iter()
    }
}

impl<State> Default for ResourceRegistry<State> {
    fn default() -> Self {
        Self {
            fixed_resources: HashMap::new(),
            template_resources: Vec::new(),
        }
    }
}

pub trait Source<State> {
    fn read(
        &self,
        state: State,
        uri: String,
    ) -> impl Future<Output = Result<Vec<ResourceContents>, Error>> + 'static + Send;

    fn wait_for_change(
        &self,
        state: State,
        uri: String,
    ) -> impl Future<Output = ()> + 'static + Send;
}

pub trait ErasedSource<State> {
    fn read_erased(
        &self,
        state: State,
        uri: String,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<ResourceContents>, Error>> + Send>>;

    fn wait_for_change_erased(
        &self,
        state: State,
        uri: String,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>>;
}

impl<State, T> ErasedSource<State> for T
where
    T: Source<State>,
{
    fn read_erased(
        &self,
        state: State,
        uri: String,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<ResourceContents>, Error>> + Send>> {
        let fut = self.read(state, uri);
        fut.boxed()
    }

    fn wait_for_change_erased(
        &self,
        state: State,
        uri: String,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        let fut = self.wait_for_change(state, uri);
        fut.boxed()
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct FixedResourceUri(pub String);

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct TemplateResourceUri(pub String);

pub struct Resource<State, Uri> {
    uri: Uri,
    name: String,
    description: Option<String>,
    mime_type: Option<String>,
    annotated: mcp_schema::Annotated,
    source: Arc<dyn ErasedSource<State> + Send + Sync>,
}

impl<State: Send + Sync + 'static, Uri> Resource<State, Uri> {
    #[must_use]
    pub fn builder() -> ResourceBuilder<State, Uri> {
        ResourceBuilder::new()
    }
}

impl<State> TryFrom<&Resource<State, FixedResourceUri>> for mcp_schema::Resource {
    type Error = serde_json::Error;

    fn try_from(resource: &Resource<State, FixedResourceUri>) -> Result<Self, Self::Error> {
        Ok(Self {
            uri: resource.uri.0.clone(),
            name: resource.name.clone(),
            description: resource.description.clone(),
            mime_type: resource.mime_type.clone(),
            annotated: resource.annotated.clone(),
        })
    }
}

impl<State> TryFrom<&Resource<State, TemplateResourceUri>> for mcp_schema::ResourceTemplate {
    type Error = serde_json::Error;

    fn try_from(resource: &Resource<State, TemplateResourceUri>) -> Result<Self, Self::Error> {
        Ok(Self {
            uri_template: resource.uri.0.clone(),
            name: resource.name.clone(),
            description: resource.description.clone(),
            mime_type: resource.mime_type.clone(),
            annotated: resource.annotated.clone(),
        })
    }
}

/// A builder for constructing a resource with validation and metadata
pub struct ResourceBuilder<State, Uri> {
    uri: Option<Uri>,
    name: Option<String>,
    description: Option<String>,
    mime_type: Option<String>,
    annotated: mcp_schema::Annotated,
    source: Option<Arc<dyn ErasedSource<State> + Send + Sync>>,
}

impl<State: Send + Sync + 'static, Uri> ResourceBuilder<State, Uri> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    #[must_use]
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    #[must_use]
    pub fn mime_type(mut self, mime_type: impl Into<String>) -> Self {
        self.mime_type = Some(mime_type.into());
        self
    }

    #[must_use]
    pub fn annotations(mut self, annotations: mcp_schema::Annotations) -> Self {
        self.annotated.annotations = Some(annotations);
        self
    }

    #[must_use]
    pub fn source(mut self, source: impl Source<State> + Send + Sync + 'static) -> Self {
        self.source = Some(Arc::new(source));
        self
    }

    /// Builds a resource.
    ///
    /// # Errors
    /// If the uri or source was not set, this will error.
    pub fn build(self) -> Result<Resource<State, Uri>, Error> {
        Ok(Resource {
            uri: self.uri.ok_or_else(|| Error {
                message: "missing uri".to_string(),
                code: 500,
            })?,
            name: self.name.unwrap_or_else(|| "unnamed resource".to_string()),
            description: self.description,
            mime_type: self.mime_type,
            annotated: self.annotated,
            source: self.source.ok_or_else(|| Error {
                message: "missing source".to_string(),
                code: 500,
            })?,
        })
    }
}

impl<State> ResourceBuilder<State, FixedResourceUri> {
    #[must_use]
    pub fn fixed_uri(mut self, name: impl Into<String>) -> Self {
        self.uri = Some(FixedResourceUri(name.into()));
        self
    }
}

impl<State> ResourceBuilder<State, TemplateResourceUri> {
    #[must_use]
    pub fn template_uri(mut self, name: impl Into<String>) -> Self {
        self.uri = Some(TemplateResourceUri(name.into()));
        self
    }
}

impl<State, Uri> Default for ResourceBuilder<State, Uri> {
    fn default() -> Self {
        Self {
            uri: None,
            name: None,
            description: None,
            mime_type: None,
            annotated: mcp_schema::Annotated {
                annotations: None,
                extra: HashMap::new(),
            },
            source: None,
        }
    }
}
