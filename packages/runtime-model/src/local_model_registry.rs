/// Local Model Registry — discovers models installed in Ollama at startup.
///
/// Instead of hardcoding which models are available, the runtime queries
/// `GET /api/tags` on the Ollama API and maps discovered models to
/// `ModelCapability` by matching name patterns.
///
/// ## Capability inference
///
/// The mapping is based on well-known model name patterns:
///
/// | Pattern          | Capabilities                             |
/// |---|---|
/// | `deepseek-r1`    | Reasoning, Summarization                 |
/// | `qwen2.5-coder`  | CodeGeneration, CodeAnalysis, Extraction |
/// | `qwen`           | Summarization, Extraction                |
/// | `bge-m3`         | Embedding                                |
/// | `llama`          | Reasoning, Summarization, Extraction     |
///
/// This inference is conservative — when in doubt, fewer capabilities
/// are advertised rather than more.
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::domain::ModelCapability;

// ── Discovered Model ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredModel {
    /// Fully qualified tag as returned by Ollama (e.g. "deepseek-r1:7b").
    pub name: String,
    /// Approximate model size in GB (derived from Ollama metadata).
    pub size_gb: f32,
    /// Capabilities inferred from the model name.
    pub capabilities: Vec<ModelCapability>,
}

// ── Ollama API response types ─────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModel>,
}

#[derive(Debug, Deserialize)]
struct OllamaModel {
    name: String,
    size: Option<u64>, // bytes
}

// ── LocalModelRegistry ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct LocalModelRegistry {
    pub available: Vec<DiscoveredModel>,
}

impl LocalModelRegistry {
    /// Query Ollama and build the registry.
    ///
    /// If Ollama is not running or returns an error, returns an empty registry
    /// (no panic). The router will then fall through to external providers.
    pub async fn discover(ollama_base_url: &str) -> Self {
        let url = format!("{ollama_base_url}/api/tags");
        match reqwest::get(&url).await {
            Ok(resp) => {
                if !resp.status().is_success() {
                    warn!(
                        status = %resp.status(),
                        "Ollama /api/tags returned non-200; treating as empty registry"
                    );
                    return Self { available: vec![] };
                }
                match resp.json::<OllamaTagsResponse>().await {
                    Ok(tags) => {
                        let models: Vec<DiscoveredModel> = tags
                            .models
                            .into_iter()
                            .map(|m| {
                                let caps = infer_capabilities(&m.name);
                                let size_gb =
                                    m.size.unwrap_or(0) as f32 / 1_073_741_824.0;
                                DiscoveredModel {
                                    name: m.name,
                                    size_gb,
                                    capabilities: caps,
                                }
                            })
                            .collect();
                        info!(count = models.len(), "Discovered local Ollama models");
                        Self { available: models }
                    }
                    Err(e) => {
                        warn!(error = %e, "Failed to parse Ollama /api/tags response");
                        Self { available: vec![] }
                    }
                }
            }
            Err(e) => {
                warn!(
                    error = %e,
                    "Could not reach Ollama at {ollama_base_url}; local model registry is empty"
                );
                Self { available: vec![] }
            }
        }
    }

    /// Check whether a specific model tag is available locally.
    pub fn is_available(&self, model_name: &str) -> bool {
        self.available.iter().any(|m| m.name == model_name)
    }

    /// Return all models that support a given capability.
    pub fn models_for(&self, capability: &ModelCapability) -> Vec<&DiscoveredModel> {
        self.available
            .iter()
            .filter(|m| m.capabilities.contains(capability))
            .collect()
    }
}

// ── Capability Inference ──────────────────────────────────────────────────────

fn infer_capabilities(name: &str) -> Vec<ModelCapability> {
    let lower = name.to_lowercase();
    let mut caps = Vec::new();

    if lower.contains("deepseek-r1") || lower.contains("deepseek_r1") {
        caps.push(ModelCapability::Reasoning);
        caps.push(ModelCapability::Summarization);
    }

    if lower.contains("qwen2.5-coder") || lower.contains("qwen2_5_coder") {
        caps.push(ModelCapability::Reasoning);
        caps.push(ModelCapability::CodeGeneration);
        caps.push(ModelCapability::CodeAnalysis);
        caps.push(ModelCapability::Extraction);
    }

    if lower.contains("qwen") && !lower.contains("coder") {
        caps.push(ModelCapability::Reasoning);
        caps.push(ModelCapability::Summarization);
        caps.push(ModelCapability::Extraction);
    }

    if lower.contains("bge-m3") || lower.contains("bge_m3") {
        caps.push(ModelCapability::Embedding);
    }

    if lower.contains("llama") {
        caps.push(ModelCapability::Reasoning);
        caps.push(ModelCapability::Summarization);
        caps.push(ModelCapability::Extraction);
    }

    if lower.contains("mistral") {
        caps.push(ModelCapability::Reasoning);
        caps.push(ModelCapability::Summarization);
    }

    // Deduplicate
    caps.sort_by_key(|c| format!("{c:?}"));
    caps.dedup_by_key(|c| format!("{c:?}"));
    caps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infer_deepseek_r1_capabilities() {
        let caps = infer_capabilities("deepseek-r1:7b");
        assert!(caps.contains(&ModelCapability::Reasoning));
        assert!(caps.contains(&ModelCapability::Summarization));
        assert!(!caps.contains(&ModelCapability::CodeGeneration));
    }

    #[test]
    fn infer_qwen_coder_capabilities() {
        let caps = infer_capabilities("qwen2.5-coder:7b");
        assert!(caps.contains(&ModelCapability::Reasoning));
        assert!(caps.contains(&ModelCapability::CodeGeneration));
        assert!(caps.contains(&ModelCapability::CodeAnalysis));
        assert!(caps.contains(&ModelCapability::Extraction));
    }

    #[test]
    fn infer_bge_m3_capabilities() {
        let caps = infer_capabilities("bge-m3");
        assert_eq!(caps, vec![ModelCapability::Embedding]);
    }
}
