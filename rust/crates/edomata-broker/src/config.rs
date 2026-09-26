//! Relay configuration.

use std::time::Duration;

/// Exponential backoff for transient publish failures.
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
    /// The delay before retry number `retry` (1-based).
    pub fn delay_for(&self, retry: u32) -> Duration {
        let factor = 2u32.saturating_pow(retry.saturating_sub(1));
        self.initial_delay
            .saturating_mul(factor)
            .min(self.max_delay)
    }

    /// Whether retry number `retry` exceeds the budget.
    pub fn exhausted(&self, retry: u32) -> bool {
        self.max_retries.is_some_and(|max| retry > max)
    }
}

/// Settings shared by the outbox and journal relays.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelayConfig {
    /// Name of the source (typically the aggregate namespace); part of every
    /// message id and the default topic / exchange. Use it as the
    /// `LeaderLock` source too so that replicas of one relay share a lock.
    pub source: String,
    /// Items published (and marked) per batch.
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
