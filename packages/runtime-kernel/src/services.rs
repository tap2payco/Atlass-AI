use std::path::Path;
use platform_contracts::manifest::RuntimeManifest;
use crate::config::ConfigLoader;

pub struct ManifestService;

impl ManifestService {
    pub fn new() -> Self {
        Self
    }

    pub fn scan_directories<P: AsRef<Path>>(&self, base_path: P) -> Vec<RuntimeManifest> {
        let mut manifests = Vec::new();
        self.walk_dir(base_path.as_ref(), &mut manifests);
        manifests
    }

    fn walk_dir(&self, dir: &Path, acc: &mut Vec<RuntimeManifest>) {
        if !dir.is_dir() {
            return;
        }

        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        if name.starts_with('.') || name == "target" || name == "node_modules" {
                            continue;
                        }
                    }
                    self.walk_dir(&path, acc);
                } else if path.is_file() {
                    if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                        if file_name == "runtime.yaml" || file_name == "runtime.yml" || file_name.ends_with(".manifest.yaml") {
                            if let Ok(manifest) = ConfigLoader::load_manifest(path.to_str().unwrap_or_default()) {
                                acc.push(manifest);
                            }
                        }
                    }
                }
            }
        }
    }
}

impl Default for ManifestService {
    fn default() -> Self {
        Self::new()
    }
}
