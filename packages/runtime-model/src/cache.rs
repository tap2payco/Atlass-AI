/// Inference response cache.
///
/// Caches `ModelResponse` objects keyed by a fast hash of the request's
/// deterministic fields. A cache hit completely bypasses provider calls,
/// with `ExecutionMetadata.cache_hit = true` in the response.
///
/// ## Cache key
///
/// Hashed fields: `capability`, `profile`, `prompt`, `system_prompt`,
/// `max_tokens`, `temperature`. Non-deterministic fields (`request_id`,
/// `trace_id`, `stream`) are excluded from the key.
///
/// ## Eviction
///
/// Simple LRU-style eviction based on `max_entries`. TTL-based expiry
/// is checked on every lookup — expired entries are treated as misses.
///
/// ## Thread safety
///
/// The `InferenceCache` is `Send + Sync` via `std::sync::Mutex`.
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use xxhash_rust::xxh64::xxh64;

use crate::domain::{ModelRequest, ModelResponse};

// ── Cache Entry ───────────────────────────────────────────────────────────────

struct CacheEntry {
    response: ModelResponse,
    inserted_at: Instant,
}

// ── InferenceCache ────────────────────────────────────────────────────────────

pub struct InferenceCache {
    store: Mutex<HashMap<u64, CacheEntry>>,
    max_entries: usize,
    ttl: Duration,
}

impl InferenceCache {
    pub fn new(max_entries: usize, ttl_seconds: u64) -> Self {
        Self {
            store: Mutex::new(HashMap::with_capacity(max_entries)),
            max_entries,
            ttl: Duration::from_secs(ttl_seconds),
        }
    }

    /// Disabled cache (zero TTL — every lookup is a miss).
    pub fn disabled() -> Self {
        Self::new(0, 0)
    }

    /// Look up a cached response for the given request.
    ///
    /// Returns `None` on miss or if the entry has expired.
    pub fn get(&self, request: &ModelRequest) -> Option<ModelResponse> {
        if self.max_entries == 0 {
            return None;
        }
        let key = compute_key(request);
        let mut store = self.store.lock().unwrap();
        if let Some(entry) = store.get(&key) {
            if entry.inserted_at.elapsed() < self.ttl {
                return Some(entry.response.clone());
            } else {
                store.remove(&key);
            }
        }
        None
    }

    /// Store a response in the cache.
    ///
    /// If the cache is at capacity, the oldest entry is evicted first.
    pub fn insert(&self, request: &ModelRequest, response: ModelResponse) {
        if self.max_entries == 0 {
            return;
        }
        let key = compute_key(request);
        let mut store = self.store.lock().unwrap();

        // Evict oldest entry if at capacity
        if store.len() >= self.max_entries {
            if let Some(oldest_key) = store
                .iter()
                .min_by_key(|(_, e)| e.inserted_at)
                .map(|(k, _)| *k)
            {
                store.remove(&oldest_key);
            }
        }

        store.insert(
            key,
            CacheEntry {
                response,
                inserted_at: Instant::now(),
            },
        );
    }

    /// Number of live (non-expired) entries.
    pub fn len(&self) -> usize {
        let store = self.store.lock().unwrap();
        let now = Instant::now();
        store
            .values()
            .filter(|e| now.duration_since(e.inserted_at) < self.ttl)
            .count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

// ── Key computation ───────────────────────────────────────────────────────────

fn compute_key(request: &ModelRequest) -> u64 {
    // Concatenate deterministic fields into a byte buffer, then hash.
    let mut buf = String::with_capacity(
        request.prompt.len() + request.system_prompt.as_deref().unwrap_or("").len() + 64,
    );
    buf.push_str(&format!("{:?}", request.capability));
    buf.push(':');
    buf.push_str(&format!("{:?}", request.profile));
    buf.push(':');
    buf.push_str(&request.prompt);
    buf.push(':');
    buf.push_str(request.system_prompt.as_deref().unwrap_or(""));
    buf.push(':');
    buf.push_str(&request.max_tokens.to_string());
    buf.push(':');
    // Temperature: use bit representation to avoid float formatting issues
    let temp_bits = request.temperature.to_bits();
    buf.push_str(&temp_bits.to_string());

    xxh64(buf.as_bytes(), 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        CapabilityProfile, CircuitState, ExecutionMetadata, ExecutionPolicy,
        ModelCapability, ModelResponse, ModelTier,
    };

    fn make_request(prompt: &str) -> ModelRequest {
        ModelRequest {
            capability: ModelCapability::Reasoning,
            profile: CapabilityProfile::Balanced,
            policy: ExecutionPolicy::PreferLocal,
            prompt: prompt.to_string(),
            system_prompt: None,
            max_tokens: 512,
            temperature: 0.7,
            request_id: "test-req".into(),
            trace_id: "test-trace".into(),
            stream: false,
        }
    }

    fn make_response(req: &ModelRequest) -> ModelResponse {
        ModelResponse {
            content: "test response".into(),
            capability: req.capability.clone(),
            metadata: ExecutionMetadata {
                provider_id: "ollama".into(),
                model_id: "deepseek-r1:7b".into(),
                tier: ModelTier::Local,
                latency_ms: 100,
                tokens_used: 50,
                cache_hit: false,
                fallback_used: false,
                retry_count: 0,
                circuit_state: CircuitState::Closed,
            },
            request_id: req.request_id.clone(),
        }
    }

    #[test]
    fn miss_on_empty_cache() {
        let cache = InferenceCache::new(100, 60);
        let req = make_request("hello");
        assert!(cache.get(&req).is_none());
    }

    #[test]
    fn hit_after_insert() {
        let cache = InferenceCache::new(100, 60);
        let req = make_request("hello");
        let resp = make_response(&req);
        cache.insert(&req, resp.clone());
        let hit = cache.get(&req);
        assert!(hit.is_some());
        assert_eq!(hit.unwrap().content, "test response");
    }

    #[test]
    fn different_prompts_have_different_keys() {
        let cache = InferenceCache::new(100, 60);
        let req1 = make_request("hello");
        let req2 = make_request("world");
        cache.insert(&req1, make_response(&req1));
        assert!(cache.get(&req2).is_none());
    }

    #[test]
    fn disabled_cache_always_misses() {
        let cache = InferenceCache::disabled();
        let req = make_request("hello");
        cache.insert(&req, make_response(&req));
        assert!(cache.get(&req).is_none());
    }
}
