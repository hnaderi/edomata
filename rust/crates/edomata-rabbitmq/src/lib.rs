//! # Edomata RabbitMQ
//!
//! A [`Publisher`] for `edomata-broker` relays backed by `lapin`:
//!
//! - publisher confirms are enabled and every message is awaited before
//!   the next one is sent, so `publish` returns only after the broker
//!   acknowledged the whole batch, in order (per-stream ordering);
//! - messages are persistent (`delivery_mode = 2`);
//! - the exchange and the routing key are chosen by functions of the
//!   message (defaults: the relay source as exchange, the stream id as
//!   routing key), so a stream always goes to the same routing key;
//! - `message_id` is the stable message id and the metadata travels as
//!   headers (see `edomata_broker::headers`).
//!
//! The publisher reconnects on the next batch after a connection failure.
//! Soft AMQP errors that a retry cannot fix (`NOT_FOUND` exchange,
//! `ACCESS_REFUSED`, `PRECONDITION_FAILED`) are reported as permanent.
//! Connection and channel failures and broker nacks are transient.
//!
//! ## Where it fits
//!
//! `edomata-rabbitmq` depends only on `edomata-core` and `edomata-broker`:
//! it is one [`Publisher`] implementation for the broker-agnostic
//! [`OutboxRelay`](edomata_broker::OutboxRelay) and
//! [`JournalRelay`](edomata_broker::JournalRelay). Applications that do not
//! use RabbitMQ don't depend on it. It has no Scala counterpart. The
//! [`lapin`] crate is re-exported for its topology and error types.
//!
//! ## Example
//!
//! An outbox relay publishing to a topic exchange, with the stream id as
//! routing key:
//!
//! ```no_run
//! use std::sync::Arc;
//!
//! use edomata_backend::eventsourcing::Backend;
//! use edomata_broker::postgres::LeaderLock;
//! use edomata_broker::{CancellationToken, MessageEncoder, OutboxRelay, RelayConfig};
//! use edomata_core::*;
//! use edomata_rabbitmq::RabbitMqPublisher;
//! use edomata_rabbitmq::lapin::ExchangeKind;
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
//! let publisher = RabbitMqPublisher::connect("amqp://guest:guest@localhost:5672/%2f")
//!     .await?
//!     .with_fixed_exchange("counters");
//! publisher.declare_exchange("counters", ExchangeKind::Topic).await?;
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
//! This crate has no Cargo feature flags.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(rustdoc::broken_intra_doc_links, rustdoc::private_intra_doc_links)]
// `doc_auto_cfg` was merged into `doc_cfg` (Rust 1.92), which now shows
// feature-gated items on docs.rs automatically.
#![cfg_attr(docsrs, feature(doc_cfg))]

use std::sync::Arc;

use async_trait::async_trait;
use edomata_broker::{BrokerMessage, PublishError, Publisher, headers};
use edomata_core::NonEmpty;
use lapin::ErrorKind;
use lapin::options::{BasicPublishOptions, ConfirmSelectOptions, ExchangeDeclareOptions};
use lapin::protocol::{AMQPErrorKind, AMQPSoftError};
use lapin::types::{AMQPValue, FieldTable, ShortString};
use lapin::{
    BasicProperties, Channel, Confirmation, Connection, ConnectionProperties, ExchangeKind,
};
use tokio::sync::Mutex;

pub use lapin;

type NameFn = dyn Fn(&BrokerMessage) -> String + Send + Sync;

/// Publishes broker messages to RabbitMQ.
///
/// Each [`BrokerMessage`] is one persistent `basic.publish` to the exchange
/// and routing key chosen by the message functions ([`default_exchange`]
/// and [`default_routing_key`] unless overridden), with `message_id`,
/// `content_type`, `timestamp` (seconds), `correlation_id` and the other
/// [`BrokerMessage::headers`] as AMQP headers. Messages are sent one by one,
/// each awaiting its publisher confirm, so a batch is confirmed in order.
///
/// The publisher keeps one connection and one confirm-mode channel. After a
/// failure it drops them, and the next `publish` (the relay's retry)
/// reconnects. Exchanges must exist: declare them with
/// [`declare_exchange`](Self::declare_exchange) or provision them
/// beforehand, otherwise publishing fails permanently with `NOT_FOUND`.
/// Messages routed to no queue are dropped by RabbitMQ and still confirmed.
pub struct RabbitMqPublisher {
    uri: String,
    state: Mutex<Option<(Connection, Channel)>>,
    exchange: Arc<NameFn>,
    routing_key: Arc<NameFn>,
}

