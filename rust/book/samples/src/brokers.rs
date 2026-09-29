//! Samples of the "Distributing events with Kafka / RabbitMQ" chapter.
//! Each publisher is behind the feature of the same name (`kafka`,
//! `rabbitmq`), like in `edomata-examples`, so that the default build pulls
//! no broker client; `--all-features` (clippy and tests in CI) compiles
//! them. They need a broker, so they are compiled but not run.

#[cfg(any(feature = "kafka", feature = "rabbitmq"))]
use std::sync::Arc;
#[cfg(any(feature = "kafka", feature = "rabbitmq"))]
use std::time::Duration;

#[cfg(any(feature = "kafka", feature = "rabbitmq"))]
use crate::eventsourcing::{Account, Event, Notification, Rejection};
#[cfg(any(feature = "kafka", feature = "rabbitmq"))]
use edomata_backend::eventsourcing::Backend;
#[cfg(any(feature = "kafka", feature = "rabbitmq"))]
use edomata_broker::{CancellationToken, MessageEncoder, OutboxRelay, RelayConfig};

#[cfg(any(feature = "kafka", feature = "rabbitmq"))]
type AccountBackend = Backend<Account, Event, Rejection, Notification>;

/// Relays the outbox to Kafka, as the leader among the replicas.
#[cfg(feature = "kafka")]
pub async fn kafka_relay(
    backend: &AccountBackend,
    pool: edomata_sqlx::PgPool,
    bootstrap: &str,
    cancel: CancellationToken,
) -> Result<(), Box<dyn std::error::Error>> {
    use edomata_broker::postgres::{LeaderLock, listen};
    use edomata_kafka::KafkaPublisher;

    // ANCHOR: kafka
    // Idempotent producer (`enable.idempotence=true`, `acks=all`), a fixed
    // topic (the default is one topic per source), the stream id as the
    // partition key, the ids and metadata as headers.
    let publisher = KafkaPublisher::builder(bootstrap)
        .with_fixed_topic("accounts-notifications")
        .with_config("compression.type", "lz4")
        .build()?;
    let relay = OutboxRelay::new(
        Arc::clone(backend.outbox()),
        Arc::new(publisher),
        MessageEncoder::<Notification>::serde(),
        RelayConfig::new("kafka_accounts").with_poll_interval(Duration::from_secs(10)),
    )
    .wake_on(backend.updates().outbox()) // writes of this process
    .wake_on(listen(pool.clone(), "accounts_outbox")); // writes of other processes
    relay
        .run_as_leader(LeaderLock::new(pool, "kafka_accounts"), cancel)
        .await?;
    // ANCHOR_END: kafka
    Ok(())
}

/// Relays the outbox to a RabbitMQ topic exchange.
#[cfg(feature = "rabbitmq")]
pub async fn rabbitmq_relay(
    backend: &AccountBackend,
    url: &str,
    cancel: CancellationToken,
) -> Result<(), Box<dyn std::error::Error>> {
    use edomata_rabbitmq::RabbitMqPublisher;
    use edomata_rabbitmq::lapin::ExchangeKind;

    // ANCHOR: rabbitmq
    // Publisher confirms, persistent messages, `message_id` set; a durable
    // topic exchange with the stream id as routing key.
    let publisher = RabbitMqPublisher::connect(url)
        .await?
        .with_fixed_exchange("accounts-notifications");
    publisher
        .declare_exchange("accounts-notifications", ExchangeKind::Topic)
        .await?;
    let relay = OutboxRelay::new(
        Arc::clone(backend.outbox()),
        Arc::new(publisher),
        MessageEncoder::<Notification>::serde(),
        RelayConfig::new("rabbitmq_accounts").with_poll_interval(Duration::from_secs(10)),
    )
    .wake_on(backend.updates().outbox());
    relay.run(cancel).await?;
    // ANCHOR_END: rabbitmq
    Ok(())
}
