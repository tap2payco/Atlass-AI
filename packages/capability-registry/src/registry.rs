use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use platform_contracts::capabilities::CapabilityDescriptor;
use platform_contracts::health::HealthState;

pub trait CapabilityProvider: Send + Sync {
    fn name(&self) -> &str;
}

/// Maps a capability name to the providers that offer it, best first.
///
/// Providers of one capability are kept sorted by `priority`, highest first.
/// Providers with equal priority keep their registration order, so the first
/// one registered wins a tie.
#[derive(Clone, Default)]
pub struct CapabilityRegistry {
    providers: Arc<RwLock<HashMap<String, Vec<CapabilityDescriptor>>>>,
}

impl CapabilityRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, descriptor: CapabilityDescriptor) {
        let mut map = self.providers.write().unwrap_or_else(|e| e.into_inner());
        let list = map.entry(descriptor.capability.clone()).or_default();
        list.push(descriptor);
        // Stable sort: equal priorities keep registration order.
        list.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    /// The best provider: the highest-priority healthy one, or, if none is
    /// healthy, the highest-priority one.
    pub fn resolve(&self, capability: &str) -> Option<CapabilityDescriptor> {
        let map = self.providers.read().unwrap_or_else(|e| e.into_inner());
        let list = map.get(capability)?;
        list.iter()
            .find(|d| d.health == HealthState::Healthy)
            .or_else(|| list.first())
            .cloned()
    }

    /// Every provider of `capability`, highest priority first.
    pub fn resolve_all(&self, capability: &str) -> Vec<CapabilityDescriptor> {
        let map = self.providers.read().unwrap_or_else(|e| e.into_inner());
        map.get(capability).cloned().unwrap_or_default()
    }

    /// Registered capability names, sorted.
    pub fn capabilities(&self) -> Vec<String> {
        let map = self.providers.read().unwrap_or_else(|e| e.into_inner());
        let mut names: Vec<String> = map.keys().cloned().collect();
        names.sort();
        names
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor(provider: &str, priority: u8, health: HealthState) -> CapabilityDescriptor {
        CapabilityDescriptor {
            capability: "cap".into(),
            version: "1".into(),
            provider: provider.into(),
            priority,
            health,
        }
    }

    #[test]
    fn highest_priority_wins_and_ties_keep_registration_order() {
        let registry = CapabilityRegistry::new();
        registry.register(descriptor("low", 1, HealthState::Healthy));
        registry.register(descriptor("first", 5, HealthState::Healthy));
        registry.register(descriptor("second", 5, HealthState::Healthy));
        assert_eq!(registry.resolve("cap").unwrap().provider, "first");
        let order: Vec<_> = registry.resolve_all("cap").into_iter().map(|d| d.provider).collect();
        assert_eq!(order, ["first", "second", "low"]);
        assert_eq!(registry.capabilities(), ["cap"]);
    }

    #[test]
    fn unhealthy_providers_are_skipped_when_a_healthy_one_exists() {
        let registry = CapabilityRegistry::new();
        registry.register(descriptor("down", 9, HealthState::Offline));
        registry.register(descriptor("up", 1, HealthState::Healthy));
        assert_eq!(registry.resolve("cap").unwrap().provider, "up");

        let only_down = CapabilityRegistry::new();
        only_down.register(descriptor("down", 9, HealthState::Offline));
        assert_eq!(only_down.resolve("cap").unwrap().provider, "down");
        assert!(only_down.resolve("missing").is_none());
    }
}
