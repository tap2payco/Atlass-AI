/// Per-provider circuit breaker.
///
/// Protects the routing loop from repeatedly calling a provider that has
/// already failed. Each provider gets its own `CircuitBreaker` instance,
/// stored in the `Router`.
///
/// ## State machine
///
/// ```text
///  Closed ──(failure_threshold reached)──► Open
///    ▲                                       │
///    │                                (recovery_window elapsed)
///    │                                       │
///    └──────(probe succeeds)────── HalfOpen ◄┘
///                    │
///            (probe fails)
///                    │
///                    ▼
///                  Open (reset timer)
/// ```
///
/// ## Thread safety
///
/// Uses `std::sync::Mutex` internally. Safe to share via `Arc<CircuitBreaker>`.
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::domain::CircuitState;

// ── Configuration ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Consecutive failures required to open the circuit.
    pub failure_threshold: u32,
    /// How long to wait before attempting a probe in HalfOpen state.
    pub recovery_window: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 3,
            recovery_window: Duration::from_secs(30),
        }
    }
}

// ── Internal state ────────────────────────────────────────────────────────────

#[derive(Debug)]
struct BreakerState {
    state: CircuitState,
    consecutive_failures: u32,
    opened_at: Option<Instant>,
}

impl Default for BreakerState {
    fn default() -> Self {
        Self {
            state: CircuitState::Closed,
            consecutive_failures: 0,
            opened_at: None,
        }
    }
}

// ── CircuitBreaker ────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    state: Mutex<BreakerState>,
}

impl CircuitBreaker {
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: Mutex::new(BreakerState::default()),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(CircuitBreakerConfig::default())
    }

    /// The current state of the circuit.
    pub fn state(&self) -> CircuitState {
        let inner = self.state.lock().unwrap();
        inner.state
    }

    /// Returns `true` if the caller should proceed with the request.
    ///
    /// - `Closed`  → always allowed.
    /// - `Open`    → allowed only if recovery window has elapsed (transitions to HalfOpen).
    /// - `HalfOpen`→ allowed (one probe).
    pub fn should_allow(&self) -> bool {
        let mut inner = self.state.lock().unwrap();
        match inner.state {
            CircuitState::Closed => true,
            CircuitState::HalfOpen => true,
            CircuitState::Open => {
                if let Some(opened_at) = inner.opened_at {
                    if opened_at.elapsed() >= self.config.recovery_window {
                        inner.state = CircuitState::HalfOpen;
                        return true;
                    }
                }
                false
            }
        }
    }

    /// Record a successful call. Resets failure count and closes the circuit.
    pub fn record_success(&self) {
        let mut inner = self.state.lock().unwrap();
        inner.state = CircuitState::Closed;
        inner.consecutive_failures = 0;
        inner.opened_at = None;
    }

    /// Record a failed call. Opens the circuit if the threshold is reached.
    ///
    /// Returns the new circuit state so the caller can emit telemetry.
    pub fn record_failure(&self) -> CircuitState {
        let mut inner = self.state.lock().unwrap();
        inner.consecutive_failures += 1;

        if inner.state == CircuitState::HalfOpen
            || inner.consecutive_failures >= self.config.failure_threshold
        {
            inner.state = CircuitState::Open;
            inner.opened_at = Some(Instant::now());
        }

        inner.state
    }

    /// Consecutive failure count (for telemetry).
    pub fn failure_count(&self) -> u32 {
        self.state.lock().unwrap().consecutive_failures
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_closed() {
        let cb = CircuitBreaker::with_defaults();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.should_allow());
    }

    #[test]
    fn opens_after_threshold() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 3,
            recovery_window: Duration::from_secs(60),
        });
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed); // not yet
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        assert!(!cb.should_allow());
    }

    #[test]
    fn resets_on_success() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 2,
            recovery_window: Duration::from_millis(10),
        });
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        // Wait for recovery window
        std::thread::sleep(Duration::from_millis(15));
        assert!(cb.should_allow()); // transitions to HalfOpen
        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);
    }
}
