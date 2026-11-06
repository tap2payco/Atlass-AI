use std::future::Future;
use std::time::Duration;
use platform_contracts::retry::RetryPolicy;

pub struct RetryRunner;

impl RetryRunner {
    pub async fn execute<F, Fut, T, E>(policy: &RetryPolicy, mut operation: F) -> Result<T, E>
    where
        F: FnMut() -> Fut + Send,
        Fut: Future<Output = Result<T, E>> + Send,
    {
        match policy {
            RetryPolicy::Never => operation().await,
            RetryPolicy::Fixed { attempts, delay } => {
                let mut last_err = None;
                for attempt in 0..*attempts {
                    match operation().await {
                        Ok(val) => return Ok(val),
                        Err(err) => {
                            last_err = Some(err);
                            if attempt + 1 < *attempts {
                                tokio::time::sleep(*delay).await;
                            }
                        }
                    }
                }
                Err(last_err.expect("At least 1 attempt executed"))
            }
            RetryPolicy::Exponential { max_attempts, initial_delay, multiplier } => {
                let mut last_err = None;
                let mut current_delay = *initial_delay;
                for attempt in 0..*max_attempts {
                    match operation().await {
                        Ok(val) => return Ok(val),
                        Err(err) => {
                            last_err = Some(err);
                            if attempt + 1 < *max_attempts {
                                tokio::time::sleep(current_delay).await;
                                current_delay = Duration::from_secs_f32(
                                    current_delay.as_secs_f32() * (*multiplier).max(1.0)
                                );
                            }
                        }
                    }
                }
                Err(last_err.expect("At least 1 attempt executed"))
            }
            RetryPolicy::Fibonacci { max_attempts, initial_delay } => {
                let mut last_err = None;
                let mut a = initial_delay.as_millis() as u64;
                let mut b = a;
                for attempt in 0..*max_attempts {
                    match operation().await {
                        Ok(val) => return Ok(val),
                        Err(err) => {
                            last_err = Some(err);
                            if attempt + 1 < *max_attempts {
                                tokio::time::sleep(Duration::from_millis(a)).await;
                                let next = a + b;
                                a = b;
                                b = next;
                            }
                        }
                    }
                }
                Err(last_err.expect("At least 1 attempt executed"))
            }
            RetryPolicy::Adaptive => {
                // Adaptive default to 3 attempts with 50ms initial delay
                let mut last_err = None;
                let mut delay = Duration::from_millis(50);
                for attempt in 0..3 {
                    match operation().await {
                        Ok(val) => return Ok(val),
                        Err(err) => {
                            last_err = Some(err);
                            if attempt + 1 < 3 {
                                tokio::time::sleep(delay).await;
                                delay *= 2;
                            }
                        }
                    }
                }
                Err(last_err.expect("At least 1 attempt executed"))
            }
        }
    }
}
