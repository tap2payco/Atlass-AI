//! # Atlas Model Runtime
//!
//! Capability-based inference routing with local-first providers
//! and an optional external fallback (Fireworks AI).
//!
//! ## Usage
//!
//! ```rust,no_run
//! use runtime_model::{ModelRuntime, ModelRuntimeConfig};
//! use runtime_model::domain::{ModelCapability, ModelRequest, CapabilityProfile, ExecutionPolicy};
//!
//! #[tokio::main]
//! async fn main() {
//!     let config = ModelRuntimeConfig::from_env();
//!     let runtime = ModelRuntime::build(config).await;
//!
//!     let response = runtime.execute_inference(ModelRequest {
//!         capability: ModelCapability::Reasoning,
//!         profile:    CapabilityProfile::Balanced,
//!         policy:     ExecutionPolicy::PreferLocal,
//!         prompt:     "Why is the sky blue?".into(),
//!         system_prompt: None,
//!         max_tokens: 512,
//!         temperature: 0.7,
//!         request_id: "req-001".into(),
//!         trace_id:   "trace-001".into(),
//!         stream: false,
//!     }).await.unwrap();
//!
//!     println!("{}", response.content);
//!     println!("Provider: {} ({})", response.metadata.provider_id, response.metadata.model_id);
//!     println!("Fallback used: {}", response.metadata.fallback_used);
//! }
//! ```
//!
//! ## Architecture
//!
//! ```text
//! ModelRuntime::execute_inference(ModelRequest)
//!     │
//!     ▼
//! InferenceCache  ──hit──► return (cache_hit=true)
//!     │ miss
//!     ▼
//! ProviderRegistry.resolve_ordered(capability, profile, policy)
//!     │   [OllamaProvider, FireworksProvider?]
//!     ▼
//! Router  ──► CircuitBreaker per provider
//!     │
//!     ├── OllamaProvider.infer()  ──ok──► cache + return
//!     │         ↓ fail
//!     └── FireworksProvider.infer() ──ok──► cache + return
//!                   ↓ fail
//!              Err(AllProvidersFailed)
//! ```

pub mod cache;
pub mod circuit_breaker;
pub mod config;
pub mod domain;
pub mod events;
pub mod local_model_registry;
pub mod metrics;
pub mod profile;
pub mod provider;
pub mod providers;
pub mod registry;
pub mod routing;

use std::sync::Arc;

use async_trait::async_trait;
use platform_contracts::health::HealthState;
use tokio::sync::mpsc;

use shared::error::InferenceError;

use crate::cache::InferenceCache;
use crate::config::ModelRuntimeConfig;
use crate::domain::{ExecutionMetadata, ModelRequest, ModelResponse};
use crate::events::ModelEvent;
use crate::local_model_registry::LocalModelRegistry;
use crate::profile::ProfileRegistry;
use crate::provider::ModelProvider;
use crate::providers::{
    external::fireworks::FireworksProvider,
    local::ollama::OllamaProvider,
};
use crate::registry::ProviderRegistry;
use crate::routing::Router;

// ── ModelRuntime ──────────────────────────────────────────────────────────────

/// The Atlas Model Runtime.
///
/// The single public type callers interact with. Implements `sdk::model::ModelRuntime`.
/// Constructed via `ModelRuntime::build(config)`.
pub struct ModelRuntime {
    router: Router,
}

impl ModelRuntime {
    /// Build the runtime from configuration.
    ///
    /// This is `async` because it calls `LocalModelRegistry::discover()` to
    /// query Ollama for installed models at startup.
    pub async fn build(config: ModelRuntimeConfig) -> Self {
        // 1. Discover local models
        let local_registry = LocalModelRegistry::discover(&config.ollama_base_url).await;

        // 2. Build profile registry (defaults + config overrides)
        let profile_registry = ProfileRegistry::with_defaults();

        // 3. Build provider list
        let mut providers: Vec<Arc<dyn ModelProvider>> = Vec::new();

        // Always add Ollama (it will report Offline if not running)
        providers.push(Arc::new(OllamaProvider::new(
            config.ollama_base_url.clone(),
            local_registry.clone(),
            profile_registry.clone(),
        )));

        // Add Fireworks only if an API key is present
        if let Some(ref key) = config.fireworks_api_key {
            if let Some(fw) =
                FireworksProvider::new(key.clone(), profile_registry.clone())
            {
                providers.push(Arc::new(fw));
            }
        }

        // 4. Build provider registry
        let provider_registry = Arc::new(ProviderRegistry::new(
            providers,
            profile_registry,
            local_registry,
            config.privacy_mode,
        ));

        // 5. Build inference cache
        let cache = Arc::new(InferenceCache::new(
            config.cache_max_entries,
            config.cache_ttl_seconds,
        ));

        // 6. Wire event sink (logs events via tracing for now)
        let event_sink: Arc<dyn Fn(ModelEvent) + Send + Sync> =
            Arc::new(|event: ModelEvent| {
                tracing::info!(
                    event = event.name(),
                    "model_runtime_event"
                );
            });

        // 7. Build router
        let router = Router::new(
            provider_registry,
            cache,
            config.circuit_breaker,
            Some(event_sink),
        );

        Self { router }
    }

    // ── Public API ────────────────────────────────────────────────────────────

    pub async fn execute_inference(
        &self,
        request: ModelRequest,
    ) -> Result<ModelResponse, InferenceError> {
        self.router.route(request).await
    }

    pub async fn execute_stream(
        &self,
        request: ModelRequest,
        tx: mpsc::Sender<String>,
    ) -> Result<ExecutionMetadata, InferenceError> {
        self.router.route_stream(request, tx).await
    }

    pub async fn health_check(&self) -> HealthState {
        // Return Healthy as a composite signal — individual provider health
        // is reflected in circuit breaker states surfaced via telemetry events.
        HealthState::Healthy
    }
}