impl std::fmt::Debug for RabbitMqPublisher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RabbitMqPublisher").finish()
    }
}

/// The default exchange: the relay source ([`BrokerMessage::source`]),
/// that is one exchange per aggregate namespace.
pub fn default_exchange(message: &BrokerMessage) -> String {
    message.source.clone()
}

/// The default routing key: the stream id ([`BrokerMessage::stream_id`]),
/// so that all messages of a stream follow the same route.
pub fn default_routing_key(message: &BrokerMessage) -> String {
    message.stream_id.clone()
}

impl RabbitMqPublisher {
    /// Connects to `uri` (`amqp://user:password@host:port/vhost`) with
    /// publisher confirms enabled and the default exchange / routing key
    /// functions.
    ///
    /// The connection is opened eagerly, so a wrong URI or unreachable
    /// broker fails here rather than on the first batch.
    ///
    /// # Errors
    ///
    /// The [`lapin::Error`] of the connection, the channel creation or
    /// `confirm.select`.
    ///
    /// ```no_run
    /// # use edomata_rabbitmq::RabbitMqPublisher;
    /// # async fn example() -> Result<(), edomata_rabbitmq::lapin::Error> {
    /// let publisher = RabbitMqPublisher::connect("amqp://guest:guest@localhost:5672/%2f")
    ///     .await?
    ///     .with_exchange(|m| format!("{}.{}", m.source, m.kind))
    ///     .with_routing_key(|m| format!("{}.{}", m.kind, m.stream_id));
    /// # let _ = publisher; Ok(()) }
    /// ```
    pub async fn connect(uri: &str) -> Result<Self, lapin::Error> {
        let publisher = Self {
            uri: uri.to_string(),
            state: Mutex::new(None),
            exchange: Arc::new(default_exchange),
            routing_key: Arc::new(default_routing_key),
        };
        publisher.ensure_channel().await?;
        Ok(publisher)
    }

    /// Chooses the exchange per message. Keep it deterministic so that a
    /// stream always goes to the same exchange.
    pub fn with_exchange<F>(mut self, exchange: F) -> Self
    where
        F: Fn(&BrokerMessage) -> String + Send + Sync + 'static,
    {
        self.exchange = Arc::new(exchange);
        self
    }

    /// Chooses the routing key per message. Keep it deterministic so that a
    /// stream always follows the same route.
    pub fn with_routing_key<F>(mut self, routing_key: F) -> Self
    where
        F: Fn(&BrokerMessage) -> String + Send + Sync + 'static,
    {
        self.routing_key = Arc::new(routing_key);
        self
    }

    /// Publishes everything to one exchange.
    pub fn with_fixed_exchange(self, exchange: impl Into<String>) -> Self {
        let exchange = exchange.into();
        self.with_exchange(move |_| exchange.clone())
    }

    /// Declares a durable exchange (convenience for setups without
    /// pre-provisioned topology). Declaring an existing exchange with the
    /// same kind and durability is a no-op.
    ///
    /// # Errors
    ///
    /// The [`lapin::Error`] of the connection or the declaration, for
    /// example `PRECONDITION_FAILED` when the exchange exists with another
    /// kind. A failed declaration closes the channel; the next call reopens
    /// it.
    pub async fn declare_exchange(
        &self,
        name: &str,
        kind: ExchangeKind,
    ) -> Result<(), lapin::Error> {
        let channel = self.channel().await?;
        channel
            .exchange_declare(
                name.into(),
                kind,
                ExchangeDeclareOptions {
                    durable: true,
                    ..ExchangeDeclareOptions::default()
                },
                FieldTable::default(),
            )
            .await
    }

