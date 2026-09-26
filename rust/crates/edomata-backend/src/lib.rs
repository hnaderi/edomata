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

#![forbid(unsafe_code)]
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
