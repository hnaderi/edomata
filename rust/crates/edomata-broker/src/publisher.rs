//! The publisher abstraction and an in-memory implementation.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use edomata_core::NonEmpty;

use crate::BrokerMessage;

/// A boxed error.
pub type BoxError = Box<dyn std::error::Error + Send + Sync + 'static>;

/// Why a batch could not be published.
#[derive(Debug, thiserror::Error)]
pub enum PublishError {
    /// The broker is unavailable or the batch was not acknowledged; the
    /// relay retries with exponential backoff.
    #[error("transient publish failure: {0}")]
    Transient(#[source] BoxError),
    /// The batch can never be published (misconfiguration, rejected
    /// message); the relay stops and reports it.
    #[error("permanent publish failure: {0}")]
    Permanent(#[source] BoxError),
}

impl PublishError {
    /// A transient failure from a message.
    pub fn transient(message: impl Into<String>) -> Self {
        PublishError::Transient(message.into().into())
    }

    /// A permanent failure from a message.
    pub fn permanent(message: impl Into<String>) -> Self {
        PublishError::Permanent(message.into().into())
    }

    /// Whether the relay should retry.
    pub fn is_transient(&self) -> bool {
        matches!(self, PublishError::Transient(_))
    }
}

/// Publishes batches of messages to a broker.
///
/// A publisher must return `Ok` only once the broker acknowledged **every**
/// message of the batch: the relays mark outbox items as sent (or advance
/// the journal checkpoint) only after `publish` succeeded, which is what
/// makes delivery at-least-once. Messages of a batch are given in sequence
/// order and must be published in that order per stream.
#[async_trait]
pub trait Publisher: Send + Sync {
    /// Publishes a batch and waits for the broker's acknowledgment.
    async fn publish(&self, batch: &NonEmpty<BrokerMessage>) -> Result<(), PublishError>;
}

#[async_trait]
impl<P: Publisher + ?Sized> Publisher for Arc<P> {
    async fn publish(&self, batch: &NonEmpty<BrokerMessage>) -> Result<(), PublishError> {
        (**self).publish(batch).await
    }
}

/// An in-memory publisher that records every message it receives, with
/// programmable failures. Meant for tests and examples.
#[derive(Debug, Default)]
pub struct RecordingPublisher {
    messages: Mutex<Vec<BrokerMessage>>,
    fail_before: AtomicUsize,
    fail_after: AtomicUsize,
    calls: AtomicUsize,
}

impl RecordingPublisher {
    /// An empty publisher.
    pub fn new() -> Self {
        Self::default()
    }

    /// The next `n` calls fail transiently **before** recording anything
    /// (the broker is down).
    pub fn fail_next(&self, n: usize) {
        self.fail_before.store(n, Ordering::SeqCst);
    }

    /// The next `n` calls record the batch and **then** fail transiently:
    /// the broker received the messages but the relay never learnt it (a
    /// crash between publishing and marking).
    pub fn fail_after_publishing_next(&self, n: usize) {
        self.fail_after.store(n, Ordering::SeqCst);
    }

    /// Every message received so far, in order.
    pub fn messages(&self) -> Vec<BrokerMessage> {
        self.messages.lock().unwrap().clone()
    }

    /// The ids of every message received so far, in order.
    pub fn ids(&self) -> Vec<String> {
        self.messages
            .lock()
            .unwrap()
            .iter()
            .map(|m| m.id.clone())
            .collect()
    }

    /// Number of `publish` calls so far (failed ones included).
    pub fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    /// Forgets every recorded message.
    pub fn clear(&self) {
        self.messages.lock().unwrap().clear();
    }
}

#[async_trait]
impl Publisher for RecordingPublisher {
    async fn publish(&self, batch: &NonEmpty<BrokerMessage>) -> Result<(), PublishError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if take_one(&self.fail_before) {
            return Err(PublishError::transient("broker unavailable"));
        }
        self.messages.lock().unwrap().extend(batch.iter().cloned());
        if take_one(&self.fail_after) {
            return Err(PublishError::transient("acknowledgment lost"));
        }
        Ok(())
    }
}

fn take_one(counter: &AtomicUsize) -> bool {
    counter
        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
        .is_ok()
}
