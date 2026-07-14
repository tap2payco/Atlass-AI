use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryHealth {
    pub repository_available: bool,
    pub watcher_active: bool,
    pub parser_status: String, // e.g., "Healthy", "Degraded"
    pub snapshot_status: String,
    pub last_synchronization_time: Option<std::time::SystemTime>,
    pub synchronization_lag_ms: u64,
    pub event_queue_depth: usize,
}
