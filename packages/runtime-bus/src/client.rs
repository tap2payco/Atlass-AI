use async_trait::async_trait;
use platform_contracts::events::RuntimeEvent;

#[async_trait]
pub trait EventPublisher: Send + Sync {
    async fn publish_event(&self, event: RuntimeEvent) -> Result<(), String>;
}
