//! Tool lookup through `capability-registry`.
//!
//! Every tool is registered as a capability named after the tool, offered by
//! a provider (e.g. "builtin", or an MCP server name) with a priority. When
//! several providers offer a tool of the same name, the highest priority
//! wins; two providers with the same top priority are a configuration error,
//! as two tools with one name always were.

use std::collections::HashMap;
use std::sync::Arc;

use capability_registry::registry::CapabilityRegistry;
use platform_contracts::capabilities::CapabilityDescriptor;
use platform_contracts::health::HealthState;

use crate::error::AgentError;
use crate::tool::Tool;

/// Priority of tools added with `AgentBuilder::tool`.
pub const DEFAULT_TOOL_PRIORITY: u8 = 100;
/// Provider name of tools added with `AgentBuilder::tool`.
pub const BUILTIN_PROVIDER: &str = "builtin";

#[derive(Default)]
pub struct ToolRegistry {
    registry: CapabilityRegistry,
    /// Keyed by the descriptor's unique provider key.
    tools: HashMap<String, Arc<dyn Tool>>,
    /// Tool names in first-registration order, so the model sees a stable list.
    names: Vec<String>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, tool: Arc<dyn Tool>, provider: &str, priority: u8) {
        let name = tool.spec().name;
        let key = format!("{provider}#{}", self.tools.len());
        self.registry.register(CapabilityDescriptor {
            capability: name.clone(),
            version: "1".into(),
            provider: key.clone(),
            priority,
            health: HealthState::Healthy,
        });
        self.tools.insert(key, tool);
        if !self.names.contains(&name) {
            self.names.push(name);
        }
    }

    /// The tool that answers to `name`: the highest-priority provider's.
    pub fn resolve(&self, name: &str) -> Option<Arc<dyn Tool>> {
        let descriptor = self.registry.resolve(name)?;
        self.tools.get(&descriptor.provider).cloned()
    }

    /// One tool per name, in registration order. Fails if the top two
    /// providers of a name share a priority (an ambiguous lookup).
    pub fn resolved(&self) -> Result<Vec<Arc<dyn Tool>>, AgentError> {
        let mut out = Vec::with_capacity(self.names.len());
        for name in &self.names {
            let providers = self.registry.resolve_all(name);
            if let [first, second, ..] = providers.as_slice() {
                if first.priority == second.priority {
                    return Err(AgentError::Config(format!("duplicate tool name '{name}'")));
                }
            }
            if let Some(tool) = self.resolve(name) {
                out.push(tool);
            }
        }
        Ok(out)
    }
}
