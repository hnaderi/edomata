//! Relay configuration.

use std::time::Duration;

/// Exponential backoff for transient publish failures.
///
/// When [`Publisher::publish`](crate::Publisher::publish) fails with
/// [`PublishError::Transient`](crate::PublishError::Transient), the relay
/// waits [`delay_for`](Self::delay_for)`(retry)` and publishes the same
/// batch again: the delay starts at `initial_delay`, doubles after every
/// failure and is capped at `max_delay`. Once the budget is
/// [`exhausted`](Self::exhausted), the relay stops with
/// [`RelayError::Publish`](crate::RelayError::Publish). Nothing is marked as
/// sent while a batch is being retried.
///
/// ```
/// use std::time::Duration;
/// use edomata_broker::RetryPolicy;
///
/// let policy = RetryPolicy {
///     initial_delay: Duration::from_millis(100),
///     max_delay: Duration::from_secs(1),
///     max_retries: Some(5),
/// };
/// assert_eq!(policy.delay_for(1), Duration::from_millis(100));
/// assert_eq!(policy.delay_for(2), Duration::from_millis(200));
/// assert_eq!(policy.delay_for(5), Duration::from_secs(1)); // capped
/// assert!(!policy.exhausted(5));
/// assert!(policy.exhausted(6));
///
/// // The default retries forever, from 200 ms up to 30 s.
/// assert_eq!(RetryPolicy::default().max_retries, None);
/// assert!(!RetryPolicy::default().exhausted(u32::MAX));
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetryPolicy {
    /// Delay before the first retry; it doubles after every failure.
    pub initial_delay: Duration,
    /// Upper bound of the delay.
    pub max_delay: Duration,
    /// Number of retries before giving up (`None`: retry forever).
    pub max_retries: Option<u32>,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            initial_delay: Duration::from_millis(200),
            max_delay: Duration::from_secs(30),
            max_retries: None,
        }
    }
}

impl RetryPolicy {
    /// The delay before retry number `retry` (1-based):
    /// `initial_delay * 2^(retry - 1)`, capped at `max_delay`. The
    /// computation saturates instead of overflowing; `0` is treated as `1`.
    pub fn delay_for(&self, retry: u32) -> Duration {
        let factor = 2u32.saturating_pow(retry.saturating_sub(1));
        self.initial_delay
            .saturating_mul(factor)
            .min(self.max_delay)
    }

    /// Whether retry number `retry` (1-based) exceeds the budget, that is
    /// `retry > max_retries`; always `false` when `max_retries` is `None`.
    pub fn exhausted(&self, retry: u32) -> bool {
        self.max_retries.is_some_and(|max| retry > max)
    }
}

/// Settings shared by the outbox and journal relays.
///
/// ```
/// use std::time::Duration;
/// use edomata_broker::{RelayConfig, RetryPolicy};
///
/// let config = RelayConfig::new("accounts")
///     .with_batch_size(500)
///     .with_poll_interval(Duration::from_secs(30))
///     .with_retry(RetryPolicy { max_retries: Some(10), ..RetryPolicy::default() });
/// assert_eq!(config.source, "accounts");
/// assert_eq!(config.batch_size, 500);
///
/// // A batch size of 0 would never make progress: it is raised to 1.
/// assert_eq!(RelayConfig::new("accounts").with_batch_size(0).batch_size, 1);
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelayConfig {
    /// Name of the source (typically the aggregate namespace); part of every
    /// message id and the default topic / exchange. Use it as the
    /// [`LeaderLock`](crate::postgres::LeaderLock) source too so that replicas of one relay share a lock.
    pub source: String,
    /// Items published (and marked) per batch; at least 1 when set through
    /// [`with_batch_size`](Self::with_batch_size). Each batch is one
    /// [`Publisher::publish`](crate::Publisher::publish) call.
    pub batch_size: usize,
    /// Polling fallback: the relay re-reads the source at least this often
    /// even without a wake-up signal.
    pub poll_interval: Duration,
    /// Backoff on transient publish failures.
    pub retry: RetryPolicy,
    /// How often a stand-by relay retries to become the leader.
    pub leader_retry_interval: Duration,
}

impl RelayConfig {
    /// Defaults: batches of 100, polling every 5 seconds, unlimited retries
    /// from 200 ms up to 30 s, leader election retried every second.
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            batch_size: 100,
            poll_interval: Duration::from_secs(5),
            retry: RetryPolicy::default(),
            leader_retry_interval: Duration::from_secs(1),
        }
    }

    /// Sets the batch size (at least 1).
    pub fn with_batch_size(mut self, batch_size: usize) -> Self {
        self.batch_size = batch_size.max(1);
        self
    }

    /// Sets the polling fallback interval.
    pub fn with_poll_interval(mut self, interval: Duration) -> Self {
        self.poll_interval = interval;
        self
    }

    /// Sets the retry policy.
    pub fn with_retry(mut self, retry: RetryPolicy) -> Self {
        self.retry = retry;
        self
    }

    /// Sets the leader election retry interval.
    pub fn with_leader_retry_interval(mut self, interval: Duration) -> Self {
        self.leader_retry_interval = interval;
        self
    }
}
