use platform_contracts::retry::RetryPolicy;
use std::time::Duration;

pub struct RetryEngine {
    policy: RetryPolicy,
}

impl RetryEngine {
    pub fn new(policy: RetryPolicy) -> Self {
        Self { policy }
    }

    pub async fn execute<F, Fut, T, E>(&self, _operation: F) -> Result<T, E>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T, E>>,
    {
        // Stub: implement execution loop matching RetryPolicy
        unimplemented!()
    }
}
