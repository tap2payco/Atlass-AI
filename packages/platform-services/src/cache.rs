use async_trait::async_trait;

#[async_trait]
pub trait Cache<K, V>: Send + Sync {
    async fn get(&self, key: &K) -> Result<Option<V>, String>;
    async fn set(&self, key: &K, value: V, ttl_secs: Option<u64>) -> Result<(), String>;
}
