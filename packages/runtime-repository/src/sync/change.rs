use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChangeSource {
    Watcher,
    Git,
    ManualRefresh,
    InitialScan,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChangeType {
    FileCreated,
    FileModified,
    FileDeleted,
    FileRenamed { old_path: PathBuf },
    ConfigurationChanged,
    DependencyChanged,
    GitBranchChanged,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryChange {
    pub source: ChangeSource,
    pub timestamp: SystemTime,
    pub change_type: ChangeType,
    pub affected_path: PathBuf,
    pub metadata: Option<serde_json::Value>,
}
