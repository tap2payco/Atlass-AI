use std::fs;
use std::path::PathBuf;
use serde::{de::DeserializeOwned, Serialize};
use crate::domain::MemoryRecord;

pub struct FileMemoryStore {
    base_dir: PathBuf,
}

impl FileMemoryStore {
    pub fn new(base_dir: PathBuf) -> Self {
        let _ = fs::create_dir_all(&base_dir);
        Self { base_dir }
    }

    pub fn save<T: Serialize>(&self, record: &MemoryRecord<T>) -> Result<(), String> {
        let file_path = self.base_dir.join(format!("{}.json", record.identity.id));
        let json = serde_json::to_string_pretty(record)
            .map_err(|e| format!("Serialization error: {}", e))?;
        fs::write(file_path, json)
            .map_err(|e| format!("File write error: {}", e))
    }

    pub fn load<T: DeserializeOwned>(&self, id: &str) -> Result<Option<MemoryRecord<T>>, String> {
        let file_path = self.base_dir.join(format!("{}.json", id));
        if !file_path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(file_path)
            .map_err(|e| format!("File read error: {}", e))?;
        let record = serde_json::from_str(&content)
            .map_err(|e| format!("Deserialization error: {}", e))?;
        Ok(Some(record))
    }

    pub fn delete(&self, id: &str) -> Result<bool, String> {
        let file_path = self.base_dir.join(format!("{}.json", id));
        if file_path.exists() {
            fs::remove_file(file_path).map_err(|e| format!("File remove error: {}", e))?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn list_all<T: DeserializeOwned>(&self) -> Result<Vec<MemoryRecord<T>>, String> {
        let mut results = Vec::new();
        if let Ok(entries) = fs::read_dir(&self.base_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    if let Ok(content) = fs::read_to_string(&path) {
                        if let Ok(record) = serde_json::from_str::<MemoryRecord<T>>(&content) {
                            results.push(record);
                        }
                    }
                }
            }
        }
        Ok(results)
    }
}
