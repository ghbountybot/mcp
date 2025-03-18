use crate::Error;
use crate::registry::resource::Source;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::Notify;

#[derive(Default)]
pub struct MemoryResource {
    contents: Vec<mcp_schema::ResourceContents>,
    change: Arc<Notify>,
}

impl MemoryResource {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    #[expect(clippy::missing_const_for_fn)]
    pub fn get(&self) -> &[mcp_schema::ResourceContents] {
        &self.contents
    }

    pub fn set(&mut self, contents: Vec<mcp_schema::ResourceContents>) {
        self.contents = contents;

        self.change.notify_waiters();
    }
}

impl<State> Source<State> for MemoryResource {
    fn read(
        &self,
        _: State,
        _: String,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<mcp_schema::ResourceContents>, Error>> + Send>>
    {
        let contents = self.contents.clone();
        Box::pin(async move { Ok(contents) })
    }

    fn wait_for_change(&self, _: State, _: String) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        let change = self.change.clone();
        Box::pin(async move { change.notified().await })
    }
}
