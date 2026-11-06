use serde::{de::DeserializeOwned, Serialize};
use crate::domain::MemoryRecord;
use crate::storage::repository::MemoryRepository;
use crate::engine::retrieval::{RetrievalEngine, RetrievalExplanation};
use crate::engine::snapshot::MemorySnapshot;

pub struct MemoryRuntime {
    repository: MemoryRepository,
    retrieval: RetrievalEngine,
}

impl MemoryRuntime {
    pub fn new() -> Self {
        Self {
            repository: MemoryRepository::new(),
            retrieval: RetrievalEngine::new(),
        }
    }

    pub fn with_repository(repository: MemoryRepository) -> Self {
        Self {
            repository,
            retrieval: RetrievalEngine::new(),
        }
    }
    
    pub async fn store<T: Send + Sync + Clone + Serialize + DeserializeOwned + 'static>(&self, record: &MemoryRecord<T>) -> Result<(), String> {
        self.repository.save(record).await
    }
    
    pub async fn retrieve<T: Send + Sync + Clone + Serialize + DeserializeOwned + 'static>(&self, _query: &str) -> Result<(Vec<MemoryRecord<T>>, RetrievalExplanation), String> {
        let all = self.repository.list_all::<T>().await?;
        Ok((all, RetrievalExplanation::default()))
    }
    
    pub async fn update<T: Send + Sync + Clone + Serialize + DeserializeOwned + 'static>(&self, record: &MemoryRecord<T>) -> Result<(), String> {
        self.repository.save(record).await
    }
    
    pub async fn archive(&self, _id: &str) -> Result<(), String> {
        Ok(())
    }
    
    pub async fn delete(&self, id: &str) -> Result<(), String> {
        self.repository.delete(id).await.map(|_| ())
    }
    
    pub async fn snapshot<T: Send + Sync + Clone + Serialize + DeserializeOwned + 'static>(&self, id: &str) -> Result<MemorySnapshot<T>, String> {
        if let Some(record) = self.repository.load::<T>(id).await? {
            crate::engine::snapshot::SnapshotManager::new().snapshot(&record)
        } else {
            Err(format!("Memory record with id '{}' not found", id))
        }
    }
    
    pub async fn query<T: Send + Sync + Clone + Serialize + DeserializeOwned + 'static>(&self, _params: &str) -> Result<Vec<MemoryRecord<T>>, String> {
        self.repository.list_all::<T>().await
    }
}

impl Default for MemoryRuntime {
    fn default() -> Self {
        Self::new()
    }
}
