//! CQRS storage: the set of components a driver provides.

use std::future::Future;
use std::sync::Arc;

use super::{NotificationsConsumer, Repository, SharedStateModel};
use crate::{BackendError, OutboxReader, Payload};

/// Everything a CQRS backend needs from a storage.
pub struct Storage<S, N> {
    /// Read and write side.
    pub repository: Arc<dyn Repository<S, N>>,
    /// Outbox access.
    pub outbox: Arc<dyn OutboxReader<N>>,
    /// Update signals.
    pub updates: Arc<dyn NotificationsConsumer>,
}

impl<S, N> Clone for Storage<S, N> {
    fn clone(&self) -> Self {
        Self {
            repository: Arc::clone(&self.repository),
            outbox: Arc::clone(&self.outbox),
            updates: Arc::clone(&self.updates),
        }
    }
}

/// Builds storages for CQRS aggregates. Mirrors Scala's
/// `StorageDriver[F, Codec[_], Handler[_]]`.
///
/// `Codec<T>` is the codec a driver needs for payloads of type `T` and
/// `Handler<N>` is the driver-specific hook that runs inside the save
/// transaction for every batch of notifications (used to maintain
/// projections atomically).
pub trait StorageDriver: Send + Sync {
    /// Codec required for payloads of type `T`.
    type Codec<T>: Send + Sync + 'static;
    /// Transactional notification handler for notifications of type `N`.
    type Handler<N: 'static>: Send + Sync + 'static;

    /// Builds a storage for one aggregate type, with an optional
    /// transactional notification handler.
    fn build<S, N>(
        &self,
        model: SharedStateModel<S>,
        state_codec: Self::Codec<S>,
        notification_codec: Self::Codec<N>,
        handler: Option<Self::Handler<N>>,
    ) -> impl Future<Output = Result<Storage<S, N>, BackendError>> + Send
    where
        S: Payload,
        N: Payload;
}
