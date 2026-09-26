//! In-memory event-sourcing storage.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::Utc;
use edomata_core::NonEmpty;
use futures::stream::BoxStream;
use uuid::Uuid;

use super::{InMemoryDriver, Tables, lock};
use crate::eventsourcing::{
    CommandState, JournalReader, JournalRepositoryReader, Notifications, NotificationsPublisher,
    Repository, RepositoryReader, SnapshotItem, SnapshotPersistence, SnapshotStore, Storage,
    StorageDriver, ValidState, as_reader, dedup,
};
use crate::{
    BackendError, CommandRef, EventMessage, EventMetadata, EventVersion, OutboxItem, OutboxReader,
    Payload, SeqNr, SharedModel, StreamId,
};

/// In-memory journal, outbox, commands and snapshots for one aggregate type.
///
/// Create one with [`InMemoryEventStore::new`], optionally seed it, and pass
/// it to [`InMemoryDriver::with_event_store`].
pub struct InMemoryEventStore<S, E, N> {
    inner: Mutex<Inner<E, N>>,
    snapshots: Arc<InMemorySnapshotPersistence<S>>,
    updates: Notifications,
}

struct Inner<E, N> {
    journal: Vec<EventMessage<E>>,
    next_seq: SeqNr,
    tables: Tables<N>,
}

impl<S, E, N> Default for InMemoryEventStore<S, E, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S, E, N> InMemoryEventStore<S, E, N> {
    /// An empty store.
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                journal: Vec::new(),
                next_seq: 1,
                tables: Tables::default(),
            }),
            snapshots: Arc::new(InMemorySnapshotPersistence::default()),
            updates: Notifications::new(),
        }
    }

    /// The snapshot persistence of this store.
    pub fn snapshots(&self) -> Arc<InMemorySnapshotPersistence<S>> {
        Arc::clone(&self.snapshots)
    }

    /// Adds journal rows as they are (sequence numbers and versions are not
    /// checked).
    pub fn seed_journal(&self, events: impl IntoIterator<Item = EventMessage<E>>) {
        let mut inner = lock(&self.inner);
        for ev in events {
            inner.next_seq = inner.next_seq.max(ev.metadata.seq_nr + 1);
            inner.journal.push(ev);
        }
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

    /// Adds persisted snapshots.
    pub fn seed_snapshots(&self, items: impl IntoIterator<Item = SnapshotItem<S>>)
    where
        S: Clone,
    {
        self.snapshots.insert_all(items);
    }

    /// The update signals of this store.
    pub fn updates(&self) -> &Notifications {
        &self.updates
    }

    /// Every journal row, by sequence number.
    pub fn journal_rows(&self) -> Vec<EventMessage<E>>
    where
        E: Clone,
    {
        let inner = lock(&self.inner);
        let mut rows = inner.journal.clone();
        rows.sort_by_key(|e| e.metadata.seq_nr);
        rows
    }
}

impl StorageDriver for InMemoryDriver {
    type Codec<T: 'static> = ();

    async fn build<S, E, R, N>(
        &self,
        model: SharedModel<S, E, R>,
        snapshot: Arc<dyn SnapshotStore<S>>,
        _event_codec: (),
        _notification_codec: (),
    ) -> Result<Storage<S, E, R, N>, BackendError>
    where
        S: Payload,
        E: Payload,
        R: Payload,
        N: Payload,
    {
        let store: Arc<InMemoryEventStore<S, E, N>> = match &self.store {
            None => Arc::new(InMemoryEventStore::new()),
            Some(_) => self.store_of()?,
        };
        let journal: Arc<dyn JournalReader<E>> =
            Arc::new(InMemoryJournalReader(Arc::clone(&store)));
        let reader: Arc<dyn RepositoryReader<S, E, R>> = Arc::new(JournalRepositoryReader::new(
            Arc::clone(&journal),
            as_reader(snapshot),
            model,
        ));
        let repository: Arc<dyn Repository<S, E, R, N>> = Arc::new(InMemoryRepository {
            store: Arc::clone(&store),
            reader: Arc::clone(&reader),
        });
        let outbox: Arc<dyn OutboxReader<N>> = Arc::new(InMemoryOutboxReader(Arc::clone(&store)));
        let updates = Arc::new(store.updates.clone());
        Ok(Storage {
            repository,
            reader,
            journal,
            outbox,
            updates,
        })
    }

    async fn snapshot<S>(
        &self,
        _state_codec: (),
    ) -> Result<Arc<dyn SnapshotPersistence<S>>, BackendError>
    where
        S: Payload,
    {
        Ok(match self.snapshots_of::<S>() {
            Some(seeded) => seeded,
            None => Arc::new(InMemorySnapshotPersistence::default()),
        })
    }
}

struct InMemoryRepository<S, E, R, N> {
    store: Arc<InMemoryEventStore<S, E, N>>,
    reader: Arc<dyn RepositoryReader<S, E, R>>,
}

