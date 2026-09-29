//! # Edomata Broker
//!
//! Optional distribution of Edomata's outbox notifications and journal
//! events to a message broker, independent of the broker itself. Concrete
//! publishers live in `edomata-kafka` (rdkafka) and `edomata-rabbitmq`
//! (lapin); an application that does not depend on those crates pulls no
//! broker client into its dependency tree.
//!
//! **The transactional outbox is the only source.** Commands write events
//! and outbox items to PostgreSQL in one transaction, as always; a relay
//! then publishes them:
//!
//! - [`Publisher`]: `publish(batch)` must return only once the broker
//!   acknowledged every [`BrokerMessage`] of the batch.
//! - [`OutboxRelay`]: reads unpublished outbox items, publishes them in
//!   sequence order and **only then** marks them as sent.
//! - [`JournalRelay`]: tails the journal from a checkpoint
//!   ([`CheckpointStore`], [`postgres::PgCheckpointStore`]) and advances it
//!   after the broker acknowledged.
//! - wake-ups: the backend's in-process signals, [`postgres::listen`]
//!   (`LISTEN/NOTIFY`, so a relay can run in another process) and a polling
//!   fallback; shutdown through a `CancellationToken`.
//! - [`postgres::LeaderLock`]: an advisory lock so that only one relay per
//!   source publishes when several replicas run.
//!
//! Delivery is **at-least-once**: every message has a stable id
//! ([`BrokerMessage::outbox_id`] / [`BrokerMessage::journal_id`]) for
//! consumers to deduplicate. Per-stream ordering is preserved: messages are
//! published in sequence order and keyed / routed by stream id. Transient
//! publish failures are retried with exponential backoff
//! ([`RetryPolicy`]); counters are exposed through [`RelayMetrics`] and
//! `tracing` events.
//!
//! ## Where it fits
//!
//! `edomata-broker` sits on top of the storage layer: it reads the
//! [`OutboxReader`](edomata_backend::OutboxReader) and
//! [`JournalReader`](edomata_backend::eventsourcing::JournalReader) of an
//! `edomata-backend` [`Backend`](edomata_backend::eventsourcing::Backend)
//! (in-memory, or PostgreSQL through `edomata-sqlx`), and its [`postgres`]
//! module uses the `edomata-postgres` naming for the checkpoint table. It
//! has no Scala counterpart. `edomata-kafka` and `edomata-rabbitmq`
//! implement [`Publisher`] on top of it.
//!
//! ```text
//! edomata-core ── edomata-backend ─┬─ edomata-sqlx (writers, NOTIFY)
//!                                  └─ edomata-broker ─┬─ edomata-kafka
//!                                                     └─ edomata-rabbitmq
//! ```
//!
//! ## Example
//!
//! A relay from an in-memory backend's outbox to a custom [`Publisher`]
//! that collects what it receives:
//!
//! ```
//! use std::sync::{Arc, Mutex};
//!
//! use edomata_backend::eventsourcing::Backend;
//! use edomata_backend::inmemory::InMemoryDriver;
//! use edomata_broker::{
//!     BrokerMessage, MessageEncoder, OutboxRelay, PublishError, Publisher, RelayConfig,
//! };
//! use edomata_core::*;
//!
//! struct Counter;
//! impl DomainModel for Counter {
//!     type State = i32; type Event = i32; type Rejection = String;
//!     fn initial(&self) -> i32 { 0 }
//!     fn transition(&self, e: &i32, s: i32) -> Result<i32, NonEmpty<String>> { Ok(s + e) }
//! }
//!
//! /// Collects every message. A real publisher sends the batch to a broker
//! /// and returns only after the broker acknowledged all of it.
//! #[derive(Default)]
//! struct Collect(Mutex<Vec<BrokerMessage>>);
//!
//! #[async_trait::async_trait]
//! impl Publisher for Collect {
//!     async fn publish(&self, batch: &NonEmpty<BrokerMessage>) -> Result<(), PublishError> {
//!         self.0.lock().unwrap().extend(batch.iter().cloned());
//!         Ok(())
//!     }
//! }
//!
//! # #[tokio::main(flavor = "current_thread")]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let dsl = Counter.dsl::<i32, String>();
//! let backend = Backend::builder(Counter, dsl)
//!     .driver(InMemoryDriver::new())
//!     .build_default()
//!     .await?;
//! let service = backend.compile(dsl.router(move |n: i32| dsl.accept(n).publish([format!("+{n}")])));
//! let cmd = CommandMessage::new("cmd-1", chrono::Utc::now(), "counter-1", 5);
//! assert!(service(cmd).await?.is_ok());
//!
//! let publisher = Arc::new(Collect::default());
//! let relay = OutboxRelay::new(
//!     Arc::clone(backend.outbox()),
//!     Arc::clone(&publisher) as Arc<dyn Publisher>,
//!     MessageEncoder::serde(),
//!     RelayConfig::new("counters"),
//! );
//!
//! // One pass publishes every pending item, then marks it as sent.
//! assert_eq!(relay.relay_once().await?, 1);
//! let sent = publisher.0.lock().unwrap().clone();
//! assert_eq!(sent[0].id, BrokerMessage::outbox_id("counters", sent[0].seq_nr));
//! assert_eq!(sent[0].stream_id, "counter-1");
//! assert_eq!(sent[0].payload_text(), r#""+5""#);
//! // The item was marked as sent once the publish succeeded.
//! assert_eq!(relay.relay_once().await?, 0);
//! # Ok(()) }
//! ```
//!
//! In production, the relay reads the PostgreSQL outbox, is woken up by
//! `LISTEN/NOTIFY`, and competes with its replicas for a leader lock:
//!
//! ```no_run
//! # use std::sync::Arc;
//! # use edomata_backend::eventsourcing::Backend;
//! # use edomata_broker::postgres::{LeaderLock, listen};
//! # use edomata_broker::{CancellationToken, MessageEncoder, OutboxRelay, RecordingPublisher, RelayConfig};
//! # use edomata_core::*;
//! # use edomata_sqlx::SqlxDriver;
//! # struct Counter;
//! # impl DomainModel for Counter {
//! #     type State = i32; type Event = i32; type Rejection = String;
//! #     fn initial(&self) -> i32 { 0 }
//! #     fn transition(&self, e: &i32, s: i32) -> Result<i32, NonEmpty<String>> { Ok(s + e) }
//! # }
//! # async fn example(pool: sqlx::PgPool) -> Result<(), Box<dyn std::error::Error>> {
//! // Writers raise `NOTIFY counters_outbox` whenever they append to the outbox.
//! let driver = SqlxDriver::for_namespace("counters", pool.clone())
//!     .await?
//!     .with_outbox_notify_channel("counters_outbox");
//! let backend = Backend::builder(Counter, Counter.dsl::<i32, String>())
//!     .driver(driver)
//!     .build_default()
//!     .await?;
//!
//! let publisher = Arc::new(RecordingPublisher::new()); // a KafkaPublisher, a RabbitMqPublisher...
//! let relay = OutboxRelay::new(
//!     Arc::clone(backend.outbox()),
//!     publisher,
//!     MessageEncoder::<String>::serde(),
//!     RelayConfig::new("counters"),
//! )
//! .wake_on(backend.updates().outbox())
//! .wake_on(listen(pool.clone(), "counters_outbox"));
//!
//! // Returns when `cancel` fires, or with the error that stopped the relay.
//! let cancel = CancellationToken::new();
//! relay.run_as_leader(LeaderLock::new(pool, "counters"), cancel).await?;
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

mod config;
mod encoder;
mod error;
mod journal_relay;
mod message;
mod metrics;
mod outbox_relay;
pub mod postgres;
mod publisher;
mod runner;

pub use config::{RelayConfig, RetryPolicy};
pub use encoder::{APPLICATION_JSON, APPLICATION_OCTET_STREAM, MessageEncoder};
pub use error::RelayError;
pub use journal_relay::{CheckpointStore, InMemoryCheckpointStore, JournalRelay};
pub use message::{BrokerMessage, MessageKind, headers};
pub use metrics::{MetricsSnapshot, RelayMetrics};
pub use outbox_relay::OutboxRelay;
pub use publisher::{BoxError, PublishError, Publisher, RecordingPublisher};

pub use tokio_util::sync::CancellationToken;
