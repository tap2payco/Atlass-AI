use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtlasConfig {
    pub environment: String,
    pub nats_url: String,
    pub database_url: String,
    pub log_level: String,
}

impl Default for AtlasConfig {
    fn default() -> Self {
        Self {
            environment: "development".to_string(),
            nats_url: "nats://localhost:4222".to_string(),
            database_url: "sqlite://atlas.db".to_string(),
            log_level: "info".to_string(),
        }
    }
}

pub trait ConfigLoader {
    fn load(&self, environment: &str) -> Result<AtlasConfig, crate::error::ConfigurationError>;
}
