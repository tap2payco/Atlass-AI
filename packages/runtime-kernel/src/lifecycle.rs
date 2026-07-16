use async_trait::async_trait;

#[async_trait]
pub trait RuntimeLifecycle: Send + Sync {
    async fn startup(&self) -> Result<(), String>;
    async fn shutdown(&self) -> Result<(), String>;
}
