use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleEntity {
    pub id: String,
    pub package_id: String,
    pub path: String,
    pub language: String,
}
