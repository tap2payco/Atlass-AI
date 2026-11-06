/// Model profiles — the mapping layer between capabilities and concrete models.
///
/// ## Design intent
///
/// No model name ever appears in routing, request, or response types.
/// A `ModelProfile` owns the list of preferred and fallback model strings
/// for a specific (capability, quality profile) pair.
///
/// Swapping `deepseek-r1:7b` for `deepseek-r1:14b` requires editing one
/// profile, not searching the codebase for hardcoded strings.
///
/// ## Loading
///
/// Profiles are loaded from `ModelRuntimeConfig` (which reads YAML/env).
/// The `ProfileRegistry` provides a `resolve(capability, profile)` method
/// that returns the best-matched `ModelProfile`.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::domain::{CapabilityProfile, ModelCapability};

// ── ModelProfile ──────────────────────────────────────────────────────────────

/// Configuration for a specific (capability × quality) combination.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelProfile {
    pub capability: ModelCapability,
    pub quality_tier: CapabilityProfile,

    /// Ordered list of model identifiers to try (primary first).
    /// These are matched against `LocalModelRegistry` at runtime.
    pub preferred_models: Vec<String>,

    /// Models to try if all `preferred_models` fail or are unavailable.
    /// Typically larger models or external model IDs.
    pub fallback_models: Vec<String>,

    pub temperature: f32,
    pub max_context_tokens: u32,
    pub max_output_tokens: u32,
    pub supports_streaming: bool,
    pub supports_tool_calling: bool,
}

// ── Defaults ──────────────────────────────────────────────────────────────────

