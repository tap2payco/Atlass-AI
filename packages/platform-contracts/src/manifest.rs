use serde::{Deserialize, Serialize};
use crate::dependencies::DependencyDescriptor;

fn default_true() -> bool { true }
fn default_false() -> bool { false }
fn default_version() -> String { "v1.0.0".to_string() }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeManifest {
    pub name: String,
    #[serde(default)]
    pub runtime_id: String,
    pub version: String,
    pub api_version: String,
    #[serde(default = "default_version")]
    pub schema_version: String,
    #[serde(default)]
    pub build_hash: String,
    #[serde(default = "default_version")]
    pub minimum_kernel_version: String,
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<DependencyDescriptor>,
    pub health_endpoint: String,
    pub metrics_prefix: String,
    #[serde(default)]
    pub supported_events: Vec<String>,
    
    // Hot pluggable runtimes support
    #[serde(default = "default_true")]
    pub auto_start: bool,
    #[serde(default = "default_false")]
    pub hot_reload: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl RuntimeManifest {
    pub fn id(&self) -> &str {
        if self.runtime_id.is_empty() {
            &self.name
        } else {
            &self.runtime_id
        }
    }
}
