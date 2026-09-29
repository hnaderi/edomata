//! # Edomata Kafka
//!
//! A [`Publisher`] for `edomata-broker` relays backed by `rdkafka`:
//!
//! - idempotent producer (`enable.idempotence=true`, `acks=all`): librdkafka's
//!   own retries never duplicate or reorder records (a batch retried by the
//!   relay after a lost acknowledgment is redelivered, as at-least-once
//!   requires);
//! - the stream id is the partition key, which keeps per-stream ordering;
//! - the topic is chosen by a function of the message (default: one topic
//!   per relay source, i.e. per aggregate namespace);
//! - the message id and metadata travel as Kafka headers (see
//!   `edomata_broker::headers`).
//!
//! `publish` returns only after every message of the batch was
//! acknowledged by the brokers; the relay marks outbox items as sent after
//! that, so delivery is at-least-once and consumers deduplicate on the
//! `edomata-id` header.
//!
//! Errors are classified for the relay: oversized or invalid messages,
//! invalid topics and topic authorization failures are
//! [`PublishError::Permanent`]; everything else (broker unavailable,
//! timeouts, queue full...) is [`PublishError::Transient`] and retried.
//!
//! ## Where it fits
//!
//! `edomata-kafka` depends only on `edomata-core` and `edomata-broker`: it
//! is one [`Publisher`] implementation for the broker-agnostic
//! [`OutboxRelay`](edomata_broker::OutboxRelay) and
//! [`JournalRelay`](edomata_broker::JournalRelay). Applications that do not
//! use Kafka don't depend on it and don't build librdkafka. It has no Scala
//! counterpart. The [`rdkafka`] crate is re-exported for its configuration
//! and error types.
//!
//! ## Example
//!
//! An outbox relay publishing to one Kafka topic:
//!
//! ```no_run
//! use std::sync::Arc;
//!
//! use edomata_backend::eventsourcing::Backend;
//! use edomata_broker::postgres::LeaderLock;
//! use edomata_broker::{CancellationToken, MessageEncoder, OutboxRelay, RelayConfig};
//! use edomata_core::*;
//! use edomata_kafka::KafkaPublisher;
//! use edomata_sqlx::SqlxDriver;
//!
//! struct Counter;
//! impl DomainModel for Counter {
//!     type State = i32; type Event = i32; type Rejection = String;
//!     fn initial(&self) -> i32 { 0 }
//!     fn transition(&self, e: &i32, s: i32) -> Result<i32, NonEmpty<String>> { Ok(s + e) }
//! }
//!
//! # async fn example(pool: sqlx::PgPool) -> Result<(), Box<dyn std::error::Error>> {
//! let backend = Backend::builder(Counter, Counter.dsl::<i32, String>())
//!     .driver(SqlxDriver::for_namespace("counters", pool.clone()).await?)
//!     .build_default()
//!     .await?;
//!
//! let publisher = KafkaPublisher::builder("localhost:9092")
//!     .with_fixed_topic("counters-notifications")
//!     .with_config("compression.type", "lz4")
//!     .build()?;
//!
//! let relay = OutboxRelay::new(
//!     Arc::clone(backend.outbox()),
//!     Arc::new(publisher),
//!     MessageEncoder::<String>::serde(),
//!     RelayConfig::new("counters"),
//! )
//! .wake_on(backend.updates().outbox());
//!
//! relay
//!     .run_as_leader(LeaderLock::new(pool, "counters"), CancellationToken::new())
//!     .await?;
//! # Ok(()) }
//! ```
//!
//! ## Feature flags
//!
//! This crate has no Cargo feature flags of its own. librdkafka is built
//! with the `rdkafka` features chosen by the workspace (`tokio`, `libz`);
//! enable further ones (for example `ssl` or `gssapi`) on your own
//! `rdkafka` dependency, which Cargo unifies with this one.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(rustdoc::broken_intra_doc_links, rustdoc::private_intra_doc_links)]
// `doc_auto_cfg` was merged into `doc_cfg` (Rust 1.92), which now shows
// feature-gated items on docs.rs automatically.
#![cfg_attr(docsrs, feature(doc_cfg))]

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use edomata_broker::{BrokerMessage, PublishError, Publisher};
use edomata_core::NonEmpty;
use rdkafka::ClientConfig;
use rdkafka::error::{KafkaError, RDKafkaErrorCode};
use rdkafka::message::{Header, OwnedHeaders};
use rdkafka::producer::{FutureProducer, FutureRecord, Producer};
use rdkafka::util::Timeout;

