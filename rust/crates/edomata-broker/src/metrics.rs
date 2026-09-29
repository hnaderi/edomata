//! Relay counters.

use std::sync::atomic::{AtomicU64, Ordering};

/// Counters of a relay, also reported as `tracing` events.
///
/// Every relay owns one, shared through
/// [`OutboxRelay::metrics`](crate::OutboxRelay::metrics) /
/// [`JournalRelay::metrics`](crate::JournalRelay::metrics) so that an
/// exporter (Prometheus, OpenTelemetry...) can read it while the relay runs.
/// Counters only grow, except [`MetricsSnapshot::lag`]; reads are relaxed
/// atomic loads, so a snapshot is not a consistent cut across counters.
///
/// ```
/// use edomata_broker::{MetricsSnapshot, RelayMetrics};
///
/// let metrics = RelayMetrics::new();
/// assert_eq!(metrics.snapshot(), MetricsSnapshot::default());
/// assert_eq!(metrics.published(), 0);
/// ```
///
/// Reading a running relay's counters:
///
/// ```no_run
/// # use edomata_broker::OutboxRelay;
/// # fn example(relay: &OutboxRelay<String>) {
/// let metrics = relay.metrics();
/// let snapshot = metrics.snapshot();
/// println!("published={} retried={} failed={} lag={}",
///     snapshot.published, snapshot.retried, snapshot.failed, snapshot.lag);
/// # }
/// ```
#[derive(Debug, Default)]
pub struct RelayMetrics {
    published: AtomicU64,
    retried: AtomicU64,
    failed: AtomicU64,
    lag: AtomicU64,
    passes: AtomicU64,
}

/// A point-in-time copy of [`RelayMetrics`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MetricsSnapshot {
    /// Messages acknowledged by the broker and marked / checkpointed.
    pub published: u64,
    /// Batches retried after a transient publish failure (one per retry).
    pub retried: u64,
    /// Batches given up on: a permanent publish failure or an exhausted
    /// retry budget.
    pub failed: u64,
    /// Items published by the last completed pass: the backlog it found,
    /// set when the pass ends.
    pub lag: u64,
    /// Relay passes completed.
    pub passes: u64,
}

impl RelayMetrics {
    /// Zeroed counters.
    pub fn new() -> Self {
        Self::default()
    }

    /// A copy of the counters.
    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            published: self.published.load(Ordering::Relaxed),
            retried: self.retried.load(Ordering::Relaxed),
            failed: self.failed.load(Ordering::Relaxed),
            lag: self.lag.load(Ordering::Relaxed),
            passes: self.passes.load(Ordering::Relaxed),
        }
    }

    /// Messages published so far.
    pub fn published(&self) -> u64 {
        self.published.load(Ordering::Relaxed)
    }

    pub(crate) fn add_published(&self, n: u64) {
        self.published.fetch_add(n, Ordering::Relaxed);
    }

    pub(crate) fn add_retried(&self) {
        self.retried.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn add_failed(&self) {
        self.failed.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn set_lag(&self, lag: u64) {
        self.lag.store(lag, Ordering::Relaxed);
    }

    pub(crate) fn add_pass(&self) {
        self.passes.fetch_add(1, Ordering::Relaxed);
    }
}
