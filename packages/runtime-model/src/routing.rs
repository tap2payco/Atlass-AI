/// Primary/fallback routing engine.
///
/// The Router is the only component that orchestrates calls to providers.
/// It consults the `ProviderRegistry`, the `CircuitBreaker` per provider,
/// and the `InferenceCache` before and after every call.
///
/// ## Algorithm
///
/// ```text
/// cache.get(request) → hit → return (cache_hit=true)
///                    → miss
///
/// registry.resolve_ordered(capability, profile, policy)
///   → [provider_a, provider_b, ...]
///
/// for each provider:
///   circuit_breaker.should_allow(provider) → false → skip (emit CircuitOpen event)
///   emit InferenceStarted
///   provider.infer(request)
///     → Ok  → circuit.record_success → cache.insert → return
///     → Err → circuit.record_failure → emit InferenceFailed
///               if circuit opened → emit CircuitOpened
///             → try next provider (emit ProviderFallback)
///
/// all providers exhausted → Err(AllProvidersFailed)
/// ```
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use tokio::sync::mpsc;
use tracing::{info, warn};

use shared::error::InferenceError;

use crate::cache::InferenceCache;
use crate::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig};
use crate::domain::{CircuitState, ExecutionMetadata, ModelRequest, ModelResponse, ModelTier};
use crate::events::{
    CircuitOpened, CircuitRecovered, InferenceCompleted, InferenceFailed, InferenceStarted,
    ModelEvent, ProviderFallback, CacheHit,
};
use crate::registry::ProviderRegistry;

pub struct Router {
    registry: Arc<ProviderRegistry>,
    circuit_breakers: Mutex<HashMap<String, Arc<CircuitBreaker>>>,
    cache: Arc<InferenceCache>,
    cb_config: CircuitBreakerConfig,
    event_sink: Option<Arc<dyn Fn(ModelEvent) + Send + Sync>>,
}

impl Router {
    pub fn new(
        registry: Arc<ProviderRegistry>,
        cache: Arc<InferenceCache>,
        cb_config: CircuitBreakerConfig,
        event_sink: Option<Arc<dyn Fn(ModelEvent) + Send + Sync>>,
    ) -> Self {
        Self {
            registry,
            circuit_breakers: Mutex::new(HashMap::new()),
            cache,
            cb_config,
            event_sink,
        }
    }

    // ── Public routing surface ────────────────────────────────────────────────

