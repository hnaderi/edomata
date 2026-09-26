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

#![forbid(unsafe_code)]

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
