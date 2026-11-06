/// Inference response cache.
///
/// Caches `ModelResponse` objects keyed by a fast hash of the request's
/// deterministic fields. A cache hit completely bypasses provider calls,
/// with `ExecutionMetadata.cache_hit = true` in the response.
///
/// ## Cache key
///
/// Hashed fields: `capability`, `profile`, `messages`, `system_prompt`,
/// `max_tokens`, `temperature`. Non-deterministic fields (`request_id`,
/// `trace_id`, `stream`) are excluded from the key. Requests that offer
/// `tools` are never cached.
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
        if self.max_entries == 0 || !is_cacheable(request) {
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
        if self.max_entries == 0 || !is_cacheable(request) {
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

/// Requests that offer tools are never cached (ADR-017): a tool loop depends
/// on external state, so replaying an earlier answer would be wrong.
fn is_cacheable(request: &ModelRequest) -> bool {
    request.tools.is_empty()
}

fn compute_key(request: &ModelRequest) -> u64 {
    // Concatenate deterministic fields into a byte buffer, then hash.
    // Messages are serialized as JSON, which is unambiguous and order-preserving.
    let messages = serde_json::to_string(&request.messages).unwrap_or_default();
    let mut buf = String::with_capacity(
        messages.len() + request.system_prompt.as_deref().unwrap_or("").len() + 64,
    );
    buf.push_str(&format!("{:?}", request.capability));
    buf.push(':');
    buf.push_str(&format!("{:?}", request.profile));
    buf.push(':');
    buf.push_str(&messages);
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
        CircuitState, ExecutionMetadata, Message, ModelCapability, ModelResponse, ModelTier,
        StopReason, ToolSpec,
    };

    fn make_request(prompt: &str) -> ModelRequest {
        let mut req = ModelRequest::text(ModelCapability::Reasoning, prompt);
        req.max_tokens = 512;
        req.request_id = "test-req".into();
        req.trace_id = "test-trace".into();
        req
    }

    fn make_response(req: &ModelRequest) -> ModelResponse {
        ModelResponse {
            content: "test response".into(),
            tool_calls: Vec::new(),
            stop_reason: StopReason::EndTurn,
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
    fn different_histories_have_different_keys() {
        let cache = InferenceCache::new(100, 60);
        let short = make_request("hello");
        let mut long = make_request("hello");
        long.messages.push(Message::assistant("hi"));
        long.messages.push(Message::user("hello"));
        cache.insert(&short, make_response(&short));
        assert!(cache.get(&long).is_none());
    }

    #[test]
    fn requests_with_tools_are_never_cached() {
        let cache = InferenceCache::new(100, 60);
        let req = make_request("list files").with_tools(vec![ToolSpec {
            name: "ls".into(),
            description: "List files".into(),
            input_schema: serde_json::json!({ "type": "object" }),
        }]);
        cache.insert(&req, make_response(&req));
        assert!(cache.get(&req).is_none());
        assert!(cache.is_empty());
    }

    #[test]
    fn disabled_cache_always_misses() {
        let cache = InferenceCache::disabled();
        let req = make_request("hello");
        cache.insert(&req, make_response(&req));
        assert!(cache.get(&req).is_none());
    }
}
