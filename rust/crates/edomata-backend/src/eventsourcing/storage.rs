//! Storage: the set of components a driver provides.

use std::future::Future;
use std::sync::Arc;

use super::{
    JournalReader, NotificationsConsumer, Repository, RepositoryReader, SnapshotPersistence,
    SnapshotStore,
};
use crate::{BackendError, OutboxReader, Payload, SharedModel};

/// Everything a backend needs from a storage.
pub struct Storage<S, E, R, N> {
    /// Write side.
    pub repository: Arc<dyn Repository<S, E, R, N>>,
    /// Read side.
    pub reader: Arc<dyn RepositoryReader<S, E, R>>,
    /// Journal access.
    pub journal: Arc<dyn JournalReader<E>>,
    /// Outbox access.
    pub outbox: Arc<dyn OutboxReader<N>>,
    /// Update signals.
    pub updates: Arc<dyn NotificationsConsumer>,
}

impl<S, E, R, N> Clone for Storage<S, E, R, N> {
    fn clone(&self) -> Self {
        Self {
            repository: Arc::clone(&self.repository),
            reader: Arc::clone(&self.reader),
            journal: Arc::clone(&self.journal),
            outbox: Arc::clone(&self.outbox),
            updates: Arc::clone(&self.updates),
        }
    }
}

/// Builds storages for event-sourced aggregates. Mirrors Scala's
/// `StorageDriver[F, Codec[_]]`.
///
/// `Codec<T>` is the codec a driver needs for payloads of type `T`: `()`
/// for the in-memory driver, a serde-based codec for PostgreSQL drivers.
pub trait StorageDriver: Send + Sync {
    /// Codec required for payloads of type `T`.
    type Codec<T>: Send + Sync + 'static;

    /// Builds a storage for one aggregate type.
    fn build<S, E, R, N>(
        &self,
        model: SharedModel<S, E, R>,
        snapshot: Arc<dyn SnapshotStore<S>>,
        event_codec: Self::Codec<E>,
        notification_codec: Self::Codec<N>,
    ) -> impl Future<Output = Result<Storage<S, E, R, N>, BackendError>> + Send
    where
        S: Payload,
        E: Payload,
        R: Payload,
        N: Payload;

    /// Builds the durable snapshot storage for states of type `S`.
    fn snapshot<S>(
        &self,
        state_codec: Self::Codec<S>,
    ) -> impl Future<Output = Result<Arc<dyn SnapshotPersistence<S>>, BackendError>> + Send
    where
        S: Payload;
}
