use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};
use shared::context::ContextPackage;
use sdk::context::ContextRequest;

struct CacheEntry {
    package: ContextPackage,
    created_at: Instant,
    ttl: Duration,
}

pub struct ContextCache {
    store: RwLock<HashMap<String, CacheEntry>>,
    default_ttl: Duration,
}

impl ContextCache {
    pub fn new(default_ttl: Duration) -> Self {
        Self {
            store: RwLock::new(HashMap::new()),
            default_ttl,
        }
    }

    pub fn build_cache_key(request: &ContextRequest) -> String {
        let mut key = format!("{:?}:{:?}", request.intent, request.max_tokens);
        let mut sorted_entities = request.required_entities.clone();
        sorted_entities.sort();
        for entity in sorted_entities {
            key.push('|');
            key.push_str(&entity);
        }
        key
    }

    pub fn get(&self, key: &str) -> Option<ContextPackage> {
        let store = self.store.read().ok()?;
        if let Some(entry) = store.get(key) {
            if entry.created_at.elapsed() <= entry.ttl {
                return Some(entry.package.clone());
            }
        }
        None
    }

    pub fn set(&self, key: String, package: ContextPackage) {
        if let Ok(mut store) = self.store.write() {
            store.insert(
                key,
                CacheEntry {
                    package,
                    created_at: Instant::now(),
                    ttl: self.default_ttl,
                },
            );
        }
    }

    pub fn clear(&self) {
        if let Ok(mut store) = self.store.write() {
            store.clear();
        }
    }

    pub fn len(&self) -> usize {
        self.store.read().map(|s| s.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for ContextCache {
    fn default() -> Self {
        Self::new(Duration::from_secs(300))
    }
}
