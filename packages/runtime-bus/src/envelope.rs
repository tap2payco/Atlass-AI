use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageEnvelope<T> {
    pub message_id: String,
    pub correlation_id: String,
    pub reply_to: Option<String>,
    pub payload: T,
}
