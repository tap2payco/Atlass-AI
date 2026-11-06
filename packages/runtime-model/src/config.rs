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
/// | `ATLAS_OLLAMA_NUM_CTX` | `8192` | Context window requested from Ollama (`0` = Ollama's own default) |
/// | `ATLAS_OLLAMA_TOOL_MODE` | `auto` | How tools reach Ollama models: `native` (Ollama's `tools` field only), `emulated` (tools described in the system prompt and JSON calls parsed from the reply, for models without tool support) or `auto` (native, switching a model to emulation the first time Ollama says it does not support tools) |
/// | `ATLAS_FIREWORKS_API_KEY` | _(none)_ | Enables Fireworks provider |
/// | `ATLAS_MODEL_PRIVACY_MODE` | `false` | Block all external providers |
/// | `ATLAS_MODEL_CACHE_MAX` | `256` | Max cached responses |
/// | `ATLAS_MODEL_CACHE_TTL` | `300` | Cache TTL in seconds |
/// | `ATLAS_MODEL_CB_THRESHOLD` | `3` | Circuit breaker failure threshold |
/// | `ATLAS_MODEL_CB_RECOVERY` | `30` | Circuit recovery window in seconds |
use std::time::Duration;

use crate::circuit_breaker::CircuitBreakerConfig;

/// Context window requested from Ollama unless `ATLAS_OLLAMA_NUM_CTX` says otherwise.
pub const DEFAULT_OLLAMA_NUM_CTX: u32 = 8_192;

/// How Ollama serves requests that carry tools (`ATLAS_OLLAMA_TOOL_MODE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OllamaToolMode {
    /// Always send tools in Ollama's `tools` field.
    Native,
    /// Always describe tools in the system prompt and parse JSON calls from
    /// the reply (Goose-style emulation).
    Emulated,
    /// Native first; a model Ollama rejects with "does not support tools" is
    /// retried, and from then on served, in emulation.
    #[default]
    Auto,
}

impl OllamaToolMode {
    /// Parse `native`, `emulated` (or `emulate`) or `auto`, ignoring case.
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "native" => Some(Self::Native),
            "emulated" | "emulate" | "emulation" => Some(Self::Emulated),
            "auto" | "" => Some(Self::Auto),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ModelRuntimeConfig {
    /// Base URL for the local Ollama instance.
    pub ollama_base_url: String,

    /// When set, Ollama serves every non-embedding capability with this model
    /// instead of resolving one from the profile lists.
    pub ollama_model_override: Option<String>,

    /// Context window, in tokens, requested from Ollama on every chat call.
    /// `None` leaves Ollama's own default (often 2–4k), which silently drops
    /// the start of longer conversations.
    pub ollama_num_ctx: Option<u32>,

    /// Native tools, emulated tools, or native with automatic fallback.
    pub ollama_tool_mode: OllamaToolMode,

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
            ollama_num_ctx: Some(DEFAULT_OLLAMA_NUM_CTX),
            ollama_tool_mode: OllamaToolMode::Auto,
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
        if let Ok(val) = std::env::var("ATLAS_OLLAMA_NUM_CTX") {
            if let Ok(n) = val.trim().parse::<u32>() {
                config.ollama_num_ctx = (n > 0).then_some(n);
            }
        }
        if let Ok(val) = std::env::var("ATLAS_OLLAMA_TOOL_MODE") {
            match OllamaToolMode::parse(&val) {
                Some(mode) => config.ollama_tool_mode = mode,
                None => tracing::warn!(
                    value = %val,
                    "ATLAS_OLLAMA_TOOL_MODE must be native, emulated or auto; using auto"
                ),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_mode_parses_and_defaults_to_auto() {
        assert_eq!(OllamaToolMode::parse("native"), Some(OllamaToolMode::Native));
        assert_eq!(OllamaToolMode::parse(" Emulated "), Some(OllamaToolMode::Emulated));
        assert_eq!(OllamaToolMode::parse("AUTO"), Some(OllamaToolMode::Auto));
        assert_eq!(OllamaToolMode::parse("sometimes"), None);
        assert_eq!(ModelRuntimeConfig::default().ollama_tool_mode, OllamaToolMode::Auto);
    }
}
