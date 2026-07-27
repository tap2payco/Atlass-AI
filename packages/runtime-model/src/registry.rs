/// Provider Registry — resolves capabilities to ordered provider lists.
///
/// This is the bridge between:
/// - `ProfileRegistry`: which model names are preferred for a capability
/// - `LocalModelRegistry`: which models are actually installed
/// - `ModelProvider` implementations: the concrete adapters
///
/// ## Resolution algorithm
///
/// 1. Look up the `ModelProfile` for (capability, quality_tier).
/// 2. Walk `preferred_models` — include the provider that owns each model.
/// 3. Walk `fallback_models` — include the provider that owns each model.
/// 4. Deduplicate while preserving order (local providers always precede external).
/// 5. Filter: if privacy_mode is active, drop all External-tier providers.
///
/// The result is an ordered `Vec<Arc<dyn ModelProvider>>` ready for the router.
use std::collections::HashMap;
use std::sync::Arc;

use crate::domain::{CapabilityProfile, ExecutionPolicy, ModelCapability, ModelTier};
use crate::local_model_registry::LocalModelRegistry;
use crate::profile::ProfileRegistry;
use crate::provider::ModelProvider;

pub struct ProviderRegistry {
    /// All registered providers, keyed by `provider_id`.
    providers: HashMap<String, Arc<dyn ModelProvider>>,
    profile_registry: ProfileRegistry,
    local_model_registry: LocalModelRegistry,
    privacy_mode: bool,
}

impl ProviderRegistry {
    pub fn new(
        providers: Vec<Arc<dyn ModelProvider>>,
        profile_registry: ProfileRegistry,
        local_model_registry: LocalModelRegistry,
        privacy_mode: bool,
    ) -> Self {
        let map = providers
            .into_iter()
            .map(|p| (p.provider_id().to_string(), p))
            .collect();
        Self {
            providers: map,
            profile_registry,
            local_model_registry,
            privacy_mode,
        }
    }

    /// Resolve an ordered list of providers for the given capability and quality tier.
    ///
    /// Returns providers sorted: Local first, External last.
    /// Providers filtered by:
    /// - They must advertise the requested capability.
    /// - If `privacy_mode` is active, External providers are excluded.
    /// - If `policy` is `RequireLocal`, External providers are excluded.
    pub fn resolve_ordered(
        &self,
        capability: &ModelCapability,
        profile: &CapabilityProfile,
        policy: &ExecutionPolicy,
    ) -> Vec<Arc<dyn ModelProvider>> {
        let exclude_external = self.privacy_mode
            || matches!(policy, ExecutionPolicy::RequireLocal);

        // Get all providers that advertise the requested capability
        let mut candidates: Vec<Arc<dyn ModelProvider>> = self
            .providers
            .values()
            .filter(|p| {
                if exclude_external && p.tier() == ModelTier::External {
                    return false;
                }
                p.supported_capabilities().contains(capability)
            })
            .cloned()
            .collect();

        // Sort: Local before External, then by model profile preference order
        let profile_entry = self.profile_registry.resolve(capability, profile);
        candidates.sort_by(|a, b| {
            // Local tier sorts before External
            let tier_ord = a.tier().cmp_order().cmp(&b.tier().cmp_order());
            if tier_ord != std::cmp::Ordering::Equal {
                return tier_ord;
            }

            // Within same tier, sort by position in preferred_models list
            if let Some(ref prof) = profile_entry {
                let pos_a = model_position(a.provider_id(), &prof.preferred_models, &prof.fallback_models);
                let pos_b = model_position(b.provider_id(), &prof.preferred_models, &prof.fallback_models);
                return pos_a.cmp(&pos_b);
            }

            std::cmp::Ordering::Equal
        });

        candidates
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

trait TierOrd {
    fn cmp_order(&self) -> u8;
}

impl TierOrd for ModelTier {
    fn cmp_order(&self) -> u8 {
        match self {
            ModelTier::Local => 0,
            ModelTier::External => 1,
        }
    }
}

fn model_position(provider_id: &str, preferred: &[String], fallback: &[String]) -> usize {
    // Preferred list gets positions 0..N, fallback gets N..2N
    for (i, m) in preferred.iter().enumerate() {
        if m.contains(provider_id) || provider_id.contains(m.as_str()) {
            return i;
        }
    }
    for (i, m) in fallback.iter().enumerate() {
        if m.contains(provider_id) || provider_id.contains(m.as_str()) {
            return preferred.len() + i;
        }
    }
    usize::MAX // unknown providers sort last
}
