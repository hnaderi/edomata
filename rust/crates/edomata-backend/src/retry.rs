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
///
/// Command handlers use it with their [`RetryConfig`]; call it directly to
/// retry your own read-decide-write loops. Waiting uses `tokio::time`, so
/// it needs a Tokio runtime with the time driver enabled.
///
/// # Errors
///
/// [`BackendError::MaxRetryExceeded`] after `max` conflicting attempts, or
/// the first error that is not a [`BackendError::VersionConflict`].
///
/// ```
/// use std::time::Duration;
/// use edomata_backend::{retry, BackendError};
///
/// # tokio::runtime::Builder::new_current_thread().enable_time().start_paused(true).build().unwrap().block_on(async {
/// // Conflicts twice, then succeeds on the third attempt.
/// let mut attempts = 0;
/// let result = retry(3, Duration::from_millis(10), || {
///     attempts += 1;
///     let outcome = if attempts < 3 { Err(BackendError::VersionConflict) } else { Ok(attempts) };
///     async move { outcome }
/// })
/// .await;
/// assert_eq!(result, Ok(3));
///
/// // Always conflicting: gives up after `max` attempts.
/// let result: Result<(), _> = retry(2, Duration::ZERO, || async { Err(BackendError::VersionConflict) }).await;
/// assert_eq!(result, Err(BackendError::MaxRetryExceeded));
/// # });
/// ```
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
///
/// # Errors
///
/// As [`retry`].
pub async fn retry_with<T, F, Fut>(config: RetryConfig, f: F) -> Result<T, BackendError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, BackendError>>,
{
    retry(config.max_retry, config.initial_delay, f).await
}
