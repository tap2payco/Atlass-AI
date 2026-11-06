use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;
use platform_contracts::manifest::RuntimeManifest;
use crate::health::HealthRegistry;
use crate::metrics::MetricsRegistry;

#[async_trait]
pub trait RuntimeLifecycle: Send + Sync {
    fn name(&self) -> &str;
    async fn startup(&self) -> Result<(), String>;
    async fn shutdown(&self) -> Result<(), String>;
}

pub struct RegisteredRuntime {
    pub manifest: RuntimeManifest,
    pub lifecycle: Arc<dyn RuntimeLifecycle>,
}

pub struct KernelSupervisor {
    runtimes: HashMap<String, RegisteredRuntime>,
    pub health_registry: Arc<HealthRegistry>,
    pub metrics_registry: Arc<MetricsRegistry>,
}

impl KernelSupervisor {
    pub fn new() -> Self {
        Self {
            runtimes: HashMap::new(),
            health_registry: Arc::new(HealthRegistry::new()),
            metrics_registry: Arc::new(MetricsRegistry::new()),
        }
    }

    pub fn register(&mut self, manifest: RuntimeManifest, lifecycle: Arc<dyn RuntimeLifecycle>) {
        let runtime_id = manifest.id().to_string();
        self.health_registry.register_endpoint(&runtime_id, &manifest.health_endpoint);
        self.metrics_registry.register_prefix(&runtime_id, &manifest.metrics_prefix);

        self.runtimes.insert(
            runtime_id,
            RegisteredRuntime { manifest, lifecycle },
        );
    }

    pub fn sort_topological_order(&self) -> Result<Vec<String>, String> {
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut adj: HashMap<String, Vec<String>> = HashMap::new();

        for id in self.runtimes.keys() {
            in_degree.insert(id.clone(), 0);
            adj.insert(id.clone(), Vec::new());
        }

        for (id, reg) in &self.runtimes {
            for dep in &reg.manifest.dependencies {
                let dep_id = dep.capability();
                if self.runtimes.contains_key(dep_id) {
                    adj.get_mut(dep_id).unwrap().push(id.clone());
                    *in_degree.get_mut(id).unwrap() += 1;
                }
            }
        }

        let mut queue: Vec<String> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(id, _)| id.clone())
            .collect();
        queue.sort();

        let mut order = Vec::new();
        while let Some(curr) = queue.pop() {
            order.push(curr.clone());
            if let Some(neighbors) = adj.get(&curr) {
                for neighbor in neighbors {
                    let deg = in_degree.get_mut(neighbor).unwrap();
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push(neighbor.clone());
                        queue.sort();
                    }
                }
            }
        }

        if order.len() != self.runtimes.len() {
            return Err("Cyclic dependency detected among registered runtimes".to_string());
        }

        Ok(order)
    }

    pub async fn boot_all(&self) -> Result<(), String> {
        let order = self.sort_topological_order()?;
        for id in &order {
            if let Some(reg) = self.runtimes.get(id) {
                if reg.manifest.enabled && reg.manifest.auto_start {
                    reg.lifecycle.startup().await?;
                    self.health_registry.update_state(id, platform_contracts::health::HealthState::Healthy);
                }
            }
        }
        Ok(())
    }

    pub async fn shutdown_all(&self) -> Result<(), String> {
        let mut order = self.sort_topological_order()?;
        order.reverse();

        for id in &order {
            if let Some(reg) = self.runtimes.get(id) {
                if reg.manifest.enabled {
                    let _ = reg.lifecycle.shutdown().await;
                    self.health_registry.update_state(id, platform_contracts::health::HealthState::Offline);
                }
            }
        }
        Ok(())
    }
}

impl Default for KernelSupervisor {
    fn default() -> Self {
        Self::new()
    }
}
