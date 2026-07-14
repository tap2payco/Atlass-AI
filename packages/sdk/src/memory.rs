use async_trait::async_trait;
use shared::id::MemoryId;
use shared::types::Result;

#[async_trait]
pub trait MemoryRuntime: Send + Sync {
    async fn store(&self, id: MemoryId, data: String) -> Result<()>;
    async fn retrieve(&self, id: MemoryId) -> Result<String>;
    async fn search(&self, query: String, limit: usize) -> Result<Vec<String>>;
}
