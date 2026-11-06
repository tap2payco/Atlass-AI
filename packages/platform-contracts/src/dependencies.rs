use serde::{Deserialize, Serialize};

fn default_min_version() -> String {
    "*".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DependencyDescriptor {
    Simple(String),
    Full {
        capability: String,
        #[serde(default = "default_min_version")]
        min_version: String,
        #[serde(default)]
        optional: bool,
    },
}

impl DependencyDescriptor {
    pub fn capability(&self) -> &str {
        match self {
            Self::Simple(cap) => cap,
            Self::Full { capability, .. } => capability,
        }
    }
}