#[async_trait]
impl<S: Payload, E: Payload, R: Payload, N: Payload> Repository<S, E, R, N>
    for InMemoryRepository<S, E, R, N>
{
    async fn load(&self, cmd: CommandRef<'_>) -> Result<CommandState<S, E, R>, BackendError> {
        let known = lock(&self.store.inner).tables.commands.contains_key(cmd.id);
        if known {
            return Ok(CommandState::Redundant);
        }
        Ok(CommandState::Aggregate(self.reader.get(cmd.address).await?))
    }

    async fn append(
        &self,
        cmd: CommandRef<'_>,
        version: SeqNr,
        _new_state: S,
        events: NonEmpty<E>,
        notifications: Vec<N>,
    ) -> Result<(), BackendError> {
        {
            let mut inner = lock(&self.store.inner);
            let now = Utc::now();
            let conflict = (0..events.len() as SeqNr).any(|i| {
                inner
                    .journal
                    .iter()
                    .any(|e| e.metadata.stream == cmd.address && e.metadata.version == version + i)
            });
            if conflict || inner.tables.commands.contains_key(cmd.id) {
                return Err(BackendError::VersionConflict);
            }
            for (i, payload) in events.into_iter().enumerate() {
                let seq_nr = inner.next_seq;
                inner.next_seq += 1;
                inner.journal.push(EventMessage {
                    metadata: EventMetadata {
                        id: Uuid::new_v4(),
                        time: now,
                        seq_nr,
                        version: version + i as EventVersion,
                        stream: cmd.address.to_owned(),
                    },
                    payload,
                });
            }
            inner
                .tables
                .insert_outbox(cmd.address, now, cmd.metadata, notifications);
            inner.tables.insert_command(cmd.id, cmd.address, cmd.time);
        }
        self.store.updates.notify_journal();
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

struct InMemoryJournalReader<S, E, N>(Arc<InMemoryEventStore<S, E, N>>);

impl<S: Payload, E: Payload, N: Payload> InMemoryJournalReader<S, E, N> {
    fn select(
        &self,
        pred: impl Fn(&EventMetadata) -> bool,
        by_version: bool,
    ) -> BoxStream<'_, Result<EventMessage<E>, BackendError>> {
        let inner = lock(&self.0.inner);
        let mut rows: Vec<EventMessage<E>> = inner
            .journal
            .iter()
            .filter(|e| pred(&e.metadata))
            .cloned()
            .collect();
        if by_version {
            rows.sort_by_key(|e| e.metadata.version);
        } else {
            rows.sort_by_key(|e| e.metadata.seq_nr);
        }
        Box::pin(futures::stream::iter(rows.into_iter().map(Ok)))
    }
}

impl<S: Payload, E: Payload, N: Payload> JournalReader<E> for InMemoryJournalReader<S, E, N> {
    fn read_stream(&self, stream_id: &str) -> BoxStream<'_, Result<EventMessage<E>, BackendError>> {
        let id = stream_id.to_owned();
        self.select(move |m| m.stream == id, true)
    }

    fn read_stream_after(
        &self,
        stream_id: &str,
        version: EventVersion,
    ) -> BoxStream<'_, Result<EventMessage<E>, BackendError>> {
        let id = stream_id.to_owned();
        self.select(move |m| m.stream == id && m.version > version, true)
    }

    fn read_stream_before(
        &self,
        stream_id: &str,
        version: EventVersion,
    ) -> BoxStream<'_, Result<EventMessage<E>, BackendError>> {
        let id = stream_id.to_owned();
        self.select(move |m| m.stream == id && m.version < version, true)
    }

    fn read_all(&self) -> BoxStream<'_, Result<EventMessage<E>, BackendError>> {
        self.select(|_| true, false)
    }

    fn read_all_after(
        &self,
        seq_nr: SeqNr,
    ) -> BoxStream<'_, Result<EventMessage<E>, BackendError>> {
        self.select(move |m| m.seq_nr > seq_nr, false)
    }

    fn read_all_before(
        &self,
        seq_nr: SeqNr,
    ) -> BoxStream<'_, Result<EventMessage<E>, BackendError>> {
        self.select(move |m| m.seq_nr < seq_nr, false)
    }
}

struct InMemoryOutboxReader<S, E, N>(Arc<InMemoryEventStore<S, E, N>>);

#[async_trait]
impl<S: Payload, E: Payload, N: Payload> OutboxReader<N> for InMemoryOutboxReader<S, E, N> {
    fn read(&self) -> BoxStream<'_, Result<OutboxItem<N>, BackendError>> {
        let items = lock(&self.0.inner).tables.unpublished();
        Box::pin(futures::stream::iter(items.into_iter().map(Ok)))
    }

    async fn mark_all_as_sent(&self, items: &NonEmpty<OutboxItem<N>>) -> Result<(), BackendError> {
        lock(&self.0.inner).tables.mark_published(items, Utc::now());
        Ok(())
    }
}

/// In-memory [`SnapshotPersistence`]: a map guarded by a mutex.
#[derive(Debug)]
pub struct InMemorySnapshotPersistence<S> {
    items: Mutex<HashMap<StreamId, ValidState<S>>>,
}

impl<S> Default for InMemorySnapshotPersistence<S> {
    fn default() -> Self {
        Self {
            items: Mutex::new(HashMap::new()),
        }
    }
}

impl<S: Clone> InMemorySnapshotPersistence<S> {
    /// An empty persistence.
    pub fn new() -> Self {
        Self::default()
    }

    /// Every persisted snapshot.
    pub fn all(&self) -> HashMap<StreamId, ValidState<S>> {
        lock(&self.items).clone()
    }

    /// Adds snapshots synchronously (used to seed test data).
    pub fn insert_all(&self, items: impl IntoIterator<Item = SnapshotItem<S>>) {
        lock(&self.items).extend(items);
    }
}

#[async_trait]
impl<S: Payload> SnapshotPersistence<S> for InMemorySnapshotPersistence<S> {
    async fn get(&self, id: &str) -> Result<Option<ValidState<S>>, BackendError> {
        Ok(lock(&self.items).get(id).cloned())
    }

    async fn put(&self, items: Vec<SnapshotItem<S>>) -> Result<(), BackendError> {
        let mut map = lock(&self.items);
        for (id, state) in dedup(items) {
            map.insert(id, state);
        }
        Ok(())
    }
}
