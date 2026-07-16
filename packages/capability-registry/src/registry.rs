use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use platform_contracts::capabilities::CapabilityDescriptor;

pub trait CapabilityProvider: Send + Sync {
    fn name(&self) -> &str;
}

#[derive(Clone)]
pub struct CapabilityRegistry {
    // Maps a capability string to a list of descriptors (providers)
    providers: Arc<RwLock<HashMap<String, Vec<CapabilityDescriptor>>>>,
}

impl CapabilityRegistry {
    pub fn new() -> Self {
        Self {
            providers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn register(&self, descriptor: CapabilityDescriptor) {
        let mut map = self.providers.write().unwrap();
        let list = map.entry(descriptor.capability.clone()).or_insert_with(Vec::new);
        list.push(descriptor);
        // Could sort by priority here
    }

    pub fn resolve(&self, capability: &str) -> Option<CapabilityDescriptor> {
        let map = self.providers.read().unwrap();
        map.get(capability).and_then(|list| list.first().cloned())
    }
}
