use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryProfile {
    pub repository_type: String,
    pub workspace_type: String,
    pub languages: Vec<String>,
    pub frameworks: Vec<String>,
    pub toolchains: Vec<String>,
    pub package_managers: Vec<String>,
    pub testing_frameworks: Vec<String>,
    pub ci_systems: Vec<String>,
}

impl Default for RepositoryProfile {
    fn default() -> Self {
        Self {
            repository_type: "unknown".to_string(),
            workspace_type: "unknown".to_string(),
            languages: vec![],
            frameworks: vec![],
            toolchains: vec![],
            package_managers: vec![],
            testing_frameworks: vec![],
            ci_systems: vec![],
        }
    }
}

pub fn detect_metadata(_path: &std::path::Path) -> RepositoryProfile {
    // Stub for layer A implementation
    RepositoryProfile {
        languages: vec!["Rust".to_string()],
        package_managers: vec!["Cargo".to_string()],
        ..Default::default()
    }
}
