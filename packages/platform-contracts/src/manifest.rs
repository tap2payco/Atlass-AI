use serde::{Deserialize, Serialize};
use crate::dependencies::DependencyDescriptor;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeManifest {
    pub name: String,
    pub runtime_id: String,
    pub version: String,
    pub api_version: String,
    pub schema_version: String,
    pub build_hash: String,
    pub minimum_kernel_version: String,
    pub capabilities: Vec<String>,
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

fn default_true() -> bool { true }
fn default_false() -> bool { false }
