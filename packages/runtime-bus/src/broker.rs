use async_trait::async_trait;
use crate::envelope::MessageEnvelope;
use serde::{de::DeserializeOwned, Serialize};

#[async_trait]
pub trait MessageBroker: Send + Sync {
    async fn publish<T: Serialize + Send + Sync>(&self, topic: &str, envelope: &MessageEnvelope<T>) -> Result<(), String>;
    
    // Abstract request/reply pattern
    async fn request<Req: Serialize + Send + Sync, Res: DeserializeOwned + Send + Sync>(
        &self,
        topic: &str,
        envelope: &MessageEnvelope<Req>,
    ) -> Result<MessageEnvelope<Res>, String>;
}
