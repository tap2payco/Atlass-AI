use shared::types::Result;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct DiscoveryConfig {
    pub root_path: PathBuf,
    pub max_depth: Option<usize>,
    pub respect_gitignore: bool,
}

#[derive(Debug, Clone)]
pub struct DiscoveryResult {
    pub is_git: bool,
    pub primary_language: Option<String>,
    pub files: Vec<PathBuf>,
}

pub struct RepositoryDiscovery {
    config: DiscoveryConfig,
}

impl RepositoryDiscovery {
    pub fn new(config: DiscoveryConfig) -> Self {
        Self { config }
    }

    pub fn discover(&self) -> Result<DiscoveryResult> {
        let is_git = self.detect_git();
        let files = self.enumerate_files()?;
        let primary_language = self.detect_language(&files);

        Ok(DiscoveryResult {
            is_git,
            primary_language,
            files,
        })
    }

    fn detect_git(&self) -> bool {
        self.config.root_path.join(".git").exists()
    }

    fn enumerate_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        
        let mut builder = ignore::WalkBuilder::new(&self.config.root_path);
        builder.hidden(false); // We want to see some hidden files, ignore handles .git
        builder.git_ignore(self.config.respect_gitignore);
        
        if let Some(depth) = self.config.max_depth {
            builder.max_depth(Some(depth));
        }

        for result in builder.build() {
            let entry = match result {
                Ok(e) => e,
                Err(_) => continue, // Log error in real implementation
            };
            
            if entry.file_type().map_or(false, |ft| ft.is_file()) {
                files.push(entry.into_path());
            }
        }

        Ok(files)
    }

    fn detect_language(&self, _files: &[PathBuf]) -> Option<String> {
        // Placeholder for language detection logic (e.g. counting extensions)
        Some("rust".to_string())
    }
}
