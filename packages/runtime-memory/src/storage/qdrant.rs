use async_trait::async_trait;
use crate::storage::adapter::StorageAdapter;
use crate::domain::MemoryRecord;

pub struct QdrantAdapter {
    // Stub
}

impl QdrantAdapter {
    pub fn new() -> Self { Self {} }
}

#[async_trait]
impl StorageAdapter for QdrantAdapter {
    fn capabilities(&self) -> Vec<String> {
        vec![
            "vector-search".to_string(),
            "hybrid-retrieval".to_string(),
            "ann-search".to_string(),
        ]
    }

    async fn store<T: Send + Sync>(&self, _record: &MemoryRecord<T>) -> Result<(), String> {
        Ok(())
    }

    async fn retrieve<T: Send + Sync>(&self, _id: &str) -> Result<Option<MemoryRecord<T>>, String> {
        Ok(None)
    }

    async fn archive(&self, _id: &str) -> Result<(), String> {
        Ok(())
    }

    async fn delete(&self, _id: &str) -> Result<(), String> {
        Ok(())
    }
}
