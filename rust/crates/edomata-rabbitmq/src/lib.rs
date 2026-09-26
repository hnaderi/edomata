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

#![forbid(unsafe_code)]

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

/// The default exchange: the relay source.
pub fn default_exchange(message: &BrokerMessage) -> String {
    message.source.clone()
}

/// The default routing key: the stream id.
pub fn default_routing_key(message: &BrokerMessage) -> String {
    message.stream_id.clone()
}

impl RabbitMqPublisher {
    /// Connects to `uri` (`amqp://user:password@host:port/vhost`) with
    /// publisher confirms enabled and the default exchange / routing key
    /// functions.
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

    /// Chooses the exchange per message.
    pub fn with_exchange<F>(mut self, exchange: F) -> Self
    where
        F: Fn(&BrokerMessage) -> String + Send + Sync + 'static,
    {
        self.exchange = Arc::new(exchange);
        self
    }

    /// Chooses the routing key per message.
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
    /// pre-provisioned topology).
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

    /// The exchange messages of `message` go to.
    pub fn exchange_for(&self, message: &BrokerMessage) -> String {
        (self.exchange)(message)
    }

    /// The routing key of `message`.
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

    /// A connected channel (reconnecting if needed).
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
