use crate::Error;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

fn template_uri_matches(_template: &str, _uri: &str) -> bool {
    todo!()
}

/// A registry for managing available resources with shared state
pub struct ResourceRegistry<State> {
    fixed_resources: HashMap<String, Resource<State>>,
    templated_resources: Vec<Resource<State>>,
}

impl<State: Send + Sync + 'static> ResourceRegistry<State> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new resource
    pub fn register(&mut self, resource: Resource<State>) {
        match &resource.uri {
            ResourceUri::Fixed(uri) => {
                self.fixed_resources.insert(uri.clone(), resource);
            }
            ResourceUri::Template(_) => {
                self.templated_resources.push(resource);
            }
        }
    }

    /// Gets a resource from a uri.
    ///
    /// # Errors
    /// If the uri does not match any of the registered resources, this will error.
    pub fn get_resource(&self, uri: &str) -> Result<&Resource<State>, Error> {
        self.fixed_resources
            .get(uri)
            .or_else(|| {
                self.templated_resources.iter().find(|resource| {
                    let ResourceUri::Template(template) = &resource.uri else {
                        panic!("resource with non-templated uri is in templated_resources")
                    };
                    template_uri_matches(template, uri)
                })
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
        let contents = self
            .get_resource(&uri)
            .map(|resource| resource.source.read(state, uri));

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
        Ok(self.get_resource(&uri)?.source.wait_for_change(state, uri))
    }

    /// Iterate through all registered fixed resources
    pub fn fixed_resources_iter(&self) -> impl Iterator<Item = &Resource<State>> {
        self.fixed_resources.values()
    }

    /// Iterate through all registered resource templates
    pub fn templated_resource_iter(&self) -> impl Iterator<Item = &Resource<State>> {
        self.templated_resources.iter()
    }
}

impl<State> Default for ResourceRegistry<State> {
    fn default() -> Self {
        Self {
            fixed_resources: HashMap::new(),
            templated_resources: Vec::new(),
        }
    }
}

pub trait Source<State> {
    fn read(
        &self,
        state: State,
        uri: String,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<mcp_schema::ResourceContents>, Error>> + Send>>;

    fn wait_for_change(
        &self,
        state: State,
        uri: String,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>>;
}

#[derive(Clone, PartialEq, Eq)]
pub enum ResourceUri {
    Fixed(String),
    Template(String),
}

pub struct Resource<State> {
    uri: ResourceUri,
    name: String,
    description: Option<String>,
    mime_type: Option<String>,
    annotated: mcp_schema::Annotated,
    source: Box<dyn Source<State> + Send>,
}

impl<State: Send + Sync + 'static> Resource<State> {
    #[must_use]
    pub fn builder() -> ResourceBuilder<State> {
        ResourceBuilder::new()
    }
}

impl<State> TryFrom<&Resource<State>> for mcp_schema::Resource {
    type Error = serde_json::Error;

    fn try_from(resource: &Resource<State>) -> Result<Self, Self::Error> {
        let ResourceUri::Fixed(uri) = resource.uri.clone() else {
            todo!()
        };

        Ok(Self {
            uri,
            name: resource.name.clone(),
            description: resource.description.clone(),
            mime_type: resource.mime_type.clone(),
            annotated: resource.annotated.clone(),
        })
    }
}

impl<State> TryFrom<&Resource<State>> for mcp_schema::ResourceTemplate {
    type Error = serde_json::Error;

    fn try_from(resource: &Resource<State>) -> Result<Self, Self::Error> {
        let ResourceUri::Template(uri_template) = resource.uri.clone() else {
            todo!()
        };

        Ok(Self {
            uri_template,
            name: resource.name.clone(),
            description: resource.description.clone(),
            mime_type: resource.mime_type.clone(),
            annotated: resource.annotated.clone(),
        })
    }
}

/// A builder for constructing a resource with validation and metadata
pub struct ResourceBuilder<State> {
    uri: Option<ResourceUri>,
    name: Option<String>,
    description: Option<String>,
    mime_type: Option<String>,
    annotated: mcp_schema::Annotated,
    source: Option<Box<dyn Source<State> + Send>>,
}

impl<State: Send + Sync + 'static> ResourceBuilder<State> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn fixed_uri(mut self, name: impl Into<String>) -> Self {
        self.uri = Some(ResourceUri::Fixed(name.into()));
        self
    }

    #[must_use]
    pub fn templated_uri(mut self, name: impl Into<String>) -> Self {
        self.uri = Some(ResourceUri::Template(name.into()));
        self
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
    pub fn source(mut self, source: Box<dyn Source<State> + Send>) -> Self {
        self.source = Some(source);
        self
    }

    /// Builds a resource.
    ///
    /// # Errors
    /// If the uri or source was not set, this will error.
    pub fn build(self) -> Result<Resource<State>, Error> {
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

impl<State> Default for ResourceBuilder<State> {
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
