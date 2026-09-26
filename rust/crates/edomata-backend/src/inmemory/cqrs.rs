//! In-memory CQRS storage.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::Utc;
use edomata_core::NonEmpty;
use futures::stream::BoxStream;

use super::{InMemoryDriver, Tables, lock};
use crate::cqrs::{
    AggregateState, CommandState, Notifications, NotificationsPublisher, Repository,
    RepositoryReader, SharedStateModel, Storage, StorageDriver,
};
use crate::{BackendError, CommandRef, OutboxItem, OutboxReader, Payload, SeqNr, StreamId};

/// Notification handler of the in-memory CQRS driver: runs synchronously
/// inside `save`, after the state and the notifications were written.
pub type InMemoryNotificationHandler<N> =
    Arc<dyn Fn(&NonEmpty<N>) -> Result<(), BackendError> + Send + Sync>;

/// In-memory states, outbox and commands for one aggregate type.
///
/// Create one with [`InMemoryStateStore::new`], optionally seed it, and pass
/// it to [`InMemoryDriver::with_state_store`].
pub struct InMemoryStateStore<S, N> {
    inner: Mutex<Inner<S, N>>,
    updates: Notifications,
}

struct Inner<S, N> {
    states: HashMap<StreamId, AggregateState<S>>,
    tables: Tables<N>,
}

impl<S, N> Default for InMemoryStateStore<S, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S, N> InMemoryStateStore<S, N> {
    /// An empty store.
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                states: HashMap::new(),
                tables: Tables::default(),
            }),
            updates: Notifications::new(),
        }
    }

    /// Adds states as they are.
    pub fn seed_states(&self, states: impl IntoIterator<Item = (StreamId, AggregateState<S>)>) {
        lock(&self.inner).states.extend(states);
    }

    /// Adds unpublished outbox rows as they are.
    pub fn seed_outbox(&self, items: impl IntoIterator<Item = OutboxItem<N>>)
    where
        N: Clone,
    {
        lock(&self.inner).tables.seed_outbox(items);
    }

    /// Records command ids as already handled.
    pub fn seed_commands<'a>(&self, ids: impl IntoIterator<Item = &'a str>)
    where
        N: Clone,
    {
        let mut inner = lock(&self.inner);
        for id in ids {
            inner.tables.insert_command(id, "", Utc::now());
        }
    }

    /// The update signals of this store.
    pub fn updates(&self) -> &Notifications {
        &self.updates
    }
}

impl StorageDriver for InMemoryDriver {
    type Codec<T> = ();
    type Handler<N: 'static> = InMemoryNotificationHandler<N>;

    async fn build<S, N>(
        &self,
        model: SharedStateModel<S>,
        _state_codec: (),
        _notification_codec: (),
        handler: Option<InMemoryNotificationHandler<N>>,
    ) -> Result<Storage<S, N>, BackendError>
    where
        S: Payload,
        N: Payload,
    {
        let store: Arc<InMemoryStateStore<S, N>> = match &self.store {
            None => Arc::new(InMemoryStateStore::new()),
            Some(_) => self.store_of()?,
        };
        let repository: Arc<dyn Repository<S, N>> = Arc::new(InMemoryCqrsRepository {
            store: Arc::clone(&store),
            model,
            handler,
        });
        let outbox: Arc<dyn OutboxReader<N>> = Arc::new(InMemoryOutboxReader(Arc::clone(&store)));
        let updates = Arc::new(store.updates.clone());
        Ok(Storage {
            repository,
            outbox,
            updates,
        })
    }
}

struct InMemoryCqrsRepository<S, N> {
    store: Arc<InMemoryStateStore<S, N>>,
    model: SharedStateModel<S>,
    handler: Option<InMemoryNotificationHandler<N>>,
}

impl<S: Payload, N: Payload> InMemoryCqrsRepository<S, N> {
    fn state_of(&self, inner: &Inner<S, N>, id: &str) -> AggregateState<S> {
        inner
            .states
            .get(id)
            .cloned()
            .unwrap_or_else(|| AggregateState::new(self.model.initial(), 0))
    }
}

#[async_trait]
impl<S: Payload, N: Payload> RepositoryReader<S> for InMemoryCqrsRepository<S, N> {
    async fn get(&self, id: &str) -> Result<AggregateState<S>, BackendError> {
        let inner = lock(&self.store.inner);
        Ok(self.state_of(&inner, id))
    }
}

#[async_trait]
impl<S: Payload, N: Payload> Repository<S, N> for InMemoryCqrsRepository<S, N> {
    async fn load(&self, cmd: CommandRef<'_>) -> Result<CommandState<S>, BackendError> {
        let inner = lock(&self.store.inner);
        if inner.tables.commands.contains_key(cmd.id) {
            return Ok(CommandState::Redundant);
        }
        Ok(CommandState::Aggregate(self.state_of(&inner, cmd.address)))
    }

    async fn save(
        &self,
        cmd: CommandRef<'_>,
        version: SeqNr,
        new_state: S,
        notifications: Vec<N>,
    ) -> Result<(), BackendError> {
        {
            let mut inner = lock(&self.store.inner);
            // Mirrors the PostgreSQL upsert: a new row is inserted at version
            // 1; an existing row is only updated when its version matches.
            match inner.states.get_mut(cmd.address) {
                None => {
                    inner
                        .states
                        .insert(cmd.address.to_owned(), AggregateState::new(new_state, 1));
                }
                Some(existing) if existing.version == version => {
                    existing.state = new_state;
                    existing.version += 1;
                }
                Some(_) => return Err(BackendError::VersionConflict),
            }
            if inner.tables.commands.contains_key(cmd.id) {
                return Err(BackendError::VersionConflict);
            }
            let now = Utc::now();
            inner.tables.insert_outbox(
                cmd.address,
                now,
                cmd.metadata,
                notifications.iter().cloned(),
            );
            if let (Some(handler), Some(ns)) = (&self.handler, NonEmpty::from_vec(notifications)) {
                handler(&ns)?;
            }
            inner.tables.insert_command(cmd.id, cmd.address, cmd.time);
        }
        self.store.updates.notify_state();
        self.store.updates.notify_outbox();
        Ok(())
    }

    async fn notify(
        &self,
        cmd: CommandRef<'_>,
        notifications: NonEmpty<N>,
    ) -> Result<(), BackendError> {
        lock(&self.store.inner).tables.insert_outbox(
            cmd.address,
            Utc::now(),
            cmd.metadata,
            notifications,
        );
        self.store.updates.notify_outbox();
        Ok(())
    }
}

struct InMemoryOutboxReader<S, N>(Arc<InMemoryStateStore<S, N>>);

#[async_trait]
impl<S: Payload, N: Payload> OutboxReader<N> for InMemoryOutboxReader<S, N> {
    fn read(&self) -> BoxStream<'_, Result<OutboxItem<N>, BackendError>> {
        let items = lock(&self.0.inner).tables.unpublished();
        Box::pin(futures::stream::iter(items.into_iter().map(Ok)))
    }

    async fn mark_all_as_sent(&self, items: &NonEmpty<OutboxItem<N>>) -> Result<(), BackendError> {
        lock(&self.0.inner).tables.mark_published(items, Utc::now());
        Ok(())
    }
}