impl ModelProfile {
    /// Construct the built-in default profiles.
    ///
    /// These are used when no explicit config is provided. YAML configuration
    /// can override any field. Model strings here are Ollama tag format on the
    /// local side and Fireworks model path format on the external side.
    pub fn defaults() -> Vec<Self> {
        vec![
            // ── Reasoning ────────────────────────────────────────────────────
            Self {
                capability: ModelCapability::Reasoning,
                quality_tier: CapabilityProfile::Balanced,
                preferred_models: vec![
                    "deepseek-r1:7b".into(),
                    "qwen2.5:3b".into(),
                    "qwen2.5-coder:3b".into()
                ],
                fallback_models: vec!["accounts/fireworks/models/deepseek-r1".into()],
                temperature: 0.6,
                max_context_tokens: 32_768,
                max_output_tokens: 4_096,
                supports_streaming: true,
                supports_tool_calling: false,
            },
            Self {
                capability: ModelCapability::Reasoning,
                quality_tier: CapabilityProfile::HighQuality,
                preferred_models: vec!["deepseek-r1:14b".into()],
                fallback_models: vec!["accounts/fireworks/models/deepseek-r1".into()],
                temperature: 0.5,
                max_context_tokens: 65_536,
                max_output_tokens: 8_192,
                supports_streaming: true,
                supports_tool_calling: false,
            },
            Self {
                capability: ModelCapability::Reasoning,
                quality_tier: CapabilityProfile::LowMemory,
                preferred_models: vec!["deepseek-r1:1.5b".into()],
                fallback_models: vec!["accounts/fireworks/models/deepseek-r1".into()],
                temperature: 0.7,
                max_context_tokens: 16_384,
                max_output_tokens: 2_048,
                supports_streaming: true,
                supports_tool_calling: false,
            },
            Self {
                capability: ModelCapability::Reasoning,
                quality_tier: CapabilityProfile::Fast,
                preferred_models: vec!["deepseek-r1:1.5b".into()],
                fallback_models: vec!["accounts/fireworks/models/deepseek-r1".into()],
                temperature: 0.7,
                max_context_tokens: 8_192,
                max_output_tokens: 1_024,
                supports_streaming: true,
                supports_tool_calling: false,
            },
            // ── Code Generation ──────────────────────────────────────────────
            Self {
                capability: ModelCapability::CodeGeneration,
                quality_tier: CapabilityProfile::Balanced,
                preferred_models: vec!["qwen2.5-coder:7b".into()],
                fallback_models: vec![
                    "accounts/fireworks/models/qwen2p5-coder-32b-instruct".into()
                ],
                temperature: 0.2,
                max_context_tokens: 32_768,
                max_output_tokens: 8_192,
                supports_streaming: true,
                supports_tool_calling: true,
            },
            Self {
                capability: ModelCapability::CodeGeneration,
                quality_tier: CapabilityProfile::HighQuality,
                preferred_models: vec!["qwen2.5-coder:14b".into()],
                fallback_models: vec![
                    "accounts/fireworks/models/qwen2p5-coder-32b-instruct".into()
                ],
                temperature: 0.1,
                max_context_tokens: 65_536,
                max_output_tokens: 16_384,
                supports_streaming: true,
                supports_tool_calling: true,
            },
            // ── Code Analysis ────────────────────────────────────────────────
            Self {
                capability: ModelCapability::CodeAnalysis,
                quality_tier: CapabilityProfile::Balanced,
                preferred_models: vec!["qwen2.5-coder:7b".into()],
                fallback_models: vec![
                    "accounts/fireworks/models/qwen2p5-coder-32b-instruct".into()
                ],
                temperature: 0.3,
                max_context_tokens: 32_768,
                max_output_tokens: 4_096,
                supports_streaming: true,
                supports_tool_calling: false,
            },
            // ── Summarization ────────────────────────────────────────────────
            Self {
                capability: ModelCapability::Summarization,
                quality_tier: CapabilityProfile::Balanced,
                preferred_models: vec!["deepseek-r1:7b".into()],
                fallback_models: vec!["accounts/fireworks/models/deepseek-r1".into()],
                temperature: 0.4,
                max_context_tokens: 32_768,
                max_output_tokens: 2_048,
                supports_streaming: true,
                supports_tool_calling: false,
            },
            // ── Extraction ───────────────────────────────────────────────────
            Self {
                capability: ModelCapability::Extraction,
                quality_tier: CapabilityProfile::Fast,
                preferred_models: vec!["qwen2.5-coder:3b".into()],
                fallback_models: vec![
                    "accounts/fireworks/models/llama-v3p1-8b-instruct".into()
                ],
                temperature: 0.1,
                max_context_tokens: 16_384,
                max_output_tokens: 2_048,
                supports_streaming: false,
                supports_tool_calling: false,
            },
            Self {
                capability: ModelCapability::Extraction,
                quality_tier: CapabilityProfile::Balanced,
                preferred_models: vec!["qwen2.5-coder:7b".into()],
                fallback_models: vec![
                    "accounts/fireworks/models/llama-v3p1-8b-instruct".into()
                ],
                temperature: 0.1,
                max_context_tokens: 32_768,
                max_output_tokens: 4_096,
                supports_streaming: false,
                supports_tool_calling: false,
            },
            // ── Embedding — local only ────────────────────────────────────────
            Self {
                capability: ModelCapability::Embedding,
                quality_tier: CapabilityProfile::Balanced,
                preferred_models: vec!["bge-m3".into()],
                fallback_models: vec![], // intentionally empty: local-only
                temperature: 0.0,        // not applicable for embeddings
                max_context_tokens: 8_192,
                max_output_tokens: 0,
                supports_streaming: false,
                supports_tool_calling: false,
            },
            // ── Classification ────────────────────────────────────────────────
            Self {
                capability: ModelCapability::Classification,
                quality_tier: CapabilityProfile::Fast,
                preferred_models: vec!["qwen2.5-coder:3b".into()],
                fallback_models: vec![
                    "accounts/fireworks/models/llama-v3p1-8b-instruct".into()
                ],
                temperature: 0.0,
                max_context_tokens: 8_192,
                max_output_tokens: 512,
                supports_streaming: false,
                supports_tool_calling: false,
            },
        ]
    }
}

// ── ProfileRegistry ──────────────────────────────────────────────────────────

/// Resolves a (capability, quality tier) pair to a `ModelProfile`.
///
/// Falls back to `CapabilityProfile::Balanced` if an exact match for
/// the requested profile is not found.
#[derive(Clone)]
pub struct ProfileRegistry {
    // key: (capability display string, quality tier as string)
    profiles: HashMap<(String, String), Arc<ModelProfile>>,
}

impl ProfileRegistry {
    pub fn new(profiles: Vec<ModelProfile>) -> Self {
        let mut map = HashMap::new();
        for p in profiles {
            let key = (p.capability.to_string(), format!("{:?}", p.quality_tier));
            map.insert(key, Arc::new(p));
        }
        Self { profiles: map }
    }

    pub fn with_defaults() -> Self {
        Self::new(ModelProfile::defaults())
    }

    /// Resolve a profile for the given capability and quality tier.
    ///
    /// Falls back to `Balanced` if the exact tier is not registered.
    pub fn resolve(
        &self,
        capability: &ModelCapability,
        quality_tier: &CapabilityProfile,
    ) -> Option<Arc<ModelProfile>> {
        let key = (capability.to_string(), format!("{quality_tier:?}"));
        if let Some(p) = self.profiles.get(&key) {
            return Some(Arc::clone(p));
        }
        // Fallback: try Balanced
        let fallback_key = (capability.to_string(), "Balanced".to_string());
        self.profiles.get(&fallback_key).map(Arc::clone)
    }
}
