use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyDescriptor {
    pub capability: String,
    pub min_version: String,
    #[serde(default)]
    pub optional: bool,
}
