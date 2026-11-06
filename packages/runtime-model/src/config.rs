/// Configuration for the Model Runtime.
///
/// Loaded once at startup and passed to `ModelRuntime::new`. Can be
/// populated from environment variables, YAML, or default values.
///
/// ## Environment variables
///
/// | Variable | Default | Description |
/// |---|---|---|
/// | `ATLAS_OLLAMA_URL` | `http://localhost:11434` | Ollama base URL |
/// | `ATLAS_OLLAMA_MODEL` | _(none)_ | Use this Ollama model for every non-embedding request, instead of the per-profile model lists |
/// | `ATLAS_FIREWORKS_API_KEY` | _(none)_ | Enables Fireworks provider |
/// | `ATLAS_MODEL_PRIVACY_MODE` | `false` | Block all external providers |
/// | `ATLAS_MODEL_CACHE_MAX` | `256` | Max cached responses |
/// | `ATLAS_MODEL_CACHE_TTL` | `300` | Cache TTL in seconds |
/// | `ATLAS_MODEL_CB_THRESHOLD` | `3` | Circuit breaker failure threshold |
/// | `ATLAS_MODEL_CB_RECOVERY` | `30` | Circuit recovery window in seconds |
use std::time::Duration;

use crate::circuit_breaker::CircuitBreakerConfig;

#[derive(Debug, Clone)]
pub struct ModelRuntimeConfig {
    /// Base URL for the local Ollama instance.
    pub ollama_base_url: String,

    /// When set, Ollama serves every non-embedding capability with this model
    /// instead of resolving one from the profile lists.
    pub ollama_model_override: Option<String>,

    /// API key for Fireworks AI. If `None`, the Fireworks provider is not registered.
    pub fireworks_api_key: Option<String>,

    /// When `true`, all `External` tier providers are filtered out of the
    /// routing pool before any inference attempt. No data leaves the machine.
    /// Equivalent to `RequireLocal` policy applied globally.
    pub privacy_mode: bool,

    /// Maximum number of responses to keep in the inference cache.
    pub cache_max_entries: usize,

    /// How long cached responses remain valid.
    pub cache_ttl_seconds: u64,

    /// Circuit breaker configuration (applies to every provider).
    pub circuit_breaker: CircuitBreakerConfig,

    /// Default `max_tokens` used when a request does not specify one.
    pub default_max_tokens: u32,
}

impl Default for ModelRuntimeConfig {
    fn default() -> Self {
        Self {
            ollama_base_url: "http://localhost:11434".into(),
            ollama_model_override: None,
            fireworks_api_key: None,
            privacy_mode: false,
            cache_max_entries: 256,
            cache_ttl_seconds: 300,
            circuit_breaker: CircuitBreakerConfig {
                failure_threshold: 3,
                recovery_window: Duration::from_secs(30),
            },
            default_max_tokens: 4_096,
        }
    }
}

impl ModelRuntimeConfig {
    /// Build config from environment variables, falling back to defaults.
    pub fn from_env() -> Self {
        let mut config = Self::default();

        if let Ok(url) = std::env::var("ATLAS_OLLAMA_URL") {
            config.ollama_base_url = url;
        }
        if let Ok(model) = std::env::var("ATLAS_OLLAMA_MODEL") {
            let model = model.trim().to_string();
            if !model.is_empty() {
                config.ollama_model_override = Some(model);
            }
        }
        if let Ok(key) = std::env::var("ATLAS_FIREWORKS_API_KEY") {
            config.fireworks_api_key = Some(key);
        }
        if let Ok(val) = std::env::var("ATLAS_MODEL_PRIVACY_MODE") {
            config.privacy_mode = val.to_lowercase() == "true" || val == "1";
        }
        if let Ok(val) = std::env::var("ATLAS_MODEL_CACHE_MAX") {
            if let Ok(n) = val.parse::<usize>() {
                config.cache_max_entries = n;
            }
        }
        if let Ok(val) = std::env::var("ATLAS_MODEL_CACHE_TTL") {
            if let Ok(n) = val.parse::<u64>() {
                config.cache_ttl_seconds = n;
            }
        }
        if let Ok(val) = std::env::var("ATLAS_MODEL_CB_THRESHOLD") {
            if let Ok(n) = val.parse::<u32>() {
                config.circuit_breaker.failure_threshold = n;
            }
        }
        if let Ok(val) = std::env::var("ATLAS_MODEL_CB_RECOVERY") {
            if let Ok(n) = val.parse::<u64>() {
                config.circuit_breaker.recovery_window = Duration::from_secs(n);
            }
        }

        config
    }
}
