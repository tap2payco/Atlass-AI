use std::collections::HashMap;
use std::sync::RwLock;
use platform_contracts::health::HealthState;

#[derive(Debug, Clone)]
pub struct HealthRecord {
    pub runtime_id: String,
    pub endpoint: String,
    pub state: HealthState,
    pub last_checked: std::time::SystemTime,
}

pub struct HealthRegistry {
    records: RwLock<HashMap<String, HealthRecord>>,
}

impl HealthRegistry {
    pub fn new() -> Self {
        Self {
            records: RwLock::new(HashMap::new()),
        }
    }

    pub fn register_endpoint(&self, runtime_id: &str, endpoint: &str) {
        if let Ok(mut map) = self.records.write() {
            map.insert(
                runtime_id.to_string(),
                HealthRecord {
                    runtime_id: runtime_id.to_string(),
                    endpoint: endpoint.to_string(),
                    state: HealthState::Healthy,
                    last_checked: std::time::SystemTime::now(),
                },
            );
        }
    }

    pub fn update_state(&self, runtime_id: &str, state: HealthState) {
        if let Ok(mut map) = self.records.write() {
            if let Some(record) = map.get_mut(runtime_id) {
                record.state = state;
                record.last_checked = std::time::SystemTime::now();
            }
        }
    }

    pub fn get_status(&self, runtime_id: &str) -> Option<HealthState> {
        let map = self.records.read().ok()?;
        map.get(runtime_id).map(|r| r.state.clone())
    }

    pub fn get_record(&self, runtime_id: &str) -> Option<HealthRecord> {
        let map = self.records.read().ok()?;
        map.get(runtime_id).cloned()
    }

    pub fn is_all_healthy(&self) -> bool {
        if let Ok(map) = self.records.read() {
            map.values().all(|r| matches!(r.state, HealthState::Healthy))
        } else {
            false
        }
    }
}

impl Default for HealthRegistry {
    fn default() -> Self {
        Self::new()
    }
}