pub use rdkafka;

type TopicFn = dyn Fn(&BrokerMessage) -> String + Send + Sync;

/// Publishes broker messages to Kafka.
///
/// Each [`BrokerMessage`] becomes one record: the topic comes from the
/// topic function ([`default_topic`] unless set on the builder), the key is
/// the stream id, the payload is the encoded payload and every
/// [`BrokerMessage::headers`] entry is a Kafka header. `publish` enqueues
/// the whole batch, then waits for every delivery report; it fails with the
/// first failed delivery, after which the relay republishes the batch.
///
/// Build it with [`KafkaPublisher::builder`]; creating the producer does not
/// contact the brokers, so a wrong address shows up as transient publish
/// failures.
pub struct KafkaPublisher {
    producer: FutureProducer,
    topic: Arc<TopicFn>,
    send_timeout: Duration,
}

impl std::fmt::Debug for KafkaPublisher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KafkaPublisher")
            .field("send_timeout", &self.send_timeout)
            .finish()
    }
}

/// Builds a [`KafkaPublisher`].
///
/// ```
/// use std::time::Duration;
/// use edomata_broker::{BrokerMessage, MessageKind};
/// use edomata_kafka::KafkaPublisher;
///
/// // Building the producer does not connect: this runs without a broker.
/// let publisher = KafkaPublisher::builder("localhost:9092")
///     .with_topic(|m: &BrokerMessage| format!("{}.{}", m.source, m.kind))
///     .with_send_timeout(Duration::from_secs(10))
///     .with_config("linger.ms", "5")
///     .build()
///     .unwrap();
///
/// # let message = BrokerMessage {
/// #     id: BrokerMessage::outbox_id("accounts", 1), source: "accounts".into(),
/// #     kind: MessageKind::Notification, stream_id: "acc-1".into(), seq_nr: 1,
/// #     time: chrono::DateTime::UNIX_EPOCH, content_type: "application/json".into(),
/// #     payload: b"{}".to_vec(), correlation: None, causation: None,
/// #     extra_headers: Default::default(),
/// # };
/// assert_eq!(publisher.topic_for(&message), "accounts.notification");
/// ```
pub struct KafkaPublisherBuilder {
    config: ClientConfig,
    topic: Arc<TopicFn>,
    send_timeout: Duration,
}

impl std::fmt::Debug for KafkaPublisherBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KafkaPublisherBuilder")
            .field("send_timeout", &self.send_timeout)
            .finish()
    }
}

impl KafkaPublisher {
    /// A builder with the idempotent-producer settings for the given
    /// `bootstrap.servers` (a comma-separated `host:port` list):
    /// `enable.idempotence=true`, `acks=all`, `message.timeout.ms=30000`, the
    /// [`default_topic`] and a 30-second send timeout.
    pub fn builder(bootstrap_servers: &str) -> KafkaPublisherBuilder {
        let mut config = ClientConfig::new();
        config
            .set("bootstrap.servers", bootstrap_servers)
            .set("enable.idempotence", "true")
            .set("acks", "all")
            .set("message.timeout.ms", "30000");
        KafkaPublisherBuilder {
            config,
            topic: Arc::new(default_topic),
            send_timeout: Duration::from_secs(30),
        }
    }

    /// The topic `message` goes to, as chosen by the topic function.
    pub fn topic_for(&self, message: &BrokerMessage) -> String {
        (self.topic)(message)
    }

    /// The underlying producer, for metadata queries or custom sends.
    pub fn producer(&self) -> &FutureProducer {
        &self.producer
    }

    /// Waits for every in-flight message to be delivered, for at most the
    /// send timeout. Call it before shutting down; `publish` already waits
    /// for its own batch.
    ///
    /// # Errors
    ///
    /// The [`KafkaError`] of librdkafka, for example when the timeout
    /// expires with messages still queued.
    pub fn flush(&self) -> Result<(), KafkaError> {
        self.producer.flush(Timeout::After(self.send_timeout))
    }
}

/// The default topic: the relay source with every character outside
/// `[A-Za-z0-9._-]` replaced by `-` (Kafka's topic name alphabet), that is
/// one topic per relay source, see [`sanitize_topic`].
pub fn default_topic(message: &BrokerMessage) -> String {
    sanitize_topic(&message.source)
}

