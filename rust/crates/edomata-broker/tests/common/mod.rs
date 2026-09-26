//! Shared fixtures: an in-memory event-sourced backend whose commands write
//! one journal event and one outbox notification each.

#![allow(dead_code)]

use std::sync::Arc;
use std::time::Duration;

use edomata_backend::eventsourcing::Backend;
use edomata_backend::inmemory::{InMemoryDriver, InMemoryEventStore};
use edomata_backend::{DomainService, OutboxItem, OutboxReader};
use edomata_broker::{MessageEncoder, RelayConfig, RetryPolicy};
use edomata_core::*;
use futures::TryStreamExt;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Notif {
    pub stream: String,
    pub n: i32,
}

pub struct Counter;

impl DomainModel for Counter {
    type State = i32;
    type Event = i32;
    type Rejection = String;

    fn initial(&self) -> i32 {
        0
    }

    fn transition(&self, e: &i32, s: i32) -> Result<i32, NonEmpty<String>> {
        Ok(s + e)
    }
}

pub type TestBackend = Backend<i32, i32, String, Notif>;

/// Command `n` on stream `s` appends event `n` and publishes `Notif { s, n }`.
pub fn app() -> App<i32, i32, i32, String, Notif, ()> {
    let dsl = Counter.dsl::<i32, Notif>();
    dsl.router(move |n| {
        dsl.aggregate_id()
            .and_then(move |stream| dsl.accept(n).then(dsl.publish([Notif { stream, n }])))
    })
}

pub async fn backend() -> TestBackend {
    Backend::builder(Counter, Counter.dsl::<i32, Notif>())
        .driver(InMemoryDriver::new())
        .build_default()
        .await
        .unwrap()
}

pub async fn backend_with_store(store: Arc<InMemoryEventStore<i32, i32, Notif>>) -> TestBackend {
    Backend::builder(Counter, Counter.dsl::<i32, Notif>())
        .driver(InMemoryDriver::with_event_store(store))
        .build_default()
        .await
        .unwrap()
}

pub fn service(backend: &TestBackend) -> DomainService<i32, String> {
    backend.compile(app())
}

pub fn cmd(stream: &str, n: i32) -> CommandMessage<i32> {
    CommandMessage::new(
        uuid::Uuid::new_v4().to_string(),
        chrono::Utc::now(),
        stream,
        n,
    )
}

/// Writes `n` on each stream in turn, `rounds` times.
pub async fn write(backend: &TestBackend, streams: &[&str], rounds: i32) {
    let service = service(backend);
    for round in 1..=rounds {
        for stream in streams {
            service(cmd(stream, round)).await.unwrap().unwrap();
        }
    }
}

pub async fn pending(reader: &dyn OutboxReader<Notif>) -> Vec<OutboxItem<Notif>> {
    reader.read().try_collect().await.unwrap()
}

pub fn encoder() -> MessageEncoder<Notif> {
    MessageEncoder::serde()
}

/// Fast retries for tests.
pub fn config(source: &str) -> RelayConfig {
    RelayConfig::new(source)
        .with_poll_interval(Duration::from_millis(50))
        .with_leader_retry_interval(Duration::from_millis(50))
        .with_retry(RetryPolicy {
            initial_delay: Duration::from_millis(5),
            max_delay: Duration::from_millis(20),
            max_retries: Some(5),
        })
}
