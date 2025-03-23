use crate::Error;
use crate::registry::resource::Source;
use mcp_schema::ResourceContents;
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

#[derive(Default)]
pub struct MemoryResourceInner {
    contents: Mutex<Vec<mcp_schema::ResourceContents>>,
    change: Notify,
}

#[derive(Clone, Default)]
pub struct MemoryResource {
    inner: Arc<MemoryResourceInner>,
}

impl MemoryResource {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn get(&self) -> Vec<ResourceContents> {
        self.inner.contents.lock().unwrap().clone()
    }

    pub fn set(&self, contents: impl IntoIterator<Item = ResourceContents>) {
        let contents = contents.into_iter().collect();
        *self.inner.contents.lock().unwrap() = contents;
        self.inner.change.notify_waiters();
    }
}

impl<State: Send> Source<State> for MemoryResource {
    fn read(
        &self,
        _: State,
        _: String,
    ) -> impl Future<Output = Result<Vec<mcp_schema::ResourceContents>, Error>> + Send + 'static
    {
        let contents = self.get();
        async move { Ok(contents) }
    }

    fn wait_for_change(&self, _: State, _: String) -> impl Future<Output = ()> + Send + 'static {
        let inner = self.inner.clone();
        async move {
            inner.change.notified().await;
        }
    }
}
