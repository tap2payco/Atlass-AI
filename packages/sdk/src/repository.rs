use async_trait::async_trait;
use shared::id::RepositoryId;
use shared::types::Result;

#[async_trait]
pub trait RepositoryRuntime: Send + Sync {
    async fn get_metadata(&self, repo_id: RepositoryId) -> Result<String>;
    async fn get_syntax_tree(&self, repo_id: RepositoryId, path: &str) -> Result<String>;
    async fn get_symbols(&self, repo_id: RepositoryId, path: &str) -> Result<String>;
    async fn get_snapshot(&self, repo_id: RepositoryId) -> Result<String>;
}
