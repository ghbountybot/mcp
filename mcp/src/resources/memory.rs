use crate::Error;
use crate::registry::resource::{ResourceSlice, Source};
use mcp_schema::ResourceContents;
use std::sync::Arc;

#[derive(Clone)]
pub struct MemoryResource {
    tx: tokio::sync::broadcast::Sender<Arc<[ResourceContents]>>,
}

impl Default for MemoryResource {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryResource {
    #[must_use]
    pub fn new() -> Self {
        let (tx, _) = tokio::sync::broadcast::channel(1);
        Self { tx }
    }

    #[must_use]
    pub fn get(&self) -> Arc<[ResourceContents]> {
        self.tx.subscribe().try_recv().unwrap()
    }

    pub fn set(&self, contents: impl IntoIterator<Item = ResourceContents>) {
        let contents: Arc<[ResourceContents]> = contents.into_iter().collect();
        self.tx.send(contents).unwrap();
    }
}

impl<State: Send> Source<State> for MemoryResource {
    fn read(
        &self,
        _: State,
        _: String,
    ) -> impl Future<Output = Result<ResourceSlice, Error>> + Send + 'static {
        let contents = self.get();
        async move { Ok(contents) }
    }

    fn wait_for_change(&self, _: State, _: String) -> impl Future<Output = ()> + Send + 'static {
        let mut recv = self.tx.subscribe();
        async move {
            recv.recv().await.unwrap();
        }
    }
}
