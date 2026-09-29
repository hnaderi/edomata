//! The publisher abstraction and an in-memory implementation.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use edomata_core::NonEmpty;

use crate::BrokerMessage;

/// A boxed, thread-safe error: the source of [`PublishError`] and of some
/// [`RelayError`](crate::RelayError) variants.
pub type BoxError = Box<dyn std::error::Error + Send + Sync + 'static>;

/// Why a batch could not be published.
///
/// The distinction drives the relay: transient failures are retried with
/// the configured [`RetryPolicy`](crate::RetryPolicy), permanent ones stop
/// the relay with [`RelayError::Publish`](crate::RelayError::Publish). In
/// both cases nothing of the batch is marked as sent. When unsure, report a
/// failure as transient: a retry at worst redelivers messages, which
/// at-least-once consumers tolerate.
///
/// ```
/// use edomata_broker::PublishError;
///
/// assert!(PublishError::transient("connection reset").is_transient());
/// assert!(!PublishError::permanent("unknown topic").is_transient());
/// ```
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
///
/// A publisher may deliver part of a batch and then fail: the relay
/// republishes the whole batch, so consumers see duplicates, never gaps.
/// Classify failures with [`PublishError`]. Implementations exist for
/// Kafka (`edomata_kafka::KafkaPublisher`), RabbitMQ
/// (`edomata_rabbitmq::RabbitMqPublisher`), and tests
/// ([`RecordingPublisher`]); `Arc<P>` is a publisher when `P` is.
///
/// ```
/// use edomata_broker::{BrokerMessage, PublishError, Publisher};
/// use edomata_core::NonEmpty;
///
/// /// Writes every message to standard output, one line each.
/// struct Stdout;
///
/// #[async_trait::async_trait]
/// impl Publisher for Stdout {
///     async fn publish(&self, batch: &NonEmpty<BrokerMessage>) -> Result<(), PublishError> {
///         for message in batch.iter() {
///             // A real broker call; map its errors to transient / permanent.
///             println!("{} {} {}", message.id, message.stream_id, message.payload_text());
///         }
///         // Return only once every message was acknowledged.
///         Ok(())
///     }
/// }
/// ```
#[async_trait]
pub trait Publisher: Send + Sync {
    /// Publishes a batch, in order, and waits for the broker's
    /// acknowledgment of every message.
    ///
    /// # Errors
    ///
    /// [`PublishError::Transient`] when the batch may succeed later (the
    /// relay retries it), [`PublishError::Permanent`] when it never will
    /// (the relay stops).
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
///
/// ```
/// use std::sync::Arc;
/// use edomata_broker::{
///     BrokerMessage, MessageKind, PublishError, Publisher, RecordingPublisher,
/// };
/// use edomata_core::NonEmpty;
///
/// # fn message(seq_nr: i64) -> BrokerMessage {
/// #     BrokerMessage {
/// #         id: BrokerMessage::outbox_id("s", seq_nr), source: "s".into(),
/// #         kind: MessageKind::Notification, stream_id: "a".into(), seq_nr,
/// #         time: chrono::DateTime::UNIX_EPOCH, content_type: "application/json".into(),
/// #         payload: b"{}".to_vec(), correlation: None, causation: None,
/// #         extra_headers: Default::default(),
/// #     }
/// # }
/// # futures::executor::block_on(async {
/// let publisher = RecordingPublisher::new();
/// let batch = NonEmpty::new(message(1));
///
/// publisher.fail_next(1); // the broker is down once
/// assert!(matches!(publisher.publish(&batch).await, Err(PublishError::Transient(_))));
/// assert!(publisher.messages().is_empty());
///
/// publisher.publish(&batch).await.unwrap();
/// assert_eq!(publisher.ids(), ["s:outbox:1"]);
/// assert_eq!(publisher.calls(), 2);
/// # });
/// ```
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
