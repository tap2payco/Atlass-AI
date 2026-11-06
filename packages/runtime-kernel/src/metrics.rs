use std::collections::HashMap;
use std::sync::RwLock;

pub struct MetricsRegistry {
    prefixes: RwLock<HashMap<String, String>>,
    counters: RwLock<HashMap<String, u64>>,
}

impl MetricsRegistry {
    pub fn new() -> Self {
        Self {
            prefixes: RwLock::new(HashMap::new()),
            counters: RwLock::new(HashMap::new()),
        }
    }

    pub fn register_prefix(&self, runtime_id: &str, prefix: &str) {
        if let Ok(mut map) = self.prefixes.write() {
            map.insert(runtime_id.to_string(), prefix.to_string());
        }
    }

    pub fn get_prefix(&self, runtime_id: &str) -> Option<String> {
        let map = self.prefixes.read().ok()?;
        map.get(runtime_id).cloned()
    }

    pub fn increment(&self, metric: &str, delta: u64) {
        if let Ok(mut map) = self.counters.write() {
            *map.entry(metric.to_string()).or_insert(0) += delta;
        }
    }

    pub fn get_counter(&self, metric: &str) -> u64 {
        let map = self.counters.read().ok();
        map.and_then(|m| m.get(metric).copied()).unwrap_or(0)
    }
}

impl Default for MetricsRegistry {
    fn default() -> Self {
        Self::new()
    }
}
