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

#![forbid(unsafe_code)]

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
    /// `bootstrap.servers`.
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

    /// The topic messages of `message`'s source go to.
    pub fn topic_for(&self, message: &BrokerMessage) -> String {
        (self.topic)(message)
    }

    /// The underlying producer.
    pub fn producer(&self) -> &FutureProducer {
        &self.producer
    }

    /// Waits for every in-flight message to be delivered.
    pub fn flush(&self) -> Result<(), KafkaError> {
        self.producer.flush(Timeout::After(self.send_timeout))
    }
}

/// The default topic: the relay source with every character outside
/// `[A-Za-z0-9._-]` replaced by `-` (Kafka's topic name alphabet).
pub fn default_topic(message: &BrokerMessage) -> String {
    sanitize_topic(&message.source)
}

/// Makes a string a valid Kafka topic name.
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
    /// Sets any librdkafka configuration property.
    pub fn with_config(mut self, key: &str, value: &str) -> Self {
        self.config.set(key, value);
        self
    }

    /// Chooses the topic per message.
    pub fn with_topic<F>(mut self, topic: F) -> Self
    where
        F: Fn(&BrokerMessage) -> String + Send + Sync + 'static,
    {
        self.topic = Arc::new(topic);
        self
    }

    /// Sends every message to one topic.
    pub fn with_fixed_topic(self, topic: impl Into<String>) -> Self {
        let topic = topic.into();
        self.with_topic(move |_| topic.clone())
    }

    /// How long to wait for a delivery acknowledgment.
    pub fn with_send_timeout(mut self, timeout: Duration) -> Self {
        self.send_timeout = timeout;
        self
    }

    /// Creates the producer.
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
