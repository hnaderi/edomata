//! Retry on version conflicts.

use std::future::Future;
use std::time::Duration;

use rand::Rng;

use crate::BackendError;

/// Retry policy applied by command handlers on
/// [`BackendError::VersionConflict`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetryConfig {
    /// Maximum number of attempts (the first attempt included).
    pub max_retry: u32,
    /// Delay before the second attempt; it doubles after every failure and a
    /// random jitter of up to 500 ms is added.
    pub initial_delay: Duration,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retry: 5,
            initial_delay: Duration::from_secs(2),
        }
    }
}

/// Runs `f` up to `max` times while it fails with
/// [`BackendError::VersionConflict`], waiting `wait` (doubling each time,
/// plus a random jitter of up to 500 ms) between attempts. When the last
/// attempt still conflicts, fails with [`BackendError::MaxRetryExceeded`].
/// Any other error is returned as is.
pub async fn retry<T, F, Fut>(max: u32, wait: Duration, mut f: F) -> Result<T, BackendError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, BackendError>>,
{
    let mut remaining = max;
    let mut wait = wait;
    loop {
        match f().await {
            Err(BackendError::VersionConflict) if remaining > 1 => {
                let jitter = Duration::from_millis(rand::rng().random_range(0..500));
                tokio::time::sleep(wait + jitter).await;
                wait *= 2;
                remaining -= 1;
            }
            Err(BackendError::VersionConflict) => return Err(BackendError::MaxRetryExceeded),
            other => return other,
        }
    }
}

/// [`retry`] with a [`RetryConfig`].
pub async fn retry_with<T, F, Fut>(config: RetryConfig, f: F) -> Result<T, BackendError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, BackendError>>,
{
    retry(config.max_retry, config.initial_delay, f).await
}
