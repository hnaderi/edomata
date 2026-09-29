//! # Edomata backend
//!
//! Backend abstractions of [Edomata](https://github.com/beyond-scale-group/edomata):
//! everything needed to run [`Edomaton`](edomata_core::Edomaton) and
//! [`Stomaton`](edomata_core::Stomaton) programs against a storage.
//!
//! - [`eventsourcing`]: journal, snapshots, repositories, the command handler
//!   with optimistic concurrency and retry, and the [`Backend`](eventsourcing::Backend)
//!   builder for event-sourced aggregates.
//! - [`cqrs`]: the same for state-only aggregates.
//! - [`inmemory`]: an in-memory [`StorageDriver`](eventsourcing::StorageDriver)
//!   and [`cqrs::StorageDriver`] for tests and prototypes.
//! - Shared pieces: [`Codec`], [`CommandStore`], [`Cache`] / [`LruCache`],
//!   [`OutboxReader`] / [`OutboxConsumer`], [`BackendError`], [`retry`].
//!
//! All traits are runtime-agnostic (`async fn` via `#[async_trait]`, `futures::Stream`);
//! time and synchronisation primitives come from Tokio.
//!
//! ```
//! use edomata_backend::eventsourcing::Backend;
//! use edomata_backend::inmemory::InMemoryDriver;
//! use edomata_backend::OutboxConsumer;
//! use edomata_core::*;
//!
//! struct Account;
//! impl DomainModel for Account {
//!     type State = u64; type Event = u64; type Rejection = String;
//!     fn initial(&self) -> u64 { 0 }
//!     fn transition(&self, e: &u64, s: u64) -> Result<u64, NonEmpty<String>> { Ok(s + e) }
//! }
//!
//! # tokio::runtime::Runtime::new().unwrap().block_on(async {
//! let dsl = Account.dsl::<u64, String>();
//! let backend = Backend::builder(Account, dsl)
//!     .driver(InMemoryDriver::new())
//!     .build_default()
//!     .await?;
//! let deposit = backend.compile(dsl.router(move |amount| {
//!     if amount == 0 {
//!         dsl.reject("empty deposit".to_string())
//!     } else {
//!         dsl.accept(amount).publish([format!("deposited {amount}")])
//!     }
//! }));
//!
//! // `Ok(Ok(()))`: accepted; `Ok(Err(reasons))`: rejected by the domain;
//! // `Err(_)`: the backend itself failed.
//! let now = chrono::Utc::now();
//! assert_eq!(deposit(CommandMessage::new("c1", now, "acc-1", 10)).await?, Ok(()));
//! assert!(deposit(CommandMessage::new("c2", now, "acc-1", 0)).await?.is_err());
//!
//! // Notifications are written to the outbox in the same transaction.
//! let mut published = Vec::new();
//! OutboxConsumer::new()
//!     .consume_once(backend.outbox().as_ref(), &mut |item| {
//!         published.push(item.data);
//!         async { Ok(()) }
//!     })
//!     .await?;
//! assert_eq!(published, ["deposited 10"]);
//! # Ok::<(), edomata_backend::BackendError>(())
//! # }).unwrap();
//! ```
//!
//! ## Where it fits
//!
//! `edomata-backend` depends only on `edomata-core`. Everything that talks
//! to a storage builds on it: `edomata-serde` implements its [`Codec`],
//! `edomata-sqlx` implements its storage drivers on PostgreSQL,
//! `edomata-backend-tests` checks drivers against it, and `edomata-saas-sqlx`,
//! `edomata-simple` and `edomata-broker` use its backends, readers and
//! errors. It is the port of the Scala `backend` module.
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

use chrono::{DateTime, Utc};
use edomata_core::{BoxFuture, CommandMessage, DomainModel, MessageMetadata, ResultNec};
use uuid::Uuid;

mod codec;
mod command_store;
pub mod cqrs;
mod error;
pub mod eventsourcing;
mod header;
pub mod inmemory;
mod lru;
mod outbox;
mod retry;
mod signal;

pub use codec::{Codec, CodecError, PayloadFormat};
pub use command_store::{CommandStore, InMemoryCommandStore};
pub use error::BackendError;
pub use futures::stream::BoxStream;
pub use lru::{Cache, LruCache};
pub use outbox::{DEFAULT_OUTBOX_BATCH_SIZE, OutboxConsumer, OutboxItem, OutboxReader};
pub use retry::{RetryConfig, retry, retry_with};
pub use signal::Signal;

/// Global sequence number of a journal or outbox row.
pub type SeqNr = i64;
/// Version of an aggregate: the number of events applied to it.
pub type EventVersion = i64;
/// Identifier of an aggregate's stream (its address).
pub type StreamId = String;

/// Metadata stored with every journaled event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventMetadata {
    /// Unique event id.
    pub id: Uuid,
    /// When the event was written.
    pub time: DateTime<Utc>,
    /// Global sequence number.
    pub seq_nr: SeqNr,
    /// Version of the aggregate after this event.
    pub version: EventVersion,
    /// Stream (aggregate) the event belongs to.
    pub stream: StreamId,
}

/// A journaled event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventMessage<T> {
    /// Event metadata.
    pub metadata: EventMetadata,
    /// Event payload.
    pub payload: T,
}

impl<T> EventMessage<T> {
    /// Changes the payload.
    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> EventMessage<U> {
        EventMessage {
            metadata: self.metadata,
            payload: f(self.payload),
        }
    }
}

/// A payload-erased view of a [`CommandMessage`]: what storages need to
/// record a handled command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommandRef<'a> {
    /// Command id.
    pub id: &'a str,
    /// When the command was issued.
    pub time: DateTime<Utc>,
    /// Target aggregate.
    pub address: &'a str,
    /// Correlation and causation.
    pub metadata: &'a MessageMetadata,
}

impl<'a, C> From<&'a CommandMessage<C>> for CommandRef<'a> {
    fn from(cmd: &'a CommandMessage<C>) -> Self {
        CommandRef {
            id: &cmd.id,
            time: cmd.time,
            address: &cmd.address,
            metadata: &cmd.metadata,
        }
    }
}

impl CommandRef<'_> {
    /// Copies this view into an owned command message with a unit payload.
    pub fn to_owned_message(&self) -> CommandMessage<()> {
        CommandMessage::with_metadata(self.id, self.time, self.address, (), self.metadata.clone())
    }
}

/// Outcome of handling a command: an error of the backend itself, or the
/// domain outcome (unit or rejection reasons). Mirrors Scala's
/// `F[EitherNec[R, Unit]]`.
pub type CommandResult<R> = Result<ResultNec<(), R>, BackendError>;

/// A compiled domain service: handles command messages of type `C`.
pub type DomainService<C, R> =
    Arc<dyn Fn(CommandMessage<C>) -> BoxFuture<'static, CommandResult<R>> + Send + Sync>;

/// A shared, object-safe domain model.
pub type SharedModel<S, E, R> =
    Arc<dyn DomainModel<State = S, Event = E, Rejection = R> + Send + Sync>;

/// Bounds required of every domain type (state, event, rejection,
/// notification) that flows through a backend.
pub trait Payload: Clone + Send + Sync + 'static {}

impl<T: Clone + Send + Sync + 'static> Payload for T {}
