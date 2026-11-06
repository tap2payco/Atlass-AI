use platform_contracts::manifest::RuntimeManifest;

pub struct ConfigLoader;

impl ConfigLoader {
    pub fn load_manifest(path: &str) -> Result<RuntimeManifest, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read manifest file at {}: {}", path, e))?;
        Self::load_manifest_str(&content)
    }

    pub fn load_manifest_str(yaml_str: &str) -> Result<RuntimeManifest, String> {
        serde_yaml::from_str::<RuntimeManifest>(yaml_str)
            .map_err(|e| format!("Failed to parse YAML manifest: {}", e))
    }
}