    pub async fn route(&self, request: ModelRequest) -> Result<ModelResponse, InferenceError> {
        // 1. Cache lookup
        if let Some(mut cached) = self.cache.get(&request) {
            self.emit(ModelEvent::CacheHit(CacheHit {
                request_id: request.request_id.clone(),
                capability: request.capability.clone(),
            }));
            cached.metadata.cache_hit = true;
            return Ok(cached);
        }

        // 2. Resolve ordered providers
        let providers = self.registry.resolve_ordered(
            &request.capability,
            &request.profile,
            &request.policy,
        );

        if providers.is_empty() {
            return Err(InferenceError::AllProvidersFailed);
        }

        let mut fallback_used = false;
        let mut retry_count: u32 = 0;

        for (idx, provider) in providers.iter().enumerate() {
            let pid = provider.provider_id().to_string();
            let cb = self.get_or_create_breaker(&pid);

            // 3. Circuit breaker gate
            if !cb.should_allow() {
                warn!(provider = %pid, "Circuit open — skipping provider");
                continue;
            }

            // 4. Emit start event
            self.emit(ModelEvent::InferenceStarted(InferenceStarted {
                request_id: request.request_id.clone(),
                trace_id: request.trace_id.clone(),
                capability: request.capability.clone(),
                provider: pid.clone(),
                model: String::new(), // provider fills model_id in response
                stream: request.stream,
            }));

            if idx > 0 {
                fallback_used = true;
            }

            let started = Instant::now();

            // 5. Call provider
            match provider.infer(&request).await {
                Ok(mut response) => {
                    let latency_ms = started.elapsed().as_millis() as u64;
                    cb.record_success();

                    // Enrich metadata
                    response.metadata.fallback_used = fallback_used;
                    response.metadata.retry_count = retry_count;
                    response.metadata.latency_ms = latency_ms;
                    response.metadata.cache_hit = false;
                    response.metadata.circuit_state = cb.state();

                    self.emit(ModelEvent::InferenceCompleted(InferenceCompleted {
                        request_id: request.request_id.clone(),
                        capability: request.capability.clone(),
                        provider: pid.clone(),
                        model: response.metadata.model_id.clone(),
                        latency_ms,
                        tokens_used: response.metadata.tokens_used,
                        cache_hit: false,
                        fallback_used,
                    }));

                    // Cache the result
                    self.cache.insert(&request, response.clone());

                    return Ok(response);
                }
                Err(e) => {
                    let new_state = cb.record_failure();
                    retry_count += 1;

                    warn!(
                        provider = %pid,
                        error = %e,
                        "Provider inference failed"
                    );

                    self.emit(ModelEvent::InferenceFailed(InferenceFailed {
                        request_id: request.request_id.clone(),
                        capability: request.capability.clone(),
                        provider: pid.clone(),
                        reason: e.to_string(),
                    }));

                    if new_state == CircuitState::Open {
                        info!(provider = %pid, failures = cb.failure_count(), "Circuit opened");
                        self.emit(ModelEvent::CircuitOpened(CircuitOpened {
                            provider: pid.clone(),
                            failure_count: cb.failure_count(),
                        }));
                    }

                    // Emit fallback event for next iteration
                    if let Some(next) = providers.get(idx + 1) {
                        self.emit(ModelEvent::ProviderFallback(ProviderFallback {
                            request_id: request.request_id.clone(),
                            from_provider: pid.clone(),
                            to_provider: next.provider_id().to_string(),
                            reason: e.to_string(),
                        }));
                    }
                }
            }
        }

        Err(InferenceError::AllProvidersFailed)
    }

    pub async fn route_stream(
        &self,
        request: ModelRequest,
        tx: mpsc::Sender<String>,
    ) -> Result<ExecutionMetadata, InferenceError> {
        let providers = self.registry.resolve_ordered(
            &request.capability,
            &request.profile,
            &request.policy,
        );

        if providers.is_empty() {
            return Err(InferenceError::AllProvidersFailed);
        }

        let mut fallback_used = false;
        let mut retry_count: u32 = 0;

        for (idx, provider) in providers.iter().enumerate() {
            let pid = provider.provider_id().to_string();
            let cb = self.get_or_create_breaker(&pid);

            if !cb.should_allow() {
                continue;
            }

            if idx > 0 {
                fallback_used = true;
            }

            let started = Instant::now();

            match provider.stream(&request, tx.clone()).await {
                Ok(mut meta) => {
                    cb.record_success();
                    meta.fallback_used = fallback_used;
                    meta.retry_count = retry_count;
                    meta.latency_ms = started.elapsed().as_millis() as u64;
                    meta.circuit_state = cb.state();
                    return Ok(meta);
                }
                Err(e) => {
                    cb.record_failure();
                    retry_count += 1;
                    warn!(provider = %pid, error = %e, "Stream failed, trying fallback");
                }
            }
        }

        Err(InferenceError::AllProvidersFailed)
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn get_or_create_breaker(&self, provider_id: &str) -> Arc<CircuitBreaker> {
        let mut map = self.circuit_breakers.lock().unwrap();
        map.entry(provider_id.to_string())
            .or_insert_with(|| Arc::new(CircuitBreaker::new(self.cb_config.clone())))
            .clone()
    }

    fn emit(&self, event: ModelEvent) {
        if let Some(ref sink) = self.event_sink {
            sink(event);
        }
    }
}