    /// The exchange `message` goes to, as chosen by the exchange function.
    pub fn exchange_for(&self, message: &BrokerMessage) -> String {
        (self.exchange)(message)
    }

    /// The routing key of `message`, as chosen by the routing key function.
    pub fn routing_key_for(&self, message: &BrokerMessage) -> String {
        (self.routing_key)(message)
    }

    async fn ensure_channel(&self) -> Result<Channel, lapin::Error> {
        let mut state = self.state.lock().await;
        if let Some((conn, channel)) = state.as_ref()
            && conn.status().connected()
            && channel.status().connected()
        {
            return Ok(channel.clone());
        }
        let conn = Connection::connect(&self.uri, ConnectionProperties::default()).await?;
        let channel = conn.create_channel().await?;
        channel
            .confirm_select(ConfirmSelectOptions::default())
            .await?;
        let out = channel.clone();
        *state = Some((conn, channel));
        Ok(out)
    }

    /// A connected channel in confirm mode, reconnecting if needed; use it
    /// to declare queues and bindings.
    ///
    /// # Errors
    ///
    /// The [`lapin::Error`] of reconnecting.
    pub async fn channel(&self) -> Result<Channel, lapin::Error> {
        self.ensure_channel().await
    }

    async fn reset(&self) {
        *self.state.lock().await = None;
    }
}

/// Misconfigurations the relay must not retry are permanent; everything
/// else (connection loss, channel closed, broker unavailable) is transient.
fn classify(error: lapin::Error) -> PublishError {
    let permanent = matches!(
        error.kind(),
        ErrorKind::ProtocolError(e)
            if matches!(
                e.kind(),
                AMQPErrorKind::Soft(
                    AMQPSoftError::NOTFOUND
                        | AMQPSoftError::ACCESSREFUSED
                        | AMQPSoftError::PRECONDITIONFAILED
                )
            )
    );
    if permanent {
        PublishError::Permanent(Box::new(error))
    } else {
        PublishError::Transient(Box::new(error))
    }
}

fn properties(message: &BrokerMessage) -> BasicProperties {
    let mut table = FieldTable::default();
    for (key, value) in message.headers() {
        if key == headers::CONTENT_TYPE || key == headers::ID {
            continue;
        }
        table.insert(ShortString::from(key), AMQPValue::LongString(value.into()));
    }
    let mut props = BasicProperties::default()
        .with_message_id(message.id.as_str().into())
        .with_delivery_mode(2)
        .with_content_type(message.content_type.as_str().into())
        .with_timestamp(message.time.timestamp().max(0) as u64)
        .with_headers(table);
    if let Some(correlation) = &message.correlation {
        props = props.with_correlation_id(correlation.as_str().into());
    }
    props
}

#[async_trait]
impl Publisher for RabbitMqPublisher {
    async fn publish(&self, batch: &NonEmpty<BrokerMessage>) -> Result<(), PublishError> {
        let channel = match self.ensure_channel().await {
            Ok(c) => c,
            Err(e) => return Err(PublishError::Transient(Box::new(e))),
        };
        for message in batch.iter() {
            let exchange = (self.exchange)(message);
            let routing_key = (self.routing_key)(message);
            let confirm = channel
                .basic_publish(
                    exchange.as_str().into(),
                    routing_key.as_str().into(),
                    BasicPublishOptions::default(),
                    &message.payload,
                    properties(message),
                )
                .await;
            let confirmation = match confirm {
                Ok(confirm) => confirm.await,
                Err(e) => Err(e),
            };
            match confirmation {
                Ok(Confirmation::Ack(_)) | Ok(Confirmation::NotRequested) => {
                    tracing::trace!(id = %message.id, exchange = %exchange, routing_key = %routing_key, "rabbitmq publish confirmed");
                }
                Ok(Confirmation::Nack(_)) => {
                    return Err(PublishError::transient(format!(
                        "RabbitMQ nacked message {}",
                        message.id
                    )));
                }
                Err(e) => {
                    self.reset().await;
                    return Err(classify(e));
                }
            }
        }
        Ok(())
    }
}
