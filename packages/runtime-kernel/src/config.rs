pub struct ConfigLoader;

impl ConfigLoader {
    pub fn load_manifest(path: &str) -> Result<crate::manifest::RuntimeManifest, String> {
        // In reality, this would read from a yaml file.
        // Returning an error as a stub for now.
        Err(format!("Stub: Cannot load config from {}", path))
    }
}
