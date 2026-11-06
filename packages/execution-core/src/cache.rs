use std::collections::HashMap;
use std::hash::Hash;
use std::sync::RwLock;
use std::time::{Duration, Instant};
use async_trait::async_trait;

#[async_trait]
pub trait Cache<K: Send + Sync, V: Send + Sync>: Send + Sync {
    async fn get(&self, key: &K) -> Option<V>;
    async fn set(&self, key: K, value: V);
    async fn remove(&self, key: &K) -> Option<V>;
    async fn clear(&self);
}

struct CacheEntry<V> {
    value: V,
    created_at: Instant,
    ttl: Option<Duration>,
}

pub struct MemoryCache<K, V> {
    store: RwLock<HashMap<K, CacheEntry<V>>>,
    default_ttl: Option<Duration>,
    max_capacity: usize,
}

impl<K: Eq + Hash + Clone + Send + Sync, V: Clone + Send + Sync> MemoryCache<K, V> {
    pub fn new(default_ttl: Option<Duration>, max_capacity: usize) -> Self {
        Self {
            store: RwLock::new(HashMap::new()),
            default_ttl,
            max_capacity,
        }
    }

    pub fn with_ttl(ttl: Duration) -> Self {
        Self::new(Some(ttl), 10_000)
    }

    pub fn unbounded() -> Self {
        Self::new(None, usize::MAX)
    }

    pub fn len(&self) -> usize {
        self.store.read().map(|s| s.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<K: Eq + Hash + Clone + Send + Sync, V: Clone + Send + Sync> Default for MemoryCache<K, V> {
    fn default() -> Self {
        Self::new(Some(Duration::from_secs(300)), 1000)
    }
}

#[async_trait]
impl<K: Eq + Hash + Clone + Send + Sync, V: Clone + Send + Sync> Cache<K, V> for MemoryCache<K, V> {
    async fn get(&self, key: &K) -> Option<V> {
        let store = self.store.read().ok()?;
        if let Some(entry) = store.get(key) {
            if let Some(ttl) = entry.ttl {
                if entry.created_at.elapsed() > ttl {
                    return None;
                }
            }
            return Some(entry.value.clone());
        }
        None
    }

    async fn set(&self, key: K, value: V) {
        if let Ok(mut store) = self.store.write() {
            // Evict arbitrary entry if over capacity
            if store.len() >= self.max_capacity && !store.contains_key(&key) {
                if let Some(first_key) = store.keys().next().cloned() {
                    store.remove(&first_key);
                }
            }
            store.insert(
                key,
                CacheEntry {
                    value,
                    created_at: Instant::now(),
                    ttl: self.default_ttl,
                },
            );
        }
    }

    async fn remove(&self, key: &K) -> Option<V> {
        if let Ok(mut store) = self.store.write() {
            store.remove(key).map(|entry| entry.value)
        } else {
            None
        }
    }

    async fn clear(&self) {
        if let Ok(mut store) = self.store.write() {
            store.clear();
        }
    }
}
