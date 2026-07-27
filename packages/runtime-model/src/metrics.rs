/// Runtime metrics for the Model Runtime.
///
/// Follows the same flat-struct pattern as `runtime-knowledge/src/metrics.rs`.
/// Accumulated during operation and exported via `observability::MetricsRecorder`.
#[derive(Debug, Default)]
pub struct InferenceMetrics {
    pub requests_total: u64,
    pub cache_hits: u64,
    pub fallbacks_triggered: u64,
    pub failures_total: u64,
    pub tokens_total: u64,
    pub latency_ms_total: u64,
}

#[derive(Debug, Default)]
pub struct CircuitBreakerMetrics {
    pub circuits_opened: u64,
    pub circuits_recovered: u64,
}

#[derive(Debug, Default)]
pub struct EmbeddingMetrics {
    pub requests_total: u64,
    pub latency_ms_total: u64,
}

#[derive(Debug, Default)]
pub struct ModelMetrics {
    pub inference: InferenceMetrics,
    pub circuit_breaker: CircuitBreakerMetrics,
    pub embedding: EmbeddingMetrics,
}
