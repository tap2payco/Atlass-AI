use async_trait::async_trait;
use crate::domain::MemoryRecord;

#[async_trait]
pub trait StorageAdapter: Send + Sync {
    fn capabilities(&self) -> Vec<String>;
    async fn store<T: Send + Sync>(&self, record: &MemoryRecord<T>) -> Result<(), String>;
    async fn retrieve<T: Send + Sync>(&self, id: &str) -> Result<Option<MemoryRecord<T>>, String>;
    async fn archive(&self, id: &str) -> Result<(), String>;
    async fn delete(&self, id: &str) -> Result<(), String>; // Replaced forget with delete
}
