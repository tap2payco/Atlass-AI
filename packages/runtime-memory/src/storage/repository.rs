use std::collections::HashMap;
use std::sync::RwLock;
use std::any::Any;
use std::sync::Arc;
use serde::{de::DeserializeOwned, Serialize};
use crate::domain::MemoryRecord;
use crate::storage::file_adapter::FileMemoryStore;

pub struct MemoryRepository {
    records: RwLock<HashMap<String, Arc<dyn Any + Send + Sync>>>,
    file_store: Option<Arc<FileMemoryStore>>,
}

impl MemoryRepository {
    pub fn new() -> Self {
        Self {
            records: RwLock::new(HashMap::new()),
            file_store: None,
        }
    }

    pub fn with_file_store(file_store: FileMemoryStore) -> Self {
        Self {
            records: RwLock::new(HashMap::new()),
            file_store: Some(Arc::new(file_store)),
        }
    }

    pub async fn save<T: Send + Sync + Clone + Serialize + 'static>(&self, record: &MemoryRecord<T>) -> Result<(), String> {
        if let Some(fs) = &self.file_store {
            fs.save(record)?;
        }
        if let Ok(mut map) = self.records.write() {
            map.insert(record.identity.id.clone(), Arc::new(record.clone()));
            Ok(())
        } else {
            Err("Lock poisoned".to_string())
        }
    }

    pub async fn load<T: Send + Sync + Clone + DeserializeOwned + 'static>(&self, id: &str) -> Result<Option<MemoryRecord<T>>, String> {
        if let Ok(map) = self.records.read() {
            if let Some(any_ptr) = map.get(id) {
                if let Some(record) = any_ptr.downcast_ref::<MemoryRecord<T>>() {
                    return Ok(Some(record.clone()));
                }
            }
        }
        if let Some(fs) = &self.file_store {
            if let Some(record) = fs.load(id)? {
                if let Ok(mut map) = self.records.write() {
                    map.insert(id.to_string(), Arc::new(record.clone()));
                }
                return Ok(Some(record));
            }
        }
        Ok(None)
    }

    pub async fn delete(&self, id: &str) -> Result<bool, String> {
        if let Some(fs) = &self.file_store {
            let _ = fs.delete(id);
        }
        if let Ok(mut map) = self.records.write() {
            Ok(map.remove(id).is_some())
        } else {
            Err("Lock poisoned".to_string())
        }
    }

    pub async fn list_all<T: Send + Sync + Clone + DeserializeOwned + 'static>(&self) -> Result<Vec<MemoryRecord<T>>, String> {
        let mut results = Vec::new();
        if let Ok(map) = self.records.read() {
            for ptr in map.values() {
                if let Some(record) = ptr.downcast_ref::<MemoryRecord<T>>() {
                    results.push(record.clone());
                }
            }
        }
        if results.is_empty() {
            if let Some(fs) = &self.file_store {
                results = fs.list_all::<T>()?;
            }
        }
        Ok(results)
    }
}

impl Default for MemoryRepository {
    fn default() -> Self {
        Self::new()
    }
}