/// Makes a string a valid Kafka topic name: every character outside
/// `[A-Za-z0-9._-]` becomes `-`, and an empty name becomes `"edomata"`.
/// It does not enforce Kafka's 249-character limit.
///
/// ```
/// use edomata_kafka::sanitize_topic;
///
/// assert_eq!(sanitize_topic("accounts"), "accounts");
/// assert_eq!(sanitize_topic("\"billing\".invoices"), "-billing-.invoices");
/// assert_eq!(sanitize_topic(""), "edomata");
/// ```
pub fn sanitize_topic(name: &str) -> String {
    let topic: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '-'
            }
        })
        .collect();
    if topic.is_empty() {
        "edomata".to_string()
    } else {
        topic
    }
}

impl KafkaPublisherBuilder {
    /// Sets any librdkafka configuration property (see librdkafka's
    /// `CONFIGURATION.md`), overriding the defaults. Setting
    /// `enable.idempotence` to `false` or `acks` below `all` weakens the
    /// ordering and durability guarantees described in the crate docs.
    pub fn with_config(mut self, key: &str, value: &str) -> Self {
        self.config.set(key, value);
        self
    }

    /// Chooses the topic per message. The function is called once per
    /// message; keep it deterministic so that a stream always goes to the
    /// same topic (per-stream ordering holds only within one topic).
    pub fn with_topic<F>(mut self, topic: F) -> Self
    where
        F: Fn(&BrokerMessage) -> String + Send + Sync + 'static,
    {
        self.topic = Arc::new(topic);
        self
    }

    /// Sends every message to one topic; streams stay ordered through the
    /// partition key.
    pub fn with_fixed_topic(self, topic: impl Into<String>) -> Self {
        let topic = topic.into();
        self.with_topic(move |_| topic.clone())
    }

    /// How long to wait for a delivery acknowledgment (and for
    /// [`KafkaPublisher::flush`]). A timeout is a transient failure. Keep it
    /// at least as long as `message.timeout.ms`.
    pub fn with_send_timeout(mut self, timeout: Duration) -> Self {
        self.send_timeout = timeout;
        self
    }

    /// Creates the producer.
    ///
    /// # Errors
    ///
    /// The [`KafkaError`] of librdkafka when the configuration is invalid.
    pub fn build(self) -> Result<KafkaPublisher, KafkaError> {
        let producer: FutureProducer = self.config.create()?;
        Ok(KafkaPublisher {
            producer,
            topic: self.topic,
            send_timeout: self.send_timeout,
        })
    }
}

fn headers_of(message: &BrokerMessage) -> OwnedHeaders {
    message
        .headers()
        .iter()
        .fold(OwnedHeaders::new(), |headers, (key, value)| {
            headers.insert(Header {
                key,
                value: Some(value.as_str()),
            })
        })
}

fn classify(error: KafkaError) -> PublishError {
    let permanent = matches!(
        &error,
        KafkaError::MessageProduction(
            RDKafkaErrorCode::MessageSizeTooLarge
                | RDKafkaErrorCode::InvalidMessage
                | RDKafkaErrorCode::InvalidMessageSize
                | RDKafkaErrorCode::InvalidTopic
                | RDKafkaErrorCode::TopicAuthorizationFailed
        )
    );
    if permanent {
        PublishError::Permanent(Box::new(error))
    } else {
        PublishError::Transient(Box::new(error))
    }
}

#[async_trait]
impl Publisher for KafkaPublisher {
    async fn publish(&self, batch: &NonEmpty<BrokerMessage>) -> Result<(), PublishError> {
        // Enqueue every message, then await the acknowledgments in order:
        // the idempotent producer preserves per-partition ordering, and the
        // stream id is the partition key.
        let topics: Vec<String> = batch.iter().map(|m| (self.topic)(m)).collect();
        let mut deliveries = Vec::with_capacity(batch.len());
        for (message, topic) in batch.iter().zip(&topics) {
            let record = FutureRecord::to(topic)
                .key(message.stream_id.as_str())
                .payload(message.payload.as_slice())
                .headers(headers_of(message));
            deliveries.push(
                self.producer
                    .send(record, Timeout::After(self.send_timeout)),
            );
        }
        for delivery in deliveries {
            let delivered = delivery.await.map_err(|(e, _)| classify(e))?;
            tracing::trace!(
                partition = delivered.partition,
                offset = delivered.offset,
                "kafka delivery acknowledged"
            );
        }
        Ok(())
    }
}
