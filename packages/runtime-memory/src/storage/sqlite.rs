use async_trait::async_trait;
use crate::storage::adapter::StorageAdapter;
use crate::domain::MemoryRecord;

pub struct SQLiteAdapter {
    // Stub
}

impl SQLiteAdapter {
    pub fn new() -> Self { Self {} }
}

#[async_trait]
impl StorageAdapter for SQLiteAdapter {
    fn capabilities(&self) -> Vec<String> {
        vec![
            "transactions".to_string(),
            "versioning".to_string(),
            "local-persistence".to_string(),
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
