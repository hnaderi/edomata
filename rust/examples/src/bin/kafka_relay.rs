//! Distributing outbox notifications to Kafka: an event-sourced account
//! service writes to PostgreSQL, and an `OutboxRelay` publishes the outbox
//! to a Kafka topic through `edomata-kafka`, woken up by the backend's
//! signal (or by `LISTEN/NOTIFY` when the relay runs in another process).
//!
//! ```sh
//! KAFKA_BOOTSTRAP=localhost:9092 cargo run -p edomata-examples --features kafka --bin kafka_relay
//! ```

use std::sync::Arc;
use std::time::Duration;

use edomata_backend::eventsourcing::Backend;
use edomata_broker::postgres::{LeaderLock, listen};
use edomata_broker::{CancellationToken, MessageEncoder, OutboxRelay, RelayConfig};
use edomata_core::*;
use edomata_examples::{command, connect};
use edomata_kafka::KafkaPublisher;
use edomata_sqlx::SqlxDriver;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Event {
    Deposited(i64),
    Withdrawn(i64),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all_fields = "camelCase")]
enum Notification {
    BalanceChanged { account: String, balance: i64 },
}

struct Accounts;

impl DomainModel for Accounts {
    type State = i64;
    type Event = Event;
    type Rejection = String;

    fn initial(&self) -> i64 {
        0
    }

    fn transition(&self, event: &Event, balance: i64) -> Result<i64, NonEmpty<String>> {
        match event {
            Event::Deposited(n) => Ok(balance + n),
            Event::Withdrawn(n) if *n <= balance => Ok(balance - n),
            Event::Withdrawn(_) => Err(NonEmpty::new("insufficient balance".to_string())),
        }
    }
}

fn service() -> App<i64, i64, Event, String, Notification, ()> {
    let dsl = Accounts.dsl::<i64, Notification>();
    dsl.router(move |amount| {
        let event = if amount >= 0 {
            Event::Deposited(amount)
        } else {
            Event::Withdrawn(-amount)
        };
        dsl.state()
            .and_then(move |balance| {
                dsl.decide(Accounts.perform(balance, Decision::accept(event.clone())))
            })
            .and_then(move |balance| {
                dsl.aggregate_id().and_then(move |account| {
                    dsl.publish([Notification::BalanceChanged { account, balance }])
                })
            })
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bootstrap =
        std::env::var("KAFKA_BOOTSTRAP").unwrap_or_else(|_| "localhost:9092".to_string());
    let pool = connect().await?;

    // The writer: outbox rows also raise NOTIFY on `accounts_outbox`, so a
    // relay in another process can listen instead of polling.
    let driver = SqlxDriver::for_namespace("kafka_accounts", pool.clone())
        .await?
        .with_outbox_notify_channel("accounts_outbox");
    let backend = Backend::builder(Accounts, Accounts.dsl::<i64, Notification>())
        .driver(driver)
        .build_default()
        .await?;

    // The relay: idempotent Kafka producer, a fixed topic (the default would
    // be one topic per source), the stream id as partition key, ids and
    // metadata as headers.
    let publisher = KafkaPublisher::builder(&bootstrap)
        .with_fixed_topic("accounts-notifications")
        .build()?;
    let relay = Arc::new(
        OutboxRelay::new(
            Arc::clone(backend.outbox()),
            Arc::new(publisher),
            MessageEncoder::<Notification>::serde(),
            RelayConfig::new("kafka_accounts").with_poll_interval(Duration::from_secs(10)),
        )
        .wake_on(backend.updates().outbox())
        .wake_on(listen(pool.clone(), "accounts_outbox")),
    );
    let cancel = CancellationToken::new();
    let running = tokio::spawn({
        let (relay, cancel, pool) = (Arc::clone(&relay), cancel.clone(), pool.clone());
        async move {
            relay
                .run_as_leader(LeaderLock::new(pool, "kafka_accounts"), cancel)
                .await
        }
    });

    let handle = backend.compile(service());
    let account = format!("acc-{}", uuid::Uuid::new_v4());
    for amount in [100, -30, 25] {
        println!(
            "{account}: {amount:+} → {:?}",
            handle(command(&account, amount)).await?
        );
    }

    tokio::time::sleep(Duration::from_secs(2)).await;
    let m = relay.metrics().snapshot();
    println!(
        "relay: published {} message(s), retried {}, lag {}",
        m.published, m.retried, m.lag
    );
    cancel.cancel();
    running.await??;
    backend.close().await?;
    Ok(())
}
